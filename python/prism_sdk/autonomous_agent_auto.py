"""Focused autonomous-agent auto methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_LEARNING_MODES,
    AUTONOMOUS_PLANNING_MODES,
    Any,
    AutonomousAutoDecisionCycleResult,
    AutonomousAutoReplanResult,
    AutonomousAutoResult,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousCrossDomainReplanResult,
    AutonomousCycleEvaluatorBridge,
    AutonomousDecisionCycle,
    AutonomousDecisionCycleRehydrationContext,
    AutonomousDecisionCycleResult,
    AutonomousDecisionCycleStateStore,
    AutonomousLearningResult,
    AutonomousPlanRefinementResult,
    AutonomousRouteCandidate,
    AutonomousRouteProposal,
    AutonomousWorkflowCheckpoint,
    BrainOutcomeEvaluator,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialSession,
    DomainEvaluatorRegistry,
    MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS,
    MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS,
    Mapping,
    ModelCandidate,
    Sequence,
    _autonomous_decision_cycle_evaluation_projection,
    _autonomous_decision_cycle_public_status,
    _decision_cycle_evaluation_digest,
    _decision_cycle_learning_metadata,
    _decision_cycle_selection_digest,
    _decision_cycle_task_metadata,
    _decision_cycle_task_metadata_from_result,
    _identifier,
    content_digest,
)

class AutonomousAgentAutoMixin:
    def run_auto(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        route_override: AutonomousRouteProposal | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        hints: Sequence[str] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        semantic_routing: bool = False,
        semantic_weight: float = 0.65,
        semantic_bandit_state: Mapping[str, Any] | None = None,
        semantic_contextual_observations: Sequence[Mapping[str, Any]] = (),
        semantic_selection_overrides: Mapping[str, Any] | None = None,
        semantic_input_tokens: int = 4_096,
        semantic_requested_output_tokens: int = 1_024,
        semantic_max_cost_per_million_tokens: int | None = None,
        semantic_max_latency_ms: int | None = None,
        semantic_min_quality: float | None = None,
        semantic_run_id: str | None = None,
        semantic_max_output_tokens: int = 1_024,
        semantic_temperature: float | None = None,
        planning_mode: str = "deterministic",
        planning_run_id: str | None = None,
        planning_max_output_tokens: int = 1_024,
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_plan_accepted: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        learning_mode: str = "off",
        workflow_execution: bool = False,
        workflow_learning: bool = False,
        workflow_trajectory_learning: bool = False,
        workflow_stage_evidence: Mapping[str, Mapping[str, Any]] | None = None,
        workflow_trajectory_discount: float = 0.90,
        workflow_trajectory_terminal_reward: float | None = None,
        cross_domain_learning: bool = False,
        cross_domain_trajectory_learning: bool = False,
        cross_domain_replan_learning: bool = False,
        cross_domain_replan_max_replans: int = 1,
        cross_domain_evidence: Mapping[str, Mapping[str, Any]] | None = None,
        cross_domain_evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry | None = None,
        evaluator_bridge: AutonomousCycleEvaluatorBridge | None = None,
        cross_domain_trajectory_discount: float = 0.90,
        cross_domain_trajectory_terminal_reward: float | None = None,
        accepted_cross_domain_plan_refinement: AutonomousCrossDomainPlanRefinementResult | None = None,
        accepted_plan_refinement: AutonomousPlanRefinementResult | None = None,
        workflow_checkpoint: AutonomousWorkflowCheckpoint | Mapping[str, Any] | None = None,
        workflow_retry_blocked: bool = False,
        workflow_max_stage_calls: int | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        decision_cycle_id: str | None = None,
        decision_cycle_store: AutonomousDecisionCycleStateStore | None = None,
        resume_decision_cycle: bool = False,
        decision_cycle_rehydrate_result: Callable[[AutonomousDecisionCycleRehydrationContext], Any] | None = None,
        **kwargs: Any,
    ) -> AutonomousAutoResult:
        """Route and execute a task, returning review-required instead of guessing silently.

        A routed task uses the same provider, tool, approval, persistence, and learning paths as
        explicit ``run``/``run_cross_domain`` calls.  ``workflow_execution=True`` opts a
        single-domain route into its checkpointable stage DAG. An accepted plan refinement is
        advisory until the caller passes it explicitly here; it can only reorder ready stages.
        ``learning_mode="online"`` selects the appropriate existing online loop after routing:
        ordinary single-domain learning, staged workflow learning, or sequential cross-domain
        learning. ``learning_mode="trajectory"`` selects delayed discounted credit for a staged
        workflow or cross-domain route; a plain single provider call must opt into
        ``workflow_execution=True`` because it has no multi-step trajectory. Both modes reuse the
        caller's latest value-only bandit state unless ``bandit_state`` is supplied explicitly.
        ``cross_domain_replan_learning=True`` selects bounded evaluator-guided cross-domain
        attempts; it requires ``cross_domain_evaluator`` and settles one trajectory before each
        retry, with ``cross_domain_replan_max_replans`` capped at three.
        ``planning_mode="provider"`` adds one explicit provider planning call after routing. For
        a single-domain route it promotes execution to the checkpointable workflow path; for a
        cross-domain route it may reorder only the already-reviewed specialists. The provider
        proposal is executed only when it is dependency-closed, non-abstaining, and not marked
        for review. Provider approval applies to both planning and execution, and a planning
        refusal returns ``planning_review_required`` without dispatching the task.
        Evaluator evidence remains caller-owned and reward is never inferred from provider
        success. The explicit route-specific flags remain available for backwards compatibility.
        An accepted cross-domain plan can reorder existing specialists only after explicit caller
        acceptance.
        An abstained route never invokes a provider.
        """

        # Bind the persistent prompt learner before routing or provider-assisted planning so the
        # automatic path cannot silently use a different prompt state than direct execution.
        kwargs = self._prompt_learning_options(kwargs)
        if decision_cycle_store is None and decision_cycle_id is not None and self.decision_cycle_persistence is not None:
            decision_cycle_store = self.decision_cycle_persistence.store
        if (decision_cycle_id is None) != (decision_cycle_store is None):
            raise BrainRunError("decision_cycle_id and decision_cycle_store must be supplied together")
        if not isinstance(resume_decision_cycle, bool):
            raise BrainRunError("resume_decision_cycle must be a boolean")
        if decision_cycle_rehydrate_result is not None and not callable(decision_cycle_rehydrate_result):
            raise BrainRunError("decision_cycle_rehydrate_result must be callable or None")

        if not isinstance(workflow_execution, bool):
            raise BrainRunError("workflow_execution must be a boolean")
        if route_override is not None:
            if not isinstance(route_override, AutonomousRouteProposal):
                raise BrainRunError("route_override must be an AutonomousRouteProposal")
            if route_override.task_digest != content_digest({"task": task}):
                raise BrainRunError("route_override task does not match the automatic task")
            if semantic_routing:
                raise BrainRunError("route_override cannot be combined with semantic_routing")
        if planning_mode not in AUTONOMOUS_PLANNING_MODES:
            raise BrainRunError(
                "planning_mode must be one of: " + ", ".join(AUTONOMOUS_PLANNING_MODES)
            )
        if planning_run_id is not None:
            _identifier("planning_run_id", planning_run_id)
        if planning_mode == "provider" and (
            accepted_plan_refinement is not None or accepted_cross_domain_plan_refinement is not None
        ):
            raise BrainRunError(
                "provider planning cannot be combined with a caller-supplied accepted plan refinement"
            )
        if learning_mode not in AUTONOMOUS_LEARNING_MODES:
            raise BrainRunError(
                "learning_mode must be one of: " + ", ".join(AUTONOMOUS_LEARNING_MODES)
            )
        if not isinstance(workflow_learning, bool) or not isinstance(workflow_trajectory_learning, bool):
            raise BrainRunError("workflow learning modes must be booleans")
        if workflow_learning and workflow_trajectory_learning:
            raise BrainRunError("workflow_learning and workflow_trajectory_learning are mutually exclusive")
        if not isinstance(cross_domain_learning, bool) or not isinstance(cross_domain_trajectory_learning, bool) or not isinstance(cross_domain_replan_learning, bool):
            raise BrainRunError("cross-domain learning modes must be booleans")
        if sum(
            int(value)
            for value in (
                cross_domain_learning,
                cross_domain_trajectory_learning,
                cross_domain_replan_learning,
            )
        ) > 1:
            raise BrainRunError(
                "cross_domain_learning, cross_domain_trajectory_learning, and cross_domain_replan_learning are mutually exclusive"
            )
        if (
            not isinstance(cross_domain_replan_max_replans, int)
            or isinstance(cross_domain_replan_max_replans, bool)
            or not 0 <= cross_domain_replan_max_replans <= MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS
        ):
            raise BrainRunError(
                f"cross_domain_replan_max_replans must be within [0, {MAX_AUTONOMOUS_CROSS_DOMAIN_REPLANS}]"
            )
        explicit_learning = (
            workflow_learning
            or workflow_trajectory_learning
            or cross_domain_learning
            or cross_domain_trajectory_learning
            or cross_domain_replan_learning
            or kwargs.get("learn") is True
        )
        if learning_mode != "off" and explicit_learning:
            raise BrainRunError(
                "learning_mode cannot be combined with explicit learning flags; choose one control surface"
            )
        if cross_domain_evaluator is not None and not isinstance(
            cross_domain_evaluator,
            (BrainOutcomeEvaluator, DomainEvaluatorRegistry),
        ):
            raise BrainRunError(
                "cross_domain_evaluator must be a BrainOutcomeEvaluator, DomainEvaluatorRegistry, or None"
            )
        if evaluator_bridge is not None and not isinstance(
            evaluator_bridge,
            AutonomousCycleEvaluatorBridge,
        ):
            raise BrainRunError(
                "evaluator_bridge must be an AutonomousCycleEvaluatorBridge or None"
            )
        if evaluator_bridge is not None and not (
            explicit_learning or learning_mode != "off"
        ):
            raise BrainRunError(
                "evaluator_bridge requires an explicit automatic learning mode"
            )
        if evaluator_bridge is not None and any(
            value is not None
            for value in (
                kwargs.get("evaluator"),
                kwargs.get("evaluator_registry"),
                kwargs.get("evidence"),
                cross_domain_evidence,
                cross_domain_evaluator,
            )
        ):
            raise BrainRunError(
                "evaluator_bridge cannot be combined with evaluator, evaluator_registry, evidence, or cross-domain evaluator inputs"
            )
        if not isinstance(workflow_retry_blocked, bool):
            raise BrainRunError("workflow_retry_blocked must be a boolean")
        if planning_mode != "provider" and not workflow_execution and (
            workflow_learning
            or workflow_trajectory_learning
            or workflow_stage_evidence is not None
            or accepted_plan_refinement is not None
            or workflow_checkpoint is not None
            or workflow_retry_blocked
            or workflow_max_stage_calls is not None
        ):
            raise BrainRunError("workflow options require workflow_execution=True")
        if workflow_max_stage_calls is not None and (
            not isinstance(workflow_max_stage_calls, int)
            or isinstance(workflow_max_stage_calls, bool)
            or not 1 <= workflow_max_stage_calls <= 16
        ):
            raise BrainRunError("workflow_max_stage_calls must be between 1 and 16")

        if "domain" in kwargs:
            raise BrainRunError("run_auto chooses the domain; pass routing hints instead")
        cycle_learning = explicit_learning or learning_mode != "off"

        def finalize_rehydrated_cycle(
            cycle: AutonomousDecisionCycle,
            rehydrated: AutonomousAutoResult,
        ) -> None:
            """Seal a non-terminal cursor after a caller rehydrates a verified private result."""

            if cycle.state.terminal_status is not None:
                return
            route_digest = cycle.state.route_digest or rehydrated.route.route_digest
            plan_digest = cycle.state.plan_refinement_digest
            if rehydrated.planning is not None:
                plan_digest = content_digest(rehydrated.planning.to_dict())
            if rehydrated.status == "planning_review_required":
                cycle.advance(
                    phase="planning_pending",
                    route_digest=route_digest,
                    plan_refinement_digest=plan_digest,
                )
                return
            if rehydrated.status in {"policy_review_required", "policy_blocked"}:
                if cycle.state.route_digest is None:
                    cycle.advance(phase="route_pending", route_digest=route_digest)
                cycle.terminal(
                    rehydrated.status,
                    outcome_digest=content_digest(rehydrated.to_dict()),
                )
                return
            if rehydrated.status == "route_review_required":
                if cycle.state.route_digest is None:
                    cycle.advance(phase="route_pending", route_digest=route_digest)
                cycle.terminal(
                    "route_review_required",
                    outcome_digest=content_digest(rehydrated.to_dict()),
                )
                return

            outcome_digest = content_digest(rehydrated.to_dict())
            selection_digest = _decision_cycle_selection_digest(rehydrated)
            if selection_digest is None:
                selection_digest = cycle.state.selection_digest
            evaluation_digest = _decision_cycle_evaluation_digest(rehydrated.result)
            if evaluation_digest is None:
                evaluation_digest = cycle.state.evaluation_digest
            derived_episode_ids, derived_settlement_digests = _decision_cycle_learning_metadata(rehydrated.result)
            episode_ids = tuple(dict.fromkeys((*cycle.state.learning_episode_ids, *derived_episode_ids)))
            settlement_digests = tuple(dict.fromkeys((*cycle.state.settlement_digests, *derived_settlement_digests)))
            if evaluation_digest is None:
                episode_ids = ()
                settlement_digests = ()
            elif episode_ids and not settlement_digests:
                settlement_digests = (evaluation_digest,)
            cycle.advance(
                phase=("settlement_pending" if evaluation_digest is not None else "evaluation_pending")
                if cycle.evaluation_enabled
                else "execution_pending",
                route_digest=route_digest,
                plan_refinement_digest=plan_digest,
                selection_digest=selection_digest,
                outcome_digest=outcome_digest,
                learning_episode_ids=episode_ids,
                evaluation_digest=evaluation_digest,
                settlement_digests=settlement_digests,
            )
            if evaluation_digest is not None:
                cycle.terminal(
                    rehydrated.execution_status,
                    outcome_digest=outcome_digest,
                    settlement_digests=settlement_digests,
                )
            else:
                cycle.terminal(rehydrated.execution_status, outcome_digest=outcome_digest)

        def rehydrate_restored_cycle(cycle: AutonomousDecisionCycle) -> AutonomousAutoResult:
            if not resume_decision_cycle:
                raise BrainRunError("persisted decision cycle requires resume_decision_cycle=True")
            if decision_cycle_rehydrate_result is None:
                raise BrainRunError("decision-cycle resume requires decision_cycle_rehydrate_result")
            rehydrated = decision_cycle_rehydrate_result(cycle.context())
            if not isinstance(rehydrated, AutonomousAutoResult):
                raise BrainRunError("decision-cycle rehydrator must return an AutonomousAutoResult")
            if cycle.state.task_decision_digest is not None:
                observed_task_decision = _decision_cycle_task_metadata_from_result(rehydrated)
                expected_task_decision = {
                    "task_intent_digest": cycle.state.task_intent_digest,
                    "task_decision_digest": cycle.state.task_decision_digest,
                    "task_decision_posture": cycle.state.task_decision_posture,
                }
                if observed_task_decision != expected_task_decision:
                    raise BrainRunError("rehydrated decision result does not match the persisted task decision identity")
            if cycle.state.route_digest is not None and rehydrated.route.route_digest != cycle.state.route_digest:
                raise BrainRunError("rehydrated decision result does not match the persisted route digest")
            if cycle.state.plan_refinement_digest is not None:
                if rehydrated.planning is None or content_digest(rehydrated.planning.to_dict()) != cycle.state.plan_refinement_digest:
                    raise BrainRunError("rehydrated decision result does not match the persisted planning digest")
            if cycle.state.outcome_digest is not None:
                rehydrated_projection = rehydrated.to_dict()
                outcome_matches = content_digest(rehydrated_projection) == cycle.state.outcome_digest
                if not outcome_matches:
                    # Accept snapshots written before execution_status and semantic_route were
                    # added to the public automatic-result projection, while still rejecting
                    # every other result mutation.
                    legacy_projection = dict(rehydrated_projection)
                    legacy_projection.pop("execution_status", None)
                    outcome_matches = content_digest(legacy_projection) == cycle.state.outcome_digest
                    if not outcome_matches:
                        legacy_projection.pop("semantic_route", None)
                        outcome_matches = content_digest(legacy_projection) == cycle.state.outcome_digest
                if not outcome_matches:
                    raise BrainRunError("rehydrated decision result does not match the persisted outcome digest")
            if cycle.state.selection_digest is not None:
                observed_selection_digest = _decision_cycle_selection_digest(rehydrated)
                if observed_selection_digest != cycle.state.selection_digest:
                    raise BrainRunError("rehydrated decision result does not match the persisted selection digest")
            if cycle.state.evaluation_digest is not None:
                observed_evaluation_digest = _decision_cycle_evaluation_digest(rehydrated.result)
                if observed_evaluation_digest != cycle.state.evaluation_digest:
                    raise BrainRunError("rehydrated decision result does not match the persisted evaluation digest")
            if cycle.state.terminal_status is not None:
                observed_terminal_status = rehydrated.execution_status
                legacy_completed = cycle.state.terminal_status == "completed" and rehydrated.status == "completed"
                if not legacy_completed and observed_terminal_status != cycle.state.terminal_status:
                    raise BrainRunError("rehydrated decision result does not match the persisted terminal status")
            finalize_rehydrated_cycle(cycle, rehydrated)
            return rehydrated

        if resume_decision_cycle and decision_cycle_store is not None:
            persisted = decision_cycle_store.load(decision_cycle_id)  # type: ignore[arg-type]
            if persisted is not None:
                persisted_mode = persisted.get("mode") if isinstance(persisted, Mapping) else getattr(persisted, "mode", None)
                persisted_trajectory_id = persisted.get("trajectory_id") if isinstance(persisted, Mapping) else getattr(persisted, "trajectory_id", None)
                requested_trajectory_id = kwargs.get("trajectory_id")
                if requested_trajectory_id is not None and requested_trajectory_id != persisted_trajectory_id:
                    raise BrainRunError("resume decision-cycle trajectory identity does not match the persisted cycle")
                decision_cycle = AutonomousDecisionCycle(
                    decision_cycle_store,
                    cycle_id=decision_cycle_id,  # type: ignore[arg-type]
                    task=task,
                    mode=persisted_mode,
                    learning_enabled=cycle_learning,
                    evaluation_enabled=cycle_learning,
                    trajectory_id=persisted_trajectory_id,
                )
                if decision_cycle.restored:
                    return rehydrate_restored_cycle(decision_cycle)

        prepare_options = {
            key: value
            for key, value in kwargs.items()
            if key
            in {
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
            }
        }
        if route_override is not None:
            prepare_options["route_override"] = route_override
        if semantic_routing:
            blueprint = self.prepare_auto_with_provider(
                task=task,
                credentials=credentials,
                model_candidates=model_candidates,
                hints=hints,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
                semantic_weight=semantic_weight,
                bandit_state=self.learning_state() if semantic_bandit_state is None else semantic_bandit_state,
                contextual_observations=semantic_contextual_observations,
                selection_overrides=semantic_selection_overrides,
                input_tokens=semantic_input_tokens,
                requested_output_tokens=semantic_requested_output_tokens,
                max_cost_per_million_tokens=semantic_max_cost_per_million_tokens,
                max_latency_ms=semantic_max_latency_ms,
                min_quality=semantic_min_quality,
                approve_provider_call=bool(kwargs.get("approve_provider_call", False)),
                run_id=semantic_run_id,
                max_output_tokens=semantic_max_output_tokens,
                temperature=semantic_temperature,
                domain_policy_mode=domain_policy_mode,
                domain_policy_evidence_ready=domain_policy_evidence_ready,
                domain_policy_evaluator_configured=domain_policy_evaluator_configured,
                domain_policy_effects_requested=domain_policy_effects_requested,
                domain_policy_effects_approved=domain_policy_effects_approved,
                **prepare_options,
            )
        else:
            blueprint = self.prepare_auto(
                task=task,
                hints=hints,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
                **prepare_options,
            )
        task_decision_metadata = _decision_cycle_task_metadata(blueprint)
        decision_cycle: AutonomousDecisionCycle | None = None
        if decision_cycle_store is not None:
            cycle_mode = "single_domain" if len(blueprint.route.selected_domains) == 1 else "cross_domain"
            cycle_trajectory = kwargs.get("trajectory_id")
            if cycle_mode == "cross_domain" and cycle_learning and cycle_trajectory is None:
                cycle_trajectory = f"{decision_cycle_id}-trajectory"
            decision_cycle = AutonomousDecisionCycle(
                decision_cycle_store,
                cycle_id=decision_cycle_id,
                task=task,
                mode=cycle_mode,
                learning_enabled=cycle_learning,
                evaluation_enabled=cycle_learning,
                trajectory_id=cycle_trajectory,
            )
            if decision_cycle.restored:
                return rehydrate_restored_cycle(decision_cycle)
            decision_cycle.advance(
                phase="route_pending",
                route_digest=(
                    blueprint.semantic_route.route.route_digest
                    if (
                        domain_policy_mode == "strict"
                        and blueprint.semantic_route is not None
                        and blueprint.semantic_route.status
                        in {"policy_review_required", "policy_blocked", "approval_required"}
                    )
                    else blueprint.route.route_digest
                ),
                **task_decision_metadata,
            )

        if (
            domain_policy_mode == "strict"
            and blueprint.semantic_route is not None
            and blueprint.semantic_route.route.abstained is False
            and blueprint.semantic_route.status
            in {"policy_review_required", "policy_blocked", "approval_required"}
        ):
            result = AutonomousAutoResult(
                status=(
                    "policy_blocked"
                    if blueprint.semantic_route.status == "policy_blocked"
                    else "policy_review_required"
                ),
                route=blueprint.semantic_route.route,
                semantic_route=blueprint.semantic_route,
                learning_mode=learning_mode,
                planning_mode=planning_mode,
                **task_decision_metadata,
            )
            if decision_cycle is not None:
                decision_cycle.terminal(
                    result.status,
                    outcome_digest=content_digest(result.to_dict()),
                )
            return result

        if blueprint.route.abstained:
            result = AutonomousAutoResult(
                status="route_review_required",
                route=blueprint.route,
                semantic_route=blueprint.semantic_route,
                learning_mode=learning_mode,
                planning_mode=planning_mode,
                **task_decision_metadata,
            )
            if decision_cycle is not None:
                decision_cycle.terminal(
                    "route_review_required",
                    outcome_digest=content_digest(result.to_dict()),
                )
            return result

        planning_result: AutonomousPlanRefinementResult | AutonomousCrossDomainPlanRefinementResult | None = None
        if planning_mode == "provider":
            if decision_cycle is not None:
                decision_cycle.advance(phase="planning_pending")
            planning_candidates = self._resolve_candidates(
                model_candidates,
                allow_empty=domain_policy_mode == "strict",
            )
            planning_credentials = self._credential_mapping(credentials)
            planning_state = kwargs.get("bandit_state")
            if planning_state is None and learning_mode != "off":
                planning_state = self.learning_state()
            planning_common = {
                "model_candidates": planning_candidates,
                "credentials": planning_credentials,
                "context": kwargs.get("context"),
                "bandit_state": planning_state,
                "contextual_observations": kwargs.get("contextual_observations", ()),
                "selection_overrides": kwargs.get("selection_overrides"),
                "input_tokens": kwargs.get("input_tokens", 4_096),
                "requested_output_tokens": kwargs.get("requested_output_tokens", 1_024),
                "max_cost_per_million_tokens": kwargs.get("max_cost_per_million_tokens"),
                "max_latency_ms": kwargs.get("max_latency_ms"),
                "min_quality": kwargs.get("min_quality"),
                "approve_provider_call": kwargs.get("approve_provider_call", False),
                "run_id": planning_run_id,
                "max_output_tokens": planning_max_output_tokens,
                "temperature": kwargs.get("temperature"),
                "prompt_template": kwargs.get("planning_prompt_template", kwargs.get("prompt_template")),
                "prompt_registry": kwargs.get("planning_prompt_registry", kwargs.get("prompt_registry")),
                "prompt_selection": kwargs.get("planning_prompt_selection", kwargs.get("prompt_selection")),
                "prompt_stage": kwargs.get("planning_prompt_stage", "planning"),
                "prompt_learning_state": kwargs.get("planning_prompt_learning_state", kwargs.get("prompt_learning_state")),
                "prompt_learning_exploration": kwargs.get("planning_prompt_learning_exploration", kwargs.get("prompt_learning_exploration", 0.35)),
                "domain_policy_mode": domain_policy_mode,
                "domain_policy_evidence_ready": domain_policy_evidence_ready,
                "domain_policy_evaluator_configured": domain_policy_evaluator_configured,
                "domain_policy_effects_requested": domain_policy_effects_requested,
                "domain_policy_effects_approved": domain_policy_effects_approved,
            }
            if blueprint.blueprint is not None:
                planning_result = self.plan_with_provider(
                    blueprint=blueprint.blueprint,
                    **planning_common,
                )
            elif blueprint.cross_domain_blueprint is not None:
                planning_result = self.plan_cross_domain_with_provider(
                    blueprint=blueprint.cross_domain_blueprint,
                    **planning_common,
                )
            else:  # pragma: no cover - AutonomousAutoBlueprint invariants make this unreachable
                raise BrainRunError("provider planning requires an executable automatic blueprint")
            if planning_result.status != "completed" or planning_result.review_required:
                if decision_cycle is not None:
                    decision_cycle.advance(
                        phase="planning_pending",
                        plan_refinement_digest=content_digest(planning_result.to_dict()),
                    )
                return AutonomousAutoResult(
                    status="planning_review_required",
                    route=blueprint.route,
                    semantic_route=blueprint.semantic_route,
                    learning_mode=learning_mode,
                    planning_mode=planning_mode,
                    planning=planning_result,
                    **task_decision_metadata,
                )
            if decision_cycle is not None:
                decision_cycle.advance(
                    phase="planning_pending",
                    plan_refinement_digest=content_digest(planning_result.to_dict()),
                )
            if blueprint.blueprint is not None:
                if not isinstance(planning_result, AutonomousPlanRefinementResult):
                    raise BrainRunError("single-domain provider planning returned the wrong proposal type")
                accepted_plan_refinement = planning_result
                # A stage priority has no meaning on the one-shot provider path. Provider
                # planning therefore explicitly opts the automatic route into its reviewed DAG.
                workflow_execution = True
            else:
                if not isinstance(planning_result, AutonomousCrossDomainPlanRefinementResult):
                    raise BrainRunError("cross-domain provider planning returned the wrong proposal type")
                accepted_cross_domain_plan_refinement = planning_result

        if not workflow_execution and (
            workflow_learning
            or workflow_trajectory_learning
            or workflow_stage_evidence is not None
            or accepted_plan_refinement is not None
            or workflow_checkpoint is not None
            or workflow_retry_blocked
            or workflow_max_stage_calls is not None
        ):
            raise BrainRunError("workflow options require workflow_execution=True")
        if decision_cycle is not None:
            decision_cycle.advance(phase="execution_pending")
        execution_kwargs = dict(kwargs)
        execution_kwargs.update(
            {
                "domain_policy_mode": domain_policy_mode,
                "domain_policy_evidence_ready": domain_policy_evidence_ready,
                "domain_policy_evaluator_configured": domain_policy_evaluator_configured,
                "domain_policy_plan_accepted": (
                    domain_policy_plan_accepted
                    if domain_policy_plan_accepted is not None
                    else planning_result is not None and planning_result.status == "completed"
                ),
                "domain_policy_effects_requested": domain_policy_effects_requested,
                "domain_policy_effects_approved": domain_policy_effects_approved,
            }
        )
        for key in {
            "context",
            "constraints",
            "desired_outputs",
            "capability",
            "risk_class",
            "max_steps",
            "require_json",
            "response_schema",
            "execution_mode",
            "max_input_tokens",
            "required_model_capabilities",
            "memory_episodes",
            "planning_prompt_template",
            "planning_prompt_registry",
            "planning_prompt_selection",
            "planning_prompt_stage",
            "planning_prompt_learning_state",
            "planning_prompt_learning_exploration",
        }:
            execution_kwargs.pop(key, None)
        routed_context = self.orchestrator._route_context(kwargs.get("context"), blueprint.route)
        execution_kwargs["context"] = routed_context
        if evaluator_bridge is not None:
            if blueprint.blueprint is not None:
                execution_kwargs["evaluator"] = evaluator_bridge.evaluator_for_domain(
                    blueprint.route.selected_domains[0]
                )
            else:
                execution_kwargs["evaluator"] = evaluator_bridge.evaluator_for_cross_domain(
                    blueprint.route.selected_domains
                )
        if learning_mode != "off":
            # The ledger is caller-owned and stores value-only state. Supplying it here lets all
            # three execution branches begin from the same state without making the caller repeat
            # a persistence detail on every automatic request.
            execution_kwargs.setdefault("bandit_state", self.learning_state())
        if blueprint.blueprint is not None:
            if (
                cross_domain_learning
                or cross_domain_trajectory_learning
                or cross_domain_replan_learning
                or cross_domain_evidence is not None
                or cross_domain_evaluator is not None
                or accepted_cross_domain_plan_refinement is not None
            ):
                raise BrainRunError("cross-domain learning options require a cross-domain route")
            if learning_mode == "online" and workflow_execution:
                workflow_learning = True
            elif learning_mode == "trajectory" and workflow_execution:
                workflow_trajectory_learning = True
            if workflow_execution:
                if execution_kwargs.pop("learn", False):
                    if not workflow_learning and not workflow_trajectory_learning:
                        raise BrainRunError(
                            "workflow_execution does not accept learn=True; select workflow_learning or workflow_trajectory_learning"
                        )
                if "checkpoint" in execution_kwargs:
                    raise BrainRunError("workflow checkpoint must be supplied as workflow_checkpoint")
                if "retry_blocked" in execution_kwargs or "max_stage_calls" in execution_kwargs:
                    raise BrainRunError(
                        "workflow retry and call limits must be supplied as workflow_retry_blocked/workflow_max_stage_calls"
                    )
                workflow_options = dict(execution_kwargs)
                workflow_options.pop("context", None)
                # The reviewed blueprint already carries the response contract; workflow stage
                # execution uses that contract and must not forward this intake-only option to
                # the stage runner as an unknown execution kwarg.
                workflow_options.pop("structured_domain_response", None)
                workflow_options["accepted_plan_refinement"] = accepted_plan_refinement
                workflow_options["checkpoint"] = workflow_checkpoint
                workflow_options["retry_blocked"] = workflow_retry_blocked
                bandit_state = workflow_options.pop("bandit_state", None)
                if workflow_stage_evidence is not None:
                    workflow_options["stage_evidence"] = workflow_stage_evidence
                if workflow_max_stage_calls is not None:
                    workflow_options["max_stage_calls"] = workflow_max_stage_calls
                if workflow_learning:
                    result = self.run_workflow_learning(
                        blueprint=blueprint.blueprint,
                        credentials=credentials,
                        model_candidates=model_candidates,
                        bandit_state=bandit_state,
                        execution_id=execution_id,
                        resume_execution=resume_execution,
                        **workflow_options,
                    )
                elif workflow_trajectory_learning:
                    workflow_options["trajectory_discount"] = workflow_trajectory_discount
                    workflow_options["trajectory_terminal_reward"] = workflow_trajectory_terminal_reward
                    result = self.run_workflow_trajectory_learning(
                        blueprint=blueprint.blueprint,
                        credentials=credentials,
                        model_candidates=model_candidates,
                        bandit_state=bandit_state,
                        execution_id=execution_id,
                        resume_execution=resume_execution,
                        **workflow_options,
                    )
                else:
                    result = self.run_workflow(
                        blueprint=blueprint.blueprint,
                        credentials=credentials,
                        model_candidates=model_candidates,
                        execution_id=execution_id,
                        resume_execution=resume_execution,
                        **workflow_options,
                    )
            else:
                if learning_mode == "online":
                    execution_kwargs["learn"] = True
                elif learning_mode == "trajectory":
                    raise BrainRunError(
                        "learning_mode='trajectory' for a single-domain route requires workflow_execution=True"
                    )
                execution_kwargs.setdefault(
                    "execution_mode",
                    blueprint.blueprint.spec.execution_mode,
                )
                result = self.run(
                    task=task,
                    domain=blueprint.route.selected_domains[0],
                    credentials=credentials,
                    model_candidates=model_candidates,
                    execution_id=execution_id,
                    resume_execution=resume_execution,
                    **execution_kwargs,
                )
        else:
            if learning_mode == "online":
                cross_domain_learning = True
            elif learning_mode == "trajectory":
                cross_domain_trajectory_learning = True
            if workflow_execution or workflow_learning or workflow_trajectory_learning or accepted_plan_refinement is not None:
                raise BrainRunError(
                    "workflow_execution and accepted_plan_refinement currently require a single-domain route"
                )
            subtasks = [
                {
                    "id": f"route-{domain}",
                    "task": task,
                    "domain": domain,
                    "capability": kwargs.get("capability"),
                    "risk_class": kwargs.get("risk_class"),
                    "constraints": kwargs.get("constraints", ()),
                    "desired_outputs": kwargs.get("desired_outputs", ()),
                    "context": routed_context,
                    "max_steps": kwargs.get("max_steps", 8),
                    "require_json": kwargs.get("require_json", False),
                    "structured_domain_response": kwargs.get("structured_domain_response", False),
                    "response_schema": kwargs.get("response_schema"),
                    "execution_mode": kwargs.get("execution_mode", "provider"),
                    "required_model_capabilities": kwargs.get("required_model_capabilities", ()),
                }
                for domain in blueprint.route.selected_domains
            ]
            if execution_kwargs.pop("learn", False):
                raise BrainRunError(
                    "cross-domain intake does not accept learn=True; select an explicit cross-domain learning mode"
                )
            if cross_domain_evidence is not None and not (
                cross_domain_learning or cross_domain_trajectory_learning or cross_domain_replan_learning
            ):
                raise BrainRunError("cross_domain_evidence requires an explicit cross-domain learning mode")
            if cross_domain_evaluator is not None and not (
                cross_domain_learning or cross_domain_trajectory_learning or cross_domain_replan_learning
            ):
                raise BrainRunError("cross_domain_evaluator requires an explicit cross-domain learning mode")
            bandit_state = execution_kwargs.get("bandit_state")
            if cross_domain_learning or cross_domain_trajectory_learning or cross_domain_replan_learning:
                bandit_state = execution_kwargs.pop("bandit_state", None)
                if bandit_state is None:
                    raise BrainRunError(
                        "cross-domain learning requires caller-owned bandit_state"
                    )
            if (
                cross_domain_replan_learning
                and cross_domain_evaluator is None
                and evaluator_bridge is None
            ):
                raise BrainRunError("cross-domain replan learning requires cross_domain_evaluator")
            if cross_domain_evidence is not None:
                if "evidence" in execution_kwargs:
                    raise BrainRunError("cross_domain_evidence cannot be combined with evidence")
                execution_kwargs["evidence"] = cross_domain_evidence
            if cross_domain_evaluator is not None:
                if "evaluator" in execution_kwargs:
                    raise BrainRunError("cross_domain_evaluator cannot be combined with evaluator")
                execution_kwargs["evaluator"] = cross_domain_evaluator
            if accepted_cross_domain_plan_refinement is not None:
                if "accepted_plan_refinement" in execution_kwargs:
                    raise BrainRunError(
                        "accepted_cross_domain_plan_refinement cannot be combined with accepted_plan_refinement"
                    )
                execution_kwargs["accepted_plan_refinement"] = accepted_cross_domain_plan_refinement
            if cross_domain_learning:
                result = self.run_cross_domain_learning(
                    task=task,
                    subtasks=subtasks,
                    credentials=credentials,
                    model_candidates=model_candidates,
                    bandit_state=bandit_state,
                    execution_id=execution_id,
                    resume_execution=resume_execution,
                    **execution_kwargs,
                )
            elif cross_domain_trajectory_learning:
                execution_kwargs["trajectory_discount"] = cross_domain_trajectory_discount
                execution_kwargs["trajectory_terminal_reward"] = cross_domain_trajectory_terminal_reward
                result = self.run_cross_domain_trajectory_learning(
                    task=task,
                    subtasks=subtasks,
                    credentials=credentials,
                    model_candidates=model_candidates,
                    bandit_state=bandit_state,
                    execution_id=execution_id,
                    resume_execution=resume_execution,
                    **execution_kwargs,
                )
            elif cross_domain_replan_learning:
                execution_kwargs["max_replans"] = cross_domain_replan_max_replans
                execution_kwargs["trajectory_discount"] = cross_domain_trajectory_discount
                execution_kwargs["trajectory_terminal_reward"] = cross_domain_trajectory_terminal_reward
                result = self.run_cross_domain_replan_learning(
                    task=task,
                    subtasks=subtasks,
                    credentials=credentials,
                    model_candidates=model_candidates,
                    bandit_state=bandit_state,
                    execution_id=execution_id,
                    resume_execution=resume_execution,
                    **execution_kwargs,
                )
            else:
                result = self.run_cross_domain(
                    task=task,
                    subtasks=subtasks,
                    credentials=credentials,
                    model_candidates=model_candidates,
                    execution_id=execution_id,
                    resume_execution=resume_execution,
                    **execution_kwargs,
                )
        automatic_result = AutonomousAutoResult(
            status="completed",
            route=blueprint.route,
            semantic_route=blueprint.semantic_route,
            result=result,
            learning_mode=learning_mode,
            planning_mode=planning_mode,
            planning=planning_result,
            **task_decision_metadata,
        )
        if decision_cycle is not None:
            cycle_outcome_digest = content_digest(automatic_result.to_dict())
            selection_digest = _decision_cycle_selection_digest(automatic_result)
            evaluation_digest = _decision_cycle_evaluation_digest(result)
            learning_episode_ids, settlement_digests = _decision_cycle_learning_metadata(result)
            if evaluation_digest is None:
                learning_episode_ids = ()
                settlement_digests = ()
            elif learning_episode_ids and not settlement_digests:
                # Python's in-process learner may settle through the bandit ledger without a
                # memory adapter. The evaluator projection is still a stable settlement boundary.
                settlement_digests = (evaluation_digest,)
            decision_cycle.advance(
                phase="evaluation_pending" if cycle_learning else "execution_pending",
                **task_decision_metadata,
                selection_digest=selection_digest,
                outcome_digest=cycle_outcome_digest,
                learning_episode_ids=learning_episode_ids,
            )
            if evaluation_digest is not None:
                decision_cycle.advance(
                    phase="settlement_pending",
                    selection_digest=selection_digest,
                    outcome_digest=cycle_outcome_digest,
                    evaluation_digest=evaluation_digest,
                    learning_episode_ids=learning_episode_ids,
                    settlement_digests=settlement_digests,
                )
            decision_cycle.terminal(
                automatic_result.execution_status,
                outcome_digest=cycle_outcome_digest,
                settlement_digests=settlement_digests,
            )
        return automatic_result

    def run_auto_cycle(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        domain: str | None = None,
        route_override: AutonomousRouteProposal | None = None,
        hints: Sequence[str] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        semantic_routing: bool = False,
        semantic_weight: float = 0.65,
        semantic_bandit_state: Mapping[str, Any] | None = None,
        semantic_contextual_observations: Sequence[Mapping[str, Any]] = (),
        semantic_selection_overrides: Mapping[str, Any] | None = None,
        semantic_selection_weights: Mapping[str, Any] | None = None,
        semantic_selection_observations: Sequence[Mapping[str, Any]] | None = None,
        semantic_input_tokens: int = 4_096,
        semantic_requested_output_tokens: int = 1_024,
        semantic_max_cost_per_million_tokens: int | None = None,
        semantic_max_latency_ms: int | None = None,
        semantic_min_quality: float | None = None,
        semantic_run_id: str | None = None,
        semantic_max_output_tokens: int = 1_024,
        semantic_temperature: float | None = None,
        learning_mode: str = "off",
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry | None = None,
        evaluator_bridge: AutonomousCycleEvaluatorBridge | None = None,
        decision_cycle_id: str | None = None,
        decision_cycle_store: AutonomousDecisionCycleStateStore | None = None,
        resume_decision_cycle: bool = False,
        decision_cycle_rehydrate_result: Callable[[AutonomousDecisionCycleRehydrationContext], Any] | None = None,
        retry_semantic_routing_on_restart: bool = False,
        **kwargs: Any,
    ) -> AutonomousAutoDecisionCycleResult:
        """Resolve one route and enter the matching autonomous decision-cycle kernel.

        This is the Python application-facing counterpart to the TypeScript automatic cycle.
        Routing occurs exactly once: deterministic routing is provider-free, and semantic routing
        is an explicitly approved classifier boundary.  The reviewed route is then passed as an
        exact override to :meth:`run_auto`, which retains the existing planning, model selection,
        prompt assembly, provider approval, tool/effect, evaluator, learning, and persistence
        contracts.  A cross-domain route therefore enters the existing fan-out/fan-in learning
        kernel automatically instead of making callers guess which execution API to choose.

        ``evaluator`` is optional.  When supplied, the cycle opts into online learning unless the
        caller chooses ``learning_mode="trajectory"``; its value-only result is projected into
        ``evaluation`` and no provider response is promoted to reward.  An
        :class:`AutonomousCycleEvaluatorBridge` may be used instead when evaluator selection is
        itself caller-owned metadata.  Restart callbacks rehydrate private results at the
        persisted boundary and must return an :class:`AutonomousAutoResult`; the public cycle
        envelope never stores that private value.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("automatic decision-cycle task must be non-empty text")
        if not isinstance(semantic_routing, bool):
            raise BrainRunError("semantic_routing must be a boolean")
        if not isinstance(retry_semantic_routing_on_restart, bool):
            raise BrainRunError("retry_semantic_routing_on_restart must be a boolean")
        if learning_mode not in AUTONOMOUS_LEARNING_MODES:
            raise BrainRunError(
                "learning_mode must be one of: " + ", ".join(AUTONOMOUS_LEARNING_MODES)
            )
        if evaluator is not None and not isinstance(evaluator, (BrainOutcomeEvaluator, DomainEvaluatorRegistry)):
            raise BrainRunError(
                "evaluator must be a BrainOutcomeEvaluator, DomainEvaluatorRegistry, or None"
            )
        if evaluator_bridge is not None and not isinstance(evaluator_bridge, AutonomousCycleEvaluatorBridge):
            raise BrainRunError("evaluator_bridge must be an AutonomousCycleEvaluatorBridge or None")
        if evaluator is not None and evaluator_bridge is not None:
            raise BrainRunError("evaluator and evaluator_bridge are mutually exclusive")
        if decision_cycle_store is None and decision_cycle_id is not None and self.decision_cycle_persistence is not None:
            decision_cycle_store = self.decision_cycle_persistence.store
        if (decision_cycle_id is None) != (decision_cycle_store is None):
            raise BrainRunError("decision_cycle_id and decision_cycle_store must be supplied together")
        if resume_decision_cycle and (decision_cycle_id is None or decision_cycle_store is None):
            raise BrainRunError("resume_decision_cycle requires decision_cycle_id and decision_cycle_store")
        if route_override is not None and semantic_routing:
            raise BrainRunError("route_override cannot be combined with semantic_routing")
        if route_override is not None and domain is not None:
            raise BrainRunError("route_override cannot be combined with domain")
        if route_override is not None and route_override.task_digest != content_digest({"task": task}):
            raise BrainRunError("route_override task does not match the automatic task")

        effective_learning_mode = learning_mode
        if (evaluator is not None or evaluator_bridge is not None) and effective_learning_mode == "off":
            effective_learning_mode = "online"
        if effective_learning_mode != "off" and evaluator is None and evaluator_bridge is None:
            raise BrainRunError(
                "automatic decision-cycle learning requires evaluator or evaluator_bridge"
            )

        # A persisted semantic route is a private provider boundary.  Do not silently issue a
        # second classifier request on restart; callers must supply the reviewed route, opt into a
        # deliberate retry, or use the lower-level rehydration API.
        if (
            resume_decision_cycle
            and semantic_routing
            and route_override is None
            and not retry_semantic_routing_on_restart
            and decision_cycle_store is not None
            and decision_cycle_store.load(decision_cycle_id) is not None  # type: ignore[arg-type]
        ):
            raise BrainRunError(
                "restart resume of provider-assisted semantic routing requires route_override or "
                "retry_semantic_routing_on_restart=True"
            )

        def route_mode(value: AutonomousRouteProposal) -> str | None:
            if value.abstained:
                return None
            return "cross_domain" if value.cross_domain and len(value.selected_domains) > 1 else "single_domain"

        def explicit_domain_route(value: str) -> AutonomousRouteProposal:
            if value not in AUTONOMOUS_DOMAINS:
                raise BrainRunError(
                    "domain must be one of: " + ", ".join(AUTONOMOUS_DOMAINS)
                )
            routed = self.route(
                task=task,
                hints=(value,),
                min_confidence=0.0,
                min_margin=0.0,
                max_domains=1,
                allow_cross_domain=False,
            )
            candidate = next((item for item in routed.candidates if item.domain == value), None)
            if candidate is None:
                profile = self.orchestrator.registry.resolve(value)
                workflow = self.orchestrator.workflow_registry.resolve(value)
                candidate = AutonomousRouteCandidate(
                    domain=value,
                    score=1.0,
                    matched_terms=("explicit_domain",),
                    capability=profile.default_capability,
                    risk_class=profile.risk_class,
                    workflow_id=workflow.workflow_id,
                )
            return AutonomousRouteProposal(
                task_digest=routed.task_digest,
                candidates=(candidate,),
                selected_domains=(value,),
                confidence=max(1.0, candidate.score),
                abstained=False,
                reason="routed",
                cross_domain=False,
                source=routed.source,
            )

        semantic_route: AutonomousSemanticRouteResult | None = None
        if route_override is not None:
            route = route_override
        elif semantic_routing:
            domain_policy_mode = kwargs.get("domain_policy_mode", "audit")
            semantic_route = self.route_with_provider(
                task=task,
                credentials=credentials,
                model_candidates=model_candidates,
                hints=hints,
                context=kwargs.get("context"),
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
                semantic_weight=semantic_weight,
                bandit_state=semantic_bandit_state,
                contextual_observations=semantic_contextual_observations,
                selection_overrides=semantic_selection_overrides,
                selection_weights=semantic_selection_weights,
                selection_observations=semantic_selection_observations,
                input_tokens=semantic_input_tokens,
                requested_output_tokens=semantic_requested_output_tokens,
                max_cost_per_million_tokens=semantic_max_cost_per_million_tokens,
                max_latency_ms=semantic_max_latency_ms,
                min_quality=semantic_min_quality,
                approve_provider_call=bool(kwargs.get("approve_provider_call", False)),
                run_id=semantic_run_id,
                max_output_tokens=semantic_max_output_tokens,
                temperature=semantic_temperature,
                domain_policy_mode=domain_policy_mode,
                domain_policy_evidence_ready=kwargs.get("domain_policy_evidence_ready"),
                domain_policy_evaluator_configured=kwargs.get("domain_policy_evaluator_configured"),
                domain_policy_effects_requested=kwargs.get("domain_policy_effects_requested"),
                domain_policy_effects_approved=kwargs.get("domain_policy_effects_approved"),
            )
            route = semantic_route.route
            if semantic_route.status != "completed":
                return AutonomousAutoDecisionCycleResult(
                    status=semantic_route.status,
                    mode=route_mode(route),
                    route=route,
                    semantic_route=semantic_route,
                )
        elif domain is not None:
            route = explicit_domain_route(domain)
        else:
            route = self.route(
                task=task,
                hints=hints,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
            )

        call_options = dict(kwargs)
        call_options.update(
            {
                "route_override": route,
                "semantic_routing": False,
                "learning_mode": effective_learning_mode,
                "decision_cycle_id": decision_cycle_id,
                "decision_cycle_store": decision_cycle_store,
                "resume_decision_cycle": resume_decision_cycle,
                "decision_cycle_rehydrate_result": decision_cycle_rehydrate_result,
            }
        )
        if evaluator_bridge is not None:
            call_options["evaluator_bridge"] = evaluator_bridge
        if evaluator is not None:
            if route_mode(route) == "cross_domain":
                call_options["cross_domain_evaluator"] = evaluator
            elif isinstance(evaluator, DomainEvaluatorRegistry):
                call_options["evaluator_registry"] = evaluator
            else:
                call_options["evaluator"] = evaluator

        final = self.run_auto(
            task=task,
            credentials=credentials,
            model_candidates=model_candidates,
            **call_options,
        )
        status = _autonomous_decision_cycle_public_status(
            final.execution_status if final.status == "completed" else final.status
        )
        episode_ids: tuple[str, ...] = ()
        settlement_digests: tuple[str, ...] = ()
        if final.result is not None:
            episode_ids, settlement_digests = _decision_cycle_learning_metadata(final.result)
        inner = AutonomousDecisionCycleResult(
            status=status,
            route=route,
            semantic_route=semantic_route,
            run=final.result if final.status == "completed" else None,
            plan_refinement=final.planning,
            learning_episode_id=episode_ids[0] if episode_ids else None,
            evaluation=(
                _autonomous_decision_cycle_evaluation_projection(final.result)
                if final.result is not None
                else None
            ),
            settlement=(
                {
                    "settlement_digests": list(settlement_digests),
                    "retention": "value_only_settlement_identity;provider_result_caller_owned",
                }
                if settlement_digests
                else None
            ),
        )
        return AutonomousAutoDecisionCycleResult(
            status=status,
            mode=route_mode(route),
            route=route,
            semantic_route=semantic_route,
            cycle=inner,
            private_result=final,
        )

    def run_auto_cycle_with_launch_admission(
        self,
        *,
        task: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> AutonomousAutoDecisionCycleResult:
        """Run one automatic decision cycle only after a provider-free launch review.

        The wrapper compiles the exact route that the cycle will use, checks every selected
        domain against the caller-owned launch admission, and then passes that route back as a
        digest-verified override.  This preserves the cycle's single-route invariant while
        ensuring that launch admission is checked before credential normalization, provider
        invocation, evaluator settlement, tool execution, or effects.  Provider-assisted
        semantic routing remains a separate boundary and is rejected here until its classifier
        call has its own explicit admission.

        ``kwargs`` accepts the ordinary :meth:`run_auto_cycle` controls, including an explicit
        ``domain`` and route/learning/persistence settings.  The wrapper owns ``route_override``
        and rejects it so a caller cannot review one route and execute another.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("launch-admitted automatic decision-cycle task must be non-empty text")
        if "route_override" in kwargs:
            raise BrainRunError(
                "run_auto_cycle_with_launch_admission owns the route override; pass routing controls instead"
            )
        semantic_routing = kwargs.pop("semantic_routing", False)
        if not isinstance(semantic_routing, bool):
            raise BrainRunError("semantic_routing must be a boolean")
        if semantic_routing:
            raise BrainRunError(
                "launch-admitted automatic decision-cycle execution requires provider-free routing; "
                "admit semantic routing separately before enabling it"
            )

        domain = kwargs.pop("domain", None)
        route_options = self._automatic_route_options(kwargs)
        if domain is not None:
            if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAINS:
                raise BrainRunError(
                    "domain must be one of: " + ", ".join(AUTONOMOUS_DOMAINS)
                )
            # Match run_auto_cycle's explicit-domain route construction exactly.  The route is
            # intentionally made from the domain alone so unrelated lexical hints cannot cause
            # the admission preview and execution route to diverge.
            route_options.update(
                {
                    "hints": (domain,),
                    "min_confidence": 0.0,
                    "min_margin": 0.0,
                    "max_domains": 1,
                    "allow_cross_domain": False,
                }
            )

        blueprint = self.prepare_auto(task=task, **route_options)
        route = blueprint.route
        cycle_options = dict(kwargs)
        cycle_options["route_override"] = route
        cycle_options["semantic_routing"] = False
        if route.abstained:
            # No domain was selected, so there is no launch scope to authorize.  Returning the
            # normal review result keeps route abstention value-free and provider-free.
            return self.run_auto_cycle(
                task=task,
                credentials=credentials,
                model_candidates=model_candidates,
                **cycle_options,
            )

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, route.selected_domains)
        return self.run_auto_cycle(
            task=task,
            credentials=credentials,
            model_candidates=model_candidates,
            **cycle_options,
        )

    def run_auto_replan_cycle(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        max_replans: int = 1,
        route_override: AutonomousRouteProposal | None = None,
        hints: Sequence[str] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        semantic_routing: bool = False,
        decision_cycle_id: str | None = None,
        decision_cycle_store: AutonomousDecisionCycleStateStore | None = None,
        **kwargs: Any,
    ) -> AutonomousAutoReplanResult:
        """Run one route-frozen evaluator/learning/replan cycle.

        The route is resolved once before execution (or once by the provider-assisted semantic
        intake path), then every bounded retry reuses that route.  A caller-owned
        :class:`BrainOutcomeEvaluator` sees only the existing value-only evaluation projection;
        its reward updates the next bandit state through the ordinary online learner.  Replan
        feedback is screened and added as transient context by the existing brain kernel.  The
        method therefore makes the complete automatic loop easy to use without weakening the
        lower-level provider, credential, tool, effect, evidence, or evaluator boundaries.

        ``decision_cycle_store`` and ``decision_cycle_id`` opt into the existing hash-chained
        metadata checkpoint.  Private route/run/evaluator values remain caller-owned and must be
        supplied through the normal ``resume_decision_cycle`` rehydration callback when a process
        restarts after a provider boundary.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("automatic replan task must be non-empty text")
        if not isinstance(evaluator, (BrainOutcomeEvaluator, DomainEvaluatorRegistry)):
            raise BrainRunError("automatic replan evaluator must be a BrainOutcomeEvaluator or DomainEvaluatorRegistry")
        if isinstance(max_replans, bool) or not isinstance(max_replans, int) or not 0 <= max_replans <= MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS:
            raise BrainRunError(
                f"automatic replan max_replans must be within [0, {MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS}]"
            )
        if decision_cycle_store is None and decision_cycle_id is not None and self.decision_cycle_persistence is not None:
            decision_cycle_store = self.decision_cycle_persistence.store
        if (decision_cycle_id is None) != (decision_cycle_store is None):
            raise BrainRunError("decision_cycle_id and decision_cycle_store must be supplied together")
        if route_override is not None and not isinstance(route_override, AutonomousRouteProposal):
            raise BrainRunError("route_override must be an AutonomousRouteProposal")
        if route_override is not None and semantic_routing:
            raise BrainRunError("route_override cannot be combined with semantic_routing")

        semantic_route: AutonomousSemanticRouteResult | None = None
        if route_override is None and not semantic_routing:
            route = self.route(
                task=task,
                hints=hints,
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
            )
            if route.abstained:
                final = AutonomousAutoResult(status="route_review_required", route=route)
                return AutonomousAutoReplanResult(
                    status="route_review_required",
                    mode=None,
                    route=route,
                    final=final,
                )
        elif route_override is None:
            semantic_route = self.route_with_provider(
                task=task,
                credentials=credentials,
                model_candidates=model_candidates,
                hints=hints,
                context=kwargs.get("context"),
                min_confidence=min_confidence,
                min_margin=min_margin,
                max_domains=max_domains,
                allow_cross_domain=allow_cross_domain,
                semantic_weight=kwargs.pop("semantic_weight", 0.65),
                bandit_state=kwargs.pop("semantic_bandit_state", None) or self.learning_state(),
                contextual_observations=kwargs.pop("semantic_contextual_observations", ()),
                selection_overrides=kwargs.pop("semantic_selection_overrides", None),
                input_tokens=kwargs.pop("semantic_input_tokens", 4_096),
                requested_output_tokens=kwargs.pop("semantic_requested_output_tokens", 1_024),
                max_cost_per_million_tokens=kwargs.pop("semantic_max_cost_per_million_tokens", None),
                max_latency_ms=kwargs.pop("semantic_max_latency_ms", None),
                min_quality=kwargs.pop("semantic_min_quality", None),
                approve_provider_call=kwargs.get("approve_provider_call", False),
                run_id=kwargs.pop("semantic_run_id", None),
                max_output_tokens=kwargs.pop("semantic_max_output_tokens", 1_024),
                temperature=kwargs.pop("semantic_temperature", None),
                domain_policy_mode=kwargs.get("domain_policy_mode", "audit"),
                domain_policy_evidence_ready=kwargs.get("domain_policy_evidence_ready"),
                domain_policy_evaluator_configured=kwargs.get("domain_policy_evaluator_configured"),
                domain_policy_effects_requested=kwargs.get("domain_policy_effects_requested"),
                domain_policy_effects_approved=kwargs.get("domain_policy_effects_approved"),
            )
            route = semantic_route.route
            if semantic_route.status != "completed":
                return AutonomousAutoReplanResult(
                    status=semantic_route.status,
                    mode=(
                        "cross_domain"
                        if route.cross_domain and len(route.selected_domains) > 1
                        else "single_domain"
                    ) if not route.abstained else None,
                    route=route,
                    semantic_route=semantic_route,
                )
            # The provider classifier has already been approved and reconciled. Hand its exact
            # route to the ordinary execution path so evaluator retries cannot classify again.
            semantic_routing = False
        else:
            route = route_override

        call_options = dict(kwargs)
        if "evaluator" in call_options or "cross_domain_evaluator" in call_options:
            raise BrainRunError("run_auto_replan_cycle reserves evaluator and cross_domain_evaluator")
        call_options.update(
            {
                "hints": hints,
                "min_confidence": min_confidence,
                "min_margin": min_margin,
                "max_domains": max_domains,
                "allow_cross_domain": allow_cross_domain,
                "semantic_routing": False,
                "route_override": route,
                "decision_cycle_id": decision_cycle_id,
                "decision_cycle_store": decision_cycle_store,
                "approve_provider_call": call_options.get("approve_provider_call", False),
            }
        )
        if call_options.get("bandit_state") is None:
            call_options["bandit_state"] = self.learning_state()

        if route is not None and route.cross_domain and len(route.selected_domains) > 1:
            call_options.update(
                {
                    "cross_domain_replan_learning": True,
                    "cross_domain_replan_max_replans": max_replans,
                    "cross_domain_evaluator": evaluator,
                    "learning_mode": "off",
                }
            )
        else:
            call_options.update(
                {
                    "learning_mode": "online",
                    "evaluator": evaluator if isinstance(evaluator, BrainOutcomeEvaluator) else None,
                    "evaluator_registry": evaluator if isinstance(evaluator, DomainEvaluatorRegistry) else None,
                    "max_replans": max_replans,
                }
            )
            if call_options["evaluator"] is None:
                call_options.pop("evaluator")
            if call_options["evaluator_registry"] is None:
                call_options.pop("evaluator_registry")

        final = self.run_auto(
            task=task,
            credentials=credentials,
            model_candidates=model_candidates,
            **call_options,
        )
        if route is None:
            route = final.route
        inner = final.result
        if final.status != "completed":
            status = final.status
            attempts: tuple[Any, ...] = ()
            evaluations: tuple[Mapping[str, Any], ...] = ()
            replan_count = 0
        elif isinstance(inner, (AutonomousLearningResult, AutonomousCrossDomainReplanResult)):
            status = inner.status
            attempts = tuple(getattr(inner, "attempts", ()))
            if isinstance(inner, AutonomousCrossDomainReplanResult):
                evaluations = tuple(
                    evaluation
                    for attempt in attempts
                    for evaluation in getattr(attempt, "evaluations", ())
                )
            else:
                evaluations = tuple(getattr(inner, "evaluations", ()))
            replan_count = int(getattr(inner, "replan_count", max(0, len(attempts) - 1)))
        else:
            status = final.execution_status
            attempts = (inner,) if inner is not None else ()
            evaluations = ()
            replan_count = 0

        return AutonomousAutoReplanResult(
            status=status,
            mode=(
                "cross_domain"
                if final.route.cross_domain and len(final.route.selected_domains) > 1
                else "single_domain"
            ) if not final.route.abstained else None,
            route=final.route,
            final=final,
            attempt_results=attempts,
            evaluations=evaluations,
            replan_count=replan_count,
            semantic_route=semantic_route if semantic_route is not None else final.semantic_route,
        )

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentAutoMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
