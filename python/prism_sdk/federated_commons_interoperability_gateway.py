"""Python parity adapter for ``AFA-policy-P31-F24`` federated commons gateway."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-policy-P31-F24"
CONTRACT_VERSION = "policy-federated-continual-commons-interoperability-gateway/1.0"
INPUT_SCHEMA = "PolicyFederationRequest4@1"
OUTPUT_SCHEMA = "PolicyFederationEnvelope6@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class PolicyFederationEnvelope:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; institution_id: str; purpose: str; semantic_profile: str; admission: str
    origin_order: tuple[str, ...]; accepted_origin_order: tuple[str, ...]; candidate_order: tuple[str, ...]; admitted_order: tuple[str, ...]; conditional_order: tuple[str, ...]; blocked_order: tuple[str, ...]; unknown_order: tuple[str, ...]; decisions: tuple[Mapping[str, Any], ...]; replay_identity: str; envelope_digest: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; federation_export: str; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.federation_export != "aggregate-digest-only" or self.raw_data_local is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.federation_id, self.institution_id, self.purpose, self.semantic_profile)) or not self.candidate_order or len(self.decisions) != len(self.candidate_order) or not self.effect_receipts: raise ResearchContractError("federation identity, locality, candidate decisions, export mode, or effects are incomplete")
        for values in (self.origin_order, self.accepted_origin_order, self.candidate_order, self.admitted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("federation orders and evidence annotations are not canonical")
        partition = list(self.admitted_order) + list(self.conditional_order) + list(self.blocked_order) + list(self.unknown_order)
        if set(partition) != set(self.candidate_order) or len(partition) != len(set(partition)): raise ResearchContractError("federation admission states do not partition candidates")
        if tuple(str(item.get("artifact_id", "")) for item in self.decisions) != self.candidate_order: raise ResearchContractError("federation decisions do not match candidate order")
        if any(not effect.startswith("exchange:permitted-artifacts:") and not effect.startswith("approval-required:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("federation effect is outside permitted-artifact gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.federation-envelope+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("federation artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def admit_policy_federation(*, request: Mapping[str, Any]) -> PolicyFederationEnvelope:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "federation_id", "institution_id", "purpose", "semantic_profile")) or request.get("input_schema") != INPUT_SCHEMA or not request.get("allowed_artifact_types") or not request.get("candidates") or int(request.get("required_origin_quorum", 0)) <= 0 or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")): raise ResearchContractError("federation identity, artifacts, candidates, quorum, locality, replay, or boundary is invalid")
    allowed = [str(item) for item in request["allowed_artifact_types"]]
    if not _canonical(allowed) or any(not item.strip() for item in allowed): raise ResearchContractError("allowed artifact types are not canonical")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("artifact_id", ""))); ids = [str(item.get("artifact_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids): raise ResearchContractError("artifact identifiers are invalid")
    origins = sorted({str(item.get("origin_institution", "")) for item in candidates}); global_failed = {gate for gate, failed in (("policy", not request.get("policy_allow", False)), ("purpose-bound", not request.get("purpose_bound", False)), ("protected-closure", not request.get("protected_closure", False)), ("raw-data-locality", request.get("raw_data_local") is not True)) if failed}
    admitted: list[str] = []; conditional: list[str] = []; blocked: list[str] = []; unknown: list[str] = []; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []
    for candidate in candidates:
        aid = str(candidate["artifact_id"]); failed = set(global_failed); pending: set[str] = set(); unknown_state = False
        if str(candidate.get("artifact_type", "")) not in set(allowed): failed.add("artifact-type-not-allowed"); omissions.add(f"{aid}:artifact-type-not-allowed")
        if str(candidate.get("purpose", "")) != str(request["purpose"]): failed.add("purpose-mismatch"); omissions.add(f"{aid}:purpose-mismatch")
        if str(candidate.get("semantic_profile", "")) != str(request["semantic_profile"]): pending.add("semantic-profile-mismatch"); omissions.add(f"{aid}:semantic-profile-mismatch")
        if not candidate.get("permitted"): failed.add("candidate-not-permitted")
        if not candidate.get("raw_data_local"): failed.add("candidate-locality")
        if not candidate.get("content_digest"): pending.add("content-digest-missing"); omissions.add(f"{aid}:content-digest-missing")
        if not candidate.get("provenance_digest"): pending.add("provenance-missing"); omissions.add(f"{aid}:provenance-missing")
        for item in candidate.get("omissions", []): pending.add("candidate-omissions"); omissions.add(f"{aid}:{item}")
        for item in candidate.get("uncertainty", []): pending.add("candidate-uncertainty"); uncertainty.add(f"{aid}:{item}")
        state = str(candidate.get("evidence_state", "unknown"))
        if state == "contradicted": failed.add("contradicted-evidence"); negative.add(f"{aid}:contradicted")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state-not-qualified"); unknown_state = True; uncertainty.add(f"{aid}:evidence-state")
        negative.add(f"{aid}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        disposition = "blocked" if failed else "unknown" if unknown_state else "conditional" if pending else "admitted"
        (blocked if disposition == "blocked" else unknown if disposition == "unknown" else conditional if disposition == "conditional" else admitted).append(aid)
        decisions.append({"artifact_id": aid, "origin": str(candidate["origin_institution"]), "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))})
        if failed: semantic_loss.append({"field": f"artifact:{aid}", "reason": "artifact failed a policy or evidence federation gate", "severity": "decision_relevant"})
    admitted = sorted(set(admitted)); conditional = sorted(set(conditional)); blocked = sorted(set(blocked)); unknown = sorted(set(unknown)); accepted_origins = sorted({str(item["origin_institution"]) for item in candidates if str(item["artifact_id"]) in admitted})
    if len(accepted_origins) < int(request["required_origin_quorum"]): omissions.add(f"origin-quorum:{len(accepted_origins)}/{request['required_origin_quorum']}")
    if not global_failed and not blocked and not request.get("signed_approval", False) or (not global_failed and not blocked and not request.get("network_permitted", False)): admission = "approval_required"
    elif global_failed or blocked: admission = "blocked"
    elif conditional or unknown or len(accepted_origins) < int(request["required_origin_quorum"]): admission = "partial"
    elif admitted: admission = "admitted"
    else: admission = "unknown"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "institution_id": request["institution_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "admission": admission, "candidate_order": ids, "admitted_order": admitted, "conditional_order": conditional, "blocked_order": blocked, "unknown_order": unknown, "decisions": decisions, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}; envelope_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"federation-envelope:{request['request_id']}", "content_type": "application/vnd.aurora.federation-envelope+json", "content_hash": envelope_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": str(request["federation_id"]), "relation": "policy-federation-gateway", "digest": envelope_digest}], "boundary": PRECLINICAL_BOUNDARY}; effects = (f"exchange:permitted-artifacts:{request['federation_id']}",) if admission == "admitted" else (f"approval-required:{request['federation_id']}",) if admission == "approval_required" else ("block:unsafe-release",); receipt = PolicyFederationEnvelope(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["institution_id"]), str(request["purpose"]), str(request["semantic_profile"]), admission, tuple(origins), tuple(accepted_origins), tuple(ids), tuple(admitted), tuple(conditional), tuple(blocked), tuple(unknown), tuple(decisions), str(request["replay_identity"]), envelope_digest, tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, effects, True, "aggregate-digest-only", PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "PolicyFederationEnvelope", "admit_policy_federation"]
