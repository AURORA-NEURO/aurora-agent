"""Focused autonomous-orchestrator cross domain methods."""

from __future__ import annotations

from .autonomy import (
    Any,
    AutonomousAuthorizationContext,
    AutonomousCostReservationCallback,
    AutonomousCrossDomainLearningResult,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousCrossDomainReplanAttempt,
    AutonomousCrossDomainReplanCheckpoint,
    AutonomousCrossDomainReplanResult,
    AutonomousCrossDomainResult,
    AutonomousCrossDomainTrajectoryLearningResult,
    AutonomousExecutionController,
    AutonomousLearningResult,
    AutonomousMemoryConsolidator,
    AutonomousPromptLearningState,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    AutonomousTaskBlueprint,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainMemoryError,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    BrainToolLoopResult,
    Callable,
    CredentialError,
    CredentialHandle,
    DomainEvaluatorRegistry,
    MAX_AUTONOMOUS_AGENT_PARALLELISM,
    MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS,
    MAX_AUTONOMOUS_EXECUTION_PLAN_BYTES,
    Mapping,
    MemoryQuery,
    MissionPolicy,
    ProviderContentPart,
    ProviderError,
    ProviderInvocationObserver,
    ProviderTool,
    Sequence,
    ThreadPoolExecutor,
    _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY,
    _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
    _assert_task_decision_allows_provider,
    _autonomous_result_digest,
    _cross_domain_execution_digest,
    _cross_domain_plan_digest,
    _cross_domain_replan_context,
    _cross_domain_replan_evaluation_projection,
    _identifier,
    _mapping_sequence,
    _memory_selection_context,
    _memory_task_decision_digest,
    _memory_task_intent_digest,
    _memory_task_lens_digest,
    _record_structured_response_feedback,
    _resolve_cross_domain_evaluator,
    _safe_json,
    _selection_overrides_with_weights,
    _sequence,
    _structured_response_evaluation,
    _text,
    assess_autonomous_cross_domain_response_set,
    content_digest,
    math,
    normalize_provider_content_parts,
    uuid,
)

