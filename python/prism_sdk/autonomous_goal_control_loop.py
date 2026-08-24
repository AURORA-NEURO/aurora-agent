"""Bounded autonomous control loop over the goal scheduler and worker.

The scheduler makes one admission decision and the worker executes one bounded batch.  A useful
autonomous service needs a third layer that can continue those decisions: it should consume fresh
caller-owned signals, retry only statuses admitted by the scheduler, stop when no safe work remains,
and never spin forever on a paused or failing objective.  This module provides that layer without
retaining task text, prompts, parameters, credentials, provider output, or live evaluator values.
"""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Callable, Mapping, Sequence
import math
from typing import Any, Literal

from .authoring import content_digest
from .autonomous_goal_scheduler import AutonomousGoalSchedulingSignal
from .autonomous_goal_worker import AutonomousGoalWorker, AutonomousGoalWorkerBatch
from .autonomous_goal_control_persistence import (
    seal_autonomous_goal_control_loop_snapshot,
    validate_autonomous_goal_control_loop_snapshot,
)
from .goals import AutonomousGoalError, AutonomousGoalLedger, AutonomousGoalRecord


GOAL_CONTROL_LOOP_SCHEMA = "bioprism-autonomous-goal-control-loop/0.1"
GOAL_CONTROL_LOOP_RETENTION = "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained"
MAX_GOAL_CONTROL_LOOP_CYCLES = 128
MAX_GOAL_CONTROL_LOOP_RUNS = 8_192
MAX_GOAL_CONTROL_LOOP_BATCH_PREFIX_BYTES = 128
GOAL_CONTROL_EVALUATION_SCHEMA = "bioprism-autonomous-goal-control-evaluation/0.1"
GOAL_CONTROL_BANDIT_SCHEMA = "bioprism-autonomous-goal-control-bandit/0.1"
MAX_GOAL_CONTROL_EVALUATIONS = 128
MAX_GOAL_CONTROL_SIGNALS = 4_096

ControlLoopStopReason = Literal[
    "all_terminal",
    "no_admissible_work",
    "cycle_budget_exhausted",
    "run_budget_exhausted",
]
GoalLoopOptionsFactory = Callable[["AutonomousGoalControlLoopContext"], Mapping[str, Any]]
GoalLoopEvaluator = Callable[["AutonomousGoalControlLoopCycle"], Sequence[Any]]
GoalLoopLearner = Callable[[Sequence[Mapping[str, Any]], Sequence[Mapping[str, Any]]], Mapping[str, Any]]
GoalLoopCheckpoint = Callable[[Mapping[str, Any]], Any]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal control loop {message}")


