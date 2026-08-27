"""Python parity adapter for ``AFA-ops-P13-F28`` federated analysis assurance."""
from __future__ import annotations
from dataclasses import dataclass
import hashlib, json, re
from typing import Any, Mapping
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-ops-P13-F28"
CONTRACT_VERSION = "ops-federated-continual-analysis-assurance/1.0"
INPUT_SCHEMA = "AnalysisQuestion4@1"
OUTPUT_SCHEMA = "QualifiedAnalysisResult7@1"

def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()
def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

@dataclass(frozen=True)
class FederatedAnalysisReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; semantic_profile: str; admission: str
    origin_order: tuple[str, ...]; analysis_order: tuple[str, ...]; rank_order: tuple[str, ...]; qualified_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    decisions: tuple[Mapping[str, Any], ...]; checkpoint_seq: int; checkpoint_digest: str; control_digest: str; replay_identity: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]; artifact: Mapping[str, Any]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.federation_id.strip() or not self.purpose.strip() or not self.semantic_profile.strip() or not self.analysis_order or len(self.decisions) != len(self.analysis_order) or not self.effect_receipts: raise ResearchContractError("analysis federation identity, locality, candidates, decisions, or effects are incomplete")
        for values in (self.origin_order, self.analysis_order, self.qualified_order, self.unresolved_order, self.blocked_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(values) != tuple(sorted(set(values))): raise ResearchContractError("analysis ordering is not canonical")
        if len(self.rank_order) != len(self.analysis_order) or set(self.rank_order) != set(self.analysis_order) or tuple(str(item.get("analysis_id", "")) for item in self.decisions) != self.analysis_order: raise ResearchContractError("analysis ranking or decisions do not match candidates")
        if set(self.qualified_order) | set(self.unresolved_order) | set(self.blocked_order) != set(self.analysis_order): raise ResearchContractError("analysis dispositions do not partition candidates")
        if any(not _digest(value) for value in (self.checkpoint_digest, self.control_digest, self.replay_identity, self.artifact.get("content_hash"))): raise ResearchContractError("analysis federation digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.qualified-analysis-result+json": raise ResearchContractError("analysis artifact type is invalid")
        if any(not effect.startswith("qualify:analysis:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("analysis effect is outside the release gate")

def assure_federated_analysis(*, request: Mapping[str, Any]) -> FederatedAnalysisReceipt:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "federation_id", "purpose", "semantic_profile")) or int(request.get("required_origin_quorum", 0)) <= 0 or int(request.get("capacity", 0)) <= 0 or int(request.get("active_runs", 0)) > int(request.get("capacity", 0)) or int(request.get("checkpoint_seq", 0)) <= 0 or not request.get("candidates") or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")): raise ResearchContractError("analysis federation identity, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid")
    candidates = sorted(request["candidates"], key=lambda item: str(item.get("analysis_id", ""))); ids = [str(item.get("analysis_id", "")) for item in candidates]; origins = sorted({str(item.get("origin", "")) for item in candidates})
    if not all(ids) or len(set(ids)) != len(ids) or not all(origins) or len(origins) < int(request["required_origin_quorum"]): raise ResearchContractError("analysis identifiers or origin quorum are invalid")
    global_failed = {gate for gate, failed in (("policy", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("network-permission", not request.get("network_permitted", False))) if failed}
    qualified: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []; omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); scores: dict[str, int] = {}
    for candidate in candidates:
        aid = str(candidate["analysis_id"]); failed = set(global_failed); pending: set[str] = set()
        if candidate.get("semantic_profile") != request["semantic_profile"]: failed.add("semantic-profile")
        if candidate.get("replay_identity") != request.get("replay_identity"): failed.add("replay-identity")
        for field, gate in (("policy_allow", "candidate-policy"), ("protected_closure", "candidate-protected-closure"), ("signed_approval", "candidate-signed-approval"), ("raw_data_local", "candidate-locality")):
            if not candidate.get(field, False): failed.add(gate)
        if int(candidate.get("independent_site_count", 0)) < int(candidate.get("required_site_quorum", 0)): pending.add("independent-site-quorum"); omissions.add(f"{aid}:sites={candidate.get('independent_site_count', 0)}/{candidate.get('required_site_quorum', 0)}")
        state = str(candidate.get("evidence_state", "unknown"));
        if state == "contradicted": failed.add("contradicted-evidence"); negative.add(f"{aid}:contradicted")
        elif state in {"unknown", "speculative"}: pending.add("evidence-state"); uncertainty.add(f"{aid}:evidence-state")
        score = int(candidate.get("baseline_delta_milli", 0)) - int(candidate.get("uncertainty_width_milli", 0)) + min(int(candidate.get("independent_site_count", 0)), 20) * 100 + (20000 if state == "proven" else 10000 if state == "supported" else 0); scores[aid] = score
        negative.add(f"{aid}:{'negative-result' if candidate.get('negative_result') else 'negative-result-not-observed'}"); disposition = "blocked" if failed else "unresolved" if pending else "qualified"; (blocked if disposition == "blocked" else unresolved if disposition == "unresolved" else qualified).append(aid); decisions.append({"analysis_id": aid, "origin": str(candidate["origin"]), "score_milli": score, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(pending), "negative_result": bool(candidate.get("negative_result"))});
        if failed: semantic_loss.append({"field": f"analysis:{aid}", "reason": "analysis attestation failed one or more release gates", "severity": "decision_relevant"})
    rank = sorted(ids, key=lambda aid: (-scores[aid], aid)); admission = "blocked" if global_failed or blocked else "unresolved" if unresolved else "qualified" if qualified else "blocked"; checkpoint_digest = _hash({"federation_id": request["federation_id"], "checkpoint_seq": request["checkpoint_seq"], "analysis_order": ids, "origin_order": origins}); control_digest = _hash({"admission": admission, "rank_order": rank, "decisions": decisions, "semantic_loss": semantic_loss}); payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "federation_id": request["federation_id"], "purpose": request["purpose"], "semantic_profile": request["semantic_profile"], "admission": admission, "analysis_order": ids, "rank_order": rank, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request["replay_identity"], "boundary": PRECLINICAL_BOUNDARY}; artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"federated-analysis-assurance:{request['request_id']}", "content_type": "application/vnd.aurora.qualified-analysis-result+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["federation_id"], "relation": "federated-analysis-assurance", "digest": control_digest}], "boundary": PRECLINICAL_BOUNDARY}; effects = (f"qualify:analysis:{request['federation_id']}",) if admission == "qualified" else ("block:unsafe-release",); receipt = FederatedAnalysisReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["federation_id"]), str(request["purpose"]), str(request["semantic_profile"]), admission, tuple(origins), tuple(ids), tuple(rank), tuple(sorted(qualified)), tuple(sorted(unresolved)), tuple(sorted(blocked)), tuple(decisions), int(request["checkpoint_seq"]), checkpoint_digest, control_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), effects, artifact, True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt

__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "FederatedAnalysisReceipt", "assure_federated_analysis"]
