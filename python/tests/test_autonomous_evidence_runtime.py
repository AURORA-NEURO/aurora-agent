from __future__ import annotations

import json
import hashlib

import pytest

from prism_sdk import (
    ArgumentError,
    InMemoryAutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimePersistenceCoordinator,
    AutonomousEvidenceRuntime,
    TransactionalJsonAutonomousEvidenceRuntimeSnapshotPersistence,
    build_autonomous_evidence_plan,
    builtin_autonomous_workflow_strategies,
    validate_autonomous_evidence_runtime_snapshot,
)


class _CasTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


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
    assert snapshot.snapshot_generation == 1
    assert snapshot.previous_snapshot_digest is None
    assert journal.snapshot(plan.plan_digest).to_dict() == snapshot.to_dict()
    legacy = snapshot.to_dict()
    legacy.pop("snapshot_generation")
    legacy.pop("previous_snapshot_digest")
    legacy["schema"] = "bioprism-python-autonomous-evidence-runtime-snapshot/0.1"
    legacy_body = dict(legacy)
    legacy_body.pop("snapshot_digest")
    legacy["snapshot_digest"] = hashlib.sha256(
        json.dumps(legacy_body, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    ).hexdigest()
    assert validate_autonomous_evidence_runtime_snapshot(legacy, expected_plan_digest=plan.plan_digest).to_dict()["schema"] == "bioprism-python-autonomous-evidence-runtime-snapshot/0.1"
    legacy_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    legacy_journal.restore(legacy, plan.plan_digest)
    upgraded = legacy_journal.snapshot(plan.plan_digest)
    assert upgraded.snapshot_generation == 1
    assert upgraded.previous_snapshot_digest is None
    assert upgraded.snapshot_digest != legacy["snapshot_digest"]
    backend = _CasTextStore()
    persistence = TransactionalJsonAutonomousEvidenceRuntimeSnapshotPersistence(backend)
    source_coordinator = AutonomousEvidenceRuntimePersistenceCoordinator(journal, plan.plan_digest, persistence)
    flushed = source_coordinator.flush()
    assert flushed["snapshot_digest"] == snapshot.snapshot_digest
    restarted_coordinator = AutonomousEvidenceRuntimePersistenceCoordinator(
        InMemoryAutonomousEvidenceRuntimeJournal(), plan.plan_digest, persistence
    )
    assert restarted_coordinator.restore()["snapshot_digest"] == flushed["snapshot_digest"]
    backend.value = json.dumps(json.loads(backend.value), indent=2)
    with pytest.raises(ArgumentError, match="not canonical"):
        persistence.read()
    persistence.write(snapshot)
    persistence.write(InMemoryAutonomousEvidenceRuntimeJournal().snapshot(plan.plan_digest))
    with pytest.raises(ArgumentError, match="compare-and-swap conflict"):
        restarted_coordinator.flush()
    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(snapshot, plan.plan_digest)
    restored = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert restored.rehydrate()["restored"] == 1

    missing_value = restored.execute([request], acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("must not reacquire")))
    assert missing_value.status == "reconciliation_required"
    assert missing_value.receipts[0].status == "reconciliation_required"
    assert len(adapters.calls) == 1


def test_evidence_runtime_persists_pending_evaluator_revision_and_accepts_after_restart() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    baseline = build_autonomous_evidence_plan((workflow,))
    plan = build_autonomous_evidence_plan((workflow,), available_evidence=tuple(item.requirement_id for item in baseline.requirements[1:]))
    request = _request(plan.requirements[0])
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    value = {"fixture": "reconcile-me", "requirement": plan.requirements[0].requirement_id}

    class _Projector:
        def project(self, _value, context):
            return [{"label": context["requirement"].label, "status": "observed"}]

    class _PendingEvaluator:
        evaluator_id = "reconciliation-evaluator"
        evaluator_version = "1"

        def evaluate(self, _input_value):
            return {"evaluator_id": self.evaluator_id, "evaluator_version": self.evaluator_version, "verdict": "indeterminate", "score": 0.5}

    class _Acquirer:
        def __init__(self):
            self.calls = 0

        def acquire(self, _context):
            self.calls += 1
            return value

    acquirer = _Acquirer()
    first = AutonomousEvidenceRuntime(plan, journal=journal).execute(
        [request], acquirer=acquirer, projector=_Projector(), evaluator=_PendingEvaluator()
    )
    assert first.status == "awaiting_evaluation"
    assert len(journal.records()) == 1
    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(journal.snapshot(plan.plan_digest), plan.plan_digest)
    restored = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert restored.rehydrate()["restored"] == 1

    class _AcceptedEvaluator:
        evaluator_id = "reconciliation-evaluator"
        evaluator_version = "2"

        def evaluate(self, _input_value):
            return {"evaluator_id": self.evaluator_id, "evaluator_version": self.evaluator_version, "verdict": "accepted", "score": 1}

    accepted = restored.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("pending reconciliation must not reacquire")),
        projector=_Projector(),
        evaluator=_AcceptedEvaluator(),
        rehydrate_value=lambda _receipt: value,
        reevaluate_pending=True,
    )
    assert accepted.status == "awaiting_evaluation", "the revised requirement is accepted while the remaining plan requirements still need evaluator decisions"
    assert accepted.receipts[0].replay == "replayed"
    assert accepted.receipts[0].evaluator_status == "accepted"
    assert len(accepted.assessments) == 1
    assert plan.requirements[0].requirement_id in accepted.completed_requirement_ids
    assert plan.requirements[0].requirement_id not in accepted.pending_evaluation_requirement_ids
    assert not accepted.missing_requirement_ids
    assert len(restored_journal.records()) == 2
    assert acquirer.calls == 1
