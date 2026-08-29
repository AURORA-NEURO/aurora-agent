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
from dataclasses import dataclass
from typing import Any

from .authoring import content_digest
from .autonomy import AUTONOMOUS_DOMAINS, AutonomousTaskOrchestrator
from .autonomous_goal_control_loop import (
    AutonomousGoalBanditLearner,
    AutonomousGoalControlLoop,
    AutonomousGoalControlLoopPreview,
    AutonomousGoalControlLoopResult,
    GoalLoopEvaluator,
    GoalLoopCheckpoint,
    GoalLoopLearner,
    GoalLoopOptionsFactory,
)
from .autonomous_goal_recovery import AutonomousGoalRecoveryCoordinator
from .autonomous_goal_scheduler import AutonomousGoalScheduleRow
from .autonomous_goal_worker import AutonomousGoalExecutionRequest, AutonomousGoalWorker
from .autonomous_goal_worker_journal import AutonomousGoalWorkerJournal
from .autonomous_goal_preview import InMemoryAutonomousGoalPreviewAdmissionLedger
from .autonomous_protected_rehydration import AutonomousProtectedRehydrationAdapter
from .autonomous_run_trace import (
    AutonomousRunTraceSession,
    AutonomousRunTraceStore,
    AutonomousRunTraceSummary,
    autonomous_run_trace_status,
)
from .autonomous_run_trace_registry import (
    AutonomousRunTraceRegistry,
    AutonomousRunTraceRegistryPublication,
    publish_autonomous_run_trace_registry_snapshot,
)
from .goals import AutonomousGoalError, AutonomousGoalLedger, AutonomousGoalRecord
from .llm_runtime import CompositeProviderInvocationObserver


GOAL_AGENT_RUNTIME_SCHEMA = "bioprism-autonomous-goal-agent-runtime/0.1"
GOAL_AGENT_RUNTIME_RETENTION = "metadata_only_goal_agent_bridge;tasks_prompts_parameters_credentials_and_results_not_retained"
GOAL_AGENT_TRACE_SCHEMA = "bioprism-autonomous-goal-agent-trace/0.1"
GOAL_AGENT_TRACE_RETENTION = "metadata_only_goal_control_trace;goal_task_prompts_parameters_credentials_and_results_not_retained"
_FORBIDDEN_RUN_OPTION_KEYS = frozenset({"task", "domain"})

GoalAgentTaskResolver = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow], str]
GoalAgentRunOptionsFactory = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow], Mapping[str, Any]]
GoalAgentActionHandoffRequest = Mapping[str, Any]
GoalAgentActionHandoffResolver = Callable[[AutonomousGoalRecord, AutonomousGoalScheduleRow, str], Mapping[str, Any] | None]
_ACTION_HANDOFF_REQUEST_KEYS = frozenset({"domain", "capability", "hints", "allow_cross_domain", "context", "connector"})


@dataclass(frozen=True, slots=True)
class AutonomousGoalAgentTracedRunResult:
    """Live caller-owned loop result plus a payload-free trace summary."""

    result: AutonomousGoalControlLoopResult
    trace: AutonomousRunTraceSummary
    trace_registry: AutonomousRunTraceRegistryPublication | None = None

    @property
    def status(self) -> str:
        return self.trace.status

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": GOAL_AGENT_TRACE_SCHEMA,
            "status": self.status,
            "trace": self.trace.to_dict(),
            **({"trace_registry": self.trace_registry.to_dict()} if self.trace_registry is not None else {}),
            "result": "caller_owned_live_result_not_serialized",
            "retention": GOAL_AGENT_TRACE_RETENTION,
            "secret_material": "never_returned",
        }


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


def _trace_domains(goal: AutonomousGoalRecord, options: Mapping[str, Any] | None = None) -> tuple[str, ...]:
    domains = [goal.domain]
    if goal.domain == "cross_domain" and options is not None:
        raw_subtasks = options.get("subtasks")
        if isinstance(raw_subtasks, Sequence) and not isinstance(raw_subtasks, (str, bytes, bytearray)):
            for item in raw_subtasks:
                if isinstance(item, Mapping) and item.get("domain") in AUTONOMOUS_DOMAINS:
                    domains.append(str(item["domain"]))
    return tuple(dict.fromkeys(domains))


