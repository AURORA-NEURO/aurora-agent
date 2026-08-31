from __future__ import annotations

import hashlib
import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousBrain,
    AutonomousLearningController,
    AutonomousLearningFeedbackPersistenceCoordinator,
    AutonomousLearningFeedbackWorker,
    BrainEvaluatorDecision,
    BrainRunResult,
    DomainEvaluatorRegistry,
    InMemoryAutonomousLearningFeedbackOutbox,
    InMemoryAutonomousLearningFeedbackPersistence,
    JsonAutonomousLearningFeedbackPersistence,
    LLMRuntime,
    SQLiteAutonomousLearningFeedbackPersistence,
    calibrate_autonomous_evaluators,
)
from prism_sdk.brain import BrainOutcomeEvaluator
from prism_sdk.errors import ArgumentError


class _TextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value


class _Workspace:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        self.calls.append((name, {} if arguments is None else dict(arguments)))
        if name != "brain_outcome_record":
            raise AssertionError(f"unexpected workspace tool {name}")
        return {
            "ok": True,
            "status": "recorded",
            "next_state": {
                "schema": "bioprism-brain-bandit/0.1",
                "generation": 1,
                "arms": [
                    {
                        "arm_id": "openai/test-model",
                        "pulls": 1,
                        "reward_sum": 0.8,
                        "failures": 0,
                        "disabled": False,
                    }
                ],
            },
            "learning_evidence": {"evidence_digest": "e" * 64},
        }


def _cases(*, domains=AUTONOMOUS_DOMAINS, holdout_label: int = 1):
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    result = []
    for domain in domains:
        adapter = registry.resolve_for_autonomous_domain(domain)
        evidence = {
            "domain": domain,
            "capability": "controller-fixture",
            "risk_class": "read_only",
            "signals": {signal: 1.0 for signal in adapter.profile.required_signals},
        }
        for index in range(4):
            result.append(
                {
                    "case_id": f"{domain}-{index}",
                    "domain": domain,
                    "evidence": evidence,
                    "context": {"domain": domain, "fixture": "controller"},
                    "label": 1 if index < 2 else holdout_label,
                    "split": "calibration" if index < 2 else "holdout",
                }
            )
    return result


def _calibration_report(*, domains=AUTONOMOUS_DOMAINS, holdout_label: int = 1):
    return calibrate_autonomous_evaluators(
        _cases(domains=domains, holdout_label=holdout_label),
        registry=DomainEvaluatorRegistry.with_builtin_autonomous_profiles(),
        domains=domains,
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
        max_expected_calibration_error=0.1,
        max_brier_score=0.1,
    )


