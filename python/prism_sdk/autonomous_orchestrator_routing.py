"""Focused autonomous-orchestrator routing methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_CROSS_DOMAIN_PLAN_REFINEMENT_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA,
    AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
    AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA,
    Any,
    AutonomousAuthorizationContext,
    AutonomousCrossDomainBlueprint,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousDomainPolicy,
    AutonomousDomainPolicyAdmission,
    AutonomousOrderedStepPlanRefinementResult,
    AutonomousPlanRefinementResult,
    AutonomousPromptLearningState,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    AutonomousRouteCandidate,
    AutonomousRouteProposal,
    AutonomousSemanticRouteCandidate,
    AutonomousSemanticRouteResult,
    AutonomousTaskBlueprint,
    BrainLearningLedger,
    BrainRunError,
    CredentialError,
    CredentialHandle,
    MAX_AUTONOMY_TEXT_BYTES,
    Mapping,
    ProviderError,
    Sequence,
    _apply_versioned_prompt,
    _cross_domain_plan_digest,
    _cross_domain_plan_response_schema,
    _identifier,
    _json_digest,
    _normalize_ordered_step_graph,
    _normalize_planning_failure,
    _ordered_step_ids,
    _ordered_step_plan_response_schema,
    _plan_refinement_response_schema,
    _planner_adaptive_prompt_selection,
    _planner_prompt_digest,
    _planning_domain_policy_admission,
    _planning_failure_projection,
    _provider_planner_context,
    _route_digest,
    _semantic_route_response_schema,
    _text,
    autonomous_domain_policy,
    content_digest,
    evaluate_autonomous_domain_policy,
    math,
)

class AutonomousOrchestratorRoutingMixin:
    def route_task(self, *, task: str, **kwargs: Any) -> AutonomousRouteProposal:
        """Route an unclassified task without contacting a provider or executing a tool."""

        return self.router.route(task, **kwargs)

    def domain_policy(self, domain: str, overrides: Mapping[str, Any] | None = None) -> AutonomousDomainPolicy:
        """Resolve one domain's bounded provider-free execution policy."""

        return autonomous_domain_policy(domain, overrides)

    def admit_domain_policy(
        self,
        domain: str,
        **kwargs: Any,
    ) -> AutonomousDomainPolicyAdmission:
        """Explain pre-provider policy gates without contacting a provider or source."""

        return evaluate_autonomous_domain_policy(self.domain_policy(domain), **kwargs)

    def route_with_provider(
        self,
        *,
        task: str,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        hints: Sequence[str] = (),
        context: Mapping[str, Any] | None = None,
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
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> AutonomousSemanticRouteResult:
        """Use one approved provider call to improve routing, then reconcile it with the catalogue.

        The provider sees the transient task and reviewed route catalogue, but its output is only
        a classification proposal. Every domain score is bounded, every domain must be returned,
        and the final route is derived from a deterministic/semantic score fusion. Malformed,
        abstaining, or contradictory provider output never creates an executable blueprint and
        falls back to the provider-free route.
        """

        deterministic = self.route_task(
            task=task,
            hints=hints,
            min_confidence=min_confidence,
            min_margin=min_margin,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
        )
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("semantic route model_candidates must be a sequence")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("semantic route credentials must be a mapping")
        if any(
            not isinstance(provider, str)
            or not isinstance(handle, CredentialHandle)
            or provider != handle.provider
            for provider, handle in credentials.items()
        ):
            raise BrainRunError("semantic route credentials must map providers to matching handles")
        if isinstance(semantic_weight, bool) or not isinstance(semantic_weight, (int, float)):
            raise BrainRunError("semantic_weight must be within [0, 1]")
        if not math.isfinite(float(semantic_weight)) or not 0.0 <= float(semantic_weight) <= 1.0:
            raise BrainRunError("semantic_weight must be within [0, 1]")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("semantic route contextual_observations must be a sequence")
        if context is not None and not isinstance(context, Mapping):
            raise BrainRunError("semantic route context must be a mapping or None")
        route_schema = _semantic_route_response_schema()
        classifier_task = (
            "Classify the following user request against the reviewed AURORA autonomous domain "
            "catalogue. Return only the required JSON object. Do not execute tools, invent a "
            "domain, or treat classification as authorization. Score every catalogue domain "
            "from 0 to 1, select at most the requested number of domains, and abstain when the "
            "request is genuinely ambiguous. User request:\n\n"
            + task
        )
        _text("semantic route classifier task", classifier_task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        classifier_context: dict[str, Any] = {
            "route_catalogue": self.router.catalogue(),
            "deterministic_route": deterministic.to_dict(),
            "semantic_route_contract": {
                "schema": AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA,
                "max_selected_domains": max_domains,
                "semantic_scores_are_proposals": True,
                "does_not_authorize": ["provider access", "tools", "external effects", "truth claims"],
            },
        }
        if context is not None:
            BrainLearningLedger._assert_safe(context)
            classifier_context["caller_context"] = dict(context)
        blueprint = self.prepare(
            task=classifier_task,
            domain="cross_domain",
            capability="routing",
            context=classifier_context,
            desired_outputs=("domain scores", "selected domains", "abstention decision"),
            require_json=True,
            response_schema=route_schema,
            max_input_tokens=input_tokens,
        )
        domain_policy_admission = _planning_domain_policy_admission(
            domain="cross_domain",
            mode=domain_policy_mode,
            estimated_input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            evidence_ready=domain_policy_evidence_ready,
            evaluator_configured=domain_policy_evaluator_configured,
            effects_requested=domain_policy_effects_requested,
            effects_approved=domain_policy_effects_approved,
        )
        if domain_policy_admission is not None and domain_policy_admission.decision != "admitted":
            return AutonomousSemanticRouteResult(
                status=(
                    "policy_blocked"
                    if domain_policy_admission.decision == "blocked"
                    else "policy_review_required"
                ),
                route=deterministic,
                deterministic_route=deterministic,
                domain_policy_admission=domain_policy_admission,
            )
        if approve_provider_call is not True:
            return AutonomousSemanticRouteResult(
                status="approval_required",
                route=deterministic,
                deterministic_route=deterministic,
                domain_policy_admission=domain_policy_admission,
            )
        selection_request = self.brain.build_adaptive_model_selection(
            task=classifier_task,
            model_candidates=model_candidates,
            credentials=credentials,
            bandit_state=bandit_state,
            context=blueprint.selection_context,
            contextual_observations=contextual_observations,
            required_capabilities=("reasoning",),
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            selection_weights=selection_weights,
            selection_observations=selection_observations,
        )
        run = self.brain.run(
            task=classifier_task,
            model_selection=selection_request,
            prompt=blueprint.prompt,
            plan=blueprint.plan,
            credentials=credentials,
            approve_provider_call=approve_provider_call,
            run_id=run_id,
            max_output_tokens=max_output_tokens,
            temperature=temperature,
            require_json=True,
            response_schema=route_schema,
            context=blueprint.selection_context,
            contextual_observations=contextual_observations,
            authorization_context=authorization_context,
            authorization_domain="cross_domain",
        )
        selection = run.selection
        selected_model = selection.get("selected_model")
        safe_model = None
        if isinstance(selected_model, Mapping) and isinstance(selected_model.get("provider"), str) and isinstance(selected_model.get("model"), str):
            safe_model = {"provider": selected_model["provider"], "model": selected_model["model"]}
        plan_value = run.plan.get("plan")
        plan_digest = plan_value.get("plan_digest") if isinstance(plan_value, Mapping) else None
        metadata = {
            "selected_model": safe_model,
            "selection_digest": selection.get("decision_digest"),
            "prompt_digest": run.prompt.get("prompt_digest"),
            "plan_digest": plan_digest,
            "outcome_digest": run.outcome_digest,
            "domain_policy_admission": domain_policy_admission,
        }
        if run.status != "completed_provider_call" or run.response is None:
            return AutonomousSemanticRouteResult(
                status=run.status if run.status in {"approval_required", "plan_refused"} else "provider_invalid",
                route=deterministic,
                deterministic_route=deterministic,
                **metadata,
            )
        raw = run.response.structured
        if not isinstance(raw, Mapping):
            return AutonomousSemanticRouteResult(
                status="provider_invalid",
                route=deterministic,
                deterministic_route=deterministic,
                **metadata,
            )
        raw_candidates = raw.get("candidates")
        raw_selected = raw.get("selected_domains")
        confidence = raw.get("confidence")
        abstain = raw.get("abstain")
        if (
            not isinstance(raw_candidates, list)
            or len(raw_candidates) != len(AUTONOMOUS_DOMAINS)
            or not isinstance(raw_selected, list)
            or not isinstance(confidence, (int, float))
            or isinstance(confidence, bool)
            or not math.isfinite(float(confidence))
            or not 0.0 <= float(confidence) <= 1.0
            or not isinstance(abstain, bool)
        ):
            return AutonomousSemanticRouteResult(
                status="provider_invalid",
                route=deterministic,
                deterministic_route=deterministic,
                **metadata,
            )
        semantic_scores: dict[str, float] = {}
        for raw_candidate in raw_candidates:
            if not isinstance(raw_candidate, Mapping):
                semantic_scores = {}
                break
            domain = raw_candidate.get("domain")
            score = raw_candidate.get("score")
            if (
                not isinstance(domain, str)
                or domain not in AUTONOMOUS_DOMAINS
                or domain in semantic_scores
                or not isinstance(score, (int, float))
                or isinstance(score, bool)
                or not math.isfinite(float(score))
                or not 0.0 <= float(score) <= 1.0
            ):
                semantic_scores = {}
                break
            semantic_scores[domain] = float(score)
        semantic_selected = tuple(raw_selected)
        if (
            len(semantic_scores) != len(AUTONOMOUS_DOMAINS)
            or any(not isinstance(domain, str) for domain in semantic_selected)
            or any(domain not in AUTONOMOUS_DOMAINS for domain in semantic_selected)
            or len(semantic_selected) > max_domains
            or len(set(semantic_selected)) != len(semantic_selected)
        ):
            return AutonomousSemanticRouteResult(
                status="provider_invalid",
                route=deterministic,
                deterministic_route=deterministic,
                **metadata,
            )
        semantic_candidates = tuple(
            AutonomousSemanticRouteCandidate(
                domain=domain,
                semantic_score=semantic_scores[domain],
                deterministic_score=next(
                    (candidate.score for candidate in deterministic.candidates if candidate.domain == domain),
                    0.0,
                ),
                combined_score=min(
                    1.0,
                    float(semantic_weight) * semantic_scores[domain]
                    + (1.0 - float(semantic_weight)) * next(
                        (candidate.score for candidate in deterministic.candidates if candidate.domain == domain),
                        0.0,
                    ),
                ),
            )
            for domain in AUTONOMOUS_DOMAINS
        )
        ranked = tuple(sorted(semantic_candidates, key=lambda candidate: (-candidate.combined_score, candidate.domain)))
        if abstain:
            return AutonomousSemanticRouteResult(
                status="provider_abstained",
                route=deterministic,
                deterministic_route=deterministic,
                semantic_candidates=ranked,
                semantic_selected_domains=semantic_selected,
                semantic_confidence=confidence,
                **metadata,
            )
        top = ranked[0]
        second = ranked[1]
        if top.domain not in semantic_selected:
            return AutonomousSemanticRouteResult(
                status="provider_disagreement",
                route=deterministic,
                deterministic_route=deterministic,
                semantic_candidates=ranked,
                semantic_selected_domains=semantic_selected,
                semantic_confidence=confidence,
                **metadata,
            )
        if top.combined_score < float(min_confidence):
            route = AutonomousRouteProposal(
                task_digest=deterministic.task_digest,
                candidates=tuple(
                    AutonomousRouteCandidate(
                        domain=domain,
                        score=candidate.combined_score,
                        matched_terms=next(
                            (item.matched_terms for item in deterministic.candidates if item.domain == domain),
                            (),
                        ),
                        capability=self.registry.resolve(domain).default_capability,
                        risk_class=self.registry.resolve(domain).risk_class,
                        workflow_id=self.workflow_registry.resolve(domain).workflow_id,
                        evidence="hybrid_deterministic_and_provider_semantic_scores",
                    )
                    for domain, candidate in ((item.domain, item) for item in ranked)
                ),
                selected_domains=(),
                confidence=top.combined_score,
                abstained=True,
                reason="insufficient_confidence",
                source="provider_semantic_hybrid",
            )
            return AutonomousSemanticRouteResult(
                status="completed",
                route=route,
                deterministic_route=deterministic,
                semantic_candidates=ranked,
                semantic_selected_domains=semantic_selected,
                semantic_confidence=confidence,
                **metadata,
            )
        selected: tuple[str, ...]
        if top.combined_score - second.combined_score < float(min_margin):
            eligible = tuple(
                candidate.domain
                for candidate in ranked
                if candidate.combined_score >= float(min_confidence)
                and candidate.combined_score >= top.combined_score - float(min_margin)
            )[:max_domains]
            selected = eligible if allow_cross_domain and len(eligible) > 1 else ()
            if selected and any(domain not in semantic_selected for domain in selected):
                return AutonomousSemanticRouteResult(
                    status="provider_disagreement",
                    route=deterministic,
                    deterministic_route=deterministic,
                    semantic_candidates=ranked,
                    semantic_selected_domains=semantic_selected,
                    semantic_confidence=confidence,
                    **metadata,
                )
            if not selected:
                route = AutonomousRouteProposal(
                    task_digest=deterministic.task_digest,
                    candidates=tuple(
                        AutonomousRouteCandidate(
                            domain=domain,
                            score=candidate.combined_score,
                            matched_terms=next(
                                (item.matched_terms for item in deterministic.candidates if item.domain == domain),
                                (),
                            ),
                            capability=self.registry.resolve(domain).default_capability,
                            risk_class=self.registry.resolve(domain).risk_class,
                            workflow_id=self.workflow_registry.resolve(domain).workflow_id,
                            evidence="hybrid_deterministic_and_provider_semantic_scores",
                        )
                        for domain, candidate in ((item.domain, item) for item in ranked)
                    ),
                    selected_domains=(),
                    confidence=top.combined_score,
                    abstained=True,
                    reason="insufficient_margin",
                    source="provider_semantic_hybrid",
                )
                return AutonomousSemanticRouteResult(
                    status="completed",
                    route=route,
                    deterministic_route=deterministic,
                    semantic_candidates=ranked,
                    semantic_selected_domains=semantic_selected,
                    semantic_confidence=confidence,
                    **metadata,
                )
        else:
            selected = (top.domain,)
        hybrid_candidates = tuple(
            AutonomousRouteCandidate(
                domain=domain,
                score=candidate.combined_score,
                matched_terms=next(
                    (item.matched_terms for item in deterministic.candidates if item.domain == domain),
                    (),
                ),
                capability=self.registry.resolve(domain).default_capability,
                risk_class=self.registry.resolve(domain).risk_class,
                workflow_id=self.workflow_registry.resolve(domain).workflow_id,
                evidence="hybrid_deterministic_and_provider_semantic_scores",
            )
            for domain, candidate in ((item.domain, item) for item in ranked)
        )
        route = AutonomousRouteProposal(
            task_digest=deterministic.task_digest,
            candidates=hybrid_candidates,
            selected_domains=selected,
            confidence=top.combined_score,
            abstained=False,
            reason="cross_domain" if len(selected) > 1 else "routed",
            cross_domain=len(selected) > 1,
            source="provider_semantic_hybrid",
        )
        return AutonomousSemanticRouteResult(
            status="completed",
            route=route,
            deterministic_route=deterministic,
            semantic_candidates=ranked,
            semantic_selected_domains=semantic_selected,
            semantic_confidence=confidence,
            **metadata,
        )

    def plan_ordered_steps_with_provider(
        self,
        *,
        task: str,
        steps: Sequence[Mapping[str, Any]],
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        domain: str | None = None,
        capability: str | None = None,
        protected_contract_digest: str | None = None,
        context: Mapping[str, Any] | None = None,
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
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str = "planning",
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> AutonomousOrderedStepPlanRefinementResult:
        """Ask a provider to order an existing dependency-closed graph without authorizing it.

        This is deliberately separate from workflow-stage refinement: mission schedulers and
        application-owned planners can submit arbitrary bounded step metadata while keeping
        tools, arguments, credentials, permissions, claims, and effects outside the provider
        proposal. The returned ordering is never dispatched by this method.
        """

        task_text = _text("ordered-step planning task", task, maximum=32_000)
        graph = _normalize_ordered_step_graph(steps)
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("ordered-step planning model_candidates must be a sequence")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("ordered-step planning credentials must be a mapping")
        if any(
            not isinstance(provider, str)
            or not isinstance(handle, CredentialHandle)
            or provider != handle.provider
            for provider, handle in credentials.items()
        ):
            raise BrainRunError("ordered-step planning credentials must map providers to matching handles")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("ordered-step planning contextual_observations must be a sequence")
        if context is not None:
            BrainLearningLedger._assert_safe(context)
        step_ids = tuple(step["id"] for step in graph)
        step_domains = tuple(dict.fromkeys(step["domain"] for step in graph))
        resolved_domain = domain or (step_domains[0] if len(step_domains) == 1 else "cross_domain")
        if resolved_domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError(f"ordered-step planning has an unsupported domain: {resolved_domain!r}")
        resolved_capability = "planning" if capability is None else _identifier(
            "ordered-step planning capability",
            capability,
        )
        if protected_contract_digest is not None:
            _route_digest(
                protected_contract_digest,
                "ordered-step planning protected_contract_digest",
            )
        task_digest = content_digest({"task": task_text})
        base_plan_digest = content_digest({"steps": list(graph)})
        planner_task = (
            "Propose a bounded ordering and focus refinement for the reviewed step graph. Return only "
            "the required JSON object. Use every existing step identifier exactly once in "
            "priority_order. Preserve dependency order. Do not add, remove, rewrite, authorize, or "
            "execute tools, arguments, credentials, permissions, effects, claims, or external writes. "
            "Mark review_required when a human should inspect the proposal. Original task:\n\n"
            + task_text
        )
        _text("ordered-step planning prompt", planner_task, maximum=32_000)
        planner_context: dict[str, Any] = {
            "planning_contract": {
                "schema": AUTONOMOUS_ORDERED_STEP_PLAN_REFINEMENT_SCHEMA,
                "task_digest": task_digest,
                "base_plan_digest": base_plan_digest,
                "protected_contract_digest": protected_contract_digest,
                "step_catalogue": [dict(step) for step in graph],
                "reconciliation": (
                    "priority_order_must_contain_each_existing_step_exactly_once_and_respect_dependencies"
                ),
                "does_not_authorize": [
                    "tools",
                    "arguments",
                    "credentials",
                    "permissions",
                    "effects",
                    "claims",
                    "external_writes",
                ],
            },
            "base_plan_metadata": {
                "domain": resolved_domain,
                "step_count": len(graph),
                "step_domains": list(step_domains),
            },
        }
        if context is not None:
            planner_context["caller_context"] = dict(context)
        response_schema = _ordered_step_plan_response_schema(step_ids)
        planner_blueprint = self.prepare(
            task=planner_task,
            domain=resolved_domain,
            capability=resolved_capability,
            context=planner_context,
            desired_outputs=("dependency-closed step priority", "focus steps", "review decision"),
            max_steps=min(128, max(1, len(graph))),
            require_json=True,
            response_schema=response_schema,
            execution_mode="provider",
            max_input_tokens=input_tokens,
            required_model_capabilities=("structured_output",),
        )
        planner_blueprint = _apply_versioned_prompt(
            planner_blueprint,
            route=None,
            prompt_template=prompt_template,
            prompt_registry=prompt_registry,
            prompt_selection=prompt_selection,
            prompt_stage=prompt_stage,
            prompt_learning_state=prompt_learning_state,
            prompt_learning_exploration=prompt_learning_exploration,
        )
        planner_prompt_digest = _planner_prompt_digest(planner_blueprint)
        adaptive_selection = _planner_adaptive_prompt_selection(planner_blueprint, prompt_registry)
        planner_learning_context, planner_selection_context, planner_learning_context_digest = _provider_planner_context(
            planner_blueprint.selection_context,
            task_family="ordered_step_plan",
        )
        domain_policy_admission = _planning_domain_policy_admission(
            domain=resolved_domain,
            mode=domain_policy_mode,
            estimated_input_tokens=input_tokens,
            requested_output_tokens=max_output_tokens,
            evidence_ready=domain_policy_evidence_ready,
            evaluator_configured=domain_policy_evaluator_configured,
            effects_requested=domain_policy_effects_requested,
            effects_approved=domain_policy_effects_approved,
        )
        base_kwargs: dict[str, Any] = {
            "task_digest": task_digest,
            "base_plan_digest": base_plan_digest,
            "protected_contract_digest": protected_contract_digest,
            "planner_prompt_digest": planner_prompt_digest,
            "adaptive_selection": adaptive_selection,
            "planner_context": planner_learning_context,
            "planner_context_digest": planner_learning_context_digest,
            "domain_policy_admission": domain_policy_admission,
        }
        if domain_policy_admission is not None and domain_policy_admission.decision != "admitted":
            return AutonomousOrderedStepPlanRefinementResult(
                status=(
                    "policy_blocked"
                    if domain_policy_admission.decision == "blocked"
                    else "policy_review_required"
                ),
                **base_kwargs,
            )
        if domain_policy_mode == "strict" and approve_provider_call is not True:
            return AutonomousOrderedStepPlanRefinementResult(
                status="approval_required",
                **base_kwargs,
            )

        if authorization_context is not None:
            authorization_context.authorize_operation(
                operation="plan",
                domain=resolved_domain,
                resource_digest=content_digest({
                    "schema": "bioprism-autonomous-plan-authorization-resource/0.1",
                    "domain": resolved_domain,
                    "task_digest": task_digest,
                    "base_plan_digest": base_plan_digest,
                    "planner_prompt_digest": _planner_prompt_digest(planner_blueprint),
                }),
            )

        selection_request: Mapping[str, Any] | None = None
        try:
            selection_request = self.brain.build_adaptive_model_selection(
                task=planner_task,
                model_candidates=model_candidates,
                credentials=credentials,
                bandit_state=bandit_state,
                context=planner_selection_context,
                contextual_observations=contextual_observations,
                required_capabilities=planner_blueprint.required_capabilities,
                input_tokens=input_tokens,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                selection_weights=selection_weights,
                selection_observations=selection_observations,
            )
            run = self.brain.run(
                task=planner_task,
                model_selection=selection_request,
                prompt=planner_blueprint.prompt,
                plan=planner_blueprint.plan,
                credentials=credentials,
                approve_provider_call=approve_provider_call,
                run_id=run_id,
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                require_json=True,
                response_schema=response_schema,
                context=planner_selection_context,
                contextual_observations=contextual_observations,
                authorization_context=authorization_context,
                authorization_domain=resolved_domain,
            )
        except (ProviderError, CredentialError) as error:
            failure = _planning_failure_projection(error)
            selected = None if selection_request is None else selection_request.get("selected_model")
            safe_model = (
                {"provider": selected["provider"], "model": selected["model"]}
                if isinstance(selected, Mapping)
                and isinstance(selected.get("provider"), str)
                and isinstance(selected.get("model"), str)
                else None
            )
            selection_digest = (
                None
                if selection_request is None or not isinstance(selection_request.get("decision_digest"), str)
                else selection_request["decision_digest"]
            )
            return AutonomousOrderedStepPlanRefinementResult(
                status="provider_failed",
                selected_model=safe_model,
                selection_digest=selection_digest,
                outcome_digest=_json_digest(
                    {
                        "status": "provider_failed",
                        "failure": failure,
                        "task_digest": task_digest,
                        "base_plan_digest": base_plan_digest,
                        "planner_context_digest": planner_learning_context_digest,
                        "planner_prompt_digest": planner_prompt_digest,
                    }
                ),
                failure=failure,
                **base_kwargs,
            )

        selected_model = run.selection.get("selected_model")
        safe_model = None
        if isinstance(selected_model, Mapping) and isinstance(selected_model.get("provider"), str) and isinstance(
            selected_model.get("model"), str
        ):
            safe_model = {"provider": selected_model["provider"], "model": selected_model["model"]}
        planner_plan_value = run.plan.get("plan")
        planner_plan_digest = (
            planner_plan_value.get("plan_digest")
            if isinstance(planner_plan_value, Mapping)
            else None
        )
        metadata: dict[str, Any] = {
            **base_kwargs,
            "selected_model": safe_model,
            "selection_digest": run.selection.get("decision_digest"),
            "planner_plan_digest": planner_plan_digest,
            "outcome_digest": _json_digest(
                {
                    "provider_outcome_digest": run.outcome_digest,
                    "learning_context_digest": planner_learning_context_digest,
                    "planner_prompt_digest": planner_prompt_digest,
                }
            ),
        }
        if run.status == "provider_failed":
            if not isinstance(run.failure, Mapping):
                raise BrainRunError("provider_failed ordered-step planning result did not contain a failure projection")
            metadata["failure"] = _normalize_planning_failure(run.failure, "ordered-step plan refinement")
        if run.status != "completed_provider_call" or run.response is None:
            return AutonomousOrderedStepPlanRefinementResult(
                status=(
                    run.status
                    if run.status in {"approval_required", "plan_refused", "provider_failed"}
                    else "provider_invalid"
                ),
                **metadata,
            )
        raw = run.response.structured
        if not isinstance(raw, Mapping):
            return AutonomousOrderedStepPlanRefinementResult(status="provider_invalid", **metadata)
        priority = raw.get("priority_order")
        focus = raw.get("focus_step_ids")
        review_required = raw.get("review_required")
        confidence = raw.get("confidence")
        abstain = raw.get("abstain")
        if (
            not isinstance(priority, list)
            or not isinstance(focus, list)
            or not isinstance(review_required, bool)
            or not isinstance(confidence, (int, float))
            or isinstance(confidence, bool)
            or not math.isfinite(float(confidence))
            or not 0.0 <= float(confidence) <= 1.0
            or not isinstance(abstain, bool)
        ):
            return AutonomousOrderedStepPlanRefinementResult(status="provider_invalid", **metadata)
        try:
            priority_ids = _ordered_step_ids(
                "ordered-step provider priority_order",
                priority,
            )
            focus_ids = _ordered_step_ids(
                "ordered-step provider focus_step_ids",
                focus,
            )
        except BrainRunError:
            return AutonomousOrderedStepPlanRefinementResult(status="provider_invalid", **metadata)
        if len(priority_ids) != len(step_ids) or set(priority_ids) != set(step_ids):
            return AutonomousOrderedStepPlanRefinementResult(status="provider_invalid", **metadata)
        if any(step_id not in step_ids for step_id in focus_ids):
            return AutonomousOrderedStepPlanRefinementResult(status="provider_invalid", **metadata)
        priority_position = {step_id: index for index, step_id in enumerate(priority_ids)}
        if any(
            priority_position[dependency] > priority_position[step["id"]]
            for step in graph
            for dependency in step["depends_on"]
        ):
            return AutonomousOrderedStepPlanRefinementResult(
                status="provider_disagreement",
                priority_step_ids=priority_ids,
                focus_step_ids=focus_ids,
                review_required=True,
                confidence=confidence,
                **metadata,
            )
        if abstain:
            return AutonomousOrderedStepPlanRefinementResult(
                status="provider_disagreement",
                priority_step_ids=priority_ids,
                focus_step_ids=focus_ids,
                review_required=True,
                confidence=confidence,
                **metadata,
            )
        return AutonomousOrderedStepPlanRefinementResult(
            status="completed",
            priority_step_ids=priority_ids,
            focus_step_ids=focus_ids,
            review_required=review_required,
            confidence=confidence,
            **metadata,
        )

    def plan_with_provider(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        context: Mapping[str, Any] | None = None,
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
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str = "planning",
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> AutonomousPlanRefinementResult:
        """Ask a provider to prioritize existing stages under a dependency-closed contract."""

        if not isinstance(blueprint, AutonomousTaskBlueprint):
            raise BrainRunError("plan refinement requires an AutonomousTaskBlueprint")
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("plan refinement model_candidates must be a sequence")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("plan refinement credentials must be a mapping")
        if any(
            not isinstance(provider, str)
            or not isinstance(handle, CredentialHandle)
            or provider != handle.provider
            for provider, handle in credentials.items()
        ):
            raise BrainRunError("plan refinement credentials must map providers to matching handles")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("plan refinement contextual_observations must be a sequence")
        stages = tuple(blueprint.workflow.stages)
        stage_ids = tuple(stage.id for stage in stages)
        dependencies = {stage.id: set(stage.depends_on) for stage in stages}
        base_plan_digest = content_digest(blueprint.plan)
        planner_task = (
            "Propose a bounded planning refinement for the reviewed workflow. Return only the "
            "required JSON object. Reorder and focus existing stages only; preserve every stage "
            "and every dependency. Do not add tools, credentials, effects, permissions, factual "
            "claims, or completed evidence. Mark review_required when a human should inspect the "
            "proposal. Original task:\n\n"
            + blueprint.spec.task
        )
        _text("plan refinement task", planner_task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        planner_context: dict[str, Any] = {
            "planning_contract": {
                "schema": AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
                "task_digest": blueprint.spec.task_digest,
                "base_plan_digest": base_plan_digest,
                "workflow_digest": blueprint.workflow.workflow_digest,
                "stage_catalogue": [
                    {
                        "id": stage.id,
                        "depends_on": list(stage.depends_on),
                        "required_capabilities": list(stage.required_capabilities),
                        "evidence_outputs": list(stage.evidence_outputs),
                        "approval_required": stage.approval_required,
                    }
                    for stage in stages
                ],
                "reconciliation": "priority_order_must_contain_each_existing_stage_exactly_once",
                "does_not_authorize": ["tools", "provider effects", "external writes", "credentials"],
            },
            "base_plan_metadata": {
                "workflow_id": blueprint.workflow.workflow_id,
                "workflow_digest": blueprint.workflow.workflow_digest,
                "domain": blueprint.profile.domain,
                "domain_pack_digest": blueprint.domain_pack.pack_digest,
                "stage_count": len(stages),
            },
        }
        if context is not None:
            BrainLearningLedger._assert_safe(context)
            planner_context["caller_context"] = dict(context)
        response_schema = _plan_refinement_response_schema(stage_ids)
        planner_blueprint = self.prepare(
            task=planner_task,
            domain=blueprint.profile.domain,
            capability="planning",
            context=planner_context,
            desired_outputs=("dependency-closed stage priority", "focus stages", "review decision"),
            max_steps=blueprint.spec.max_steps,
            require_json=True,
            response_schema=response_schema,
            execution_mode="provider",
            max_input_tokens=input_tokens,
            required_model_capabilities=blueprint.required_capabilities,
        )
        planner_blueprint = _apply_versioned_prompt(
            planner_blueprint,
            route=None,
            prompt_template=prompt_template,
            prompt_registry=prompt_registry,
            prompt_selection=prompt_selection,
            prompt_stage=prompt_stage,
            prompt_learning_state=prompt_learning_state,
            prompt_learning_exploration=prompt_learning_exploration,
        )
        planner_prompt_digest = _planner_prompt_digest(planner_blueprint)
        adaptive_selection = _planner_adaptive_prompt_selection(planner_blueprint, prompt_registry)
        planner_learning_context, planner_selection_context, planner_learning_context_digest = _provider_planner_context(
            planner_blueprint.selection_context,
            task_family=blueprint.workflow.workflow_id,
        )
        domain_policy_admission = _planning_domain_policy_admission(
            domain=blueprint.profile.domain,
            mode=domain_policy_mode,
            estimated_input_tokens=input_tokens,
            requested_output_tokens=max_output_tokens,
            evidence_ready=domain_policy_evidence_ready,
            evaluator_configured=domain_policy_evaluator_configured,
            effects_requested=domain_policy_effects_requested,
            effects_approved=domain_policy_effects_approved,
        )
        if domain_policy_admission is not None and domain_policy_admission.decision != "admitted":
            return AutonomousPlanRefinementResult(
                status=("policy_blocked" if domain_policy_admission.decision == "blocked" else "policy_review_required"),
                task_digest=blueprint.spec.task_digest,
                base_plan_digest=base_plan_digest,
                workflow_digest=blueprint.workflow.workflow_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
            )
        if domain_policy_mode == "strict" and approve_provider_call is not True:
            return AutonomousPlanRefinementResult(
                status="approval_required",
                task_digest=blueprint.spec.task_digest,
                base_plan_digest=base_plan_digest,
                workflow_digest=blueprint.workflow.workflow_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
            )
        if authorization_context is not None:
            authorization_context.authorize_operation(
                operation="plan",
                domain=blueprint.profile.domain,
                resource_digest=content_digest({
                    "schema": "bioprism-autonomous-plan-authorization-resource/0.1",
                    "domain": blueprint.profile.domain,
                    "task_digest": blueprint.spec.task_digest,
                    "base_plan_digest": base_plan_digest,
                    "planner_prompt_digest": planner_prompt_digest,
                    "planner_context_digest": planner_learning_context_digest,
                }),
            )
        selection_request = self.brain.build_adaptive_model_selection(
            task=planner_task,
            model_candidates=model_candidates,
            credentials=credentials,
            bandit_state=bandit_state,
            context=planner_selection_context,
            contextual_observations=contextual_observations,
            required_capabilities=blueprint.required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            selection_weights=selection_weights,
            selection_observations=selection_observations,
        )
        try:
            run = self.brain.run(
                task=planner_task,
                model_selection=selection_request,
                prompt=planner_blueprint.prompt,
                plan=planner_blueprint.plan,
                credentials=credentials,
                approve_provider_call=approve_provider_call,
                run_id=run_id,
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                require_json=True,
                response_schema=response_schema,
                context=planner_selection_context,
                contextual_observations=contextual_observations,
                authorization_context=authorization_context,
                authorization_domain=blueprint.profile.domain,
            )
        except (ProviderError, CredentialError) as error:
            failure = _planning_failure_projection(error)
            selected = selection_request.get("selected_model")
            safe_model = (
                {"provider": selected["provider"], "model": selected["model"]}
                if isinstance(selected, Mapping)
                and isinstance(selected.get("provider"), str)
                and isinstance(selected.get("model"), str)
                else None
            )
            selection_digest = selection_request.get("decision_digest")
            if not isinstance(selection_digest, str):
                selection_digest = None
            return AutonomousPlanRefinementResult(
                status="provider_failed",
                task_digest=blueprint.spec.task_digest,
                base_plan_digest=base_plan_digest,
                workflow_digest=blueprint.workflow.workflow_digest,
                selected_model=safe_model,
                selection_digest=selection_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                outcome_digest=_json_digest({
                    "status": "provider_failed",
                    "failure": failure,
                    "task_digest": blueprint.spec.task_digest,
                    "base_plan_digest": base_plan_digest,
                    "planner_context_digest": planner_learning_context_digest,
                    "planner_prompt_digest": planner_prompt_digest,
                }),
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
                failure=failure,
            )
        selection = run.selection
        selected_model = selection.get("selected_model")
        safe_model = None
        if isinstance(selected_model, Mapping) and isinstance(
            selected_model.get("provider"), str
        ) and isinstance(selected_model.get("model"), str):
            safe_model = {
                "provider": selected_model["provider"],
                "model": selected_model["model"],
            }
        planner_plan_value = run.plan.get("plan")
        planner_plan_digest = (
            planner_plan_value.get("plan_digest")
            if isinstance(planner_plan_value, Mapping)
            else None
        )
        metadata = {
            "task_digest": blueprint.spec.task_digest,
            "base_plan_digest": base_plan_digest,
            "workflow_digest": blueprint.workflow.workflow_digest,
            "selected_model": safe_model,
            "selection_digest": selection.get("decision_digest"),
            "planner_prompt_digest": planner_prompt_digest,
            "adaptive_selection": adaptive_selection,
            "planner_plan_digest": planner_plan_digest,
            "outcome_digest": _json_digest({
                "provider_outcome_digest": run.outcome_digest,
                "learning_context_digest": planner_learning_context_digest,
                "planner_prompt_digest": planner_prompt_digest,
            }),
            "planner_context": planner_learning_context,
            "planner_context_digest": planner_learning_context_digest,
        }
        if domain_policy_admission is not None:
            metadata["domain_policy_admission"] = domain_policy_admission
        if run.status == "provider_failed":
            if not isinstance(run.failure, Mapping):
                raise BrainRunError("provider_failed planning result did not contain a failure projection")
            metadata["failure"] = _normalize_planning_failure(run.failure, "plan refinement")
        if run.status != "completed_provider_call" or run.response is None:
            return AutonomousPlanRefinementResult(
                status=run.status if run.status in {"approval_required", "plan_refused", "provider_failed"} else "provider_invalid",
                **metadata,
            )
        raw = run.response.structured
        if not isinstance(raw, Mapping):
            return AutonomousPlanRefinementResult(status="provider_invalid", **metadata)
        priority = raw.get("priority_order")
        focus = raw.get("focus_stage_ids")
        review_required = raw.get("review_required")
        confidence = raw.get("confidence")
        abstain = raw.get("abstain")
        if (
            not isinstance(priority, list)
            or not isinstance(focus, list)
            or not isinstance(review_required, bool)
            or not isinstance(confidence, (int, float))
            or isinstance(confidence, bool)
            or not math.isfinite(float(confidence))
            or not 0.0 <= float(confidence) <= 1.0
            or not isinstance(abstain, bool)
            or any(not isinstance(stage_id, str) for stage_id in [*priority, *focus])
            or len(priority) != len(stage_ids)
            or tuple(priority) != tuple(dict.fromkeys(priority))
            or set(priority) != set(stage_ids)
            or len(focus) != len(set(focus))
            or any(stage_id not in stage_ids for stage_id in focus)
        ):
            return AutonomousPlanRefinementResult(status="provider_invalid", **metadata)
        priority_position = {stage_id: index for index, stage_id in enumerate(priority)}
        if any(
            priority_position[dependency] > priority_position[stage_id]
            for stage_id, required in dependencies.items()
            for dependency in required
            if dependency in priority_position
        ):
            return AutonomousPlanRefinementResult(status="provider_disagreement", **metadata)
        if abstain:
            return AutonomousPlanRefinementResult(
                status="provider_disagreement",
                priority_stage_ids=tuple(priority),
                focus_stage_ids=tuple(focus),
                review_required=True,
                confidence=confidence,
                **metadata,
            )
        return AutonomousPlanRefinementResult(
            status="completed",
            priority_stage_ids=tuple(priority),
            focus_stage_ids=tuple(focus),
            review_required=review_required,
            confidence=confidence,
            **metadata,
        )

    def plan_cross_domain_with_provider(
        self,
        *,
        blueprint: AutonomousCrossDomainBlueprint,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        context: Mapping[str, Any] | None = None,
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
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str = "planning",
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
    ) -> AutonomousCrossDomainPlanRefinementResult:
        """Ask a provider to prioritize existing cross-domain specialists only."""

        if not isinstance(blueprint, AutonomousCrossDomainBlueprint):
            raise BrainRunError("cross-domain plan refinement requires an AutonomousCrossDomainBlueprint")
        if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
            raise BrainRunError("cross-domain plan refinement model_candidates must be a sequence")
        if not isinstance(credentials, Mapping):
            raise BrainRunError("cross-domain plan refinement credentials must be a mapping")
        if any(
            not isinstance(provider, str)
            or not isinstance(handle, CredentialHandle)
            or provider != handle.provider
            for provider, handle in credentials.items()
        ):
            raise BrainRunError("cross-domain plan refinement credentials must map providers to matching handles")
        if not isinstance(contextual_observations, Sequence) or isinstance(
            contextual_observations, (str, bytes)
        ):
            raise BrainRunError("cross-domain plan refinement contextual_observations must be a sequence")
        child_ids = tuple(blueprint.child_ids)
        base_plan_digest = _cross_domain_plan_digest(blueprint)
        planner_task = (
            "Propose a bounded cross-domain planning refinement. Return only the required JSON object. "
            "Reorder and focus the existing specialist children only; preserve every child and the "
            "final synthesis. Do not add domains, tools, credentials, effects, permissions, factual "
            "claims, or completed evidence. Mark review_required when a human should inspect the "
            "proposal."
        )
        _text("cross-domain plan refinement task", planner_task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        required_model_capabilities = tuple(
            sorted(
                {
                    capability
                    for child in blueprint.child_blueprints
                    for capability in child.required_capabilities
                }
            )
        )
        planner_context: dict[str, Any] = {
            "planning_contract": {
                "schema": AUTONOMOUS_CROSS_DOMAIN_PLAN_REFINEMENT_SCHEMA,
                "task_digest": blueprint.task_digest,
                "base_plan_digest": base_plan_digest,
                "child_catalogue": [
                    {
                        "id": child_id,
                        "task": child.spec.task,
                        "task_digest": child.spec.task_digest,
                        "context_digest": child.spec.context_digest,
                        "domain": child.profile.domain,
                        "capability": child.spec.capability,
                        "risk_class": child.spec.risk_class,
                        "workflow_id": child.workflow.workflow_id,
                        "workflow_digest": child.workflow.workflow_digest,
                        "domain_pack_digest": child.domain_pack.pack_digest,
                        "stage_ids": [stage.id for stage in child.workflow.stages],
                    }
                    for child_id, child in zip(blueprint.child_ids, blueprint.child_blueprints)
                ],
                "synthesis": {
                    "domain": blueprint.synthesis_blueprint.profile.domain,
                    "workflow_digest": blueprint.synthesis_blueprint.workflow.workflow_digest,
                    "domain_pack_digest": blueprint.synthesis_blueprint.domain_pack.pack_digest,
                },
                "reconciliation": "priority_order_must_contain_each_existing_child_exactly_once",
                "does_not_authorize": ["tools", "provider effects", "external writes", "credentials"],
            },
            "base_plan_metadata": {
                "task": blueprint.task,
                "task_digest": blueprint.task_digest,
                "child_count": len(child_ids),
                "selected_domains": [child.profile.domain for child in blueprint.child_blueprints],
                "synthesis_workflow_id": blueprint.synthesis_blueprint.workflow.workflow_id,
            },
        }
        if context is not None:
            BrainLearningLedger._assert_safe(context)
            planner_context["caller_context"] = dict(context)
        response_schema = _cross_domain_plan_response_schema(child_ids)
        planner_blueprint = self.prepare(
            task=planner_task,
            domain="cross_domain",
            capability="planning",
            context=planner_context,
            desired_outputs=("dependency-aware specialist priority", "focus children", "review decision"),
            max_steps=blueprint.synthesis_blueprint.spec.max_steps,
            require_json=True,
            response_schema=response_schema,
            execution_mode="provider",
            max_input_tokens=input_tokens,
            required_model_capabilities=required_model_capabilities,
        )
        planner_blueprint = _apply_versioned_prompt(
            planner_blueprint,
            route=None,
            prompt_template=prompt_template,
            prompt_registry=prompt_registry,
            prompt_selection=prompt_selection,
            prompt_stage=prompt_stage,
            prompt_learning_state=prompt_learning_state,
            prompt_learning_exploration=prompt_learning_exploration,
        )
        planner_prompt_digest = _planner_prompt_digest(planner_blueprint)
        adaptive_selection = _planner_adaptive_prompt_selection(planner_blueprint, prompt_registry)
        planner_learning_context, planner_selection_context, planner_learning_context_digest = _provider_planner_context(
            planner_blueprint.selection_context,
            task_family=blueprint.synthesis_blueprint.workflow.workflow_id,
        )
        domain_policy_admission = _planning_domain_policy_admission(
            domain="cross_domain",
            mode=domain_policy_mode,
            estimated_input_tokens=input_tokens,
            requested_output_tokens=max_output_tokens,
            evidence_ready=domain_policy_evidence_ready,
            evaluator_configured=domain_policy_evaluator_configured,
            effects_requested=domain_policy_effects_requested,
            effects_approved=domain_policy_effects_approved,
        )
        if domain_policy_admission is not None and domain_policy_admission.decision != "admitted":
            return AutonomousCrossDomainPlanRefinementResult(
                status=("policy_blocked" if domain_policy_admission.decision == "blocked" else "policy_review_required"),
                task_digest=blueprint.task_digest,
                base_plan_digest=base_plan_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
            )
        if domain_policy_mode == "strict" and approve_provider_call is not True:
            return AutonomousCrossDomainPlanRefinementResult(
                status="approval_required",
                task_digest=blueprint.task_digest,
                base_plan_digest=base_plan_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
            )
        if authorization_context is not None:
            authorization_context.authorize_operation(
                operation="plan",
                domain="cross_domain",
                resource_digest=content_digest({
                    "schema": "bioprism-autonomous-plan-authorization-resource/0.1",
                    "domain": "cross_domain",
                    "task_digest": blueprint.task_digest,
                    "base_plan_digest": base_plan_digest,
                    "planner_prompt_digest": planner_prompt_digest,
                    "planner_context_digest": planner_learning_context_digest,
                }),
            )
        selection_request = self.brain.build_adaptive_model_selection(
            task=planner_task,
            model_candidates=model_candidates,
            credentials=credentials,
            bandit_state=bandit_state,
            context=planner_selection_context,
            contextual_observations=contextual_observations,
            required_capabilities=required_model_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            selection_overrides=selection_overrides,
            selection_weights=selection_weights,
            selection_observations=selection_observations,
        )
        try:
            run = self.brain.run(
                task=planner_task,
                model_selection=selection_request,
                prompt=planner_blueprint.prompt,
                plan=planner_blueprint.plan,
                credentials=credentials,
                approve_provider_call=approve_provider_call,
                run_id=run_id,
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                require_json=True,
                response_schema=response_schema,
                context=planner_selection_context,
                contextual_observations=contextual_observations,
                authorization_context=authorization_context,
                authorization_domain="cross_domain",
            )
        except (ProviderError, CredentialError) as error:
            failure = _planning_failure_projection(error)
            selected = selection_request.get("selected_model")
            safe_model = (
                {"provider": selected["provider"], "model": selected["model"]}
                if isinstance(selected, Mapping)
                and isinstance(selected.get("provider"), str)
                and isinstance(selected.get("model"), str)
                else None
            )
            selection_digest = selection_request.get("decision_digest")
            if not isinstance(selection_digest, str):
                selection_digest = None
            return AutonomousCrossDomainPlanRefinementResult(
                status="provider_failed",
                task_digest=blueprint.task_digest,
                base_plan_digest=base_plan_digest,
                selected_model=safe_model,
                selection_digest=selection_digest,
                planner_prompt_digest=planner_prompt_digest,
                adaptive_selection=adaptive_selection,
                outcome_digest=_json_digest({
                    "status": "provider_failed",
                    "failure": failure,
                    "task_digest": blueprint.task_digest,
                    "base_plan_digest": base_plan_digest,
                    "planner_context_digest": planner_learning_context_digest,
                    "planner_prompt_digest": planner_prompt_digest,
                }),
                planner_context=planner_learning_context,
                planner_context_digest=planner_learning_context_digest,
                domain_policy_admission=domain_policy_admission,
                failure=failure,
            )
        selected_model = run.selection.get("selected_model")
        safe_model = None
        if isinstance(selected_model, Mapping) and isinstance(selected_model.get("provider"), str) and isinstance(
            selected_model.get("model"), str
        ):
            safe_model = {"provider": selected_model["provider"], "model": selected_model["model"]}
        planner_plan_value = run.plan.get("plan")
        planner_plan_digest = planner_plan_value.get("plan_digest") if isinstance(planner_plan_value, Mapping) else None
        metadata = {
            "task_digest": blueprint.task_digest,
            "base_plan_digest": base_plan_digest,
            "selected_model": safe_model,
            "selection_digest": run.selection.get("decision_digest"),
            "planner_prompt_digest": planner_prompt_digest,
            "adaptive_selection": adaptive_selection,
            "planner_plan_digest": planner_plan_digest,
            "outcome_digest": _json_digest({
                "provider_outcome_digest": run.outcome_digest,
                "learning_context_digest": planner_learning_context_digest,
                "planner_prompt_digest": planner_prompt_digest,
            }),
            "planner_context": planner_learning_context,
            "planner_context_digest": planner_learning_context_digest,
        }
        if domain_policy_admission is not None:
            metadata["domain_policy_admission"] = domain_policy_admission
        if run.status == "provider_failed":
            if not isinstance(run.failure, Mapping):
                raise BrainRunError("provider_failed cross-domain planning result did not contain a failure projection")
            metadata["failure"] = _normalize_planning_failure(run.failure, "cross-domain plan refinement")
        if run.status != "completed_provider_call" or run.response is None:
            return AutonomousCrossDomainPlanRefinementResult(
                status=run.status if run.status in {"approval_required", "plan_refused", "provider_failed"} else "provider_invalid",
                **metadata,
            )
        raw = run.response.structured
        if not isinstance(raw, Mapping):
            return AutonomousCrossDomainPlanRefinementResult(status="provider_invalid", **metadata)
        priority = raw.get("priority_order")
        focus = raw.get("focus_child_ids")
        review_required = raw.get("review_required")
        confidence = raw.get("confidence")
        abstain = raw.get("abstain")
        if (
            not isinstance(priority, list)
            or not isinstance(focus, list)
            or not isinstance(review_required, bool)
            or not isinstance(confidence, (int, float))
            or isinstance(confidence, bool)
            or not math.isfinite(float(confidence))
            or not 0.0 <= float(confidence) <= 1.0
            or not isinstance(abstain, bool)
            or any(not isinstance(child_id, str) for child_id in [*priority, *focus])
            or len(priority) != len(child_ids)
            or tuple(priority) != tuple(dict.fromkeys(priority))
            or set(priority) != set(child_ids)
            or len(focus) != len(set(focus))
            or any(child_id not in child_ids for child_id in focus)
        ):
            return AutonomousCrossDomainPlanRefinementResult(status="provider_invalid", **metadata)
        if abstain:
            return AutonomousCrossDomainPlanRefinementResult(
                status="provider_disagreement",
                priority_child_ids=tuple(priority),
                focus_child_ids=tuple(focus),
                review_required=True,
                confidence=confidence,
                **metadata,
            )
        return AutonomousCrossDomainPlanRefinementResult(
            status="completed",
            priority_child_ids=tuple(priority),
            focus_child_ids=tuple(focus),
            review_required=review_required,
            confidence=confidence,
            **metadata,
        )

    @staticmethod
    def _route_context(
        context: Mapping[str, Any] | None,
        route: AutonomousRouteProposal,
    ) -> dict[str, Any]:
        """Bind route identity into transient selection context without accepting spoofing."""

        resolved = {} if context is None else dict(context)
        if "autonomous_route" in resolved:
            raise BrainRunError("context cannot override the automatic route binding")
        resolved["autonomous_route"] = {
            "route_digest": route.route_digest,
            "reason": route.reason,
            "confidence": route.confidence,
            "selected_domains": list(route.selected_domains),
            "cross_domain": route.cross_domain,
        }
        return resolved

# Keep methods discoverable through the public AutonomousTaskOrchestrator facade.
for _name, _descriptor in vars(AutonomousOrchestratorRoutingMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousTaskOrchestrator.{_name}"