def _goal_result_trace_status(result: Any) -> str:
    status = result.get("status") if isinstance(result, Mapping) else getattr(result, "status", None)
    return autonomous_run_trace_status(status if isinstance(status, str) else "unknown")


def _control_loop_trace_status(result: AutonomousGoalControlLoopResult) -> str:
    def count(name: str) -> int:
        value = result.status_counts.get(name, 0)
        return int(value) if isinstance(value, int) and not isinstance(value, bool) else 0

    completed = count("completed")
    paused = count("paused")
    blocked = count("blocked")
    failed = count("failed")
    if failed and not completed and not paused and not blocked:
        return "failed"
    if paused or blocked:
        return "partial" if completed else "paused"
    if failed:
        return "partial" if completed else "failed"
    if result.stop_reason == "all_terminal":
        return "completed"
    if completed:
        return "partial"
    return "paused" if result.stop_reason == "no_admissible_work" else "unknown"


def _selection_trace_callback(session: AutonomousRunTraceSession, existing: Any | None = None) -> Callable[..., Any]:
    """Translate selector-local statuses into the shared trace status vocabulary."""

    def callback(**event: Any) -> Any:
        if existing is not None:
            existing(**event)
        raw_status = event.get("status")
        status = {
            "running": "running",
            "selected": "completed",
            "abstained": "refused",
        }.get(raw_status, "failed")
        detail_digest = event.get("detail_digest") or content_digest(
            {
                "candidate_count": event.get("candidate_count"),
                "eligible_candidate_count": event.get("eligible_candidate_count"),
                "strategy": event.get("strategy"),
                "failover": event.get("failover"),
            }
        )
        return session.record(
            phase=event.get("phase"),
            status=status,
            provider=event.get("selected_provider"),
            model=event.get("selected_model"),
            selection_digest=event.get("selection_digest"),
            attempt=event.get("attempt"),
            detail_digest=detail_digest,
            failure_code=event.get("failure_code"),
        )

    return callback


