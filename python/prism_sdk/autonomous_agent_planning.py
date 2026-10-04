"""Focused autonomous-agent intake, planning, and workflow methods."""

from __future__ import annotations

from .autonomy import (
    Any,
    AutonomousAutoBlueprint,
    AutonomousClarificationRecompile,
    AutonomousCrossDomainBlueprint,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousDomainPolicy,
    AutonomousDomainTaskLens,
    AutonomousOrderedStepPlanRefinementResult,
    AutonomousPlanRefinementResult,
    AutonomousRouteCandidate,
    AutonomousRouteProposal,
    AutonomousSemanticRouteResult,
    AutonomousTaskBlueprint,
    AutonomousTaskClarificationPlan,
    AutonomousTaskClarificationResolution,
    AutonomousTaskDecision,
    AutonomousTaskIntent,
    BrainRunError,
    CredentialHandle,
    CredentialSession,
    MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS,
    MAX_AUTONOMY_TEXT_BYTES,
    Mapping,
    ModelCandidate,
    Sequence,
    _authorize_launch_admission_domains,
    _portfolio_items_domains_for_launch_admission,
    _portfolio_plan_domains_for_launch_admission,
    _text,
    content_digest,
    plan_autonomous_task_clarification,
    resolve_autonomous_task_clarification,
    validate_autonomous_domain_policy,
    validate_autonomous_domain_task_lens,
    validate_autonomous_task_clarification_plan,
    validate_autonomous_task_clarification_recompile,
    validate_autonomous_task_clarification_resolution,
    validate_autonomous_task_decision,
    validate_autonomous_task_intent,
)

