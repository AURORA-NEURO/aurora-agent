"""Focused autonomous-agent evidence methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_DOMAIN_PACK_SCHEMA,
    Any,
    ArgumentError,
    AutonomousAuthorizationContext,
    AutonomousClaimIntegrityAcquisitionBinding,
    AutonomousClaimIntegrityAssessment,
    AutonomousClaimIntegrityClaim,
    AutonomousClaimIntegrityEvidenceAuthority,
    AutonomousClaimIntegrityEvidence,
    AutonomousClaimIntegrityPolicy,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorDispatchResult,
    AutonomousConnectorIntentFacade,
    AutonomousConnectorOperationFacade,
    AutonomousConnectorOperationRegistry,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorSelectionPlan,
    AutonomousCrossDomainResponseAssessment,
    AutonomousEvidencePlan,
    AutonomousEvidenceRuntime,
    AutonomousEvidenceRuntimeJournal,
    AutonomousEvidenceRuntimeResult,
    AutonomousOutcomeIntegrityAssessment,
    AutonomousOutcomeIntegrityClaimBinding,
    AutonomousOutcomeIntegrityRun,
    AutonomousTaskBlueprint,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialSession,
    Mapping,
    ModelCandidate,
    Sequence,
    _authorize_launch_admission_domains,
    _build_domain_capability_contracts,
    _connector_plan_domains_for_launch_admission,
    _launch_admission_domains,
    _mission_domains_for_launch_admission,
    assess_autonomous_claim_integrity,
    assess_autonomous_cross_domain_response_set,
    assess_autonomous_outcome_integrity,
    bind_autonomous_claim_integrity_acquisition_requests,
    bind_autonomous_outcome_integrity_claims,
    content_digest,
    plan_autonomous_claim_integrity_acquisition,
    project_autonomous_outcome_integrity_run,
    reassess_autonomous_claim_integrity,
    settle_autonomous_claim_integrity_acquisition,
    register_builtin_autonomous_connectors,
    register_builtin_autonomous_domain_connectors,
)

class AutonomousAgentEvidenceMixin:
    def plan_information_acquisition(
        self,
        *,
        task: str,
        candidates: Sequence[Mapping[str, Any] | Any],
        domains: Sequence[str] | None = None,
        hints: Sequence[str] = (),
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        policy: Mapping[str, Any] | Any | None = None,
        satisfied_candidate_ids: Sequence[str] = (),
    ) -> Any:
        """Choose the next evidence/context acquisitions without dispatching anything.

        Automatic intake binds the plan to the deterministic route digest.  Explicit ``domains``
        are caller-selected and therefore get an explicit-domain digest rather than being
        presented as semantic routing evidence.  The returned plan is the input to a caller's
        reviewed evidence queue; this method never contacts a source, provider, tool, learner,
        or credential store.
        """

        from .autonomous_information_acquisition import plan_autonomous_information_acquisition

        if not isinstance(task, str) or not task.strip() or "\x00" in task or len(task.encode("utf-8")) > 32_000:
            raise BrainRunError("information acquisition task is outside its bound")
        task_digest = content_digest({"task": task})
        if domains is None:
            route = self.route(
                task=task,
                hints=hints,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
            )
            if route.abstained:
                raise BrainRunError("information acquisition planning requires route review before candidate selection")
            requested_domains = route.selected_domains or ((route.primary_domain,) if route.primary_domain else ())
            route_digest = route.route_digest
        else:
            requested_domains = tuple(domains)
            route_digest = content_digest({"schema": "bioprism-python-explicit-information-domains/0.1", "domains": list(requested_domains)})
        if not requested_domains:
            raise BrainRunError("information acquisition planning requires at least one routed domain")
        return plan_autonomous_information_acquisition(
            task_digest=task_digest,
            route_digest=route_digest,
            candidates=candidates,
            requested_domains=requested_domains,
            policy=policy,
            satisfied_candidate_ids=satisfied_candidate_ids,
        )

    def assess_claim_integrity(
        self,
        *,
        task: str,
        claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
        evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
        reference_time: str,
        policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
    ) -> AutonomousClaimIntegrityAssessment:
        """Fuse evidence metadata before a provider or effect can rely on a claim.

        This is intentionally adjacent to information acquisition planning: the result's
        provider-free actions can become the next candidate capabilities, while source dispatch,
        contradiction resolution, and reproduction remain separately approved caller work.
        """

        if not isinstance(task, str) or not task.strip() or "\x00" in task or len(task.encode("utf-8")) > 32_000:
            raise BrainRunError("claim integrity task is outside its bound")
        return assess_autonomous_claim_integrity(
            context_digest=content_digest({"task": task}),
            claims=claims,
            evidence=evidence,
            reference_time=reference_time,
            policy=policy,
        )

    def reassess_claim_integrity(
        self,
        previous: AutonomousClaimIntegrityAssessment,
        *,
        claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
        evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
        reference_time: str,
        policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
    ) -> AutonomousClaimIntegrityAssessment:
        """Continue a claim decision chain after caller-owned evidence changes."""

        return reassess_autonomous_claim_integrity(
            previous,
            claims=claims,
            evidence=evidence,
            reference_time=reference_time,
            policy=policy,
        )

    def settle_claim_integrity_acquisition(
        self,
        previous: AutonomousClaimIntegrityAssessment,
        bridge: Any,
        binding: AutonomousClaimIntegrityAcquisitionBinding,
        execution: Any,
        *,
        claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
        existing_evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
        acquired_evidence: Sequence[Mapping[str, Any]],
        reference_time: str,
        evidence_authority: AutonomousClaimIntegrityEvidenceAuthority,
        policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
    ) -> AutonomousClaimIntegrityAssessment:
        """Reassess caller-reviewed claim evidence against its exact evaluated source receipt."""

        return settle_autonomous_claim_integrity_acquisition(
            previous,
            bridge,
            binding,
            execution,
            claims=claims,
            existing_evidence=existing_evidence,
            acquired_evidence=acquired_evidence,
            reference_time=reference_time,
            evidence_authority=evidence_authority,
            policy=policy,
        )

    def plan_claim_integrity_acquisition(
        self,
        assessment: AutonomousClaimIntegrityAssessment,
        *,
        candidates: Sequence[Mapping[str, Any] | Any],
        policy: Mapping[str, Any] | Any | None = None,
        requested_domains: Sequence[str] | None = None,
    ) -> Any:
        """Translate unresolved claim actions into a reviewed, provider-free acquisition plan."""

        return plan_autonomous_claim_integrity_acquisition(
            assessment,
            candidates=candidates,
            policy=policy,
            requested_domains=requested_domains,
        )

    def bind_claim_integrity_acquisition(
        self,
        bridge: Any,
        requests: Sequence[Mapping[str, Any]],
    ) -> AutonomousClaimIntegrityAcquisitionBinding:
        """Bind one caller-owned request to each candidate selected by an integrity bridge.

        The returned binding is metadata-only when serialized.  Its transient request batch is
        ordered by the information planner and carries assessment, bridge, plan, candidate, and
        candidate-digest metadata into the existing reviewed evidence runtime.
        """

        return bind_autonomous_claim_integrity_acquisition_requests(bridge, requests)

    def execute_claim_integrity_acquisition(
        self,
        bridge: Any,
        registry: Any,
        requests: Sequence[Mapping[str, Any]],
        *,
        prepare_options: Mapping[str, Any] | None = None,
        execute_options: Mapping[str, Any] | None = None,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> Any:
        """Execute only the reviewed evidence requests selected for unresolved claims.

        This closes the integrity-to-source queue without collapsing authorization boundaries:
        ``approve_source_dispatch`` remains required by the evidence controller, and provider
        contracts/evaluators remain caller-owned.  A fresh evidence plan is compiled for exactly
        the selected candidate domains and the controller rechecks readiness before dispatch.
        """

        from .autonomous_evidence_execution import AutonomousEvidenceExecutionController

        binding = self.bind_claim_integrity_acquisition(bridge, requests)
        plan = bridge.acquisition_plan
        if plan is None:
            raise ArgumentError("integrity acquisition bridge has no executable plan")
        evidence_plan = self.evidence_plan(
            plan.selected_domains,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )
        preparation = dict(prepare_options or {})
        health_store = preparation.pop("health_store", None)
        controller = AutonomousEvidenceExecutionController(registry, health_store)
        execution_plan = controller.prepare(evidence_plan, **preparation)
        execution = dict(execute_options or {})
        if "provider_contracts" not in execution and "provider_contracts" in preparation:
            execution["provider_contracts"] = preparation["provider_contracts"]
        return controller.execute(execution_plan, evidence_plan, binding.requests, **execution)

    def execute_claim_integrity_acquisition_resumable(
        self,
        bridge: Any,
        registry: Any,
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        checkpoint_store: Any,
        reconciliation_authority_id: str | None = None,
        reconciliation_authority_version: str | None = None,
        reconciliation_authority_config_digest: str | None = None,
        prepare_options: Mapping[str, Any] | None = None,
        execute_options: Mapping[str, Any] | None = None,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> Any:
        """Resume the integrity-selected evidence queue through the existing checkpoint fence."""

        binding = self.bind_claim_integrity_acquisition(bridge, requests)
        plan = bridge.acquisition_plan
        if plan is None:
            raise ArgumentError("integrity acquisition bridge has no executable plan")
        return self.execute_reviewed_evidence_resumable(
            registry,
            plan.selected_domains,
            binding.requests,
            job_id=job_id,
            checkpoint_store=checkpoint_store,
            reconciliation_authority_id=reconciliation_authority_id,
            reconciliation_authority_version=reconciliation_authority_version,
            reconciliation_authority_config_digest=reconciliation_authority_config_digest,
            prepare_options=prepare_options,
            execute_options=execute_options,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )

    def project_outcome_integrity_run(
        self,
        result: Any,
        *,
        task_digest: str | None = None,
        domain: str | None = None,
    ) -> AutonomousOutcomeIntegrityRun:
        """Project a direct, automatic, or cross-domain result into a reliance identity."""

        return project_autonomous_outcome_integrity_run(
            result,
            task_digest=task_digest,
            domain=domain,
        )

    def bind_outcome_integrity_claims(
        self,
        result: Any,
        bindings: Sequence[Mapping[str, Any] | AutonomousOutcomeIntegrityClaimBinding],
        *,
        task_digest: str | None = None,
        domain: str | None = None,
    ) -> tuple[AutonomousOutcomeIntegrityClaimBinding, ...]:
        """Bind explicit claim declarations to the exact result output and response digest."""

        run = self.project_outcome_integrity_run(result, task_digest=task_digest, domain=domain)
        return bind_autonomous_outcome_integrity_claims(run, bindings)

    def assess_outcome_integrity(
        self,
        result: Any,
        *,
        claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
        evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
        claim_bindings: Sequence[Mapping[str, Any] | AutonomousOutcomeIntegrityClaimBinding],
        reference_time: str,
        policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
        response_assessment: AutonomousCrossDomainResponseAssessment | None = None,
        require_completed_run: bool = True,
        require_response_assessment: bool = False,
        require_synthesis: bool = False,
        task_digest: str | None = None,
        domain: str | None = None,
    ) -> AutonomousOutcomeIntegrityAssessment:
        """Run the final provider-free claim/reliance gate for one exact autonomous outcome.

        The result remains transient.  The returned assessment contains only digests, counts,
        statuses, and next actions; it does not turn a provider response into external truth or
        authorize a new provider, source, tool, effect, or evaluator settlement.
        """

        run = self.project_outcome_integrity_run(result, task_digest=task_digest, domain=domain)
        return assess_autonomous_outcome_integrity(
            run=run,
            claims=claims,
            evidence=evidence,
            claim_bindings=claim_bindings,
            reference_time=reference_time,
            policy=policy,
            response_assessment=response_assessment,
            require_completed_run=require_completed_run,
            require_response_assessment=require_response_assessment,
            require_synthesis=require_synthesis,
        )

    def assess_cross_domain_responses(
        self,
        responses: Sequence[Mapping[str, Any]],
        *,
        task: str | None = None,
        context_digest: str | None = None,
        requested_domains: Sequence[str] | None = None,
        alignments: Sequence[Mapping[str, Any]] = (),
        require_synthesis: bool = False,
        require_complete_alignment: bool = True,
        minimum_reward: float = 0.8,
        minimum_alignment_confidence: float = 0.75,
        contradiction_confidence_threshold: float = 0.75,
    ) -> AutonomousCrossDomainResponseAssessment:
        """Gate structured specialist outputs before cross-domain synthesis.

        ``responses`` are transient caller/provider values.  The returned assessment keeps only
        response/evaluation digests, bounded scores, explicit alignment metadata, and next
        actions.  Supplying ``task`` creates a digest-only context binding; its text is never
        retained by this façade.
        """

        if task is not None:
            if context_digest is not None:
                raise BrainRunError("cross-domain response assessment accepts task or context_digest, not both")
            if not isinstance(task, str) or not task.strip() or "\x00" in task or len(task.encode("utf-8")) > 32_000:
                raise BrainRunError("cross-domain response assessment task is outside its bound")
            context_digest = content_digest({"task": task})
        return assess_autonomous_cross_domain_response_set(
            responses,
            requested_domains=requested_domains,
            context_digest=context_digest,
            alignments=alignments,
            require_synthesis=require_synthesis,
            require_complete_alignment=require_complete_alignment,
            minimum_reward=minimum_reward,
            minimum_alignment_confidence=minimum_alignment_confidence,
            contradiction_confidence_threshold=contradiction_confidence_threshold,
        )

    def evidence_plan(
        self,
        domains: Sequence[str] = AUTONOMOUS_DOMAINS,
        *,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> AutonomousEvidencePlan:
        """Compile the reviewed evidence contract without dispatching providers or tools."""

        return self.orchestrator.evidence_plan(
            domains,
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
        """Create a bounded runtime for caller-owned evidence acquisition and evaluation."""

        return self.orchestrator.evidence_runtime(
            domains,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
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
        """Run evidence acquisition through explicit caller-owned adapters."""

        return self.orchestrator.acquire_evidence(
            domains,
            requests,
            acquirer=acquirer,
            projector=projector,
            evaluator=evaluator,
            rehydrate_value=rehydrate_value,
            parent_evidence_digests=parent_evidence_digests,
            stop_on_failure=stop_on_failure,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            journal=journal,
            authorization_context=authorization_context,
            authorization_domain=authorization_domain,
            authorization_capability=authorization_capability,
            authorization_risk_class=authorization_risk_class,
        )

    def prepare_reviewed_evidence(
        self,
        registry: Any,
        domains: Sequence[str] = AUTONOMOUS_DOMAINS,
        *,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        **options: Any,
    ) -> Any:
        """Prepare a generic adapter execution plan without contacting a source.

        This is the high-level bridge for applications that use one agent across files,
        browsers, databases, scientific sources, enterprise systems, and other reviewed
        connectors.  ``options`` are forwarded to the typed execution controller's planning
        boundary; source dispatch remains impossible until the matching execute method receives
        explicit approval.
        """

        from .autonomous_evidence_execution import AutonomousEvidenceExecutionController

        if not hasattr(registry, "registry_digest"):
            raise ArgumentError("prepare_reviewed_evidence requires an evidence adapter registry")
        evidence_plan = self.evidence_plan(
            domains,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )
        return AutonomousEvidenceExecutionController(registry, options.pop("health_store", None)).prepare(evidence_plan, **options)

    def execute_reviewed_evidence(
        self,
        registry: Any,
        domains: Sequence[str],
        requests: Sequence[Mapping[str, Any]],
        *,
        prepare_options: Mapping[str, Any] | None = None,
        execute_options: Mapping[str, Any] | None = None,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> Any:
        """Run the complete generic reviewed-evidence lifecycle through the agent facade."""

        from .autonomous_evidence_execution import AutonomousEvidenceExecutionController

        plan = self.evidence_plan(domains, available_evidence=available_evidence, completed_stages=completed_stages)
        preparation = dict(prepare_options or {})
        health_store = preparation.pop("health_store", None)
        controller = AutonomousEvidenceExecutionController(registry, health_store)
        execution_plan = controller.prepare(plan, **preparation)
        return controller.execute(execution_plan, plan, requests, **dict(execute_options or {}))

    def execute_reviewed_evidence_resumable(
        self,
        registry: Any,
        domains: Sequence[str],
        requests: Sequence[Mapping[str, Any]],
        *,
        job_id: str,
        checkpoint_store: Any,
        reconciliation_authority_id: str | None = None,
        reconciliation_authority_version: str | None = None,
        reconciliation_authority_config_digest: str | None = None,
        prepare_options: Mapping[str, Any] | None = None,
        execute_options: Mapping[str, Any] | None = None,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> Any:
        """Execute or resume generic reviewed evidence with a digest-bound checkpoint."""

        from .autonomous_evidence_execution import AutonomousEvidenceExecutionController
        from .autonomous_evidence_execution_resumable import AutonomousEvidenceExecutionResumableController

        plan = self.evidence_plan(domains, available_evidence=available_evidence, completed_stages=completed_stages)
        preparation = dict(prepare_options or {})
        health_store = preparation.pop("health_store", None)
        controller = AutonomousEvidenceExecutionController(registry, health_store)
        execution_plan = controller.prepare(plan, **preparation)
        return AutonomousEvidenceExecutionResumableController(
            controller,
            checkpoint_store,
            job_id,
            reconciliation_authority_id=reconciliation_authority_id,
            reconciliation_authority_version=reconciliation_authority_version,
            reconciliation_authority_config_digest=reconciliation_authority_config_digest,
        ).run(
            execution_plan,
            plan,
            requests,
            **dict(execute_options or {}),
        )

    def run_with_reviewed_evidence(
        self,
        *,
        task: str,
        requests: Sequence[Mapping[str, Any]],
        acquirer: Any,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        domains: Sequence[str] | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        projector: Any | None = None,
        evaluator: Any | None = None,
        rehydrate_value: Callable[[Mapping[str, Any]], Any] | None = None,
        parent_evidence_digests: Sequence[str] = (),
        stop_on_failure: bool = False,
        reevaluate_pending: bool = False,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        journal: AutonomousEvidenceRuntimeJournal | None = None,
        approve_source_dispatch: bool = False,
        allow_incomplete_evidence: bool = False,
        approve_provider_call: bool = False,
        provider_run_override: Any | None = None,
        before_provider_run: Callable[[Any], None] | None = None,
        prompt_builder: Callable[[AutonomousEvidenceRuntimeResult], Mapping[str, Any]] | None = None,
        run_mode: str = "auto",
        run_options: Mapping[str, Any] | None = None,
    ) -> Any:
        """Acquire accepted evidence and compose it with the ordinary autonomous run.

        This is the façade-level bridge between the provider-free evidence runtime and model
        execution.  The bridge keeps source dispatch approval, evidence acceptance, and provider
        approval independent.  Its serialized result is metadata-only; raw evidence values,
        prompt projections, and provider responses remain transient caller-owned objects.
        """

        from .autonomous_evidence_brain import run_autonomous_evidence_backed

        return run_autonomous_evidence_backed(
            self,
            task=task,
            requests=requests,
            acquirer=acquirer,
            credentials=credentials,
            domains=domains,
            model_candidates=model_candidates,
            projector=projector,
            evaluator=evaluator,
            rehydrate_value=rehydrate_value,
            parent_evidence_digests=parent_evidence_digests,
            stop_on_failure=stop_on_failure,
            reevaluate_pending=reevaluate_pending,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            journal=journal,
            approve_source_dispatch=approve_source_dispatch,
            allow_incomplete_evidence=allow_incomplete_evidence,
            approve_provider_call=approve_provider_call,
            provider_run_override=provider_run_override,
            before_provider_run=before_provider_run,
            prompt_builder=prompt_builder,
            run_mode=run_mode,
            run_options=run_options,
        )

    def run_with_reviewed_evidence_with_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Acquire reviewed evidence only after its complete domain scope is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _launch_admission_domains(kwargs.get("domains")),
        )
        return self.run_with_reviewed_evidence(**kwargs)

    def run_with_llm_evidence(
        self,
        *,
        task: str,
        requests: Sequence[Mapping[str, Any]],
        adapter: Any,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        projector: Any | None = None,
        **kwargs: Any,
    ) -> Any:
        """Run reviewed evidence acquisition through a provider-backed adapter or router.

        ``AutonomousLLMEvidenceAdapter`` and ``AutonomousLLMEvidenceAdapterRouter`` both expose
        ``acquire``.  A router also exposes ``project``; a single adapter uses its
        ``project_value`` method.  The remaining keyword arguments are passed to
        :meth:`run_with_reviewed_evidence`, so source approval, evidence acceptance, provider
        approval, journaling, and model-routing policy stay visible at the call site.
        """

        if not callable(getattr(adapter, "acquire", None)):
            raise ArgumentError("LLM evidence adapter must expose a callable acquire method")
        selected_projector = projector
        if selected_projector is None:
            candidate = getattr(adapter, "project", None)
            if callable(candidate):
                selected_projector = candidate
            else:
                candidate = getattr(adapter, "project_value", None)
                if callable(candidate):
                    selected_projector = candidate
        return self.run_with_reviewed_evidence(
            task=task,
            requests=requests,
            acquirer=adapter,
            credentials=credentials,
            projector=selected_projector,
            **kwargs,
        )

    def run_with_llm_evidence_with_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run adapter-backed evidence acquisition only after its domain scope is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _launch_admission_domains(kwargs.get("domains")),
        )
        return self.run_with_llm_evidence(**kwargs)

    def run_with_domain_evidence_catalogue(
        self,
        *,
        task: str,
        catalogue: Any,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        domains: Sequence[str] | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        available_evidence: Sequence[str] = (),
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        prepare_options: Mapping[str, Any] | None = None,
        prepare_for_requirement: Callable[[Any], Mapping[str, Any]] | None = None,
        execute_options: Mapping[str, Any] | None = None,
        max_parallel_requirements: int | None = None,
        allow_incomplete_evidence: bool = False,
        approve_source_dispatch: bool = False,
        approve_provider_call: bool = False,
        provider_run_override: Any | None = None,
        before_provider_run: Callable[[Any], None] | None = None,
        prompt_builder: Callable[[Any], Mapping[str, Any]] | None = None,
        run_mode: str = "auto",
        run_options: Mapping[str, Any] | None = None,
    ) -> Any:
        """Compose reviewed catalogue routes with the ordinary autonomous brain lifecycle.

        Each workflow evidence requirement gets its own digest-bound catalogue reconciliation.
        Source dispatch, evidence acceptance, and provider approval remain separate; the default
        prompt contains only reconciliation metadata, while ``prompt_builder`` is the explicit
        transient bridge for caller-owned values. The resulting envelope keeps typed preparation
        and reconciliation objects available to the caller but serializes only digests and status.
        """

        from .autonomous_domain_evidence_brain import run_autonomous_domain_evidence_backed

        return run_autonomous_domain_evidence_backed(
            self,
            task=task,
            catalogue=catalogue,
            credentials=credentials,
            domains=domains,
            model_candidates=model_candidates,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            prepare_options=prepare_options,
            prepare_for_requirement=prepare_for_requirement,
            execute_options=execute_options,
            max_parallel_requirements=max_parallel_requirements,
            allow_incomplete_evidence=allow_incomplete_evidence,
            approve_source_dispatch=approve_source_dispatch,
            approve_provider_call=approve_provider_call,
            provider_run_override=provider_run_override,
            before_provider_run=before_provider_run,
            prompt_builder=prompt_builder,
            run_mode=run_mode,
            run_options=run_options,
        )

    def run_with_domain_evidence_catalogue_with_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run catalogue-backed evidence workflows only after their domain scope is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _launch_admission_domains(kwargs.get("domains")),
        )
        return self.run_with_domain_evidence_catalogue(**kwargs)

    def run_resumable_evidence_backed(self, **kwargs: Any) -> Any:
        """Run or resume reviewed evidence-backed work through a caller checkpoint sink.

        The resumable boundary never replays a provider result implicitly.  Callers must provide
        a caller-owned evidence journal and either rehydrate an observed provider result or pass
        an explicit provider-resume decision.
        """

        from .autonomous_evidence_backed_resumable import run_autonomous_evidence_backed_resumable

        return run_autonomous_evidence_backed_resumable(self, **kwargs)

    def run_resumable_evidence_backed_with_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run or resume evidence-backed work only after its complete scope is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _launch_admission_domains(kwargs.get("domains")),
        )
        return self.run_resumable_evidence_backed(**kwargs)

    def run_resumable_llm_evidence(
        self,
        *,
        task: str,
        requests: Sequence[Mapping[str, Any]],
        adapter: Any,
        projector: Any | None = None,
        **kwargs: Any,
    ) -> Any:
        """Run or resume provider-backed reviewed evidence with the same explicit adapter seam."""

        if not callable(getattr(adapter, "acquire", None)):
            raise ArgumentError("LLM evidence adapter must expose a callable acquire method")
        selected_projector = projector
        if selected_projector is None:
            candidate = getattr(adapter, "project", None)
            if callable(candidate):
                selected_projector = candidate
            else:
                candidate = getattr(adapter, "project_value", None)
                if callable(candidate):
                    selected_projector = candidate
        return self.run_resumable_evidence_backed(
            task=task,
            requests=requests,
            acquirer=adapter,
            projector=selected_projector,
            **kwargs,
        )

    def run_resumable_llm_evidence_with_launch_admission(
        self,
        *,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run or resume adapter-backed evidence only after its domain scope is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _launch_admission_domains(kwargs.get("domains")),
        )
        return self.run_resumable_llm_evidence(**kwargs)

    def evidence_backed_controller(self, job_id: str, persistence: Any) -> Any:
        """Create a serialized, optionally CAS-fenced evidence-backed restart controller."""

        from .autonomous_evidence_backed_resumable import AutonomousEvidenceBackedController

        return AutonomousEvidenceBackedController(self, job_id, persistence)

    def workflows(self) -> list[dict[str, Any]]:
        """Return the deterministic workflow contracts available to automatic intake."""

        return self.orchestrator.workflow_registry.catalogue()

    def domain_packs(self) -> list[dict[str, Any]]:
        """Return reviewed capability/evidence contracts for every configured domain."""

        return self.orchestrator.pack_registry.catalogue()

    def domain_pack(self, domain: str) -> dict[str, Any]:
        """Return one metadata-only domain pack without exposing task or credential material."""

        return self.orchestrator.pack_registry.resolve(domain).to_dict()

    def domain_pack_tool_plan(self, domain: str) -> dict[str, Any]:
        """Show how registered tools cover a pack without granting or executing a tool."""

        pack = self.orchestrator.pack_registry.resolve(domain)
        registered = [] if self.tool_registry is None else self.tool_registry.tools_for((domain,))
        available = sorted({tool.capability for tool in registered})
        required = set(pack.tool_capabilities)
        profile = self.orchestrator.registry.resolve(domain)
        workflow = self.orchestrator.workflow_registry.resolve(domain)
        contracts = _build_domain_capability_contracts(profile, pack, workflow)
        adapted_available = sorted(
            {
                contract.capability
                for contract in contracts
                if any(tool.capability in contract.tool_capabilities for tool in registered)
            }
        )
        return {
            "schema": AUTONOMOUS_DOMAIN_PACK_SCHEMA,
            "domain": domain,
            "pack_id": pack.pack_id,
            "pack_digest": pack.pack_digest,
            "required_tool_capabilities": list(pack.tool_capabilities),
            "available_tool_capabilities": available,
            "covered_tool_capabilities": sorted(required.intersection(available)),
            "missing_tool_capabilities": sorted(required.difference(available)),
            "capability_adapters": [contract.to_dict() for contract in contracts],
            "adapted_covered_capabilities": adapted_available,
            "adapted_missing_capabilities": sorted(required.difference(adapted_available)),
            "adapter_posture": "reviewed_exact_aliases; no_fuzzy_matching",
            "registered_tool_count": len(registered),
            "execution": "metadata_only; registration_is_not_authorization",
        }

    def connector_catalogue(self) -> dict[str, Any]:
        """Return the redacted caller-owned connector catalogue, if one is configured."""

        if self.connector_registry is None:
            return {
                "schema": AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
                "digest": None,
                "connectors": [],
                "connector_count": 0,
                "execution": "metadata_only;no_connector_registry_configured",
                "secret_material": "never_returned",
            }
        return self.connector_registry.to_dict()

    def register_builtin_connectors(
        self,
        *,
        operation_registry: Any | None = None,
        connector_id: str = "builtin.offline-evidence",
        version: str = "1.0.0",
        approval_required: bool = True,
        replace: bool = False,
        receipt_sink: Callable[[Any], Any] | None = None,
        receipt_store: Any | None = None,
    ) -> Any:
        """Install the credentialless all-domain connector adapter.

        The adapter is intentionally local and deterministic. It normalizes caller-supplied
        metadata into transient observations; it does not discover sources, invoke a provider,
        or turn a registration into authorization. If the agent has no connector runtime yet,
        this method creates one so the returned registration is immediately usable through the
        same selection, approval, receipt, and replay gates as an external connector.
        """

        if self.connector_registry is None:
            self.connector_registry = AutonomousConnectorRegistry()
        if self.connector_runtime is None:
            self.connector_runtime = AutonomousConnectorRuntime(
                self.connector_registry,
                receipt_sink=receipt_sink,
                receipt_store=receipt_store,
            )
        elif receipt_sink is not None or receipt_store is not None:
            raise BrainRunError(
                "receipt_sink and receipt_store must be supplied when creating the connector runtime"
            )
        try:
            return register_builtin_autonomous_connectors(
                self.connector_registry,
                operation_registry,
                connector_id=connector_id,
                version=version,
                approval_required=approval_required,
                replace=replace,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("built-in connector registration failed") from error

    def register_builtin_domain_connectors(
        self,
        *,
        operation_registry: Any | None = None,
        connector_id: str = "builtin.offline-evidence",
        version: str = "1.0.0",
        approval_required: bool = True,
        replace: bool = False,
        receipt_sink: Callable[[Any], Any] | None = None,
        receipt_store: Any | None = None,
    ) -> tuple[Any, ...]:
        """Install one exact-capability credentialless connector for every domain.

        Domain-scoped manifests preserve the complete reviewed operation vocabulary within the
        provider-manifest capability bound. This portfolio is the recommended registration for
        :meth:`run_connector_workflow`; the single all-domain registration remains useful for
        compact probes and broad routing tests.
        """

        if self.connector_registry is None:
            self.connector_registry = AutonomousConnectorRegistry()
        if self.connector_runtime is None:
            self.connector_runtime = AutonomousConnectorRuntime(
                self.connector_registry,
                receipt_sink=receipt_sink,
                receipt_store=receipt_store,
            )
        elif receipt_sink is not None or receipt_store is not None:
            raise BrainRunError(
                "receipt_sink and receipt_store must be supplied when creating the connector runtime"
            )
        try:
            return register_builtin_autonomous_domain_connectors(
                self.connector_registry,
                operation_registry,
                connector_id=connector_id,
                version=version,
                approval_required=approval_required,
                replace=replace,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("built-in domain connector registration failed") from error

    def run_connector_workflow(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        checkpoint: Any | None = None,
        run_id: str | None = None,
        approved: bool = False,
        retry_blocked: bool = False,
        max_stage_calls: int | None = None,
        request_for_stage: Callable[[Any], Mapping[str, Any]] | None = None,
        rehydrate_payload: Callable[[Any], Any] | None = None,
        operation_registry: Any | None = None,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        evidence_runtime: Any | None = None,
        evidence_projector: Any | None = None,
        evidence_evaluator: Any | None = None,
        require_evidence_acceptance: bool | None = None,
        parent_evidence_digests: Sequence[str] = (),
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> Any:
        """Run a blueprint's workflow DAG through reviewed connectors without provider credentials.

        This is a separate execution mode from :meth:`run_workflow`: it preserves the same
        workflow checkpoint/status model, but each stage is dispatched through the configured
        connector registry. Provider invocation is never implicit, and replayed payloads require
        caller-owned rehydration by digest.
        """

        from .autonomous_connector_workflow import run_autonomous_connector_workflow

        if self.connector_runtime is None:
            raise BrainRunError("connector runtime is not configured")
        try:
            return run_autonomous_connector_workflow(
                self.connector_runtime,
                blueprint=blueprint,
                checkpoint=checkpoint,
                run_id=run_id,
                approved=approved,
                retry_blocked=retry_blocked,
                max_stage_calls=max_stage_calls,
                request_for_stage=request_for_stage,
                rehydrate_payload=rehydrate_payload,
                operation_registry=operation_registry,
                selection_signals=selection_signals,
                evidence_runtime=evidence_runtime,
                evidence_projector=evidence_projector,
                evidence_evaluator=evidence_evaluator,
                require_evidence_acceptance=require_evidence_acceptance,
                parent_evidence_digests=parent_evidence_digests,
                trace_event_callback=trace_event_callback,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("connector workflow execution failed") from error

    def run_connector_workflow_with_launch_admission(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run a connector workflow only after the blueprint domain passes launch admission."""

        domain = getattr(getattr(blueprint, "spec", None), "domain", None)
        _authorize_launch_admission_domains(launch_admission, (domain,))
        return self.run_connector_workflow(blueprint=blueprint, **kwargs)

    def run_connector_mission(
        self,
        *,
        mission: Any,
        checkpoint: Any | None = None,
        approved: bool = False,
        retry_blocked: bool = False,
        max_step_calls: int | None = None,
        request_for_step: Callable[[Any], Mapping[str, Any]] | None = None,
        rehydrate_payload: Callable[[Any], Any] | None = None,
        resume_outputs: Mapping[str, Any] | None = None,
        operation_registry: Any | None = None,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
        feedback_ledger: Any | None = None,
        feedback_by_step: Mapping[str, Mapping[str, Any]] | None = None,
        quality_evaluator: Callable[[Any], Mapping[str, Any]] | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
    ) -> Any:
        """Execute a typed mission DAG through reviewed connectors without model credentials.

        This path preserves the existing ``MissionRequest`` graph while replacing provider-backed
        tool dispatch with exact connector selection.  Mission checkpoints retain only digests;
        caller-owned payload/output rehydration is required after restart, and evaluator rewards
        are accepted only through the explicit feedback ledger.
        """

        from .autonomous_connector_mission import run_autonomous_connector_mission

        if self.connector_runtime is None:
            raise BrainRunError("connector runtime is not configured")
        try:
            return run_autonomous_connector_mission(
                self.connector_runtime,
                mission=mission,
                checkpoint=checkpoint,
                approved=approved,
                retry_blocked=retry_blocked,
                max_step_calls=max_step_calls,
                request_for_step=request_for_step,
                rehydrate_payload=rehydrate_payload,
                resume_outputs=resume_outputs,
                operation_registry=operation_registry,
                selection_signals=selection_signals,
                feedback_ledger=feedback_ledger,
                feedback_by_step=feedback_by_step,
                quality_evaluator=quality_evaluator,
                trace_event_callback=trace_event_callback,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("connector mission execution failed") from error

    def run_connector_mission_with_provider_planning(
        self,
        *,
        mission: Any,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        provider_planning_options: Mapping[str, Any] | None = None,
        accepted_plan_refinement: Any | None = None,
        accept_plan: bool = False,
        execution_options: Mapping[str, Any] | None = None,
    ) -> Any:
        """Plan an existing connector mission, then execute only after explicit acceptance.

        The provider sees a redacted step catalogue containing identifiers, domains,
        capabilities, objectives, dependencies, and required flags.  It cannot see connector
        arguments or receive a dispatch authority.  ``accepted_plan_refinement`` is the
        restart-safe caller-owned replay path: pass the previously reviewed value-only proposal
        to avoid replaying the planner provider on resume.  ``execution_options`` is forwarded
        only after the plan passes every contract check.

        A planning review or provider error returns a metadata-rich
        ``AutonomousConnectorPlannedMissionRun`` with no connector calls.  A completed plan is
        still not enough to execute; ``accept_plan=True`` and the ordinary connector approval in
        ``execution_options`` are both required.
        """

        from .autonomous_connector_mission import (
            AutonomousConnectorPlannedMissionRun,
            _normalize_request,
            apply_autonomous_ordered_step_plan,
            connector_mission_planner_steps,
            connector_mission_protected_contract_digest,
        )

        if not isinstance(accept_plan, bool):
            raise BrainRunError("connector mission accept_plan must be boolean")
        if provider_planning_options is not None and not isinstance(provider_planning_options, Mapping):
            raise BrainRunError("provider_planning_options must be a mapping or None")
        if execution_options is not None and not isinstance(execution_options, Mapping):
            raise BrainRunError("execution_options must be a mapping or None")
        if accepted_plan_refinement is not None and provider_planning_options is not None:
            raise BrainRunError(
                "accepted_plan_refinement cannot be combined with provider_planning_options; "
                "choose a live planning pass or caller-owned replay"
            )
        request, steps = _normalize_request(mission)
        protected_contract_digest = connector_mission_protected_contract_digest(request, steps=steps)
        if accepted_plan_refinement is None:
            planning_options = {} if provider_planning_options is None else dict(provider_planning_options)
            forbidden = {
                "task",
                "steps",
                "credentials",
                "model_candidates",
                "protected_contract_digest",
            }
            invalid = sorted(forbidden.intersection(planning_options))
            if invalid:
                raise BrainRunError(
                    "provider_planning_options cannot override protected planner inputs: "
                    + ", ".join(invalid)
                )
            refinement = self.plan_ordered_steps_with_provider(
                task=request.goal,
                steps=connector_mission_planner_steps(steps),
                credentials=credentials,
                model_candidates=model_candidates,
                protected_contract_digest=protected_contract_digest,
                **planning_options,
            )
        else:
            refinement = accepted_plan_refinement

        from .autonomy import AutonomousOrderedStepPlanRefinementResult

        if not isinstance(refinement, AutonomousOrderedStepPlanRefinementResult):
            raise BrainRunError("provider planning did not return an ordered-step refinement")
        if refinement.status == "approval_required":
            result_status = "planning_approval_required"
        elif refinement.status in {"policy_review_required"}:
            result_status = "planning_policy_review_required"
        elif refinement.status in {"policy_blocked"}:
            result_status = "planning_policy_blocked"
        elif refinement.status in {"provider_invalid"}:
            result_status = "planning_provider_invalid"
        elif refinement.status in {"provider_disagreement", "plan_refused"}:
            result_status = "planning_provider_disagreement"
        elif refinement.status != "completed" or refinement.review_required or not accept_plan:
            result_status = "planning_review_required"
        else:
            planned_mission = apply_autonomous_ordered_step_plan(
                request,
                refinement,
                protected_contract_digest=protected_contract_digest,
            )
            options = {} if execution_options is None else dict(execution_options)
            if "mission" in options:
                raise BrainRunError("execution_options cannot override the planned mission")
            execution = self.run_connector_mission(mission=planned_mission, **options)
            return AutonomousConnectorPlannedMissionRun(
                status=execution.status,
                mission=planned_mission,
                protected_contract_digest=protected_contract_digest,
                plan_refinement=refinement,
                execution=execution,
            )

        return AutonomousConnectorPlannedMissionRun(
            status=result_status,
            mission=request,
            protected_contract_digest=protected_contract_digest,
            plan_refinement=refinement,
            execution=None,
        )

    def run_connector_mission_with_provider_planning_and_launch_admission(
        self,
        *,
        mission: Any,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        provider_planning_options: Mapping[str, Any] | None = None,
        accepted_plan_refinement: Any | None = None,
        accept_plan: bool = False,
        execution_options: Mapping[str, Any] | None = None,
    ) -> Any:
        """Run provider planning and connector execution behind the domain launch gate.

        Domain admission occurs before planner credential resolution, provider invocation, or
        connector setup.  The planner and connector still retain their independent approvals.
        """

        from .autonomous_connector_mission import _normalize_request

        _request, steps = _normalize_request(mission)
        _authorize_launch_admission_domains(
            launch_admission,
            tuple(dict.fromkeys(step.domain for step in steps)),
        )
        return self.run_connector_mission_with_provider_planning(
            mission=mission,
            credentials=credentials,
            model_candidates=model_candidates,
            provider_planning_options=provider_planning_options,
            accepted_plan_refinement=accepted_plan_refinement,
            accept_plan=accept_plan,
            execution_options=execution_options,
        )

    def run_connector_mission_with_launch_admission(
        self,
        *,
        mission: Any,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> Any:
        """Run a connector mission only after every domain-labelled step is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _mission_domains_for_launch_admission(mission),
        )
        return self.run_connector_mission(mission=mission, **kwargs)

    def connector_selection_plan(
        self,
        domains: Sequence[str],
        *,
        capability: str | None = None,
        selection_signals: Mapping[str, Mapping[str, Any]] | None = None,
    ) -> AutonomousConnectorSelectionPlan:
        """Build a review-only connector route plan through the configured registry.

        Supplying ``selection_signals`` opts into weighted evidence selection; the signals are
        caller/evaluator-owned and are reduced to bounded scores and a digest in the plan. This
        method never invokes a connector or grants approval.
        """

        if self.connector_registry is None:
            raise BrainRunError("connector registry is not configured")
        try:
            if selection_signals is None:
                return self.connector_registry.select_for_domains(domains, capability=capability)
            if capability is None:
                raise BrainRunError("adaptive connector selection requires capability")
            return self.connector_registry.select_adaptive_for_domains(
                domains,
                capability=capability,
                selection_signals=selection_signals,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("connector selection planning failed") from error

    def connector_operation_facade(
        self,
        *,
        operation_registry: Any | None = None,
    ) -> AutonomousConnectorOperationFacade:
        """Return the high-level operation facade for the configured connector runtime.

        This keeps the lower-level dispatch API available for infrastructure callers while
        giving application code one typed entrypoint for operation validation, exact connector
        selection, approval binding, and replay.  The returned facade still performs no network
        I/O itself; all external behavior remains inside the caller-owned connector executor.
        """

        if self.connector_registry is None or self.connector_runtime is None:
            raise BrainRunError("connector runtime is not configured")
        if operation_registry is not None and not isinstance(
            operation_registry, AutonomousConnectorOperationRegistry
        ):
            raise BrainRunError("operation_registry must be an AutonomousConnectorOperationRegistry")
        return AutonomousConnectorOperationFacade(
            self.connector_registry,
            self.connector_runtime,
            operation_registry,
        )

    def connector_intent_facade(
        self,
        *,
        operation_registry: Any | None = None,
    ) -> AutonomousConnectorIntentFacade:
        """Return a task-to-operation facade spanning the built-in autonomous domains.

        Planning uses the same reviewed domain router as provider-backed autonomous runs, then
        resolves exact operation/capability labels from the connector catalogue.  It never
        turns task text into an authorization decision; connector approval, effects, and replay
        remain enforced by the operation facade.
        """

        return AutonomousConnectorIntentFacade(
            self.connector_operation_facade(operation_registry=operation_registry),
            self.route,
        )

    def dispatch_connector(
        self,
        plan: AutonomousConnectorSelectionPlan | Mapping[str, Any],
        request: AutonomousConnectorDispatchRequest,
        *,
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        authorization_domain: str | None = None,
        authorization_capability: str | None = None,
        authorization_risk_class: str | None = None,
    ) -> AutonomousConnectorDispatchResult:
        """Dispatch one connector only through a configured, plan-verifying runtime."""

        if self.connector_runtime is None:
            raise BrainRunError("connector runtime is not configured")
        try:
            return self.connector_runtime.dispatch_from_plan(
                plan,
                request,
                trace_event_callback=trace_event_callback,
                authorization_context=authorization_context,
                authorization_domain=authorization_domain,
                authorization_capability=authorization_capability,
                authorization_risk_class=authorization_risk_class,
            )
        except (ArgumentError, BrainRunError):
            raise
        except Exception as error:
            raise BrainRunError("connector dispatch failed") from error

    def dispatch_connector_with_launch_admission(
        self,
        plan: AutonomousConnectorSelectionPlan | Mapping[str, Any],
        request: AutonomousConnectorDispatchRequest,
        *,
        launch_admission: Mapping[str, Any],
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        authorization_domain: str | None = None,
        authorization_capability: str | None = None,
        authorization_risk_class: str | None = None,
    ) -> AutonomousConnectorDispatchResult:
        """Dispatch a reviewed connector plan only after every plan domain is admitted."""

        _authorize_launch_admission_domains(
            launch_admission,
            _connector_plan_domains_for_launch_admission(plan),
        )
        return self.dispatch_connector(
            plan,
            request,
            trace_event_callback=trace_event_callback,
            authorization_context=authorization_context,
            authorization_domain=authorization_domain,
            authorization_capability=authorization_capability,
            authorization_risk_class=authorization_risk_class,
        )

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentEvidenceMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, staticmethod) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
