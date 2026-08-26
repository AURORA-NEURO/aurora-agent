"""Workflow-protocol parity for AFA-adapter-P01-F15."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .adapter_throughput_evidence_surveillance_research_copilot import (
    ThroughputCopilotEvidenceObservation,
    run_throughput_evidence_surveillance_research_copilot,
)
from .research_contracts import (
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_CONTRACT_VERSION,
    ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError,
    research_artifact_digest,
)

CANONICAL_STAGES = ("stage:checkpoint", "stage:admit-capacity", "stage:surveil-evidence", "stage:publish-receipt")

@dataclass(frozen=True)
class ThroughputEvidenceSurveillanceWorkflowReceipt:
    request_id: str; workflow_id: str; agent_id: str; batch_id: str; checkpoint_seq: int; disposition: str
    stage_order: tuple[str, ...]; plan_order: tuple[str, ...]; completed_order: tuple[str, ...]; blocked_order: tuple[str, ...]; compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]; selected_order: tuple[str, ...]; unresolved_order: tuple[str, ...]; denied_order: tuple[str, ...]; overflow_order: tuple[str, ...]
    replay_identity: str; copilot_run_digest: str; checkpoint_digest: str; workflow_digest: str
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]; artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_CONTRACT_VERSION; feature_id: str = ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID; raw_data_local: bool = True; boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_CONTRACT_VERSION or self.feature_id != ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.agent_id.strip() or not self.batch_id.strip() or self.checkpoint_seq <= 0 or self.stage_order != CANONICAL_STAGES or not self.effect_receipts:
            raise ResearchContractError("throughput workflow identity, stages, locality, checkpoint, or effects are incomplete")
        for values in (self.plan_order, self.completed_order, self.blocked_order, self.compensation_order, self.candidate_order, self.selected_order, self.unresolved_order, self.denied_order, self.overflow_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values: raise ResearchContractError("throughput workflow ordering is not canonical")
        for value in (self.replay_identity, self.copilot_run_digest, self.checkpoint_digest, self.workflow_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value): raise ResearchContractError("throughput workflow digest is invalid")
        if any(not effect.startswith("schedule:research-work:") and not effect.startswith("compensate:research-work:") and effect != "block:unsafe-release" for effect in self.effect_receipts): raise ResearchContractError("throughput workflow effect is outside schedule gate")

def schedule_throughput_evidence_surveillance_workflow(*, request_id: str, workflow_id: str, checkpoint_id: str, request: Mapping[str, Any], requested_stage_order: Sequence[str], budget_units: int, replay_identity: str, boundary: str = PRECLINICAL_BOUNDARY) -> ThroughputEvidenceSurveillanceWorkflowReceipt:
    if not request_id.strip() or not workflow_id.strip() or not checkpoint_id.strip() or budget_units <= 0 or tuple(requested_stage_order) != CANONICAL_STAGES or boundary != PRECLINICAL_BOUNDARY or not re.fullmatch(r"[0-9a-f]{64}", replay_identity): raise ResearchContractError("throughput workflow identity, stages, budget, replay, or boundary is invalid")
    observations = tuple(item if isinstance(item, ThroughputCopilotEvidenceObservation) else ThroughputCopilotEvidenceObservation(**item) for item in request.get("observations", ()))
    copilot = run_throughput_evidence_surveillance_research_copilot(request_id=str(request.get("request_id", request_id)), agent_id=str(request["agent_id"]), batch_id=str(request["batch_id"]), checkpoint_seq=int(request["checkpoint_seq"]), capacity=int(request["capacity"]), declared_tools=tuple(request["declared_tools"]), requested_tool=str(request["requested_tool"]), max_tool_calls=int(request["max_tool_calls"]), dry_run=bool(request["dry_run"]), approval_reference=request.get("approval_reference"), approval_granted=bool(request["approval_granted"]), observations=observations, min_relevance_score=int(request.get("min_relevance_score", 0)), policy_allow=bool(request.get("policy_allow", True)), protected_closure=bool(request.get("protected_closure", True)), raw_data_local=bool(request.get("raw_data_local", True)), replay_identity=str(request["replay_identity"]))
    plan = set(f"plan:{stage}" for stage in CANONICAL_STAGES); completed = set(CANONICAL_STAGES); compensation: set[str] = set(); plan.add("plan:publish-qualified-throughput-artifact" if copilot.selected_order else "plan:retain-unresolved-throughput-evidence")
    if budget_units < len(plan): compensation.add("compensate:research-work:budget-exhausted")
    omissions = set(copilot.omissions); omissions.update({"workflow:policy-denied"} if not request.get("policy_allow", True) else set()); omissions.update({"workflow:protected-closure-incomplete"} if not request.get("protected_closure", True) else set()); omissions.update({"workflow:approval-required"} if not request.get("approval_granted", False) and not request.get("dry_run", False) else set())
    plan_order, completed_order, compensation_order, omissions_order = tuple(sorted(plan)), tuple(sorted(completed)), tuple(sorted(compensation)), tuple(sorted(omissions))
    checkpoint_digest = research_artifact_digest({"workflow_id": workflow_id, "checkpoint_id": checkpoint_id, "checkpoint_seq": request["checkpoint_seq"], "stage_order": list(CANONICAL_STAGES), "replay_identity": replay_identity}); copilot_run_digest = research_artifact_digest(copilot.__dict__); workflow_digest = research_artifact_digest({"workflow_id": workflow_id, "plan_order": list(plan_order), "completed_order": list(completed_order), "compensation_order": list(compensation_order), "checkpoint_digest": checkpoint_digest, "copilot_run_digest": copilot_run_digest, "budget_units": budget_units})
    disposition = copilot.disposition; blocked_order = ("stage:release",) if disposition == "blocked" else (); payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_CONTRACT_VERSION, "feature_id": ADAPTER_THROUGHPUT_EVIDENCE_SURVEILLANCE_WORKFLOW_FABRIC_FEATURE_ID, "workflow_id": workflow_id, "batch_id": request["batch_id"], "stage_order": list(CANONICAL_STAGES), "plan_order": list(plan_order), "completed_order": list(completed_order), "compensation_order": list(compensation_order), "candidate_order": list(copilot.candidate_order), "selected_order": list(copilot.selected_order), "unresolved_order": list(copilot.unresolved_order), "denied_order": list(copilot.denied_order), "overflow_order": list(copilot.overflow_order), "replay_identity": replay_identity, "copilot_run_digest": copilot_run_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "omissions": list(omissions_order), "uncertainty": list(copilot.uncertainty), "negative_evidence": list(copilot.negative_evidence), "boundary": PRECLINICAL_BOUNDARY}; artifact = {"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.throughput-research-workflow+json"}
    effect = ("block:unsafe-release",) if disposition == "blocked" else (f"compensate:research-work:{workflow_id}",) if compensation_order else (f"schedule:research-work:{workflow_id}",)
    receipt = ThroughputEvidenceSurveillanceWorkflowReceipt(request_id, workflow_id, str(request["agent_id"]), str(request["batch_id"]), int(request["checkpoint_seq"]), disposition, CANONICAL_STAGES, plan_order, completed_order, blocked_order, compensation_order, tuple(sorted(copilot.candidate_order)), tuple(sorted(copilot.selected_order)), tuple(sorted(copilot.unresolved_order)), tuple(sorted(copilot.denied_order)), tuple(sorted(copilot.overflow_order)), replay_identity, copilot_run_digest, checkpoint_digest, workflow_digest, omissions_order, tuple(sorted(copilot.uncertainty)), tuple(sorted(copilot.negative_evidence)), effect, artifact); receipt.validate(); return receipt