class AutonomousAgentPlanningMixin:
    @staticmethod
    def _credential_mapping(
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
    ) -> dict[str, CredentialHandle]:
        if isinstance(credentials, CredentialSession):
            return credentials.handles()
        if not isinstance(credentials, Mapping):
            raise BrainRunError("credentials must be a mapping or CredentialSession")
        resolved = dict(credentials)
        if any(
            not isinstance(provider, str) or not isinstance(handle, CredentialHandle)
            or provider != handle.provider
            for provider, handle in resolved.items()
        ):
            raise BrainRunError("credentials must map provider names to matching opaque handles")
        return resolved

    def prepare(self, **kwargs: Any) -> AutonomousTaskBlueprint:
        """Build a domain-aware plan and prompt without contacting any provider."""

        return self.orchestrator.prepare(**kwargs)

    def clarification_plan(
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
        max_questions: int = MAX_AUTONOMOUS_TASK_CLARIFICATION_QUESTIONS,
    ) -> AutonomousTaskClarificationPlan:
        """Compile bounded user questions before provider, source, tool, or effect work.

        This is an explicit preflight boundary. It reuses the exact task artifacts that would
        shape execution, but its result is guidance only. A caller must collect answers, rebuild
        the task description/intent/decision when scope changes, and still pass the existing
        provider, evidence, tool, credential, evaluator, and effect gates.
        """

        blueprint = self.orchestrator.prepare(
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
            max_input_tokens=max_input_tokens,
            required_model_capabilities=required_model_capabilities,
        )
        if blueprint.task_intent is None or blueprint.task_lens is None or blueprint.domain_policy is None or blueprint.task_decision is None:
            raise BrainRunError("clarification plan requires complete prepared task artifacts")
        return plan_autonomous_task_clarification(
            intent=blueprint.task_intent,
            lens=blueprint.task_lens,
            policy=blueprint.domain_policy,
            decision=blueprint.task_decision,
            max_questions=max_questions,
        )

    def resolve_clarification(
        self,
        *,
        plan: AutonomousTaskClarificationPlan | Mapping[str, Any],
        task: str,
        answers: Mapping[str, str],
    ) -> AutonomousTaskClarificationResolution:
        """Turn transient answers into a digest-only receipt bound to the original task."""

        task_text = _text("clarification task", task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        return resolve_autonomous_task_clarification(
            plan,
            task_digest=content_digest({"task": task_text}),
            answers=answers,
        )

    def validate_clarification(
        self,
        *,
        plan: AutonomousTaskClarificationPlan | Mapping[str, Any],
        receipt: AutonomousTaskClarificationResolution | Mapping[str, Any],
    ) -> AutonomousTaskClarificationResolution:
        """Rehydrate a persisted clarification receipt against its exact plan.

        This is intentionally a validation boundary rather than an execution shortcut. It
        verifies receipt integrity, question identity, answer counts, and blocked/resolved
        invariants, but it cannot recover answer values and does not authorize a provider,
        source, tool, evaluator, credential, or external effect. Callers must still recompile
        task intent and decision artifacts after incorporating any material answer.
        """

        return validate_autonomous_task_clarification_resolution(receipt, plan=plan)

    def validate_task_lens(
        self,
        *,
        value: AutonomousDomainTaskLens | Mapping[str, Any],
        expected_domain: str | None = None,
    ) -> AutonomousDomainTaskLens:
        """Validate a persisted domain lens before task classification or planning resumes."""

        return validate_autonomous_domain_task_lens(value, expected_domain=expected_domain)

    def validate_task_intent(
        self,
        *,
        value: AutonomousTaskIntent | Mapping[str, Any],
        lens: AutonomousDomainTaskLens | Mapping[str, Any] | None = None,
        expected_task_digest: str | None = None,
    ) -> AutonomousTaskIntent:
        """Validate persisted intent metadata and optionally bind it to the current task/lens."""

        return validate_autonomous_task_intent(
            value,
            lens=lens,
            expected_task_digest=expected_task_digest,
        )

    def validate_domain_policy(
        self,
        *,
        value: AutonomousDomainPolicy | Mapping[str, Any],
        expected_domain: str | None = None,
    ) -> AutonomousDomainPolicy:
        """Validate a persisted domain policy before a resumed decision replay."""

        return validate_autonomous_domain_policy(value, expected_domain=expected_domain)

    def validate_task_decision(
        self,
        *,
        value: AutonomousTaskDecision | Mapping[str, Any],
        intent: Any | None = None,
        lens: Any | None = None,
        policy: Any | None = None,
        required_model_capabilities: Sequence[str] | None = None,
    ) -> AutonomousTaskDecision:
        """Validate a persisted task decision before a resumed execution boundary.

        Without bindings this verifies the decision's canonical metadata and digest.  Supplying
        the live intent, lens, and policy additionally replays the deterministic decision so a
        stale task artifact cannot silently retain old approval requirements.
        """

        return validate_autonomous_task_decision(
            value,
            intent=intent,
            lens=lens,
            policy=policy,
            required_model_capabilities=required_model_capabilities,
        )

    def recompile_clarification(
        self,
        *,
        plan: AutonomousTaskClarificationPlan | Mapping[str, Any],
        receipt: AutonomousTaskClarificationResolution | Mapping[str, Any],
        task: str,
        clarified_task: str,
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
    ) -> AutonomousClarificationRecompile:
        """Recompile a clarified task only after its complete receipt is verified.

        The caller explicitly supplies ``clarified_task`` after rehydrating transient answer
        values.  This keeps answer interpretation in the application, where it can be reviewed,
        while the agent enforces the original task digest, exact plan binding, complete-answer
        status, and fixed domain before building a fresh intent, decision, prompt, and execution
        plan.  The returned live blueprint is usable by the ordinary execution APIs; its
        ``to_dict`` projection contains no task text, answer values, credentials, or provider
        payloads.
        """

        resolved_plan = validate_autonomous_task_clarification_plan(plan)
        original_task_text = _text("clarification task", task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        original_task_digest = content_digest({"task": original_task_text})
        if resolved_plan.task_digest != original_task_digest:
            raise BrainRunError("clarification recompile task does not match the original plan")
        resolved_receipt = validate_autonomous_task_clarification_resolution(receipt, plan=resolved_plan)
        if resolved_receipt.task_digest != original_task_digest:
            raise BrainRunError("clarification recompile receipt does not match the original task")
        if resolved_receipt.status != "resolved":
            raise BrainRunError("clarification recompile requires a resolved clarification receipt")
        clarified_task_text = _text("clarified task", clarified_task, maximum=MAX_AUTONOMY_TEXT_BYTES)
        blueprint = self.prepare(
            task=clarified_task_text,
            domain=resolved_plan.domain,
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
            max_input_tokens=max_input_tokens,
            required_model_capabilities=required_model_capabilities,
            memory_episodes=memory_episodes,
            memory_lesson_references=memory_lesson_references,
        )
        if blueprint.task_intent is None or blueprint.task_decision is None:
            raise BrainRunError("clarification recompile produced incomplete task artifacts")
        execution_plan_digest = content_digest(blueprint.to_dict()["plan"])
        return AutonomousClarificationRecompile(
            plan_digest=resolved_plan.plan_digest,
            resolution_digest=resolved_receipt.resolution_digest,
            original_task_digest=original_task_digest,
            recompiled_task_digest=blueprint.spec.task_digest,
            domain=blueprint.spec.domain,
            workflow_id=blueprint.workflow.workflow_id,
            recompiled_intent_digest=blueprint.task_intent.intent_digest,
            recompiled_decision_digest=blueprint.task_decision.decision_digest,
            execution_plan_digest=execution_plan_digest,
            blueprint=blueprint,
        )

    def validate_clarification_recompile(
        self,
        *,
        value: Mapping[str, Any],
        plan: AutonomousTaskClarificationPlan | Mapping[str, Any] | None = None,
        receipt: AutonomousTaskClarificationResolution | Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Validate a persisted recompile projection without restoring transient task values."""

        return validate_autonomous_task_clarification_recompile(value, plan=plan, receipt=receipt)

    def route(self, *, task: str, **kwargs: Any) -> AutonomousRouteProposal:
        """Return an auditable domain proposal without contacting a provider."""

        return self.orchestrator.route_task(task=task, **kwargs)

    def route_with_provider(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
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
    ) -> AutonomousSemanticRouteResult:
        """Improve provider-free routing with a bounded, caller-approved semantic proposal."""

        self._assert_selection_promotion_admitted()
        candidates = self._resolve_candidates(
            model_candidates,
            allow_empty=domain_policy_mode == "strict",
        )
        resolved_credentials = self._credential_mapping(credentials)
        resolved_overrides = None if selection_overrides is None else dict(selection_overrides)
        if self.health_ledger is not None:
            resolved_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(), resolved_overrides
            )
        return self.orchestrator.route_with_provider(
            task=task,
            model_candidates=candidates,
            credentials=resolved_credentials,
            hints=hints,
            context=context,
            min_confidence=min_confidence,
            min_margin=min_margin,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            semantic_weight=semantic_weight,
            bandit_state=bandit_state,
            contextual_observations=contextual_observations,
            selection_overrides=resolved_overrides,
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

    def prepare_auto(self, **kwargs: Any) -> AutonomousAutoBlueprint:
        """Build an automatic single-domain, cross-domain, or review-required blueprint."""

        return self.orchestrator.prepare_auto(**kwargs)

    def action_plan(self, *, task: str, domain: str | None = None, **kwargs: Any) -> dict[str, Any]:
        """Compile the prepared automatic route into one metadata-only next-action handoff.

        The returned plan is provider-free and does not authorize provider, evidence, tool,
        evaluator, credential, or effect dispatch.  Callers can persist the plan for review,
        then invoke the existing explicit admission boundary that corresponds to its
        ``next_action``.
        """

        from .autonomous_action_plan import plan_autonomous_action

        if domain is None:
            blueprint = self.orchestrator.prepare_auto(task=task, **kwargs)
        else:
            # Explicit domain requests are still represented as a normal route proposal so the
            # action plan has one stable digest boundary.  The route is caller-selected, while
            # the domain workflow/policy/decision artifacts remain the same as automatic intake.
            route = self.orchestrator.route_task(
                task=task,
                hints=(domain,),
                allow_cross_domain=False,
                min_confidence=0.0,
                min_margin=0.0,
            )
            profile = self.orchestrator.registry.resolve(domain)
            workflow = self.orchestrator.workflow_registry.resolve(domain)
            candidate = next((item for item in route.candidates if item.domain == domain), None)
            if candidate is None:
                candidate = AutonomousRouteCandidate(
                    domain=domain,
                    score=1.0,
                    matched_terms=("explicit_domain",),
                    capability=profile.default_capability,
                    risk_class=profile.risk_class,
                    workflow_id=workflow.workflow_id,
                )
            explicit_route = AutonomousRouteProposal(
                task_digest=route.task_digest,
                candidates=(candidate,),
                selected_domains=(domain,),
                confidence=max(1.0, candidate.score),
                abstained=False,
                reason="routed",
                cross_domain=False,
                source=route.source,
            )
            prepare_kwargs = dict(kwargs)
            # These options belong to automatic routing and have no meaning once the caller
            # has selected an explicit domain.
            for route_option in ("hints", "min_confidence", "min_margin", "max_domains", "allow_cross_domain"):
                prepare_kwargs.pop(route_option, None)
            blueprint = AutonomousAutoBlueprint(
                route=explicit_route,
                blueprint=self.orchestrator.prepare(task=task, domain=domain, **prepare_kwargs),
            )
        return plan_autonomous_action(blueprint).to_dict()

    def admit_action_plan(
        self,
        plan: Mapping[str, Any] | Any,
        *,
        approvals: Mapping[str, bool] | None = None,
        reviewed: bool = False,
    ) -> dict[str, Any]:
        """Record the caller's explicit gates against one exact action-plan digest.

        Admission is provider-free and does not consume credentials.  A returned ``admitted``
        record is still not an authorization token; it is the value-only input required by
        :meth:`execute_action_plan` before that method may delegate to ``run_auto``.
        """

        from .autonomous_action_execution import admit_autonomous_action_plan

        return admit_autonomous_action_plan(
            plan,
            approvals=approvals,
            reviewed=reviewed,
        ).to_dict()

    def execute_action_plan(
        self,
        *,
        task: str,
        plan: Mapping[str, Any] | Any,
        credentials: Mapping[str, CredentialHandle] | CredentialSession | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        approvals: Mapping[str, bool] | None = None,
        reviewed: bool = False,
        domain: str | None = None,
        route_options: Mapping[str, Any] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Replay, admit, and execute one digest-bound action plan.

        ``route_options`` (or equivalent routing keys in ``kwargs``) must reproduce the
        provider-free plan.  The task is transient and is never copied into the plan or the
        admission record.  If review, policy, evidence, or approval gates are incomplete, the
        method returns an :class:`AutonomousActionExecution` without touching a provider,
        connector, evaluator, learner, or credential.

        Once admitted, the recommended path is translated into existing execution controls:
        workflow paths opt into checkpointable workflow execution, planning paths opt into the
        provider-planning boundary, and cross-domain paths retain the reviewed fan-out/fan-in
        route.  The caller still owns credentials, evidence, evaluator settlement, and effects.
        """

        from .autonomous_action_execution import (
            AutonomousActionExecution,
            admit_autonomous_action_plan,
        )
        from .autonomous_action_plan import AutonomousActionPlan

        if isinstance(plan, Mapping):
            parsed_plan = AutonomousActionPlan.from_dict(plan)
        elif isinstance(plan, AutonomousActionPlan):
            parsed_plan = plan
        else:
            raise BrainRunError("execute_action_plan requires an AutonomousActionPlan or serialized plan")
        if route_options is not None and not isinstance(route_options, Mapping):
            raise BrainRunError("execute_action_plan route_options must be a mapping")
        normalized_route_options = dict(route_options or {})
        route_option_names = {
            "hints",
            "min_confidence",
            "min_margin",
            "max_domains",
            "allow_cross_domain",
            "context",
            "constraints",
            "desired_outputs",
            "capability",
            "risk_class",
            "max_steps",
            "require_json",
            "structured_domain_response",
            "response_schema",
            "execution_mode",
            "max_input_tokens",
            "required_model_capabilities",
            "memory_episodes",
            "domain_policy_mode",
            "domain_policy_evidence_ready",
            "domain_policy_evaluator_configured",
            "domain_policy_effects_requested",
            "domain_policy_effects_approved",
        }
        execution_options = dict(kwargs)
        for name in route_option_names:
            if name not in execution_options:
                continue
            value = execution_options.pop(name)
            if name in normalized_route_options and normalized_route_options[name] != value:
                raise BrainRunError(f"execute_action_plan {name} differs between route_options and execution options")
            normalized_route_options[name] = value
        for name in ("credentials", "model_candidates", "approvals", "reviewed", "plan", "domain"):
            if name in normalized_route_options:
                raise BrainRunError(f"execute_action_plan route_options cannot contain {name}")

        expected_public = self.action_plan(
            task=task,
            domain=domain,
            **normalized_route_options,
        )
        expected_plan = AutonomousActionPlan.from_dict(expected_public)
        if expected_plan.plan_digest != parsed_plan.plan_digest:
            raise BrainRunError(
                "action plan is stale or was compiled with different task, route, or blueprint inputs"
            )

        admission = admit_autonomous_action_plan(
            parsed_plan,
            approvals=approvals,
            reviewed=reviewed,
        )
        if admission.status != "admitted":
            return AutonomousActionExecution(
                status=admission.status,
                plan=parsed_plan,
                admission=admission,
            )
        if credentials is None:
            raise BrainRunError("admitted action-plan execution requires caller-supplied credentials")

        # The automatic runner owns routing. Explicit domain plans are reproduced through the
        # same deterministic hint boundary used by action_plan, never by bypassing the router.
        execution_options.update(normalized_route_options)
        if domain is not None:
            execution_options.update(
                {
                    "hints": (domain,),
                    "min_confidence": 0.0,
                    "min_margin": 0.0,
                    "allow_cross_domain": False,
                }
            )

        def enable_gate(name: str) -> None:
            current = execution_options.get(name)
            if current is not None and current is not True:
                raise BrainRunError(f"action-plan approval contradicts explicit {name}=False")
            execution_options[name] = True

        for gate in admission.approved_approvals:
            if gate == "provider_call":
                enable_gate("approve_provider_call")
            elif gate == "evidence_dispatch":
                enable_gate("domain_policy_evidence_ready")
            elif gate == "plan_acceptance":
                enable_gate("domain_policy_plan_accepted")
            elif gate == "effect_approval":
                enable_gate("domain_policy_effects_requested")
                enable_gate("domain_policy_effects_approved")
            elif gate == "evaluator_settlement":
                # Evaluator identity/evidence remain caller-owned kwargs. The admission gate
                # proves only that the caller reviewed the requirement, never that reward exists.
                continue

        if admission.execution_path == "workflow":
            enable_gate("workflow_execution")
        elif admission.execution_path == "planning":
            current_planning_mode = execution_options.get("planning_mode")
            if current_planning_mode is not None and current_planning_mode != "provider":
                raise BrainRunError("planning action plans require planning_mode='provider'")
            execution_options["planning_mode"] = "provider"

        result = self.run_auto(
            task=task,
            credentials=credentials,
            model_candidates=model_candidates,
            **execution_options,
        )
        execution_status = getattr(result, "execution_status", None)
        return AutonomousActionExecution(
            status="completed" if execution_status == "completed" else "review_required",
            plan=parsed_plan,
            admission=admission,
            result=result,
        )

    def execute_action_handoff(
        self,
        *,
        task: str,
        handoff: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        domain: str | None = None,
        **kwargs: Any,
    ) -> Any:
        """Revalidate and execute one operator-produced action dispatch handoff.

        The handoff proves plan/admission continuity only.  This method replays the embedded
        plan against transient task input and delegates to :meth:`execute_action_plan`, leaving
        credentials, provider approval, evidence, tools, evaluator settlement, and effects as
        separate caller-owned gates.
        """

        from .autonomous_action_admission_controller import validate_autonomous_action_dispatch_handoff

        reserved = {"approvals", "reviewed", "plan", "handoff"}
        if reserved.intersection(kwargs):
            raise BrainRunError("execute_action_handoff does not allow overriding handoff admission fields")
        normalized = validate_autonomous_action_dispatch_handoff(handoff)
        selected_domains = normalized["selected_domains"]
        if domain is not None and domain not in selected_domains and not (domain == "cross_domain" and normalized["cross_domain"]):
            raise BrainRunError("action handoff does not cover the requested domain")
        approvals = {gate: True for gate in normalized["admission"]["approved_approvals"]}
        execution = self.execute_action_plan(
            task=task,
            plan=normalized["plan"],
            credentials=credentials,
            model_candidates=model_candidates,
            approvals=approvals,
            reviewed=True,
            domain=domain,
            **kwargs,
        )
        execution_plan = getattr(execution, "plan", None)
        execution_admission = getattr(execution, "admission", None)
        if execution_plan is None or execution_admission is None or execution_plan.plan_digest != normalized["plan_digest"] or execution_admission.admission_digest != normalized["admission_digest"]:
            raise BrainRunError("action handoff admission drifted during execution replay")
        return execution

    def plan_workflow_portfolio(
        self,
        requests: Sequence[Any],
        *,
        require_all_domains: bool = False,
        allow_partial: bool = True,
    ) -> dict[str, Any]:
        """Compile multiple reviewed domain workflows into dependency waves without dispatch.

        The portfolio compiler is intentionally provider-free.  It composes the same twelve
        domain profiles, workflow DAGs, evidence plans, and task decisions used by ordinary
        execution, but returns only request/task/route/workflow digests and bounded metadata.
        Call :meth:`verify_workflow_portfolio` after caller-owned rehydration and hand individual
        ready blueprints to the existing approved workflow runner.
        """

        from .autonomous_workflow_portfolio import plan_autonomous_workflow_portfolio

        return plan_autonomous_workflow_portfolio(
            self,
            requests,
            require_all_domains=require_all_domains,
            allow_partial=allow_partial,
        ).to_dict()

    def admit_workflow_portfolio(
        self,
        requests: Sequence[Any],
        *,
        plan: Mapping[str, Any] | Any | None = None,
        verify_plan: bool = True,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        require_available_tools: bool = False,
        require_calibrated_learning: bool = False,
        input_tokens: int = 4_096,
        output_tokens: int = 1_024,
        max_cost_per_million_tokens: float | None = None,
        max_latency_ms: float | None = None,
        min_quality: float | None = None,
        readiness_options: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Project provider-free readiness and model admission for a reviewed portfolio.

        Admission is intentionally separate from execution. It checks current domain readiness,
        model capability/constraint coverage, optional tool/evidence/calibration gates, and
        dependency closure, while returning only a digest-bound metadata image for caller review.
        """

        from .autonomous_workflow_portfolio_admission import admit_autonomous_workflow_portfolio

        return admit_autonomous_workflow_portfolio(
            self,
            requests,
            plan=plan,
            verify_plan=verify_plan,
            model_candidates=model_candidates,
            require_available_tools=require_available_tools,
            require_calibrated_learning=require_calibrated_learning,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            readiness_options=readiness_options,
        ).to_dict()

    def verify_workflow_portfolio(
        self,
        plan: Mapping[str, Any],
        requests: Sequence[Any],
        *,
        require_all_domains: bool | None = None,
        allow_partial: bool | None = None,
    ) -> dict[str, Any]:
        """Replay a workflow portfolio plan and return a digest-bound metadata verification."""

        from .autonomous_workflow_portfolio import verify_autonomous_workflow_portfolio

        return verify_autonomous_workflow_portfolio(
            self,
            plan,
            requests,
            require_all_domains=require_all_domains,
            allow_partial=allow_partial,
        ).to_dict()

    def execute_workflow_portfolio(
        self,
        plan: Mapping[str, Any] | Any,
        requests: Sequence[Any],
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        job_id: str,
        max_parallelism: int = 4,
        stop_on_error: bool = False,
        admission: Mapping[str, Any] | Any | None = None,
        checkpoint: Mapping[str, Any] | Any | None = None,
        checkpoint_sink: Any | None = None,
        rehydrate_result: Any | None = None,
        workflow_options_factory: Any | None = None,
    ) -> Any:
        """Execute a previously reviewed workflow portfolio in dependency waves.

        The portfolio runner performs a provider-free replay before dispatch, then delegates each
        ready item to this agent's ordinary credential/model-aware workflow runner.  Checkpoints
        contain only digests and status metadata; callers retain and rehydrate successful raw runs.
        """

        from .autonomous_workflow_portfolio import execute_autonomous_workflow_portfolio

        return execute_autonomous_workflow_portfolio(
            self,
            plan,
            requests,
            credentials=credentials,
            model_candidates=model_candidates,
            job_id=job_id,
            max_parallelism=max_parallelism,
            stop_on_error=stop_on_error,
            admission=admission,
            checkpoint=checkpoint,
            checkpoint_sink=checkpoint_sink,
            rehydrate_result=rehydrate_result,
            workflow_options_factory=workflow_options_factory,
        )

    def execute_workflow_portfolio_with_launch_admission(
        self,
        plan: Mapping[str, Any] | Any,
        requests: Sequence[Any],
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Execute a workflow portfolio only after every planned item domain is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _portfolio_plan_domains_for_launch_admission(plan),
        )
        return self.execute_workflow_portfolio(plan, requests, **kwargs)

    def execute_workflow_portfolio_evidence(
        self,
        execution: Any,
        *,
        items: Sequence[Any],
        runtime: Any,
        plan: Any | None = None,
        evidence_plan: Any | None = None,
        journal_for: Any | None = None,
        max_parallelism: int = 4,
        stop_on_failure: bool = False,
        progress_sink: Any | None = None,
    ) -> Any:
        """Supervise caller-owned evidence across a completed provider portfolio.

        Provider execution and evidence truth remain separate boundaries.  This method composes
        the existing evidence runtime across portfolio dependency waves, propagating only
        provider/result digests and parent evidence digests between items.  Acquirers, projectors,
        evaluators, journals, and transient values remain caller-owned.
        """

        from .autonomous_workflow_portfolio_evidence import (
            execute_autonomous_workflow_portfolio_evidence,
        )

        return execute_autonomous_workflow_portfolio_evidence(
            self,
            execution,
            items=items,
            runtime=runtime,
            plan=plan,
            evidence_plan=evidence_plan,
            journal_for=journal_for,
            max_parallelism=max_parallelism,
            stop_on_failure=stop_on_failure,
            progress_sink=progress_sink,
        )

    def execute_workflow_portfolio_evidence_with_launch_admission(
        self,
        execution: Any,
        *,
        items: Sequence[Any],
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Supervise portfolio evidence only after every evidence item domain is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _portfolio_items_domains_for_launch_admission(items),
        )
        return self.execute_workflow_portfolio_evidence(
            execution,
            items=items,
            **kwargs,
        )

    def execute_workflow_portfolio_evidence_resumable(
        self,
        execution: Any,
        *,
        job_id: str,
        items: Sequence[Any],
        runtime: Any,
        checkpoint_sink: Any,
        checkpoint: Any | None = None,
        plan: Any | None = None,
        evidence_plan: Any | None = None,
        require_admission: bool = True,
        runtime_policy_digest: str | None = None,
        journal_for: Any | None = None,
        max_parallelism: int = 4,
        stop_on_failure: bool = False,
        progress_sink: Any | None = None,
    ) -> Any:
        """Run portfolio evidence with digest-bound checkpoints and journal replay."""

        from .autonomous_workflow_portfolio_evidence import (
            execute_autonomous_workflow_portfolio_evidence_resumable,
        )

        return execute_autonomous_workflow_portfolio_evidence_resumable(
            self,
            execution,
            job_id=job_id,
            items=items,
            runtime=runtime,
            checkpoint_sink=checkpoint_sink,
            checkpoint=checkpoint,
            plan=plan,
            evidence_plan=evidence_plan,
            require_admission=require_admission,
            runtime_policy_digest=runtime_policy_digest,
            journal_for=journal_for,
            max_parallelism=max_parallelism,
            stop_on_failure=stop_on_failure,
            progress_sink=progress_sink,
        )

    def execute_workflow_portfolio_evidence_resumable_with_launch_admission(
        self,
        execution: Any,
        *,
        job_id: str,
        items: Sequence[Any],
        runtime: Any,
        checkpoint_sink: Any,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Resume portfolio evidence only after every checkpointed item domain is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _portfolio_items_domains_for_launch_admission(items),
        )
        return self.execute_workflow_portfolio_evidence_resumable(
            execution,
            job_id=job_id,
            items=items,
            runtime=runtime,
            checkpoint_sink=checkpoint_sink,
            **kwargs,
        )

    def admit_workflow_portfolio_evidence_work(
        self,
        queue: Any,
        execution: Any,
        *,
        job_id: str,
        evidence_plan_digest: str,
        item_request_digests: Sequence[str],
        checkpoint_digest: str | None = None,
        max_attempts: int = 3,
        now: int | None = None,
    ) -> tuple[Any, ...]:
        """Admit reviewed provider items into a lease-fenced evidence work queue."""

        from .autonomous_workflow_portfolio_evidence_queue import (
            admit_autonomous_workflow_portfolio_evidence_work_items,
        )

        return admit_autonomous_workflow_portfolio_evidence_work_items(
            queue,
            job_id=job_id,
            execution=execution,
            evidence_plan_digest=evidence_plan_digest,
            item_request_digests=item_request_digests,
            checkpoint_digest=checkpoint_digest,
            max_attempts=max_attempts,
            now=now,
        )

    def prepare_auto_with_provider(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousAutoBlueprint:
        """Use BYOK semantic routing, then build a reconciled automatic blueprint."""

        self._assert_selection_promotion_admitted()
        candidates = self._resolve_candidates(
            model_candidates,
            allow_empty=kwargs.get("domain_policy_mode") == "strict",
        )
        resolved_credentials = self._credential_mapping(credentials)
        selection_overrides = kwargs.pop("selection_overrides", None)
        if self.health_ledger is not None:
            selection_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(), selection_overrides
            )
        return self.orchestrator.prepare_auto_with_provider(
            task=task,
            model_candidates=candidates,
            credentials=resolved_credentials,
            selection_overrides=selection_overrides,
            **kwargs,
        )

    def plan_with_provider(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousPlanRefinementResult:
        """Ask a BYOK provider to prioritize an existing blueprint's reviewed workflow stages."""

        kwargs = self._prompt_learning_options(kwargs)
        self._assert_selection_promotion_admitted()
        candidates = self._resolve_candidates(
            model_candidates,
            allow_empty=kwargs.get("domain_policy_mode") == "strict",
        )
        resolved_credentials = self._credential_mapping(credentials)
        selection_overrides = kwargs.pop("selection_overrides", None)
        if self.health_ledger is not None:
            selection_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(), selection_overrides
            )
        return self.orchestrator.plan_with_provider(
            blueprint=blueprint,
            model_candidates=candidates,
            credentials=resolved_credentials,
            selection_overrides=selection_overrides,
            **kwargs,
        )

    def plan_ordered_steps_with_provider(
        self,
        *,
        task: str,
        steps: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousOrderedStepPlanRefinementResult:
        """Ask a BYOK provider to order an existing dependency-closed step graph."""

        kwargs = self._prompt_learning_options(kwargs)
        self._assert_selection_promotion_admitted()
        candidates = self._resolve_candidates(
            model_candidates,
            allow_empty=kwargs.get("domain_policy_mode") == "strict",
        )
        resolved_credentials = self._credential_mapping(credentials)
        selection_overrides = kwargs.pop("selection_overrides", None)
        if self.health_ledger is not None:
            selection_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(), selection_overrides
            )
        return self.orchestrator.plan_ordered_steps_with_provider(
            task=task,
            steps=steps,
            model_candidates=candidates,
            credentials=resolved_credentials,
            selection_overrides=selection_overrides,
            **kwargs,
        )

    def plan_cross_domain_with_provider(
        self,
        *,
        blueprint: AutonomousCrossDomainBlueprint,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousCrossDomainPlanRefinementResult:
        """Ask a BYOK provider to prioritize an existing cross-domain fan-out."""

        kwargs = self._prompt_learning_options(kwargs)
        self._assert_selection_promotion_admitted()
        candidates = self._resolve_candidates(
            model_candidates,
            allow_empty=kwargs.get("domain_policy_mode") == "strict",
        )
        resolved_credentials = self._credential_mapping(credentials)
        selection_overrides = kwargs.pop("selection_overrides", None)
        if self.health_ledger is not None:
            selection_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(), selection_overrides
            )
        return self.orchestrator.plan_cross_domain_with_provider(
            blueprint=blueprint,
            model_candidates=candidates,
            credentials=resolved_credentials,
            selection_overrides=selection_overrides,
            **kwargs,
        )

    def prepare_cross_domain(self, **kwargs: Any) -> AutonomousCrossDomainBlueprint:
        """Build bounded specialist fan-out and synthesis work without provider contact."""

        return self.orchestrator.prepare_cross_domain(**kwargs)

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentPlanningMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
