"""Focused autonomous-agent learning methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_DOMAIN_LEARNING_STATE_SCHEMA,
    AUTONOMOUS_PLANNING_QUALITY_SETTLEMENT_SCHEMA,
    Any,
    ArgumentError,
    AutonomousAutoResult,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousDomainResponseContract,
    AutonomousLearningResult,
    AutonomousMemoryConsolidationObservation,
    AutonomousOrderedStepPlanRefinementResult,
    AutonomousPlanRefinementResult,
    AutonomousPromptAdaptiveSelection,
    BRAIN_CONTEXT_LEARNING_STATE_SCHEMA,
    BrainEvaluatorDecision,
    BrainLearningEpisode,
    BrainLearningLedger,
    BrainLearningTrajectory,
    BrainLearningTrajectoryResult,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    BrainToolLoopResult,
    Callable,
    DomainEvaluatorRegistry,
    MAX_AUTONOMY_CONTEXT_BYTES,
    Mapping,
    Sequence,
    _context_identity_digest,
    _ensure_bandit_arm,
    _identifier,
    _json_digest,
    _planner_context_binding,
    _safe_json,
    _sequence,
    _structured_response_decision,
    _structured_response_evaluation,
    _structured_response_result,
    extract_autonomous_prompt_learning_selections,
    math,
    replay_autonomous_domain_response_evaluation,
    validate_autonomous_domain_response_evaluation,
)

class AutonomousAgentLearningMixin:
    def learning_state(self) -> dict[str, Any]:
        """Return the latest caller-persisted bandit state or a first-run exploration state."""

        if self.ledger is not None:
            state = self.ledger.latest_state()
            if state is not None:
                return state
        return {
            "schema": "bioprism-brain-bandit/0.1",
            "generation": 0,
            "arms": [],
        }

    def consolidate_memory(
        self,
        observations: Sequence[Mapping[str, Any] | AutonomousMemoryConsolidationObservation],
        *,
        generation: int | None = None,
    ) -> dict[str, Any]:
        """Consolidate explicit evaluator observations into the configured lesson index.

        The index is deliberately separate from episodic recall: only value-only evaluator
        observations may promote a lesson, while a caller-owned resolver controls any transient
        text returned to a future prompt.
        """

        if self.memory_consolidator is None:
            raise BrainRunError("memory_consolidator is not configured")
        return self.memory_consolidator.consolidate(observations, generation=generation)

    def memory_references(
        self,
        *,
        domain: str,
        capability: str | None = None,
        lesson_resolver: Callable[[str], str | None] | None = None,
        lesson_context_resolver: Callable[[Mapping[str, Any]], str | None] | None = None,
        limit: int = 8,
    ) -> list[dict[str, Any]]:
        """Return stable lesson references with transient caller-owned text.

        The context-aware resolver receives the requested domain, capability, scope, and
        evaluator confidence so a deployment can authorize text lookup before prompt assembly.
        """

        if self.memory_consolidator is None:
            raise BrainRunError("memory_consolidator is not configured")
        return self.memory_consolidator.prompt_references(
            domain=domain,
            capability=capability,
            lesson_resolver=lesson_resolver,
            lesson_context_resolver=lesson_context_resolver,
            limit=limit,
        )

    def domain_learning_state(
        self,
        domain: str,
        *,
        capability: str | None = None,
        risk_class: str | None = None,
        task_family: str | None = None,
    ) -> dict[str, Any]:
        """Return the evaluator-linked bandit state for one built-in domain context.

        The returned state is directly usable as ``bandit_state`` for a domain-scoped run. The
        lookup is keyed by the stable domain/capability/risk identity, not by task text, so every
        domain can accumulate separate model feedback without leaking prompts or provider data.
        """

        profile = self.orchestrator.registry.resolve(domain)
        resolved_capability = profile.default_capability if capability is None else _identifier("learning capability", capability)
        resolved_risk = profile.risk_class if risk_class is None else _identifier("learning risk_class", risk_class)
        resolved_task_family = None if task_family is None else _identifier("learning task_family", task_family)
        context = {
            "domain": profile.domain,
            "capability": resolved_capability,
            "risk_class": resolved_risk,
            "task_family": resolved_task_family,
        }
        if self.ledger is not None:
            snapshot = self.ledger.contextual_state(context)
        else:
            snapshot = {
                "schema": BRAIN_CONTEXT_LEARNING_STATE_SCHEMA,
                "context": context,
                "context_digest": _context_identity_digest(context),
                "bandit_state": self.learning_state(),
                "observed": False,
                "evaluation_count": 0,
                "last_evaluator_id": None,
                "last_evaluator_version": None,
                "retention": "context_identity_and_evaluator_bandit_metadata_only",
            }
        evaluator = self.domain_evaluator(profile.domain)
        result = {
            "schema": AUTONOMOUS_DOMAIN_LEARNING_STATE_SCHEMA,
            "domain": profile.domain,
            "capability": resolved_capability,
            "risk_class": resolved_risk,
            "task_family": resolved_task_family,
            "context_digest": snapshot["context_digest"],
            "evaluator": {
                "domain": profile.evaluator_domain,
                "evaluator_id": evaluator.evaluator_id,
                "evaluator_version": evaluator.evaluator_version,
            },
            "bandit_state": dict(snapshot["bandit_state"]),
            "observed": bool(snapshot["observed"]),
            "evaluation_count": snapshot["evaluation_count"],
            "last_evaluator_id": snapshot["last_evaluator_id"],
            "last_evaluator_version": snapshot["last_evaluator_version"],
            "learning_authority": "explicit_domain_evaluator_feedback_only",
            "retention": "context_identity_evaluator_metadata_and_bandit_state_only",
        }
        return _safe_json("autonomous domain learning state", result, maximum=MAX_AUTONOMY_CONTEXT_BYTES)

    def domain_learning_coverage(self, domains: Sequence[str] | None = None) -> dict[str, Any]:
        """Summarize evaluator-linked learning readiness across every autonomous domain."""

        selected = tuple(AUTONOMOUS_DOMAINS) if domains is None else _sequence(
            "domain learning coverage domains", domains, maximum=len(AUTONOMOUS_DOMAINS)
        )
        unknown = sorted(set(selected).difference(AUTONOMOUS_DOMAINS))
        if unknown:
            raise BrainRunError("domain learning coverage contains unknown domains: " + ", ".join(unknown))
        rows: list[dict[str, Any]] = []
        for domain in selected:
            state = self.domain_learning_state(domain)
            bandit = state["bandit_state"]
            arms = bandit.get("arms", []) if isinstance(bandit, Mapping) else []
            valid_arms = [arm for arm in arms if isinstance(arm, Mapping)]
            rows.append(
                {
                    "domain": domain,
                    "capability": state["capability"],
                    "risk_class": state["risk_class"],
                    "context_digest": state["context_digest"],
                    "observed": state["observed"],
                    "evaluation_count": state["evaluation_count"],
                    "generation": bandit.get("generation", 0) if isinstance(bandit, Mapping) else 0,
                    "arm_count": len(valid_arms),
                    "explored_arm_count": sum(
                        1
                        for arm in valid_arms
                        if isinstance(arm.get("pulls"), int) and arm.get("pulls", 0) > 0
                    ),
                    "evaluator": dict(state["evaluator"]),
                }
            )
        return {
            "schema": "bioprism-python-autonomous-domain-learning-coverage/0.1",
            "domains": list(selected),
            "domain_count": len(rows),
            "rows": rows,
            "learning_authority": "explicit_domain_evaluator_feedback_only",
            "state_access": "agent.domain_learning_state(domain, capability, risk_class)",
            "secret_material": "never_returned",
        }

    def calibrate_evaluators(
        self,
        cases: Sequence[Mapping[str, Any]],
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        domains: Sequence[str] | None = None,
        seed: str = "default",
        holdout_fraction: float = 0.2,
        bins: int = 10,
        min_calibration_cases_per_domain: int = 4,
        min_holdout_cases_per_domain: int = 2,
        max_expected_calibration_error: float = 0.15,
        max_brier_score: float = 0.15,
        require_all_domains: bool = True,
    ) -> dict[str, Any]:
        """Calibrate the reviewed domain evaluator catalogue without provider calls.

        The caller owns the evidence cases and labels.  The agent returns only aggregate metrics
        and digests, so this method is safe to use as a pre-admission gate before enabling
        evaluator-linked online learning or learned model selection.
        """

        from .autonomous_evaluator_calibration import calibrate_autonomous_evaluators

        try:
            return calibrate_autonomous_evaluators(
                cases,
                registry=evaluator_registry,
                domains=domains,
                seed=seed,
                holdout_fraction=holdout_fraction,
                bins=bins,
                min_calibration_cases_per_domain=min_calibration_cases_per_domain,
                min_holdout_cases_per_domain=min_holdout_cases_per_domain,
                max_expected_calibration_error=max_expected_calibration_error,
                max_brier_score=max_brier_score,
                require_all_domains=require_all_domains,
            )
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("evaluator calibration was rejected") from error

    def replay_evaluator_calibration(
        self,
        report: Mapping[str, Any],
        cases: Sequence[Mapping[str, Any]],
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
    ) -> dict[str, Any]:
        """Replay calibration against caller-owned cases and expose catalogue/case drift."""

        from .autonomous_evaluator_calibration import replay_autonomous_evaluator_calibration

        try:
            return replay_autonomous_evaluator_calibration(
                report,
                cases,
                registry=evaluator_registry,
            )
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("evaluator calibration replay was rejected") from error

    def admit_evaluator_calibration(self, report: Mapping[str, Any], domain: str) -> dict[str, Any]:
        """Return a digest-bound, domain-scoped decision for enabling learned updates."""

        from .autonomous_evaluator_calibration import admit_autonomous_evaluator_calibration

        try:
            return admit_autonomous_evaluator_calibration(report, domain)
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("evaluator calibration admission was rejected") from error

    def learning_controller(
        self,
        *,
        calibration_report: Mapping[str, Any] | None = None,
        require_calibrated_learning: bool = False,
        ledger: BrainLearningLedger | None = None,
    ) -> Any:
        """Build the calibration-gated settlement controller for delayed learning updates.

        The import is intentionally lazy so the existing agent construction path does not pay
        for SQLite/persistence dependencies unless a caller enables the delayed-feedback path.
        """

        from .autonomous_learning_controller import AutonomousLearningController

        return AutonomousLearningController(
            self.brain,
            ledger=self.ledger if ledger is None else ledger,
            calibration_report=calibration_report,
            require_calibrated_learning=require_calibrated_learning,
        )

    def deployment_readiness(
        self,
        *,
        policy: Mapping[str, Any] | None = None,
        capabilities: Mapping[str, Any] | None = None,
        readiness_options: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Join this agent's keyless readiness with deployment-owned capability gates.

        The auditor is intentionally lazy and provider-free: it returns a review artifact but
        never resolves credentials, initializes persistence/queues, acquires evidence, or grants
        authority.  ``readiness_options`` is forwarded to the existing local readiness projection.
        """

        from .autonomous_deployment_readiness import audit_autonomous_agent_deployment_readiness

        return audit_autonomous_agent_deployment_readiness(
            self,
            policy=policy,
            capabilities=capabilities,
            readiness_options=readiness_options,
        )

    def domain_audit(
        self,
        *,
        available_tool_names: Sequence[str] | None = None,
        available_evidence: Sequence[str] | None = None,
        completed_stages: Mapping[str, Sequence[str]] | None = None,
    ) -> dict[str, Any]:
        """Audit this agent's all-domain contracts before any dispatch.

        The audit reads the agent's bound profile/workflow registries and, when present, the
        registered tool names.  It never resolves credentials, contacts a provider, acquires
        evidence, executes a tool, mutates learning, or treats registration as authorization.
        ``available_evidence`` remains caller-owned because the agent cannot infer source truth
        from a provider response or a tool registration.
        """

        from .autonomous_domain_audit import audit_autonomous_agent_domain_contracts

        return audit_autonomous_agent_domain_contracts(
            self,
            available_tool_names=available_tool_names,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )

    def domain_operating_kit(self, domain: str) -> Any:
        """Return one complete provider-free operating contract for a built-in domain."""

        from .autonomous_domain_operating_kit import build_autonomous_domain_operating_kit

        return build_autonomous_domain_operating_kit(domain)

    def domain_operating_kits(self, domains: Sequence[str] | None = None) -> tuple[Any, ...]:
        """Return deterministic operating contracts for the requested built-in domains."""

        from .autonomous_domain_operating_kit import build_autonomous_domain_operating_kits

        return build_autonomous_domain_operating_kits(domains)

    def validate_domain_operating_kit(self, value: Mapping[str, Any] | Any) -> Any:
        """Rebuild and validate a caller-held operating contract against current metadata."""

        from .autonomous_domain_operating_kit import validate_autonomous_domain_operating_kit

        return validate_autonomous_domain_operating_kit(value)

    def launch_preflight(
        self,
        *,
        available_tool_names: Sequence[str] | None = None,
        available_evidence: Sequence[str] | None = None,
        completed_stages: Mapping[str, Sequence[str]] | None = None,
        readiness_options: Mapping[str, Any] | None = None,
        deployment_policy: Mapping[str, Any] | Any | None = None,
        deployment_capabilities: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Compose every local launch gate into one all-domain review artifact.

        This is deliberately a preflight projection, not an activation or authorization call.
        It joins contract, model/provider, evidence, deployment, and learning posture while
        consuming no credential, contacting no provider/source, executing no tool, and mutating
        no learner.  Callers still own approval, source truth, credential provisioning, effect
        authorization, and durable runtime scheduling.
        """

        from .autonomous_launch_preflight import audit_autonomous_agent_launch_preflight

        return audit_autonomous_agent_launch_preflight(
            self,
            available_tool_names=available_tool_names,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
            readiness_options=readiness_options,
            deployment_policy=deployment_policy,
            deployment_capabilities=deployment_capabilities,
        )

    def launch_admission(
        self,
        preflight_report: Mapping[str, Any] | None = None,
        *,
        decision: str,
        approved_domains: Sequence[str] | None = None,
        authorization_digest: str | None = None,
        reason: str | None = None,
        admission_id: str = "autonomous-launch-admission",
    ) -> dict[str, Any]:
        """Bind an explicit caller review decision to one launch-preflight digest.

        The resulting value-only admission record is useful to a deployment-owned scheduler or
        approval service, but it does not grant provider, source, tool, credential, learner, queue,
        or effect authority.  When ``preflight_report`` is omitted, a fresh provider-free preflight
        is generated before the decision is recorded.
        """

        from .autonomous_launch_admission import create_autonomous_launch_admission

        report = self.launch_preflight() if preflight_report is None else preflight_report
        return create_autonomous_launch_admission(
            report,
            decision=decision,
            approved_domains=approved_domains,
            authorization_digest=authorization_digest,
            reason=reason,
            admission_id=admission_id,
        )

    def domain_evaluator(
        self,
        domain: str,
        *,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        fallback_domain: str | None = None,
    ) -> BrainOutcomeEvaluator:
        """Resolve the reviewed value-only evaluator for one autonomous domain."""

        registry = evaluator_registry or DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
        if not isinstance(registry, DomainEvaluatorRegistry):
            raise BrainRunError("evaluator_registry must be a DomainEvaluatorRegistry or None")
        evaluator = registry.resolve_for_autonomous_domain(domain, fallback_domain=fallback_domain)
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("domain evaluator registry returned an invalid evaluator")
        return evaluator

    def prepare_learning_episode(
        self,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
        *,
        evidence: Mapping[str, Any] | None = None,
        arm_id: str | None = None,
        episode_id: str | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningEpisode:
        """Persist a value-only delayed-feedback handle for a completed agent result."""

        return self.brain.prepare_learning_episode(
            result,
            evidence=evidence,
            arm_id=arm_id,
            episode_id=episode_id,
            ledger=self.ledger if ledger is None else ledger,
        )

    @staticmethod
    def restore_learning_episode(value: Mapping[str, Any]) -> BrainLearningEpisode:
        """Validate a caller-rehydrated episode projection before delayed settlement."""

        return BrainLearningEpisode.from_mapping(value)

    def settle_planning_quality(
        self,
        plan: AutonomousPlanRefinementResult
        | AutonomousCrossDomainPlanRefinementResult
        | AutonomousOrderedStepPlanRefinementResult,
        *,
        domain: str,
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        failed: bool | None = None,
        evidence_digest: str | None = None,
        feedback_digest: str | None = None,
        capability: str = "planning",
        risk_class: str = "planning_review",
        task_family: str | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> dict[str, Any]:
        """Credit a completed provider planning proposal independently from execution quality.

        Planning has no provider episode of its own, so this method creates a metadata-only
        planning identity and submits it through the same Rust bandit outcome boundary used by
        executions. The plan proposal is projected by digest and stage/child/step ids only. Model
        transport health is never incremented; the optional health ledger receives an evaluator
        quality observation keyed by the planning outcome digest.
        """

        if not isinstance(
            plan,
            (
                AutonomousPlanRefinementResult,
                AutonomousCrossDomainPlanRefinementResult,
                AutonomousOrderedStepPlanRefinementResult,
            ),
        ):
            raise BrainRunError("planning quality requires a plan refinement result")
        if plan.status != "completed":
            return {
                "schema": AUTONOMOUS_PLANNING_QUALITY_SETTLEMENT_SCHEMA,
                "status": "not_eligible",
                "plan_refinement": plan.to_dict(),
                "planner_context": None,
                "planner_context_digest": None,
                "evaluation": None,
                "next_state": None,
                "model_quality": None,
                "reason": "planning_proposal_not_completed",
                "retention": "value_only_planning_quality_no_payloads",
                "secret_material": "never_returned",
            }
        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError("planning quality domain must be a built-in autonomous domain")
        if not isinstance(evaluator_id, str) or not evaluator_id.strip() or not isinstance(evaluator_version, str) or not evaluator_version.strip():
            raise BrainRunError("planning quality evaluator identity must be non-empty")
        if isinstance(reward, bool) or not isinstance(reward, (int, float)) or not math.isfinite(float(reward)) or not -1.0 <= float(reward) <= 1.0:
            raise BrainRunError("planning quality reward must be finite and within [-1, 1]")
        if not isinstance(passed, bool):
            raise BrainRunError("planning quality passed must be boolean")
        effective_failed = not passed if failed is None else failed
        if not isinstance(effective_failed, bool) or (passed and effective_failed):
            raise BrainRunError("planning quality failed must be boolean and cannot conflict with passed")
        decision = BrainEvaluatorDecision(
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            reward=float(reward),
            passed=passed,
            failed=effective_failed,
            feedback_digest=feedback_digest,
            failure_class=None,
            evidence_digest=evidence_digest,
            replan_requested=False,
            replan_instruction=None,
        )
        selected = plan.selected_model
        if not isinstance(selected, Mapping):
            raise BrainRunError("completed planning quality result is missing selected_model")
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not provider.strip() or not isinstance(model, str) or not model.strip():
            raise BrainRunError("planning quality selected_model is malformed")
        required_digests = {
            "selection_digest": plan.selection_digest,
            "planner_prompt_digest": plan.planner_prompt_digest,
            "planner_plan_digest": plan.planner_plan_digest,
            "outcome_digest": plan.outcome_digest,
        }
        if any(not isinstance(value, str) or len(value) != 64 for value in required_digests.values()):
            raise BrainRunError("completed planning quality result is missing a planning digest")
        if plan.planner_context is None and plan.planner_context_digest is None:
            for name, value in (("capability", capability), ("risk_class", risk_class)):
                if not isinstance(value, str) or not value.strip():
                    raise BrainRunError(f"planning quality {name} must be a non-empty string")
            if task_family is not None and (not isinstance(task_family, str) or not task_family.strip()):
                raise BrainRunError("planning quality task_family must be a non-empty string or None")
            context = {
                "domain": domain,
                "capability": capability,
                "risk_class": risk_class,
                "task_family": task_family,
            }
            context_digest = _context_identity_digest(context)
        else:
            context, context_digest = _planner_context_binding(
                plan.planner_context,
                plan.planner_context_digest,
                "planning quality planner_context",
            )
        planning_identity = {
            "kind": "planning_quality",
            "plan_outcome_digest": plan.outcome_digest,
            "selection_digest": plan.selection_digest,
            "planner_plan_digest": plan.planner_plan_digest,
            "base_plan_digest": plan.base_plan_digest,
        }
        if isinstance(plan, AutonomousOrderedStepPlanRefinementResult):
            # Ordered-step proposals protect a caller-owned graph contract. Include that
            # identity in the learning key so two equally successful orderings for different
            # graphs cannot replay each other's credit.
            planning_identity["protected_contract_digest"] = plan.protected_contract_digest
        planning_outcome_digest = _json_digest(planning_identity)
        current_state = self.learning_state() if bandit_state is None else dict(bandit_state)
        normalized_state = _ensure_bandit_arm(
            current_state,
            f"{provider}/{model}",
            context_digest=context_digest,
            context=context,
        )
        replay = {
            "schema": "bioprism-python-autonomous-planning-quality-replay/0.1",
            "plan_outcome_digest": plan.outcome_digest,
            "planning_outcome_digest": planning_outcome_digest,
            "selection_digest": plan.selection_digest,
            "evaluator_id": evaluator_id,
            "evaluator_version": evaluator_version,
            "retention": "metadata_and_digests_only",
        }
        report = self.brain.workspace.tool(
            "brain_outcome_record",
            {
                "run": {
                    "run_id": f"planning:{planning_outcome_digest}",
                    "selection_digest": plan.selection_digest,
                    "prompt_digest": plan.planner_prompt_digest,
                    "plan_digest": plan.planner_plan_digest,
                    "provider": provider,
                    "model": model,
                    "outcome_digest": planning_outcome_digest,
                    "request_id": None,
                },
                "assessment": decision.to_dict(),
                "bandit_state": normalized_state,
                "arm_id": f"{provider}/{model}",
                "context_digest": context_digest,
                "context": context,
                "idempotency_key": f"planning:{plan.outcome_digest}",
            },
        )
        if not isinstance(report, Mapping) or not report.get("ok"):
            raise BrainRunError("brain planning quality recording returned a refusal")
        if ledger is not None:
            ledger.append(report, context_digest=context_digest, replay=replay)
        next_state = report.get("next_state")
        if not isinstance(next_state, Mapping):
            raise BrainRunError("brain planning quality recording returned no next_state")

        quality_base = {
            "provider": provider,
            "model": model,
            "domain": domain,
            "capability": capability,
            "risk_class": risk_class,
            "evaluator_id": evaluator_id,
            "evaluator_version": evaluator_version,
            "reward": max(0.0, min(1.0, float(reward))),
            "passed": passed,
            "outcome_digest": planning_outcome_digest,
            "evidence_digest": evidence_digest,
            "feedback_digest": feedback_digest,
            "retention": "metadata_only_model_quality_no_payloads",
            "secret_material": "never_returned",
        }
        if self.health_ledger is None:
            model_quality = {"status": "not_configured", **quality_base}
        else:
            try:
                receipt = self.health_ledger.record_evaluation(
                    provider=provider,
                    model=model,
                    domain=domain,
                    capability=capability,
                    risk_class=risk_class,
                    evaluator_id=evaluator_id,
                    evaluator_version=evaluator_version,
                    reward=quality_base["reward"],
                    passed=passed,
                    outcome_digest=planning_outcome_digest,
                    evidence_digest=evidence_digest,
                    feedback_digest=feedback_digest,
                )
                model_quality = {"status": "recorded", **quality_base, "health_record_digest": receipt["record_digest"], "replayed": bool(receipt.get("replayed", False))}
            except Exception as error:
                model_quality = {"status": "failed", **quality_base, "error_class": type(error).__name__}
        return {
            "schema": AUTONOMOUS_PLANNING_QUALITY_SETTLEMENT_SCHEMA,
            "status": "settled",
            "plan_refinement": plan.to_dict(),
            "planner_context": dict(context),
            "planner_context_digest": context_digest,
            "evaluation": decision.to_dict(),
            "next_state": dict(next_state),
            "model_quality": model_quality,
            "reason": None,
            "retention": "value_only_planning_quality_no_payloads",
            "secret_material": "never_returned",
        }

    def settle_auto_planning_quality(
        self,
        result: AutonomousAutoResult,
        *,
        evaluator_id: str,
        evaluator_version: str,
        reward: float,
        passed: bool,
        domain: str | None = None,
        **kwargs: Any,
    ) -> dict[str, Any]:
        """Settle the provider planner attached to an automatic route result.

        This convenience boundary accepts the result returned by ``run_auto`` and refuses when
        routing or provider planning paused before a completed proposal. Cross-domain planning is
        scoped to the explicit ``cross_domain`` context unless the caller supplies another reviewed
        domain identity.
        """

        if not isinstance(result, AutonomousAutoResult):
            raise BrainRunError("automatic planning quality requires an AutonomousAutoResult")
        if result.planning is None:
            return {
                "schema": AUTONOMOUS_PLANNING_QUALITY_SETTLEMENT_SCHEMA,
                "status": "not_eligible",
                "plan_refinement": None,
                "evaluation": None,
                "next_state": None,
                "model_quality": None,
                "reason": "planning_proposal_not_completed",
                "retention": "value_only_planning_quality_no_payloads",
                "secret_material": "never_returned",
            }
        resolved_domain = domain or ("cross_domain" if result.route.cross_domain else result.route.primary_domain)
        if not isinstance(resolved_domain, str) or not resolved_domain:
            raise BrainRunError("automatic planning quality cannot infer a planner domain")
        return self.settle_planning_quality(
            result.planning,
            domain=resolved_domain,
            evaluator_id=evaluator_id,
            evaluator_version=evaluator_version,
            reward=reward,
            passed=passed,
            **kwargs,
        )

    def _record_model_quality_feedback(
        self,
        episode: BrainLearningEpisode,
        decision: BrainEvaluatorDecision,
        *,
        credited_reward: float | None = None,
    ) -> dict[str, Any]:
        """Project explicit evaluator credit into model-health without touching transport health.

        The Python health ledger is intentionally append-only and value-only. A quality record
        never increments provider attempts or changes a circuit; it only contributes to the
        model arm's quality prior. The raw evaluator decision may use the wider ``[-1, 1]``
        learning range, while the routing prior is bounded to ``[0, 1]`` by clamping.
        """

        selected = episode.evaluation_input.get("selected_model")
        context = episode.evaluation_input.get("context")
        context_map = context if isinstance(context, Mapping) else {}
        if not isinstance(selected, Mapping):
            return {
                "status": "failed",
                "error_class": "missing_selected_model",
                "retention": "metadata_only_model_quality_no_payloads",
                "secret_material": "never_returned",
            }
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str) or not provider.strip() or not model.strip():
            return {
                "status": "failed",
                "error_class": "malformed_selected_model",
                "retention": "metadata_only_model_quality_no_payloads",
                "secret_material": "never_returned",
            }
        base = {
            "provider": provider,
            "model": model,
            "domain": str(context_map.get("domain") or "unknown_domain"),
            "capability": str(context_map.get("capability") or "unknown_capability"),
            "risk_class": str(context_map.get("risk_class") or "unknown_risk"),
            "evaluator_id": decision.evaluator_id,
            "evaluator_version": decision.evaluator_version,
            "reward": max(0.0, min(1.0, float(decision.reward))),
            "passed": decision.passed,
            "outcome_digest": episode.evaluation_input.get("outcome_digest"),
            "evidence_digest": decision.evidence_digest,
            "feedback_digest": decision.feedback_digest,
            "retention": "metadata_only_model_quality_no_payloads",
            "secret_material": "never_returned",
        }
        if self.health_ledger is None:
            return {"status": "not_configured", **base}
        outcome_digest = base["outcome_digest"]
        if not isinstance(outcome_digest, str):
            return {"status": "failed", **base, "error_class": "missing_outcome_digest"}
        try:
            receipt = self.health_ledger.record_evaluation(
                provider=provider,
                model=model,
                domain=base["domain"],
                capability=base["capability"],
                risk_class=base["risk_class"],
                evaluator_id=decision.evaluator_id,
                evaluator_version=decision.evaluator_version,
                reward=base["reward"],
                passed=decision.passed,
                outcome_digest=outcome_digest,
                evidence_digest=decision.evidence_digest,
                feedback_digest=decision.feedback_digest,
            )
            return {"status": "recorded", **base, "health_record_digest": receipt["record_digest"], "replayed": bool(receipt.get("replayed", False))}
        except Exception as error:
            return {"status": "failed", **base, "error_class": type(error).__name__}

    def settle_learning_episode(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator | None = None,
        domain: str | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        fallback_domain: str | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        evidence: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Settle delayed feedback without retaining or reloading provider content.

        A caller may provide an explicit evaluator or resolve one of the reviewed autonomous
        domain evaluators. The evidence packet is transient and the ledger receives only its
        digest plus the value-only bandit report.
        """

        if evaluator is None:
            if domain is None:
                raise BrainRunError("settle_learning_episode requires evaluator or domain")
            evaluator = self.domain_evaluator(
                domain,
                evaluator_registry=evaluator_registry,
                fallback_domain=fallback_domain,
            )
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator or None")
        decision, report = evaluator.evaluate_episode(
            self.brain,
            episode,
            bandit_state=self.learning_state() if bandit_state is None else bandit_state,
            evidence=evidence,
            ledger=self.ledger if ledger is None else ledger,
        )
        return decision, {**report, "model_quality": self._record_model_quality_feedback(episode, decision)}

    def settle_structured_response(
        self,
        result: Any,
        *,
        episode: BrainLearningEpisode | Mapping[str, Any] | None = None,
        contract: AutonomousDomainResponseContract | None = None,
        credited_reward: float | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Settle the opt-in structural response signal into the adaptive bandit.

        This boundary is deliberately separate from task-quality evaluation.  It credits only
        response composition (stage coverage, domain-field coverage, uncertainty disclosure,
        and related contract signals), while the normal domain evaluator remains responsible
        for task-specific correctness.  If an episode is omitted, a value-only episode is
        prepared and registered in the configured ledger.  Supplying ``contract`` replays the
        transient provider response before settlement; omitting it is suitable for a restarted
        evaluator worker that has retained only the validated evaluation projection.
        """

        normalized_result = _structured_response_result(result)
        raw_evaluation = _structured_response_evaluation(result)
        if raw_evaluation is None:
            raise BrainRunError(
                "structured-response settlement requires a completed structured domain response evaluation"
            )
        evaluation = validate_autonomous_domain_response_evaluation(raw_evaluation)
        if contract is not None:
            if not isinstance(contract, AutonomousDomainResponseContract):
                raise BrainRunError("structured-response contract must be an AutonomousDomainResponseContract or None")
            structured = self.orchestrator._workflow_structured_output(normalized_result)
            if structured is None:
                raise BrainRunError("structured-response replay requires a transient structured provider response")
            replay_autonomous_domain_response_evaluation(structured, contract, evaluation)
        resolved_ledger = self.ledger if ledger is None else ledger
        if episode is None:
            normalized_episode = self.prepare_learning_episode(
                normalized_result,
                ledger=resolved_ledger,
            )
        else:
            normalized_episode = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        if normalized_episode.run_id != (
            normalized_result.run_id
            if isinstance(normalized_result, BrainRunResult)
            else normalized_result.brain_run.run_id
        ):
            raise BrainRunError("structured-response episode does not belong to the supplied result")
        decision = _structured_response_decision(
            evaluation.to_dict(),
            episode_evidence_digest=normalized_episode.evidence_digest,
            credited_reward=credited_reward,
        )
        adapter = BrainOutcomeEvaluator(
            lambda _evaluation_input: decision,
            evaluator_id=decision.evaluator_id,
            evaluator_version=decision.evaluator_version,
        )
        settled_decision, report = adapter.settle_episode(
            self.brain,
            normalized_episode,
            decision=decision,
            bandit_state=self.learning_state() if bandit_state is None else bandit_state,
            ledger=resolved_ledger,
        )
        return settled_decision, {
            **report,
            "structured_response": {
                "evaluation_digest": evaluation.evaluation_digest,
                "response_digest": evaluation.response_digest,
                "credited_reward": settled_decision.reward,
                "evaluator_authority": evaluation.evaluator_authority,
                "retention": "value_only;response_and_credentials_not_retained",
            },
        }

    def settle_learning_decision(
        self,
        episode: BrainLearningEpisode | Mapping[str, Any],
        *,
        decision: BrainEvaluatorDecision,
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> tuple[BrainEvaluatorDecision, dict[str, Any]]:
        """Apply a restart-safe, already-evaluated decision without re-running a provider.

        Evaluator workers use this method when their private evidence processing completed in a
        different process. Only :class:`BrainEvaluatorDecision` crosses this seam; the episode
        ledger retains its original digests and the Rust kernel still validates the reward update.
        """

        if not isinstance(decision, BrainEvaluatorDecision):
            raise BrainRunError("decision must be a BrainEvaluatorDecision")
        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        result = evaluator.settle_episode(
            self.brain,
            episode,
            decision=decision,
            bandit_state=self.learning_state() if bandit_state is None else bandit_state,
            ledger=self.ledger if ledger is None else ledger,
        )
        settled_decision, report = result
        return settled_decision, {**report, "model_quality": self._record_model_quality_feedback(episode, settled_decision)}

    def prepare_learning_trajectory(
        self,
        results: Sequence[BrainRunResult | BrainToolLoopResult | BrainMissionResult],
        *,
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        arm_ids: Sequence[str | None] | None = None,
        trajectory_id: str | None = None,
        discount: float = 0.90,
        terminal_reward: float | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningTrajectory:
        """Register an ordered value-only trajectory for later evaluator settlement."""

        return self.brain.prepare_learning_trajectory(
            results,
            evidence_by_step=evidence_by_step,
            arm_ids=arm_ids,
            trajectory_id=trajectory_id,
            discount=discount,
            terminal_reward=terminal_reward,
            ledger=self.ledger if ledger is None else ledger,
        )

    @staticmethod
    def restore_learning_trajectory(value: Mapping[str, Any]) -> BrainLearningTrajectory:
        """Validate a caller-rehydrated trajectory projection before settlement."""

        return BrainLearningTrajectory.from_mapping(value)

    def settle_learning_trajectory(
        self,
        trajectory: BrainLearningTrajectory | Mapping[str, Any],
        *,
        evaluator: BrainOutcomeEvaluator,
        bandit_state: Mapping[str, Any] | None = None,
        evidence_by_step: Sequence[Mapping[str, Any] | None] | None = None,
        ledger: BrainLearningLedger | None = None,
    ) -> BrainLearningTrajectoryResult:
        """Apply one evaluator's discounted delayed credit across a persisted trajectory."""

        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("evaluator must be a BrainOutcomeEvaluator")
        result = evaluator.evaluate_trajectory(
            self.brain,
            trajectory,
            bandit_state=self.learning_state() if bandit_state is None else bandit_state,
            evidence_by_step=evidence_by_step,
            ledger=self.ledger if ledger is None else ledger,
        )
        for episode, decision, credited in zip(result.trajectory.episodes, result.decisions, result.credited_rewards):
            self._record_model_quality_feedback(episode, decision, credited_reward=credited)
        return result

    def _record_model_quality_from_learning_result(self, result: Any) -> None:
        """Persist quality feedback emitted by any learning orchestration shape.

        Lower-level delayed settlement uses :meth:`_record_model_quality_feedback`; the
        high-level online-learning orchestrator settles directly inside the brain for sequential
        replanning and cross-domain fan-out. This adapter closes that seam so every domain and
        every specialist/synthesis arm receives the same quality prior without requiring a
        provider replay. All failures remain diagnostic-only because the bandit settlement has
        already completed and provider execution must not be repeated.
        """

        if self.health_ledger is None:
            return

        def decision_mapping(value: Any) -> Mapping[str, Any] | None:
            candidate = value.get("decision") if isinstance(value, Mapping) else getattr(value, "decision", value)
            if hasattr(candidate, "to_dict"):
                candidate = candidate.to_dict()
            return candidate if isinstance(candidate, Mapping) else None

        pairs: list[tuple[BrainRunResult, Mapping[str, Any]]] = []
        if isinstance(result, AutonomousLearningResult):
            for brain_result, evaluation in zip(result.attempts, result.evaluations):
                if isinstance(brain_result, (BrainRunResult, BrainToolLoopResult)):
                    decision = decision_mapping(evaluation)
                    if decision is not None:
                        pairs.append((brain_result.brain_run if isinstance(brain_result, BrainToolLoopResult) else brain_result, decision))
        elif hasattr(result, "cross_domain") and hasattr(result, "evaluations"):
            cross_domain = getattr(result, "cross_domain")
            evaluations = getattr(result, "evaluations")
            brain_results = self._trace_brain_results(cross_domain)
            for brain_result, evaluation in zip(brain_results, evaluations):
                decision = decision_mapping(evaluation)
                if decision is not None:
                    pairs.append((brain_result, decision))
        elif hasattr(result, "attempts"):
            # Replan-cycle envelopes contain nested cross-domain attempts. Keep this walker
            # structural and bounded; it never scans arbitrary provider payload mappings.
            for attempt in getattr(result, "attempts", ()):
                nested = getattr(attempt, "cross_domain", None)
                evaluations = getattr(attempt, "evaluations", ())
                if nested is None:
                    continue
                for brain_result, evaluation in zip(self._trace_brain_results(nested), evaluations):
                    decision = decision_mapping(evaluation)
                    if decision is not None:
                        pairs.append((brain_result, decision))

        for brain_result, decision in pairs:
            selected = brain_result.selection.get("selected_model")
            context = brain_result.selection.get("context")
            if not isinstance(selected, Mapping) or not isinstance(context, Mapping):
                continue
            provider = selected.get("provider")
            model = selected.get("model")
            outcome_digest = brain_result.outcome_digest
            reward = decision.get("reward")
            passed = decision.get("passed")
            if not isinstance(provider, str) or not isinstance(model, str) or not isinstance(outcome_digest, str):
                continue
            if not isinstance(reward, (int, float)) or isinstance(reward, bool) or not math.isfinite(float(reward)) or not isinstance(passed, bool):
                continue
            try:
                self.health_ledger.record_evaluation(
                    provider=provider,
                    model=model,
                    domain=str(context.get("domain") or "unknown_domain"),
                    capability=str(context.get("capability") or "unknown_capability"),
                    risk_class=str(context.get("risk_class") or "unknown_risk"),
                    evaluator_id=str(decision.get("evaluator_id") or "unknown_evaluator"),
                    evaluator_version=str(decision.get("evaluator_version") or "unknown_version"),
                    reward=max(0.0, min(1.0, float(reward))),
                    passed=passed,
                    outcome_digest=outcome_digest,
                    evidence_digest=decision.get("evidence_digest"),
                    feedback_digest=decision.get("feedback_digest"),
                )
            except Exception:
                continue

    def _prompt_learning_options(self, options: Mapping[str, Any]) -> dict[str, Any]:
        """Bind configured prompt learning persistence to one high-level run."""

        if not isinstance(options, Mapping):
            raise BrainRunError("autonomous prompt learning options must be a mapping")
        resolved = dict(options)
        if self.memory_consolidator is not None:
            resolved.setdefault("memory_consolidator", self.memory_consolidator)
        coordinator = self.prompt_learning_coordinator
        if coordinator is None:
            return resolved
        supplied_state = resolved.get("prompt_learning_state")
        if supplied_state is not None and supplied_state is not coordinator.state:
            raise BrainRunError(
                "prompt_learning_state cannot override the agent's persistent prompt learner"
            )
        supplied_registry = resolved.get("prompt_registry")
        if supplied_registry is not None and supplied_registry is not coordinator.registry:
            raise BrainRunError(
                "prompt_registry must be the same registry as the agent's prompt learner"
            )
        supplied_planning_state = resolved.get("planning_prompt_learning_state")
        if supplied_planning_state is not None and supplied_planning_state is not coordinator.state:
            raise BrainRunError(
                "planning_prompt_learning_state cannot override the agent's persistent prompt learner"
            )
        supplied_planning_registry = resolved.get("planning_prompt_registry")
        if supplied_planning_registry is not None and supplied_planning_registry is not coordinator.registry:
            raise BrainRunError(
                "planning_prompt_registry must be the same registry as the agent's prompt learner"
            )
        resolved["prompt_registry"] = coordinator.registry
        resolved["prompt_learning_state"] = coordinator.state
        return resolved

    def prompt_learning_selections(self, result: Any) -> tuple[AutonomousPromptAdaptiveSelection, ...]:
        """Recover exact metadata-only prompt choices from a completed or paused run."""

        coordinator = self.prompt_learning_coordinator
        if coordinator is None:
            raise BrainRunError("prompt learning coordinator is not configured")
        try:
            return extract_autonomous_prompt_learning_selections(result, coordinator.registry)
        except ArgumentError as error:
            raise BrainRunError("autonomous prompt learning result is not settleable") from error

    def settle_prompt_learning(self, selection: AutonomousPromptAdaptiveSelection, **kwargs: Any) -> Any:
        """Apply explicit evaluator credit to one selection and persist it with CAS fencing."""

        coordinator = self.prompt_learning_coordinator
        if coordinator is None:
            raise BrainRunError("prompt learning coordinator is not configured")
        try:
            return coordinator.settle(selection, **kwargs)
        except ArgumentError as error:
            raise BrainRunError("autonomous prompt learning settlement failed") from error

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentLearningMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