def _result(domain: str = "coding", run_id: str = "run-1") -> BrainRunResult:
    context = {
        "domain": domain,
        "capability": domain,
        "risk_class": "read_only",
        "task_family": "controller-fixture",
    }
    context_digest = hashlib.sha256(
        json.dumps(context, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    return BrainRunResult(
        run_id=run_id,
        status="completed",
        selection={
            "selected_model": {"provider": "openai", "model": "test-model"},
            "decision_digest": "d" * 64,
            "context": context,
            "context_digest": context_digest,
        },
        prompt={"prompt_digest": "a" * 64},
        plan={"plan": {"plan_digest": "b" * 64}},
        response=None,
        outcome_digest="c" * 64,
    )


def _decision() -> BrainEvaluatorDecision:
    return BrainEvaluatorDecision(
        evaluator_id="fixture-evaluator",
        evaluator_version="1",
        reward=0.8,
        passed=True,
    )


def _evaluator() -> BrainOutcomeEvaluator:
    return BrainOutcomeEvaluator(
        lambda _evaluation_input: {"reward": 0.8, "passed": True},
        evaluator_id="fixture-evaluator",
        evaluator_version="1",
    )


def _bandit_state() -> dict[str, object]:
    return {"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []}


def test_controller_admits_every_domain_only_after_calibration():
    report = _calibration_report()
    controller = AutonomousLearningController(
        AutonomousBrain(object(), LLMRuntime()),
        calibration_report=report,
        require_calibrated_learning=True,
    )

    assert controller.to_dict()["calibration_ready_domain_count"] == len(AUTONOMOUS_DOMAINS)
    assert controller.to_dict()["calibration_decision"] == "admit_learning"
    for domain in AUTONOMOUS_DOMAINS:
        assert controller.assert_learning_admission(domain)["decision"] == "admit_learning"

    held = _calibration_report(domains=("coding",), holdout_label=0)
    held_controller = AutonomousLearningController(
        AutonomousBrain(object(), LLMRuntime()),
        calibration_report=held,
        require_calibrated_learning=True,
    )
    with pytest.raises(RuntimeError, match="holding coding"):
        held_controller.assert_learning_admission("coding")


def test_controller_outbox_worker_settles_once_without_provider_replay():
    workspace = _Workspace()
    controller = AutonomousLearningController(AutonomousBrain(workspace, LLMRuntime()))
    outbox = InMemoryAutonomousLearningFeedbackOutbox()
    episode = controller.prepare_episode(_result())
    command = controller.enqueue_episode_settlement(
        outbox,
        episode,
        decision=_decision(),
        bandit_state=_bandit_state(),
        now=1_000,
    )

    worker = AutonomousLearningFeedbackWorker(outbox, controller, _evaluator())
    first = worker.run(worker_id="worker-a", now=1_000)
    second = worker.run(worker_id="worker-a", now=1_000)

    assert first["applied"] == 1
    assert first["failed"] == 0
    assert first["rows"][0]["command_id"] == command.command_id
    assert second["inspected"] == 0
    assert outbox.get(command.command_id).status == "applied"
    assert len(workspace.calls) == 1


def test_outbox_lease_expiry_requires_reconciliation_and_requeue():
    clock = [1_000]
    outbox = InMemoryAutonomousLearningFeedbackOutbox(clock=lambda: clock[0])
    controller = AutonomousLearningController(AutonomousBrain(object(), LLMRuntime()))
    episode = controller.prepare_episode(_result())
    command = controller.enqueue_episode_settlement(
        outbox,
        episode,
        decision=_decision(),
        bandit_state=_bandit_state(),
        now=1_000,
    )

    claimed = outbox.claim("worker-a", lease_ms=100, now=1_000)
    assert claimed is not None and claimed.status == "leased"
    assert outbox.claim("worker-b", now=1_000) is None
    assert outbox.reconcile_expired(now=1_101) == 1
    assert outbox.get(command.command_id).status == "reconciliation_required"
    requeued = outbox.requeue(command.command_id, now=1_101)
    assert requeued.status == "pending"
    assert outbox.claim("worker-b", now=1_101).status == "leased"


def test_outbox_rejects_evidence_and_secret_material_before_enqueue():
    controller = AutonomousLearningController(AutonomousBrain(object(), LLMRuntime()))
    outbox = InMemoryAutonomousLearningFeedbackOutbox()
    episode = controller.prepare_episode(_result())

    with pytest.raises(ArgumentError):
        controller.enqueue_episode_settlement(
            outbox,
            episode,
            decision=_decision(),
            bandit_state={**_bandit_state(), "api_key": "must-not-cross"},
        )
    with pytest.raises(ArgumentError):
        controller.enqueue_episode_settlement(
            outbox,
            episode,
            decision=_decision(),
            bandit_state=_bandit_state(),
            command_id="bad content",
        )


def test_outbox_persistence_round_trips_and_fences_stale_writer(tmp_path):
    controller = AutonomousLearningController(AutonomousBrain(object(), LLMRuntime()))
    outbox = InMemoryAutonomousLearningFeedbackOutbox()
    controller.enqueue_episode_settlement(
        outbox,
        controller.prepare_episode(_result()),
        decision=_decision(),
        bandit_state=_bandit_state(),
        now=1_000,
    )
    memory = InMemoryAutonomousLearningFeedbackPersistence()
    coordinator = AutonomousLearningFeedbackPersistenceCoordinator(outbox, memory)
    assert coordinator.restore()["status"] == "empty"
    snapshot = coordinator.flush()
    assert memory.read()["snapshot_digest"] == snapshot["snapshot_digest"]

    restored = InMemoryAutonomousLearningFeedbackOutbox()
    restored_coordinator = AutonomousLearningFeedbackPersistenceCoordinator(restored, memory)
    assert restored_coordinator.restore()["status"] == "restored"
    assert len(restored.commands()) == 1

    stale = AutonomousLearningFeedbackPersistenceCoordinator(
        InMemoryAutonomousLearningFeedbackOutbox(), memory
    )
    assert stale.restore()["status"] == "restored"
    restored.cancel(restored.commands()[0].command_id, now=2_000)
    restored_coordinator.flush()
    with pytest.raises(ArgumentError, match="compare-and-swap conflict"):
        stale.flush()

    text_store = _TextStore()
    json_persistence = JsonAutonomousLearningFeedbackPersistence(text_store)
    json_persistence.write(snapshot)
    assert json_persistence.read() == snapshot
    assert text_store.value == json.dumps(snapshot, sort_keys=True, separators=(",", ":"))

    sqlite_path = tmp_path / "learning-feedback.sqlite"
    with SQLiteAutonomousLearningFeedbackPersistence(sqlite_path) as sqlite_persistence:
        sqlite_persistence.write(snapshot)
        assert sqlite_persistence.read() == snapshot
        assert sqlite_persistence.write_if_unchanged("0" * 64, snapshot) is False
