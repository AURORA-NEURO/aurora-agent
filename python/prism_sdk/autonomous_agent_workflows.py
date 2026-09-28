"""Focused autonomous-agent workflows methods."""

from __future__ import annotations

from .autonomy import (
    Any,
    AutonomousCrossDomainLearningResult,
    AutonomousCrossDomainReplanResult,
    AutonomousCrossDomainResult,
    AutonomousCrossDomainTrajectoryLearningResult,
    AutonomousRunTraceSession,
    AutonomousRunTraceStore,
    AutonomousTaskBlueprint,
    AutonomousTracedRunResult,
    AutonomousWorkflowLearningResult,
    AutonomousWorkflowRun,
    AutonomousWorkflowTrajectoryLearningResult,
    CredentialHandle,
    CredentialSession,
    Mapping,
    ModelCandidate,
    Sequence,
    _cross_domain_subtask_domains_for_launch_admission,
    autonomous_run_trace_status,
    uuid,
)

class AutonomousAgentWorkflowsMixin:
    def run_cross_domain_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainLearningResult:
        """Run fan-out and synthesis while adapting routing between completed episodes."""

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=tuple(
                dict.fromkeys(
                    ["cross_domain"]
                    + [
                        value.get("domain")
                        for value in subtasks
                        if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                    ]
            )),
            task=task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_cross_domain_learning(
                task=task,
                subtasks=subtasks,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_cross_domain_learning_with_launch_admission(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainLearningResult:
        """Run adaptive fan-out/fan-in only after every specialist domain is admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        requested_domains = _cross_domain_subtask_domains_for_launch_admission(subtasks)
        authorize_autonomous_launch_domains(launch_admission, requested_domains)
        return self.run_cross_domain_learning(
            task=task,
            subtasks=subtasks,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_cross_domain_with_launch_admission(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainResult:
        """Run a fan-out/fan-in task only when admission covers every specialist domain."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        requested_domains = _cross_domain_subtask_domains_for_launch_admission(subtasks)
        authorize_autonomous_launch_domains(launch_admission, requested_domains)
        return self.run_cross_domain(
            task=task,
            subtasks=subtasks,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_cross_domain_trajectory_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainTrajectoryLearningResult:
        """Run fan-out and synthesis with one delayed, discounted trajectory update."""

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=tuple(
                dict.fromkeys(
                    ["cross_domain"]
                    + [
                        value.get("domain")
                        for value in subtasks
                        if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                    ]
                )
            ),
            task=task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_cross_domain_trajectory_learning(
                task=task,
                subtasks=subtasks,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_cross_domain_trajectory_learning_with_launch_admission(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainTrajectoryLearningResult:
        """Run trajectory learning only after every specialist domain is admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        requested_domains = _cross_domain_subtask_domains_for_launch_admission(subtasks)
        authorize_autonomous_launch_domains(launch_admission, requested_domains)
        return self.run_cross_domain_trajectory_learning(
            task=task,
            subtasks=subtasks,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_cross_domain_replan_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainReplanResult:
        """Run bounded evaluator-guided cross-domain replans with delayed credit per attempt."""

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=tuple(
                dict.fromkeys(
                    ["cross_domain"]
                    + [
                        value.get("domain")
                        for value in subtasks
                        if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                    ]
                )
            ),
            task=task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_cross_domain_replan_learning(
                task=task,
                subtasks=subtasks,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_cross_domain_replan_learning_with_launch_admission(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainReplanResult:
        """Run bounded cross-domain replanning only after every domain is admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        requested_domains = _cross_domain_subtask_domains_for_launch_admission(subtasks)
        authorize_autonomous_launch_domains(launch_admission, requested_domains)
        return self.run_cross_domain_replan_learning(
            task=task,
            subtasks=subtasks,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_workflow(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowRun:
        """Run a staged workflow with the agent's catalogue, health, and durable state."""

        run_options = self._prompt_learning_options(kwargs)
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=run_options,
            tool_domains=(blueprint.spec.domain,),
            task=blueprint.spec.task,
            resume_learning=True,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_workflow(
                blueprint=blueprint,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_workflow_with_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowRun:
        """Run a reviewed workflow only after its blueprint domain is admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (blueprint.spec.domain,))
        return self.run_workflow(
            blueprint=blueprint,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_workflow_with_trace(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Execute a staged workflow with live stage/provider trace events."""

        self._trace_store(trace_store)
        resolved_run_id = run_id or kwargs.pop("execution_id", None) or f"trace-{uuid.uuid4().hex}"
        session = AutonomousRunTraceSession(
            trace_store,
            run_id=resolved_run_id,
            task_digest=blueprint.spec.task_digest,
            domains=(blueprint.spec.domain,),
        )
        session.started()
        live_observer = session.provider_observer()
        try:
            result = self.run_workflow(
                blueprint=blueprint,
                credentials=credentials,
                model_candidates=model_candidates,
                execution_id=resolved_run_id,
                invocation_observer=live_observer,
                trace_event_callback=session.record,
                **kwargs,
            )
            receipts: list[Mapping[str, Any]] = []
            for stage in result.stage_results:
                if stage.result is not None:
                    for brain_result in self._trace_brain_results(stage.result):
                        receipts.extend(brain_result.provider_invocations)
            if not trace_store.events({"run_id": resolved_run_id, "phase": "provider_invocation_finished"}):
                session.record_provider_receipts(receipts)
            checkpoint_digest = getattr(result.checkpoint, "checkpoint_digest", None)
            session.complete(
                status=autonomous_run_trace_status(result.status),
                plan_digest=blueprint.plan.get("plan_digest"),
                detail_digest=checkpoint_digest,
            )
        except Exception as error:
            session.fail(failure_class=type(error).__name__, failure_code="execution_error")
            raise
        return AutonomousTracedRunResult(result=result, trace=session.summary())

    def run_workflow_with_trace_and_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        trace_store: AutonomousRunTraceStore,
        run_id: str | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousTracedRunResult:
        """Trace a workflow only after its blueprint domain is launch-admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (blueprint.spec.domain,))
        return self.run_workflow_with_trace(
            blueprint=blueprint,
            credentials=credentials,
            trace_store=trace_store,
            run_id=run_id,
            model_candidates=model_candidates,
            **kwargs,
        )

    def run_workflow_learning(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowLearningResult:
        """Run staged workflow learning, resuming the latest value-only bandit state by default."""

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=(blueprint.spec.domain,),
            task=blueprint.spec.task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_workflow_learning(
                blueprint=blueprint,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_workflow_learning_with_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowLearningResult:
        """Run workflow learning only after its blueprint domain is launch-admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (blueprint.spec.domain,))
        return self.run_workflow_learning(
            blueprint=blueprint,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_workflow_cycle(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> "AutonomousWorkflowCycleResult":
        """Run a bounded evaluator-guided workflow recovery cycle.

        The cycle remains explicit and opt-in.  Provider credentials are still supplied as
        opaque handles or a live credential session, and every retry is finalized through the
        same execution controller used by ordinary workflow calls.
        """

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=(blueprint.spec.domain,),
            task=blueprint.spec.task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_workflow_cycle(
                blueprint=blueprint,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_workflow_cycle_with_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> "AutonomousWorkflowCycleResult":
        """Run a workflow recovery cycle only after its domain is launch-admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (blueprint.spec.domain,))
        return self.run_workflow_cycle(
            blueprint=blueprint,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_workflow_trajectory_learning(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowTrajectoryLearningResult:
        """Run a staged workflow and apply one delayed, discounted trajectory update."""

        options = self._prompt_learning_options(kwargs)
        options["bandit_state"] = self.learning_state() if bandit_state is None else bandit_state
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=(blueprint.spec.domain,),
            task=blueprint.spec.task,
            resume_learning=False,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_workflow_trajectory_learning(
                blueprint=blueprint,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_workflow_trajectory_learning_with_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousWorkflowTrajectoryLearningResult:
        """Run workflow trajectory learning only after its domain is launch-admitted."""

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (blueprint.spec.domain,))
        return self.run_workflow_trajectory_learning(
            blueprint=blueprint,
            credentials=credentials,
            model_candidates=model_candidates,
            bandit_state=bandit_state,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_cross_domain(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> AutonomousCrossDomainResult:
        """Run specialist fan-out and synthesis through the shared safety/learning envelope."""

        run_options = self._prompt_learning_options(kwargs)
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=run_options,
            tool_domains=tuple(
                dict.fromkeys(
                    ["cross_domain"]
                    + [
                        value.get("domain")
                        for value in subtasks
                        if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                    ]
                )
            ),
            task=task,
            resume_learning=True,
            attach_execution_plan_context=False,
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run_cross_domain(
                task=task,
                subtasks=subtasks,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentWorkflowsMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
