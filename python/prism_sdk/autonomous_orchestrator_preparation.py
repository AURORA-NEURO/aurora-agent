"""Focused autonomous-orchestrator preparation methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AUTONOMY_SCHEMA,
    Any,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationError,
    AutonomousAutoBlueprint,
    AutonomousBrain,
    AutonomousContextBudgetOptions,
    AutonomousCostReservationCallback,
    AutonomousCrossDomainBlueprint,
    AutonomousEvidencePlan,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
    AutonomousMemoryConsolidator,
    AutonomousPlanBuilder,
    AutonomousPromptBuilder,
    AutonomousRouteProposal,
    AutonomousSemanticRouteResult,
    AutonomousTaskBlueprint,
    AutonomousTaskSpec,
    BrainEpisodicMemory,
    BrainLearningCycleResult,
    BrainMemoryError,
    BrainRunError,
    BrainRunResult,
    BrainToolLoopResult,
    Callable,
    CredentialHandle,
    MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS,
    MAX_AUTONOMY_MEMORY_ITEMS,
    MAX_AUTONOMY_TEXT_BYTES,
    Mapping,
    MemoryQuery,
    ProviderTool,
    Sequence,
    _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY,
    _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
    _AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY,
    _identifier,
    _resolve_domain_capability_contract,
    _safe_json,
    _sequence,
    _structured_response_evaluation,
    _text,
    autonomous_domain_policy,
    autonomous_domain_task_lens,
    build_autonomous_domain_response_contract,
    build_autonomous_evidence_plan,
    content_digest,
    evaluate_autonomous_domain_response,
    infer_autonomous_task_decision,
    infer_autonomous_task_intent,
    replace,
    route_autonomous_capability,
    task_facet_digests,
    validate_autonomous_provider_domain_response,
)

class AutonomousOrchestratorPreparationMixin:
    def prepare(
        self,
        *,
        task: str,
        domain: str,
        capability: str | None = None,
        risk_class: str | None = None,
        constraints: Sequence[str] = (),
        desired_outputs: Sequence[str] = (),
        context: Mapping[str, Any] | None = None,
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        execution_mode: str = "provider",
        max_input_tokens: int = 4_096,
        required_model_capabilities: Sequence[str] = (),
        memory_episodes: Sequence[Mapping[str, Any]] = (),
        memory_lesson_references: Sequence[Mapping[str, Any]] = (),
    ) -> AutonomousTaskBlueprint:
        profile = self.registry.resolve(domain)
        workflow = self.workflow_registry.resolve(profile.domain)
        domain_pack = self.pack_registry.resolve(profile.domain)
        unsupported_workflow_capabilities = sorted(
            {
                capability
                for stage in workflow.stages
                for capability in stage.required_capabilities
                if capability not in set(profile.capabilities)
            }
        )
        if unsupported_workflow_capabilities:
            raise BrainRunError(
                "workflow requires capabilities outside the domain profile: "
                + ", ".join(unsupported_workflow_capabilities)
            )
        capability_route = route_autonomous_capability(
            task,
            profile.domain,
            explicit_capability=None if capability is None else _identifier("capability", capability),
        )
        resolved_capability = capability_route.selected_capability or profile.default_capability
        resolved_risk = profile.risk_class if risk_class is None else _identifier("risk_class", risk_class)
        capability_contract = _resolve_domain_capability_contract(
            profile,
            domain_pack,
            workflow,
            resolved_capability,
        )
        if not isinstance(structured_domain_response, bool):
            raise BrainRunError("structured_domain_response must be a boolean")
        if structured_domain_response and response_schema is not None:
            raise BrainRunError("structured_domain_response cannot be combined with response_schema")
        response_contract = (
            build_autonomous_domain_response_contract(profile, workflow=workflow)
            if structured_domain_response
            else None
        )
        resolved_require_json = require_json or structured_domain_response
        resolved_response_schema = response_schema
        if response_contract is not None:
            resolved_response_schema = response_contract.response_schema
        if resolved_require_json and resolved_response_schema is None:
            resolved_response_schema = workflow.response_schema()
        spec = AutonomousTaskSpec(
            task=task,
            domain=profile.domain,
            capability=resolved_capability,
            risk_class=resolved_risk,
            constraints=tuple(constraints),
            desired_outputs=tuple(desired_outputs),
            context={} if context is None else context,
            max_steps=max_steps,
            require_json=resolved_require_json,
            structured_domain_response=structured_domain_response,
            response_schema=resolved_response_schema,
            execution_mode=execution_mode,
        )
        extra_capabilities = _sequence("required_model_capabilities", required_model_capabilities)
        required = tuple(
            dict.fromkeys(
                (
                    *profile.required_model_capabilities,
                    *domain_pack.model_capabilities,
                    *extra_capabilities,
                )
            )
        )
        evidence_plan = build_autonomous_evidence_plan((workflow,))
        task_lens = autonomous_domain_task_lens(spec.domain)
        task_intent = infer_autonomous_task_intent(
            task=spec.task,
            task_digest=spec.task_digest,
            domain=spec.domain,
            capability=spec.capability,
            risk_class=spec.risk_class,
            workflow_id=workflow.workflow_id,
            lens=task_lens,
            constraints=spec.constraints,
            desired_outputs=spec.desired_outputs,
        )
        task_decision = infer_autonomous_task_decision(
            intent=task_intent,
            lens=task_lens,
            policy=autonomous_domain_policy(spec.domain),
            required_model_capabilities=required,
        )
        selection_context = {
            "schema": AUTONOMY_SCHEMA,
            "workflow": "autonomous_task",
            "domain": spec.domain,
            "capability": spec.capability,
            "risk_class": spec.risk_class,
            "task_lens_id": task_lens.lens_id,
            "task_lens_digest": task_lens.lens_digest,
            "task_lens_model_capability_hints": list(task_lens.model_capability_hints),
            "task_lens_evaluator_signals": list(task_lens.evaluator_signals),
            "task_lens_planning_dimensions": list(task_lens.planning_dimensions),
            "task_intent_id": task_intent.intent_id,
            "task_intent_digest": task_intent.intent_digest,
            "task_intent_action_mode": task_intent.action_mode,
            "task_intent_requested_effect": task_intent.requested_effect,
            "task_intent_evidence_mode": task_intent.evidence_mode,
            "task_intent_ambiguity_flags": list(task_intent.ambiguity_flags),
            "task_decision_id": task_decision.decision_id,
            "task_decision_digest": task_decision.decision_digest,
            "task_decision_posture": task_decision.posture,
            "task_decision_recommended_path": task_decision.recommended_path,
            "task_decision_approval_requirements": list(task_decision.approval_requirements),
            "task_decision_review_reasons": list(task_decision.review_reasons),
            "execution_mode": spec.execution_mode,
            "domain_capabilities": list(profile.capabilities),
            "domain_pack_id": domain_pack.pack_id,
            "domain_pack_version": domain_pack.pack_version,
            "domain_pack_digest": domain_pack.pack_digest,
            "domain_pack_tool_capabilities": list(domain_pack.tool_capabilities),
            "domain_pack_evidence_requirements": list(domain_pack.evidence_requirements),
            "domain_pack_review_triggers": list(domain_pack.review_triggers),
            "domain_pack_evaluator_id": domain_pack.evaluator_id,
            "workflow_id": workflow.workflow_id,
            "workflow_digest": workflow.workflow_digest,
            "workflow_stage_ids": [stage.id for stage in workflow.stages],
            "workflow_evaluator_signals": list(workflow.evaluator_signals),
            "evidence_plan_digest": evidence_plan.plan_digest,
            "evidence_requirement_count": len(evidence_plan.requirements),
            "evidence_next_stage_ids": list(evidence_plan.next_stage_ids),
            "task_digest": spec.task_digest,
            "user_context_digest": spec.context_digest,
            "context_keys": sorted(str(key) for key in spec.context),
            "required_model_capabilities": list(required),
            "capability_contract_digest": capability_contract.contract_digest,
            "capability_tool_capabilities": list(capability_contract.tool_capabilities),
            "capability_stage_ids": list(capability_contract.stage_ids),
            "capability_evidence_outputs": list(capability_contract.evidence_outputs),
            "capability_evaluator_signals": list(capability_contract.evaluator_signals),
            "capability_route_digest": capability_route.route_digest,
            "capability_route_reason": capability_route.reason,
            "capability_route_confidence": capability_route.confidence,
        }
        if memory_lesson_references:
            lesson_metadata = []
            for index, reference in enumerate(memory_lesson_references):
                if not isinstance(reference, Mapping):
                    raise BrainRunError(f"memory lesson reference {index} must be a mapping")
                lesson_id = reference.get("lesson_id")
                lesson_digest = reference.get("lesson_digest")
                if not isinstance(lesson_id, str) or not isinstance(lesson_digest, str):
                    raise BrainRunError(f"memory lesson reference {index} is missing a stable identity")
                lesson_metadata.append({"lesson_id": lesson_id, "lesson_digest": lesson_digest})
            selection_context["consolidated_memory_retrieval_digest"] = content_digest(
                {"lessons": lesson_metadata}
            )
        runtime_execution_plan = spec.context.get(_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY)
        if runtime_execution_plan is not None:
            if not isinstance(runtime_execution_plan, Mapping):
                raise BrainRunError("autonomous execution plan context must be a mapping")
            plan_digest = runtime_execution_plan.get("plan_digest")
            plan_status = runtime_execution_plan.get("status")
            if not isinstance(plan_digest, str) or not isinstance(plan_status, str):
                raise BrainRunError("autonomous execution plan context is missing digest or status")
            selection_context["execution_plan_digest"] = plan_digest
            selection_context["execution_plan_status"] = plan_status
        runtime_stage_plan = spec.context.get(_AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY)
        if runtime_stage_plan is not None:
            if not isinstance(runtime_stage_plan, Mapping):
                raise BrainRunError("autonomous workflow stage plan context must be a mapping")
            stage_plan_digest = runtime_stage_plan.get("stage_plan_digest")
            stage_id = runtime_stage_plan.get("stage_id")
            if not isinstance(stage_plan_digest, str) or not isinstance(stage_id, str):
                raise BrainRunError("autonomous workflow stage plan context is missing digest or stage_id")
            selection_context["stage_execution_plan_digest"] = stage_plan_digest
            selection_context["stage_id"] = stage_id
        prompt = AutonomousPromptBuilder.build(
            spec,
            profile,
            domain_pack=domain_pack,
            workflow=workflow,
            capability_contract=capability_contract,
            max_input_tokens=max_input_tokens,
            memory_episodes=memory_episodes,
            memory_lesson_references=memory_lesson_references,
        )
        plan = AutonomousPlanBuilder.build(spec, workflow, domain_pack)
        _safe_json("autonomous selection context", selection_context)
        return AutonomousTaskBlueprint(
            spec=spec,
            profile=profile,
            domain_pack=domain_pack,
            workflow=workflow,
            selection_context=selection_context,
            prompt=prompt,
            plan=plan,
            required_capabilities=required,
            domain_policy=autonomous_domain_policy(spec.domain),
            task_lens=task_lens,
            task_intent=task_intent,
            task_decision=task_decision,
            capability_route=capability_route,
            response_contract=response_contract,
        )

    def evidence_plan(
        self,
        domains: Sequence[str] = AUTONOMOUS_DOMAINS,
        *,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> AutonomousEvidencePlan:
        """Compile the reviewed evidence contract for one or more autonomous domains.

        This is a planning-only operation.  It is useful before a provider call to decide which
        caller-owned files, connector reads, tool observations, or human reviews are still needed.
        It never dispatches a connector, turns a label into proof, or stores raw evidence.
        """

        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)) or not domains:
            raise BrainRunError("evidence_plan domains must contain at least one domain")
        normalized = tuple(_identifier("evidence_plan domain", domain) for domain in domains)
        if len(normalized) != len(set(normalized)):
            raise BrainRunError("evidence_plan domains must be unique")
        workflows = tuple(self.workflow_registry.resolve(domain) for domain in normalized)
        return build_autonomous_evidence_plan(
            workflows,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )

    def evidence_runtime(
        self,
        domains: Sequence[str] = AUTONOMOUS_DOMAINS,
        *,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        journal: AutonomousEvidenceRuntimeJournal | None = None,
    ) -> AutonomousEvidenceRuntime:
        """Create the caller-owned acquisition/evaluation runtime for an evidence plan."""

        return AutonomousEvidenceRuntime(
            self.evidence_plan(
                domains,
                available_evidence=available_evidence,
                completed_stages=completed_stages,
            ),
            journal=journal,
        )

    def acquire_evidence(
        self,
        domains: Sequence[str],
        requests: Sequence[Mapping[str, Any]],
        *,
        acquirer: Any,
        projector: Any | None = None,
        evaluator: Any | None = None,
        rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
        parent_evidence_digests: Sequence[str] = (),
        stop_on_failure: bool = False,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        journal: AutonomousEvidenceRuntimeJournal | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        authorization_domain: str | None = None,
        authorization_capability: str | None = None,
        authorization_risk_class: str | None = None,
    ) -> AutonomousEvidenceRuntimeResult:
        """Acquire and optionally evaluate evidence through application-owned adapters.

        Raw values remain transient in the runtime result.  Durable journals receive only
        metadata, digests, bounded observations, and explicit evaluator verdicts.
        """

        runtime = self.evidence_runtime(
            domains,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            journal=journal,
        )
        return runtime.execute(
            requests,
            acquirer=acquirer,
            projector=projector,
            evaluator=evaluator,
            rehydrate_value=rehydrate_value,
            parent_evidence_digests=parent_evidence_digests,
            stop_on_failure=stop_on_failure,
            authorization_context=authorization_context,
            authorization_domain=authorization_domain,
            authorization_capability=authorization_capability,
            authorization_risk_class=authorization_risk_class,
        )

    @staticmethod
    def _route_review_proposal(
        route: AutonomousRouteProposal,
        *,
        reason: str = "insufficient_confidence",
    ) -> AutonomousRouteProposal:
        """Convert an otherwise usable route into an explicit non-executable review value."""

        if route.abstained:
            return route
        return AutonomousRouteProposal(
            task_digest=route.task_digest,
            candidates=route.candidates,
            selected_domains=(),
            confidence=route.confidence,
            abstained=True,
            reason=reason,
            cross_domain=False,
            source=route.source,
        )

    def _prepare_auto_from_route(
        self,
        *,
        task: str,
        route: AutonomousRouteProposal,
        semantic_route: AutonomousSemanticRouteResult | None = None,
        context: Mapping[str, Any] | None = None,
        constraints: Sequence[str] = (),
        desired_outputs: Sequence[str] = (),
        capability: str | None = None,
        risk_class: str | None = None,
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        execution_mode: str = "provider",
        max_input_tokens: int = 4_096,
        required_model_capabilities: Sequence[str] = (),
        memory_episodes: Sequence[Mapping[str, Any]] = (),
    ) -> AutonomousAutoBlueprint:
        if semantic_route is not None and semantic_route.status != "completed":
            route = self._route_review_proposal(route)
        if route.abstained:
            return AutonomousAutoBlueprint(route=route, semantic_route=semantic_route)
        routed_context = self._route_context(context, route)
        if len(route.selected_domains) == 1:
            blueprint = self.prepare(
                task=task,
                domain=route.selected_domains[0],
                capability=capability,
                risk_class=risk_class,
                constraints=constraints,
                desired_outputs=desired_outputs,
                context=routed_context,
                max_steps=max_steps,
                require_json=require_json,
                structured_domain_response=structured_domain_response,
                response_schema=response_schema,
                execution_mode=execution_mode,
                max_input_tokens=max_input_tokens,
                required_model_capabilities=required_model_capabilities,
                memory_episodes=memory_episodes,
            )
            return AutonomousAutoBlueprint(
                route=route,
                blueprint=blueprint,
                semantic_route=semantic_route,
                capability_route=blueprint.capability_route,
            )
        subtasks = [
            {
                "id": f"route-{domain}",
                "task": task,
                "domain": domain,
                "capability": capability,
                "risk_class": risk_class,
                "constraints": constraints,
                "desired_outputs": desired_outputs,
                "context": routed_context,
                "max_steps": max_steps,
                "require_json": require_json,
                "structured_domain_response": structured_domain_response,
                "response_schema": response_schema,
                "execution_mode": execution_mode,
                "required_model_capabilities": required_model_capabilities,
            }
            for domain in route.selected_domains
        ]
        cross_domain = self.prepare_cross_domain(
            task=task,
            subtasks=subtasks,
            context=routed_context,
            desired_outputs=desired_outputs or (
                "domain-attributed findings",
                "cross-domain conflicts and uncertainty",
                "safe next actions",
            ),
            child_execution_mode=execution_mode,
            synthesis_execution_mode=execution_mode,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            max_input_tokens=max_input_tokens,
        )
        return AutonomousAutoBlueprint(
            route=route,
            cross_domain_blueprint=cross_domain,
            semantic_route=semantic_route,
        )

    def prepare_auto(
        self,
        *,
        task: str,
        route_override: AutonomousRouteProposal | None = None,
        hints: Sequence[str] = (),
        context: Mapping[str, Any] | None = None,
        constraints: Sequence[str] = (),
        desired_outputs: Sequence[str] = (),
        capability: str | None = None,
        risk_class: str | None = None,
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        execution_mode: str = "provider",
        max_input_tokens: int = 4_096,
        required_model_capabilities: Sequence[str] = (),
        memory_episodes: Sequence[Mapping[str, Any]] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
    ) -> AutonomousAutoBlueprint:
        """Create a single- or cross-domain blueprint, or an explicit review request."""

        if route_override is None:
            route = self.route_task(
                task=task,
                hints=hints,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
            )
        else:
            if not isinstance(route_override, AutonomousRouteProposal):
                raise BrainRunError("route_override must be an AutonomousRouteProposal")
            expected_task_digest = content_digest({"task": task})
            if route_override.task_digest != expected_task_digest:
                raise BrainRunError("route_override task does not match the automatic task")
            route = route_override
        return self._prepare_auto_from_route(
            task=task,
            route=route,
            context=context,
            constraints=constraints,
            desired_outputs=desired_outputs,
            capability=capability,
            risk_class=risk_class,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            execution_mode=execution_mode,
            max_input_tokens=max_input_tokens,
            required_model_capabilities=required_model_capabilities,
            memory_episodes=memory_episodes,
        )

    def prepare_auto_with_provider(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        hints: Sequence[str] = (),
        context: Mapping[str, Any] | None = None,
        constraints: Sequence[str] = (),
        desired_outputs: Sequence[str] = (),
        capability: str | None = None,
        risk_class: str | None = None,
        max_steps: int = 8,
        require_json: bool = False,
        structured_domain_response: bool = False,
        response_schema: Mapping[str, Any] | None = None,
        execution_mode: str = "provider",
        max_input_tokens: int = 4_096,
        required_model_capabilities: Sequence[str] = (),
        memory_episodes: Sequence[Mapping[str, Any]] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        semantic_weight: float = 0.65,
        bandit_state: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
        input_tokens: int = 4_096,
        requested_output_tokens: int = 1_024,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        approve_provider_call: bool = False,
        run_id: str | None = None,
        max_output_tokens: int = 1_024,
        temperature: float | None = None,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
    ) -> AutonomousAutoBlueprint:
        """Use a caller-approved classifier, reconcile it, then build the executable blueprint."""

        semantic = self.route_with_provider(
            task=task,
            model_candidates=model_candidates,
            credentials=credentials,
            hints=hints,
            context=context,
            min_confidence=min_confidence,
            min_margin=min_margin,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            semantic_weight=semantic_weight,
            bandit_state=bandit_state,
            contextual_observations=contextual_observations,
            selection_overrides=selection_overrides,
            selection_weights=selection_weights,
            selection_observations=selection_observations,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            domain_policy_mode=domain_policy_mode,
            domain_policy_evidence_ready=domain_policy_evidence_ready,
            domain_policy_evaluator_configured=domain_policy_evaluator_configured,
            domain_policy_effects_requested=domain_policy_effects_requested,
            domain_policy_effects_approved=domain_policy_effects_approved,
        )
        return self._prepare_auto_from_route(
            task=task,
            route=semantic.route,
            semantic_route=semantic,
            context=context,
            constraints=constraints,
            desired_outputs=desired_outputs,
            capability=capability,
            risk_class=risk_class,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            execution_mode=execution_mode,
            max_input_tokens=max_input_tokens,
            required_model_capabilities=required_model_capabilities,
            memory_episodes=memory_episodes,
        )

    @staticmethod
    def route_request_for(
        blueprint: AutonomousTaskBlueprint,
        *,
        max_tools: int = 128,
    ) -> dict[str, Any]:
        """Create a bounded capability-route proposal from a prepared domain blueprint.

        The proposal is intentionally only a router query. It does not grant a tool, authorize a
        side effect, or persist the task text. The live capability service remains authoritative
        for resolution and the caller remains authoritative for execution.
        """

        if not isinstance(max_tools, int) or isinstance(max_tools, bool) or not 1 <= max_tools <= 128:
            raise BrainRunError("max_tools must be between 1 and 128")
        needs = [
            {
                "id": f"stage-{stage.id}",
                "query": f"{blueprint.profile.domain} {stage.objective}: {blueprint.spec.task}",
                "max_items": max_tools,
                "domain_pack_id": blueprint.domain_pack.pack_id,
                "domain_pack_digest": blueprint.domain_pack.pack_digest,
                "required_tool_capabilities": list(blueprint.domain_pack.tool_capabilities),
            }
            for stage in blueprint.workflow.stages
        ]
        return {
            "goal": blueprint.spec.task,
            "needs": needs,
            "max_tools": max_tools,
            "include_tools": True,
        }

    def prepare_cross_domain(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        context: Mapping[str, Any] | None = None,
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
        max_input_tokens: int = 4_096,
    ) -> AutonomousCrossDomainBlueprint:
        """Prepare bounded fan-out/fan-in work without contacting a provider or tool."""

        _text("cross-domain task", task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        if not isinstance(subtasks, Sequence) or isinstance(subtasks, (str, bytes)):
            raise BrainRunError("cross-domain subtasks must be a sequence")
        if not 1 <= len(subtasks) <= 8:
            raise BrainRunError("cross-domain subtasks must contain between 1 and 8 items")
        if context is not None:
            _safe_json("cross-domain context", context)
        replan_context = None
        if context is not None and _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY in context:
            replan_context = context[_AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY]
            if not isinstance(replan_context, Mapping):
                raise BrainRunError("cross-domain replan context must be a mapping")
        parent_context = None if context is None else {
            key: value
            for key, value in context.items()
            if key != _AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY
        }
        parent_digest = content_digest({"task": task})
        children: list[AutonomousTaskBlueprint] = []
        child_ids: list[str] = []
        allowed = {
            "id", "task", "domain", "capability", "risk_class", "constraints", "desired_outputs",
            "context", "max_steps", "require_json", "structured_domain_response", "response_schema", "execution_mode",
            "required_model_capabilities",
        }
        for index, raw in enumerate(subtasks):
            if not isinstance(raw, Mapping):
                raise BrainRunError("cross-domain subtasks must contain mappings")
            unknown = sorted(set(raw).difference(allowed))
            if unknown:
                raise BrainRunError("cross-domain subtask contains unsupported fields: " + ", ".join(unknown))
            child_id = raw.get("id", f"child-{index + 1}")
            child_ids.append(_identifier("cross-domain child id", child_id))
            child_task = _text("cross-domain child task", raw.get("task"), maximum=MAX_AUTONOMY_TEXT_BYTES)
            child_context: dict[str, Any] = {
                "cross_domain_parent_digest": parent_digest,
                "cross_domain_child_id": child_id,
            }
            if context is not None:
                child_context["parent_context"] = dict(parent_context or {})
                if replan_context is not None:
                    child_context[_AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY] = dict(replan_context)
            raw_context = raw.get("context")
            if raw_context is not None:
                if not isinstance(raw_context, Mapping):
                    raise BrainRunError("cross-domain child context must be a mapping")
                child_context["child_context"] = dict(raw_context)
            child = self.prepare(
                task=child_task,
                domain=_identifier("cross-domain child domain", raw.get("domain")),
                capability=raw.get("capability"),
                risk_class=raw.get("risk_class"),
                constraints=raw.get("constraints", ()),
                desired_outputs=raw.get("desired_outputs", ()),
                context=child_context,
                max_steps=raw.get("max_steps", max_steps),
                require_json=raw.get("require_json", require_json),
                structured_domain_response=raw.get("structured_domain_response", structured_domain_response),
                response_schema=raw.get("response_schema"),
                execution_mode=raw.get("execution_mode", child_execution_mode),
                max_input_tokens=max_input_tokens,
                required_model_capabilities=raw.get("required_model_capabilities", ()),
            )
            children.append(child)
        synthesis_context = {
            "cross_domain_parent_digest": parent_digest,
            "children": [
                {
                    "id": child_id,
                    "domain": child.profile.domain,
                    "capability": child.spec.capability,
                    "task_digest": child.spec.task_digest,
                    "workflow_id": child.workflow.workflow_id,
                    "workflow_digest": child.workflow.workflow_digest,
                    "stage_ids": [stage.id for stage in child.workflow.stages],
                }
                for child_id, child in zip(child_ids, children)
            ],
        }
        if context is not None:
            synthesis_context["parent_context"] = dict(parent_context or {})
            if replan_context is not None:
                synthesis_context[_AUTONOMOUS_CROSS_DOMAIN_REPLAN_CONTEXT_KEY] = dict(replan_context)
        synthesis = self.prepare(
            task=f"Synthesize the domain analyses for: {task}",
            domain="cross_domain",
            capability="cross_domain_synthesis",
            desired_outputs=desired_outputs,
            context=synthesis_context,
            max_steps=max_steps,
            require_json=require_json,
            structured_domain_response=structured_domain_response,
            response_schema=response_schema,
            execution_mode=synthesis_execution_mode,
            max_input_tokens=max_input_tokens,
        )
        return AutonomousCrossDomainBlueprint(
            task_digest=parent_digest,
            child_blueprints=tuple(children),
            synthesis_blueprint=synthesis,
            child_ids=tuple(child_ids),
            task=task,
        )

    @staticmethod
    def _memory(
        brain: AutonomousBrain,
        memory: BrainEpisodicMemory | None,
        memory_query: MemoryQuery | Mapping[str, Any] | None,
        memory_limit: int,
        *,
        task: str,
        domain: str,
        capability: str | None,
        risk_class: str | None,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> tuple[BrainEpisodicMemory | None, tuple[Mapping[str, Any], ...]]:
        store = memory if memory is not None else brain.memory
        if store is None:
            return None, ()
        if not isinstance(store, BrainEpisodicMemory):
            raise BrainRunError("memory must be a BrainEpisodicMemory or None")
        if not isinstance(memory_limit, int) or isinstance(memory_limit, bool) or not 1 <= memory_limit <= MAX_AUTONOMY_MEMORY_ITEMS:
            raise BrainRunError(f"memory_limit must be between 1 and {MAX_AUTONOMY_MEMORY_ITEMS}")
        query = memory_query
        if query is None:
            query = MemoryQuery(
                domain=domain,
                capability=capability,
                risk_class=risk_class,
                task_facets=task_facet_digests(task),
                limit=memory_limit,
            )
        elif isinstance(query, Mapping):
            # Explicit filters remain caller-owned, but an ordinary metadata query should not
            # silently fall back to unrelated recent episodes.  Exact task-digest/facet queries
            # are left untouched; otherwise add local digest-only facets as a relevance gate.
            normalized_query = dict(query)
            if "task_digest" not in normalized_query and "task_facets" not in normalized_query:
                normalized_query["task_facets"] = list(task_facet_digests(task))
            query = normalized_query
        elif isinstance(query, MemoryQuery) and query.task_digest is None and not query.task_facets:
            query = MemoryQuery(
                domain=query.domain,
                capability=query.capability,
                risk_class=query.risk_class,
                task_facets=task_facet_digests(task),
                tags=query.tags,
                statuses=query.statuses,
                include_failed=query.include_failed,
                limit=query.limit,
            )
        try:
            episodes = tuple(
                brain.recall_memory(
                    query,
                    limit=memory_limit,
                    memory=store,
                    authorization_context=authorization_context,
                    authorization_domain=domain,
                    authorization_capability=capability,
                    authorization_risk_class=risk_class,
                )
            )
        except BrainMemoryError as error:
            raise BrainRunError("autonomous memory retrieval failed") from error
        return store, episodes

    @staticmethod
    def _authorize_memory_evaluation(
        authorization_context: AutonomousAuthorizationContext | None,
        *,
        domain: str,
        episode_id: str,
        decision_digest: str,
        trajectory_id: str | None = None,
        trajectory_step: int | None = None,
    ) -> None:
        """Authorize one metadata-only evaluation write immediately before persistence.

        Episode writes and evaluation writes are separate durable operations.  Keeping this
        check at the final write boundary prevents workflow, mission, and cross-domain helpers
        from accidentally bypassing the same caller-issued memory grant used by ``remember_result``.
        """

        if authorization_context is None:
            return
        authorization_context.authorize_operation(
            operation="memory_write",
            domain=domain,
            resource_digest=content_digest(
                {
                    "schema": "bioprism-autonomous-memory-evaluation-authorization-resource/0.1",
                    "episode_id": episode_id,
                    "decision_digest": decision_digest,
                    "trajectory_id": trajectory_id,
                    "trajectory_step": trajectory_step,
                }
            ),
        )

    @staticmethod
    def _consolidated_memory(
        consolidator: AutonomousMemoryConsolidator | None,
        lesson_resolver: Callable[[str], str | None] | None,
        lesson_context_resolver: Callable[[Mapping[str, Any]], str | None] | None,
        *,
        domains: Sequence[str],
        capability: str | None,
        limit: int,
        retrieve: bool,
        required: bool,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> tuple[Mapping[str, Any], ...]:
        """Resolve stable lesson digests into transient, domain-scoped prompt references."""

        if not isinstance(retrieve, bool) or not isinstance(required, bool):
            raise BrainRunError("consolidated memory retrieve/required flags must be booleans")
        if not isinstance(limit, int) or isinstance(limit, bool) or not 1 <= limit <= MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS:
            raise BrainRunError(
                f"consolidated_memory_limit must be between 1 and {MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS}"
            )
        if lesson_resolver is not None and not callable(lesson_resolver):
            raise BrainRunError("memory_lesson_resolver must be callable or None")
        if lesson_context_resolver is not None and not callable(lesson_context_resolver):
            raise BrainRunError("memory_lesson_context_resolver must be callable or None")
        if lesson_resolver is not None and lesson_context_resolver is not None:
            raise BrainRunError("memory_lesson_resolver and memory_lesson_context_resolver are mutually exclusive")
        requested = retrieve and consolidator is not None and (lesson_resolver is not None or lesson_context_resolver is not None)
        if required and not requested:
            raise BrainRunError(
                "consolidated_memory_required needs a consolidator and one lesson resolver"
            )
        if not requested:
            return ()
        references: dict[tuple[str, str], Mapping[str, Any]] = {}
        try:
            for domain in dict.fromkeys(domains):
                if authorization_context is not None:
                    authorization_context.authorize_operation(
                        operation="memory_retrieval",
                        domain=domain,
                        resource_digest=content_digest(
                            {
                                "schema": "bioprism-autonomous-consolidated-memory-authorization-resource/0.1",
                                "domain": domain,
                                "capability": capability,
                                "limit": limit,
                                "resolver": "lesson_context" if lesson_context_resolver is not None else "lesson",
                            }
                        ),
                    )
                for reference in consolidator.prompt_references(
                    domain=domain,
                    capability=capability,
                    lesson_resolver=lesson_resolver,
                    lesson_context_resolver=lesson_context_resolver,
                    limit=limit,
                ):
                    key = (str(reference["lesson_id"]), str(reference["lesson_digest"]))
                    references.setdefault(key, dict(reference))
        except AutonomousAuthorizationError:
            raise
        except Exception as error:
            raise BrainRunError("autonomous consolidated memory retrieval failed") from error
        return tuple(list(references.values())[:MAX_AUTONOMOUS_MEMORY_CONSOLIDATION_PROMPT_LESSONS])

    @staticmethod
    def _merge_options(
        options: Mapping[str, Any] | None,
        *,
        context: Mapping[str, Any],
        content_parts: Sequence[Mapping[str, Any]] | None,
        required_capabilities: Sequence[str],
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
        route_request: Mapping[str, Any] | None,
        enforce_route_tools: bool,
        require_resolved_route: bool,
        provider_tools: Sequence[ProviderTool],
        tool_choice: str | None,
        max_provider_failovers: int,
        authorization_context: AutonomousAuthorizationContext | None,
        authorization_domain: str,
        reserve_cost: AutonomousCostReservationCallback | None,
    ) -> dict[str, Any]:
        if options is not None and not isinstance(options, Mapping):
            raise BrainRunError("mission_options must be a mapping or None")
        merged = {} if options is None else dict(options)
        forbidden = {"task", "model_candidates", "prompt", "plan", "credentials", "mission_policy", "trace_event_callback"}
        unknown = sorted(forbidden.intersection(merged))
        if unknown:
            raise BrainRunError("mission_options cannot override generated fields: " + ", ".join(unknown))
        generated = {
            "context": dict(context),
            "content_parts": None if content_parts is None else tuple(dict(part) for part in content_parts),
            "contextual_observations": [dict(item) for item in contextual_observations],
            "required_capabilities": list(required_capabilities),
            "input_tokens": input_tokens,
            "context_budget": context_budget,
            "requested_output_tokens": requested_output_tokens,
            "approve_provider_call": approve_provider_call,
            "approve_mission_dispatch": approve_mission_dispatch,
            "run_id": run_id,
            "max_output_tokens": max_output_tokens,
            "response_schema": None if response_schema is None else dict(response_schema),
            "idempotency_key": idempotency_key,
            "route_request": None if route_request is None else dict(route_request),
            "enforce_route_tools": enforce_route_tools,
            "require_resolved_route": require_resolved_route,
            "provider_tools": tuple(provider_tools),
            "tool_choice": tool_choice,
            "max_provider_failovers": max_provider_failovers,
            "authorization_context": authorization_context,
            "authorization_domain": authorization_domain,
            "reserve_cost": reserve_cost,
        }
        for name, value in (
            ("max_cost_per_million_tokens", max_cost_per_million_tokens),
            ("max_latency_ms", max_latency_ms),
            ("min_quality", min_quality),
            ("selection_overrides", selection_overrides),
        ):
            if value is not None:
                generated[name] = value
        merged.update(generated)
        return merged

    @classmethod
    def _attach_domain_response_evaluation(
        cls,
        blueprint: AutonomousTaskBlueprint,
        result: Any,
    ) -> Any:
        """Validate and score an opt-in structured response at the provider boundary."""

        contract = blueprint.response_contract
        if contract is None:
            return result
        if isinstance(result, BrainLearningCycleResult):
            attempts = tuple(cls._attach_domain_response_evaluation(blueprint, attempt) for attempt in result.attempts)
            final_result = cls._attach_domain_response_evaluation(blueprint, result.final_result)
            return replace(result, attempts=attempts, final_result=final_result)
        response = cls._workflow_provider_response(result)
        normalized = validate_autonomous_provider_domain_response(response, contract)
        if normalized is None:  # pragma: no cover - the contract branch requires a value
            raise BrainRunError("structured domain response validation returned no response")
        evaluation = evaluate_autonomous_domain_response(normalized.to_dict(), contract).to_dict()
        if isinstance(result, BrainRunResult):
            return replace(result, response_evaluation=evaluation)
        if isinstance(result, BrainToolLoopResult):
            return replace(
                result,
                brain_run=replace(result.brain_run, response_evaluation=evaluation),
            )
        return replace(result, brain_run=replace(result.brain_run, response_evaluation=evaluation))

    @staticmethod
    def _apply_direct_response_review_gate(
        result: Any,
        *,
        require_response_review: bool,
    ) -> Any:
        """Project a failed structural response into an explicit caller-review state.

        Provider transport success is deliberately retained in the response and evaluation
        metadata.  The status only communicates that the answer did not earn admission as a
        completed autonomous result; callers can opt out when they need the legacy projection.
        """

        if not require_response_review:
            return result
        evaluation = _structured_response_evaluation(result)
        if not isinstance(evaluation, Mapping) or evaluation.get("passed") is not False:
            return result
        status = getattr(result, "status", None)
        if not isinstance(status, str) or not status.startswith("completed"):
            return result
        return replace(result, status="response_review_required")

# Keep methods discoverable through the public AutonomousTaskOrchestrator facade.
for _name, _descriptor in vars(AutonomousOrchestratorPreparationMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousTaskOrchestrator.{_name}"
