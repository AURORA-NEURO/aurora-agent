"""Python parity surface for AFA-adapter-P02-F13.

This is a resumable local workflow around the typed retrieval copilot.  It
keeps stage, checkpoint, budget, compensation, replay, omission, and negative
evidence receipts explicit; it never turns a partial synthesis into a pass.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .adapter_local_retrieval_synthesis_research_copilot import (
    LocalRetrievalSynthesisCandidate,
    run_local_retrieval_synthesis_research_copilot,
)
from .research_contracts import (
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION,
    ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

CANONICAL_STAGES = (
    "stage:checkpoint",
    "stage:compile-synthesis",
    "stage:persist-artifact",
    "stage:validate-input",
)


@dataclass(frozen=True)
class LocalRetrievalSynthesisWorkflowReceipt:
    request_id: str
    workflow_id: str
    query_id: str
    study_id: str
    scope: str
    checkpoint_id: str
    disposition: str
    stage_order: tuple[str, ...]
    plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]
    candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]
    omitted_order: tuple[str, ...]
    uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]
    contradictory_order: tuple[str, ...]
    synthesis_receipt_digest: str
    checkpoint_digest: str
    workflow_digest: str
    replay_identity: str
    budget_units: int
    required_budget: int
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION
    feature_id: str = ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (
            self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            or self.contract_version != ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION
            or self.feature_id != ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID
            or self.boundary != PRECLINICAL_BOUNDARY
            or not self.raw_data_local
            or not self.request_id.strip()
            or not self.workflow_id.strip()
            or not self.query_id.strip()
            or not self.study_id.strip()
            or not self.scope.strip()
            or not self.checkpoint_id.strip()
            or self.stage_order != CANONICAL_STAGES
            or not self.plan_order
            or not self.effect_receipts
            or self.budget_units <= 0
            or self.required_budget <= 0
            or self.required_budget != len(self.plan_order)
        ):
            raise ResearchContractError("workflow identity, stages, plan, locality, budget, or effects are incomplete")
        for values in (
            self.plan_order, self.blocked_order, self.compensation_order,
            self.candidate_order, self.selected_order, self.omitted_order,
            self.uncertainty_order, self.negative_order, self.contradictory_order,
            self.omissions, self.uncertainty, self.negative_evidence,
            self.effect_receipts,
        ):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("workflow ordering is not canonical")
        if set(self.selected_order) | set(self.omitted_order) != set(self.candidate_order):
            raise ResearchContractError("workflow evidence states do not partition candidates")
        blocked = bool(self.blocked_order)
        if blocked and self.completed_order:
            raise ResearchContractError("blocked workflow cannot report completed stages")
        if not blocked and self.completed_order != self.stage_order:
            raise ResearchContractError("unblocked workflow must complete every stage")
        for value in (
            self.synthesis_receipt_digest, self.checkpoint_digest,
            self.workflow_digest, self.replay_identity,
            self.artifact.get("content_hash"),
        ):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("workflow digest is invalid")
        expected_effect = (
            "block:unsafe-release" if self.disposition == "blocked"
            else f"compensate:research-work:{self.workflow_id}" if self.compensation_order
            else f"schedule:research-work:{self.workflow_id}"
        )
        if self.effect_receipts != (expected_effect,):
            raise ResearchContractError("workflow effect does not match disposition and compensation")


def run_local_retrieval_synthesis_workflow(
    *,
    request_id: str,
    query_id: str,
    requester: str,
    intent: str,
    study_ids: Sequence[str],
    required_modalities: Sequence[str],
    comparability_profile: str,
    max_results: int,
    candidates: Sequence[LocalRetrievalSynthesisCandidate],
    copilot_id: str,
    algorithm_version: str,
    workflow_id: str,
    requested_stage_order: Sequence[str],
    checkpoint_id: str,
    budget_units: int,
    replay_identity: str,
    policy_allow: bool = True,
    protected_closure: bool = True,
    raw_data_local: bool = True,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> LocalRetrievalSynthesisWorkflowReceipt:
    if (
        not request_id.strip() or not query_id.strip() or not requester.strip()
        or not intent.strip() or len(study_ids) != 1 or not study_ids[0].strip()
        or not required_modalities or max_results <= 0 or not candidates
        or not workflow_id.strip() or not checkpoint_id.strip() or budget_units <= 0
        or tuple(requested_stage_order) != CANONICAL_STAGES
        or boundary != PRECLINICAL_BOUNDARY or not raw_data_local
        or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)
    ):
        raise ResearchContractError("workflow identity, single-study query, stages, budget, locality, replay, or boundary is invalid")
    copilot = run_local_retrieval_synthesis_research_copilot(
        request_id=request_id, query_id=query_id, requester=requester,
        intent=intent, study_ids=study_ids, required_modalities=required_modalities,
        comparability_profile=comparability_profile, max_results=max_results,
        candidates=candidates, copilot_id=copilot_id, algorithm_version=algorithm_version,
        replay_identity=replay_identity, policy_allow=policy_allow,
        protected_closure_satisfied=protected_closure, raw_data_local=raw_data_local,
        boundary=boundary,
    )
    stage_order = CANONICAL_STAGES
    plan_order = tuple(sorted([*(f"plan:{stage}" for stage in stage_order), "plan:retain-evidence-state", "plan:persist-replayable-artifact"]))
    required_budget = len(plan_order)
    budget_blocked = budget_units < required_budget
    blocked_gate = budget_blocked or not policy_allow or not protected_closure or not raw_data_local or copilot.disposition == "blocked"
    disposition = "blocked" if blocked_gate else copilot.disposition
    completed_order = () if blocked_gate else stage_order
    blocked_order = ("stage:release",) if blocked_gate else ()
    compensation = set()
    if budget_blocked:
        compensation.add("compensate:research-work:budget-exhausted")
    if copilot.omitted_order:
        compensation.add("compensate:research-work:retain-omitted-evidence")
    if not policy_allow:
        compensation.add("compensate:research-work:policy-review")
    compensation_order = tuple(sorted(compensation))
    omissions = {f"evidence:{item}:omitted" for item in copilot.omitted_order}
    if not policy_allow:
        omissions.add("workflow:policy-denied")
    if not protected_closure:
        omissions.add("workflow:protected-closure-incomplete")
    if budget_blocked:
        omissions.add("workflow:budget-exhausted")
    omissions_order = tuple(sorted(omissions))
    uncertainty = set(copilot.uncertainty_order)
    if budget_blocked:
        uncertainty.add("workflow:budget-unmeasured")
    uncertainty_order = tuple(sorted(uncertainty))
    synthesis_receipt_digest = research_artifact_digest(copilot.__dict__)
    checkpoint_digest = research_artifact_digest({"workflow_id": workflow_id, "checkpoint_id": checkpoint_id, "stage_order": list(stage_order), "replay_identity": replay_identity})
    workflow_digest = research_artifact_digest({"workflow_id": workflow_id, "plan_order": list(plan_order), "completed_order": list(completed_order), "blocked_order": list(blocked_order), "compensation_order": list(compensation_order), "checkpoint_digest": checkpoint_digest, "budget_units": budget_units, "required_budget": required_budget, "replay_identity": replay_identity})
    effect = (
        ("block:unsafe-release",) if disposition == "blocked"
        else (f"compensate:research-work:{workflow_id}",) if compensation_order
        else (f"schedule:research-work:{workflow_id}",)
    )
    payload = {
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION,
        "feature_id": ADAPTER_LOCAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID,
        "request_id": request_id, "workflow_id": workflow_id, "query_id": query_id,
        "study_id": study_ids[0], "scope": intent, "checkpoint_id": checkpoint_id,
        "disposition": disposition, "stage_order": list(stage_order),
        "plan_order": list(plan_order), "completed_order": list(completed_order),
        "blocked_order": list(blocked_order), "compensation_order": list(compensation_order),
        "candidate_order": list(copilot.candidate_order), "selected_order": list(copilot.selected_order),
        "omitted_order": list(copilot.omitted_order), "uncertainty_order": list(copilot.uncertainty_order),
        "negative_order": list(copilot.negative_order), "contradictory_order": list(copilot.contradictory_order),
        "synthesis_receipt_digest": synthesis_receipt_digest, "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest, "replay_identity": replay_identity,
        "budget_units": budget_units, "required_budget": required_budget,
        "omissions": list(omissions_order), "uncertainty": list(uncertainty_order),
        "negative_evidence": list(copilot.negative_order), "effect_receipts": list(effect),
        "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY,
    }
    receipt = LocalRetrievalSynthesisWorkflowReceipt(
        request_id=request_id, workflow_id=workflow_id, query_id=query_id,
        study_id=study_ids[0], scope=intent, checkpoint_id=checkpoint_id,
        disposition=disposition, stage_order=stage_order, plan_order=plan_order,
        completed_order=completed_order, blocked_order=blocked_order,
        compensation_order=compensation_order, candidate_order=tuple(copilot.candidate_order),
        selected_order=tuple(copilot.selected_order), omitted_order=tuple(copilot.omitted_order),
        uncertainty_order=tuple(copilot.uncertainty_order), negative_order=tuple(copilot.negative_order),
        contradictory_order=tuple(copilot.contradictory_order),
        synthesis_receipt_digest=synthesis_receipt_digest, checkpoint_digest=checkpoint_digest,
        workflow_digest=workflow_digest, replay_identity=replay_identity,
        budget_units=budget_units, required_budget=required_budget,
        omissions=omissions_order, uncertainty=uncertainty_order,
        negative_evidence=tuple(copilot.negative_order), effect_receipts=effect,
        artifact={"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.local-retrieval-synthesis-workflow+json"},
    )
    receipt.validate()
    return receipt
