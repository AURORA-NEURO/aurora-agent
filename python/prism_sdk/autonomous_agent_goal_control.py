"""Focused long-horizon goal-control methods for the application facade."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Callable, Mapping

from .autonomous_goal_control_loop import (
    AutonomousGoalControlLoopPreview,
    AutonomousGoalControlLoopResult,
    GoalLoopCheckpoint,
)
from .autonomous_goal_preview import InMemoryAutonomousGoalPreviewAdmissionLedger
from .autonomous_goal_recovery import AutonomousGoalRecoveryCoordinator
from .autonomous_goal_worker_journal import AutonomousGoalWorkerJournal
from .autonomous_protected_rehydration import AutonomousProtectedRehydrationAdapter
from .brain import BrainRunError
from .errors import ArgumentError
from .goals import AutonomousGoalLedger

if TYPE_CHECKING:
    from .autonomous_goal_agent import AutonomousGoalAgentRuntime


class AutonomousAgentGoalControlMixin:

    def goal_agent_runtime(
        self,
        ledger: AutonomousGoalLedger,
        *,
        task_resolver: Callable[[Any, Any], str] | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
        run_options_factory: Callable[[Any, Any], Mapping[str, Any]] | None = None,
        action_handoff_resolver: Callable[[Any, Any, str], Mapping[str, Any] | None] | None = None,
        evaluator: Any | None = None,
        learner: Any | None = None,
        journal: AutonomousGoalWorkerJournal | None = None,
        recovery: AutonomousGoalRecoveryCoordinator | None = None,
        preview_admission_ledger: InMemoryAutonomousGoalPreviewAdmissionLedger | None = None,
        batch_id_prefix: str = "autonomous-goal-agent",
    ) -> AutonomousGoalAgentRuntime:
        """Create the long-horizon goal bridge bound to this complete agent facade.

        The goal scheduler remains metadata-only, while each claimed goal is executed through
        this agent's normal session-aware routing surface.  That means model inventory, opaque
        credential handles, prompt/plan construction, provider approval, connector/tool policy,
        and online learning remain the same boundaries as a direct ``run`` call.  The task and
        run-options callbacks are invoked only at execution time and are never persisted by the
        goal ledger.  When configured, ``action_handoff_resolver`` supplies a caller-owned,
        digest-bound operator handoff that is replayed through ``execute_action_handoff`` before
        the provider run boundary.  Protected task rehydration, the worker journal, ordered crash
        recovery, and live preview-admission revocation are forwarded unchanged; the runtime checks
        that those caller-owned objects belong to the same goal ledger and journal.
        """

        from .autonomous_goal_agent import AutonomousGoalAgentRuntime

        try:
            return AutonomousGoalAgentRuntime(
                self.orchestrator,
                ledger,
                agent=self,
                task_resolver=task_resolver,
                protected_rehydration=protected_rehydration,
                run_options_factory=run_options_factory,
                action_handoff_resolver=action_handoff_resolver,
                evaluator=evaluator,
                learner=learner,
                journal=journal,
                recovery=recovery,
                preview_admission_ledger=preview_admission_ledger,
                batch_id_prefix=batch_id_prefix,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("goal agent runtime could not be created") from error

    def run_goal_control_loop(
        self,
        ledger: AutonomousGoalLedger,
        *,
        task_resolver: Callable[[Any, Any], str] | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
        run_options_factory: Callable[[Any, Any], Mapping[str, Any]] | None = None,
        action_handoff_resolver: Callable[[Any, Any, str], Mapping[str, Any] | None] | None = None,
        evaluator: Any | None = None,
        learner: Any | None = None,
        journal: AutonomousGoalWorkerJournal | None = None,
        recovery: AutonomousGoalRecoveryCoordinator | None = None,
        preview_admission_ledger: InMemoryAutonomousGoalPreviewAdmissionLedger | None = None,
        batch_id_prefix: str = "autonomous-goal-agent",
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: Callable[[Any], Mapping[str, Any]] | None = None,
        max_cycles: int = 128,
        max_total_runs: int = 8_192,
        run_id: str | None = None,
        resume_snapshot: Mapping[str, Any] | None = None,
        checkpoint: GoalLoopCheckpoint | None = None,
        expected_preview_digest: str | None = None,
        preview_approval: Mapping[str, Any] | None = None,
    ) -> AutonomousGoalControlLoopResult:
        """Run bounded goals using a task resolver or protected receipt rehydration.

        When both task sources are supplied, ``task_resolver`` takes precedence. A protected
        adapter can be used alone; unlike ``goal_agent_runtime()``, this convenience method is
        an execution entry point and requires one of those task sources when selected work runs.
        """

        runtime = self.goal_agent_runtime(
            ledger,
            task_resolver=task_resolver,
            protected_rehydration=protected_rehydration,
            run_options_factory=run_options_factory,
            action_handoff_resolver=action_handoff_resolver,
            evaluator=evaluator,
            learner=learner,
            journal=journal,
            recovery=recovery,
            preview_admission_ledger=preview_admission_ledger,
            batch_id_prefix=batch_id_prefix,
        )
        return runtime.run(
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

    async def run_goal_control_loop_async(
        self,
        ledger: AutonomousGoalLedger,
        *,
        task_resolver: Callable[[Any, Any], Any] | None = None,
        protected_rehydration: AutonomousProtectedRehydrationAdapter | None = None,
        run_options_factory: Callable[[Any, Any], Any] | None = None,
        action_handoff_resolver: Callable[[Any, Any, str], Any] | None = None,
        evaluator: Any | None = None,
        learner: Any | None = None,
        journal: AutonomousGoalWorkerJournal | None = None,
        recovery: AutonomousGoalRecoveryCoordinator | None = None,
        preview_admission_ledger: InMemoryAutonomousGoalPreviewAdmissionLedger | None = None,
        batch_id_prefix: str = "autonomous-goal-agent",
        schedule_options: Mapping[str, Any] | None = None,
        options_factory: Callable[[Any], Any] | None = None,
        max_cycles: int = 128,
        max_total_runs: int = 8_192,
        run_id: str | None = None,
        resume_snapshot: Mapping[str, Any] | None = None,
        checkpoint: GoalLoopCheckpoint | None = None,
        expected_preview_digest: str | None = None,
        preview_approval: Mapping[str, Any] | None = None,
    ) -> AutonomousGoalControlLoopResult:
        """Async-host counterpart that keeps the complete goal lifecycle in one facade call."""

        runtime = self.goal_agent_runtime(
            ledger,
            task_resolver=task_resolver,
            protected_rehydration=protected_rehydration,
            run_options_factory=run_options_factory,
            action_handoff_resolver=action_handoff_resolver,
            evaluator=evaluator,
            learner=learner,
            journal=journal,
            recovery=recovery,
            preview_admission_ledger=preview_admission_ledger,
            batch_id_prefix=batch_id_prefix,
        )
        return await runtime.run_async(
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

    def preview_goal_control_loop(
        self,
        ledger: AutonomousGoalLedger,
        *,
        batch_id_prefix: str = "autonomous-goal-agent",
        schedule_options: Mapping[str, Any] | None = None,
    ) -> AutonomousGoalControlLoopPreview:
        """Inspect the next goal admission decision without requiring task rehydration.

        The preview is provider-free and does not call a resolver, open credentials, invoke an
        evaluator, mutate learner state, or enter any execution boundary.  A task resolver is
        still required when the returned runtime is later used for ``run``.
        """

        runtime = self.goal_agent_runtime(
            ledger,
            task_resolver=None,
            batch_id_prefix=batch_id_prefix,
        )
        return runtime.preview(schedule_options=schedule_options)
