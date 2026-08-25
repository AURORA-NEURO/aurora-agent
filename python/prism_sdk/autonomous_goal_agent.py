"""Bridge durable goal control loops to the real autonomous model/provider facade.

The goal ledger deliberately stores only task and outcome digests.  This adapter supplies the
missing application seam: a caller-owned task rehydrator returns transient task text, while a
second caller-owned factory supplies model candidates, credential handles, approval callbacks,
memory, policy, and other run options only after a goal has been admitted.  The actual
``AutonomousTaskOrchestrator`` remains responsible for routing, prompts, selection, invocation,
effects, and online learning; this module only composes it with the bounded goal worker/loop.
"""

from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from typing import Any

from .autonomy import AUTONOMOUS_DOMAINS, AutonomousTaskOrchestrator
from .autonomous_goal_control_loop import (
    AutonomousGoalBanditLearner,
    AutonomousGoalControlLoop,
    AutonomousGoalControlLoopResult,
    GoalLoopEvaluator,
    GoalLoopLearner,
    GoalLoopOptionsFactory,
)
from .autonomous_goal_scheduler import AutonomousGoalScheduleRow
from .autonomous_goal_worker import AutonomousGoalExecutionRequest, AutonomousGoalWorker
from .autonomous_goal_worker_journal import AutonomousGoalWorkerJournal
from .goals import AutonomousGoalError, AutonomousGoalLedger, AutonomousGoalRecord


GOAL_AGENT_RUNTIME_SCHEMA = "bioprism-autonomous-goal-agent-runtime/0.1"
GOAL_AGENT_RUNTIME_RETENTION = "metadata_only_goal_agent_bridge;tasks_prompts_parameters_credentials_and_results_not_retained"
_FORBIDDEN_RUN_OPTION_KEYS = frozenset({"task", "domain"})

GoalAgentTaskResolver = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow], str]
GoalAgentRunOptionsFactory = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow], Mapping[str, Any]]


def _fail(message: str) -> None:
    raise AutonomousGoalError(f"autonomous goal agent runtime {message}")


def _options(value: Any) -> dict[str, Any]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        _fail("run options factory must return a mapping")
    unknown = sorted(set(value).intersection(_FORBIDDEN_RUN_OPTION_KEYS))
    if unknown:
        _fail("run options cannot override goal " + ", ".join(unknown))
    if any(not isinstance(key, str) or not key.strip() or "\x00" in key for key in value):
        _fail("run options contain an invalid key")
    if len(value) > 128:
        _fail("run options contain too many fields")
    # Do not JSON-clone this mapping: credential handles, abort callbacks, effect boundaries, and
    # provider observers are intentionally transient Python objects.  The worker never serializes
    # this mapping and the executor receives it only in the initiating process.
    return dict(value)


