from __future__ import annotations

import json
import threading

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousEvidencePlan,
    AutonomousEvidenceReconciliationPlan,
    AutonomousEvidenceReconciliationResult,
    AutonomousEvidenceReconciliationRoute,
    AutonomousEvidenceRequirement,
    AutonomousEvidenceSourceReconciler,
    ArgumentError,
    content_digest,
)


def _plan(domain: str) -> tuple[AutonomousEvidencePlan, AutonomousEvidenceRequirement]:
    workflow_digest = content_digest({"workflow": domain, "version": 1})
    requirement = AutonomousEvidenceRequirement(
        requirement_id=f"{domain}:answer:answer",
        domain=domain,
        workflow_id=f"{domain}:answer",
        workflow_digest=workflow_digest,
        stage_id="answer",
        label="answer",
        objective=f"Reconcile bounded evidence for {domain}.",
        required_capabilities=("evidence_reconciliation",),
        evaluator_signals=("agreement",),
    )
    return (
        AutonomousEvidencePlan(
            domains=(domain,),
            workflow_ids=(requirement.workflow_id,),
            workflow_digests=(workflow_digest,),
            requirements=(requirement,),
            missing_requirement_ids=(requirement.requirement_id,),
            coverage_status="not_evaluated",
        ),
        requirement,
    )


class _StaticAcquirer:
    def __init__(self, value: object = None, error: Exception | None = None, tracker: dict[str, int] | None = None) -> None:
        self.value = value
        self.error = error
        self.tracker = tracker

    def acquire(self, _context: dict[str, object]) -> object:
        if self.tracker is not None:
            self.tracker["calls"] = self.tracker.get("calls", 0) + 1
        if self.error is not None:
            raise self.error
        return self.value


def _routes(domain: str, values: tuple[object, ...], *, tracker: dict[str, int] | None = None) -> tuple[AutonomousEvidenceReconciliationRoute, ...]:
    return tuple(
        AutonomousEvidenceReconciliationRoute(
            source_id=f"source-{index}-{domain}",
            source_digest=content_digest({"source": domain, "route": index}),
            request_id=f"request-{index}-{domain}",
            metadata={"operation": "lookup", "domain": domain, "route": index},
            acquirer=_StaticAcquirer(value, tracker=tracker),
        )
        for index, value in enumerate(values)
    )


def test_reconciliation_covers_all_domains_with_consensus_dissent_and_restart_round_trips() -> None:
    for domain in AUTONOMOUS_DOMAINS:
        evidence_plan, requirement = _plan(domain)
        reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
        plan = reconciler.prepare(
            requirement.requirement_id,
            _routes(domain, ({"answer": "same"}, {"answer": "same"}, {"answer": "different"})),
            quorum=2,
            max_concurrency=2,
            parent_evidence_digests=(content_digest({"parent": domain}),),
        )
        result = reconciler.execute(
            plan,
            _routes(domain, ({"answer": "same"}, {"answer": "same"}, {"answer": "different"})),
            approve_source_dispatch=True,
        )
        assert result.status == "consensus_with_dissent", domain
        assert result.consensus_normalized_digest == content_digest({"answer": "same"})
        assert result.disagreement_digest is not None
        assert result.values["source-0-" + domain] == {"answer": "same"}
        assert result.normalized_values["source-2-" + domain] == {"answer": "different"}
        plan_wire = plan.to_dict()
        result_wire = result.to_dict()
        assert "same" not in json.dumps(plan_wire)
        assert "same" not in json.dumps(result_wire)
        assert AutonomousEvidenceReconciliationPlan.from_dict(plan_wire).to_dict() == plan_wire
        assert AutonomousEvidenceReconciliationResult.from_dict(result_wire).to_dict() == result_wire
        assert result_wire["observed_count"] == 3
        assert result_wire["failed_count"] == 0


