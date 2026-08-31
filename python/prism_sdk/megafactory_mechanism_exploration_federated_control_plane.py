"""Python parity adapter for ``AFA-megafactory-P08-F32``."""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-megafactory-P08-F32"
FEATURE_VERSION = "megafactory-federated-continual-mechanism-control-plane/1.0"
INPUT_SCHEMA = "MechanismQuestion4@1"
OUTPUT_SCHEMA = "MechanismPortfolio8@1"


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


@dataclass(frozen=True)
class FederatedMechanismReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; semantic_profile: str; admission: str
    origin_order: tuple[str, ...]; admitted_origin_order: tuple[str, ...]; mechanism_order: tuple[str, ...]; rank_order: tuple[str, ...]; admitted_order: tuple[str, ...]; conditional_order: tuple[str, ...]; blocked_order: tuple[str, ...]; unknown_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]; checkpoint_seq: int; checkpoint_digest: str; control_digest: str; replay_identity: str; semantic_loss: tuple[Mapping[str, Any], ...]
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, FEATURE_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.mechanism_order or len(self.decisions) != len(self.mechanism_order) or not self.effect_receipts:
            raise ResearchContractError("mechanism federation identity, locality, candidates, decisions, or effects are incomplete")
        for values in (self.origin_order, self.admitted_origin_order, self.mechanism_order, self.admitted_order, self.conditional_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(values) != tuple(sorted(set(values))): raise ResearchContractError("mechanism federation ordering is not canonical")
        if tuple(str(d.get("mechanism_id", "")) for d in self.decisions) != self.mechanism_order or set(self.rank_order) != set(self.mechanism_order) or len(self.rank_order) != len(self.mechanism_order): raise ResearchContractError("mechanism ranking or decisions do not match candidates")
        if set(self.admitted_order) | set(self.conditional_order) | set(self.blocked_order) | set(self.unknown_order) != set(self.mechanism_order): raise ResearchContractError("mechanism dispositions do not partition candidates")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash"))): raise ResearchContractError("mechanism federation digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.federated-mechanism-portfolio+json": raise ResearchContractError("mechanism federation artifact type is invalid")
        if any(not (effect.startswith("exchange:permitted-summaries:") or effect.startswith("manage:local-capability:") or effect == "block:unsafe-release") for effect in self.effect_receipts): raise ResearchContractError("mechanism effect is outside governed gate")


def operate_megafactory_mechanisms(*, request: Mapping[str, Any]) -> FederatedMechanismReceipt:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "federation_id", "purpose", "semantic_profile")) or int(request.get("required_origin_quorum", 0)) <= 0 or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("candidates") or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")): raise ResearchContractError("mechanism federation identity, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("mechanism_id", ""))); ids = [str(item.get("mechanism_id", "")) for item in candidates]
    if not all(ids) or len(set(ids)) != len(ids): raise ResearchContractError("mechanism identifiers must be unique and non-empty")
    origins = sorted({str(item.get("origin", "")) for item in candidates})
    if not all(origins) or len(origins) < int(request["required_origin_quorum"]): raise ResearchContractError("declared origin quorum is not available")
    global_failed = {gate for gate, failed in (("policy", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("network-permission", not request.get("network_permitted", False)), ("origin-quorum", len(origins) < int(request["required_origin_quorum"]))) if failed}
    admitted: list[str] = []; conditional: list[str] = []; blocked: list[str] = []; decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); scores: dict[str, int] = {}
    for candidate in candidates:
        failed = set(global_failed); pending: set[str] = set(); mid = str(candidate["mechanism_id"])
        if candidate.get("semantic_profile") != request["semantic_profile"]: failed.add("semantic-profile")
        if candidate.get("replay_identity") != request.get("replay_identity"): failed.add("replay-identity")
        for field, gate in (("policy_allow", "candidate-policy"), ("protected_closure", "candidate-protected-closure"), ("signed_approval", "candidate-signed-approval"), ("oracle_verified", "oracle-verification"), ("raw_data_local", "candidate-locality")):
            if not candidate.get(field, False): failed.add(gate)
        score = int(candidate.get("support_score_milli", 0)) + (20000 if candidate.get("oracle_verified") else 0) + min(int(candidate.get("freshness_seq", 0)), 20) * 100 - min(int(candidate.get("omission_count", 0)), 20) * 200; scores[mid] = score
        state = str(candidate.get("evidence_state", "unknown"))
        if state == "contradicted": failed.add("contradicted-evidence"); negative.add(f"{mid}:contradicted")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{mid}:evidence-state")
        if int(candidate.get("omission_count", 0)) > 0: pending.add("omission-closure"); omissions.add(f"{mid}:omissions={candidate['omission_count']}")
        negative.add(f"{mid}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}")
        disposition = "blocked" if failed else "conditional" if pending else "admitted"; (blocked if disposition == "blocked" else conditional if disposition == "conditional" else admitted).append(mid)
        decisions.append({"mechanism_id": mid, "origin": str(candidate["origin"]), "support_score_milli": int(candidate.get("support_score_milli", 0)), "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))})
        if failed: semantic_loss.append({"field": f"mechanism:{mid}", "reason": "mechanism attestation failed one or more federation gates", "severity": "decision_relevant"})
    rank = sorted(ids, key=lambda mid: (-scores[mid], mid)); admission = "blocked" if global_failed or blocked else "approval_required" if conditional else "unknown" if not admitted else "admitted"; admitted_origins = sorted({str(c["origin"]) for c in candidates if str(c["mechanism_id"]) in admitted})
    checkpoint_digest = _hash({"federation_id": request["federation_id"], "checkpoint_seq": request["checkpoint_seq"], "mechanism_order": ids, "origin_order": origins}); control_digest = _hash({"admission": admission, "rank_order": rank, "decisions": decisions, "semantic_loss": semantic_loss})
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": FEATURE_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "admission": admission, "mechanism_order": ids, "rank_order": rank, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}
    artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"federated-mechanism-control:{request['request_id']}", "content_type": "application/vnd.aurora.federated-mechanism-portfolio+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["federation_id"], "relation": "federated-mechanism-control", "digest": control_digest}], "boundary": PRECLINICAL_BOUNDARY}
    effects = (f"exchange:permitted-summaries:{request['federation_id']}", f"manage:local-capability:{request['federation_id']}") if admission == "admitted" else ("block:unsafe-release",)
    receipt = FederatedMechanismReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, FEATURE_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["semantic_profile"]), admission, tuple(origins), tuple(admitted_origins), tuple(ids), tuple(rank), tuple(sorted(admitted)), tuple(sorted(conditional)), tuple(sorted(blocked)), (), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, control_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), tuple(effects), artifact, True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "FEATURE_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedMechanismReceipt", "operate_megafactory_mechanisms"]