def _subtasks(value: Any) -> tuple[Mapping[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or not value or len(value) > 64:
        _fail("cross-domain run options require 1..64 subtasks")
    if any(not isinstance(item, Mapping) for item in value):
        _fail("cross-domain subtasks must contain mappings")
    return tuple(value)


class AutonomousGoalAgentRuntime:
    """Run durable goals through ``AutonomousTaskOrchestrator`` with bounded adaptive control.

    ``task_resolver`` is called during worker preparation and may read the caller's protected
    queue. ``run_options_factory`` is called only after the goal crosses the scheduler claim
    boundary, which lets callers open short-lived credential scopes or construct approval and
    observer callbacks without placing them in goal state. Neither callback is serializable or
    included in a result projection.
    """

    def __init__(
        self,
        orchestrator: AutonomousTaskOrchestrator,
        ledger: AutonomousGoalLedger,
        *,
        agent: Any | None = None,
        task_resolver: GoalAgentTaskResolver,
        run_options_factory: GoalAgentRunOptionsFactory | None = None,
        evaluator: GoalLoopEvaluator | None = None,
        learner: GoalLoopLearner | AutonomousGoalBanditLearner | None = None,
        journal: AutonomousGoalWorkerJournal | None = None,
        batch_id_prefix: str = "autonomous-goal-agent",
    ) -> None:
        if not isinstance(orchestrator, AutonomousTaskOrchestrator):
            _fail("orchestrator must be an AutonomousTaskOrchestrator")
        if agent is not None:
            if not callable(getattr(agent, "run", None)) or not callable(getattr(agent, "run_cross_domain", None)):
                _fail("agent must expose callable run and run_cross_domain methods")
            if getattr(agent, "orchestrator", None) is not orchestrator:
                _fail("agent must be bound to the supplied orchestrator")
        if not isinstance(ledger, AutonomousGoalLedger):
            _fail("ledger must be an AutonomousGoalLedger")
        if not callable(task_resolver):
            _fail("task_resolver must be callable")
        if run_options_factory is not None and not callable(run_options_factory):
            _fail("run_options_factory must be callable or None")
        if journal is not None and not isinstance(journal, AutonomousGoalWorkerJournal):
            _fail("journal must be an AutonomousGoalWorkerJournal or None")
        if not isinstance(batch_id_prefix, str) or not batch_id_prefix.strip() or "\x00" in batch_id_prefix or len(batch_id_prefix.encode("utf-8")) > 128:
            _fail("batch_id_prefix is outside its bounded contract")
        self.orchestrator = orchestrator
        self.agent = agent
        self.ledger = ledger
        self.task_resolver = task_resolver
        self.run_options_factory = run_options_factory
        self.batch_id_prefix = batch_id_prefix.strip()
        self.worker = AutonomousGoalWorker(
            ledger,
            resolver=self._resolve,
            executor=self._execute,
            journal=journal,
        )
        self.loop = AutonomousGoalControlLoop(
            self.worker,
            batch_id_prefix=self.batch_id_prefix,
            evaluator=evaluator,
            learner=learner,
        )

    def _resolve(self, goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) -> Mapping[str, Any]:
        if goal.domain not in AUTONOMOUS_DOMAINS:
            _fail(f"goal {goal.goal_id} has an unsupported autonomous domain")
        task = self.task_resolver(goal, row)
        if not isinstance(task, str) or not task.strip() or "\x00" in task or len(task.encode("utf-8")) > 32_000:
            _fail(f"task_resolver returned an invalid task for goal {goal.goal_id}")
        # Options are intentionally fetched at execution time, not placed in the worker request.
        # This keeps TypeScript/Python behavior aligned for non-cloneable credential/callback
        # objects and prevents them from entering a worker digest.
        return {"task": task}

    def _run_options(self, goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) -> dict[str, Any]:
        supplied = {} if self.run_options_factory is None else self.run_options_factory(goal, row)
        options = _options(supplied)
        if goal.domain == "cross_domain":
            options["subtasks"] = _subtasks(options.get("subtasks"))
        elif "subtasks" in options:
            _fail("single-domain run options cannot contain subtasks")
        return options

    def _execute(self, request: AutonomousGoalExecutionRequest) -> Any:
        options = self._run_options(request.goal, request.schedule_row)
        if request.goal.domain == "cross_domain":
            subtasks = options.pop("subtasks")
            if self.agent is not None:
                return self.agent.run_cross_domain(task=request.task, subtasks=subtasks, **options)
            return self.orchestrator.run_cross_domain(task=request.task, subtasks=subtasks, **options)
        if self.agent is not None:
            return self.agent.run(task=request.task, domain=request.goal.domain, **options)
        return self.orchestrator.run(task=request.task, domain=request.goal.domain, **options)

    def metadata(self) -> dict[str, Any]:
        return {
            "schema": GOAL_AGENT_RUNTIME_SCHEMA,
            "batch_id_prefix": self.batch_id_prefix,
            "domain_count": len(AUTONOMOUS_DOMAINS),
            "domains": list(AUTONOMOUS_DOMAINS),
            "execution_surface": "autonomous_agent_facade" if self.agent is not None else "autonomous_task_orchestrator",
            "retention": GOAL_AGENT_RUNTIME_RETENTION,
            "secret_material": "never_returned",
        }

    def run(
        self,
        *,
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: GoalLoopOptionsFactory | None = None,
        max_cycles: int = 128,
        max_total_runs: int = 8_192,
    ) -> AutonomousGoalControlLoopResult:
        return self.loop.run(
            schedule_options=schedule_options,
            options_factory=options_factory,
            max_cycles=max_cycles,
            max_total_runs=max_total_runs,
        )


__all__ = [
    "GOAL_AGENT_RUNTIME_RETENTION",
    "GOAL_AGENT_RUNTIME_SCHEMA",
    "AutonomousGoalAgentRuntime",
    "GoalAgentRunOptionsFactory",
    "GoalAgentTaskResolver",
]
