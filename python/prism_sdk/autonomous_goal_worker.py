"""Execute scheduled goals through caller-owned task rehydration.

The goal ledger and scheduler intentionally do not retain task text.  This module closes the
runtime loop without weakening that boundary: a resolver briefly rehydrates a task from the
application's protected work queue, an executor performs one approved attempt, and the worker
settles only status, criterion/evaluator digests, and bounded failure metadata.  The resolver and
executor are never serialized, and the live execution result is available only to the initiating
caller through the in-memory batch object.
"""

from __future__ import annotations

import asyncio
import inspect
import json
from collections.abc import Awaitable, Callable, Mapping, Sequence
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from threading import Lock
from typing import Any, Literal

from .authoring import canonical_json, content_digest, utf8_scalar_byte_length
from .autonomous_goal_scheduler import (
    AutonomousGoalClaim,
    MAX_GOAL_SCHEDULE_SELECTED,
    AutonomousGoalSchedule,
    AutonomousGoalScheduleRow,
    AutonomousGoalScheduler,
    AutonomousGoalClaimResult,
)
from .autonomous_goal_worker_journal import AutonomousGoalWorkerEvent, AutonomousGoalWorkerJournal
from ._async_callback_bridge import (
    bridge_callback_to_loop,
    is_async_callable,
    run_in_thread_and_drain,
)
from .goals import (
    MAX_GOAL_BLOCKERS,
    MAX_GOALS,
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
GoalResolver = Callable[
    [AutonomousGoalRecord, AutonomousGoalScheduleRow],
    Mapping[str, Any] | Awaitable[Mapping[str, Any]],
]
GoalExecutor = Callable[["AutonomousGoalExecutionRequest"], Any | Awaitable[Any]]
GoalDispatchIntentPersister = Callable[[AutonomousGoalWorkerEvent], None | Awaitable[None]]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal worker {message}")


def _digest(value: Any) -> str:
    return content_digest(value)


def _task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail("resolved task must be a non-empty NUL-free string")
    byte_length = utf8_scalar_byte_length(value)
    if byte_length is None:
        _fail("resolved task must contain only Unicode scalar values")
    if byte_length > MAX_GOAL_WORKER_TASK_BYTES:
        _fail("resolved task exceeds its bounded input size")
    return value


def _status(value: Any) -> str:
    if isinstance(value, Mapping):
        value = value.get("status")
    else:
        value = getattr(value, "status", None)
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail("executor result status is outside its bounded contract")
    byte_length = utf8_scalar_byte_length(value)
    if byte_length is None or byte_length > 128:
        _fail("executor result status is outside its bounded contract")
    return value.strip()


def _field(value: Any, name: str, default: Any) -> Any:
    if isinstance(value, Mapping):
        return value.get(name, default)
    return getattr(value, name, default)


def _deny_frozen_parameter_mutation(*_args: Any, **_kwargs: Any) -> None:
    raise TypeError("autonomous goal execution parameters are immutable")


class _FrozenGoalParameterObject(dict[str, Any]):
    __setitem__ = _deny_frozen_parameter_mutation
    __delitem__ = _deny_frozen_parameter_mutation
    clear = _deny_frozen_parameter_mutation
    pop = _deny_frozen_parameter_mutation
    popitem = _deny_frozen_parameter_mutation
    setdefault = _deny_frozen_parameter_mutation
    update = _deny_frozen_parameter_mutation
    __ior__ = _deny_frozen_parameter_mutation
    __init__ = _deny_frozen_parameter_mutation


class _FrozenGoalParameterArray(list[Any]):
    __setitem__ = _deny_frozen_parameter_mutation
    __delitem__ = _deny_frozen_parameter_mutation
    append = _deny_frozen_parameter_mutation
    clear = _deny_frozen_parameter_mutation
    extend = _deny_frozen_parameter_mutation
    insert = _deny_frozen_parameter_mutation
    pop = _deny_frozen_parameter_mutation
    remove = _deny_frozen_parameter_mutation
    reverse = _deny_frozen_parameter_mutation
    sort = _deny_frozen_parameter_mutation
    __iadd__ = _deny_frozen_parameter_mutation
    __imul__ = _deny_frozen_parameter_mutation
    __init__ = _deny_frozen_parameter_mutation


def _freeze_goal_parameters(value: Any) -> Any:
    if isinstance(value, dict):
        frozen = dict.__new__(_FrozenGoalParameterObject)
        dict.__init__(frozen, ((key, _freeze_goal_parameters(item)) for key, item in value.items()))
        return frozen
    if isinstance(value, list):
        frozen = list.__new__(_FrozenGoalParameterArray)
        list.__init__(frozen, (_freeze_goal_parameters(item) for item in value))
        return frozen
    return value


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
    task_digest: str
    execution_binding_digest: str

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
    """Transient run result with an in-process marker for executor-boundary entry."""

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
    dispatched: bool = False

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
        journal: AutonomousGoalWorkerJournal | None = None,
        persist_dispatch_intent: GoalDispatchIntentPersister | None = None,
    ) -> None:
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("ledger must be an AutonomousGoalLedger")
        if not callable(resolver):
            _fail("resolver must be callable")
        if not callable(executor):
            _fail("executor must be callable")
        if scheduler is not None and not isinstance(scheduler, AutonomousGoalScheduler):
            _fail("scheduler must be an AutonomousGoalScheduler")
        if journal is not None and not isinstance(journal, AutonomousGoalWorkerJournal):
            _fail("journal must be an AutonomousGoalWorkerJournal")
        if persist_dispatch_intent is not None and not callable(persist_dispatch_intent):
            _fail("persist_dispatch_intent must be callable")
        if persist_dispatch_intent is not None and journal is None:
            _fail("persist_dispatch_intent requires a worker journal")
        self.ledger = ledger
        self.resolver = resolver
        self.executor = executor
        self.scheduler = scheduler or AutonomousGoalScheduler()
        self.journal = journal
        self.persist_dispatch_intent = persist_dispatch_intent
        self._dispatch_persistence_lock = Lock()

    def run(self, *, schedule_options: Mapping[str, Any] | None = None, batch_id: str | None = None) -> AutonomousGoalWorkerBatch:
        release = None if self.journal is None else self.journal.begin_worker_run()
        try:
            return self._run_admitted(schedule_options=schedule_options, batch_id=batch_id)
        finally:
            if release is not None:
                release()

    async def run_async(
        self,
        *,
        schedule_options: Mapping[str, Any] | None = None,
        batch_id: str | None = None,
    ) -> AutonomousGoalWorkerBatch:
        """Run the same bounded worker contract with sync or async callbacks.

        SQLite and the deterministic scheduling/settlement path run in a worker thread. Awaitable
        resolver and executor callbacks are resumed on the caller's event loop, which keeps async
        transports usable without duplicating goal admission or settlement logic. Cancellation
        waits for the worker thread to drain before it is re-raised: an in-flight external action
        cannot be safely force-cancelled or assumed not to have completed.
        """

        loop = asyncio.get_running_loop()
        worker = self._with_async_callbacks(loop)
        return await run_in_thread_and_drain(
            worker.run,
            schedule_options=schedule_options,
            batch_id=batch_id,
        )

    def _with_async_callbacks(self, loop: asyncio.AbstractEventLoop) -> "AutonomousGoalWorker":
        """Clone the worker with callbacks that settle awaitables on ``loop``."""

        return AutonomousGoalWorker(
            self.ledger,
            resolver=bridge_callback_to_loop(self.resolver, loop),
            executor=bridge_callback_to_loop(self.executor, loop),
            scheduler=self.scheduler,
            journal=self.journal,
            persist_dispatch_intent=(
                None
                if self.persist_dispatch_intent is None
                else bridge_callback_to_loop(self.persist_dispatch_intent, loop)
            ),
        )

    def _run_admitted(self, *, schedule_options: Mapping[str, Any] | None = None, batch_id: str | None = None) -> AutonomousGoalWorkerBatch:
        if is_async_callable(self.resolver) or is_async_callable(self.executor) or is_async_callable(self.persist_dispatch_intent):
            _fail("async callbacks require run_async()")
        if schedule_options is not None and not isinstance(schedule_options, Mapping):
            _fail("schedule_options must be a mapping or None")
        options = {} if schedule_options is None else dict(schedule_options)
        if self.journal is not None:
            if not isinstance(batch_id, str) or not batch_id.strip() or "\x00" in batch_id:
                _fail("batch_id is required and bounded when a journal is configured")
            batch_id_bytes = utf8_scalar_byte_length(batch_id)
            if batch_id_bytes is None or batch_id_bytes > 256:
                _fail("batch_id is required and bounded when a journal is configured")
        # Scheduling over the full bounded ledger is necessary to close dependencies whose
        # completed prerequisites are older than the newest goal records.
        goals = self.ledger.list(limit=MAX_GOALS)
        schedule = self.scheduler.plan(goals, options)
        if self.journal is not None:
            self.journal.require_capacity(6 * len(schedule.selected_goal_ids))
        rows = {row.goal_id: row for row in schedule.rows if row.decision == "admit"}
        prepared: dict[str, AutonomousGoalExecutionRequest] = {}
        for goal_id in schedule.selected_goal_ids:
            goal = self.ledger.get(goal_id)
            row = rows.get(goal_id)
            if goal is None or row is None:
                _fail(f"schedule admission disappeared for goal {goal_id}")
            if self.journal is not None:
                self.journal.assert_no_active(goal_id)
            resolved = self.resolver(goal, row)
            if inspect.isawaitable(resolved):
                if inspect.iscoroutine(resolved):
                    resolved.close()
                _fail("resolver returned an awaitable; use run_async()")
            if not isinstance(resolved, Mapping):
                _fail(f"resolver returned a non-mapping for goal {goal_id}")
            task = _task(resolved.get("task"))
            resolved_domain = resolved.get("domain", goal.domain)
            if resolved_domain != goal.domain:
                _fail(f"resolver domain does not match goal {goal_id}")
            parameters = resolved.get("parameters", {})
            if not isinstance(parameters, Mapping):
                _fail(f"resolver parameters must be a mapping for goal {goal_id}")
            if goal_task_digest(task) != goal.task_digest:
                _fail(f"resolver task digest does not match goal {goal_id}")
            # The JSON round trip detaches resolver-owned values; the recursive frozen containers
            # keep an executor from changing them after the execution-binding digest is sealed.
            copied_parameters = json.loads(canonical_json(dict(parameters)))
            frozen_parameters = _freeze_goal_parameters(copied_parameters)
            prepared[goal_id] = AutonomousGoalExecutionRequest(
                goal=goal,
                schedule_row=row,
                task=task,
                parameters=frozen_parameters,
                schedule_digest=schedule.schedule_digest,
                task_digest=goal.task_digest,
                execution_binding_digest=_digest({"parameters": frozen_parameters}),
            )
        selected_ids = list(schedule.selected_goal_ids)
        selected_id_set = set(selected_ids)
        remaining = list(selected_ids)
        completed_waves: set[str] = set()
        execution_waves: list[list[str]] = []
        while remaining:
            wave = [
                goal_id
                for goal_id in remaining
                if all(
                    dependency_id not in selected_id_set
                    or dependency_id in completed_waves
                    for dependency_id in prepared[goal_id].schedule_row.dependencies
                )
            ]
            if not wave:
                _fail(
                    "schedule cannot be divided into dependency-closed execution waves"
                )
            execution_waves.append(wave)
            completed_waves.update(wave)
            remaining = [
                goal_id for goal_id in remaining if goal_id not in completed_waves
            ]
        if self.journal is not None and batch_id is not None:
            for request in prepared.values():
                self.journal.record(
                    batch_id=batch_id,
                    goal_id=request.goal.goal_id,
                    phase="prepared",
                    attempt=request.goal.attempt,
                    revision=request.goal.revision,
                    schedule_digest=schedule.schedule_digest,
                    task_digest=request.task_digest,
                    execution_binding_digest=request.execution_binding_digest,
                )
        claim = (
            self.scheduler.claim(
                self.ledger,
                schedule,
                now_ns=schedule.now_ns,
            )
            if schedule.selected_goal_ids
            else None
        )
        runs_by_id: dict[str, AutonomousGoalWorkerRun] = {}
        if claim is not None:
            claim_by_id = {item.goal_id: item for item in claim.claims}
            if self.journal is not None and batch_id is not None:
                for item in claim.claims:
                    current = self.ledger.get(item.goal_id)
                    if current is None:
                        _fail(
                            f"claimed goal {item.goal_id} disappeared before journaling"
                        )
                    request = prepared.get(item.goal_id)
                    if request is None:
                        _fail(
                            f"prepared request for goal {item.goal_id} disappeared before journaling"
                        )
                    self.journal.record(
                        batch_id=batch_id,
                        goal_id=item.goal_id,
                        phase="claimed",
                        attempt=current.attempt,
                        revision=current.revision,
                        schedule_digest=schedule.schedule_digest,
                        claim_digest=claim.claim_digest,
                        task_digest=request.task_digest,
                        execution_binding_digest=request.execution_binding_digest,
                    )
            for wave in execution_waves:
                if len(wave) == 1 or schedule.max_concurrent == 1:
                    wave_runs = [
                        self._execute_claimed(
                            goal_id,
                            claim_by_id[goal_id],
                            prepared[goal_id],
                            claim,
                            schedule,
                            batch_id,
                        )
                        for goal_id in wave
                    ]
                else:
                    with ThreadPoolExecutor(
                        max_workers=min(schedule.max_concurrent, len(wave))
                    ) as pool:
                        futures = {
                            goal_id: pool.submit(
                                self._execute_claimed,
                                goal_id,
                                claim_by_id[goal_id],
                                prepared[goal_id],
                                claim,
                                schedule,
                                batch_id,
                            )
                            for goal_id in wave
                        }
                        # Completion timing is variable; the public digest stays in schedule order.
                        wave_runs = [
                            futures[goal_id].result() for goal_id in wave
                        ]
                runs_by_id.update((run.goal_id, run) for run in wave_runs)
        runs = [
            runs_by_id[goal_id]
            for goal_id in schedule.selected_goal_ids
            if goal_id in runs_by_id
        ]
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

    def _execute_claimed(
        self,
        goal_id: str,
        claim_row: AutonomousGoalClaim,
        request: AutonomousGoalExecutionRequest,
        claim: AutonomousGoalClaimResult,
        schedule: AutonomousGoalSchedule,
        batch_id: str | None,
    ) -> AutonomousGoalWorkerRun:
        current = self.ledger.get(goal_id)
        if (
            current is None
            or current.status != "running"
            or current.revision != claim_row.running_revision
        ):
            _fail(f"claimed goal {goal_id} changed before execution")
        unmet_dependencies = []
        for dependency_id in request.schedule_row.dependencies:
            dependency = self.ledger.get(dependency_id)
            if dependency is None or dependency.status != "completed":
                unmet_dependencies.append(
                    {
                        "goal_id": dependency_id,
                        "status": "missing" if dependency is None else dependency.status,
                    }
                )
        if unmet_dependencies:
            outcome_digest = _digest(
                {
                    "goal_id": goal_id,
                    "attempt": current.attempt,
                    "result_status": "dependency_not_completed",
                    "dependencies": unmet_dependencies,
                }
            )
            blockers = [
                f"dependency_not_completed:{_digest({'goal_id': item['goal_id']})}:{item['status']}"
                for item in unmet_dependencies
            ]
            if len(blockers) > MAX_GOAL_BLOCKERS:
                omitted = len(blockers) - MAX_GOAL_BLOCKERS + 1
                blockers = blockers[: MAX_GOAL_BLOCKERS - 1] + [
                    f"dependency_not_completed:additional:{omitted}"
                ]
            updated = self.ledger.transition(
                goal_id,
                "paused",
                expected_revision=current.revision,
                blockers=tuple(blockers),
                next_action_digest=goal_task_digest("goal-dependency-retry"),
                outcome_digest=outcome_digest,
            )
            if self.journal is not None and batch_id is not None:
                self.journal.record(
                    batch_id=batch_id,
                    goal_id=goal_id,
                    phase="settled",
                    attempt=current.attempt,
                    revision=updated.revision,
                    schedule_digest=schedule.schedule_digest,
                    claim_digest=claim.claim_digest,
                    outcome_digest=outcome_digest,
                    task_digest=request.task_digest,
                    execution_binding_digest=request.execution_binding_digest,
                )
            return AutonomousGoalWorkerRun(
                goal_id=goal_id,
                domain=current.domain,
                attempt=current.attempt,
                execution_status="paused",
                goal_status=updated.status,
                outcome_digest=outcome_digest,
                schedule_digest=schedule.schedule_digest,
                claim_digest=claim.claim_digest,
                dispatched=False,
            )
        if self.journal is not None and batch_id is not None:
            self.journal.require_capacity(2)
            if self.persist_dispatch_intent is not None:
                with self._dispatch_persistence_lock:
                    dispatch_event = self.journal.record(
                        batch_id=batch_id,
                        goal_id=goal_id,
                        phase="dispatch_started",
                        attempt=current.attempt,
                        revision=current.revision,
                        schedule_digest=schedule.schedule_digest,
                        claim_digest=claim.claim_digest,
                        task_digest=request.task_digest,
                        execution_binding_digest=request.execution_binding_digest,
                    )
                    self.persist_dispatch_intent(dispatch_event)
            else:
                self.journal.record(
                    batch_id=batch_id,
                    goal_id=goal_id,
                    phase="dispatch_started",
                    attempt=current.attempt,
                    revision=current.revision,
                    schedule_digest=schedule.schedule_digest,
                    claim_digest=claim.claim_digest,
                    task_digest=request.task_digest,
                    execution_binding_digest=request.execution_binding_digest,
                )
        try:
            live_result = self.executor(request)
            if inspect.isawaitable(live_result):
                if inspect.iscoroutine(live_result):
                    live_result.close()
                _fail("executor returned an awaitable; use run_async()")
            result_status = _status(live_result)
            updated = self._settle(current, result_status, live_result)
            outcome_digest = _digest(
                {
                    "goal_id": goal_id,
                    "attempt": current.attempt,
                    "result_status": result_status,
                }
            )
        except Exception as error:
            error_class = type(error).__name__
            if self.journal is not None:
                self.journal.recover(self.ledger, goal_id=goal_id)
                updated = self.ledger.get(goal_id)
                if (
                    updated is None
                    or updated.status != "blocked"
                    or "worker_restart_after_dispatch_requires_reconciliation"
                    not in updated.blockers
                    or updated.outcome_digest is None
                ):
                    raise AutonomousGoalError(
                        f"goal {goal_id} dispatch outcome is unknown and could not be fenced for reconciliation"
                    ) from error
                outcome_digest = updated.outcome_digest
            else:
                latest = self.ledger.get(goal_id)
                if (
                    latest is None
                    or latest.status != "running"
                    or latest.attempt != current.attempt
                ):
                    raise AutonomousGoalError(
                        f"goal {goal_id} dispatch outcome is unknown and its running state changed"
                    ) from error
                outcome_digest = _digest(
                    {
                        "goal_id": goal_id,
                        "attempt": current.attempt,
                        "result_status": "worker_dispatch_outcome_unknown",
                        "error_class": error_class,
                    }
                )
                updated = self.ledger.transition(
                    goal_id,
                    "blocked",
                    expected_revision=latest.revision,
                    blockers=("worker_dispatch_outcome_requires_reconciliation",),
                    next_action_digest=goal_task_digest("goal-reconciliation-review"),
                    outcome_digest=outcome_digest,
                )
            return AutonomousGoalWorkerRun(
                goal_id=goal_id,
                domain=current.domain,
                attempt=current.attempt,
                execution_status="blocked",
                goal_status=updated.status,
                outcome_digest=outcome_digest,
                schedule_digest=schedule.schedule_digest,
                claim_digest=claim.claim_digest,
                error_class=error_class,
                error_digest=_digest({"error_class": error_class}),
                dispatched=True,
            )
        if self.journal is not None and batch_id is not None:
            self.journal.record(
                batch_id=batch_id,
                goal_id=goal_id,
                phase="settled",
                attempt=current.attempt,
                revision=updated.revision,
                schedule_digest=schedule.schedule_digest,
                claim_digest=claim.claim_digest,
                outcome_digest=outcome_digest,
                task_digest=request.task_digest,
                execution_binding_digest=request.execution_binding_digest,
            )
        return AutonomousGoalWorkerRun(
            goal_id=goal_id,
            domain=current.domain,
            attempt=current.attempt,
            execution_status=updated.status,
            goal_status=updated.status,
            outcome_digest=outcome_digest,
            schedule_digest=schedule.schedule_digest,
            claim_digest=claim.claim_digest,
            live_result=live_result,
            dispatched=True,
        )

    def _settle(self, current: AutonomousGoalRecord, result_status: str, result: Any) -> AutonomousGoalRecord:
        outcome_digest = _digest({"goal_id": current.goal_id, "attempt": current.attempt, "result_status": result_status})
        updates = _criterion_updates(result)
        metadata = _settlement_metadata(result)
        statuses = {criterion.criterion_id: criterion.status for criterion in current.criteria}
        for update in updates:
            criterion_id = update.get("criterion_id")
            if criterion_id in statuses and "status" in update:
                statuses[criterion_id] = update["status"]
        criteria_complete = all(
            not criterion.required or statuses[criterion.criterion_id] in {"satisfied", "waived"}
            for criterion in current.criteria
        )
        target = goal_status_for_result(result_status, criteria_complete=criteria_complete)
        transition_metadata = {key: value for key, value in metadata.items() if value is not None}
        return self.ledger.transition(
            current.goal_id,
            target,
            expected_revision=current.revision,
            criterion_updates=updates,
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
