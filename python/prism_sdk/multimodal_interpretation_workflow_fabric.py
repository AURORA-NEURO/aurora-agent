"""Python parity adapter for ``AFA-api-P14-F14`` interpretation workflow fabric."""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-api-P14-F14"
CONTRACT_VERSION = "api-multimodal-multi-study-interpretation-workflow-fabric/1.0"
INPUT_SCHEMA = "EvidenceBackedResult2@1"
OUTPUT_SCHEMA = "InteractiveInterpretation4@1"
STAGES = ("compile-context", "compare-studies", "render-interpretation", "retain-receipt")


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class InterpretationWorkflowReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; workflow_id: str; scope: str; semantic_profile: str; query: str; disposition: str
    stage_order: tuple[str, ...]; study_order: tuple[str, ...]; modality_order: tuple[str, ...]; rank_order: tuple[str, ...]; qualified_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; blocked_order: tuple[str, ...]; incomparable_order: tuple[str, ...]; missing_modality_order: tuple[str, ...]; panel_order: tuple[str, ...]; action_receipts: tuple[str, ...]; checks: tuple[str, ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; replay_identity: str; comparability_digest: str; workflow_digest: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str
    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.workflow_id, self.scope, self.semantic_profile, self.query)) or self.stage_order != STAGES or not self.study_order or not self.modality_order or len(self.rank_order) != len(self.study_order) or not self.action_receipts or not self.checks or not self.effect_receipts: raise ResearchContractError("interpretation workflow identity, stages, locality, studies, checks, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.qualified_order, self.unresolved_order, self.blocked_order, self.incomparable_order, self.missing_modality_order, self.panel_order, self.action_receipts, self.checks, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("interpretation workflow ordering is not canonical")
        if set(self.rank_order) != set(self.study_order): raise ResearchContractError("interpretation rank order is not a study permutation")
        partition = list(self.qualified_order) + list(self.unresolved_order) + list(self.blocked_order)
        if set(partition) != set(self.study_order) or len(partition) != len(set(partition)): raise ResearchContractError("interpretation dispositions do not partition studies")
        if any(not effect.startswith("operate:interpretation-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("interpretation effect is outside the workflow gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.interactive-interpretation+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("interpretation artifact type or digest is invalid")
    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def run_interpretation_workflow(*, request: Mapping[str, Any]) -> InterpretationWorkflowReceipt:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "workflow_id", "scope", "semantic_profile", "query")) or request.get("input_schema") != INPUT_SCHEMA or not request.get("studies") or not request.get("required_modalities") or int(request.get("min_studies", 0)) <= 0 or int(request.get("max_panels", 0)) <= 0 or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("expected_comparability_digest")) or not _digest(request.get("replay_identity")): raise ResearchContractError("interpretation workflow identity, bounds, locality, replay, or boundary is invalid")
    required_modalities = [str(item) for item in request["required_modalities"]]
    if not _canonical(required_modalities) or any(not item.strip() for item in required_modalities): raise ResearchContractError("required modalities are not canonical")
    studies = sorted(request["studies"], key=lambda item: str(item.get("study_id", ""))); ids = [str(item.get("study_id", "")) for item in studies]
    if not all(ids) or len(set(ids)) != len(ids) or any(not _canonical([str(modality) for modality in study.get("modality_order", [])]) for study in studies): raise ResearchContractError("study identity or modality order is invalid")
    required_set = set(required_modalities); global_failed = {gate for gate, failed in (("policy", not request.get("policy_allow", False)), ("protected-closure", not request.get("protected_closure", False)), ("signed-approval", not request.get("signed_approval", False)), ("raw-data-locality", request.get("raw_data_local") is not True), ("adversarial-input", bool(request.get("adversarial_events")))) if failed}
    scores: dict[str, int] = {}; qualified: list[str] = []; unresolved: list[str] = []; blocked: list[str] = []; incomparable: set[str] = set(); missing_modality: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); decisions: list[dict[str, Any]] = []; semantic_loss: list[dict[str, Any]] = []
    for study in studies:
        sid = str(study["study_id"]); failed = set(global_failed); conditional: set[str] = set(); modalities = set(str(item) for item in study.get("modality_order", []))
        for modality in sorted(required_set - modalities): missing_modality.add(f"{sid}:{modality}"); conditional.add("required-modality-missing"); omissions.add(f"{sid}:missing-modality:{modality}")
        if not study.get("artifact_digest"): conditional.add("artifact-digest-missing"); omissions.add(f"{sid}:artifact-digest-missing")
        if not study.get("provenance_digest"): conditional.add("provenance-missing"); omissions.add(f"{sid}:provenance-missing")
        if study.get("comparability_digest") != request["expected_comparability_digest"]: conditional.add("cross-study-incomparability"); incomparable.add(sid); omissions.add(f"{sid}:comparability-mismatch")
        for item in study.get("omissions", []): conditional.add("study-omissions"); omissions.add(f"{sid}:{item}")
        for item in study.get("uncertainty", []): conditional.add("study-uncertainty"); uncertainty.add(f"{sid}:{item}")
        state = str(study.get("evidence_state", "unknown"))
        if state == "contradicted": failed.add("contradicted-evidence")
        elif state in {"unknown", "speculative"}: conditional.add("evidence-state-not-qualified"); uncertainty.add(f"{sid}:evidence-state")
        negative.add(f"{sid}:{'negative-result' if study.get('negative_result') else 'negative-result-not-observed'}")
        score = int(study.get("interpretation_score_milli", 0)) + (20_000 if state == "proven" else 10_000 if state == "supported" else 0) - len(conditional) * 500; scores[sid] = score
        disposition = "blocked" if failed else "unresolved" if conditional else "qualified"; (blocked if disposition == "blocked" else unresolved if disposition == "unresolved" else qualified).append(sid); decisions.append({"study_id": sid, "score_milli": score, "disposition": disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(conditional), "negative_result": bool(study.get("negative_result"))})
        if failed: semantic_loss.append({"field": f"study:{sid}", "reason": "study failed a multimodal interpretation workflow gate", "severity": "decision_relevant"})
    rank = sorted(ids, key=lambda sid: (-scores[sid], sid)); selected: list[str] = []; spent = 0
    by_id = {str(item["study_id"]): item for item in studies}
    for sid in rank:
        if sid not in qualified: continue
        if len(selected) >= int(request["max_panels"]): unresolved.append(sid); omissions.add(f"{sid}:panel-capacity"); continue
        cost = len(by_id[sid].get("modality_order", [])) + 1
        if cost > int(request["budget_units"]) - spent: unresolved.append(sid); omissions.add(f"{sid}:budget-ceiling")
        else: spent += cost; selected.append(sid)
    selected = sorted(set(selected)); qualified = sorted(set(selected)); unresolved = sorted(set(unresolved)); blocked = sorted(set(blocked));
    if len(selected) < int(request["min_studies"]): omissions.add(f"study-quorum:{len(selected)}/{request['min_studies']}")
    disposition = "blocked" if global_failed or blocked else "unresolved" if unresolved or len(selected) < int(request["min_studies"]) else "qualified"
    checks = tuple(sorted({f"stage:{stage}" for stage in STAGES} | {"study-identity", "modality-closure", "cross-study-comparability", "provenance-closure", "negative-evidence-retention", "policy-boundary", "replay-identity"})); payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "scope": request["scope"], "semantic_profile": request["semantic_profile"], "query": request["query"], "stage_order": list(STAGES), "study_order": ids, "rank_order": rank, "selected_order": selected, "decisions": decisions, "replay_identity": request["replay_identity"], "comparability_digest": request["expected_comparability_digest"], "boundary": PRECLINICAL_BOUNDARY}; workflow_digest = _hash(payload); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"interactive-interpretation:{request['workflow_id']}", "content_type": "application/vnd.aurora.interactive-interpretation+json", "content_hash": workflow_digest, "semantic_loss": semantic_loss, "provenance": [{"source_id": str(request["workflow_id"]), "relation": "interpretation-workflow-fabric", "digest": workflow_digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = InterpretationWorkflowReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["workflow_id"]), str(request["scope"]), str(request["semantic_profile"]), str(request["query"]), disposition, STAGES, tuple(ids), tuple(sorted({str(modality) for study in studies for modality in study.get("modality_order", [])})), tuple(rank), tuple(qualified), tuple(unresolved), tuple(blocked), tuple(sorted(incomparable)), tuple(sorted(missing_modality)), tuple(selected), tuple(sorted(("action:render-interpretation", "action:retain-research-object") if disposition == "qualified" else ("action:retain-omission-certificate",))), checks, tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), str(request["replay_identity"]), str(request["expected_comparability_digest"]), workflow_digest, artifact, (f"operate:interpretation-workflow:{request['workflow_id']}",) if disposition == "qualified" else ("block:unsafe-release",), True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "InterpretationWorkflowReceipt", "run_interpretation_workflow"]
