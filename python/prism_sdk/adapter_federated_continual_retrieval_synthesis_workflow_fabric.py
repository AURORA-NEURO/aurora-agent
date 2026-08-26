"""Python parity surface for AFA-adapter-P02-F16.

An A2 federated-continual workflow that keeps raw observations institution-local,
admits a purpose-bound peer quorum, and emits replayable aggregate evidence.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Sequence

from .adapter_federated_continual_retrieval_synthesis_research_copilot import (
    FederatedContinualRetrievalSynthesisCandidate,
    run_federated_continual_retrieval_synthesis_research_copilot,
)
from .research_contracts import (
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION,
    ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)

CANONICAL_STAGES = (
    "stage:checkpoint", "stage:validate-federation", "stage:admit-peer-quorum",
    "stage:compile-synthesis", "stage:seal-aggregate-envelope",
    "stage:persist-artifact", "stage:validate-output",
)


@dataclass(frozen=True)
class FederatedContinualRetrievalSynthesisWorkflowReceipt:
    request_id: str; workflow_id: str; query_id: str; batch_id: str
    checkpoint_id: str; checkpoint_seq: int; capacity: int; queue_digest: str
    comparability_digest: str; federation_id: str; purpose: str; peer_ids: tuple[str, ...]
    min_peer_quorum: int; aggregate_only: bool; endpoint: str; federation_digest: str
    disposition: str; stage_order: tuple[str, ...]; plan_order: tuple[str, ...]
    completed_order: tuple[str, ...]; blocked_order: tuple[str, ...]
    compensation_order: tuple[str, ...]; candidate_order: tuple[str, ...]
    selected_order: tuple[str, ...]; omitted_order: tuple[str, ...]
    overflow_order: tuple[str, ...]; uncertainty_order: tuple[str, ...]
    negative_order: tuple[str, ...]; contradictory_order: tuple[str, ...]
    synthesis_receipt_digest: str; workflow_digest: str; checkpoint_digest: str
    replay_identity: str; budget_units: int; required_budget: int
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]; effect_receipts: tuple[str, ...]
    artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    contract_version: str = ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION
    feature_id: str = ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or
            self.contract_version != ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION or
            self.feature_id != ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID or
            self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or
            not self.request_id.strip() or not self.workflow_id.strip() or not self.query_id.strip() or
            not self.batch_id.strip() or not self.checkpoint_id.strip() or self.checkpoint_seq <= 0 or
            self.capacity <= 0 or not self.federation_id.strip() or not self.purpose.strip() or
            tuple(sorted(set(self.peer_ids))) != self.peer_ids or
            len(self.peer_ids) < self.min_peer_quorum or self.min_peer_quorum <= 0 or not self.aggregate_only or
            not self.endpoint.strip() or self.stage_order != CANONICAL_STAGES or not self.plan_order or
            not self.effect_receipts or self.budget_units <= 0 or self.required_budget != len(self.plan_order)):
            raise ResearchContractError("federated workflow identity, quorum, stages, locality, budget, or effects are incomplete")
        for value in (self.queue_digest, self.comparability_digest, self.federation_digest,
                      self.synthesis_receipt_digest, self.workflow_digest, self.checkpoint_digest,
                      self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("federated workflow digest is invalid")
        for values in (self.peer_ids, self.plan_order, self.blocked_order,
                       self.compensation_order, self.candidate_order, self.selected_order,
                       self.omitted_order, self.overflow_order, self.uncertainty_order,
                       self.negative_order, self.contradictory_order, self.omissions,
                       self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("federated workflow ordering is not canonical")
        if set(self.selected_order) | set(self.omitted_order) != set(self.candidate_order):
            raise ResearchContractError("federated workflow evidence states do not partition candidates")
        if not set(self.overflow_order).issubset(self.omitted_order):
            raise ResearchContractError("federated overflow must be an omitted candidate subset")
        blocked = bool(self.blocked_order)
        if blocked and self.completed_order:
            raise ResearchContractError("blocked federated workflow cannot report completed stages")
        if not blocked and self.completed_order != self.stage_order:
            raise ResearchContractError("unblocked federated workflow must complete every stage")
        expected = ("block:unsafe-release" if self.disposition == "blocked" else
                    f"compensate:research-work:{self.workflow_id}" if self.compensation_order else
                    f"schedule:research-work:{self.workflow_id}")
        if self.effect_receipts != (expected,):
            raise ResearchContractError("federated workflow effect does not match disposition and compensation")


def run_federated_continual_retrieval_synthesis_workflow(
    *, request_id: str, query_id: str, requester: str, intent: str,
    study_ids: Sequence[str], required_modalities: Sequence[str], comparability_profile: str,
    max_results: int, candidates: Sequence[FederatedContinualRetrievalSynthesisCandidate],
    copilot_id: str, algorithm_version: str, tool_id: str, comparability_digest: str,
    batch_id: str, checkpoint_seq: int, capacity: int, queue_digest: str, checkpoint_digest: str,
    federation_id: str, purpose: str, peer_ids: Sequence[str], min_peer_quorum: int,
    aggregate_only: bool, endpoint: str, federation_digest: str, workflow_id: str,
    requested_stage_order: Sequence[str], checkpoint_id: str, budget_units: int,
    replay_identity: str, approval_token: str = "approval:workflow-f16", policy_allow: bool = True,
    protected_closure: bool = True, raw_data_local: bool = True,
    boundary: str = PRECLINICAL_BOUNDARY,
) -> FederatedContinualRetrievalSynthesisWorkflowReceipt:
    hex64 = lambda value: isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None
    if (not request_id.strip() or not query_id.strip() or not requester.strip() or not intent.strip() or
        not study_ids or not required_modalities or not comparability_profile.strip() or max_results <= 0 or
        not candidates or not workflow_id.strip() or not checkpoint_id.strip() or checkpoint_seq <= 0 or
        capacity <= 0 or budget_units <= 0 or tuple(requested_stage_order) != CANONICAL_STAGES or
        boundary != PRECLINICAL_BOUNDARY or not raw_data_local or not aggregate_only or
        not federation_id.strip() or not purpose.strip() or not peer_ids or
        tuple(sorted(set(peer_ids))) != tuple(peer_ids) or min_peer_quorum <= 0 or
        len(peer_ids) < min_peer_quorum or not endpoint.strip() or
        not all(hex64(value) for value in (replay_identity, comparability_digest, queue_digest,
                                           checkpoint_digest, federation_digest))):
        raise ResearchContractError("federated workflow identity, quorum, stages, budget, locality, replay, or boundary is invalid")
    copilot = run_federated_continual_retrieval_synthesis_research_copilot(
        request_id=request_id, query_id=query_id, requester=requester, intent=intent,
        study_ids=study_ids, required_modalities=required_modalities,
        comparability_profile=comparability_profile, max_results=max_results, candidates=candidates,
        copilot_id=copilot_id, algorithm_version=algorithm_version, tool_id=tool_id,
        comparability_digest=comparability_digest, batch_id=batch_id, checkpoint_seq=checkpoint_seq,
        capacity=capacity, queue_digest=queue_digest, checkpoint_digest=checkpoint_digest,
        federation_id=federation_id, purpose=purpose, peer_ids=peer_ids,
        min_peer_quorum=min_peer_quorum, aggregate_only=aggregate_only, endpoint=endpoint,
        federation_digest=federation_digest, approval_token=approval_token,
        replay_identity=replay_identity, policy_allow=policy_allow,
        protected_closure_satisfied=protected_closure, raw_data_local=raw_data_local, boundary=boundary)
    stage_order = CANONICAL_STAGES
    plan_order = tuple(sorted((*tuple(f"plan:{stage}" for stage in stage_order),
                               "plan:retain-denied-federation", "plan:retain-overflow",
                               "plan:persist-replayable-artifact")))
    required_budget = len(plan_order)
    budget_blocked = budget_units < required_budget
    quorum_blocked = len(peer_ids) < min_peer_quorum
    blocked_gate = budget_blocked or quorum_blocked or not policy_allow or not protected_closure or not raw_data_local or not aggregate_only or copilot.disposition == "blocked"
    disposition = "blocked" if blocked_gate else copilot.disposition
    completed_order = () if blocked_gate else stage_order
    blocked_order = ("stage:release",) if blocked_gate else ()
    compensation = set()
    if budget_blocked: compensation.add("compensate:research-work:budget-exhausted")
    if quorum_blocked: compensation.add("compensate:research-work:peer-quorum")
    if copilot.overflow_order: compensation.add("compensate:research-work:retain-overflow")
    if copilot.omitted_order or copilot.uncertainty_order: compensation.add("compensate:research-work:retain-unresolved-evidence")
    if not policy_allow: compensation.add("compensate:research-work:policy-review")
    if not aggregate_only: compensation.add("compensate:research-work:raw-data-exchange-denied")
    compensation_order = tuple(sorted(compensation))
    omissions = {f"evidence:{item}:overflow" for item in copilot.overflow_order}
    omissions.update(f"evidence:{item}:omitted" for item in copilot.omitted_order)
    omissions.update(f"evidence:{item}:contradictory" for item in copilot.contradictory_order)
    if not policy_allow: omissions.add("workflow:policy-denied")
    if not protected_closure: omissions.add("workflow:protected-closure-incomplete")
    if not raw_data_local: omissions.add("workflow:raw-data-locality-failed")
    if not aggregate_only: omissions.add("federation:raw-data-exchange-denied")
    if quorum_blocked: omissions.add("federation:peer-quorum-unmet")
    if budget_blocked: omissions.add("workflow:budget-exhausted")
    omissions_order = tuple(sorted(omissions))
    uncertainty = set(copilot.uncertainty_order)
    if budget_blocked: uncertainty.add("workflow:budget-unmeasured")
    if quorum_blocked: uncertainty.add("federation:quorum-unmeasured")
    uncertainty_order = tuple(sorted(uncertainty))
    synthesis_receipt_digest = research_artifact_digest(copilot.__dict__)
    checkpoint_digest_value = research_artifact_digest({"workflow_id": workflow_id, "checkpoint_id": checkpoint_id, "checkpoint_seq": checkpoint_seq, "stage_order": list(stage_order), "replay_identity": replay_identity})
    workflow_digest = research_artifact_digest({"workflow_id": workflow_id, "batch_id": batch_id, "federation_id": federation_id, "peer_ids": list(peer_ids), "plan_order": list(plan_order), "completed_order": list(completed_order), "blocked_order": list(blocked_order), "compensation_order": list(compensation_order), "checkpoint_digest": checkpoint_digest_value, "queue_digest": queue_digest, "budget_units": budget_units, "required_budget": required_budget, "replay_identity": replay_identity})
    effect = ("block:unsafe-release",) if disposition == "blocked" else (f"compensate:research-work:{workflow_id}",) if compensation_order else (f"schedule:research-work:{workflow_id}",)
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_CONTRACT_VERSION, "feature_id": ADAPTER_FEDERATED_CONTINUAL_RETRIEVAL_SYNTHESIS_WORKFLOW_FABRIC_FEATURE_ID, "request_id": request_id, "workflow_id": workflow_id, "query_id": query_id, "batch_id": batch_id, "checkpoint_id": checkpoint_id, "checkpoint_seq": checkpoint_seq, "capacity": capacity, "queue_digest": queue_digest, "comparability_digest": comparability_digest, "federation_id": federation_id, "purpose": purpose, "peer_ids": list(peer_ids), "min_peer_quorum": min_peer_quorum, "aggregate_only": aggregate_only, "endpoint": endpoint, "federation_digest": federation_digest, "disposition": disposition, "stage_order": list(stage_order), "plan_order": list(plan_order), "completed_order": list(completed_order), "blocked_order": list(blocked_order), "compensation_order": list(compensation_order), "candidate_order": list(copilot.candidate_order), "selected_order": list(copilot.selected_order), "omitted_order": list(copilot.omitted_order), "overflow_order": list(copilot.overflow_order), "uncertainty_order": list(copilot.uncertainty_order), "negative_order": list(copilot.negative_order), "contradictory_order": list(copilot.contradictory_order), "synthesis_receipt_digest": synthesis_receipt_digest, "workflow_digest": workflow_digest, "checkpoint_digest": checkpoint_digest_value, "replay_identity": replay_identity, "budget_units": budget_units, "required_budget": required_budget, "omissions": list(omissions_order), "uncertainty": list(uncertainty_order), "negative_evidence": list(copilot.negative_order), "effect_receipts": list(effect), "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    receipt = FederatedContinualRetrievalSynthesisWorkflowReceipt(request_id=request_id, workflow_id=workflow_id, query_id=query_id, batch_id=batch_id, checkpoint_id=checkpoint_id, checkpoint_seq=checkpoint_seq, capacity=capacity, queue_digest=queue_digest, comparability_digest=comparability_digest, federation_id=federation_id, purpose=purpose, peer_ids=tuple(peer_ids), min_peer_quorum=min_peer_quorum, aggregate_only=aggregate_only, endpoint=endpoint, federation_digest=federation_digest, disposition=disposition, stage_order=stage_order, plan_order=plan_order, completed_order=completed_order, blocked_order=blocked_order, compensation_order=compensation_order, candidate_order=tuple(copilot.candidate_order), selected_order=tuple(copilot.selected_order), omitted_order=tuple(copilot.omitted_order), overflow_order=tuple(copilot.overflow_order), uncertainty_order=tuple(copilot.uncertainty_order), negative_order=tuple(copilot.negative_order), contradictory_order=tuple(copilot.contradictory_order), synthesis_receipt_digest=synthesis_receipt_digest, workflow_digest=workflow_digest, checkpoint_digest=checkpoint_digest_value, replay_identity=replay_identity, budget_units=budget_units, required_budget=required_budget, omissions=omissions_order, uncertainty=uncertainty_order, negative_evidence=tuple(copilot.negative_order), effect_receipts=effect, artifact={"content_hash": research_artifact_digest(payload), "media_type": "application/vnd.aurora.federated-continual-retrieval-synthesis-workflow+json"})
    receipt.validate()
    return receipt
