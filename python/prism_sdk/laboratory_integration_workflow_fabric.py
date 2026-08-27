"""Python parity adapter for ``AFA-interweave-P11-F14``.

The adapter preflights and schedules a typed instrument plan but never invokes hardware.  It
retains modality gaps, comparability failures, uncertainty, approvals, and compensation receipts.
"""
from __future__ import annotations

from dataclasses import dataclass, asdict
import hashlib
import json
import re
from typing import Any, Mapping

from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError

FEATURE_ID = "AFA-interweave-P11-F14"
CONTRACT_VERSION = "interweave-multimodal-laboratory-integration-workflow-fabric/1.0"
INPUT_SCHEMA = "InstrumentActionRequest2@1"
OUTPUT_SCHEMA = "InstrumentActionReceipt4@1"
STAGES = ("preflight", "validate-closure", "schedule", "checkpoint", "retain-receipt")


def _hash(value: Any) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()).hexdigest()


def _digest(value: Any) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _canonical(values: list[str] | tuple[str, ...]) -> bool:
    return list(values) == sorted(set(values))


@dataclass(frozen=True)
class LaboratoryWorkflowReceipt:
    schema_version: str; contract_version: str; feature_id: str; request_id: str; workflow_id: str; scope: str; semantic_profile: str; disposition: str
    stage_order: tuple[str, ...]; study_order: tuple[str, ...]; modality_order: tuple[str, ...]; action_order: tuple[str, ...]; scheduled_order: tuple[str, ...]; pending_order: tuple[str, ...]; blocked_order: tuple[str, ...]; compensation_order: tuple[str, ...]; missing_modality_order: tuple[str, ...]; incomparable_order: tuple[str, ...]; decisions: tuple[Mapping[str, Any], ...]; checkpoint_digest: str; workflow_digest: str; replay_identity: str; semantic_loss: tuple[Mapping[str, Any], ...]; omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; artifact: Mapping[str, Any]; effect_receipts: tuple[str, ...]; raw_data_local: bool; boundary: str

    def validate(self) -> None:
        if (self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID) or self.stage_order != STAGES or not all(str(value).strip() for value in (self.request_id, self.workflow_id, self.scope, self.semantic_profile)) or not self.study_order or not self.action_order or len(self.decisions) != len(self.action_order) or self.raw_data_local is not True or self.boundary != PRECLINICAL_BOUNDARY or not self.effect_receipts: raise ResearchContractError("laboratory workflow identity, stages, locality, actions, or effects are incomplete")
        for values in (self.study_order, self.modality_order, self.action_order, self.scheduled_order, self.pending_order, self.blocked_order, self.compensation_order, self.missing_modality_order, self.incomparable_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if not _canonical(values): raise ResearchContractError("laboratory workflow ordering is not canonical")
        if any(decision.get("action_id") != action_id for decision, action_id in zip(self.decisions, self.action_order)): raise ResearchContractError("laboratory decisions do not match action order")
        partition = list(self.scheduled_order) + list(self.pending_order) + list(self.blocked_order)
        if set(partition) != set(self.action_order) or len(partition) != len(set(partition)): raise ResearchContractError("laboratory action states do not partition the plan")
        if any(not effect.startswith("schedule:research-work:") and not effect.startswith("compensate:") and effect not in {"approval-required:instrument", "block:unsafe-release"} for effect in self.effect_receipts): raise ResearchContractError("laboratory effect is outside the schedule gate")
        if self.artifact.get("content_type") != "application/vnd.aurora.instrument-action-receipt+json" or not _digest(self.artifact.get("content_hash")): raise ResearchContractError("laboratory artifact type or digest is invalid")

    def to_dict(self) -> dict[str, Any]:
        value = asdict(self)
        for key, item in value.items():
            if isinstance(item, tuple): value[key] = list(item)
        return value


def orchestrate_laboratory_workflow(*, request: Mapping[str, Any]) -> LaboratoryWorkflowReceipt:
    if any(not str(request.get(field, "")).strip() for field in ("request_id", "workflow_id", "scope", "semantic_profile")) or request.get("schema_version") != INPUT_SCHEMA or not request.get("studies") or not request.get("required_modalities") or not request.get("actions") or tuple(request.get("stage_order", ())) != STAGES or int(request.get("budget_units", 0)) <= 0 or int(request.get("budget_units", 0)) > int(request.get("max_budget_units", 0)) or request.get("raw_data_local") is not True or request.get("boundary") != PRECLINICAL_BOUNDARY or not _digest(request.get("replay_identity")) or not _digest(request.get("expected_comparability_digest")): raise ResearchContractError("laboratory workflow identity, stages, bounds, locality, replay, or boundary is invalid")
    required_modalities = [str(value) for value in request["required_modalities"]]
    if not _canonical(required_modalities) or any(not value.strip() for value in required_modalities): raise ResearchContractError("required modality order is not canonical")
    studies = sorted(request["studies"], key=lambda item: str(item.get("study_id", ""))); study_ids = [str(item.get("study_id", "")) for item in studies]
    if not all(study_ids) or len(set(study_ids)) != len(study_ids) or any(not _canonical([str(value) for value in study.get("modality_order", [])]) or not _digest(study.get("comparability_digest")) or not _digest(study.get("artifact_digest")) or not _digest(study.get("provenance_digest")) for study in studies): raise ResearchContractError("study identity, modality, comparability, artifact, or provenance is invalid")
    actions = sorted(request["actions"], key=lambda item: str(item.get("action_id", ""))); action_ids = [str(item.get("action_id", "")) for item in actions]
    if not all(action_ids) or len(set(action_ids)) != len(action_ids) or any(str(action.get("study_id", "")) not in study_ids or not str(action.get("modality", "")).strip() or not str(action.get("instrument_id", "")).strip() or not str(action.get("operation", "")).strip() or (not action.get("reversible", False) and not _digest(action.get("evidence_digest"))) for action in actions): raise ResearchContractError("action identity, study binding, operation, or evidence is invalid")
    by_study = {str(study["study_id"]): study for study in studies}; missing: set[str] = set(); incomparable: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); semantic_loss: list[Mapping[str, Any]] = []
    for study in studies:
        sid = str(study["study_id"]); modalities = set(str(value) for value in study.get("modality_order", []))
        for modality in sorted(set(required_modalities) - modalities): missing.add(f"{sid}:{modality}"); omissions.add(f"{sid}:missing-modality:{modality}")
        if study.get("comparability_digest") != request["expected_comparability_digest"]: incomparable.add(sid); omissions.add(f"{sid}:comparability-mismatch")
        for item in study.get("omissions", []): omissions.add(f"{sid}:{item}")
        for item in study.get("uncertainty", []): uncertainty.add(f"{sid}:{item}")
        negative.add(f"{sid}:{'negative-result' if study.get('negative_result') else 'negative-result-not-observed'}")
        if str(study.get("evidence_state", "unknown")) in {"unknown", "speculative"}: uncertainty.add(f"{sid}:evidence-state")
        if str(study.get("evidence_state")) == "contradicted": semantic_loss.append({"field": f"study:{sid}", "reason": "contradicted study cannot authorize physical action", "severity": "decision_relevant"})
    data_omissions = bool(omissions)
    global_failures = [reason for reason, failed in (("policy-denied", request.get("policy_allow") is not True), ("protected-closure-incomplete", request.get("protected_closure") is not True), ("signed-approval-missing", request.get("signed_approval") is not True), ("raw-data-locality-failed", request.get("raw_data_local") is not True), ("adversarial-event", bool(request.get("adversarial_events")))) if failed]
    omissions.update(f"workflow:{reason}" for reason in global_failures)
    closure_incomplete = bool(missing or incomparable or data_omissions or uncertainty or any(str(study.get("evidence_state")) == "contradicted" for study in studies))
    approval_only = bool(global_failures) and all(reason == "signed-approval-missing" for reason in global_failures) and not closure_incomplete
    disposition = "blocked" if global_failures and not approval_only else "approval_required" if approval_only else "partial" if closure_incomplete else "qualified"
    scheduled: list[str] = []; pending: list[str] = []; blocked: list[str] = []; decisions: list[Mapping[str, Any]] = []; spent = 0
    for action in actions:
        aid = str(action["action_id"]); study = by_study[str(action["study_id"])]
        failed = set(global_failures); conditional: set[str] = set(); modalities = set(str(value) for value in study.get("modality_order", []))
        if str(action["modality"]) not in modalities: conditional.add("action-modality-missing")
        if any(modality not in modalities for modality in required_modalities): conditional.add("study-modality-closure-incomplete")
        if study.get("comparability_digest") != request["expected_comparability_digest"]: conditional.add("study-incomparable")
        if study.get("omissions"): conditional.add("study-omissions")
        if study.get("uncertainty"): conditional.add("study-uncertainty")
        if str(study.get("evidence_state", "unknown")) in {"unknown", "speculative"}: conditional.add("evidence-state-not-qualified")
        if str(study.get("evidence_state")) == "contradicted": failed.add("contradicted-evidence")
        cost = int(action.get("cost_units", 0));
        if cost > int(request["budget_units"]) - spent: conditional.add("budget-ceiling")
        action_disposition = "blocked" if failed else "pending" if conditional else "scheduled"
        if action_disposition == "blocked": blocked.append(aid)
        elif action_disposition == "pending": pending.append(aid)
        else: scheduled.append(aid); spent += cost
        decisions.append({"action_id": aid, "study_id": action["study_id"], "modality": action["modality"], "disposition": action_disposition, "failed_gates": sorted(failed), "conditional_gates": sorted(conditional), "cost_units": cost})
    compensation = sorted({f"compensate:{aid}" for aid in pending + blocked})
    payload = {"schema_version": OUTPUT_SCHEMA, "request_id": request["request_id"], "workflow_id": request["workflow_id"], "disposition": disposition, "stage_order": list(STAGES), "study_order": sorted(study_ids), "action_order": action_ids, "scheduled_order": sorted(scheduled), "pending_order": sorted(pending), "blocked_order": sorted(blocked), "replay_identity": request["replay_identity"]}
    workflow_digest = _hash(payload); checkpoint_digest = _hash({"workflow_digest": workflow_digest, "checkpoint": "pre-physical-execution", "spent_units": spent}); artifact = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "artifact_id": f"instrument-action-receipt:{request['request_id']}", "content_type": "application/vnd.aurora.instrument-action-receipt+json", "content_hash": _hash(payload), "semantic_loss": semantic_loss, "provenance": [{"source_id": request["workflow_id"], "relation": "laboratory-integration-workflow", "digest": _hash(payload)}], "boundary": PRECLINICAL_BOUNDARY}
    effects = [f"schedule:research-work:{request['workflow_id']}"] if disposition == "qualified" else ["approval-required:instrument"] if disposition == "approval_required" else ["block:unsafe-release", *compensation]
    receipt = LaboratoryWorkflowReceipt(RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID, str(request["request_id"]), str(request["workflow_id"]), str(request["scope"]), str(request["semantic_profile"]), disposition, STAGES, tuple(sorted(study_ids)), tuple(required_modalities), tuple(action_ids), tuple(sorted(scheduled)), tuple(sorted(pending)), tuple(sorted(blocked)), tuple(compensation), tuple(sorted(missing)), tuple(sorted(incomparable)), tuple(decisions), checkpoint_digest, workflow_digest, str(request["replay_identity"]), tuple(semantic_loss), tuple(sorted(omissions)), tuple(sorted(uncertainty)), tuple(sorted(negative)), artifact, tuple(sorted(effects)), True, PRECLINICAL_BOUNDARY)
    receipt.validate(); return receipt


__all__ = ["LaboratoryWorkflowReceipt", "orchestrate_laboratory_workflow", "FEATURE_ID", "CONTRACT_VERSION", "INPUT_SCHEMA", "OUTPUT_SCHEMA"]
