"""Federated continual statistical/causal/ML workflow fabric for ``AFA-examples-P13-F16``."""
from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-examples-P13-F16"
CONTRACT_VERSION = "examples-federated-continual-statistical-analysis-workflow-fabric/1.0"
INPUT_SCHEMA = "AnalysisWorkflowDraft5@1"
OUTPUT_SCHEMA = "AnalysisWorkflowRun8@1"
STAGES = ("admit", "checkpoint", "validate", "schedule", "retain-receipt")


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class StatisticalAnalysisWorkflowRun:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; federation_id: str; purpose: str; semantic_profile: str; disposition: str; stage_order: tuple[str, ...]; required_candidate_order: tuple[str, ...]; selected_candidate_order: tuple[str, ...]; pending_candidate_order: tuple[str, ...]; blocked_candidate_order: tuple[str, ...]; compensated_candidate_order: tuple[str, ...]; omission_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]; negative_evidence_order: tuple[str, ...]; checkpoint_digest: str; replay_identity: str; workflow_digest: str; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; aggregate_only: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.boundary != PRECLINICAL_BOUNDARY or self.raw_data_local is not True or self.aggregate_only is not True or tuple(self.stage_order) != STAGES or not self.required_candidate_order or not self.effect_receipts or not all(isinstance(value, str) and value.strip() for value in (self.request_id, self.federation_id, self.purpose, self.semantic_profile)):
            raise ResearchContractError("analysis workflow identity, stages, locality, aggregate boundary, or effects are incomplete")
        for values in (self.required_candidate_order, self.selected_candidate_order, self.pending_candidate_order, self.blocked_candidate_order, self.compensated_candidate_order, self.omission_order, self.uncertainty_order, self.negative_evidence_order, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("analysis workflow ordering is not canonical")
        required = set(self.required_candidate_order); states = list(self.selected_candidate_order) + list(self.pending_candidate_order) + list(self.blocked_candidate_order)
        if len(required) != len(self.required_candidate_order) or len(states) != len(required) or set(states) != required or any(item not in required for item in self.compensated_candidate_order): raise ResearchContractError("analysis candidate states do not partition the required plan")
        for digest in (self.checkpoint_digest, self.replay_identity, self.workflow_digest, self.artifact.get("content_hash")):
            if not _digest(digest): raise ResearchContractError("analysis workflow digest is invalid")
        if self.artifact.get("content_type") != "application/vnd.aurora.analysis-workflow-run+json": raise ResearchContractError("analysis workflow artifact type is invalid")
        if any(not effect.startswith("schedule:analysis-workflow:") and not effect.startswith("compensate:analysis-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("analysis workflow effect is outside schedule/compensation gate")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def assure_statistical_analysis_workflow(*, draft: Mapping[str, Any]) -> StatisticalAnalysisWorkflowRun:
    required = ("request_id", "federation_id", "purpose", "semantic_profile")
    if any(not str(draft.get(field, "")).strip() for field in required) or not draft.get("required_candidate_order") or not _canonical([str(item) for item in draft["required_candidate_order"]]) or tuple(draft.get("stage_order", ())) != STAGES or draft.get("raw_data_local") is not True or draft.get("aggregate_only") is not True or int(draft.get("budget_units", 0)) <= 0 or int(draft.get("budget_units", 0)) > int(draft.get("max_budget_units", 0)) or draft.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(draft.get("replay_identity")):
        raise ResearchContractError("analysis workflow request identity, stage protocol, locality, aggregate boundary, budget, replay, or boundary is invalid")
    required_order = [str(item) for item in draft["required_candidate_order"]]
    if len(set(required_order)) != len(required_order): raise ResearchContractError("required analysis candidates must be unique")
    candidates = {}; selected: set[str] = set(); pending: set[str] = set(); blocked: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    for candidate in draft.get("candidates", []):
        cid = str(candidate.get("candidate_id", "")); omissions_list = [str(item) for item in candidate.get("omissions", [])]; uncertainty_list = [str(item) for item in candidate.get("uncertainty", [])]
        if not cid.strip() or cid not in set(required_order) or cid in candidates or not _canonical(omissions_list) or not _canonical(uncertainty_list) or not str(candidate.get("estimand", "")).strip() or not str(candidate.get("method_family", "")).strip() or candidate.get("semantic_profile") != draft["semantic_profile"] or not _digest(candidate.get("baseline_digest")) or not _digest(candidate.get("provenance_digest")) or candidate.get("local_data") is not True or int(candidate.get("quality_score_milli", 0)) > 1000:
            raise ResearchContractError("analysis candidate identity, evidence, baseline, provenance, profile, quality, or locality is invalid")
        candidates[cid] = candidate; omissions.update(f"candidate:{cid}:{item}" for item in omissions_list); uncertainty.update(f"candidate:{cid}:{item}" for item in uncertainty_list)
        if candidate.get("negative_result"): negative.add(f"candidate:{cid}:negative-result")
        state = str(candidate.get("evidence_state", "unknown"))
        if state in {"proven", "supported"} and int(candidate.get("quality_score_milli", 0)) >= 700 and not omissions_list and not uncertainty_list: selected.add(cid)
        elif state == "contradicted": blocked.add(cid); negative.add(f"candidate:{cid}:contradicted")
        else: pending.add(cid); uncertainty.add(f"candidate:{cid}:not-qualified")
    for cid in set(required_order) - set(candidates): pending.add(cid); omissions.add(f"missing-candidate:{cid}"); uncertainty.add(f"missing-candidate:{cid}")
    violations: set[str] = set()
    for name, field in (("policy", "policy_allow"), ("protected-closure", "protected_closure"), ("signed-approval", "signed_approval"), ("federation-approval", "federation_approved")):
        if draft.get(field) is not True: violations.add(name)
    for event in draft.get("adversarial_events", []): violations.add(f"adversarial:{event}"); omissions.add(f"workflow:adversarial:{event}")
    disposition = "blocked" if violations or draft.get("adversarial_events") else "partial" if pending or blocked or uncertainty else "qualified"
    if disposition == "blocked": blocked = set(required_order); selected.clear(); pending.clear()
    selected_order = sorted(selected); pending_order = sorted(pending); blocked_order = sorted(blocked)
    checkpoint = {"request_id": draft["request_id"], "stage_order": list(STAGES), "required_candidate_order": required_order, "selected_candidate_order": selected_order, "pending_candidate_order": pending_order, "blocked_candidate_order": blocked_order, "replay_identity": draft["replay_identity"]}
    checkpoint_digest = _hash(checkpoint); workflow_digest = _hash({"checkpoint_digest": checkpoint_digest, "semantic_profile": draft["semantic_profile"], "disposition": disposition}); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"examples-analysis-workflow:{draft['request_id']}", "content_type": "application/vnd.aurora.analysis-workflow-run+json", "content_hash": checkpoint_digest, "semantic_loss": [], "provenance": [{"source_id": draft["federation_id"], "relation": "analysis-workflow-checkpoint", "digest": checkpoint_digest}], "boundary": PRECLINICAL_BOUNDARY}; receipt = StatisticalAnalysisWorkflowRun(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(draft["request_id"]), str(draft["federation_id"]), str(draft["purpose"]), str(draft["semantic_profile"]), disposition, STAGES, tuple(required_order), tuple(selected_order), tuple(pending_order), tuple(blocked_order), tuple(pending_order), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), checkpoint_digest, str(draft["replay_identity"]), workflow_digest, artifact, (f"schedule:analysis-workflow:{draft['request_id']}",) if disposition == "qualified" else (f"compensate:analysis-workflow:{draft['request_id']}",) if disposition == "partial" else ("block:unsafe-release",), True, True, PRECLINICAL_BOUNDARY); receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA", "StatisticalAnalysisWorkflowRun", "assure_statistical_analysis_workflow"]
