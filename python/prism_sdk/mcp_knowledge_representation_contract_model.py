"""Federated continual typed-knowledge representation (``AFA-mcp-P04-F08``).

Only typed claim attestations and aggregate peer summaries cross this SDK boundary. Raw study
records stay local, and unknown, speculative, contradictory, omitted, and policy-denied claims
remain visible rather than being promoted to a knowledge-world assertion.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-mcp-P04-F08"
CONTRACT_VERSION = "mcp-federated-continual-knowledge-representation-contract-model/1.0"
INPUT_SCHEMA = "ScopedResearchClaims4@1"
OUTPUT_SCHEMA = "TypedKnowledgeWorld2@1"
CONTENT_TYPE = "application/vnd.aurora.mcp-typed-knowledge-world-2+json"

def _hash(value: Any) -> str: return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool: return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: list[str]) -> bool: return values == sorted(set(values))

@dataclass(frozen=True)
class TypedKnowledgeWorldReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; semantic_profile: str; disposition: str
    claim_order: tuple[str, ...]; selected_claim_order: tuple[str, ...]; unresolved_claim_order: tuple[str, ...]; blocked_claim_order: tuple[str, ...]; missing_claim_order: tuple[str, ...]
    peer_order: tuple[str, ...]; qualified_peer_order: tuple[str, ...]; missing_peer_order: tuple[str, ...]; omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]
    replay_identity: str; world_digest: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; aggregate_only: bool; boundary: str
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or not self.request_id.strip() or not self.federation_id.strip() or not self.semantic_profile.strip() or not self.claim_order or not self.peer_order or not self.effect_receipts: raise ResearchContractError("knowledge identity, claims, peers, locality, or effects are incomplete")
        for values in (self.claim_order, self.selected_claim_order, self.unresolved_claim_order, self.blocked_claim_order, self.missing_claim_order, self.peer_order, self.qualified_peer_order, self.missing_peer_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.effect_receipts):
            if not _canonical(list(values)): raise ResearchContractError("knowledge ordering is not canonical")
        ids = set(self.claim_order); parts = list(self.selected_claim_order) + list(self.unresolved_claim_order) + list(self.blocked_claim_order)
        if set(parts) != ids or len(parts) != len(ids): raise ResearchContractError("knowledge claim states do not partition claims")
        peers = set(self.peer_order); peer_parts = list(self.qualified_peer_order) + list(self.missing_peer_order)
        if set(peer_parts) != peers or len(peer_parts) != len(peers): raise ResearchContractError("knowledge peer states do not partition peers")
        if not all(_digest(v) for v in (self.replay_identity, self.world_digest, self.artifact.get("content_hash"))): raise ResearchContractError("knowledge digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY: raise ResearchContractError("knowledge artifact metadata is invalid")
        expected = [f"read:local-knowledge-world:{self.request_id}"] if self.disposition == "qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts) != expected: raise ResearchContractError("knowledge effect is invalid")

def model_knowledge_representation(*, request: Mapping[str, Any]) -> TypedKnowledgeWorldReceipt:
    if any(not str(request.get(k, "")).strip() for k in ("request_id", "federation_id", "semantic_profile")) or request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("required_claim_order") or not _canonical([str(v) for v in request["required_claim_order"]]) or not request.get("claims") or not request.get("peers") or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) > len(request["peers"]) or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True: raise ResearchContractError("knowledge identity, closure, quorum, replay, locality, or boundary is invalid")
    claims = sorted(request["claims"], key=lambda item: str(item.get("claim_id", ""))); claim_order = [str(c.get("claim_id", "")) for c in claims]
    if not all(claim_order) or len(set(claim_order)) != len(claim_order): raise ResearchContractError("claim identities must be unique and non-empty")
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for claim in claims:
        cid = str(claim["claim_id"]); state = str(claim.get("evidence_state", "unknown")); omissions.update(f"{cid}:{v}" for v in claim.get("omissions", [])); uncertainty.update(f"{cid}:{v}" for v in claim.get("uncertainty", []))
        if claim.get("negative_result") is True: negative.add(f"{cid}:negative-result")
        if state == "contradicted" or claim.get("local_only") is not True or claim.get("permitted") is not True: blocked.add(cid)
        elif state in {"unknown", "speculative"} or claim.get("omissions") or claim.get("uncertainty"): unresolved.add(cid)
        else: selected.add(cid)
    required = {str(v) for v in request["required_claim_order"]}; missing = required - set(claim_order); omissions.update(f"{v}:required-claim-missing" for v in missing)
    peers = sorted(request["peers"], key=lambda item: str(item.get("institution_id", ""))); peer_order = [str(p.get("institution_id", "")) for p in peers]
    if not all(peer_order) or len(set(peer_order)) != len(peer_order): raise ResearchContractError("peer identities must be unique and non-empty")
    qualified_peers: set[str] = set(); missing_peers: set[str] = set()
    for peer in peers:
        if peer.get("signed") is True and peer.get("permitted") is True and peer.get("aggregate_only") is True and str(peer.get("semantic_profile", "")) == str(request["semantic_profile"]) and str(peer.get("replay_identity", "")) == str(request["replay_identity"]) and _digest(peer.get("knowledge_digest")): qualified_peers.add(str(peer["institution_id"]))
        else: missing_peers.add(str(peer["institution_id"]))
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("request:peer-quorum-incomplete")
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("federation_approved") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True
    if global_block: blocked.update(claim_order); selected.clear(); unresolved.clear(); omissions.add("request:knowledge-release-gate-blocked")
    if required & blocked: disposition = "blocked"
    elif global_block: disposition = "blocked"
    elif required <= selected and not missing and not unresolved and not blocked and len(qualified_peers) >= int(request["minimum_peer_quorum"]): disposition = "qualified"
    else: disposition = "unresolved"
    selected_order, unresolved_order, blocked_order, missing_order = sorted(selected), sorted(unresolved), sorted(blocked), sorted(missing); qualified_order, missing_peer_order = sorted(qualified_peers), sorted(missing_peers); omission_order, uncertainty_order, negative_order = sorted(omissions), sorted(uncertainty), sorted(negative); effects = [f"read:local-knowledge-world:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": str(request["request_id"]), "federation_id": str(request["federation_id"]), "semantic_profile": str(request["semantic_profile"]), "disposition": disposition, "claim_order": claim_order, "selected_claim_order": selected_order, "unresolved_claim_order": unresolved_order, "blocked_claim_order": blocked_order, "missing_claim_order": missing_order, "peer_order": peer_order, "qualified_peer_order": qualified_order, "missing_peer_order": missing_peer_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_order, "replay_identity": str(request["replay_identity"]), "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}; digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"mcp-typed-knowledge-world:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}
    receipt = TypedKnowledgeWorldReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["semantic_profile"]), disposition, tuple(claim_order), tuple(selected_order), tuple(unresolved_order), tuple(blocked_order), tuple(missing_order), tuple(peer_order), tuple(qualified_order), tuple(missing_peer_order), tuple(omission_order), tuple(uncertainty_order), tuple(negative_order), str(request["replay_identity"]), digest, artifact, tuple(effects), True, True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "TypedKnowledgeWorldReceipt", "model_knowledge_representation"]
