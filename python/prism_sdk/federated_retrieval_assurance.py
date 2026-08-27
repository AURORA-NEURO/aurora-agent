"""Federated continual retrieval assurance for ``AFA-cli-P02-F28``.

The adapter consumes local and peer-produced summaries only.  It never performs network
retrieval, moves raw evidence, or upgrades unknown peer state into a qualified exchange.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib, json, re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError
from .retrieval_synthesis_assurance import assure_retrieval_synthesis

FEATURE_ID = "AFA-cli-P02-F28"
CONTRACT_VERSION = "cli-federated-continual-retrieval-synthesis-assurance/1.0"
INPUT_SCHEMA = "ScopedRetrievalQuery4@1"
OUTPUT_SCHEMA = "EvidenceSynthesis7@1"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))

@dataclass(frozen=True)
class FederatedRetrievalAssuranceReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; corpus_id: str; scope: str; query: str; disposition: str
    candidate_order: tuple[str, ...]; rank_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    required_source_order: tuple[str, ...]; observed_source_order: tuple[str, ...]; missing_source_order: tuple[str, ...]; stale_order: tuple[str, ...]; contradiction_order: tuple[str, ...]
    peer_order: tuple[str, ...]; qualified_peer_order: tuple[str, ...]; missing_peer_order: tuple[str, ...]; blocked_peer_order: tuple[str, ...]; checks: tuple[str, ...]
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; replay_identity: str; local_evidence_digest: str; federation_envelope_digest: str
    artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.corpus_id, self.scope, self.query)) or not self.candidate_order or len(self.rank_order) != len(self.candidate_order) or not self.peer_order or not self.checks or not self.effect_receipts:
            raise ResearchContractError("federated retrieval identity, locality, candidates, peers, checks, or effects are incomplete")
        for values in (self.candidate_order, self.selected_order, self.unresolved_order, self.blocked_order, self.required_source_order, self.observed_source_order, self.missing_source_order, self.stale_order, self.contradiction_order, self.peer_order, self.qualified_peer_order, self.missing_peer_order, self.blocked_peer_order, self.checks, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(list(values)):
                raise ResearchContractError("federated retrieval orders and evidence annotations are not canonical")
        if set(self.rank_order) != set(self.candidate_order):
            raise ResearchContractError("federated retrieval rank order is not a candidate permutation")
        candidate_partition = list(self.selected_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(candidate_partition) != set(self.candidate_order) or len(candidate_partition) != len(set(candidate_partition)):
            raise ResearchContractError("federated retrieval dispositions do not partition candidates")
        peer_partition = list(self.qualified_peer_order) + list(self.missing_peer_order) + list(self.blocked_peer_order)
        if set(peer_partition) != set(self.peer_order) or len(peer_partition) != len(set(peer_partition)):
            raise ResearchContractError("federated retrieval peer states do not partition peers")
        if any(not effect.startswith("exchange:aggregate-evidence:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("federated retrieval effect is outside aggregate-only exchange gate")
        for digest in (self.replay_identity, self.local_evidence_digest, self.federation_envelope_digest, self.artifact.get("content_hash")):
            if not _digest(digest):
                raise ResearchContractError("federated retrieval digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.federated-evidence-synthesis+json":
            raise ResearchContractError("federated retrieval artifact type is invalid")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value

def assure_federated_retrieval(*, request: Mapping[str, Any]) -> FederatedRetrievalAssuranceReceipt:
    required = ("request_id", "corpus_id", "scope", "query", "federation_id", "purpose", "origin_institution", "semantic_profile")
    if any(not str(request.get(field, "")).strip() for field in required) or request.get("query_schema") != INPUT_SCHEMA or not request.get("candidates") or not request.get("required_source_ids") or not request.get("peer_institution_order") or not request.get("peer_evidence") or int(request.get("required_peer_quorum", 0)) <= 0 or int(request.get("required_peer_quorum", 0)) > len(request["peer_institution_order"]) or not request.get("permitted_artifact_order") or "evidence-synthesis" not in request["permitted_artifact_order"] or not _digest(request.get("replay_identity")) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("federated retrieval identity, peer quorum, allow-list, replay, locality, or boundary is invalid")
    for field in ("required_source_ids", "peer_institution_order", "permitted_artifact_order"):
        values = [str(item) for item in request[field]]
        if not _canonical(values) or any(not value.strip() for value in values):
            raise ResearchContractError("federated retrieval declarations are not canonical")
    peer_ids = [str(item) for item in request["peer_institution_order"]]
    peers = sorted(request["peer_evidence"], key=lambda item: str(item.get("institution_id", "")))
    ids = [str(peer.get("institution_id", "")) for peer in peers]
    if not all(ids) or len(ids) != len(set(ids)) or set(ids) != set(peer_ids):
        raise ResearchContractError("peer identities must cover the declared federation in canonical order")
    qualified: set[str] = set(); missing: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for peer in peers:
        pid = str(peer["institution_id"]); state = str(peer.get("evidence_state", "unknown")); hard = str(peer.get("permitted_artifact", "")) != "evidence-synthesis" or str(peer.get("semantic_profile", "")) != str(request["semantic_profile"]) or str(peer.get("replay_identity", "")) != str(request["replay_identity"]) or not _digest(peer.get("artifact_digest")) or not _digest(peer.get("provenance_digest")) or state == "contradicted"
        if not _digest(peer.get("artifact_digest")): omissions.add(f"peer:{pid}:artifact-digest-missing")
        if not _digest(peer.get("provenance_digest")): omissions.add(f"peer:{pid}:provenance-digest-missing")
        if str(peer.get("semantic_profile", "")) != str(request["semantic_profile"]): omissions.add(f"peer:{pid}:semantic-profile-mismatch")
        if str(peer.get("replay_identity", "")) != str(request["replay_identity"]): omissions.add(f"peer:{pid}:replay-mismatch")
        for item in peer.get("omissions", []): omissions.add(f"peer:{pid}:{item}")
        for item in peer.get("uncertainty", []): uncertainty.add(f"peer:{pid}:{item}")
        if state in {"unknown", "speculative"}: uncertainty.add(f"peer:{pid}:evidence-state")
        negative.add(f"peer:{pid}:{'negative-result' if peer.get('negative_result') else 'negative-result-not-observed'}")
        if hard: blocked.add(pid)
        elif state in {"proven", "supported"}: qualified.add(pid)
        else: missing.add(pid)
    if len(qualified) < int(request["required_peer_quorum"]):
        omissions.add(f"peer-quorum:{len(qualified)}/{request['required_peer_quorum']}"); uncertainty.add("federation:peer-quorum-incomplete")
    if request.get("policy_allow") is not True: omissions.add("federation:policy-denied")
    if request.get("protected_closure") is not True: omissions.add("federation:protected-closure-incomplete")
    if request.get("signed_approval") is not True: omissions.add("federation:signed-approval-missing")
    omissions.update(f"federation:adversarial:{event}" for event in request.get("adversarial_events", []))
    local_request = dict(request); local_request["query_schema"] = "ScopedRetrievalQuery3@1"; local_request.pop("federation_id", None); local_request.pop("purpose", None); local_request.pop("origin_institution", None); local_request.pop("peer_institution_order", None); local_request.pop("required_peer_quorum", None); local_request.pop("peer_evidence", None); local_request.pop("semantic_profile", None); local_request.pop("permitted_artifact_order", None); local_request.pop("signed_approval", None)
    local = assure_retrieval_synthesis(request=local_request)
    global_block = request.get("policy_allow") is not True or request.get("protected_closure") is not True or request.get("signed_approval") is not True or bool(request.get("adversarial_events")) or bool(blocked)
    quorum_ok = len(qualified) >= int(request["required_peer_quorum"])
    disposition = "blocked" if global_block or local.disposition == "blocked" else "unresolved" if local.disposition == "unresolved" or not quorum_ok else "qualified"
    semantic_loss = list(local.artifact.get("semantic_loss", []))
    if global_block: semantic_loss.append({"field": "federation", "reason": "peer, policy, approval, or adversarial gate blocks aggregate exchange", "severity": "decision_relevant"})
    peer_order = tuple(peer_ids); qualified_order = tuple(sorted(qualified)); missing_order = tuple(sorted(missing)); blocked_order = tuple(sorted(blocked)); exchange_payload = {"schema_version": OUTPUT_SCHEMA, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "origin_institution": request["origin_institution"], "peer_order": list(peer_order), "qualified_peer_order": list(qualified_order), "local_evidence_digest": local.evidence_digest, "replay_identity": request["replay_identity"], "semantic_profile": request["semantic_profile"], "disposition": disposition, "raw_data_local": True}; federation_digest = _hash(exchange_payload)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "corpus_id": request["corpus_id"], "scope": request["scope"], "query": request["query"], "candidate_order": list(local.candidate_order), "rank_order": list(local.rank_order), "selected_order": list(local.selected_order), "unresolved_order": list(local.unresolved_order), "blocked_order": list(local.blocked_order), "peer_order": list(peer_order), "qualified_peer_order": list(qualified_order), "missing_peer_order": list(missing_order), "blocked_peer_order": list(blocked_order), "local_evidence_digest": local.evidence_digest, "federation_envelope_digest": federation_digest, "replay_identity": request["replay_identity"], "disposition": disposition, "boundary": PRECLINICAL_BOUNDARY}; artifact_digest = _hash(payload)
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"federated-evidence-synthesis:{request['request_id']}", "content_type": "application/vnd.aurora.federated-evidence-synthesis+json", "content_hash": artifact_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": str(request["federation_id"]), "relation": "federated-retrieval-synthesis-assurance", "digest": artifact_digest}], "boundary": PRECLINICAL_BOUNDARY}
    checks = tuple(sorted({"schema-version", "local-retrieval-verdict", "peer-artifact-allow-list", "peer-provenance-closure", "peer-semantic-profile", "peer-replay-identity", "peer-quorum", "aggregate-only-locality", "signed-approval", "negative-evidence-retention"}))
    receipt = FederatedRetrievalAssuranceReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["corpus_id"]), str(request["scope"]), str(request["query"]), disposition, tuple(local.candidate_order), tuple(local.rank_order), tuple(local.selected_order), tuple(local.unresolved_order), tuple(local.blocked_order), tuple(local.required_source_order), tuple(local.observed_source_order), tuple(local.missing_source_order), tuple(local.stale_order), tuple(local.contradiction_order), peer_order, qualified_order, missing_order, blocked_order, checks, tuple(sorted(omissions | set(local.omissions))), tuple(sorted(uncertainty | set(local.uncertainty))), tuple(sorted(negative | set(local.negative_evidence))), str(request["replay_identity"]), str(local.evidence_digest), federation_digest, artifact, (f"exchange:aggregate-evidence:{request['federation_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedRetrievalAssuranceReceipt", "assure_federated_retrieval"]
