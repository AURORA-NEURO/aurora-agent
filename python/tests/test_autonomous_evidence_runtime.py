from __future__ import annotations

import json
import hashlib

import pytest

from prism_sdk import (
    ArgumentError,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationError,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
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


def _authorization_context(plan, *, operations=("evidence_acquisition", "evaluation")):
    ledger = AutonomousAuthorizationLedger(max_grants=4, max_events=256)
    grant = ledger.issue(
        grant_id="evidence-runtime-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=plan.domains,
        allowed_operations=operations,
        allowed_capabilities=("analysis",),
        allowed_risk_classes=("read_only",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=None,
    )
    return ledger, AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=grant.authorization_digest,
        domains=tuple(plan.domains),
        capability="analysis",
        risk_class="read_only",
        request_prefix="evidence",
        clock=lambda: 1_200,
    )


def test_evidence_runtime_acquires_and_evaluates_every_builtin_domain_without_retaining_raw_values_in_wire() -> None:
    plan = build_autonomous_evidence_plan(builtin_autonomous_workflow_strategies())
    adapters = _Adapters()
    ledger, authorization_context = _authorization_context(plan)
    result = AutonomousEvidenceRuntime(plan).execute(
        [_request(requirement, index) for index, requirement in enumerate(plan.requirements)],
        acquirer=adapters,
        projector=adapters,
        evaluator=adapters,
        authorization_context=authorization_context,
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
    assert sum(event.event_type == "request_allowed" for event in ledger.events()) == len(plan.requirements) * 2


def test_evidence_runtime_authorization_denies_before_acquisition_and_does_not_record_a_failure() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    plan = build_autonomous_evidence_plan((workflow,))
    ledger, authorization_context = _authorization_context(plan, operations=("evaluation",))
    calls = 0

    def acquire(_context):
        nonlocal calls
        calls += 1
        return {"should_not": "run"}

    with pytest.raises(AutonomousAuthorizationError, match="operation authorization was refused"):
        AutonomousEvidenceRuntime(plan).execute(
            [_request(plan.requirements[0])],
            acquirer=acquire,
            authorization_context=authorization_context,
        )
    assert calls == 0
    assert [event.event_type for event in ledger.events()] == ["grant_issued"]


def test_evidence_runtime_authorization_denies_evaluation_before_callback_and_replay_reuses_without_acquisition_auth() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    plan = build_autonomous_evidence_plan((workflow,))
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    adapters = _Adapters()
    acquisition_ledger, acquisition_context = _authorization_context(plan, operations=("evidence_acquisition",))
    first = AutonomousEvidenceRuntime(plan, journal=journal).execute(
        [_request(plan.requirements[0])],
        acquirer=adapters,
        projector=adapters,
        authorization_context=acquisition_context,
    )
    assert first.status == "awaiting_evaluation"
    assert len(acquisition_ledger.events()) == 2

    evaluation_ledger, evaluation_context = _authorization_context(plan, operations=("evidence_acquisition",))
    evaluator_calls = 0

    class _Evaluator(_Adapters):
        def evaluate(self, input_value):
            nonlocal evaluator_calls
            evaluator_calls += 1
            return super().evaluate(input_value)

    restored = AutonomousEvidenceRuntime(plan, journal=journal)
    restored.rehydrate()
    with pytest.raises(AutonomousAuthorizationError, match="operation authorization was refused"):
        restored.execute(
            [_request(plan.requirements[0])],
            acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("must not reacquire")),
            evaluator=_Evaluator(),
            rehydrate_value=lambda _receipt: {"fixture": "metadata-only-test-value", "requirement": plan.requirements[0].requirement_id},
            authorization_context=evaluation_context,
            reevaluate_pending=True,
        )
    assert evaluator_calls == 0
    assert [event.event_type for event in evaluation_ledger.events()] == ["grant_issued"]


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


def test_evidence_runtime_replay_preserves_missing_output_partial_status() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    baseline = build_autonomous_evidence_plan((workflow,))
    plan = build_autonomous_evidence_plan(
        (workflow,),
        available_evidence=tuple(item.requirement_id for item in baseline.requirements[1:]),
    )
    requirement = plan.requirements[0]
    request = _request(requirement)
    value = {"fixture": "unprojected-value", "requirement": requirement.requirement_id}
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    acquisition_calls = 0

    def acquire(_context):
        nonlocal acquisition_calls
        acquisition_calls += 1
        return value

    fresh = AutonomousEvidenceRuntime(plan, journal=journal).execute([request], acquirer=acquire)
    assert fresh.status == "partial"
    assert fresh.receipts[0].evidence_status == "missing_required_outputs"
    assert not fresh.receipts[0].observed_requirement_ids
    assert not fresh.pending_evaluation_requirement_ids
    assert fresh.missing_requirement_ids == (requirement.requirement_id,)

    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(journal.snapshot(plan.plan_digest), plan.plan_digest)
    replay_runtime = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert replay_runtime.rehydrate()["restored"] == 1
    replayed = replay_runtime.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("replay must not reacquire")),
        rehydrate_value=lambda _receipt: value,
    )

    assert replayed.receipts[0].replay == "replayed"
    assert replayed.status == fresh.status
    assert replayed.pending_evaluation_requirement_ids == fresh.pending_evaluation_requirement_ids
    assert replayed.missing_requirement_ids == fresh.missing_requirement_ids
    assert acquisition_calls == 1


