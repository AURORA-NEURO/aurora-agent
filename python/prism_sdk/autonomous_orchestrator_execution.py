"""Focused autonomous-orchestrator execution methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAIN_POLICY_MODES,
    AUTONOMOUS_EXECUTION_MODES,
    Any,
    AutonomousAuthorizationContext,
    AutonomousContextBudgetOptions,
    AutonomousCostReservationCallback,
    AutonomousDomainPolicyError,
    AutonomousExecutionController,
    AutonomousGoalCriterion,
    AutonomousGoalError,
    AutonomousGoalLedger,
    AutonomousLearningResult,
    AutonomousMemoryConsolidator,
    AutonomousPromptLearningState,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    AutonomousTaskBlueprint,
    BrainEpisodicMemory,
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    BrainToolLoopResult,
    Callable,
    CredentialHandle,
    DomainEvaluatorRegistry,
    GOAL_RETENTION,
    GOAL_STEP_SCHEMA,
    Mapping,
    MemoryQuery,
    MissionPolicy,
    ProviderContentPart,
    ProviderInvocationObserver,
    ProviderTool,
    Sequence,
    _apply_versioned_prompt,
    _assert_task_decision_allows_provider,
    _emit_trace_event,
    _goal_learning_settlement_metadata,
    _identifier,
    _json_text,
    _merge_goal_settlement_metadata,
    _route_digest,
    _selection_overrides_with_weights,
    _sequence,
    autonomous_domain_policy,
    content_digest,
    evaluate_autonomous_domain_policy,
    goal_status_for_result,
    goal_task_digest,
    normalize_provider_content_parts,
)

class AutonomousOrchestratorExecutionMixin:
    def _execute(
        self,
        blueprint: AutonomousTaskBlueprint,
        *,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        ledger: BrainLearningLedger | None,
        bandit_state: Mapping[str, Any] | None,
        content_parts: Sequence[Mapping[str, Any]] | None,
        contextual_observations: Sequence[Mapping[str, Any]],
        input_tokens: int,
        context_budget: AutonomousContextBudgetOptions | Mapping[str, Any] | None,
        requested_output_tokens: int,
        max_cost_per_million_tokens: int | None,
        max_latency_ms: int | None,
        min_quality: float | None,
        selection_overrides: Mapping[str, Any] | None,
        approve_provider_call: bool,
        approve_mission_dispatch: bool,
        run_id: str | None,
        max_output_tokens: int,
        temperature: float | None,
        response_schema: Mapping[str, Any] | None,
        idempotency_key: str | None,
        mission_policy: MissionPolicy | Mapping[str, Any] | None,
        mission_options: Mapping[str, Any] | None,
        route_request: Mapping[str, Any] | None,
        enforce_route_tools: bool,
        require_resolved_route: bool,
        provider_tools: Sequence[ProviderTool],
        tool_choice: str | None,
        max_provider_failovers: int,
        execution_mode: str,
        tool_loop_options: Mapping[str, Any] | None,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        reserve_cost: AutonomousCostReservationCallback | None = None,
    ) -> BrainRunResult | BrainToolLoopResult | BrainMissionResult:
        # Keep the legacy ``mission_policy`` shorthand while making the execution route
        # explicit for new callers.
        if execution_mode not in AUTONOMOUS_EXECUTION_MODES:
            raise BrainRunError(f"unsupported autonomous execution mode: {execution_mode!r}")
        effective_mode = "mission" if execution_mode == "provider" and mission_policy is not None else execution_mode
        if effective_mode == "provider":
            result = self.brain.run_adaptive(
                task=blueprint.spec.task,
                model_candidates=model_candidates,
                prompt=blueprint.prompt,
                plan=blueprint.plan,
                credentials=credentials,
                ledger=ledger,
                bandit_state=bandit_state,
                content_parts=content_parts,
                context=blueprint.selection_context,
                contextual_observations=contextual_observations,
                required_capabilities=blueprint.required_capabilities,
                input_tokens=input_tokens,
                context_budget=context_budget,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                approve_provider_call=approve_provider_call,
                run_id=run_id,
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                require_json=blueprint.spec.require_json,
                response_schema=response_schema or blueprint.spec.response_schema,
                idempotency_key=idempotency_key,
                tools=provider_tools,
                tool_choice=tool_choice,
                max_provider_failovers=max_provider_failovers,
                execution_controller=execution_controller,
                invocation_observer=invocation_observer,
                reserve_cost=reserve_cost,
                trace_event_callback=trace_event_callback,
                authorization_context=authorization_context,
                authorization_domain=blueprint.spec.domain,
            )
            return self._attach_domain_response_evaluation(blueprint, result)
        if effective_mode == "tool_loop":
            if tool_loop_options is not None and not isinstance(tool_loop_options, Mapping):
                raise BrainRunError("tool_loop_options must be a mapping or None")
            loop_options = {} if tool_loop_options is None else dict(tool_loop_options)
            forbidden = {
                "task", "model_candidates", "prompt", "plan", "credentials", "context",
                "contextual_observations", "required_capabilities", "ledger", "selection_overrides",
            }
            unknown = sorted(forbidden.intersection(loop_options))
            if unknown:
                raise BrainRunError("tool_loop_options cannot override generated fields: " + ", ".join(unknown))
            loop_options.update(
                {
                    "approve_provider_call": approve_provider_call,
                    "run_id": run_id,
                    "max_output_tokens": max_output_tokens,
                    "temperature": temperature,
                    "require_json": blueprint.spec.require_json,
                    "response_schema": response_schema or blueprint.spec.response_schema,
                    "idempotency_key": idempotency_key,
                    "tool_choice": tool_choice,
                    "approve_mission_dispatch": approve_mission_dispatch,
                }
            )
            if mission_policy is not None:
                loop_options["mission_policy"] = mission_policy
            if provider_tools:
                loop_options["provider_tools"] = tuple(provider_tools)
            if route_request is not None:
                loop_options["route_request"] = dict(route_request)
                # Route enforcement narrows the provider-visible surface even for a native
                # callback-authorized loop. Mission-policy intersection remains conditional on
                # the built-in mission authorizer, but route evidence must constrain every facade.
                loop_options["enforce_route_tools"] = enforce_route_tools
                loop_options["require_resolved_route"] = require_resolved_route
            result = self.brain.run_adaptive_tool_loop(
                task=blueprint.spec.task,
                model_candidates=model_candidates,
                prompt=blueprint.prompt,
                plan=blueprint.plan,
                credentials=credentials,
                ledger=ledger,
                bandit_state=bandit_state,
                content_parts=content_parts,
                context=blueprint.selection_context,
                contextual_observations=contextual_observations,
                required_capabilities=blueprint.required_capabilities,
                input_tokens=input_tokens,
                context_budget=context_budget,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                tool_loop_options=loop_options,
                max_provider_failovers=max_provider_failovers,
                execution_controller=execution_controller,
                invocation_observer=invocation_observer,
                reserve_cost=reserve_cost,
                trace_event_callback=trace_event_callback,
                authorization_context=authorization_context,
                authorization_domain=blueprint.spec.domain,
            )
            return self._attach_domain_response_evaluation(blueprint, result)
        if effective_mode != "mission":
            raise BrainRunError(f"unsupported autonomous execution mode: {effective_mode!r}")
        if mission_policy is None:
            raise BrainRunError("mission execution requires mission_policy")
        options = self._merge_options(
            mission_options,
            context=blueprint.selection_context,
            content_parts=content_parts,
            required_capabilities=blueprint.required_capabilities,
            contextual_observations=contextual_observations,
            input_tokens=input_tokens,
            context_budget=context_budget,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            approve_provider_call=approve_provider_call,
            approve_mission_dispatch=approve_mission_dispatch,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            response_schema=response_schema or blueprint.spec.response_schema,
            idempotency_key=idempotency_key,
            route_request=route_request,
            enforce_route_tools=enforce_route_tools,
            require_resolved_route=require_resolved_route,
            provider_tools=provider_tools,
            tool_choice=tool_choice,
            max_provider_failovers=max_provider_failovers,
            authorization_context=authorization_context,
            authorization_domain=blueprint.spec.domain,
            reserve_cost=reserve_cost,
        )
        result = self.brain.run_adaptive_mission(
            task=blueprint.spec.task,
            model_candidates=model_candidates,
            prompt=blueprint.prompt,
            plan=blueprint.plan,
            credentials=credentials,
            mission_policy=mission_policy,
            ledger=ledger,
            bandit_state=bandit_state,
            execution_controller=execution_controller,
            invocation_observer=invocation_observer,
            trace_event_callback=trace_event_callback,
            **options,
        )
        return self._attach_domain_response_evaluation(blueprint, result)

    @staticmethod
    def _append_replan(
        prompt: Mapping[str, Any],
        *,
        attempt: int,
        result: BrainRunResult | BrainToolLoopResult,
        decision: BrainEvaluatorDecision,
    ) -> dict[str, Any]:
        current = dict(prompt)
        raw_context = current.get("context", [])
        if not isinstance(raw_context, Sequence) or isinstance(raw_context, (str, bytes)):
            raise BrainRunError("autonomous prompt context must be a sequence when replanning")
        chunks = [dict(chunk) for chunk in raw_context if isinstance(chunk, Mapping)]
        if len(chunks) != len(raw_context) or any(chunk.get("id") == "autonomy-replan" for chunk in chunks):
            raise BrainRunError("autonomous prompt has malformed or duplicate replan context")
        if isinstance(result, BrainRunResult):
            previous_status = result.status
            previous_outcome_digest = result.outcome_digest
        else:
            previous_status = result.status
            previous_outcome_digest = content_digest(
                {
                    "brain_outcome_digest": result.brain_run.outcome_digest,
                    "status": result.status,
                    "provider_loop_status": None if result.provider_loop is None else result.provider_loop.status,
                    "turns": None if result.provider_loop is None else result.provider_loop.turns,
                    "tool_calls": None if result.provider_loop is None else result.provider_loop.tool_calls,
                }
            )
        chunks.append(
            {
                "id": "autonomy-replan",
                "role": "developer",
                "content": _json_text(
                    {
                        "workflow": "bounded_autonomous_replan",
                        "attempt": attempt,
                        "previous_status": previous_status,
                        "previous_outcome_digest": previous_outcome_digest,
                        "failure_class": decision.failure_class,
                        "instruction": decision.replan_instruction,
                        "does_not_authorize": ["new tools", "new credentials", "external effects"],
                    }
                ),
                "required": True,
                "priority": 950,
            }
        )
        current["context"] = chunks
        return current

    def run_learning(
        self,
        *,
        task: str,
        domain: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        bandit_state: Mapping[str, Any],
        memory: BrainEpisodicMemory | None = None,
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        evidence: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        max_replans: int = 1,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
        memory_tags: Sequence[str] = (),
        **kwargs: Any,
    ) -> AutonomousLearningResult:
        """Run a provider task, explicitly score it, update bandit state, and optionally replan.

        This provider-only learning path is useful for ordinary answers and analysis. Mission
        tasks can use :meth:`run` with ``mission_policy`` plus the existing durable mission
        learning cycle. A failed evaluation never silently becomes a reward: it is recorded with
        the evaluator's bounded decision and only then may request one more proposal.
        """

        return self.run(
            task=task,
            domain=domain,
            model_candidates=model_candidates,
            credentials=credentials,
            bandit_state=bandit_state,
            memory=memory,
            evaluator=evaluator,
            evaluator_registry=evaluator_registry,
            evidence=evidence,
            ledger=ledger,
            max_replans=max_replans,
            memory_query=memory_query,
            memory_limit=memory_limit,
            memory_tags=memory_tags,
            learn=True,
            **kwargs,
        )

    def _run_prepared(self, blueprint: AutonomousTaskBlueprint, **kwargs: Any) -> BrainRunResult | BrainToolLoopResult | BrainMissionResult:
        allowed = {
            "model_candidates", "credentials", "ledger", "content_parts", "contextual_observations", "input_tokens", "context_budget",
            "requested_output_tokens", "max_cost_per_million_tokens", "max_latency_ms", "min_quality",
            "selection_overrides", "approve_provider_call", "approve_mission_dispatch", "run_id",
            "max_output_tokens", "temperature", "response_schema", "idempotency_key", "mission_policy",
            "mission_options", "route_request", "enforce_route_tools", "require_resolved_route",
            "provider_tools", "tool_choice", "max_provider_failovers", "prompt", "execution_mode",
            "tool_loop_options", "bandit_state",
            "execution_controller", "invocation_observer",
            "trace_event_callback", "authorization_context", "reserve_cost",
        }
        unknown = sorted(set(kwargs).difference(allowed))
        if unknown:
            raise BrainRunError("unsupported autonomous execution options: " + ", ".join(unknown))
        prompt = kwargs.pop("prompt", blueprint.prompt)
        if prompt is not blueprint.prompt:
            replacement = AutonomousTaskBlueprint(
                spec=blueprint.spec,
                profile=blueprint.profile,
                domain_pack=blueprint.domain_pack,
                workflow=blueprint.workflow,
                selection_context=blueprint.selection_context,
                prompt=prompt,
                plan=blueprint.plan,
                required_capabilities=blueprint.required_capabilities,
                domain_policy=blueprint.domain_policy,
                task_lens=blueprint.task_lens,
                task_intent=blueprint.task_intent,
                task_decision=blueprint.task_decision,
                response_contract=blueprint.response_contract,
            )
        else:
            replacement = blueprint
        kwargs.setdefault("execution_mode", blueprint.spec.execution_mode)
        kwargs.setdefault("tool_loop_options", None)
        return self._execute(replacement, **kwargs)

    def run_goal_step(
        self,
        *,
        goal_store: AutonomousGoalLedger,
        goal_id: str,
        task: str,
        domain: str,
        goal_criteria: Sequence[AutonomousGoalCriterion | Mapping[str, Any]] = (),
        goal_max_attempts: int = 8,
        criterion_updates: Sequence[Mapping[str, Any]] = (),
        settlement_metadata: Mapping[str, Any] | None = None,
        run_options: Mapping[str, Any] | None = None,
        run_callable: Callable[..., Any] | None = None,
        settlement_metadata_factory: Callable[[Any], Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Run one bounded attempt while advancing a durable objective lifecycle.

        Provider responses remain transient in the returned ``result`` value.  Only the goal
        transition, result status, and a value-only outcome digest enter the durable ledger.  A
        provider approval pause, route review, evaluator-incomplete completion, or retryable
        failure therefore becomes an explicit resumable goal state instead of an apparent success.
        """

        if not isinstance(goal_store, AutonomousGoalLedger):
            raise BrainRunError("goal_store must be an AutonomousGoalLedger")
        if not isinstance(run_options, Mapping) and run_options is not None:
            raise BrainRunError("run_options must be a mapping or None")
        if not isinstance(settlement_metadata, Mapping) and settlement_metadata is not None:
            raise BrainRunError("settlement_metadata must be a mapping or None")
        metadata = {} if settlement_metadata is None else dict(settlement_metadata)
        unknown_metadata = sorted(set(metadata).difference({"evaluator_digest", "learning_state_digest", "progress_digest"}))
        if unknown_metadata:
            raise BrainRunError("unsupported goal settlement metadata: " + ", ".join(unknown_metadata))
        for metadata_name, metadata_value in metadata.items():
            if metadata_value is not None:
                try:
                    _route_digest(metadata_value, f"goal settlement {metadata_name}")
                except BrainRunError as error:
                    raise BrainRunError(f"goal settlement {metadata_name} must be a digest or None") from error
        options = {} if run_options is None else dict(run_options)
        if "task" in options or "domain" in options:
            raise BrainRunError("run_options cannot override goal task or domain")
        try:
            task_digest = goal_task_digest(task)
            current = goal_store.get(goal_id)
            capability = options.get("capability")
            risk_class = options.get("risk_class")
            if current is None:
                current = goal_store.create(
                    goal_id=goal_id,
                    task_digest=task_digest,
                    domain=domain,
                    capability=capability,
                    risk_class=risk_class,
                    criteria=goal_criteria,
                    max_attempts=goal_max_attempts,
                )
            else:
                if current.task_digest != task_digest or current.domain != domain:
                    raise BrainRunError("goal identity does not match the requested task or domain")
                if capability is not None and current.capability != capability:
                    raise BrainRunError("goal capability does not match the requested capability")
                if risk_class is not None and current.risk_class != risk_class:
                    raise BrainRunError("goal risk_class does not match the requested risk class")
            if current.status in {"completed", "cancelled"}:
                terminal_outcome_digest = content_digest(
                    {"goal_id": current.goal_id, "attempt": current.attempt, "result_status": "terminal"}
                )
                return {
                    "schema": GOAL_STEP_SCHEMA,
                    "goal": current.to_dict(),
                    "result": None,
                    "result_status": "terminal",
                    "goal_status": current.status,
                    "outcome_digest": terminal_outcome_digest,
                    "evaluator_digest": current.evaluator_digest,
                    "learning_state_digest": current.learning_state_digest,
                    "progress_digest": current.progress_digest,
                    "retention": GOAL_RETENTION,
                    "secret_material": "never_returned",
                }
            if current.status in {"blocked", "failed"}:
                current = goal_store.transition(current.goal_id, "ready", expected_revision=current.revision)
            running = goal_store.transition(current.goal_id, "running", expected_revision=current.revision)
        except AutonomousGoalError as error:
            raise BrainRunError("goal lifecycle admission failed") from error

        try:
            if run_callable is not None and not callable(run_callable):
                raise BrainRunError("run_callable must be callable or None")
            result = (
                self.run(task=task, domain=domain, **options)
                if run_callable is None
                else run_callable(task=task, domain=domain, **options)
            )
        except Exception as error:
            try:
                exception_status = f"exception:{type(error).__name__}"
                goal_store.transition(
                    running.goal_id,
                    "failed",
                    expected_revision=running.revision,
                    blockers=(f"exception:{type(error).__name__}",),
                    next_action_digest=goal_task_digest("goal-retry"),
                    outcome_digest=content_digest(
                        {"goal_id": running.goal_id, "attempt": running.attempt, "result_status": exception_status}
                    ),
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("goal failure transition failed") from transition_error
            raise

        result_status = result.get("status") if isinstance(result, Mapping) else getattr(result, "status", None)
        if (
            not isinstance(result_status, str)
            or not result_status.strip()
            or "\x00" in result_status
            or len(result_status.encode("utf-8")) > 128
        ):
            result_status = "failed"
        else:
            result_status = result_status.strip()
        outcome_digest = content_digest(
            {"goal_id": running.goal_id, "attempt": running.attempt, "result_status": result_status}
        )
        try:
            metadata = _merge_goal_settlement_metadata(metadata, settlement_metadata_factory, result)
        except BrainRunError:
            try:
                goal_store.transition(
                    running.goal_id,
                    "blocked",
                    expected_revision=running.revision,
                    blockers=("settlement:metadata_factory",),
                    next_action_digest=goal_task_digest("goal-settlement-review"),
                    outcome_digest=outcome_digest,
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("goal metadata settlement failed and could not be checkpointed") from transition_error
            raise
        evaluator_digest = metadata.get("evaluator_digest")
        settled = running
        try:
            if criterion_updates:
                settled = goal_store.update_criteria(
                    running.goal_id,
                    criterion_updates,
                    expected_revision=running.revision,
                )
                if evaluator_digest is None:
                    evaluator_digest = content_digest(
                        {"criteria": [criterion.to_dict() for criterion in settled.criteria]}
                    )
            target_status = goal_status_for_result(
                result_status,
                criteria_complete=settled.required_criteria_complete,
            )
            transition_metadata = {
                key: value
                for key, value in (
                    ("evaluator_digest", evaluator_digest),
                    ("learning_state_digest", metadata.get("learning_state_digest")),
                    ("progress_digest", metadata.get("progress_digest")),
                )
                if value is not None
            }
            updated = goal_store.transition(
                settled.goal_id,
                target_status,
                expected_revision=settled.revision,
                blockers=(() if target_status == "completed" else (f"result:{result_status}",)),
                next_action_digest=(None if target_status == "completed" else goal_task_digest(f"goal-next:{result_status}")),
                outcome_digest=outcome_digest,
                **transition_metadata,
            )
        except AutonomousGoalError as error:
            try:
                goal_store.transition(
                    settled.goal_id,
                    "blocked",
                    expected_revision=settled.revision,
                    blockers=(f"settlement:{type(error).__name__}",),
                    next_action_digest=goal_task_digest("goal-settlement-review"),
                    outcome_digest=outcome_digest,
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("goal lifecycle settlement failed and could not be checkpointed") from transition_error
            raise BrainRunError("goal lifecycle settlement failed") from error
        return {
            "schema": GOAL_STEP_SCHEMA,
            "goal": updated.to_dict(),
            "result": result,
            "result_status": result_status,
            "goal_status": updated.status,
            "outcome_digest": outcome_digest,
            "evaluator_digest": updated.evaluator_digest,
            "learning_state_digest": updated.learning_state_digest,
            "progress_digest": updated.progress_digest,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }

    def run_cross_domain_goal_step(
        self,
        *,
        goal_store: AutonomousGoalLedger,
        goal_id: str,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        goal_criteria: Sequence[AutonomousGoalCriterion | Mapping[str, Any]] = (),
        goal_max_attempts: int = 8,
        criterion_updates: Sequence[Mapping[str, Any]] = (),
        settlement_metadata: Mapping[str, Any] | None = None,
        run_options: Mapping[str, Any] | None = None,
        run_callable: Callable[..., Any] | None = None,
        settlement_metadata_factory: Callable[[Any], Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Run one bounded cross-domain fan-out/fan-in attempt under a durable goal."""

        if not isinstance(goal_store, AutonomousGoalLedger):
            raise BrainRunError("goal_store must be an AutonomousGoalLedger")
        if not isinstance(subtasks, Sequence) or isinstance(subtasks, (str, bytes, bytearray)):
            raise BrainRunError("cross-domain goal subtasks must be a sequence")
        if not isinstance(run_options, Mapping) and run_options is not None:
            raise BrainRunError("run_options must be a mapping or None")
        if not isinstance(settlement_metadata, Mapping) and settlement_metadata is not None:
            raise BrainRunError("settlement_metadata must be a mapping or None")
        metadata = {} if settlement_metadata is None else dict(settlement_metadata)
        unknown_metadata = sorted(set(metadata).difference({"evaluator_digest", "learning_state_digest", "progress_digest"}))
        if unknown_metadata:
            raise BrainRunError("unsupported goal settlement metadata: " + ", ".join(unknown_metadata))
        for metadata_name, metadata_value in metadata.items():
            if metadata_value is not None:
                _route_digest(metadata_value, f"goal settlement {metadata_name}")
        options = {} if run_options is None else dict(run_options)
        if any(name in options for name in ("task", "subtasks", "domain")):
            raise BrainRunError("run_options cannot override cross-domain goal task, subtasks, or domain")
        try:
            task_digest = goal_task_digest(task)
            current = goal_store.get(goal_id)
            capability = options.get("capability")
            risk_class = options.get("risk_class")
            if current is None:
                current = goal_store.create(
                    goal_id=goal_id,
                    task_digest=task_digest,
                    domain="cross_domain",
                    capability=capability,
                    risk_class=risk_class,
                    criteria=goal_criteria,
                    max_attempts=goal_max_attempts,
                )
            else:
                if current.task_digest != task_digest or current.domain != "cross_domain":
                    raise BrainRunError("cross-domain goal identity does not match the requested task")
                if capability is not None and current.capability != capability:
                    raise BrainRunError("goal capability does not match the requested capability")
                if risk_class is not None and current.risk_class != risk_class:
                    raise BrainRunError("goal risk_class does not match the requested risk class")
            if current.status in {"completed", "cancelled"}:
                terminal_outcome_digest = content_digest(
                    {"goal_id": current.goal_id, "attempt": current.attempt, "result_status": "terminal"}
                )
                return {
                    "schema": GOAL_STEP_SCHEMA,
                    "goal": current.to_dict(),
                    "result": None,
                    "result_status": "terminal",
                    "goal_status": current.status,
                    "outcome_digest": terminal_outcome_digest,
                    "evaluator_digest": current.evaluator_digest,
                    "learning_state_digest": current.learning_state_digest,
                    "progress_digest": current.progress_digest,
                    "retention": GOAL_RETENTION,
                    "secret_material": "never_returned",
                }
            if current.status in {"blocked", "failed"}:
                current = goal_store.transition(current.goal_id, "ready", expected_revision=current.revision)
            running = goal_store.transition(current.goal_id, "running", expected_revision=current.revision)
        except AutonomousGoalError as error:
            raise BrainRunError("cross-domain goal lifecycle admission failed") from error

        try:
            if run_callable is not None and not callable(run_callable):
                raise BrainRunError("run_callable must be callable or None")
            result = (
                self.run_cross_domain(task=task, subtasks=subtasks, **options)
                if run_callable is None
                else run_callable(task=task, subtasks=subtasks, **options)
            )
        except Exception as error:
            try:
                exception_status = f"exception:{type(error).__name__}"
                goal_store.transition(
                    running.goal_id,
                    "failed",
                    expected_revision=running.revision,
                    blockers=(exception_status,),
                    next_action_digest=goal_task_digest("goal-retry"),
                    outcome_digest=content_digest(
                        {"goal_id": running.goal_id, "attempt": running.attempt, "result_status": exception_status}
                    ),
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("cross-domain goal failure transition failed") from transition_error
            raise

        result_status = getattr(result, "status", None)
        if (
            not isinstance(result_status, str)
            or not result_status.strip()
            or "\x00" in result_status
            or len(result_status.encode("utf-8")) > 128
        ):
            result_status = "failed"
        else:
            result_status = result_status.strip()
        outcome_digest = content_digest(
            {"goal_id": running.goal_id, "attempt": running.attempt, "result_status": result_status}
        )
        try:
            metadata = _merge_goal_settlement_metadata(metadata, settlement_metadata_factory, result)
        except BrainRunError:
            try:
                goal_store.transition(
                    running.goal_id,
                    "blocked",
                    expected_revision=running.revision,
                    blockers=("settlement:metadata_factory",),
                    next_action_digest=goal_task_digest("goal-settlement-review"),
                    outcome_digest=outcome_digest,
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("cross-domain goal metadata settlement failed and could not be checkpointed") from transition_error
            raise
        settled = running
        evaluator_digest = metadata.get("evaluator_digest")
        progress_digest = metadata.get("progress_digest")
        if progress_digest is None:
            child_statuses = tuple(
                status
                for child in getattr(result, "child_results", ())
                if isinstance(status := getattr(child, "status", None), str)
            )
            progress_digest = content_digest(
                {
                    "result_status": result_status,
                    "child_statuses": child_statuses,
                    "completed_children": getattr(result, "completed_children", None),
                    "total_children": getattr(result, "total_children", None),
                }
            )
        try:
            if criterion_updates:
                settled = goal_store.update_criteria(
                    running.goal_id,
                    criterion_updates,
                    expected_revision=running.revision,
                )
                if evaluator_digest is None:
                    evaluator_digest = content_digest(
                        {"criteria": [criterion.to_dict() for criterion in settled.criteria]}
                    )
            target_status = goal_status_for_result(
                result_status,
                criteria_complete=settled.required_criteria_complete,
            )
            transition_metadata = {
                key: value
                for key, value in (
                    ("evaluator_digest", evaluator_digest),
                    ("learning_state_digest", metadata.get("learning_state_digest")),
                    ("progress_digest", progress_digest),
                )
                if value is not None
            }
            updated = goal_store.transition(
                settled.goal_id,
                target_status,
                expected_revision=settled.revision,
                blockers=(() if target_status == "completed" else (f"result:{result_status}",)),
                next_action_digest=(None if target_status == "completed" else goal_task_digest(f"goal-next:{result_status}")),
                outcome_digest=outcome_digest,
                **transition_metadata,
            )
        except AutonomousGoalError as error:
            try:
                goal_store.transition(
                    settled.goal_id,
                    "blocked",
                    expected_revision=settled.revision,
                    blockers=(f"settlement:{type(error).__name__}",),
                    next_action_digest=goal_task_digest("goal-settlement-review"),
                    outcome_digest=outcome_digest,
                )
            except AutonomousGoalError as transition_error:
                raise BrainRunError("cross-domain goal settlement failed and could not be checkpointed") from transition_error
            raise BrainRunError("cross-domain goal settlement failed") from error
        return {
            "schema": GOAL_STEP_SCHEMA,
            "goal": updated.to_dict(),
            "result": result,
            "result_status": result_status,
            "goal_status": updated.status,
            "outcome_digest": outcome_digest,
            "evaluator_digest": updated.evaluator_digest,
            "learning_state_digest": updated.learning_state_digest,
            "progress_digest": updated.progress_digest,
            "retention": GOAL_RETENTION,
            "secret_material": "never_returned",
        }

    def run_goal_learning_step(
        self,
        *,
        goal_store: AutonomousGoalLedger,
        goal_id: str,
        task: str,
        domain: str,
        bandit_state: Mapping[str, Any],
        memory: BrainEpisodicMemory | None = None,
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        evidence: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        learning_mode: str = "online",
        max_replans: int = 1,
        cycle_id: str | None = None,
        goal_criteria: Sequence[AutonomousGoalCriterion | Mapping[str, Any]] = (),
        goal_max_attempts: int = 8,
        criterion_updates: Sequence[Mapping[str, Any]] = (),
        settlement_metadata: Mapping[str, Any] | None = None,
        run_options: Mapping[str, Any] | None = None,
        memory_tags: Sequence[str] = (),
    ) -> dict[str, Any]:
        """Run one goal attempt through the real online-learning loop.

        ``learning_mode`` is ``online`` for immediate bandit credit or ``replan`` when the caller
        wants bounded evaluator-requested retries. Cross-domain goals additionally expose a
        delayed-credit trajectory mode. The goal ledger receives only digests derived from
        evaluator projections, the next bandit state,
        and stable attempt identities. Model candidates and opaque credentials remain transient
        inputs to the selected runner.
        """

        if learning_mode not in {"online", "replan"}:
            raise BrainRunError("goal learning_mode must be online or replan")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("goal learning bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if not isinstance(run_options, Mapping) and run_options is not None:
            raise BrainRunError("goal learning run_options must be a mapping or None")
        options = {} if run_options is None else dict(run_options)
        if any(name in options for name in ("task", "domain", "learn", "bandit_state")):
            raise BrainRunError("goal learning run_options cannot override task, domain, learn, or bandit_state")
        options["model_candidates"] = options.get("model_candidates", ())
        options["credentials"] = options.get("credentials", {})
        options["bandit_state"] = bandit_state
        options["memory"] = memory if memory is not None else options.get("memory")
        options["ledger"] = ledger if ledger is not None else options.get("ledger")
        options["memory_tags"] = tuple(memory_tags)
        options["learn"] = True
        if evaluator is not None:
            options["evaluator"] = evaluator
        if evaluator_registry is not None:
            options["evaluator_registry"] = evaluator_registry
        if evidence is not None:
            options["evidence"] = evidence
        options["max_replans"] = max_replans
        if cycle_id is not None:
            _identifier("goal learning cycle_id", cycle_id)

        def runner(**runner_options: Any) -> Any:
            return self.run_learning(
                task=runner_options.pop("task"),
                domain=runner_options.pop("domain"),
                **runner_options,
            )

        return self.run_goal_step(
            goal_store=goal_store,
            goal_id=goal_id,
            task=task,
            domain=domain,
            goal_criteria=goal_criteria,
            goal_max_attempts=goal_max_attempts,
            criterion_updates=criterion_updates,
            settlement_metadata=settlement_metadata,
            run_options=options,
            run_callable=runner,
            settlement_metadata_factory=lambda result: _goal_learning_settlement_metadata(result, cycle_id=cycle_id),
        )

    def run_cross_domain_goal_learning_step(
        self,
        *,
        goal_store: AutonomousGoalLedger,
        goal_id: str,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        bandit_state: Mapping[str, Any],
        memory: BrainEpisodicMemory | None = None,
        evaluator: Any = None,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
        learning_mode: str = "online",
        max_replans: int = 1,
        cycle_id: str | None = None,
        goal_criteria: Sequence[AutonomousGoalCriterion | Mapping[str, Any]] = (),
        goal_max_attempts: int = 8,
        criterion_updates: Sequence[Mapping[str, Any]] = (),
        settlement_metadata: Mapping[str, Any] | None = None,
        run_options: Mapping[str, Any] | None = None,
        memory_tags: Sequence[str] = (),
    ) -> dict[str, Any]:
        """Run one cross-domain goal through sequential, trajectory, or replan learning."""

        if learning_mode not in {"online", "trajectory", "replan"}:
            raise BrainRunError("cross-domain goal learning_mode must be online, trajectory, or replan")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain goal learning bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if not isinstance(run_options, Mapping) and run_options is not None:
            raise BrainRunError("cross-domain goal learning run_options must be a mapping or None")
        options = {} if run_options is None else dict(run_options)
        if any(name in options for name in ("task", "subtasks", "bandit_state")):
            raise BrainRunError("cross-domain goal learning run_options cannot override task, subtasks, or bandit_state")
        options["model_candidates"] = options.get("model_candidates", ())
        options["credentials"] = options.get("credentials", {})
        options["bandit_state"] = bandit_state
        options["memory"] = memory if memory is not None else options.get("memory")
        options["ledger"] = ledger if ledger is not None else options.get("ledger")
        options["memory_tags"] = tuple(memory_tags)
        if evaluator is not None:
            options["evaluator"] = evaluator
        if evidence is not None:
            options["evidence"] = evidence
        if learning_mode == "replan":
            options["max_replans"] = max_replans
        if cycle_id is not None:
            _identifier("cross-domain goal learning cycle_id", cycle_id)
            options.setdefault("run_id", cycle_id)
            if learning_mode in {"trajectory", "replan"}:
                options.setdefault("trajectory_id", cycle_id)

        if learning_mode == "online":
            runner_function = self.run_cross_domain_learning
        elif learning_mode == "trajectory":
            runner_function = self.run_cross_domain_trajectory_learning
        else:
            runner_function = self.run_cross_domain_replan_learning

        def runner(**runner_options: Any) -> Any:
            return runner_function(
                task=runner_options.pop("task"),
                subtasks=runner_options.pop("subtasks"),
                **runner_options,
            )

        return self.run_cross_domain_goal_step(
            goal_store=goal_store,
            goal_id=goal_id,
            task=task,
            subtasks=subtasks,
            goal_criteria=goal_criteria,
            goal_max_attempts=goal_max_attempts,
            criterion_updates=criterion_updates,
            settlement_metadata=settlement_metadata,
            run_options=options,
            run_callable=runner,
            settlement_metadata_factory=lambda result: _goal_learning_settlement_metadata(result, cycle_id=cycle_id),
        )

    def run(
        self,
        *,
        task: str,
        domain: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        capability: str | None = None,
        risk_class: str | None = None,
        constraints: Sequence[str] = (),
        desired_outputs: Sequence[str] = (),
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str = "answer",
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        require_response_review: bool = True,
        response_schema: Mapping[str, Any] | None = None,
        execution_mode: str = "provider",
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_plan_accepted: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        required_model_capabilities: Sequence[str] = (),
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
        memory_consolidator: AutonomousMemoryConsolidator | None = None,
        memory_lesson_resolver: Callable[[str], str | None] | None = None,
        memory_lesson_context_resolver: Callable[[Mapping[str, Any]], str | None] | None = None,
        consolidated_memory_limit: int = 8,
        retrieve_consolidated_memory: bool = True,
        consolidated_memory_required: bool = False,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        input_tokens: int = 4_096,
        context_budget: AutonomousContextBudgetOptions | Mapping[str, Any] | None = None,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
        approved_stage_ids: Sequence[str] = (),
        run_id: str | None = None,
        max_output_tokens: int = 2_048,
        temperature: float | None = None,
        idempotency_key: str | None = None,
        mission_policy: MissionPolicy | Mapping[str, Any] | None = None,
        mission_options: Mapping[str, Any] | None = None,
        route_request: Mapping[str, Any] | None = None,
        auto_route: bool = False,
        enforce_route_tools: bool = True,
        require_resolved_route: bool = True,
        provider_tools: Sequence[ProviderTool] = (),
        tool_choice: str | None = None,
        max_provider_failovers: int = 2,
        tool_loop_options: Mapping[str, Any] | None = None,
        execution_controller: AutonomousExecutionController | None = None,
        learn: bool = False,
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        evidence: Mapping[str, Any] | None = None,
        max_replans: int = 1,
        memory_tags: Sequence[str] = (),
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        reserve_cost: AutonomousCostReservationCallback | None = None,
    ) -> Any:
        """Run one domain-aware task through adaptive selection and bounded invocation.

        Pass ``learn=True`` with a bandit state and episodic memory to run the explicit provider
        learning loop. Pass ``mission_policy`` to promote the provider proposal into the existing
        route/mission executor; dispatch still requires ``approve_mission_dispatch=True``.
        """

        if not isinstance(require_response_review, bool):
            raise BrainRunError("require_response_review must be a boolean")
        selection_overrides = _selection_overrides_with_weights(
            selection_overrides,
            selection_weights,
            selection_observations,
        )

        normalized_content_parts = (
            None if content_parts is None else normalize_provider_content_parts(content_parts)
        )

        store, recalled = self._memory(
            self.brain,
            memory,
            memory_query,
            memory_limit,
            task=task,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            authorization_context=authorization_context,
        )
        consolidated_references = self._consolidated_memory(
            memory_consolidator,
            memory_lesson_resolver,
            memory_lesson_context_resolver,
            domains=(domain,),
            capability=capability,
            limit=consolidated_memory_limit,
            retrieve=retrieve_consolidated_memory,
            required=consolidated_memory_required,
            authorization_context=authorization_context,
        )
        blueprint = self.prepare(
            task=task,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            constraints=constraints,
            desired_outputs=desired_outputs,
            context=context,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            execution_mode=execution_mode,
            max_input_tokens=input_tokens,
            required_model_capabilities=required_model_capabilities,
            memory_episodes=recalled,
            memory_lesson_references=consolidated_references,
        )
        route_binding = blueprint.selection_context.get("autonomous_route")
        blueprint = _apply_versioned_prompt(
            blueprint,
            route=route_binding if isinstance(route_binding, Mapping) else None,
            prompt_template=prompt_template,
            prompt_registry=prompt_registry,
            prompt_selection=prompt_selection,
            prompt_stage=prompt_stage,
            prompt_learning_state=prompt_learning_state,
            prompt_learning_exploration=prompt_learning_exploration,
        )
        _assert_task_decision_allows_provider(
            blueprint.task_decision,
            scope="autonomous execution",
        )
        if invocation_observer is not None and not all(
            callable(getattr(invocation_observer, name, None)) for name in ("before", "after")
        ):
            raise BrainRunError("invocation_observer must implement before and after")
        if trace_event_callback is not None and not callable(trace_event_callback):
            raise BrainRunError("trace_event_callback must be callable or None")
        route_context = blueprint.selection_context.get("autonomous_route")
        route_digest = route_context.get("route_digest") if isinstance(route_context, Mapping) else None
        if domain_policy_mode not in AUTONOMOUS_DOMAIN_POLICY_MODES:
            raise AutonomousDomainPolicyError(
                "domain_policy_mode must be one of: " + ", ".join(AUTONOMOUS_DOMAIN_POLICY_MODES)
            )
        if not all(
            value is None or isinstance(value, bool)
            for value in (
                domain_policy_evidence_ready,
                domain_policy_evaluator_configured,
                domain_policy_plan_accepted,
                domain_policy_effects_requested,
                domain_policy_effects_approved,
            )
        ):
            raise AutonomousDomainPolicyError("strict domain policy gate values must be booleans or None")
        strict_policy = autonomous_domain_policy(blueprint.domain_pack.domain)
        policy_admission = None
        if domain_policy_mode == "strict":
            policy_admission = evaluate_autonomous_domain_policy(
                strict_policy,
                route_confidence=(
                    float(route_context.get("confidence", 1.0))
                    if isinstance(route_context, Mapping) and isinstance(route_context.get("confidence", 1.0), (int, float))
                    else 1.0
                ),
                estimated_input_tokens=input_tokens,
                requested_output_tokens=max_output_tokens,
                structured_response=bool(require_json or response_schema),
                evidence_ready=domain_policy_evidence_ready,
                evaluator_configured=(
                    domain_policy_evaluator_configured
                    if domain_policy_evaluator_configured is not None
                    else bool(learn or evaluator or evaluator_registry)
                ),
                plan_accepted=domain_policy_plan_accepted,
                effects_requested=domain_policy_effects_requested,
                effects_approved=(
                    domain_policy_effects_approved
                    if domain_policy_effects_approved is not None
                    else approve_provider_call
                ),
            )
            if policy_admission.decision != "admitted":
                raise AutonomousDomainPolicyError(
                    "strict autonomous domain policy admission failed: "
                    + ", ".join(policy_admission.reasons)
                )
            max_provider_failovers = min(
                max_provider_failovers,
                max(0, strict_policy.max_provider_attempts - 1),
            )
            loop_options = {} if tool_loop_options is None else dict(tool_loop_options)
            loop_options["max_turns"] = min(
                int(loop_options.get("max_turns", strict_policy.max_tool_turns)),
                strict_policy.max_tool_turns,
            )
            tool_loop_options = loop_options
        _emit_trace_event(
            trace_event_callback,
            phase="plan_compiled",
            status="running",
            route_digest=route_digest,
            plan_digest=blueprint.plan.get("plan_digest"),
        )

        def finish_learning_trace(value: Any) -> Any:
            result_status = getattr(value, "status", "unknown")
            detail_digest = content_digest({"status": result_status, "task_digest": blueprint.spec.task_digest})
            _emit_trace_event(
                trace_event_callback,
                phase="evaluation_settled",
                status="running",
                detail_digest=detail_digest,
            )
            _emit_trace_event(
                trace_event_callback,
                phase="learning_prepared",
                status="running",
                detail_digest=detail_digest,
            )
            return value
        if not isinstance(auto_route, bool):
            raise BrainRunError("auto_route must be a boolean")
        approved_stage_ids = _sequence(
            "workflow approved_stage_ids",
            approved_stage_ids,
            maximum=len(blueprint.workflow.stages),
        )
        unknown_approved_stages = sorted(
            set(approved_stage_ids).difference(stage.id for stage in blueprint.workflow.stages)
        )
        if unknown_approved_stages:
            raise BrainRunError(
                "workflow approved_stage_ids contains unknown stages: "
                + ", ".join(unknown_approved_stages)
            )
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise BrainRunError("bandit_state must be a mapping or None")
            BrainLearningLedger._assert_safe(bandit_state)
        effective_execution_mode = (
            "mission" if execution_mode == "provider" and mission_policy is not None else execution_mode
        )
        effective_route_request = route_request
        if auto_route:
            if effective_execution_mode == "provider":
                raise BrainRunError("auto_route requires execution_mode=tool_loop or mission")
            if effective_route_request is None:
                effective_route_request = self.route_request_for(blueprint)
        if learn:
            if mission_policy is not None and execution_mode != "tool_loop":
                if bandit_state is None:
                    raise BrainRunError("bandit_state is required for mission learning")
                if store is None:
                    raise BrainRunError("memory is required for mission learning")
                evaluator_value = evaluator
                if evaluator_value is None:
                    registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
                    evaluator_value = registry.resolve_for_autonomous_domain(
                        blueprint.domain_pack.domain,
                        fallback_domain=blueprint.domain_pack.evaluator_domain,
                    )
                options = {} if mission_options is None else dict(mission_options)
                options.update(
                    {
                        "context": dict(blueprint.selection_context),
                        "content_parts": normalized_content_parts,
                        "contextual_observations": [dict(item) for item in contextual_observations],
                        "required_capabilities": list(blueprint.required_capabilities),
                        "input_tokens": input_tokens,
                        "requested_output_tokens": requested_output_tokens,
                        "max_cost_per_million_tokens": max_cost_per_million_tokens,
                        "max_latency_ms": max_latency_ms,
                        "min_quality": min_quality,
                        "selection_overrides": selection_overrides,
                        "approve_provider_call": approve_provider_call,
                        "approve_mission_dispatch": approve_mission_dispatch,
                        "run_id": run_id,
                        "max_output_tokens": max_output_tokens,
                        "temperature": temperature,
                        "response_schema": response_schema or blueprint.spec.response_schema,
                        "idempotency_key": idempotency_key,
                        "route_request": effective_route_request,
                        "enforce_route_tools": enforce_route_tools,
                        "require_resolved_route": require_resolved_route,
                        "provider_tools": provider_tools,
                        "tool_choice": tool_choice,
                        "max_provider_failovers": max_provider_failovers,
                        "authorization_context": authorization_context,
                        "authorization_domain": blueprint.spec.domain,
                        "reserve_cost": reserve_cost,
                    }
                )
                learning_cycle = self.brain.run_adaptive_mission_learning_cycle(
                    task=blueprint.spec.task,
                    model_candidates=model_candidates,
                    prompt=blueprint.prompt,
                    plan=blueprint.plan,
                    credentials=credentials,
                    mission_policy=mission_policy,
                    evaluator=evaluator_value,
                    bandit_state=bandit_state,
                    ledger=ledger,
                    memory=store,
                    memory_query=memory_query,
                    memory_limit=memory_limit,
                    memory_tags=memory_tags,
                    evidence=evidence,
                    max_replans=max_replans,
                    mission_options=options,
                    execution_controller=execution_controller,
                    invocation_observer=invocation_observer,
                    trace_event_callback=trace_event_callback,
                )
                return finish_learning_trace(
                    self._attach_domain_response_evaluation(blueprint, learning_cycle)
                )
            if bandit_state is None:
                raise BrainRunError("bandit_state is required when learn=True")
            return finish_learning_trace(self._run_learning_from_blueprint(
                blueprint,
                model_candidates=model_candidates,
                credentials=credentials,
                bandit_state=bandit_state,
                store=store,
                evaluator=evaluator,
                evaluator_registry=evaluator_registry,
                evidence=evidence,
                ledger=ledger,
                max_replans=max_replans,
                memory_tags=memory_tags,
                require_response_review=require_response_review,
                execution_kwargs={
                    "contextual_observations": contextual_observations,
                    "content_parts": normalized_content_parts,
                    "input_tokens": input_tokens,
                    "context_budget": context_budget,
                    "requested_output_tokens": requested_output_tokens,
                    "max_cost_per_million_tokens": max_cost_per_million_tokens,
                    "max_latency_ms": max_latency_ms,
                    "min_quality": min_quality,
                    "selection_overrides": selection_overrides,
                    "approve_provider_call": approve_provider_call,
                    "approve_mission_dispatch": approve_mission_dispatch,
                    "run_id": run_id,
                    "max_output_tokens": max_output_tokens,
                    "temperature": temperature,
                    "response_schema": response_schema,
                    "idempotency_key": idempotency_key,
                    "mission_policy": mission_policy if execution_mode == "tool_loop" else None,
                    "mission_options": mission_options,
                    "route_request": effective_route_request,
                    "enforce_route_tools": enforce_route_tools,
                    "require_resolved_route": require_resolved_route,
                    "provider_tools": provider_tools,
                    "tool_choice": tool_choice,
                    "max_provider_failovers": max_provider_failovers,
                    "execution_mode": execution_mode,
                    "tool_loop_options": tool_loop_options,
                    "bandit_state": bandit_state,
                    "reserve_cost": reserve_cost,
                    "execution_controller": execution_controller,
                    "invocation_observer": invocation_observer,
                    "trace_event_callback": trace_event_callback,
                    "authorization_context": authorization_context,
                    },
                ))
        result = self._execute(
            blueprint,
            model_candidates=model_candidates,
            credentials=credentials,
            ledger=ledger,
            content_parts=normalized_content_parts,
            contextual_observations=contextual_observations,
            input_tokens=input_tokens,
            context_budget=context_budget,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            approve_provider_call=approve_provider_call,
            approve_mission_dispatch=approve_mission_dispatch,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            response_schema=response_schema,
            idempotency_key=idempotency_key,
            mission_policy=mission_policy,
            mission_options=mission_options,
            route_request=effective_route_request,
            enforce_route_tools=enforce_route_tools,
            require_resolved_route=require_resolved_route,
            provider_tools=provider_tools,
            tool_choice=tool_choice,
            max_provider_failovers=max_provider_failovers,
            execution_mode=execution_mode,
            tool_loop_options=tool_loop_options,
            bandit_state=bandit_state,
            execution_controller=execution_controller,
            invocation_observer=invocation_observer,
            trace_event_callback=trace_event_callback,
            authorization_context=authorization_context,
            reserve_cost=reserve_cost,
        )
        return self._apply_direct_response_review_gate(
            result,
            require_response_review=require_response_review,
        )

# Keep methods discoverable through the public AutonomousTaskOrchestrator facade.
for _name, _descriptor in vars(AutonomousOrchestratorExecutionMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousTaskOrchestrator.{_name}"
