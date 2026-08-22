from __future__ import annotations

import pytest

from prism_sdk import (
    ArgumentError,
    InMemoryAutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntime,
    build_autonomous_evidence_plan,
    builtin_autonomous_workflow_strategies,
)


def _request(requirement, index: int = 0) -> dict[str, object]:
    return {
        "requirement_id": requirement.requirement_id,
        "source_id": f"fixture-source-{index}",
        "request_id": f"fixture-request-{index}",
        "metadata": {"fixture": True, "domain": requirement.domain},
    }


class _Adapters:
    evaluator_id = "fixture-evaluator"
    evaluator_version = "2026.08"

    def __init__(self) -> None:
        self.calls: list[str] = []

    def acquire(self, context):
        self.calls.append(context["requirement"].requirement_id)
        return {"fixture": "metadata-only-test-value", "requirement": context["requirement"].requirement_id}

    def project(self, _value, context):
        return [{"label": context["requirement"].label, "kind": "fact", "status": "observed", "confidence": 0.95}]

    def evaluate(self, input_value):
        return {
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "verdict": "accepted",
            "score": 1,
            "evidence_digest": input_value["requirement"].workflow_digest,
        }


def test_evidence_runtime_acquires_and_evaluates_every_builtin_domain_without_retaining_raw_values_in_wire() -> None:
    plan = build_autonomous_evidence_plan(builtin_autonomous_workflow_strategies())
    adapters = _Adapters()
    result = AutonomousEvidenceRuntime(plan).execute(
        [_request(requirement, index) for index, requirement in enumerate(plan.requirements)],
        acquirer=adapters,
        projector=adapters,
        evaluator=adapters,
    )

    assert len(plan.domains) == 12
    assert len(plan.requirements) > 40
    assert len(adapters.calls) == len(plan.requirements)
    assert result.status == "completed"
    assert not result.missing_requirement_ids
    assert not result.pending_evaluation_requirement_ids
    assert len(result.receipts) == len(plan.requirements)
    assert len(result.assessments) == len(plan.requirements)
    assert "values" not in result.to_dict()
    assert result.to_dict()["retention"] == "metadata_only;raw_values_caller_owned"
    assert result.values


def test_evidence_runtime_makes_evaluation_and_acquisition_failures_explicit() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    plan = build_autonomous_evidence_plan((workflow,))
    request = _request(plan.requirements[0])
    adapters = _Adapters()
    no_evaluator = AutonomousEvidenceRuntime(plan).execute([request], acquirer=adapters, projector=adapters)
    assert no_evaluator.status == "awaiting_evaluation"
    assert no_evaluator.receipts[0].evaluator_status == "not_evaluated"

    class _FailingAcquirer:
        def acquire(self, _context):
            raise RuntimeError("fixture acquisition failure")

    failed = AutonomousEvidenceRuntime(plan).execute([request], acquirer=_FailingAcquirer())
    assert failed.status == "failed"
    assert failed.receipts[0].status == "failed"

    with pytest.raises(ArgumentError):
        AutonomousEvidenceRuntime(plan).execute(
            [{**request, "metadata": {"api_key": "must never enter evidence metadata"}}],
            acquirer=adapters,
        )


def test_evidence_runtime_replay_requires_value_reconciliation_after_journal_rehydration() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    plan = build_autonomous_evidence_plan((workflow,))
    request = _request(plan.requirements[0])
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    adapters = _Adapters()
    first = AutonomousEvidenceRuntime(plan, journal=journal).execute([request], acquirer=adapters, projector=adapters)
    assert first.receipts[0].replay == "fresh"
    snapshot = journal.snapshot(plan.plan_digest)
    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(snapshot, plan.plan_digest)
    restored = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert restored.rehydrate()["restored"] == 1

    missing_value = restored.execute([request], acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("must not reacquire")))
    assert missing_value.status == "reconciliation_required"
    assert missing_value.receipts[0].status == "reconciliation_required"
    assert len(adapters.calls) == 1

    replayed = restored.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("must not reacquire")),
        rehydrate_value=lambda _receipt: {"fixture": "metadata-only-test-value", "requirement": plan.requirements[0].requirement_id},
    )
    assert replayed.receipts[0].replay == "replayed"
    assert replayed.receipts[0].status != "reconciliation_required"
    assert len(adapters.calls) == 1
