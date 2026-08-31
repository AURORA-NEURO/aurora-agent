"""Prospective high-throughput context workflow fabric parity contract."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION,
    THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ThroughputContextJob:
    job_id: str
    context_digest: str
    replay_identity: str
    state: str = "supported"
    ready: bool = True
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class ThroughputContextWorkflowReceipt:
    request_id: str
    batch_id: str
    query_id: str
    goal: str
    disposition: str
    queue_order: tuple[str, ...]
    scheduled_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    concurrency: int
    budget_units: int
    consumed_budget_units: int
    batch_digest: str
    replay_identity: str
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    effect_receipts: tuple[str, ...]
    artifact: Mapping[str, Any]
    feature_id: str = THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID
    contract_version: str = THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.contract_version != THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_CONTRACT_VERSION or self.feature_id != THROUGHPUT_CONTEXT_WORKFLOW_FABRIC_FEATURE_ID:
            raise ResearchContractError("throughput workflow schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.batch_id.strip() or not self.query_id.strip() or not self.goal.strip() or not self.queue_order or self.concurrency < 1 or self.budget_units < 1 or self.consumed_budget_units > self.budget_units or not self.effect_receipts:
            raise ResearchContractError("throughput workflow identity, queue, concurrency, budget, locality, or effects are incomplete")
        for values in (self.queue_order, self.scheduled_order, self.blocked_order, self.unknown_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput workflow vectors are not canonical")
        classified = set(self.scheduled_order) | set(self.blocked_order) | set(self.unknown_order)
        if classified != set(self.queue_order):
            raise ResearchContractError("throughput jobs do not partition outcomes")
        for value in (self.batch_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput workflow digest is invalid")
        if any(not effect.startswith("schedule:throughput-context-workflow:") and effect != "block:unsafe-release" for effect in self.effect_receipts):
            raise ResearchContractError("throughput workflow effect is outside batch gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({"schema_version": self.schema_version, "contract_version": self.contract_version, "feature_id": self.feature_id, "request_id": self.request_id, "batch_id": self.batch_id, "query_id": self.query_id, "goal": self.goal, "disposition": self.disposition, "queue_order": list(self.queue_order), "scheduled_order": list(self.scheduled_order), "blocked_order": list(self.blocked_order), "unknown_order": list(self.unknown_order), "concurrency": self.concurrency, "budget_units": self.budget_units, "consumed_budget_units": self.consumed_budget_units, "batch_digest": self.batch_digest, "replay_identity": self.replay_identity, "omissions": list(self.omissions), "uncertainty": list(self.uncertainty), "negative_evidence": list(self.negative_evidence), "effect_receipts": list(self.effect_receipts), "artifact": dict(self.artifact), "raw_data_local": self.raw_data_local, "boundary": self.boundary})


def compile_throughput_context_workflow(*, request_id: str, batch_id: str, query_id: str, goal: str, jobs: Sequence[ThroughputContextJob], max_concurrency: int, budget_units: int, replay_identity: str, policy_allow: bool = True, protected_closure: bool = True, raw_data_local: bool = True) -> ThroughputContextWorkflowReceipt:
    if not request_id.strip() or not batch_id.strip() or not query_id.strip() or not goal.strip() or not jobs or max_concurrency < 1 or budget_units < 1 or not re.fullmatch(r"[0-9a-f]{64}", replay_identity):
        raise ResearchContractError("throughput workflow identity, jobs, concurrency, budget, replay, or boundary is invalid")
    ordered = tuple(sorted(jobs, key=lambda job: job.job_id)); queue = tuple(job.job_id for job in ordered)
    if len(set(queue)) != len(queue) or any(not value.strip() for value in queue):
        raise ResearchContractError("throughput job identifiers must be unique and non-empty")
    scheduled: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set(); omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); consumed = 0; gates_open = policy_allow and protected_closure and raw_data_local
    for job in ordered:
        if not gates_open or not job.raw_data_local or job.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(job.job_id); omissions.add(f"job:{job.job_id}:policy-locality-gate-blocked")
        elif job.replay_identity != replay_identity:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:replay-mismatch")
        elif not job.ready:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:not-ready")
        elif job.state in {"proven", "supported"} and consumed < budget_units:
            scheduled.add(job.job_id); consumed += 1
        elif job.state in {"speculative", "unknown"}:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:evidence-uncertain")
        elif job.state == "contradicted":
            blocked.add(job.job_id); negative.add(f"job:{job.job_id}:contradicted")
        else:
            blocked.add(job.job_id); omissions.add(f"job:{job.job_id}:budget-exhausted")
    disposition = "blocked" if not gates_open else ("admitted" if len(scheduled) == len(queue) else "refinement_required")
    if budget_units < len(queue): omissions.add("workflow:budget-exhausted")
    if not policy_allow: omissions.add("workflow:policy-denied")
    if not protected_closure: omissions.add("workflow:protected-closure-incomplete")
    if not raw_data_local: omissions.add("workflow:raw-data-locality-failed")
    batch_digest = research_artifact_digest({"batch_id": batch_id, "queue_order": list(queue), "scheduled_order": sorted(scheduled), "blocked_order": sorted(blocked), "unknown_order": sorted(unknown), "concurrency": max_concurrency, "consumed_budget_units": consumed, "replay_identity": replay_identity})
    effects = (f"schedule:throughput-context-workflow:{batch_id}",) if disposition == "admitted" else ("block:unsafe-release",)
    artifact = {"content_hash": research_artifact_digest({"request_id": request_id, "batch_digest": batch_digest}), "media_type": "application/vnd.aurora.throughput-context-workflow+json"}
    receipt = ThroughputContextWorkflowReceipt(request_id=request_id, batch_id=batch_id, query_id=query_id, goal=goal, disposition=disposition, queue_order=queue, scheduled_order=tuple(sorted(scheduled)), blocked_order=tuple(sorted(blocked)), unknown_order=tuple(sorted(unknown)), concurrency=max_concurrency, budget_units=budget_units, consumed_budget_units=consumed, batch_digest=batch_digest, replay_identity=replay_identity, omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)), effect_receipts=effects, artifact=artifact)
    receipt.validate(); return receipt