def test_reconciliation_requires_approval_and_rejects_route_or_normalizer_drift() -> None:
    evidence_plan, requirement = _plan("science")
    reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
    routes = _routes("science", ({"answer": "A"}, {"answer": "a"}))
    plan = reconciler.prepare(requirement.requirement_id, routes, quorum=2, normalizer_id="lower", normalizer_version="1")
    with pytest.raises(ArgumentError, match="explicit approval"):
        reconciler.execute(plan, routes)

    changed = (
        routes[0],
        AutonomousEvidenceReconciliationRoute(
            source_id=routes[1].source_id,
            source_digest=routes[1].source_digest,
            request_id=routes[1].request_id,
            metadata={"operation": "changed", "domain": "science", "route": 1},
            acquirer=routes[1].acquirer,
        ),
    )
    with pytest.raises(ArgumentError, match="route changed"):
        reconciler.execute(plan, changed, approve_source_dispatch=True, normalizer=lambda value, _context: value)
    with pytest.raises(ArgumentError, match="normalizer contract"):
        reconciler.execute(plan, routes, approve_source_dispatch=True, normalizer=lambda value, _context: value, normalizer_id="other")

    normalized = reconciler.execute(
        plan,
        routes,
        approve_source_dispatch=True,
        normalizer=lambda value, _context: {"answer": value["answer"].lower()},
        normalizer_id="lower",
        normalizer_version="1",
    )
    assert normalized.status == "consensus"
    assert normalized.consensus_normalized_digest == content_digest({"answer": "a"})


def test_reconciliation_distinguishes_disagreement_from_insufficient_evidence_and_redacts_failures() -> None:
    evidence_plan, requirement = _plan("coding")
    tracker: dict[str, int] = {}
    reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
    disagreement_routes = (
        AutonomousEvidenceReconciliationRoute("source-a", content_digest("a"), "request-a", {"route": "a"}, _StaticAcquirer({"answer": "a"}, tracker=tracker)),
        AutonomousEvidenceReconciliationRoute("source-b", content_digest("b"), "request-b", {"route": "b"}, _StaticAcquirer({"answer": "b"}, tracker=tracker)),
        AutonomousEvidenceReconciliationRoute("source-c", content_digest("c"), "request-c", {"route": "c"}, _StaticAcquirer(error=RuntimeError("provider payload must not cross"), tracker=tracker)),
    )
    plan = reconciler.prepare(requirement.requirement_id, disagreement_routes, quorum=2)
    result = reconciler.execute(plan, disagreement_routes, approve_source_dispatch=True)
    assert result.status == "disagreement"
    assert result.consensus_normalized_digest is None
    assert result.source_results[-1].status == "failed"
    assert "provider payload" not in json.dumps(result.to_dict())
    assert tracker["calls"] == 3

    require_all = reconciler.prepare(requirement.requirement_id, disagreement_routes, quorum=1, require_all=True)
    all_result = reconciler.execute(require_all, disagreement_routes, approve_source_dispatch=True)
    assert all_result.status == "insufficient_evidence"

    tampered = json.loads(json.dumps(result.to_dict()))
    tampered["source_results"][0]["value_digest"] = content_digest({"tampered": True})
    with pytest.raises(ArgumentError):
        AutonomousEvidenceReconciliationResult.from_dict(tampered)


def test_reconciliation_bounds_concurrent_route_execution() -> None:
    evidence_plan, requirement = _plan("operations")
    state = {"active": 0, "max_active": 0}
    lock = threading.Lock()

    class TrackedAcquirer:
        def acquire(self, _context: dict[str, object]) -> dict[str, str]:
            with lock:
                state["active"] += 1
                state["max_active"] = max(state["max_active"], state["active"])
            with lock:
                state["active"] -= 1
            return {"answer": "bounded"}

    routes = tuple(
        AutonomousEvidenceReconciliationRoute(
            source_id=f"bounded-{index}", source_digest=content_digest(index), request_id=None,
            metadata={"route": index}, acquirer=TrackedAcquirer(),
        )
        for index in range(4)
    )
    reconciler = AutonomousEvidenceSourceReconciler(evidence_plan)
    plan = reconciler.prepare(requirement.requirement_id, routes, quorum=3, max_concurrency=2)
    result = reconciler.execute(plan, routes, approve_source_dispatch=True)
    assert result.status == "consensus"
    assert state["max_active"] <= 2
