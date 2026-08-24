"""Bounded autonomous control loop over the goal scheduler and worker.

The scheduler makes one admission decision and the worker executes one bounded batch.  A useful
autonomous service needs a third layer that can continue those decisions: it should consume fresh
caller-owned signals, retry only statuses admitted by the scheduler, stop when no safe work remains,
and never spin forever on a paused or failing objective.  This module provides that layer without
retaining task text, prompts, parameters, credentials, provider output, or live evaluator values.
"""

from __future__ import annotations

from dataclasses import dataclass
from collections.abc import Callable, Mapping
from typing import Any, Literal

from .authoring import content_digest
from .autonomous_goal_worker import AutonomousGoalWorker, AutonomousGoalWorkerBatch
from .goals import AutonomousGoalError, AutonomousGoalLedger


GOAL_CONTROL_LOOP_SCHEMA = "bioprism-autonomous-goal-control-loop/0.1"
GOAL_CONTROL_LOOP_RETENTION = "metadata_only_goal_control;tasks_prompts_parameters_credentials_and_results_not_retained"
MAX_GOAL_CONTROL_LOOP_CYCLES = 128
MAX_GOAL_CONTROL_LOOP_RUNS = 8_192
MAX_GOAL_CONTROL_LOOP_BATCH_PREFIX_BYTES = 128

ControlLoopStopReason = Literal[
    "all_terminal",
    "no_admissible_work",
    "cycle_budget_exhausted",
    "run_budget_exhausted",
]
GoalLoopOptionsFactory = Callable[["AutonomousGoalControlLoopContext"], Mapping[str, Any]]


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

    def to_dict(self) -> dict[str, Any]:
        public = self.batch.to_dict()
        claim = public["claim"]
        return {
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

    def __init__(self, worker: AutonomousGoalWorker, *, batch_id_prefix: str = "autonomous-goal-loop") -> None:
        if not isinstance(worker, AutonomousGoalWorker):
            _fail("worker must be an AutonomousGoalWorker")
        self.worker = worker
        self.batch_id_prefix = _prefix(batch_id_prefix)

    def run(
        self,
        *,
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: GoalLoopOptionsFactory | None = None,
        max_cycles: int = MAX_GOAL_CONTROL_LOOP_CYCLES,
        max_total_runs: int = MAX_GOAL_CONTROL_LOOP_RUNS,
    ) -> AutonomousGoalControlLoopResult:
        if schedule_options is not None and not isinstance(schedule_options, Mapping):
            _fail("schedule_options must be a mapping or None")
        if options_factory is not None and not callable(options_factory):
            _fail("options_factory must be callable or None")
        max_cycles = _integer(max_cycles, name="max_cycles", minimum=1, maximum=MAX_GOAL_CONTROL_LOOP_CYCLES)
        max_total_runs = _integer(max_total_runs, name="max_total_runs", minimum=1, maximum=MAX_GOAL_CONTROL_LOOP_RUNS)
        base_options = {} if schedule_options is None else dict(schedule_options)
        cycles: list[AutonomousGoalControlLoopCycle] = []
        previous: dict[str, Any] | None = None
        total_selected = 0
        total_claimed = 0
        total_runs = 0
        status_counts: dict[str, int] = {}
        domain_counts: dict[str, int] = {}
        stop_reason: ControlLoopStopReason = "cycle_budget_exhausted"

        for cycle_number in range(1, max_cycles + 1):
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
            cycles.append(cycle)
            public = cycle.to_dict()
            previous = public
            total_selected += public["selected"]
            total_claimed += public["claimed"]
            total_runs += public["runs"]
            for run in batch.runs:
                status_counts[run.goal_status] = status_counts.get(run.goal_status, 0) + 1
                domain_counts[run.domain] = domain_counts.get(run.domain, 0) + 1
            include_paused = options.get("include_paused", True)
            allow_failed_retry = options.get("allow_failed_retry", False)
            if not isinstance(include_paused, bool) or not isinstance(allow_failed_retry, bool):
                _fail("schedule retry and pause policies must be boolean")
            if not batch.schedule.selected_goal_ids:
                stop_reason = "all_terminal" if _all_terminal(self.worker.ledger) else "no_admissible_work"
                break
            if not batch.runs:
                stop_reason = "no_admissible_work"
                break
            if not _has_eligible_work(self.worker.ledger, include_paused=include_paused, allow_failed_retry=allow_failed_retry):
                stop_reason = "all_terminal" if _all_terminal(self.worker.ledger) else "no_admissible_work"
                break
        else:
            stop_reason = "cycle_budget_exhausted"

        summary = [cycle.to_dict() for cycle in cycles]
        digest = content_digest({"schema": GOAL_CONTROL_LOOP_SCHEMA, "cycles": summary, "stop_reason": stop_reason, "total_selected": total_selected, "total_claimed": total_claimed, "total_runs": total_runs, "status_counts": dict(sorted(status_counts.items())), "domain_counts": dict(sorted(domain_counts.items())), "retention": GOAL_CONTROL_LOOP_RETENTION, "secret_material": "never_returned"})
        return AutonomousGoalControlLoopResult(
            cycles=tuple(cycles),
            stop_reason=stop_reason,
            total_selected=total_selected,
            total_claimed=total_claimed,
            total_runs=total_runs,
            status_counts=dict(status_counts),
            domain_counts=dict(domain_counts),
            loop_digest=digest,
        )


__all__ = [
    "GOAL_CONTROL_LOOP_RETENTION",
    "GOAL_CONTROL_LOOP_SCHEMA",
    "MAX_GOAL_CONTROL_LOOP_BATCH_PREFIX_BYTES",
    "MAX_GOAL_CONTROL_LOOP_CYCLES",
    "MAX_GOAL_CONTROL_LOOP_RUNS",
    "AutonomousGoalControlLoop",
    "AutonomousGoalControlLoopContext",
    "AutonomousGoalControlLoopCycle",
    "AutonomousGoalControlLoopResult",
    "ControlLoopStopReason",
    "GoalLoopOptionsFactory",
]