class AutonomousOrchestratorCrossDomainMixin:
    def run_cross_domain(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str = "answer",
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        execution_plan_context: Mapping[str, Any] | None = None,
        desired_outputs: Sequence[str] = (
            "domain-attributed findings",
            "cross-domain conflicts and uncertainty",
            "safe next actions",
        ),
        child_execution_mode: str = "provider",
        synthesis_execution_mode: str = "provider",
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        response_schema: Mapping[str, Any] | None = None,
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
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
        approve_provider_call: bool = False,
        approve_mission_dispatch: bool = False,
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
        tool_loop_options: Mapping[str, Any] | None = None,
        max_provider_failovers: int = 2,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_plan_accepted: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        synthesize: bool = True,
        allow_partial: bool = False,
        max_parallelism: int = 1,
        bandit_state: Mapping[str, Any] | None = None,
        accepted_plan_refinement: AutonomousCrossDomainPlanRefinementResult | None = None,
        response_alignments: Sequence[Mapping[str, Any]] = (),
        require_response_alignment: bool = False,
        minimum_response_reward: float = 0.8,
        minimum_response_alignment_confidence: float = 0.75,
        response_contradiction_confidence_threshold: float = 0.75,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        reserve_cost: AutonomousCostReservationCallback | None = None,
    ) -> AutonomousCrossDomainResult:
        """Execute bounded domain specialists, then optionally synthesize their outputs.

        Children run in accepted priority order (or declaration order when no refinement is
        accepted) and results are returned in that same order. ``max_parallelism`` optionally
        overlaps independent specialist provider calls; synthesis still waits for every child
        future, and the shared execution controller remains the authority for aggregate steps,
        provider calls, failovers, tools, and cost. A child failure or pending approval prevents
        synthesis unless ``allow_partial`` is explicitly enabled. This method never invents a
        child permission or silently persists provider output into learning memory.
        """

        normalized_content_parts = (
            None if content_parts is None else normalize_provider_content_parts(content_parts)
        )
        selection_overrides = _selection_overrides_with_weights(
            selection_overrides,
            selection_weights,
            selection_observations,
        )
        if not isinstance(synthesize, bool) or not isinstance(allow_partial, bool):
            raise BrainRunError("synthesize and allow_partial must be booleans")
        if not isinstance(require_response_alignment, bool):
            raise BrainRunError("require_response_alignment must be a boolean")
        if (
            not isinstance(max_parallelism, int)
            or isinstance(max_parallelism, bool)
            or not 1 <= max_parallelism <= MAX_AUTONOMOUS_AGENT_PARALLELISM
        ):
            raise BrainRunError(
                "cross-domain max_parallelism must be between 1 and "
                f"{MAX_AUTONOMOUS_AGENT_PARALLELISM}"
            )
        response_alignments = _mapping_sequence(
            "cross-domain response_alignments",
            response_alignments,
            maximum=64,
        )
        for name, value in (
            ("minimum_response_reward", minimum_response_reward),
            ("minimum_response_alignment_confidence", minimum_response_alignment_confidence),
            ("response_contradiction_confidence_threshold", response_contradiction_confidence_threshold),
        ):
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0.0 <= float(value) <= 1.0:
                raise BrainRunError(f"{name} must be finite and within [0, 1]")
        if execution_plan_context is not None:
            if not isinstance(execution_plan_context, Mapping):
                raise BrainRunError("cross-domain execution_plan_context must be a mapping or None")
            _safe_json("cross-domain execution_plan_context", execution_plan_context, maximum=MAX_AUTONOMOUS_EXECUTION_PLAN_BYTES)
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise BrainRunError("cross-domain bandit_state must be a mapping or None")
            BrainLearningLedger._assert_safe(bandit_state)
        blueprint = self.prepare_cross_domain(
            task=task,
            subtasks=subtasks,
            context=context,
            desired_outputs=desired_outputs,
            child_execution_mode=child_execution_mode,
            synthesis_execution_mode=synthesis_execution_mode,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            max_input_tokens=input_tokens,
        )
        for child_id, child in zip(blueprint.child_ids, blueprint.child_blueprints):
            _assert_task_decision_allows_provider(
                child.task_decision,
                scope=f"cross-domain child {child_id}",
            )
        _assert_task_decision_allows_provider(
            blueprint.synthesis_blueprint.task_decision,
            scope="cross-domain synthesis",
        )
        plan_priority, plan_refinement_digest, plan_focus_child_ids = self._accepted_cross_domain_plan(
            blueprint,
            accepted_plan_refinement,
        )
        child_by_id = dict(zip(blueprint.child_ids, blueprint.child_blueprints))
        execution_child_ids = tuple(
            sorted(blueprint.child_ids, key=lambda child_id: plan_priority.get(child_id, len(plan_priority)))
        )

        def execute_child(
            child_id: str,
        ) -> BrainRunResult | BrainToolLoopResult | BrainMissionResult:
            child = child_by_id[child_id]
            child_context = dict(child.spec.context)
            if execution_plan_context is not None:
                child_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = dict(execution_plan_context)
            if plan_refinement_digest is not None:
                child_context["accepted_cross_domain_plan"] = {
                    "refinement_digest": plan_refinement_digest,
                    "priority_rank": plan_priority[child_id],
                    "focus": child_id in plan_focus_child_ids,
                }
            try:
                result = self.run(
                    task=child.spec.task,
                    domain=child.spec.domain,
                    model_candidates=model_candidates,
                    credentials=credentials,
                    capability=child.spec.capability,
                    risk_class=child.spec.risk_class,
                    constraints=child.spec.constraints,
                    desired_outputs=child.spec.desired_outputs,
                    context=child_context,
                    content_parts=normalized_content_parts,
                    prompt_template=prompt_template,
                    prompt_registry=prompt_registry,
                    prompt_selection=prompt_selection,
                    prompt_stage=prompt_stage,
                    prompt_learning_state=prompt_learning_state,
                    prompt_learning_exploration=prompt_learning_exploration,
                    max_steps=child.spec.max_steps,
                    require_json=child.spec.require_json,
                    structured_domain_response=child.spec.structured_domain_response,
                    require_response_review=False,
                    # ``prepare`` stores the generated contract schema in the spec for replay, but
                    # the structured mode owns that schema at the run boundary. Passing it back as
                    # a custom schema would incorrectly trip the mutually-exclusive option guard.
                    response_schema=None if child.spec.structured_domain_response else child.spec.response_schema,
                    execution_mode=child.spec.execution_mode,
                    required_model_capabilities=tuple(
                        capability
                        for capability in child.required_capabilities
                        if capability not in child.profile.required_model_capabilities
                    ),
                    ledger=ledger,
                    memory=memory,
                    memory_query=memory_query,
                    memory_limit=memory_limit,
                    memory_consolidator=memory_consolidator,
                    memory_lesson_resolver=memory_lesson_resolver,
                    memory_lesson_context_resolver=memory_lesson_context_resolver,
                    consolidated_memory_limit=consolidated_memory_limit,
                    retrieve_consolidated_memory=retrieve_consolidated_memory,
                    consolidated_memory_required=consolidated_memory_required,
                    contextual_observations=contextual_observations,
                    input_tokens=input_tokens,
                    requested_output_tokens=requested_output_tokens,
                    max_cost_per_million_tokens=max_cost_per_million_tokens,
                    max_latency_ms=max_latency_ms,
                    min_quality=min_quality,
                    selection_overrides=selection_overrides,
                    bandit_state=bandit_state,
                    approve_provider_call=approve_provider_call,
                    approve_mission_dispatch=approve_mission_dispatch,
                    run_id=self._cross_domain_identity("cross-child", run_id, child_id),
                    max_output_tokens=max_output_tokens,
                    temperature=temperature,
                    idempotency_key=self._cross_domain_identity("cross-key", idempotency_key, child_id),
                    mission_policy=mission_policy,
                    mission_options=mission_options,
                    route_request=route_request,
                    auto_route=auto_route,
                    enforce_route_tools=enforce_route_tools,
                    require_resolved_route=require_resolved_route,
                    provider_tools=provider_tools,
                    tool_choice=tool_choice,
                    max_provider_failovers=max_provider_failovers,
                    domain_policy_mode=domain_policy_mode,
                    domain_policy_evidence_ready=domain_policy_evidence_ready,
                    domain_policy_evaluator_configured=domain_policy_evaluator_configured,
                    domain_policy_plan_accepted=domain_policy_plan_accepted,
                    domain_policy_effects_requested=domain_policy_effects_requested,
                    domain_policy_effects_approved=domain_policy_effects_approved,
                    tool_loop_options=self._cross_domain_tool_loop_options(
                        tool_loop_options,
                        execution_id=self._cross_domain_identity("cross-tool", run_id, child_id),
                        domain=child.spec.domain,
                    ),
                    execution_controller=execution_controller,
                    invocation_observer=invocation_observer,
                    trace_event_callback=trace_event_callback,
                    authorization_context=authorization_context,
                    reserve_cost=reserve_cost,
                )
            except (ProviderError, CredentialError) as error:
                result = self._provider_failure_result(
                    child,
                    child_id=child_id,
                    error=error,
                    run_id=self._cross_domain_identity("cross-child", run_id, child_id),
                )
            if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                raise BrainRunError("cross-domain child returned an unsupported result")
            return result

        if max_parallelism == 1 or len(execution_child_ids) == 1:
            child_results = [execute_child(child_id) for child_id in execution_child_ids]
        else:
            # Submit in accepted order and collect in that order. Worker completion can vary, but
            # the public result and synthesis input remain deterministic. No evaluator or
            # learning update runs in this concurrent path; those APIs intentionally preserve
            # ordered delayed-credit semantics.
            with ThreadPoolExecutor(
                max_workers=min(max_parallelism, len(execution_child_ids)),
                thread_name_prefix="aurora-cross-domain",
            ) as pool:
                futures = [pool.submit(execute_child, child_id) for child_id in execution_child_ids]
                child_results = [future.result() for future in futures]

        complete = [result.status.startswith("completed") for result in child_results]
        response_assessment = None
        if structured_domain_response:
            entries = self._cross_domain_response_entries(
                blueprint,
                execution_child_ids,
                child_results,
            )
            if entries is not None:
                response_assessment = assess_autonomous_cross_domain_response_set(
                    entries,
                    requested_domains=tuple(child.profile.domain for child in blueprint.child_blueprints),
                    context_digest=blueprint.task_digest,
                    alignments=response_alignments,
                    require_synthesis=False,
                    require_complete_alignment=require_response_alignment,
                    minimum_reward=float(minimum_response_reward),
                    minimum_alignment_confidence=float(minimum_response_alignment_confidence),
                    contradiction_confidence_threshold=float(response_contradiction_confidence_threshold),
                )
        # Partial fan-in is useful only when at least one specialist produced usable evidence.
        # Never spend another provider call synthesizing an empty or entirely blocked fan-out.
        # This keeps provider/credential failures from becoming a successful-looking conclusion.
        if not all(complete) and (not allow_partial or (synthesize and not any(complete))):
            status = (
                "reconciliation_required"
                if any(result.status == "reconciliation_required" for result in child_results)
                else "approval_required"
                if any(result.status == "approval_required" for result in child_results)
                else "child_failed"
                if any(result.status == "provider_failed" for result in child_results)
                else "child_incomplete"
            )
            return AutonomousCrossDomainResult(
                status,
                blueprint,
                tuple(child_results),
                None,
                plan_refinement_digest,
                execution_child_ids,
                response_assessment,
                max_parallelism=max_parallelism,
            )
        if synthesize and response_assessment is not None and not response_assessment.ready_to_synthesize:
            return AutonomousCrossDomainResult(
                "response_review_required",
                blueprint,
                tuple(child_results),
                None,
                plan_refinement_digest,
                execution_child_ids,
                response_assessment,
                max_parallelism=max_parallelism,
            )
        if not synthesize:
            return AutonomousCrossDomainResult(
                "children_completed" if all(complete) else "children_partial",
                blueprint,
                tuple(child_results),
                None,
                plan_refinement_digest,
                execution_child_ids,
                response_assessment,
                max_parallelism=max_parallelism,
            )
        child_outputs = [
            {
                "id": child_id,
                "domain": child.profile.domain,
                "workflow_id": child.workflow.workflow_id,
                "workflow_digest": child.workflow.workflow_digest,
                "status": result.status,
                "output": self._cross_domain_output(result),
                "output_digest": content_digest({"output": self._cross_domain_output(result)}),
            }
            for child_id, result in zip(execution_child_ids, child_results)
            for child in (child_by_id[child_id],)
        ]
        synthesis_context = dict(blueprint.synthesis_blueprint.spec.context)
        synthesis_context["child_outputs"] = child_outputs
        if execution_plan_context is not None:
            synthesis_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = dict(execution_plan_context)
        if plan_refinement_digest is not None:
            synthesis_context["accepted_cross_domain_plan"] = {
                "refinement_digest": plan_refinement_digest,
                "priority_child_ids": list(execution_child_ids),
                "focus_child_ids": list(plan_focus_child_ids),
            }
        synthesis = blueprint.synthesis_blueprint
        synthesis_result = self._provider_failure_call(
            synthesis,
            child_id="synthesis",
            run_id=self._cross_domain_identity("cross-synthesis", run_id, "synthesis"),
            invoke=lambda: self.run(
            task=synthesis.spec.task,
            domain=synthesis.spec.domain,
            model_candidates=model_candidates,
            credentials=credentials,
            capability=synthesis.spec.capability,
            risk_class=synthesis.spec.risk_class,
            constraints=synthesis.spec.constraints,
            desired_outputs=synthesis.spec.desired_outputs,
            context=synthesis_context,
            content_parts=normalized_content_parts,
            prompt_template=prompt_template,
            prompt_registry=prompt_registry,
            prompt_selection=prompt_selection,
            prompt_stage=prompt_stage,
            prompt_learning_state=prompt_learning_state,
            prompt_learning_exploration=prompt_learning_exploration,
            max_steps=synthesis.spec.max_steps,
            require_json=synthesis.spec.require_json,
            structured_domain_response=synthesis.spec.structured_domain_response,
            require_response_review=False,
            response_schema=None if synthesis.spec.structured_domain_response else synthesis.spec.response_schema,
            execution_mode=synthesis.spec.execution_mode,
            ledger=ledger,
            memory=memory,
            memory_query=memory_query,
            memory_limit=memory_limit,
            memory_consolidator=memory_consolidator,
            memory_lesson_resolver=memory_lesson_resolver,
            memory_lesson_context_resolver=memory_lesson_context_resolver,
            consolidated_memory_limit=consolidated_memory_limit,
            retrieve_consolidated_memory=retrieve_consolidated_memory,
            consolidated_memory_required=consolidated_memory_required,
            contextual_observations=contextual_observations,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            bandit_state=bandit_state,
            approve_provider_call=approve_provider_call,
            approve_mission_dispatch=approve_mission_dispatch,
            run_id=self._cross_domain_identity("cross-synthesis", run_id, "synthesis"),
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            idempotency_key=self._cross_domain_identity("cross-key", idempotency_key, "synthesis"),
            mission_policy=mission_policy,
            mission_options=mission_options,
            route_request=route_request,
            auto_route=auto_route,
            enforce_route_tools=enforce_route_tools,
            require_resolved_route=require_resolved_route,
            provider_tools=provider_tools,
            tool_choice=tool_choice,
            max_provider_failovers=max_provider_failovers,
            domain_policy_mode=domain_policy_mode,
            domain_policy_evidence_ready=domain_policy_evidence_ready,
            domain_policy_evaluator_configured=domain_policy_evaluator_configured,
            domain_policy_plan_accepted=domain_policy_plan_accepted,
            domain_policy_effects_requested=domain_policy_effects_requested,
            domain_policy_effects_approved=domain_policy_effects_approved,
            tool_loop_options=self._cross_domain_tool_loop_options(
                tool_loop_options,
                execution_id=self._cross_domain_identity("cross-tool", run_id, "synthesis"),
                domain=synthesis.spec.domain,
            ),
            execution_controller=execution_controller,
            invocation_observer=invocation_observer,
            trace_event_callback=trace_event_callback,
            authorization_context=authorization_context,
            reserve_cost=reserve_cost,
            ),
        )
        if not isinstance(synthesis_result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
            raise BrainRunError("cross-domain synthesis returned an unsupported result")
        if structured_domain_response and synthesis_result.status.startswith("completed"):
            entries = self._cross_domain_response_entries(
                blueprint,
                execution_child_ids,
                child_results,
                synthesis_result=synthesis_result,
            )
            if entries is not None:
                response_assessment = assess_autonomous_cross_domain_response_set(
                    entries,
                    requested_domains=tuple(child.profile.domain for child in blueprint.child_blueprints),
                    context_digest=blueprint.task_digest,
                    alignments=response_alignments,
                    require_synthesis=True,
                    require_complete_alignment=require_response_alignment,
                    minimum_reward=float(minimum_response_reward),
                    minimum_alignment_confidence=float(minimum_response_alignment_confidence),
                    contradiction_confidence_threshold=float(response_contradiction_confidence_threshold),
                )
        result_status = "completed" if synthesis_result.status.startswith("completed") else synthesis_result.status
        if response_assessment is not None and response_assessment.status != "completed" and synthesis_result.status.startswith("completed"):
            result_status = "response_review_required"
        return AutonomousCrossDomainResult(
            result_status,
            blueprint,
            tuple(child_results),
            synthesis_result,
            plan_refinement_digest,
            execution_child_ids,
            response_assessment,
            max_parallelism=max_parallelism,
        )

    def run_cross_domain_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_tags: Sequence[str] = (),
        ledger: BrainLearningLedger | None = None,
        accepted_plan_refinement: AutonomousCrossDomainPlanRefinementResult | None = None,
        **kwargs: Any,
    ) -> AutonomousCrossDomainLearningResult:
        """Run specialists and synthesis with sequential online learning updates.

        ``evidence`` is keyed by child id and optionally ``"synthesis"``. Each completed result
        is scored before the next selection, so this path is genuinely adaptive rather than a
        batch of rewards written after all routing decisions have already happened.
        """

        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain learning bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        memory_store = memory if memory is not None else self.brain.memory
        if memory_store is None:
            raise BrainRunError("memory is required for cross-domain online learning")
        if not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("cross-domain learning memory must be a BrainEpisodicMemory")
        authorization_context = kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("cross-domain learning authorization_context must be an AutonomousAuthorizationContext or None")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("cross-domain evaluator must be a BrainOutcomeEvaluator or None")
        if evaluator_registry is not None and not isinstance(evaluator_registry, DomainEvaluatorRegistry):
            raise BrainRunError("cross-domain evaluator_registry must be a DomainEvaluatorRegistry or None")
        if evidence is not None:
            if not isinstance(evidence, Mapping) or any(
                not isinstance(key, str) or not isinstance(value, Mapping)
                for key, value in evidence.items()
            ):
                raise BrainRunError("cross-domain evidence must map ids to mappings")
            _safe_json("cross-domain learning evidence", evidence, maximum=1_000_000)
        normalized_tags = _sequence("cross-domain learning memory_tags", memory_tags, maximum=32)

        def take(name: str, default: Any) -> Any:
            return kwargs.pop(name, default)

        context = take("context", None)
        content_parts = take("content_parts", None)
        normalized_content_parts = (
            None if content_parts is None else normalize_provider_content_parts(content_parts)
        )
        desired_outputs = take(
            "desired_outputs",
            ("domain-attributed findings", "cross-domain conflicts and uncertainty", "safe next actions"),
        )
        child_execution_mode = take("child_execution_mode", "provider")
        synthesis_execution_mode = take("synthesis_execution_mode", "provider")
        max_steps = take("max_steps", 8)
        require_json = take("require_json", False)
        structured_domain_response = take("structured_domain_response", False)
        response_schema = take("response_schema", None)
        memory_query = take("memory_query", None)
        memory_limit = take("memory_limit", 8)
        contextual_observations = take("contextual_observations", ())
        input_tokens = take("input_tokens", 4_096)
        requested_output_tokens = take("requested_output_tokens", 2_048)
        max_cost_per_million_tokens = take("max_cost_per_million_tokens", None)
        max_latency_ms = take("max_latency_ms", None)
        min_quality = take("min_quality", None)
        selection_overrides = take("selection_overrides", None)
        approve_provider_call = take("approve_provider_call", False)
        approve_mission_dispatch = take("approve_mission_dispatch", False)
        run_id = take("run_id", None)
        max_output_tokens = take("max_output_tokens", 2_048)
        temperature = take("temperature", None)
        idempotency_key = take("idempotency_key", None)
        mission_policy = take("mission_policy", None)
        mission_options = take("mission_options", None)
        route_request = take("route_request", None)
        auto_route = take("auto_route", False)
        enforce_route_tools = take("enforce_route_tools", True)
        require_resolved_route = take("require_resolved_route", True)
        provider_tools = take("provider_tools", ())
        tool_choice = take("tool_choice", None)
        tool_loop_options = take("tool_loop_options", None)
        max_provider_failovers = take("max_provider_failovers", 2)
        domain_policy_mode = take("domain_policy_mode", "audit")
        domain_policy_evidence_ready = take("domain_policy_evidence_ready", None)
        domain_policy_evaluator_configured = take("domain_policy_evaluator_configured", None)
        domain_policy_plan_accepted = take("domain_policy_plan_accepted", None)
        domain_policy_effects_requested = take("domain_policy_effects_requested", None)
        domain_policy_effects_approved = take("domain_policy_effects_approved", None)
        synthesize = take("synthesize", True)
        allow_partial = take("allow_partial", False)
        execution_plan_context = take("execution_plan_context", None)
        execution_controller = take("execution_controller", None)
        authorization_context = take("authorization_context", authorization_context)
        if kwargs:
            raise BrainRunError(
                "unsupported cross-domain learning options: " + ", ".join(sorted(kwargs))
            )
        if not isinstance(synthesize, bool) or not isinstance(allow_partial, bool):
            raise BrainRunError("cross-domain learning synthesize and allow_partial must be booleans")
        if execution_plan_context is not None:
            if not isinstance(execution_plan_context, Mapping):
                raise BrainRunError("cross-domain learning execution_plan_context must be a mapping or None")
            _safe_json("cross-domain learning execution_plan_context", execution_plan_context, maximum=MAX_AUTONOMOUS_EXECUTION_PLAN_BYTES)

        blueprint = self.prepare_cross_domain(
            task=task,
            subtasks=subtasks,
            context=context,
            desired_outputs=desired_outputs,
            child_execution_mode=child_execution_mode,
            synthesis_execution_mode=synthesis_execution_mode,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            max_input_tokens=input_tokens,
        )
        plan_priority, plan_refinement_digest, plan_focus_child_ids = self._accepted_cross_domain_plan(
            blueprint,
            accepted_plan_refinement,
        )
        child_by_id = dict(zip(blueprint.child_ids, blueprint.child_blueprints))
        execution_child_ids = tuple(
            sorted(blueprint.child_ids, key=lambda child_id: plan_priority.get(child_id, len(plan_priority)))
        )
        state: Mapping[str, Any] = dict(bandit_state)
        child_results: list[BrainRunResult | BrainToolLoopResult | BrainMissionResult] = []
        evaluations: list[Mapping[str, Any]] = []
        memory_receipts: list[Mapping[str, Any]] = []
        registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()

        def evaluator_for(domain: str, fallback_domain: str) -> BrainOutcomeEvaluator:
            selected = evaluator or registry.resolve_for_autonomous_domain(
                domain,
                fallback_domain=fallback_domain,
            )
            if not isinstance(selected, BrainOutcomeEvaluator):
                raise BrainRunError("cross-domain evaluator registry returned an invalid evaluator")
            return selected

        def evaluate_result(
            *,
            scope: str,
            item_id: str,
            blueprint_item: AutonomousTaskBlueprint,
            result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
            item_evidence: Mapping[str, Any] | None,
        ) -> None:
            nonlocal state
            if not result.status.startswith("completed"):
                return
            resolved = evaluator_for(
                blueprint_item.domain_pack.domain,
                blueprint_item.domain_pack.evaluator_domain,
            )
            decision, report = resolved.evaluate_and_record_with_decision(
                self.brain,
                result,
                bandit_state=state,
                evidence=item_evidence,
                ledger=ledger,
            )
            brain_result = result if isinstance(result, BrainRunResult) else result.brain_run
            next_state = report.get("next_state")
            if isinstance(next_state, Mapping):
                state = dict(next_state)
            structured_feedback = _record_structured_response_feedback(
                self.brain,
                result,
                bandit_state=state,
                ledger=ledger,
            )
            if structured_feedback is not None:
                state, structured_evaluation_record = structured_feedback
                evaluations.append(structured_evaluation_record)
            episode_id = f"cross-{scope}-{item_id}-{brain_result.run_id}"
            if len(episode_id.encode("utf-8")) > 256:
                episode_id = "cross-episode-" + content_digest(
                    {"scope": scope, "item_id": item_id, "run_id": brain_result.run_id}
                )
            receipt = self.brain.remember_result(
                result,
                task=blueprint_item.spec.task,
                episode_id=episode_id,
                context=_memory_selection_context(blueprint_item),
                tags=[
                    *normalized_tags,
                    f"domain:{blueprint_item.spec.domain}",
                    f"cross_domain:{scope}",
                    f"item:{item_id}",
                ],
                lesson=decision.replan_instruction if decision.replan_requested else None,
                provenance={
                    "scope": scope,
                    "item_id": item_id,
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "task_lens_digest": _memory_task_lens_digest(blueprint_item),
                    "task_intent_digest": _memory_task_intent_digest(blueprint_item),
                    "task_decision_digest": _memory_task_decision_digest(blueprint_item),
                },
                memory=memory_store,
                authorization_context=authorization_context,
                authorization_domain=blueprint_item.spec.domain,
                authorization_capability=blueprint_item.spec.capability,
                authorization_risk_class=blueprint_item.spec.risk_class,
            )
            decision_digest = content_digest(decision.to_dict())
            self._authorize_memory_evaluation(
                authorization_context,
                domain=blueprint_item.spec.domain,
                episode_id=episode_id,
                decision_digest=decision_digest,
            )
            try:
                evaluation_receipt = memory_store.record_evaluation(
                    episode_id,
                    {**decision.to_dict(), "decision_digest": decision_digest},
                ).to_dict()
            except BrainMemoryError as error:
                raise BrainRunError("cross-domain evaluation memory record failed") from error
            memory_receipts.extend((receipt, evaluation_receipt))
            evaluations.append(
                {
                    "scope": scope,
                    "item_id": item_id,
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": report.get("status"),
                        "next_state": report.get("next_state"),
                        "learning_evidence": report.get("learning_evidence"),
                    },
                }
            )

        for child_id in execution_child_ids:
            child = child_by_id[child_id]
            child_context = dict(child.spec.context)
            if execution_plan_context is not None:
                child_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = dict(execution_plan_context)
            if plan_refinement_digest is not None:
                child_context["accepted_cross_domain_plan"] = {
                    "refinement_digest": plan_refinement_digest,
                    "priority_rank": plan_priority[child_id],
                    "focus": child_id in plan_focus_child_ids,
                }
            result = self._provider_failure_call(
                child,
                child_id=child_id,
                run_id=self._cross_domain_identity("cross-child", run_id, child_id),
                invoke=lambda: self.run(
                task=child.spec.task,
                domain=child.spec.domain,
                model_candidates=model_candidates,
                credentials=credentials,
                capability=child.spec.capability,
                risk_class=child.spec.risk_class,
                constraints=child.spec.constraints,
                desired_outputs=child.spec.desired_outputs,
                context=child_context,
                content_parts=normalized_content_parts,
                max_steps=child.spec.max_steps,
                require_json=child.spec.require_json,
                structured_domain_response=child.spec.structured_domain_response,
                require_response_review=False,
                response_schema=None if child.spec.structured_domain_response else child.spec.response_schema,
                execution_mode=child.spec.execution_mode,
                required_model_capabilities=tuple(
                    capability
                    for capability in child.required_capabilities
                    if capability not in child.profile.required_model_capabilities
                ),
                ledger=ledger,
                memory=memory_store,
                memory_query=memory_query,
                memory_limit=memory_limit,
                contextual_observations=contextual_observations,
                input_tokens=input_tokens,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                approve_provider_call=approve_provider_call,
                approve_mission_dispatch=approve_mission_dispatch,
                run_id=self._cross_domain_identity("cross-child", run_id, child_id),
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                idempotency_key=self._cross_domain_identity("cross-key", idempotency_key, child_id),
                mission_policy=mission_policy,
                mission_options=mission_options,
                route_request=route_request,
                auto_route=auto_route,
                enforce_route_tools=enforce_route_tools,
                require_resolved_route=require_resolved_route,
                 provider_tools=provider_tools,
                 tool_choice=tool_choice,
                 max_provider_failovers=max_provider_failovers,
                 domain_policy_mode=domain_policy_mode,
                 domain_policy_evidence_ready=domain_policy_evidence_ready,
                 domain_policy_evaluator_configured=domain_policy_evaluator_configured,
                 domain_policy_plan_accepted=domain_policy_plan_accepted,
                 domain_policy_effects_requested=domain_policy_effects_requested,
                 domain_policy_effects_approved=domain_policy_effects_approved,
                 tool_loop_options=self._cross_domain_tool_loop_options(
                    tool_loop_options,
                    execution_id=self._cross_domain_identity("cross-tool", run_id, child_id),
                    domain=child.spec.domain,
                ),
                bandit_state=state,
                execution_controller=execution_controller,
                authorization_context=authorization_context,
                ),
            )
            if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                raise BrainRunError("cross-domain learning child returned an unsupported result")
            child_results.append(result)
            item_evidence = None if evidence is None else evidence.get(child_id)
            evaluate_result(
                scope="child",
                item_id=child_id,
                blueprint_item=child,
                result=result,
                item_evidence=item_evidence,
            )
            # A strict learning fan-out must not spend provider/evaluator budget on later
            # specialists after a refusal or provider-boundary failure.  The caller can opt into
            # sibling continuation with allow_partial=True; already-settled results remain in the
            # ordered envelope either way.
            if not allow_partial and not result.status.startswith("completed"):
                break

        complete = [result.status.startswith("completed") for result in child_results]
        if not all(complete) and (not allow_partial or (synthesize and not any(complete))):
            status = (
                "approval_required"
                if any(result.status == "approval_required" for result in child_results)
                else "child_failed"
                if any(result.status == "provider_failed" for result in child_results)
                else "child_incomplete"
            )
            cross_domain = AutonomousCrossDomainResult(
                status,
                blueprint,
                tuple(child_results),
                None,
                plan_refinement_digest,
                execution_child_ids[: len(child_results)],
            )
            return AutonomousCrossDomainLearningResult(status, cross_domain, tuple(evaluations), state, tuple(memory_receipts))
        if not synthesize:
            status = "children_completed" if all(complete) else "children_partial"
            cross_domain = AutonomousCrossDomainResult(
                status,
                blueprint,
                tuple(child_results),
                None,
                plan_refinement_digest,
                execution_child_ids,
            )
            return AutonomousCrossDomainLearningResult(status, cross_domain, tuple(evaluations), state, tuple(memory_receipts))

        child_outputs = [
            {
                "id": child_id,
                "domain": child.profile.domain,
                "workflow_id": child.workflow.workflow_id,
                "workflow_digest": child.workflow.workflow_digest,
                "status": result.status,
                "output": self._cross_domain_output(result),
                "output_digest": content_digest({"output": self._cross_domain_output(result)}),
            }
            for child_id, result in zip(execution_child_ids, child_results)
            for child in (child_by_id[child_id],)
        ]
        synthesis = blueprint.synthesis_blueprint
        synthesis_context = dict(synthesis.spec.context)
        synthesis_context["child_outputs"] = child_outputs
        if execution_plan_context is not None:
            synthesis_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = dict(execution_plan_context)
        if plan_refinement_digest is not None:
            synthesis_context["accepted_cross_domain_plan"] = {
                "refinement_digest": plan_refinement_digest,
                "priority_child_ids": list(execution_child_ids),
                "focus_child_ids": list(plan_focus_child_ids),
            }
        synthesis_result = self._provider_failure_call(
            synthesis,
            child_id="synthesis",
            run_id=self._cross_domain_identity("cross-synthesis", run_id, "synthesis"),
            invoke=lambda: self.run(
            task=synthesis.spec.task,
            domain=synthesis.spec.domain,
            model_candidates=model_candidates,
            credentials=credentials,
            capability=synthesis.spec.capability,
            risk_class=synthesis.spec.risk_class,
            constraints=synthesis.spec.constraints,
            desired_outputs=synthesis.spec.desired_outputs,
            context=synthesis_context,
            content_parts=normalized_content_parts,
            max_steps=synthesis.spec.max_steps,
            require_json=synthesis.spec.require_json,
            structured_domain_response=synthesis.spec.structured_domain_response,
            require_response_review=False,
            response_schema=None if synthesis.spec.structured_domain_response else synthesis.spec.response_schema,
            execution_mode=synthesis.spec.execution_mode,
            ledger=ledger,
            memory=memory_store,
            memory_query=memory_query,
            memory_limit=memory_limit,
            contextual_observations=contextual_observations,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            approve_provider_call=approve_provider_call,
            approve_mission_dispatch=approve_mission_dispatch,
            run_id=self._cross_domain_identity("cross-synthesis", run_id, "synthesis"),
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            idempotency_key=self._cross_domain_identity("cross-key", idempotency_key, "synthesis"),
            mission_policy=mission_policy,
            mission_options=mission_options,
            route_request=route_request,
            auto_route=auto_route,
            enforce_route_tools=enforce_route_tools,
            require_resolved_route=require_resolved_route,
             provider_tools=provider_tools,
             tool_choice=tool_choice,
             max_provider_failovers=max_provider_failovers,
             domain_policy_mode=domain_policy_mode,
             domain_policy_evidence_ready=domain_policy_evidence_ready,
             domain_policy_evaluator_configured=domain_policy_evaluator_configured,
             domain_policy_plan_accepted=domain_policy_plan_accepted,
             domain_policy_effects_requested=domain_policy_effects_requested,
             domain_policy_effects_approved=domain_policy_effects_approved,
             tool_loop_options=self._cross_domain_tool_loop_options(
                tool_loop_options,
                execution_id=self._cross_domain_identity("cross-tool", run_id, "synthesis"),
                domain=synthesis.spec.domain,
            ),
            bandit_state=state,
            execution_controller=execution_controller,
            authorization_context=authorization_context,
            ),
        )
        if not isinstance(synthesis_result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
            raise BrainRunError("cross-domain learning synthesis returned an unsupported result")
        evaluate_result(
            scope="synthesis",
            item_id="synthesis",
            blueprint_item=synthesis,
            result=synthesis_result,
            item_evidence=None if evidence is None else evidence.get("synthesis"),
        )
        status = "completed" if synthesis_result.status.startswith("completed") else synthesis_result.status
        cross_domain = AutonomousCrossDomainResult(
            status,
            blueprint,
            tuple(child_results),
            synthesis_result,
            plan_refinement_digest,
            execution_child_ids,
        )
        return AutonomousCrossDomainLearningResult(status, cross_domain, tuple(evaluations), state, tuple(memory_receipts))

    def settle_cross_domain_trajectory_learning(
        self,
        *,
        cross_domain: AutonomousCrossDomainResult,
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_tags: Sequence[str] = (),
        trajectory_id: str | None = None,
        trajectory_discount: float = 0.90,
        trajectory_terminal_reward: float | None = None,
        retain_replan_instruction: bool = True,
        ledger: BrainLearningLedger | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> AutonomousCrossDomainTrajectoryLearningResult:
        """Settle delayed credit for already-executed caller-owned cross-domain results.

        This is the durable-job handoff: a caller can collect one raw result from each worker
        lease, assemble the verified ``AutonomousCrossDomainResult``, and apply one evaluator
        trajectory without re-invoking any provider.
        """

        if not isinstance(cross_domain, AutonomousCrossDomainResult):
            raise BrainRunError("cross-domain trajectory settlement requires an execution result")
        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        evaluator = _resolve_cross_domain_evaluator(
            evaluator,
            [
                *[child.profile.domain for child in cross_domain.blueprint.child_blueprints],
                cross_domain.blueprint.synthesis_blueprint.profile.domain,
            ],
        )
        if not isinstance(retain_replan_instruction, bool):
            raise BrainRunError("retain_replan_instruction must be a boolean")
        if evidence is not None:
            if not isinstance(evidence, Mapping) or any(
                not isinstance(key, str) or not isinstance(value, Mapping)
                for key, value in evidence.items()
            ):
                raise BrainRunError("cross-domain trajectory evidence must map ids to mappings")
            _safe_json("cross-domain trajectory evidence", evidence, maximum=1_000_000)
        memory_store = memory if memory is not None else self.brain.memory
        if not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("cross-domain trajectory memory must be a BrainEpisodicMemory")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("cross-domain trajectory authorization_context must be an AutonomousAuthorizationContext or None")
        normalized_tags = _sequence("cross-domain trajectory memory_tags", memory_tags, maximum=32)
        items = self._cross_domain_trajectory_items(
            cross_domain,
            allow_partial=True,
        )
        results = [item[3] for item in items]
        evidence_packets = [None if evidence is None else evidence.get(item[1]) for item in items]
        trajectory = self.brain.prepare_learning_trajectory(
            results,
            evidence_by_step=evidence_packets,
            trajectory_id=trajectory_id or f"cross-domain-{content_digest({'task_digest': cross_domain.blueprint.task_digest, 'runs': [_autonomous_result_digest(result) for result in results]})}",
            discount=trajectory_discount,
            terminal_reward=trajectory_terminal_reward,
            ledger=ledger,
        )
        trajectory_result = evaluator.evaluate_trajectory(
            self.brain,
            trajectory,
            bandit_state=bandit_state,
            evidence_by_step=evidence_packets,
            ledger=ledger,
        )
        evaluations: list[Mapping[str, Any]] = []
        memory_receipts: list[Mapping[str, Any]] = []
        for index, ((scope, item_id, blueprint_item, result), decision, recording) in enumerate(
            zip(items, trajectory_result.decisions, trajectory_result.recordings)
        ):
            episode_id = trajectory.episodes[index].episode_id
            lesson = decision.replan_instruction if decision.replan_requested and retain_replan_instruction else None
            episode_receipt = self.brain.remember_result(
                result,
                task=blueprint_item.spec.task,
                episode_id=episode_id,
                context=_memory_selection_context(blueprint_item),
                tags=[
                    *normalized_tags,
                    f"domain:{blueprint_item.spec.domain}",
                    f"cross_domain:{scope}",
                    f"item:{item_id}",
                    "learning:trajectory",
                ],
                lesson=lesson,
                provenance={
                    "scope": scope,
                    "item_id": item_id,
                    "trajectory_id": trajectory.trajectory_id,
                    "trajectory_step": index,
                    "credited_reward": trajectory_result.credited_rewards[index],
                    "cross_domain_plan_refinement_digest": cross_domain.plan_refinement_digest,
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "task_lens_digest": _memory_task_lens_digest(blueprint_item),
                    "task_intent_digest": _memory_task_intent_digest(blueprint_item),
                    "task_decision_digest": _memory_task_decision_digest(blueprint_item),
                },
                memory=memory_store,
                authorization_context=authorization_context,
                authorization_domain=blueprint_item.spec.domain,
                authorization_capability=blueprint_item.spec.capability,
                authorization_risk_class=blueprint_item.spec.risk_class,
            )
            try:
                evaluation_record = decision.to_dict()
                if not retain_replan_instruction:
                    evaluation_record["replan_instruction_digest"] = (
                        None
                        if decision.replan_instruction is None
                        else content_digest(decision.replan_instruction)
                    )
                    evaluation_record.pop("replan_instruction", None)
                decision_digest = content_digest(evaluation_record)
                self._authorize_memory_evaluation(
                    authorization_context,
                    domain=blueprint_item.spec.domain,
                    episode_id=episode_id,
                    decision_digest=decision_digest,
                    trajectory_id=trajectory.trajectory_id,
                    trajectory_step=index,
                )
                evaluation_receipt = memory_store.record_evaluation(
                    episode_id,
                    {
                        **evaluation_record,
                        "decision_digest": decision_digest,
                    },
                ).to_dict()
            except BrainMemoryError as error:
                raise BrainRunError("cross-domain trajectory evaluation memory record failed") from error
            memory_receipts.extend((episode_receipt, evaluation_receipt))
            evaluations.append(
                {
                    "scope": scope,
                    "item_id": item_id,
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": recording.get("status"),
                        "next_state": recording.get("next_state"),
                        "learning_evidence": recording.get("learning_evidence"),
                        "trajectory_id": trajectory.trajectory_id,
                        "trajectory_step": index,
                        "credited_reward": trajectory_result.credited_rewards[index],
                    },
                }
            )
        return AutonomousCrossDomainTrajectoryLearningResult(
            status=cross_domain.status,
            cross_domain=cross_domain,
            trajectory_result=trajectory_result,
            evaluations=tuple(evaluations),
            bandit_state=trajectory_result.bandit_state,
            memory_receipts=tuple(memory_receipts),
        )

    def run_cross_domain_trajectory_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_tags: Sequence[str] = (),
        trajectory_id: str | None = None,
        trajectory_discount: float = 0.90,
        trajectory_terminal_reward: float | None = None,
        ledger: BrainLearningLedger | None = None,
        **kwargs: Any,
    ) -> AutonomousCrossDomainTrajectoryLearningResult:
        """Run cross-domain specialists and synthesis, then settle one delayed trajectory.

        This mode uses one explicit evaluator identity for the complete fan-out/synthesis
        sequence. That requirement is intentional: a trajectory must have comparable reward
        semantics, while domain-specific evaluators can still be composed by the caller into one
        value-only cross-domain rubric.
        """

        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        evaluator = _resolve_cross_domain_evaluator(
            evaluator,
            [
                *[
                    value.get("domain")
                    for value in subtasks
                    if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                ],
                "cross_domain",
            ],
        )
        if evidence is not None:
            if not isinstance(evidence, Mapping) or any(
                not isinstance(key, str) or not isinstance(value, Mapping)
                for key, value in evidence.items()
            ):
                raise BrainRunError("cross-domain trajectory evidence must map ids to mappings")
            _safe_json("cross-domain trajectory evidence", evidence, maximum=1_000_000)
        memory_store = memory if memory is not None else self.brain.memory
        if not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("cross-domain trajectory memory must be a BrainEpisodicMemory")
        authorization_context = kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("cross-domain trajectory authorization_context must be an AutonomousAuthorizationContext or None")
        normalized_tags = _sequence("cross-domain trajectory memory_tags", memory_tags, maximum=32)
        execution_options = dict(kwargs)
        execution_options.pop("bandit_state", None)
        execution_options["memory"] = memory_store
        execution_options["ledger"] = ledger
        allow_partial = execution_options.get("allow_partial", False)
        if not isinstance(allow_partial, bool):
            raise BrainRunError("cross-domain trajectory allow_partial must be a boolean")
        cross_domain = self.run_cross_domain(
            task=task,
            subtasks=subtasks,
            model_candidates=model_candidates,
            credentials=credentials,
            bandit_state=bandit_state,
            **execution_options,
        )
        items = self._cross_domain_trajectory_items(
            cross_domain,
            allow_partial=allow_partial,
        )
        results = [item[3] for item in items]
        evidence_packets = [None if evidence is None else evidence.get(item[1]) for item in items]
        trajectory = self.brain.prepare_learning_trajectory(
            results,
            evidence_by_step=evidence_packets,
            trajectory_id=trajectory_id or f"cross-domain-{content_digest({'task': task, 'runs': [result.brain_run.run_id if isinstance(result, (BrainToolLoopResult, BrainMissionResult)) else result.run_id for result in results]})}",
            discount=trajectory_discount,
            terminal_reward=trajectory_terminal_reward,
            ledger=ledger,
        )
        trajectory_result = evaluator.evaluate_trajectory(
            self.brain,
            trajectory,
            bandit_state=bandit_state,
            evidence_by_step=evidence_packets,
            ledger=ledger,
        )
        evaluations: list[Mapping[str, Any]] = []
        memory_receipts: list[Mapping[str, Any]] = []
        for index, ((scope, item_id, blueprint_item, result), decision, recording) in enumerate(
            zip(items, trajectory_result.decisions, trajectory_result.recordings)
        ):
            episode_id = trajectory.episodes[index].episode_id
            episode_receipt = self.brain.remember_result(
                result,
                task=blueprint_item.spec.task,
                episode_id=episode_id,
                context=_memory_selection_context(blueprint_item),
                tags=[
                    *normalized_tags,
                    f"domain:{blueprint_item.spec.domain}",
                    f"cross_domain:{scope}",
                    f"item:{item_id}",
                    "learning:trajectory",
                ],
                lesson=decision.replan_instruction if decision.replan_requested else None,
                provenance={
                    "scope": scope,
                    "item_id": item_id,
                    "trajectory_id": trajectory.trajectory_id,
                    "trajectory_step": index,
                    "credited_reward": trajectory_result.credited_rewards[index],
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "task_lens_digest": _memory_task_lens_digest(blueprint_item),
                    "task_intent_digest": _memory_task_intent_digest(blueprint_item),
                    "task_decision_digest": _memory_task_decision_digest(blueprint_item),
                },
                memory=memory_store,
                authorization_context=authorization_context,
                authorization_domain=blueprint_item.spec.domain,
                authorization_capability=blueprint_item.spec.capability,
                authorization_risk_class=blueprint_item.spec.risk_class,
            )
            try:
                decision_digest = content_digest(decision.to_dict())
                self._authorize_memory_evaluation(
                    authorization_context,
                    domain=blueprint_item.spec.domain,
                    episode_id=episode_id,
                    decision_digest=decision_digest,
                    trajectory_id=trajectory.trajectory_id,
                    trajectory_step=index,
                )
                evaluation_receipt = memory_store.record_evaluation(
                    episode_id,
                    {
                        **decision.to_dict(),
                        "decision_digest": decision_digest,
                    },
                ).to_dict()
            except BrainMemoryError as error:
                raise BrainRunError("cross-domain trajectory evaluation memory record failed") from error
            memory_receipts.extend((episode_receipt, evaluation_receipt))
            evaluations.append(
                {
                    "scope": scope,
                    "item_id": item_id,
                    "decision": decision.to_dict(),
                    "recording": {
                        "status": recording.get("status"),
                        "next_state": recording.get("next_state"),
                        "learning_evidence": recording.get("learning_evidence"),
                        "trajectory_id": trajectory.trajectory_id,
                        "trajectory_step": index,
                        "credited_reward": trajectory_result.credited_rewards[index],
                    },
                }
            )
        return AutonomousCrossDomainTrajectoryLearningResult(
            status=cross_domain.status,
            cross_domain=cross_domain,
            trajectory_result=trajectory_result,
            evaluations=tuple(evaluations),
            bandit_state=trajectory_result.bandit_state,
            memory_receipts=tuple(memory_receipts),
        )

    def run_cross_domain_replan_learning(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_tags: Sequence[str] = (),
        max_replans: int = 1,
        trajectory_id: str | None = None,
        trajectory_discount: float = 0.90,
        trajectory_terminal_reward: float | None = None,
        run_id: str | None = None,
        idempotency_key: str | None = None,
        checkpoint: AutonomousCrossDomainReplanCheckpoint | Mapping[str, Any] | None = None,
        checkpoint_sink: Callable[[AutonomousCrossDomainReplanCheckpoint], Any] | None = None,
        ledger: BrainLearningLedger | None = None,
        **kwargs: Any,
    ) -> AutonomousCrossDomainReplanResult:
        """Run bounded evaluator-guided fan-out/fan-in retries with delayed credit per attempt.

        Every completed attempt is settled before the next route decision. The projected learner
        state therefore affects the next specialist and synthesis selections, while the approved
        route, caller-owned tools, credentials, effect policy, and aggregate execution controller
        remain unchanged. A replan instruction is inserted only as a reserved developer context
        packet and is never copied into the value-only result projection. ``checkpoint_sink`` is
        called only after an attempt's trajectory has settled; a caller can persist the resulting
        metadata and resume at the next attempt by supplying the checkpoint and the same
        value-only bandit state plus the caller-owned continuation context.
        """

        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("cross-domain replan bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        evaluator = _resolve_cross_domain_evaluator(
            evaluator,
            [
                *[
                    value.get("domain")
                    for value in subtasks
                    if isinstance(value, Mapping) and isinstance(value.get("domain"), str)
                ],
                "cross_domain",
            ],
        )
        memory_store = memory if memory is not None else self.brain.memory
        if not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("cross-domain replan memory must be a BrainEpisodicMemory")
        authorization_context = kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("cross-domain replan authorization_context must be an AutonomousAuthorizationContext or None")
        if evidence is not None:
            if not isinstance(evidence, Mapping) or any(
                not isinstance(key, str) or not isinstance(value, Mapping)
                for key, value in evidence.items()
            ):
                raise BrainRunError("cross-domain replan evidence must map ids to mappings")
            _safe_json("cross-domain replan evidence", evidence, maximum=1_000_000)
        normalized_tags = _sequence("cross-domain replan memory_tags", memory_tags, maximum=32)
        if not isinstance(max_replans, int) or isinstance(max_replans, bool) or not 0 <= max_replans <= MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS:
            raise BrainRunError(
                f"cross-domain replan max_replans must be within [0, {MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS}]"
            )
        base_context = kwargs.pop("context", None)
        if base_context is None:
            base_context = {}
        elif not isinstance(base_context, Mapping):
            raise BrainRunError("cross-domain replan context must be a mapping or None")
        base_context = dict(_safe_json("cross-domain replan context", base_context))
        if checkpoint_sink is not None and not callable(checkpoint_sink):
            raise BrainRunError("cross-domain replan checkpoint_sink must be callable or None")
        checkpoint_value: AutonomousCrossDomainReplanCheckpoint | None
        if checkpoint is None:
            checkpoint_value = None
        elif isinstance(checkpoint, AutonomousCrossDomainReplanCheckpoint):
            checkpoint_value = checkpoint
        elif isinstance(checkpoint, Mapping):
            checkpoint_value = AutonomousCrossDomainReplanCheckpoint.from_dict(checkpoint)
        else:
            raise BrainRunError("cross-domain replan checkpoint must be a checkpoint, mapping, or None")
        if checkpoint_value is None and _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY in base_context:
            raise BrainRunError(
                "cross-domain replan continuation context requires a matching checkpoint"
            )

        initial_context = {
            key: value
            for key, value in base_context.items()
            if key != _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY
        }
        preparation_options: dict[str, Any] = {"context": initial_context}
        for option_name in (
            "desired_outputs",
            "child_execution_mode",
            "synthesis_execution_mode",
            "max_steps",
            "require_json",
            "structured_domain_response",
            "response_schema",
        ):
            if option_name in kwargs:
                preparation_options[option_name] = kwargs[option_name]
        if "input_tokens" in kwargs:
            preparation_options["max_input_tokens"] = kwargs["input_tokens"]
        base_blueprint = self.prepare_cross_domain(
            task=task,
            subtasks=subtasks,
            **preparation_options,
        )
        base_plan_digest = _cross_domain_plan_digest(base_blueprint)
        if checkpoint_value is not None:
            if checkpoint_value.task_digest != base_blueprint.task_digest:
                raise BrainRunError("cross-domain replan checkpoint task does not match the request")
            if checkpoint_value.base_plan_digest != base_plan_digest:
                raise BrainRunError("cross-domain replan checkpoint plan does not match the request")
            if checkpoint_value.max_replans != max_replans:
                raise BrainRunError("cross-domain replan checkpoint max_replans does not match the request")
            if run_id is None:
                base_run_id = checkpoint_value.run_id
            else:
                base_run_id = _identifier("cross-domain replan run_id", run_id)
                if base_run_id != checkpoint_value.run_id:
                    raise BrainRunError("cross-domain replan checkpoint run_id does not match the request")
        elif run_id is None:
            base_run_id = f"cross-replan-{uuid.uuid4().hex}"
        else:
            base_run_id = _identifier("cross-domain replan run_id", run_id)
        if idempotency_key is not None:
            _text("cross-domain replan idempotency_key", idempotency_key, maximum=256)
        if trajectory_id is None and checkpoint_value is not None:
            base_trajectory_id = checkpoint_value.trajectory_base_id
        elif trajectory_id is None:
            base_trajectory_id = "cross-domain-replan-" + content_digest(
                {"task": task, "run_id": base_run_id}
            )
        else:
            base_trajectory_id = _text("cross-domain replan trajectory_id", trajectory_id, maximum=512)
        if checkpoint_value is not None and base_trajectory_id != checkpoint_value.trajectory_base_id:
            raise BrainRunError("cross-domain replan checkpoint trajectory identity does not match the request")

        def attempt_identity(prefix: str, attempt: int, name: str) -> str:
            candidate = f"{prefix}-attempt-{attempt}"
            if len(candidate) > 128:
                candidate = "cross-replan-" + content_digest({"prefix": prefix, "attempt": attempt})[:48]
            return _identifier(name, candidate)

        def attempt_trajectory_id(attempt: int) -> str:
            candidate = f"{base_trajectory_id}-attempt-{attempt}"
            if len(candidate.encode("utf-8")) > 512:
                candidate = "cross-domain-replan-trajectory-" + content_digest(
                    {"trajectory_id": base_trajectory_id, "attempt": attempt}
                )
            return _text("cross-domain replan attempt trajectory_id", candidate, maximum=512)

        state: Mapping[str, Any] = dict(bandit_state)
        if checkpoint_value is not None and checkpoint_value.attempt > 0:
            if checkpoint_value.bandit_state_digest != content_digest(state):
                raise BrainRunError("cross-domain replan checkpoint bandit state does not match the request")
        if checkpoint_value is None:
            checkpoint_value = AutonomousCrossDomainReplanCheckpoint(
                run_id=base_run_id,
                task_digest=base_blueprint.task_digest,
                base_plan_digest=base_plan_digest,
                trajectory_base_id=base_trajectory_id,
                max_replans=max_replans,
                attempt=0,
                status="initial",
            )
        elif checkpoint_value.status in {
            "completed",
            "completed_without_replan",
            "replan_limit_reached",
        }:
            raise BrainRunError("cross-domain replan checkpoint is already terminal")
        if checkpoint_value.status == "retry_ready":
            retry_context = base_context.get(_AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY)
            if not isinstance(retry_context, Mapping):
                raise BrainRunError("resuming a cross-domain replan requires the caller-owned retry context")
            if checkpoint_value.next_context_digest != content_digest(retry_context):
                raise BrainRunError("caller-owned retry context does not match the replan checkpoint")
        start_attempt = checkpoint_value.attempt + 1
        attempts_before = checkpoint_value.attempt
        current_context = dict(base_context)
        attempts: list[AutonomousCrossDomainReplanAttempt] = []

        def persist_checkpoint(value: AutonomousCrossDomainReplanCheckpoint) -> None:
            if checkpoint_sink is None:
                return
            try:
                checkpoint_sink(value)
            except Exception as error:
                raise BrainRunError("cross-domain replan checkpoint persistence failed") from error

        def checkpoint_after_attempt(
            *,
            attempt_result: AutonomousCrossDomainReplanAttempt,
            status: str,
            next_context: Mapping[str, Any] | None = None,
        ) -> AutonomousCrossDomainReplanCheckpoint:
            trajectory_ids = [*checkpoint_value.attempt_trajectory_ids, attempt_result.trajectory_result.trajectory.trajectory_id]
            outcome_digests = [*checkpoint_value.attempt_outcome_digests, attempt_result.outcome_digest]
            checkpoint_result = AutonomousCrossDomainReplanCheckpoint(
                run_id=base_run_id,
                task_digest=base_blueprint.task_digest,
                base_plan_digest=base_plan_digest,
                trajectory_base_id=base_trajectory_id,
                max_replans=max_replans,
                attempt=attempt_result.attempt,
                status=status,
                replan_count=max(0, attempt_result.attempt - 1),
                attempt_trajectory_ids=tuple(trajectory_ids),
                attempt_outcome_digests=tuple(outcome_digests),
                last_plan_digest=attempt_result.plan_digest,
                last_outcome_digest=attempt_result.outcome_digest,
                next_context_digest=None if next_context is None else content_digest(next_context),
                replan_instruction_digest=attempt_result.replan_instruction_digest if next_context is not None else None,
                bandit_state_digest=content_digest(attempt_result.bandit_state),
            )
            persist_checkpoint(checkpoint_result)
            return checkpoint_result

        for attempt in range(start_attempt, max_replans + 2):
            attempt_run_id = attempt_identity(base_run_id, attempt, "cross-domain replan attempt run_id")
            attempt_key = None if idempotency_key is None else attempt_identity(
                idempotency_key,
                attempt,
                "cross-domain replan attempt idempotency_key",
            )
            execution_options = dict(kwargs)
            execution_options.update(
                {
                    "context": dict(current_context),
                    "memory": memory_store,
                    "ledger": ledger,
                    "run_id": attempt_run_id,
                    "idempotency_key": attempt_key,
                }
            )
            cross_domain = self.run_cross_domain(
                task=task,
                subtasks=subtasks,
                model_candidates=model_candidates,
                credentials=credentials,
                bandit_state=state,
                **execution_options,
            )
            has_completed_result = any(
                result.status.startswith("completed")
                for result in (
                    *cross_domain.child_results,
                    *((cross_domain.synthesis_result,) if cross_domain.synthesis_result is not None else ()),
                )
            )
            if not has_completed_result:
                return AutonomousCrossDomainReplanResult(
                    status=cross_domain.status,
                    final=None,
                    attempts=tuple(attempts),
                    replan_count=max(0, attempts_before + len(attempts) - 1),
                    attempts_before=attempts_before,
                    checkpoint=checkpoint_value,
                )
            trajectory_result = self.settle_cross_domain_trajectory_learning(
                cross_domain=cross_domain,
                bandit_state=state,
                evaluator=evaluator,
                evidence=evidence,
                memory=memory_store,
                memory_tags=[*normalized_tags, f"attempt:{attempt}"],
                trajectory_id=attempt_trajectory_id(attempt),
                trajectory_discount=trajectory_discount,
                trajectory_terminal_reward=trajectory_terminal_reward,
                retain_replan_instruction=False,
                ledger=ledger,
                authorization_context=authorization_context,
            )
            state = dict(trajectory_result.bandit_state)
            decisions = trajectory_result.trajectory_result.decisions
            requested = [decision for decision in decisions if decision.replan_requested]
            selected_decision = requested[-1] if requested else None
            plan_digest = _cross_domain_plan_digest(cross_domain.blueprint)
            outcome_digest = _cross_domain_execution_digest(cross_domain)
            projections = tuple(
                _cross_domain_replan_evaluation_projection(item)
                for item in trajectory_result.evaluations
            )
            instruction_digest = (
                None
                if selected_decision is None or selected_decision.replan_instruction is None
                else content_digest(selected_decision.replan_instruction)
            )
            attempt_result = AutonomousCrossDomainReplanAttempt(
                attempt=attempt,
                status=cross_domain.status,
                cross_domain=cross_domain,
                trajectory_result=trajectory_result.trajectory_result,
                evaluations=projections,
                bandit_state=state,
                plan_digest=plan_digest,
                outcome_digest=outcome_digest,
                learning_episode_ids=tuple(
                    episode.episode_id for episode in trajectory_result.trajectory_result.trajectory.episodes
                ),
                replan_requested=selected_decision is not None,
                replan_instruction_digest=instruction_digest,
                memory_receipts=trajectory_result.memory_receipts,
            )
            attempts.append(attempt_result)
            if selected_decision is None:
                passed = bool(decisions) and all(decision.passed for decision in decisions)
                final_status = "completed" if cross_domain.status == "completed" and passed else "completed_without_replan"
                checkpoint_value = checkpoint_after_attempt(
                    attempt_result=attempt_result,
                    status=final_status,
                )
                return AutonomousCrossDomainReplanResult(
                    status=final_status,
                    final=attempt_result,
                    attempts=tuple(attempts),
                    replan_count=attempt - 1,
                    attempts_before=attempts_before,
                    checkpoint=checkpoint_value,
                )
            if attempt > max_replans:
                checkpoint_value = checkpoint_after_attempt(
                    attempt_result=attempt_result,
                    status="replan_limit_reached",
                )
                return AutonomousCrossDomainReplanResult(
                    status="replan_limit_reached",
                    final=attempt_result,
                    attempts=tuple(attempts),
                    replan_count=attempt - 1,
                    attempts_before=attempts_before,
                    checkpoint=checkpoint_value,
                )
            next_context = _cross_domain_replan_context(
                attempt=attempt + 1,
                plan_digest=plan_digest,
                outcome_digest=outcome_digest,
                decision=selected_decision,
            )
            current_context[_AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY] = next_context
            checkpoint_value = checkpoint_after_attempt(
                attempt_result=attempt_result,
                status="retry_ready",
                next_context=next_context,
            )
        raise BrainRunError("cross-domain replan loop exited without a terminal result")

    def _run_learning_from_blueprint(
        self,
        blueprint: AutonomousTaskBlueprint,
        *,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        bandit_state: Mapping[str, Any],
        store: BrainEpisodicMemory | None,
        evaluator: BrainOutcomeEvaluator | None,
        evaluator_registry: DomainEvaluatorRegistry | None,
        evidence: Mapping[str, Any] | None,
        ledger: BrainLearningLedger | None,
        max_replans: int,
        memory_tags: Sequence[str],
        require_response_review: bool,
        execution_kwargs: Mapping[str, Any],
    ) -> AutonomousLearningResult:
        if store is None:
            raise BrainRunError("memory is required for autonomous online learning")
        authorization_context = execution_kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("autonomous learning authorization_context must be an AutonomousAuthorizationContext or None")
        resolved_evaluator = evaluator
        if resolved_evaluator is None:
            registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
            resolved_evaluator = registry.resolve_for_autonomous_domain(
                blueprint.domain_pack.domain,
                fallback_domain=blueprint.domain_pack.evaluator_domain,
            )
        if not isinstance(resolved_evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        current_prompt = blueprint.prompt
        state: Mapping[str, Any] = dict(bandit_state)
        attempts: list[BrainRunResult | BrainToolLoopResult] = []
        evaluations: list[dict[str, Any]] = []
        receipts: list[dict[str, Any]] = []
        final_status = "completed"
        replans = 0
        for attempt in range(max_replans + 1):
            kwargs = dict(execution_kwargs)
            kwargs["prompt"] = current_prompt
            # Feed the evaluator's latest value-only state into the next model-selection call;
            # recording a reward without changing the next arm choice is not online learning.
            kwargs["bandit_state"] = state
            result = self._run_prepared(
                blueprint,
                model_candidates=model_candidates,
                credentials=credentials,
                ledger=ledger,
                **kwargs,
            )
            if not isinstance(result, (BrainRunResult, BrainToolLoopResult)):
                raise BrainRunError("autonomous online learning does not accept mission results")
            attempts.append(result)
            if result.status not in {"completed_provider_call", "completed_provider_tool_loop"}:
                final_status = result.status
                break
            structured_evaluation = _structured_response_evaluation(result)
            if (
                require_response_review
                and isinstance(structured_evaluation, Mapping)
                and structured_evaluation.get("passed") is False
            ):
                structured_feedback = _record_structured_response_feedback(
                    self.brain,
                    result,
                    bandit_state=state,
                    ledger=ledger,
                )
                if structured_feedback is not None:
                    state, structured_evaluation_record = structured_feedback
                    evaluations.append(structured_evaluation_record)
                final_status = "response_review_required"
                break
            decision, report = resolved_evaluator.evaluate_and_record_with_decision(
                self.brain,
                result,
                bandit_state=state,
                evidence=evidence,
                ledger=ledger,
            )
            brain_result = result if isinstance(result, BrainRunResult) else result.brain_run
            next_state = report.get("next_state")
            if isinstance(next_state, Mapping):
                state = dict(next_state)
            structured_feedback = _record_structured_response_feedback(
                self.brain,
                result,
                bandit_state=state,
                ledger=ledger,
            )
            if structured_feedback is not None:
                state, structured_evaluation_record = structured_feedback
                evaluations.append(structured_evaluation_record)
            episode_id = f"{brain_result.run_id}-attempt-{attempt}"
            receipt = self.brain.remember_result(
                result,
                task=blueprint.spec.task,
                episode_id=episode_id,
                # The full selection context is intentionally rich for the
                # live selector, but episodic memory has a tighter bounded
                # context-key contract. Lens guidance is already retained by
                # the blueprint/plan digests; keep the replay projection
                # compact and carry the lens digest in provenance below.
                context=_memory_selection_context(blueprint),
                tags=[*memory_tags, f"domain:{blueprint.spec.domain}", f"attempt:{attempt}"],
                lesson=decision.replan_instruction if decision.replan_requested else None,
                provenance={
                    "evaluator_id": decision.evaluator_id,
                    "evaluator_version": decision.evaluator_version,
                    "task_lens_digest": _memory_task_lens_digest(blueprint),
                    "task_intent_digest": _memory_task_intent_digest(blueprint),
                    "task_decision_digest": _memory_task_decision_digest(blueprint),
                },
                memory=store,
                authorization_context=authorization_context,
                authorization_domain=blueprint.spec.domain,
                authorization_capability=blueprint.spec.capability,
                authorization_risk_class=blueprint.spec.risk_class,
            )
            try:
                decision_digest = content_digest(decision.to_dict())
                self._authorize_memory_evaluation(
                    authorization_context,
                    domain=blueprint.spec.domain,
                    episode_id=episode_id,
                    decision_digest=decision_digest,
                )
                evaluation_receipt = store.record_evaluation(
                    episode_id,
                    {**decision.to_dict(), "decision_digest": decision_digest},
                ).to_dict()
            except BrainMemoryError as error:
                raise BrainRunError("autonomous evaluation memory record failed") from error
            receipts.extend((receipt, evaluation_receipt))
            evaluations.append({"decision": decision.to_dict(), "recording": {"status": report.get("status"), "next_state": report.get("next_state"), "learning_evidence": report.get("learning_evidence")}})
            if not decision.failed or not decision.replan_requested:
                final_status = "completed" if decision.passed else "completed_without_replan"
                break
            if attempt >= max_replans:
                final_status = "replan_limit_reached"
                break
            replans += 1
            current_prompt = self._append_replan(current_prompt, attempt=attempt + 1, result=result, decision=decision)
        return AutonomousLearningResult(
            status=final_status,
            blueprint=blueprint,
            final_result=attempts[-1],
            attempts=tuple(attempts),
            evaluations=tuple(evaluations),
            memory_receipts=tuple(receipts),
            replan_count=replans,
            bandit_state=state,
        )

# Keep methods discoverable through the public AutonomousTaskOrchestrator facade.
for _name, _descriptor in vars(AutonomousOrchestratorCrossDomainMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousTaskOrchestrator.{_name}"
