"""MCP federated continual multimodal-ingestion assurance (``AFA-mcp-P06-F28``).

The SDK transports typed modality attestations and aggregate-only peer summaries.  It never
loads raw imaging/omics bytes, performs harmonization, or turns an unresolved/contradictory
record into a qualified research conclusion.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-mcp-P06-F28"
CONTRACT_VERSION = "mcp-federated-continual-multimodal-ingestion-assurance-harness/1.0"
INPUT_SCHEMA = "RawModalityBundle4@1"
OUTPUT_SCHEMA = "HarmonizedResearchObject7@1"
CONTENT_TYPE = "application/vnd.aurora.mcp-harmonized-research-object-7+json"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str]) -> bool:
    return values == sorted(set(values))


@dataclass(frozen=True)
class HarmonizedResearchObjectReceipt:
    schema_version: str
    contract_version: str
    feature_id: str
    request_id: str
    federation_id: str
    semantic_profile: str
    disposition: str
    modality_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    unresolved_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    missing_modality_order: tuple[str, ...]
    peer_order: tuple[str, ...]
    qualified_peer_order: tuple[str, ...]
    missing_peer_order: tuple[str, ...]
    omission_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_evidence_order: tuple[str, ...]
    semantic_loss_order: tuple[str, ...]
    replay_identity: str
    harmonization_digest: str
    artifact: Mapping[str, Any]
    effect_receipts: tuple[str, ...]
    raw_data_local: bool
    aggregate_only: bool
    boundary: str

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple):
                value[key] = list(item)
        return value

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.artifact.get("boundary") != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or not all(isinstance(v, str) and v.strip() for v in (self.request_id, self.federation_id, self.semantic_profile)) or not self.modality_order or not self.peer_order or not self.effect_receipts:
            raise ResearchContractError("MCP ingestion identity, locality, modalities, peers, or effects are incomplete")
        for values in (self.modality_order, self.selected_order, self.unresolved_order, self.blocked_order, self.missing_modality_order, self.peer_order, self.qualified_peer_order, self.missing_peer_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.semantic_loss_order, self.effect_receipts):
            if not _canonical(list(values)):
                raise ResearchContractError("MCP ingestion ordering is not canonical")
        if set(self.selected_order) | set(self.unresolved_order) | set(self.blocked_order) != set(self.modality_order) or len(self.selected_order) + len(self.unresolved_order) + len(self.blocked_order) != len(set(self.modality_order)):
            raise ResearchContractError("MCP modality states do not partition modalities")
        if set(self.qualified_peer_order) | set(self.missing_peer_order) != set(self.peer_order) or len(self.qualified_peer_order) + len(self.missing_peer_order) != len(set(self.peer_order)):
            raise ResearchContractError("MCP peer states do not partition peers")
        if not all(_digest(v) for v in (self.replay_identity, self.harmonization_digest, self.artifact.get("content_hash"))):
            raise ResearchContractError("MCP ingestion digest is invalid")
        if self.artifact.get("content_type") != CONTENT_TYPE:
            raise ResearchContractError("MCP ingestion artifact type is invalid")
        expected = [f"verify:mcp-multimodal-ingestion:{self.request_id}"] if self.disposition == "qualified" else ["block:unsafe-release"]
        if list(self.effect_receipts) != expected:
            raise ResearchContractError("MCP ingestion effect is invalid")


def assure_multimodal_ingestion(*, request: Mapping[str, Any]) -> HarmonizedResearchObjectReceipt:
    required = ("request_id", "federation_id", "semantic_profile")
    if any(not str(request.get(k, "")).strip() for k in required) or request.get("schema_version") != RESEARCH_CONTRACT_SCHEMA_VERSION or request.get("boundary") != PRECLINICAL_BOUNDARY or not request.get("required_modality_order") or not _canonical([str(v) for v in request["required_modality_order"]]) or not request.get("modalities") or not request.get("peers") or int(request.get("minimum_peer_quorum", 0)) <= 0 or int(request.get("minimum_peer_quorum", 0)) > len(request["peers"]) or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True:
        raise ResearchContractError("MCP ingestion identity, closure, quorum, replay, locality, or boundary is invalid")
    modalities = sorted(request["modalities"], key=lambda item: str(item.get("modality_id", "")))
    modality_order = [str(m.get("modality_id", "")) for m in modalities]
    if not all(modality_order) or len(modality_order) != len(set(modality_order)):
        raise ResearchContractError("modality identities must be unique and non-empty")
    selected: set[str] = set(); unresolved: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); semantic_loss: set[str] = set()
    for modality in modalities:
        mid = str(modality["modality_id"]); state = str(modality.get("state", "unknown")); prefix = f"{mid}:"
        if modality.get("negative_result") is True: negative.add(prefix + "negative-result")
        omissions.update(prefix + str(v) for v in modality.get("omissions", [])); uncertainty.update(prefix + str(v) for v in modality.get("uncertainty", []))
        malformed = any(not _digest(modality.get(k)) for k in ("content_digest", "provenance_digest", "qc_digest", "replay_identity")) or str(modality.get("semantic_profile", "")) != str(request["semantic_profile"]) or str(modality.get("replay_identity", "")) != str(request["replay_identity"]) or not str(modality.get("schema_version", "")).strip()
        if state == "contradicted" or modality.get("local_only") is not True or modality.get("permitted") is not True or modality.get("raw_bytes_carried") is True: blocked.add(mid)
        elif malformed or state == "unmeasured": unresolved.add(mid); semantic_loss.add(prefix + "unmeasured-or-unverified")
        elif state == "unknown" or modality.get("omissions") or modality.get("uncertainty"): unresolved.add(mid)
        else: selected.add(mid)
    required_ids = {str(v) for v in request["required_modality_order"]}; missing = required_ids - set(modality_order)
    omissions.update(f"{mid}:required-modality-missing" for mid in missing)
    peers = sorted(request["peers"], key=lambda item: str(item.get("institution_id", ""))); peer_order = [str(p.get("institution_id", "")) for p in peers]
    if not all(peer_order) or len(peer_order) != len(set(peer_order)): raise ResearchContractError("peer identities must be unique and non-empty")
    qualified_peers: set[str] = set(); missing_peers: set[str] = set()
    for peer in peers:
        pid = str(peer["institution_id"])
        if peer.get("signed") is True and peer.get("permitted") is True and peer.get("aggregate_only") is True and str(peer.get("semantic_profile", "")) == str(request["semantic_profile"]) and str(peer.get("replay_identity", "")) == str(request["replay_identity"]) and _digest(peer.get("harmonization_digest")): qualified_peers.add(pid)
        else: missing_peers.add(pid)
    if len(qualified_peers) < int(request["minimum_peer_quorum"]): uncertainty.add("request:peer-quorum-incomplete")
    if request.get("policy_allow") is not True: negative.add("request:policy-denied")
    if request.get("protected_closure") is not True: uncertainty.add("request:protected-closure-incomplete")
    if request.get("signed_approval") is not True: uncertainty.add("request:signed-approval-missing")
    if request.get("federation_approved") is not True: uncertainty.add("request:federation-approval-missing")
    negative.update(f"adversarial:{v}" for v in request.get("adversarial_events", []))
    global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or request.get("federation_approved") is not True or request.get("raw_data_local") is not True or request.get("aggregate_only") is not True or bool(request.get("adversarial_events"))
    if global_block: blocked.update(modality_order); selected.clear(); unresolved.clear(); omissions.add("request:mcp-multimodal-release-gate-blocked")
    required_block = bool(required_ids & blocked)
    disposition = "blocked" if global_block or required_block else "qualified" if required_ids <= selected and not missing and len(qualified_peers) >= int(request["minimum_peer_quorum"]) and not unresolved and not blocked else "unresolved"
    selected_order, unresolved_order, blocked_order, missing_order = sorted(selected), sorted(unresolved), sorted(blocked), sorted(missing)
    qualified_peer_order, missing_peer_order = sorted(qualified_peers), sorted(missing_peers)
    omission_order, uncertainty_order, negative_order, loss_order = sorted(omissions), sorted(uncertainty), sorted(negative), sorted(semantic_loss)
    effects = [f"verify:mcp-multimodal-ingestion:{request['request_id']}"] if disposition == "qualified" else ["block:unsafe-release"]
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": str(request["request_id"]), "federation_id": str(request["federation_id"]), "semantic_profile": str(request["semantic_profile"]), "disposition": disposition, "modality_order": modality_order, "selected_order": selected_order, "unresolved_order": unresolved_order, "blocked_order": blocked_order, "missing_modality_order": missing_order, "peer_order": peer_order, "qualified_peer_order": qualified_peer_order, "missing_peer_order": missing_peer_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_order, "semantic_loss_order": loss_order, "replay_identity": str(request["replay_identity"]), "effect_receipts": effects, "raw_data_local": True, "aggregate_only": True, "boundary": PRECLINICAL_BOUNDARY}
    harmonization_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"mcp-harmonized-research-object:{request['request_id']}", "content_type": CONTENT_TYPE, "content_hash": harmonization_digest, "semantic_loss": [], "provenance": [], "boundary": PRECLINICAL_BOUNDARY}
    receipt = HarmonizedResearchObjectReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["semantic_profile"]), disposition, tuple(modality_order), tuple(selected_order), tuple(unresolved_order), tuple(blocked_order), tuple(missing_order), tuple(peer_order), tuple(qualified_peer_order), tuple(missing_peer_order), tuple(omission_order), tuple(uncertainty_order), tuple(negative_order), tuple(loss_order), str(request["replay_identity"]), harmonization_digest, artifact, tuple(effects), True, True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


def validate_multimodal_ingestion_receipt(value: Mapping[str, Any]) -> HarmonizedResearchObjectReceipt:
    tuples = {k: tuple(value.get(k, [])) for k in ("modality_order", "selected_order", "unresolved_order", "blocked_order", "missing_modality_order", "peer_order", "qualified_peer_order", "missing_peer_order", "omission_order", "uncertainty_order", "negative_evidence_order", "semantic_loss_order", "effect_receipts")}
    receipt = HarmonizedResearchObjectReceipt(*(value.get(k) for k in ("schema_version", "contract_version", "feature_id", "request_id", "federation_id", "semantic_profile", "disposition")), **tuples, replay_identity=value.get("replay_identity"), harmonization_digest=value.get("harmonization_digest"), artifact=value.get("artifact", {}), raw_data_local=value.get("raw_data_local"), aggregate_only=value.get("aggregate_only"), boundary=value.get("boundary"))
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "HarmonizedResearchObjectReceipt", "assure_multimodal_ingestion", "validate_multimodal_ingestion_receipt"]