def test_evidence_runtime_replay_preserves_observed_unevaluated_status() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    baseline = build_autonomous_evidence_plan((workflow,))
    plan = build_autonomous_evidence_plan(
        (workflow,),
        available_evidence=tuple(item.requirement_id for item in baseline.requirements[1:]),
    )
    requirement = plan.requirements[0]
    request = _request(requirement)
    value = {"fixture": "observed-value", "requirement": requirement.requirement_id}
    journal = InMemoryAutonomousEvidenceRuntimeJournal()
    projection_calls = 0

    def project(_value, context):
        nonlocal projection_calls
        projection_calls += 1
        return [{"label": context["requirement"].label, "status": "observed"}]

    fresh = AutonomousEvidenceRuntime(plan, journal=journal).execute(
        [request],
        acquirer=lambda _context: value,
        projector=project,
    )
    assert fresh.status == "awaiting_evaluation"
    assert fresh.receipts[0].evidence_status == "declared_for_evaluator"
    assert fresh.pending_evaluation_requirement_ids == (requirement.requirement_id,)
    assert not fresh.missing_requirement_ids

    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(journal.snapshot(plan.plan_digest), plan.plan_digest)
    replay_runtime = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert replay_runtime.rehydrate()["restored"] == 1
    replayed = replay_runtime.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("replay must not reacquire")),
        projector=lambda _value, _context: (_ for _ in ()).throw(RuntimeError("replay must not reproject")),
        rehydrate_value=lambda _receipt: value,
    )

    assert replayed.receipts[0].replay == "replayed"
    assert replayed.status == fresh.status
    assert replayed.pending_evaluation_requirement_ids == fresh.pending_evaluation_requirement_ids
    assert replayed.missing_requirement_ids == fresh.missing_requirement_ids
    assert projection_calls == 1


def test_evidence_runtime_replay_preserves_rejected_assessment_as_pending() -> None:
    workflow = next(item for item in builtin_autonomous_workflow_strategies() if item.domain == "science")
    baseline = build_autonomous_evidence_plan((workflow,))
    plan = build_autonomous_evidence_plan(
        (workflow,),
        available_evidence=tuple(item.requirement_id for item in baseline.requirements[1:]),
    )
    requirement = plan.requirements[0]
    request = _request(requirement)
    value = {"fixture": "rejected-value", "requirement": requirement.requirement_id}
    journal = InMemoryAutonomousEvidenceRuntimeJournal()

    class _RejectingEvaluator:
        evaluator_id = "rejection-evaluator"
        evaluator_version = "1"

        def __init__(self) -> None:
            self.calls = 0

        def evaluate(self, _input_value):
            self.calls += 1
            return {
                "evaluator_id": self.evaluator_id,
                "evaluator_version": self.evaluator_version,
                "verdict": "rejected",
                "score": 0,
            }

    evaluator = _RejectingEvaluator()
    fresh = AutonomousEvidenceRuntime(plan, journal=journal).execute(
        [request],
        acquirer=lambda _context: value,
        projector=lambda _value, context: [{"label": context["requirement"].label, "status": "observed"}],
        evaluator=evaluator,
    )
    assert fresh.status == "awaiting_evaluation"
    assert fresh.assessments[0].verdict == "rejected"
    assert fresh.pending_evaluation_requirement_ids == (requirement.requirement_id,)

    restored_journal = InMemoryAutonomousEvidenceRuntimeJournal()
    restored_journal.restore(journal.snapshot(plan.plan_digest), plan.plan_digest)
    replay_runtime = AutonomousEvidenceRuntime(plan, journal=restored_journal)
    assert replay_runtime.rehydrate()["restored"] == 1
    replayed = replay_runtime.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("replay must not reacquire")),
        rehydrate_value=lambda _receipt: value,
    )

    assert replayed.receipts[0].replay == "replayed"
    assert replayed.assessments[0].verdict == "rejected"
    assert replayed.status == fresh.status
    assert replayed.completed_requirement_ids == fresh.completed_requirement_ids
    assert replayed.pending_evaluation_requirement_ids == fresh.pending_evaluation_requirement_ids
    assert replayed.missing_requirement_ids == fresh.missing_requirement_ids
    assert evaluator.calls == 1

    class _AcceptingEvaluator:
        evaluator_id = "rejection-evaluator"
        evaluator_version = "2"

        def __init__(self) -> None:
            self.calls = 0

        def evaluate(self, _input_value):
            self.calls += 1
            return {
                "evaluator_id": self.evaluator_id,
                "evaluator_version": self.evaluator_version,
                "verdict": "accepted",
                "score": 1,
            }

    accepting_evaluator = _AcceptingEvaluator()
    revised = replay_runtime.execute(
        [request],
        acquirer=lambda _context: (_ for _ in ()).throw(RuntimeError("revision must not reacquire")),
        evaluator=accepting_evaluator,
        rehydrate_value=lambda _receipt: value,
        reevaluate_pending=True,
    )

    assert revised.status == "awaiting_evaluation", "pre-available requirements still lack evaluator decisions"
    assert revised.receipts[0].evaluator_status == "accepted"
    assert revised.assessments[0].verdict == "accepted"
    assert revised.completed_requirement_ids == (requirement.requirement_id,)
    assert not revised.pending_evaluation_requirement_ids
    assert accepting_evaluator.calls == 1