def _integer(value: Any, *, name: str, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        _fail(f"{name} is outside its integer bounds")
    return value


def _prefix(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > MAX_GOAL_CONTROL_LOOP_BATCH_PREFIX_BYTES:
        _fail("batch_id_prefix is outside its bounded contract")
    return value.strip()


def _identifier(value: Any, *, name: str, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        _fail(f"{name} is outside its bounded identifier contract")
    return value.strip()


def _digest(value: Any, *, name: str, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _portable_number(value: float | int) -> float | int:
    """Match JSON number spelling across Python and TypeScript for digest parity."""
    return int(value) if isinstance(value, float) and value.is_integer() else value


@dataclass(frozen=True, slots=True)
class AutonomousGoalEvaluation:
    """Explicit evaluator credit for one executed goal; no raw evidence is retained."""

    goal_id: str
    domain: str
    attempt: int
    outcome_digest: str
    evaluator_id: str
    evaluator_version: str
    reward: float
    passed: bool
    evidence_digest: str | None
    failure_class: str | None
    feedback_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": GOAL_CONTROL_EVALUATION_SCHEMA,
            "goal_id": self.goal_id,
            "domain": self.domain,
            "attempt": self.attempt,
            "outcome_digest": self.outcome_digest,
            "evaluator_id": self.evaluator_id,
            "evaluator_version": self.evaluator_version,
            "reward": _portable_number(self.reward),
            "passed": self.passed,
            "evidence_digest": self.evidence_digest,
            "failure_class": self.failure_class,
            "feedback_digest": self.feedback_digest,
            "retention": "metadata_only_explicit_evaluator_credit",
            "secret_material": "never_returned",
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any], *, goal: AutonomousGoalRecord | Mapping[str, Any], outcome_digest: str, attempt: int) -> "AutonomousGoalEvaluation":
        if not isinstance(value, Mapping):
            _fail("evaluator output must be a mapping")
        allowed = {"goal_id", "evaluator_id", "evaluator_version", "reward", "passed", "evidence_digest", "failure_class", "feedback_digest"}
        if set(value).difference(allowed):
            _fail("evaluator output contains unsupported fields")
        goal_id = goal.goal_id if isinstance(goal, AutonomousGoalRecord) else _identifier(goal.get("goal_id"), name="goal.goal_id")
        if value.get("goal_id", goal_id) != goal_id:
            _fail(f"evaluator output goal_id does not match {goal_id}")
        reward = value.get("reward")
        if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1.0 <= float(reward) <= 1.0:
            _fail("evaluator reward must be finite and within [-1, 1]")
        if not isinstance(value.get("passed"), bool):
            _fail("evaluator passed must be boolean")
        evaluator_id = _identifier(value.get("evaluator_id"), name="evaluator_id", maximum=128)
        evaluator_version = _identifier(value.get("evaluator_version"), name="evaluator_version", maximum=128)
        evidence_digest = _digest(value.get("evidence_digest"), name="evidence_digest", allow_none=True)
        failure_class = value.get("failure_class")
        failure_class = None if failure_class is None else _identifier(failure_class, name="failure_class", maximum=128)
        body = {
            "schema": GOAL_CONTROL_EVALUATION_SCHEMA,
            "goal_id": goal_id,
            "domain": goal.domain if isinstance(goal, AutonomousGoalRecord) else _identifier(goal.get("domain"), name="goal.domain"),
            "attempt": attempt,
            "outcome_digest": _digest(outcome_digest, name="outcome_digest"),
            "evaluator_id": evaluator_id,
            "evaluator_version": evaluator_version,
            "reward": _portable_number(float(reward)),
            "passed": value["passed"],
            "evidence_digest": evidence_digest,
            "failure_class": failure_class,
        }
        feedback_digest = _digest(value.get("feedback_digest"), name="feedback_digest", allow_none=True) or content_digest(body)
        return cls(
            goal_id=goal_id,
            domain=body["domain"],
            attempt=attempt,
            outcome_digest=body["outcome_digest"],
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            reward=body["reward"],
            passed=body["passed"],
            evidence_digest=evidence_digest,
            failure_class=failure_class,
            feedback_digest=feedback_digest,
        )


class AutonomousGoalBanditLearner:
    """Value-only UCB-style domain learner used when a loop has an evaluator but no custom learner.

    It adapts future goal admission priorities, not permissions or provider authority. Rewards are
    accepted only from explicit evaluator packets; transport status is never converted to reward.
    """

    def __init__(self, *, state: Mapping[str, Any] | None = None, exploration: float = 0.35) -> None:
        if isinstance(exploration, bool) or not isinstance(exploration, (int, float)) or not math.isfinite(float(exploration)) or not 0.0 <= float(exploration) <= 2.0:
            _fail("bandit exploration is outside its bounds")
        self.exploration = float(exploration)
        self.generation = 0
        self.arms: dict[str, dict[str, float | int]] = {}
        if state is not None:
            self._restore(state)

    def _restore(self, state: Mapping[str, Any]) -> None:
        if not isinstance(state, Mapping) or state.get("schema") != GOAL_CONTROL_BANDIT_SCHEMA:
            _fail("bandit state schema is invalid")
        exploration = state.get("exploration", self.exploration)
        if isinstance(exploration, bool) or not isinstance(exploration, (int, float)) or not math.isfinite(float(exploration)) or not 0.0 <= float(exploration) <= 2.0:
            _fail("bandit state exploration is outside its bounds")
        self.exploration = float(exploration)
        self.generation = _integer(state.get("generation"), name="bandit generation", minimum=0, maximum=2**31 - 1)
        raw_arms = state.get("arms")
        if not isinstance(raw_arms, Sequence) or isinstance(raw_arms, (str, bytes, bytearray)) or len(raw_arms) > 128:
            _fail("bandit arms are outside their bounds")
        self.arms.clear()
        for raw in raw_arms:
            if not isinstance(raw, Mapping):
                _fail("bandit arm is malformed")
            domain = _identifier(raw.get("domain"), name="bandit arm domain", maximum=128)
            if domain in self.arms:
                _fail("bandit state contains duplicate domains")
            pulls = _integer(raw.get("pulls"), name="bandit arm pulls", minimum=0, maximum=2**31 - 1)
            failures = _integer(raw.get("failures"), name="bandit arm failures", minimum=0, maximum=2**31 - 1)
            if failures > pulls:
                _fail("bandit arm failures exceed pulls")
            reward_sum = raw.get("reward_sum")
            if isinstance(reward_sum, bool) or not isinstance(reward_sum, (int, float)) or not math.isfinite(float(reward_sum)) or not -pulls <= float(reward_sum) <= pulls:
                _fail("bandit arm reward_sum is outside its bounds")
            self.arms[domain] = {"pulls": pulls, "failures": failures, "reward_sum": float(reward_sum)}

    def restore(self, state: Mapping[str, Any]) -> None:
        """Replace value-only state after a process restart."""

        self._restore(state)

    def snapshot(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_CONTROL_BANDIT_SCHEMA,
            "generation": self.generation,
            "arms": [{"domain": domain, **self.arms[domain]} for domain in sorted(self.arms)],
            "exploration": _portable_number(self.exploration),
            "retention": "value_only_goal_domain_bandit_state",
            "secret_material": "never_returned",
        }
        body["arms"] = [
            {"domain": domain, "pulls": int(self.arms[domain]["pulls"]), "failures": int(self.arms[domain]["failures"]), "reward_sum": _portable_number(float(self.arms[domain]["reward_sum"]))}
            for domain in sorted(self.arms)
        ]
        return {**body, "state_digest": content_digest(body)}

    def update(self, evaluations: Sequence[Mapping[str, Any]], goals: Sequence[Mapping[str, Any]]) -> dict[str, Any]:
        if not isinstance(evaluations, Sequence) or isinstance(evaluations, (str, bytes, bytearray)) or len(evaluations) > MAX_GOAL_CONTROL_EVALUATIONS:
            _fail("bandit evaluations are outside their bounds")
        for raw in evaluations:
            if not isinstance(raw, Mapping) or not isinstance(raw.get("passed"), bool):
                _fail("bandit evaluation is malformed")
            domain = _identifier(raw.get("domain"), name="bandit evaluation domain", maximum=128)
            reward = raw.get("reward")
            if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1.0 <= float(reward) <= 1.0:
                _fail("bandit evaluation reward is outside its bounds")
            arm = self.arms.setdefault(domain, {"pulls": 0, "failures": 0, "reward_sum": 0.0})
            arm["pulls"] = int(arm["pulls"]) + 1
            arm["reward_sum"] = float(arm["reward_sum"]) + float(reward)
            if not bool(raw.get("passed")):
                arm["failures"] = int(arm["failures"]) + 1
        if self.generation >= 2**31 - 1:
            _fail("bandit generation is exhausted")
        self.generation += 1
        total_pulls = max(1, sum(int(arm["pulls"]) for arm in self.arms.values()))
        signals: list[dict[str, Any]] = []
        for goal in goals:
            status = goal.get("status")
            if status not in {"ready", "paused", "failed"}:
                continue
            goal_id = _identifier(goal.get("goal_id"), name="bandit goal_id")
            domain = _identifier(goal.get("domain"), name="bandit goal domain", maximum=128)
            arm = self.arms.get(domain, {"pulls": 0, "failures": 0, "reward_sum": 0.0})
            pulls = int(arm["pulls"])
            if pulls == 0:
                score = 1.0
            else:
                mean = (float(arm["reward_sum"]) / pulls + 1.0) / 2.0
                score = min(1.0, max(0.0, mean + self.exploration * math.sqrt(math.log(total_pulls + 1.0) / pulls)))
            urgency = min(1.0, int(arm["failures"]) / max(1, pulls))
            signals.append({"goal_id": goal_id, "priority": _portable_number(round(score, 4)), "urgency": _portable_number(round(urgency, 4)), "estimated_cost": 1, "dependencies": []})
            if len(signals) >= MAX_GOAL_CONTROL_SIGNALS:
                break
        signals.sort(key=lambda item: (-item["priority"], -item["urgency"], item["goal_id"]))
        snapshot = self.snapshot()
        return {"schema": GOAL_CONTROL_BANDIT_SCHEMA, "generation": self.generation, "learning_state_digest": snapshot["state_digest"], "signals": signals, "signals_digest": content_digest(signals), "retention": "value_only_goal_bandit_update", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousGoalControlLoopContext:
    """Metadata-only input supplied to a per-cycle scheduling policy."""

    cycle: int
    previous_cycle: Mapping[str, Any] | None
    ledger_stats: Mapping[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": GOAL_CONTROL_LOOP_SCHEMA,
            "cycle": self.cycle,
            "previous_cycle": None if self.previous_cycle is None else dict(self.previous_cycle),
            "ledger_stats": dict(self.ledger_stats),
            "retention": GOAL_CONTROL_LOOP_RETENTION,
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousGoalControlLoopCycle:
    cycle: int
    batch: AutonomousGoalWorkerBatch
    evaluations: tuple[AutonomousGoalEvaluation, ...] = ()
    learning_state_digest: str | None = None
    next_signals: tuple[Mapping[str, Any], ...] = ()

    def to_dict(self) -> dict[str, Any]:
        public = self.batch.to_dict()
        claim = public["claim"]
        body = {
            "cycle": self.cycle,
            "schedule_digest": self.batch.schedule.schedule_digest,
            "claim_digest": None if claim is None else claim["claim_digest"],
            "worker_digest": self.batch.worker_digest,
            "selected": len(self.batch.schedule.selected_goal_ids),
            "claimed": 0 if claim is None else len(claim["claims"]),
            "runs": len(self.batch.runs),
            "counts": dict(public["counts"]),
            "selected_domains": list(self.batch.schedule.selected_domains),
            "missing_domains": list(self.batch.schedule.missing_domains),
            "retention": GOAL_CONTROL_LOOP_RETENTION,
            "secret_material": "never_returned",
        }
        if self.evaluations:
            body["evaluated"] = len(self.evaluations)
            body["evaluation_digest"] = content_digest([evaluation.to_dict() for evaluation in self.evaluations])
        if self.learning_state_digest is not None:
            body["learning_state_digest"] = self.learning_state_digest
            body["signals_digest"] = content_digest(list(self.next_signals))
        return body

    @property
    def live_results(self) -> tuple[Any, ...]:
        return self.batch.live_results


@dataclass(frozen=True, slots=True)
class AutonomousGoalControlLoopResult:
    cycles: tuple[AutonomousGoalControlLoopCycle, ...]
    stop_reason: ControlLoopStopReason
    total_selected: int
    total_claimed: int
    total_runs: int
    status_counts: Mapping[str, int]
    domain_counts: Mapping[str, int]
    loop_digest: str
    evaluation_count: int = 0
    evaluation_digest: str | None = None
    learning_state_digest: str | None = None
    restored_cycle_count: int = 0
    cycle_history_digest: str | None = None

    @property
    def live_results(self) -> tuple[Any, ...]:
        return tuple(value for cycle in self.cycles for value in cycle.live_results)

    def to_dict(self) -> dict[str, Any]:
        body = {
            "schema": GOAL_CONTROL_LOOP_SCHEMA,
            "cycles": [cycle.to_dict() for cycle in self.cycles],
            "stop_reason": self.stop_reason,
            "total_selected": self.total_selected,
            "total_claimed": self.total_claimed,
            "total_runs": self.total_runs,
            "status_counts": dict(sorted(self.status_counts.items())),
            "domain_counts": dict(sorted(self.domain_counts.items())),
            "retention": GOAL_CONTROL_LOOP_RETENTION,
            "secret_material": "never_returned",
        }
        if self.evaluation_digest is not None:
            body["evaluation_count"] = self.evaluation_count
            body["evaluation_digest"] = self.evaluation_digest
        if self.learning_state_digest is not None:
            body["learning_state_digest"] = self.learning_state_digest
        if self.restored_cycle_count:
            body["restored_cycle_count"] = self.restored_cycle_count
            body["cycle_history_digest"] = self.cycle_history_digest
        return {**body, "loop_digest": self.loop_digest}


def _has_eligible_work(ledger: AutonomousGoalLedger, *, include_paused: bool, allow_failed_retry: bool) -> bool:
    raw_counts = ledger.stats().get("statuses", {})
    counts = raw_counts if isinstance(raw_counts, Mapping) else {}
    if int(counts.get("ready", 0)) > 0:
        return True
    if include_paused and int(counts.get("paused", 0)) > 0:
        return True
    # Aggregate status does not expose each failed record's remaining attempt budget.  A failed
    # row is therefore conservatively held as potentially retryable; if every failed row is
    # exhausted, the loop reports no_admissible_work instead of falsely claiming completion.
    return allow_failed_retry and int(counts.get("failed", 0)) > 0


def _all_terminal(ledger: AutonomousGoalLedger) -> bool:
    raw_counts = ledger.stats().get("statuses", {})
    counts = raw_counts if isinstance(raw_counts, Mapping) else {}
    return bool(counts) and set(counts).issubset({"completed", "cancelled"})


class AutonomousGoalControlLoop:
    """Continue bounded goal-worker cycles until safe work is exhausted or a budget is hit.

    ``options_factory`` is the policy seam for fresh caller-owned signals.  Its input and output
    are metadata-only schedule projections.  The loop never calls a provider itself; the worker's
    resolver/executor remain the only transient task and effect boundary.
    """

    def __init__(
        self,
        worker: AutonomousGoalWorker,
        *,
        batch_id_prefix: str = "autonomous-goal-loop",
        evaluator: GoalLoopEvaluator | None = None,
        learner: GoalLoopLearner | AutonomousGoalBanditLearner | None = None,
    ) -> None:
        if not isinstance(worker, AutonomousGoalWorker):
            _fail("worker must be an AutonomousGoalWorker")
        self.worker = worker
        self.batch_id_prefix = _prefix(batch_id_prefix)
        if evaluator is not None and not callable(evaluator):
            _fail("evaluator must be callable or None")
        if learner is not None and not isinstance(learner, AutonomousGoalBanditLearner) and not callable(learner):
            _fail("learner must be callable, an AutonomousGoalBanditLearner, or None")
        if learner is not None and evaluator is None:
            _fail("learner requires an explicit evaluator")
        self.evaluator = evaluator
        self.learner = AutonomousGoalBanditLearner() if evaluator is not None and learner is None else learner

    def run(
        self,
        *,
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: GoalLoopOptionsFactory | None = None,
        max_cycles: int = MAX_GOAL_CONTROL_LOOP_CYCLES,
        max_total_runs: int = MAX_GOAL_CONTROL_LOOP_RUNS,
        run_id: str | None = None,
        resume_snapshot: Mapping[str, Any] | None = None,
        checkpoint: GoalLoopCheckpoint | None = None,
    ) -> AutonomousGoalControlLoopResult:
        if schedule_options is not None and not isinstance(schedule_options, Mapping):
            _fail("schedule_options must be a mapping or None")
        if options_factory is not None and not callable(options_factory):
            _fail("options_factory must be callable or None")
        if run_id is not None and not isinstance(run_id, str):
            _fail("run_id must be a string or None")
        if resume_snapshot is not None and not isinstance(resume_snapshot, Mapping):
            _fail("resume_snapshot must be a mapping or None")
        if checkpoint is not None and not callable(checkpoint):
            _fail("checkpoint must be callable or None")
        max_cycles = _integer(max_cycles, name="max_cycles", minimum=1, maximum=MAX_GOAL_CONTROL_LOOP_CYCLES)
        max_total_runs = _integer(max_total_runs, name="max_total_runs", minimum=1, maximum=MAX_GOAL_CONTROL_LOOP_RUNS)
        base_options = {} if schedule_options is None else dict(schedule_options)
        cycles: list[AutonomousGoalControlLoopCycle] = []
        history: list[dict[str, Any]] = []
        previous: dict[str, Any] | None = None
        total_selected = 0
        total_claimed = 0
        total_runs = 0
        status_counts: dict[str, int] = {}
        domain_counts: dict[str, int] = {}
        stop_reason: ControlLoopStopReason = "cycle_budget_exhausted"
        learned_signals: list[Mapping[str, Any]] | None = None
        evaluation_digests: list[str] = []
        evaluation_count = 0
        learning_state_digest: str | None = None
        previous_checkpoint: dict[str, Any] | None = None
        restored_cycle_count = 0

        if resume_snapshot is not None:
            restored = validate_autonomous_goal_control_loop_snapshot(resume_snapshot)
            restored_cycle_count = int(restored["completed_cycles"])
            history = [dict(item) for item in restored["cycle_summaries"]]
            previous = None if restored["previous_cycle"] is None else dict(restored["previous_cycle"])
            total_selected = int(restored["total_selected"])
            total_claimed = int(restored["total_claimed"])
            total_runs = int(restored["total_runs"])
            status_counts = {str(key): int(value) for key, value in restored["status_counts"].items()}
            domain_counts = {str(key): int(value) for key, value in restored["domain_counts"].items()}
            evaluation_count = int(restored["evaluation_count"])
            evaluation_digests = [str(value) for value in restored["evaluation_digests"]]
            learning_state_digest = restored["learning_state_digest"]
            learned_signals = [dict(signal) for signal in restored["learned_signals"]]
            previous_checkpoint = dict(restored)
            if run_id is not None and run_id != restored["run_id"]:
                _fail("run_id does not match the resume snapshot")
            run_id = str(restored["run_id"])
            learner_state = restored["learner_state"]
            if learner_state is not None:
                if not isinstance(self.learner, AutonomousGoalBanditLearner):
                    _fail("resume snapshot contains built-in learner state but this loop has no compatible bandit")
                self.learner.restore(learner_state)

        checkpoint_run_id = _identifier(run_id or self.batch_id_prefix, name="run_id") if (checkpoint is not None or resume_snapshot is not None) else (run_id or self.batch_id_prefix)
        start_cycle = 1 if resume_snapshot is None else int(previous_checkpoint["next_cycle"])

        def emit_checkpoint(current_stop_reason: ControlLoopStopReason) -> None:
            nonlocal previous_checkpoint
            if checkpoint is None:
                return
            learner_state: Mapping[str, Any] | None = self.learner.snapshot() if isinstance(self.learner, AutonomousGoalBanditLearner) else None
            descriptor = {
                "schema": "bioprism-autonomous-goal-control-checkpoint/0.1",
                "run_id": checkpoint_run_id,
                "next_cycle": len(history) + 1,
                "cycle_summaries": history,
                "previous_cycle": previous,
                "completed_cycles": len(history),
                "total_selected": total_selected,
                "total_claimed": total_claimed,
                "total_runs": total_runs,
                "status_counts": dict(sorted(status_counts.items())),
                "domain_counts": dict(sorted(domain_counts.items())),
                "evaluation_count": evaluation_count,
                "evaluation_digests": evaluation_digests,
                "learning_state_digest": learning_state_digest,
                "learned_signals": [] if learned_signals is None else [dict(signal) for signal in learned_signals],
                "learner_state": learner_state,
                "stop_reason": current_stop_reason,
                "generation": 1 if previous_checkpoint is None else int(previous_checkpoint["generation"]) + 1,
                "previous_snapshot_digest": None if previous_checkpoint is None else previous_checkpoint["snapshot_digest"],
                "retention": "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
                "secret_material": "never_returned",
            }
            snapshot = seal_autonomous_goal_control_loop_snapshot(descriptor)
            checkpoint(snapshot)
            previous_checkpoint = snapshot

        for cycle_number in range(start_cycle, max_cycles + 1):
            remaining_runs = max_total_runs - total_runs
            if remaining_runs <= 0:
                stop_reason = "run_budget_exhausted"
                break
            context = AutonomousGoalControlLoopContext(
                cycle=cycle_number,
                previous_cycle=previous,
                ledger_stats=self.worker.ledger.stats(),
            )
            options = dict(base_options)
            if learned_signals is not None:
                options["signals"] = [dict(signal) for signal in learned_signals]
            if options_factory is not None:
                supplied = options_factory(context)
                if not isinstance(supplied, Mapping):
                    _fail("options_factory must return a mapping")
                options.update(dict(supplied))
            requested_selected = options.get("max_selected", 1)
            requested_selected = _integer(requested_selected, name="schedule_options.max_selected", minimum=1, maximum=128)
            effective_selected = min(requested_selected, remaining_runs)
            options["max_selected"] = effective_selected
            requested_concurrent = options.get("max_concurrent", effective_selected)
            requested_concurrent = _integer(requested_concurrent, name="schedule_options.max_concurrent", minimum=1, maximum=128)
            options["max_concurrent"] = min(requested_concurrent, effective_selected)
            batch_id = f"{self.batch_id_prefix}:cycle-{cycle_number}"
            if len(batch_id.encode("utf-8")) > 256:
                _fail("generated batch_id exceeds its worker bound")
            batch = self.worker.run(schedule_options=options, batch_id=batch_id if self.worker.journal is not None else None)
            cycle = AutonomousGoalControlLoopCycle(cycle=cycle_number, batch=batch)
            evaluations: tuple[AutonomousGoalEvaluation, ...] = ()
            next_signals: tuple[Mapping[str, Any], ...] = ()
            if self.evaluator is not None and batch.runs:
                raw_evaluations = self.evaluator(cycle)
                if not isinstance(raw_evaluations, Sequence) or isinstance(raw_evaluations, (str, bytes, bytearray)):
                    _fail("evaluator must return a sequence")
                if len(raw_evaluations) != len(batch.runs) or len(raw_evaluations) > MAX_GOAL_CONTROL_EVALUATIONS:
                    _fail("evaluator must return exactly one evaluation for every worker run")
                by_id = {run.goal_id: run for run in batch.runs}
                normalized: list[AutonomousGoalEvaluation] = []
                for raw in raw_evaluations:
                    packet: Mapping[str, Any] | None = None
                    if isinstance(raw, AutonomousGoalEvaluation):
                        packet = {
                            "goal_id": raw.goal_id,
                            "evaluator_id": raw.evaluator_id,
                            "evaluator_version": raw.evaluator_version,
                            "reward": raw.reward,
                            "passed": raw.passed,
                            "evidence_digest": raw.evidence_digest,
                            "failure_class": raw.failure_class,
                            "feedback_digest": raw.feedback_digest,
                        }
                    elif isinstance(raw, Mapping):
                        packet = raw
                    goal_id = packet.get("goal_id") if packet is not None else None
                    run = by_id.get(goal_id)
                    if run is None:
                        _fail("evaluator output references an unknown goal")
                    goal = self.worker.ledger.get(run.goal_id)
                    if goal is None:
                        _fail(f"evaluated goal {run.goal_id} disappeared")
                    candidate = AutonomousGoalEvaluation.from_mapping(packet, goal=goal, outcome_digest=run.outcome_digest, attempt=run.attempt)
                    run = by_id.get(candidate.goal_id)
                    if run is None or candidate.domain != run.domain or candidate.attempt != run.attempt or candidate.outcome_digest != run.outcome_digest:
                        _fail(f"evaluator output does not match worker run {candidate.goal_id}")
                    normalized.append(candidate)
                if len({item.goal_id for item in normalized}) != len(normalized):
                    _fail("evaluator returned duplicate goal evaluations")
                evaluations = tuple(normalized)
                evaluation_digests.append(content_digest([item.to_dict() for item in evaluations]))
                evaluation_count += len(evaluations)
                goals_for_learning = [record.to_dict() for record in self.worker.ledger.list(limit=512)]
                if self.learner is not None:
                    if isinstance(self.learner, AutonomousGoalBanditLearner):
                        update = self.learner.update([item.to_dict() for item in evaluations], goals_for_learning)
                    else:
                        update = self.learner([item.to_dict() for item in evaluations], goals_for_learning)
                    if not isinstance(update, Mapping):
                        _fail("learner must return a mapping")
                    learning_state_digest = _digest(update.get("learning_state_digest"), name="learning_state_digest") if update.get("learning_state_digest") is not None else content_digest(update)
                    raw_signals = update.get("signals", ())
                    if not isinstance(raw_signals, Sequence) or isinstance(raw_signals, (str, bytes, bytearray)) or len(raw_signals) > MAX_GOAL_CONTROL_SIGNALS:
                        _fail("learner signals are outside their bounds")
                    normalized_signals = tuple(AutonomousGoalSchedulingSignal.from_mapping(signal).to_dict() for signal in raw_signals)
                    next_signals = normalized_signals
                    learned_signals = list(normalized_signals)
                for evaluation in evaluations:
                    current = self.worker.ledger.get(evaluation.goal_id)
                    if current is None:
                        _fail(f"evaluated goal {evaluation.goal_id} disappeared before feedback settlement")
                    self.worker.ledger.transition(
                        evaluation.goal_id,
                        current.status,
                        expected_revision=current.revision,
                        blockers=current.blockers,
                        next_action_digest=current.next_action_digest,
                        evaluator_digest=evaluation.feedback_digest,
                        learning_state_digest=learning_state_digest,
                    )
            cycle = AutonomousGoalControlLoopCycle(cycle=cycle_number, batch=batch, evaluations=evaluations, learning_state_digest=learning_state_digest, next_signals=next_signals)
            cycles.append(cycle)
            public = cycle.to_dict()
            previous = public
            total_selected += public["selected"]
            total_claimed += public["claimed"]
            total_runs += public["runs"]
            history.append(public)
            for run in batch.runs:
                status_counts[run.goal_status] = status_counts.get(run.goal_status, 0) + 1
                domain_counts[run.domain] = domain_counts.get(run.domain, 0) + 1
            include_paused = options.get("include_paused", True)
            allow_failed_retry = options.get("allow_failed_retry", False)
            if not isinstance(include_paused, bool) or not isinstance(allow_failed_retry, bool):
                _fail("schedule retry and pause policies must be boolean")
            should_break = False
            if not batch.schedule.selected_goal_ids:
                stop_reason = "all_terminal" if _all_terminal(self.worker.ledger) else "no_admissible_work"
                should_break = True
            elif not batch.runs:
                stop_reason = "no_admissible_work"
                should_break = True
            elif not _has_eligible_work(self.worker.ledger, include_paused=include_paused, allow_failed_retry=allow_failed_retry):
                stop_reason = "all_terminal" if _all_terminal(self.worker.ledger) else "no_admissible_work"
                should_break = True
            emit_checkpoint(stop_reason)
            if should_break:
                break
        else:
            stop_reason = "cycle_budget_exhausted"

        summary = [cycle.to_dict() for cycle in cycles]
        evaluation_digest = content_digest(evaluation_digests) if evaluation_digests else None
        cycle_history_digest = content_digest(history) if restored_cycle_count else None
        digest_body = {"schema": GOAL_CONTROL_LOOP_SCHEMA, "cycles": history if restored_cycle_count else summary, "stop_reason": stop_reason, "total_selected": total_selected, "total_claimed": total_claimed, "total_runs": total_runs, "status_counts": dict(sorted(status_counts.items())), "domain_counts": dict(sorted(domain_counts.items())), "retention": GOAL_CONTROL_LOOP_RETENTION, "secret_material": "never_returned"}
        if evaluation_digest is not None:
            digest_body["evaluation_digest"] = evaluation_digest
        if learning_state_digest is not None:
            digest_body["learning_state_digest"] = learning_state_digest
        if restored_cycle_count:
            digest_body["restored_cycle_count"] = restored_cycle_count
            digest_body["cycle_history_digest"] = cycle_history_digest
        digest = content_digest(digest_body)
        return AutonomousGoalControlLoopResult(
            cycles=tuple(cycles),
            stop_reason=stop_reason,
            total_selected=total_selected,
            total_claimed=total_claimed,
            total_runs=total_runs,
            status_counts=dict(status_counts),
            domain_counts=dict(domain_counts),
            loop_digest=digest,
            evaluation_count=evaluation_count,
            evaluation_digest=evaluation_digest,
            learning_state_digest=learning_state_digest,
            restored_cycle_count=restored_cycle_count,
            cycle_history_digest=cycle_history_digest,
        )


__all__ = [
    "GOAL_CONTROL_LOOP_RETENTION",
    "GOAL_CONTROL_LOOP_SCHEMA",
    "MAX_GOAL_CONTROL_LOOP_BATCH_PREFIX_BYTES",
    "MAX_GOAL_CONTROL_LOOP_CYCLES",
    "MAX_GOAL_CONTROL_LOOP_RUNS",
    "GOAL_CONTROL_EVALUATION_SCHEMA",
    "GOAL_CONTROL_BANDIT_SCHEMA",
    "MAX_GOAL_CONTROL_EVALUATIONS",
    "MAX_GOAL_CONTROL_SIGNALS",
    "AutonomousGoalControlLoop",
    "AutonomousGoalBanditLearner",
    "AutonomousGoalEvaluation",
    "AutonomousGoalControlLoopContext",
    "AutonomousGoalControlLoopCycle",
    "AutonomousGoalControlLoopResult",
    "ControlLoopStopReason",
    "GoalLoopOptionsFactory",
    "GoalLoopEvaluator",
    "GoalLoopLearner",
    "GoalLoopCheckpoint",
]
