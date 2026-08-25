"""High-throughput context workbench parity contract.

The implementation is deliberately admission-oriented: every queued job is classified,
resource limits remain visible, and only a fully admitted local batch exposes release actions.
"""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .research_contracts import (
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION,
    THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID,
    research_artifact_digest,
)


@dataclass(frozen=True)
class ThroughputContextWorkbenchJob:
    job_id: str
    context_digest: str
    replay_identity: str
    state: str = "supported"
    ready: bool = True
    cost_units: int = 1
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY


@dataclass(frozen=True)
class ThroughputContextWorkbenchReceipt:
    session_id: str
    query_id: str
    goal: str
    disposition: str
    queue_order: tuple[str, ...]
    admitted_job_order: tuple[str, ...]
    blocked_job_order: tuple[str, ...]
    unknown_job_order: tuple[str, ...]
    view_order: tuple[str, ...]
    action_order: tuple[str, ...]
    blocked_action_order: tuple[str, ...]
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
    feature_id: str = THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID
    contract_version: str = THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if (self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
                or self.contract_version != THROUGHPUT_CONTEXT_WORKBENCH_CONTRACT_VERSION
                or self.feature_id != THROUGHPUT_CONTEXT_WORKBENCH_FEATURE_ID):
            raise ResearchContractError("throughput workbench schema, feature, or version mismatch")
        if (self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local
                or not self.session_id.strip() or not self.query_id.strip() or not self.goal.strip()
                or not self.queue_order or not self.view_order or not self.action_order
                or not self.effect_receipts or self.concurrency <= 0 or self.budget_units <= 0
                or self.consumed_budget_units > self.budget_units
                or self.disposition not in {"ready", "needs_refinement", "blocked"}):
            raise ResearchContractError("throughput workbench identity, queue, budget, concurrency, view, action, locality, or disposition is incomplete")
        for values in (self.queue_order, self.admitted_job_order, self.blocked_job_order,
                       self.unknown_job_order, self.view_order, self.action_order,
                       self.blocked_action_order, self.omissions, self.uncertainty,
                       self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("throughput workbench vectors are not canonical")
        classified = set(self.admitted_job_order) | set(self.blocked_job_order) | set(self.unknown_job_order)
        if classified != set(self.queue_order):
            raise ResearchContractError("throughput jobs do not partition outcomes")
        for value in (self.batch_digest, self.replay_identity, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
                raise ResearchContractError("throughput workbench digest is invalid")
        if any(not effect.startswith("view:local-throughput-workbench:") and effect != "block:unsafe-release"
               for effect in self.effect_receipts):
            raise ResearchContractError("throughput workbench effect is outside read-only view gate")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "session_id": self.session_id,
            "query_id": self.query_id,
            "goal": self.goal,
            "disposition": self.disposition,
            "queue_order": list(self.queue_order),
            "admitted_job_order": list(self.admitted_job_order),
            "blocked_job_order": list(self.blocked_job_order),
            "unknown_job_order": list(self.unknown_job_order),
            "view_order": list(self.view_order),
            "action_order": list(self.action_order),
            "blocked_action_order": list(self.blocked_action_order),
            "concurrency": self.concurrency,
            "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units,
            "batch_digest": self.batch_digest,
            "replay_identity": self.replay_identity,
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "effect_receipts": list(self.effect_receipts),
            "artifact": dict(self.artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })


def render_throughput_context_workbench(*, session_id: str, query_id: str, goal: str,
                                        projection_disposition: str,
                                        jobs: Sequence[ThroughputContextWorkbenchJob],
                                        max_concurrency: int, budget_units: int,
                                        replay_identity: str, policy_allow: bool = True,
                                        raw_data_local: bool = True) -> ThroughputContextWorkbenchReceipt:
    if (not session_id.strip() or not query_id.strip() or not goal.strip() or not jobs
            or max_concurrency <= 0 or budget_units <= 0
            or not re.fullmatch(r"[0-9a-f]{64}", replay_identity)):
        raise ResearchContractError("throughput workbench identity, jobs, concurrency, budget, or replay is invalid")
    ordered = tuple(sorted(jobs, key=lambda job: job.job_id))
    queue = tuple(job.job_id for job in ordered)
    if any(not job_id.strip() for job_id in queue) or len(set(queue)) != len(queue):
        raise ResearchContractError("throughput job identifiers must be unique and non-empty")
    admitted: set[str] = set(); blocked: set[str] = set(); unknown: set[str] = set()
    views = {"view:queue", "view:job-state", "view:budget-and-concurrency", "view:replay-identity"}
    actions = {"action:inspect-job", "action:replay-local-batch"}; blocked_actions: set[str] = set()
    omissions: set[str] = set(); uncertainty: set[str] = set(); negative: set[str] = set(); consumed = 0
    gates_open = policy_allow and raw_data_local
    for job in ordered:
        if not gates_open or not job.raw_data_local or job.boundary != PRECLINICAL_BOUNDARY:
            blocked.add(job.job_id); omissions.add(f"job:{job.job_id}:policy-locality-blocked")
        elif job.replay_identity != replay_identity:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:replay-mismatch")
        elif not job.ready:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:not-ready")
        elif job.state in {"speculative", "unknown"}:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:evidence-uncertain")
        elif job.state not in {"proven", "supported"}:
            blocked.add(job.job_id); negative.add(f"job:{job.job_id}:contradicted")
        elif len(admitted) >= max_concurrency:
            unknown.add(job.job_id); uncertainty.add(f"job:{job.job_id}:concurrency-window")
        elif consumed + job.cost_units > budget_units:
            blocked.add(job.job_id); omissions.add(f"job:{job.job_id}:budget-exhausted")
        else:
            admitted.add(job.job_id); consumed += job.cost_units
    if not gates_open:
        omissions.add("workbench:policy-or-locality-blocked"); disposition = "blocked"
    elif projection_disposition == "admitted" and len(admitted) == len(queue):
        actions.update({"action:open-decision-section", "action:export-local-batch"}); disposition = "ready"
    else:
        actions.update({"action:review-queue-outcomes", "action:request-batch-refinement"})
        uncertainty.add("workbench:throughput-projection-not-admitted"); disposition = "needs_refinement"
    if disposition == "blocked":
        blocked_actions.update({"action:open-decision-section", "action:export-local-batch", "action:replay-local-batch"})
        actions = {"action:inspect-block-reason"}
    if unknown: views.add("view:uncertain-jobs")
    if blocked: views.add("view:blocked-jobs")
    batch_digest = research_artifact_digest({"queue_order": list(queue), "admitted_order": sorted(admitted),
                                             "blocked_order": sorted(blocked), "unknown_order": sorted(unknown),
                                             "concurrency": max_concurrency, "budget_units": budget_units,
                                             "consumed_budget_units": consumed, "replay_identity": replay_identity})
    effect_receipts = ("block:unsafe-release",) if disposition == "blocked" else (f"view:local-throughput-workbench:{session_id}",)
    artifact = {"content_hash": research_artifact_digest({"session_id": session_id, "batch_digest": batch_digest}),
                "media_type": "application/vnd.aurora.throughput-context-workbench+json"}
    receipt = ThroughputContextWorkbenchReceipt(
        session_id=session_id, query_id=query_id, goal=goal, disposition=disposition,
        queue_order=queue, admitted_job_order=tuple(sorted(admitted)), blocked_job_order=tuple(sorted(blocked)),
        unknown_job_order=tuple(sorted(unknown)), view_order=tuple(sorted(views)), action_order=tuple(sorted(actions)),
        blocked_action_order=tuple(sorted(blocked_actions)), concurrency=max_concurrency, budget_units=budget_units,
        consumed_budget_units=consumed, batch_digest=batch_digest, replay_identity=replay_identity,
        omissions=tuple(sorted(omissions)), uncertainty=tuple(sorted(uncertainty)), negative_evidence=tuple(sorted(negative)),
        effect_receipts=effect_receipts, artifact=artifact)
    receipt.validate()
    return receipt