def _action_handoff(value: Any, goal: AutonomousGoalRecord) -> dict[str, Any] | None:
    if value is None:
        return None
    handoff_source = value
    request: dict[str, Any] = {}
    if isinstance(value, Mapping) and "handoff" in value:
        handoff_source = value.get("handoff")
        supplied_request = value.get("request", {})
        if not isinstance(supplied_request, Mapping):
            _fail("action handoff request must be a mapping")
        request = dict(supplied_request)
    if not isinstance(handoff_source, Mapping):
        _fail("action handoff must be a mapping")
    from .autonomous_action_admission_controller import validate_autonomous_action_dispatch_handoff

    try:
        handoff = validate_autonomous_action_dispatch_handoff(handoff_source)
    except Exception as error:
        _fail(f"action handoff validation failed: {error}")
    unsupported = [key for key in request if not isinstance(key, str) or key not in _ACTION_HANDOFF_REQUEST_KEYS]
    if unsupported:
        _fail("action handoff request contains unsupported fields: " + ", ".join(map(str, unsupported)))
    if goal.domain == "cross_domain":
        request_domain = request.get("domain")
        if request_domain is not None and request_domain != "cross_domain":
            _fail("cross-domain goal action handoffs cannot select a single-domain request")
        if not handoff["cross_domain"] and "cross_domain" not in handoff["selected_domains"]:
            _fail("cross-domain goal action handoff is not cross-domain")
        if request_domain is None and not handoff["cross_domain"]:
            request["domain"] = "cross_domain"
    else:
        if goal.domain not in handoff["selected_domains"]:
            _fail(f"action handoff does not cover goal domain {goal.domain}")
        request_domain = request.get("domain")
        if request_domain is not None and request_domain != goal.domain:
            _fail(f"action handoff request domain does not match goal {goal.domain}")
        request["domain"] = goal.domain
    return {"handoff": handoff, "request": request}


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
        task_resolver: GoalAgentTaskResolver | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
        run_options_factory: GoalAgentRunOptionsFactory | None = None,
        action_handoff_resolver: GoalAgentActionHandoffResolver | None = None,
        evaluator: GoalLoopEvaluator | None = None,
        learner: GoalLoopLearner | AutonomousGoalBanditLearner | None = None,
        journal: AutonomousGoalWorkerJournal | None = None,
        recovery: AutonomousGoalRecoveryCoordinator | None = None,
        preview_admission_ledger: InMemoryAutonomousGoalPreviewAdmissionLedger | None = None,
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
        if task_resolver is not None and not callable(task_resolver):
            _fail("task_resolver must be callable or None")
        if protected_rehydration is not None and not isinstance(protected_rehydration, AutonomousProtectedRehydrationAdapter):
            _fail("protected_rehydration must be an AutonomousProtectedRehydrationAdapter or None")
        if run_options_factory is not None and not callable(run_options_factory):
            _fail("run_options_factory must be callable or None")
        if action_handoff_resolver is not None and not callable(action_handoff_resolver):
            _fail("action_handoff_resolver must be callable or None")
        if action_handoff_resolver is not None and agent is None:
            _fail("action_handoff_resolver requires an agent facade")
        if action_handoff_resolver is not None and not callable(getattr(agent, "execute_action_handoff", None)):
            _fail("agent must expose execute_action_handoff when action_handoff_resolver is configured")
        if journal is not None and not isinstance(journal, AutonomousGoalWorkerJournal):
            _fail("journal must be an AutonomousGoalWorkerJournal or None")
        if recovery is not None and not isinstance(recovery, AutonomousGoalRecoveryCoordinator):
            _fail("recovery must be an AutonomousGoalRecoveryCoordinator or None")
        if recovery is not None and recovery.ledger is not ledger:
            _fail("recovery coordinator must own the supplied ledger")
        if recovery is not None and (journal is None or recovery.journal.journal is not journal):
            _fail("recovery coordinator must own the supplied worker journal")
        if preview_admission_ledger is not None and not isinstance(preview_admission_ledger, InMemoryAutonomousGoalPreviewAdmissionLedger):
            _fail("preview_admission_ledger must be an InMemoryAutonomousGoalPreviewAdmissionLedger or None")
        if not isinstance(batch_id_prefix, str) or not batch_id_prefix.strip() or "\x00" in batch_id_prefix or len(batch_id_prefix.encode("utf-8")) > 128:
            _fail("batch_id_prefix is outside its bounded contract")
        self.orchestrator = orchestrator
        self.agent = agent
        self.ledger = ledger
        self.task_resolver = task_resolver
        self.protected_rehydration = protected_rehydration
        self.run_options_factory = run_options_factory
        self.action_handoff_resolver = action_handoff_resolver
        self.recovery = recovery
        self.preview_admission_ledger = preview_admission_ledger
        self.batch_id_prefix = batch_id_prefix.strip()
        self._trace_context: dict[str, Any] | None = None
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
            preview_admission_ledger=preview_admission_ledger,
        )

    def _resolve(self, goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) -> Mapping[str, Any]:
        if goal.domain not in AUTONOMOUS_DOMAINS:
            _fail(f"goal {goal.goal_id} has an unsupported autonomous domain")
        if self.task_resolver is not None:
            task = self.task_resolver(goal, row)
        elif self.protected_rehydration is not None:
            receipt = {
                "goal_id": goal.goal_id,
                "task_digest": goal.task_digest,
                "value_digest": goal.task_digest,
                "domain": goal.domain,
                "attempt": goal.attempt,
                "revision": goal.revision,
                "request_digest": content_digest(row.to_dict()),
            }
            task = self.protected_rehydration.resolve_receipt(
                receipt,
                domain=goal.domain,
                purpose="goal_task",
                value_kind="goal_task",
                one_time=False,
                digest_scheme="utf8_sha256",
            )
        else:
            _fail("task rehydration is not configured")
        if not isinstance(task, str) or not task.strip() or "\x00" in task or len(task.encode("utf-8")) > 32_000:
            _fail(f"resolved task is invalid for goal {goal.goal_id}")
        resolved_handoff = None if self.action_handoff_resolver is None else self.action_handoff_resolver(goal, row, task)
        binding = _action_handoff(resolved_handoff, goal)
        # Options are intentionally fetched at execution time, not placed in the worker request.
        # This keeps TypeScript/Python behavior aligned for non-cloneable credential/callback
        # objects and prevents them from entering a worker digest.
        return {"task": task, "parameters": {} if binding is None else {"action_handoff": binding}}

    def _run_options(self, goal: AutonomousGoalRecord, row: AutonomousGoalScheduleRow) -> dict[str, Any]:
        supplied = {} if self.run_options_factory is None else self.run_options_factory(goal, row)
        options = _options(supplied)
        # Goal context is durable metadata and must reach the transient model/planner boundary.
        # A factory may repeat the values for explicitness, but it cannot silently execute a goal
        # under a different capability or risk class than the one admitted by the ledger.
        for name, expected in (("capability", goal.capability), ("risk_class", goal.risk_class)):
            if expected is None:
                continue
            supplied_value = options.get(name)
            if supplied_value is not None and supplied_value != expected:
                _fail(f"run options {name} does not match goal {name}")
            options[name] = expected
        if goal.domain == "cross_domain":
            options["subtasks"] = _subtasks(options.get("subtasks"))
        elif "subtasks" in options:
            _fail("single-domain run options cannot contain subtasks")
        return options

    def _execute(self, request: AutonomousGoalExecutionRequest) -> Any:
        options = self._run_options(request.goal, request.schedule_row)
        trace_context = self._trace_context
        trace_domains = _trace_domains(request.goal, options)
        plan_digest = None if trace_context is None else content_digest(
            {
                "schema": GOAL_AGENT_TRACE_SCHEMA,
                "goal_id": request.goal.goal_id,
                "task_digest": request.goal.task_digest,
                "domain": request.goal.domain,
                "capability": request.goal.capability,
                "risk_class": request.goal.risk_class,
                "attempt": request.goal.attempt,
                "max_attempts": request.goal.max_attempts,
                "revision": request.goal.revision,
                "schedule_digest": request.schedule_digest,
            }
        )
        if trace_context is not None:
            trace_context["session"].record(
                phase="plan_compiled",
                status="running",
                domains=trace_domains,
                plan_digest=plan_digest,
                detail_digest=content_digest(
                    {
                        "goal_id": request.goal.goal_id,
                        "attempt": request.goal.attempt,
                        "revision": request.goal.revision,
                        "execution_binding_digest": request.execution_binding_digest,
                    }
                ),
            )
            existing_observer = options.get("invocation_observer")
            if existing_observer is not None:
                options["invocation_observer"] = CompositeProviderInvocationObserver(
                    [existing_observer, trace_context["observer"]]
                )
            else:
                options["invocation_observer"] = trace_context["observer"]
            options["trace_event_callback"] = _selection_trace_callback(
                trace_context["session"],
                options.get("trace_event_callback"),
            )
        binding = _action_handoff(request.parameters.get("action_handoff"), request.goal)
        if binding is not None:
            if self.agent is None:
                _fail("action handoff execution requires an agent facade")
            replay_request = dict(binding["request"])
            overlap = sorted(set(replay_request).intersection(options))
            if overlap:
                _fail("action handoff request overlaps run options: " + ", ".join(overlap))
            result = self.agent.execute_action_handoff(task=request.task, handoff=binding["handoff"], **replay_request, **options)
        elif request.goal.domain == "cross_domain":
            subtasks = options.pop("subtasks")
            if self.agent is not None:
                result = self.agent.run_cross_domain(task=request.task, subtasks=subtasks, **options)
            else:
                result = self.orchestrator.run_cross_domain(task=request.task, subtasks=subtasks, **options)
        elif self.agent is not None:
            result = self.agent.run(task=request.task, domain=request.goal.domain, **options)
        else:
            result = self.orchestrator.run(task=request.task, domain=request.goal.domain, **options)
        if trace_context is not None:
            status = result.get("status") if isinstance(result, Mapping) else getattr(result, "status", None)
            trace_context["session"].record(
                phase="evaluation_settled",
                status=_goal_result_trace_status(result),
                domains=trace_domains,
                plan_digest=plan_digest,
                detail_digest=content_digest(
                    {
                        "goal_id": request.goal.goal_id,
                        "attempt": request.goal.attempt,
                        "result_status": status if isinstance(status, str) else "unknown",
                    }
                ),
            )
        return result

    def metadata(self) -> dict[str, Any]:
        return {
            "schema": GOAL_AGENT_RUNTIME_SCHEMA,
            "batch_id_prefix": self.batch_id_prefix,
            "domain_count": len(AUTONOMOUS_DOMAINS),
            "domains": list(AUTONOMOUS_DOMAINS),
            "execution_surface": "autonomous_goal_action_handoff_facade" if self.action_handoff_resolver is not None else ("autonomous_agent_facade" if self.agent is not None else "autonomous_task_orchestrator"),
            "action_handoff_execution": "verified_handoff_replay_before_run_boundary" if self.action_handoff_resolver is not None else "not_configured",
            "task_rehydration": "caller_task_resolver_precedence" if self.task_resolver is not None else ("protected_receipt_adapter_fallback" if self.protected_rehydration is not None else "not_configured_preview_only"),
            "recovery_execution": "ordered_journal_then_control_checkpoint" if self.recovery is not None else "caller_composed",
            "trace_execution": "metadata_only_goal_control_trace",
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
        run_id: str | None = None,
        resume_snapshot: Mapping[str, Any] | None = None,
        checkpoint: GoalLoopCheckpoint | None = None,
        expected_preview_digest: str | None = None,
        preview_approval: Mapping[str, Any] | None = None,
    ) -> AutonomousGoalControlLoopResult:
        if self.recovery is not None:
            if expected_preview_digest is not None or preview_approval is not None:
                _fail("preview admission cannot be combined with recovery-owned resume")
            if checkpoint is not None:
                _fail("checkpoint is owned by the recovery coordinator")
            if resume_snapshot is not None:
                _fail("resume_snapshot is owned by the recovery coordinator")
            return self.recovery.resume(
                self.loop,
                options={
                    "schedule_options": schedule_options,
                    "options_factory": options_factory,
                    "max_cycles": max_cycles,
                    "max_total_runs": max_total_runs,
                    "run_id": run_id,
                    "checkpoint": self.recovery.checkpoint,
                    "expected_preview_digest": expected_preview_digest,
                    "preview_approval": preview_approval,
                },
            )
        return self.loop.run(
            schedule_options=schedule_options,
            options_factory=options_factory,
            max_cycles=max_cycles,
            max_total_runs=max_total_runs,
            run_id=run_id,
            resume_snapshot=resume_snapshot,
            checkpoint=checkpoint,
            expected_preview_digest=expected_preview_digest,
            preview_approval=preview_approval,
        )

    def preview(self, *, schedule_options: Mapping[str, Any] | None = None) -> AutonomousGoalControlLoopPreview:
        """Return the next goal admission explanation without rehydrating or dispatching work."""

        return self.loop.preview(schedule_options=schedule_options)

    def run_with_trace(
        self,
        *,
        trace_store: AutonomousRunTraceStore,
        run_id: str,
        trace_registry: AutonomousRunTraceRegistry | None = None,
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: GoalLoopOptionsFactory | None = None,
        max_cycles: int = 128,
        max_total_runs: int = 8_192,
        resume_snapshot: Mapping[str, Any] | None = None,
        checkpoint: GoalLoopCheckpoint | None = None,
        expected_preview_digest: str | None = None,
        preview_approval: Mapping[str, Any] | None = None,
    ) -> AutonomousGoalAgentTracedRunResult:
        if not all(callable(getattr(trace_store, name, None)) for name in ("append", "events")):
            _fail("run_with_trace requires a trace store")
        if trace_registry is not None and not isinstance(trace_registry, AutonomousRunTraceRegistry):
            _fail("run_with_trace trace_registry must be an AutonomousRunTraceRegistry")
        if self._trace_context is not None:
            _fail("run_with_trace cannot be re-entered while another trace is active")
        goals = self.ledger.list(limit=512)
        unsupported = [goal.domain for goal in goals if goal.domain not in AUTONOMOUS_DOMAINS]
        if unsupported:
            _fail("run_with_trace found unsupported goal domains: " + ", ".join(unsupported))
        domains = tuple(dict.fromkeys(goal.domain for goal in goals)) or ("cross_domain",)
        goal_metadata = [
            {
                "goal_id": goal.goal_id,
                "task_digest": goal.task_digest,
                "domain": goal.domain,
                "capability": goal.capability,
                "risk_class": goal.risk_class,
                "status": goal.status,
                "attempt": goal.attempt,
                "max_attempts": goal.max_attempts,
                "revision": goal.revision,
            }
            for goal in goals
        ]
        task_digest = content_digest({"schema": GOAL_AGENT_TRACE_SCHEMA, "run_id": run_id, "goals": goal_metadata})
        plan_digest = content_digest({"schema": GOAL_AGENT_TRACE_SCHEMA, "batch_id_prefix": self.batch_id_prefix, "goals": goal_metadata})
        session = AutonomousRunTraceSession(trace_store, run_id=run_id, task_digest=task_digest, domains=domains)
        session.started(detail_digest=content_digest({"goal_count": len(goal_metadata), "domain_count": len(domains)}))
        session.record(
            phase="plan_compiled",
            status="running",
            domains=domains,
            plan_digest=plan_digest,
            detail_digest=content_digest({"goal_count": len(goal_metadata), "domain_count": len(domains)}),
        )
        self._trace_context = {
            "session": session,
            "observer": session.provider_observer(),
        }
        try:
            result = self.run(
                schedule_options=schedule_options,
                options_factory=options_factory,
                max_cycles=max_cycles,
                max_total_runs=max_total_runs,
                run_id=run_id,
                resume_snapshot=resume_snapshot,
                checkpoint=checkpoint,
                expected_preview_digest=expected_preview_digest,
                preview_approval=preview_approval,
            )
            session.record(
                phase="learning_prepared",
                status="running",
                domains=domains,
                plan_digest=plan_digest,
                detail_digest=content_digest(
                    {
                        "total_selected": result.total_selected,
                        "total_claimed": result.total_claimed,
                        "total_runs": result.total_runs,
                        "evaluation_count": result.evaluation_count,
                        "evaluation_digest": result.evaluation_digest,
                        "learning_state_digest": result.learning_state_digest,
                        "stop_reason": result.stop_reason,
                    }
                ),
            )
            session.complete(
                status=_control_loop_trace_status(result),
                domains=domains,
                plan_digest=plan_digest,
                detail_digest=content_digest(result.to_dict()),
            )
            publication = None if trace_registry is None else publish_autonomous_run_trace_registry_snapshot(trace_registry, trace_store, run_id)
            return AutonomousGoalAgentTracedRunResult(result=result, trace=session.summary(), trace_registry=publication)
        except Exception as error:
            try:
                session.fail(
                    failure_class=type(error).__name__,
                    failure_code="goal_control_loop_error",
                    detail_digest=content_digest({"failure_class": type(error).__name__}),
                )
            except Exception:
                pass
            if trace_registry is not None:
                publish_autonomous_run_trace_registry_snapshot(trace_registry, trace_store, run_id)
            raise
        finally:
            self._trace_context = None

    def restore(self, *, now_ns: int | None = None) -> dict[str, Any]:
        if self.recovery is None:
            _fail("restore requires a recovery coordinator")
        return self.recovery.restore(now_ns=now_ns)


__all__ = [
    "GOAL_AGENT_TRACE_RETENTION",
    "GOAL_AGENT_TRACE_SCHEMA",
    "GOAL_AGENT_RUNTIME_RETENTION",
    "GOAL_AGENT_RUNTIME_SCHEMA",
    "AutonomousGoalAgentRuntime",
    "AutonomousGoalAgentTracedRunResult",
    "GoalAgentActionHandoffRequest",
    "GoalAgentActionHandoffResolver",
    "GoalAgentRunOptionsFactory",
    "GoalAgentTaskResolver",
]
