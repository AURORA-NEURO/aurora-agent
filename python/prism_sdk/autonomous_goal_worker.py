"""Execute scheduled goals through caller-owned task rehydration.

The goal ledger and scheduler intentionally do not retain task text.  This module closes the
runtime loop without weakening that boundary: a resolver briefly rehydrates a task from the
application's protected work queue, an executor performs one approved attempt, and the worker
settles only status, criterion/evaluator digests, and bounded failure metadata.  The resolver and
executor are never serialized, and the live execution result is available only to the initiating
caller through the in-memory batch object.
"""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Callable, Mapping, Sequence
from typing import Any, Literal

from .authoring import content_digest
from .autonomous_goal_scheduler import (
    MAX_GOAL_SCHEDULE_SELECTED,
    AutonomousGoalSchedule,
    AutonomousGoalScheduleRow,
    AutonomousGoalScheduler,
    AutonomousGoalClaimResult,
)
from .goals import (
    GOAL_RETENTION,
    AutonomousGoalError,
    AutonomousGoalLedger,
    AutonomousGoalRecord,
    GoalStatus,
    goal_status_for_result,
    goal_task_digest,
)


GOAL_WORKER_SCHEMA = "bioprism-autonomous-goal-worker/0.1"
GOAL_WORKER_RETENTION = "metadata_only_goal_execution;task_and_execution_values_not_retained"
MAX_GOAL_WORKER_RUNS = MAX_GOAL_SCHEDULE_SELECTED
MAX_GOAL_WORKER_TASK_BYTES = 32_000
_SETTLEMENT_KEYS = frozenset({"evaluator_digest", "learning_state_digest", "progress_digest"})

WorkerRunStatus = Literal["completed", "paused", "blocked", "failed"]
GoalResolver = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow], Mapping[str, Any]]
GoalExecutor = Callable[["AutonomousGoalExecutionRequest"], Any]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal worker {message}")


def _digest(value: Any) -> str:
    return content_digest(value)


def _task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail("resolved task must be a non-empty NUL-free string")
    if len(value.encode("utf-8")) > MAX_GOAL_WORKER_TASK_BYTES:
        _fail("resolved task exceeds its bounded input size")
    return value


def _status(value: Any) -> str:
    if isinstance(value, Mapping):
        value = value.get("status")
    else:
        value = getattr(value, "status", None)
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > 128:
        _fail("executor result status is outside its bounded contract")
    return value.strip()


def _field(value: Any, name: str, default: Any) -> Any:
    if isinstance(value, Mapping):
        return value.get(name, default)
    return getattr(value, name, default)


def _settlement_metadata(value: Any) -> dict[str, str | None]:
    raw = _field(value, "settlement_metadata", {})
    if raw is None:
        return {}
    if not isinstance(raw, Mapping):
        _fail("executor settlement_metadata must be a mapping")
    unknown = sorted(set(raw).difference(_SETTLEMENT_KEYS))
    if unknown:
        _fail("executor settlement_metadata contains unsupported fields: " + ", ".join(unknown))
    normalized: dict[str, str | None] = {}
    for name, item in raw.items():
        if item is not None and (not isinstance(item, str) or len(item) != 64 or any(char not in "0123456789abcdef" for char in item)):
            _fail(f"executor settlement_metadata.{name} must be a lowercase SHA-256 digest or None")
        normalized[name] = item
    return normalized


def _criterion_updates(value: Any) -> tuple[Mapping[str, Any], ...]:
    raw = _field(value, "criterion_updates", ())
    if raw is None:
        return ()
    if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes, bytearray)) or len(raw) > 64:
        _fail("executor criterion_updates are outside their bounds")
    if any(not isinstance(item, Mapping) for item in raw):
        _fail("executor criterion_updates must contain mappings")
    return tuple(raw)


@dataclass(frozen=True, slots=True)
class AutonomousGoalExecutionRequest:
    """Transient rehydrated work passed to one caller-owned executor."""

    goal: AutonomousGoalRecord
    schedule_row: AutonomousGoalScheduleRow
    task: str
    parameters: Mapping[str, Any]
    schedule_digest: str

    def metadata(self) -> dict[str, Any]:
        return {
            "goal_id": self.goal.goal_id,
            "domain": self.goal.domain,
            "attempt": self.goal.attempt,
            "revision": self.goal.revision,
            "schedule_digest": self.schedule_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousGoalWorkerRun:
    goal_id: str
    domain: str
    attempt: int
    execution_status: WorkerRunStatus
    goal_status: GoalStatus
    outcome_digest: str
    schedule_digest: str
    claim_digest: str
    error_class: str | None = None
    error_digest: str | None = None
    live_result: Any = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "goal_id": self.goal_id,
            "domain": self.domain,
            "attempt": self.attempt,
            "execution_status": self.execution_status,
            "goal_status": self.goal_status,
            "outcome_digest": self.outcome_digest,
            "schedule_digest": self.schedule_digest,
            "claim_digest": self.claim_digest,
            "error_class": self.error_class,
            "error_digest": self.error_digest,
        }


