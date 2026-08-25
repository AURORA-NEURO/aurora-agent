"""Local checkpointed context workflow fabric parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ContextWorkflowStage:
    stage_id: str
    depends_on: tuple[str, ...] = ()
    budget_units: int = 1
    required: bool = True


@dataclass(frozen=True)
class ContextWorkflowReceipt:
    request_id: str
    workflow_id: str
    query_id: str
    goal: str
    disposition: str
    stage_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    checkpoint_digest: str
    workflow_digest: str
    context_digest: str
    replay_identity: str
    budget_units: int
    consumed_budget_units: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = CONTEXT_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION or self.feature_id != CONTEXT_WORKFLOW_FABRIC_FEATURE_ID:
            raise ResearchContractError("workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.query_id.strip() or not self.goal.strip() or not self.stage_order or not self.plan_order or self.budget_units < 1 or self.consumed_budget_units > self.budget_units or not self.effect_receipts:
            raise ResearchContractError("workflow identity, stage plan, budget, locality, or effects are incomplete")
        if any(not stage.strip() for stage in self.stage_order) or len(set(self.stage_order)) != len(self.stage_order) or any(stage not in self.stage_order for stage in (*self.completed_order, *self.blocked_order)) or set(self.completed_order) & set(self.blocked_order):
            raise ResearchContractError("workflow stage coverage is invalid")
        for values in (self.plan_order, self.blocked_order, self.compensation_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("workflow vectors are not canonical")
        if len(set(self.completed_order)) != len(self.completed_order):
            raise ResearchContractError("workflow completed order contains duplicates")
        for value in (self.checkpoint_digest, self.workflow_digest, self.context_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("workflow digest is invalid")
        if any(not effect.startswith("schedule:context-workflow:") and not effect.startswith("compensate:context-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("workflow effect is outside schedule/compensation gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id,
            "request_id": self.request_id, "workflow_id": self.workflow_id, "query_id": self.query_id, "goal": self.goal,
            "disposition": self.disposition, "stage_order": list(self.stage_order), "plan_order": list(self.plan_order),
            "completed_order": list(self.completed_order), "blocked_order": list(self.blocked_order), "compensation_order": list(self.compensation_order),
            "checkpoint_digest": self.checkpoint_digest, "workflow_digest": self.workflow_digest, "context_digest": self.context_digest,
            "replay_identity": self.replay_identity, "budget_units": self.budget_units, "consumed_budget_units": self.consumed_budget_units,
            "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary,
        })


def _topological_order(stages: Sequence[ContextWorkflowStage]) -> tuple[str, ...]:
    stage_map = {stage.stage_id: stage for stage in stages}
    if len(stage_map) != len(stages) or any(not key.strip() for key in stage_map):
        raise ResearchContractError("workflow stage identifiers must be unique and non-empty")
    indegree = {stage.stage_id: len(set(stage.depends_on)) for stage in stages}
    outgoing: dict[str, set[str]] = {stage.stage_id: set() for stage in stages}
    for stage in stages:
        if len(set(stage.depends_on)) != len(stage.depends_on) or stage.stage_id in stage.depends_on or any(dependency not in stage_map for dependency in stage.depends_on):
            raise ResearchContractError(f"stage {stage.stage_id} has invalid dependencies")
        for dependency in stage.depends_on:
            outgoing[dependency].add(stage.stage_id)
    ready = sorted(stage_id for stage_id, degree in indegree.items() if degree == 0)
    order: list[str] = []
    while ready:
        stage_id = ready.pop(0); order.append(stage_id)
        for child in sorted(outgoing[stage_id]):
            indegree[child] -= 1
            if indegree[child] == 0:
                ready.append(child); ready.sort()
    if len(order) != len(stages):
        raise ResearchContractError("workflow dependency cycle detected")
    return tuple(order)


def compile_context_workflow(*, request_id: str, workflow_id: str, query_id: str, goal: str, checkpoint_id: str, stages: Sequence[ContextWorkflowStage], budget_units: int, context_digest: str, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> ContextWorkflowReceipt:
    if not request_id.strip() or not workflow_id.strip() or not query_id.strip() or not goal.strip() or not checkpoint_id.strip() or not stages or budget_units < 1 or not re.fullmatch(r"[0-9a-f]{64}", context_digest) or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("workflow identity, stages, budget, replay, or boundary is invalid")
    stage_order = _topological_order(stages)
    stage_map = {stage.stage_id: stage for stage in stages}
    plan = tuple(sorted(f"plan:execute:{stage_id}" for stage_id in stage_order))
    total_budget = sum(stage.budget_units for stage in stages)
    gates_open = policy_allow and protected_closure and raw_data_local
    completed: list[str] = []; blocked: set[str] = set(); consumed = 0
    for stage_id in stage_order:
        cost = stage_map[stage_id].budget_units
        if not gates_open or consumed + cost > budget_units:
            blocked.add(stage_id)
        else:
            consumed += cost; completed.append(stage_id)
    omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set()
    if budget_units < total_budget: omissions.add("workflow:budget-exhausted")
    if not policy_allow: omissions.add("workflow:policy-denied")
    if not protected_closure: omissions.add("workflow:protected-closure-incomplete")
    if not raw_data_local: omissions.add("workflow:raw-data-locality-failed")
    if blocked: uncertainty.add("workflow:blocked-stages-retained-for-replay")
    disposition = "blocked" if not gates_open else ("admitted" if not blocked else "refinement_required")
    compensation = {f"compensate:context-workflow:{workflow_id}:retain-checkpoint"} if completed and disposition != "admitted" else set()
    checkpoint = research_artifact_digest({"workflow_id": workflow_id, "checkpoint_id": checkpoint_id, "stage_order": list(stage_order), "completed_order": completed, "blocked_order": sorted(blocked), "replay_identity": replay_identity})
    workflow = research_artifact_digest({"workflow_id": workflow_id, "plan_order": list(plan), "checkpoint_digest": checkpoint, "budget_units": budget_units, "consumed_budget_units": consumed, "replay_identity": replay_identity})
    effects = (f"schedule:context-workflow:{workflow_id}",) if disposition == "admitted" else tuple(sorted((*compensation, "block:unsafe-release")))
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "workflow_digest": workflow}), "media_type": "application/vnd.aurora.context-workflow-receipt+json"}
    receipt = ContextWorkflowReceipt(request_id=request_id, workflow_id=workflow_id, query_id=query_id, goal=goal, disposition=disposition, stage_order=stage_order, plan_order=plan, completed_order=tuple(completed), blocked_order=tuple(sorted(blocked)), compensation_order=tuple(sorted(compensation)), checkpoint_digest=checkpoint, workflow_digest=workflow, context_digest=context_digest, replay_identity=replay_identity, budget_units=budget_units, consumed_budget_units=consumed, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