@dataclass(frozen=True, slots=True)
class AutonomousGoalWorkerBatch:
    schedule: AutonomousGoalSchedule
    claim: AutonomousGoalClaimResult | None
    runs: tuple[AutonomousGoalWorkerRun, ...]
    worker_digest: str

    @property
    def live_results(self) -> tuple[Any, ...]:
        return tuple(run.live_result for run in self.runs)

    def to_dict(self) -> dict[str, Any]:
        claim = None if self.claim is None else self.claim.to_dict()
        rows = [run.to_dict() for run in self.runs]
        body = {
            "schema": GOAL_WORKER_SCHEMA,
            "schedule": self.schedule.to_dict(),
            "claim": claim,
            "runs": rows,
            "counts": {
                "selected": len(self.schedule.selected_goal_ids),
                "claimed": 0 if self.claim is None else len(self.claim.claims),
                "settled": len(rows),
                "completed": sum(run.goal_status == "completed" for run in self.runs),
                "paused": sum(run.goal_status == "paused" for run in self.runs),
                "blocked": sum(run.goal_status == "blocked" for run in self.runs),
                "failed": sum(run.goal_status == "failed" for run in self.runs),
            },
            "worker_digest": self.worker_digest,
            "retention": GOAL_WORKER_RETENTION,
            "goal_retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }
        return body


class AutonomousGoalWorker:
    """Schedule, claim, rehydrate, execute, and settle a bounded goal batch.

    ``resolver`` is the only place task text enters the worker.  It must return a transient
    mapping containing ``task`` and may return ``parameters`` for the executor.  Neither is
    copied into the ledger, schedule, claim receipt, worker digest, or public ``to_dict`` view.
    ``executor`` may return a status-bearing object/mapping plus optional ``criterion_updates``
    and digest-only ``settlement_metadata``.
    """

    def __init__(
        self,
        ledger: AutonomousGoalLedger,
        *,
        resolver: GoalResolver,
        executor: GoalExecutor,
        scheduler: AutonomousGoalScheduler | None = None,
    ) -> None:
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("ledger must be an AutonomousGoalLedger")
        if not callable(resolver):
            _fail("resolver must be callable")
        if not callable(executor):
            _fail("executor must be callable")
        if scheduler is not None and not isinstance(scheduler, AutonomousGoalScheduler):
            _fail("scheduler must be an AutonomousGoalScheduler")
        self.ledger = ledger
        self.resolver = resolver
        self.executor = executor
        self.scheduler = scheduler or AutonomousGoalScheduler()

    def run(self, *, schedule_options: Mapping[str, Any] | None = None) -> AutonomousGoalWorkerBatch:
        if schedule_options is not None and not isinstance(schedule_options, Mapping):
            _fail("schedule_options must be a mapping or None")
        options = {} if schedule_options is None else dict(schedule_options)
        # The ledger intentionally caps one bounded listing at 512 rows.  The scheduler's
        # admission cap is smaller, so this is enough to make one worker pass deterministic.
        goals = self.ledger.list(limit=512)
        schedule = self.scheduler.plan(goals, options)
        rows = {row.goal_id: row for row in schedule.rows if row.decision == "admit"}
        prepared: dict[str, AutonomousGoalExecutionRequest] = {}
        for goal_id in schedule.selected_goal_ids:
            goal = self.ledger.get(goal_id)
            row = rows.get(goal_id)
            if goal is None or row is None:
                _fail(f"schedule admission disappeared for goal {goal_id}")
            resolved = self.resolver(goal, row)
            if not isinstance(resolved, Mapping):
                _fail(f"resolver returned a non-mapping for goal {goal_id}")
            task = _task(resolved.get("task"))
            resolved_domain = resolved.get("domain", goal.domain)
            if resolved_domain != goal.domain:
                _fail(f"resolver domain does not match goal {goal_id}")
            parameters = resolved.get("parameters", {})
            if not isinstance(parameters, Mapping):
                _fail(f"resolver parameters must be a mapping for goal {goal_id}")
            prepared[goal_id] = AutonomousGoalExecutionRequest(
                goal=goal,
                schedule_row=row,
                task=task,
                parameters=dict(parameters),
                schedule_digest=schedule.schedule_digest,
            )
        claim = self.scheduler.claim(
            self.ledger,
            schedule,
            now_ns=options.get("now_ns") if isinstance(options.get("now_ns"), int) else None,
        ) if schedule.selected_goal_ids else None
        runs: list[AutonomousGoalWorkerRun] = []
        if claim is not None:
            claim_by_id = {item.goal_id: item for item in claim.claims}
            for goal_id in schedule.selected_goal_ids:
                claim_row = claim_by_id[goal_id]
                request = prepared[goal_id]
                current = self.ledger.get(goal_id)
                if current is None or current.status != "running" or current.revision != claim_row.running_revision:
                    _fail(f"claimed goal {goal_id} changed before execution")
                try:
                    live_result = self.executor(request)
                    result_status = _status(live_result)
                    updated = self._settle(current, result_status, live_result)
                    outcome_digest = _digest({"goal_id": goal_id, "attempt": current.attempt, "result_status": result_status})
                    runs.append(
                        AutonomousGoalWorkerRun(
                            goal_id=goal_id,
                            domain=current.domain,
                            attempt=current.attempt,
                            execution_status=updated.status,
                            goal_status=updated.status,
                            outcome_digest=outcome_digest,
                            schedule_digest=schedule.schedule_digest,
                            claim_digest=claim.claim_digest,
                            live_result=live_result,
                        )
                    )
                except Exception as error:
                    error_class = type(error).__name__
                    outcome_digest = _digest({"goal_id": goal_id, "attempt": current.attempt, "result_status": f"exception:{error_class}"})
                    try:
                        updated = self.ledger.transition(
                            goal_id,
                            "failed",
                            expected_revision=current.revision,
                            blockers=(f"exception:{error_class}",),
                            next_action_digest=goal_task_digest("goal-retry"),
                            outcome_digest=outcome_digest,
                        )
                    except AutonomousGoalError as transition_error:
                        raise AutonomousGoalError(f"goal {goal_id} failed without a durable failure transition") from transition_error
                    runs.append(
                        AutonomousGoalWorkerRun(
                            goal_id=goal_id,
                            domain=current.domain,
                            attempt=current.attempt,
                            execution_status="failed",
                            goal_status=updated.status,
                            outcome_digest=outcome_digest,
                            schedule_digest=schedule.schedule_digest,
                            claim_digest=claim.claim_digest,
                            error_class=error_class,
                            error_digest=_digest({"error_class": error_class}),
                        )
                    )
        body = {
            "schema": GOAL_WORKER_SCHEMA,
            "schedule_digest": schedule.schedule_digest,
            "claim_digest": None if claim is None else claim.claim_digest,
            "runs": [run.to_dict() for run in runs],
            "retention": GOAL_WORKER_RETENTION,
            "goal_retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }
        return AutonomousGoalWorkerBatch(
            schedule=schedule,
            claim=claim,
            runs=tuple(runs),
            worker_digest=_digest(body),
        )

    def _settle(self, current: AutonomousGoalRecord, result_status: str, result: Any) -> AutonomousGoalRecord:
        outcome_digest = _digest({"goal_id": current.goal_id, "attempt": current.attempt, "result_status": result_status})
        updates = _criterion_updates(result)
        settled = current
        if updates:
            settled = self.ledger.update_criteria(current.goal_id, updates, expected_revision=current.revision)
        metadata = _settlement_metadata(result)
        target = goal_status_for_result(result_status, criteria_complete=settled.required_criteria_complete)
        transition_metadata = {key: value for key, value in metadata.items() if value is not None}
        return self.ledger.transition(
            current.goal_id,
            target,
            expected_revision=settled.revision,
            blockers=(() if target == "completed" else (f"result:{result_status}",)),
            next_action_digest=(None if target == "completed" else goal_task_digest(f"goal-next:{result_status}")),
            outcome_digest=outcome_digest,
            **transition_metadata,
        )


__all__ = [
    "GOAL_WORKER_RETENTION",
    "GOAL_WORKER_SCHEMA",
    "MAX_GOAL_WORKER_RUNS",
    "MAX_GOAL_WORKER_TASK_BYTES",
    "AutonomousGoalExecutionRequest",
    "AutonomousGoalWorker",
    "AutonomousGoalWorkerBatch",
    "AutonomousGoalWorkerRun",
]
