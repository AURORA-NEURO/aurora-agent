"""Focused autonomous-orchestrator workflow methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_EXECUTION_MODES,
    AUTONOMOUS_WORKFLOW_EVALUATOR_SCHEMA,
    AUTONOMOUS_WORKFLOW_STAGE_STATUSES,
    Any,
    AutonomousAuthorizationContext,
    AutonomousCostReservationCallback,
    AutonomousCrossDomainBlueprint,
    AutonomousCrossDomainPlanRefinementResult,
    AutonomousCrossDomainResult,
    AutonomousCrossDomainStepResult,
    AutonomousDomainToolRuntime,
    AutonomousExecutionController,
    AutonomousPlanRefinementResult,
    AutonomousPromptLearningState,
    AutonomousPromptRegistry,
    AutonomousPromptSelectionPlan,
    AutonomousPromptTemplate,
    AutonomousTaskBlueprint,
    AutonomousWorkflowCheckpoint,
    AutonomousWorkflowEvaluator,
    AutonomousWorkflowLearningResult,
    AutonomousWorkflowRun,
    AutonomousWorkflowStage,
    AutonomousWorkflowStageEvaluation,
    AutonomousWorkflowStageResult,
    AutonomousWorkflowTrajectoryLearningResult,
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
    MAX_AUTONOMOUS_EXECUTION_PLAN_BYTES,
    MAX_AUTONOMOUS_WORKFLOW_STAGE_EVIDENCE,
    MAX_AUTONOMY_CONTEXT_BYTES,
    MAX_AUTONOMY_TEXT_BYTES,
    Mapping,
    MemoryQuery,
    MissionPolicy,
    ProviderContentPart,
    ProviderError,
    ProviderInvocationObserver,
    ProviderTool,
    Sequence,
    _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
    _AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY,
    _assert_task_decision_allows_provider,
    _autonomous_result_digest,
    _cross_domain_plan_digest,
    _emit_trace_event,
    _identifier,
    _mapping_sequence,
    _memory_selection_context,
    _memory_task_decision_digest,
    _memory_task_intent_digest,
    _memory_task_lens_digest,
    _record_workflow_stage_response_feedback,
    _safe_json,
    _sequence,
    _text,
    _workflow_digest,
    assess_autonomous_cross_domain_response_set,
    compile_autonomous_workflow_stage_execution_plan,
    content_digest,
    evaluate_autonomous_workflow_stage_response,
    math,
    uuid,
)

class AutonomousOrchestratorWorkflowMixin:
    @staticmethod
    def _workflow_provider_response(
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    ) -> Any:
        if isinstance(result, BrainRunResult):
            return result.response
        if isinstance(result, BrainToolLoopResult):
            if result.provider_loop is not None and result.provider_loop.final_response is not None:
                return result.provider_loop.final_response
            return result.brain_run.response
        return result.brain_run.response

    @classmethod
    def _workflow_structured_output(
        cls,
        result: BrainRunResult | BrainToolLoopResult | BrainMissionResult,
    ) -> Mapping[str, Any] | None:
        response = cls._workflow_provider_response(result)
        structured = getattr(response, "structured", None)
        return dict(structured) if isinstance(structured, Mapping) else None

    @staticmethod
    def _workflow_execution_status(result: BrainRunResult | BrainToolLoopResult | BrainMissionResult) -> str:
        if result.status == "approval_required":
            return "approval_required"
        if not result.status.startswith("completed"):
            return "provider_failed"
        return "completed"

    @staticmethod
    def _workflow_stage_route_request(
        route_request: Mapping[str, Any] | None,
        *,
        task: str,
        stage: AutonomousWorkflowStage,
    ) -> dict[str, Any] | None:
        if route_request is None:
            return None
        if not isinstance(route_request, Mapping):
            raise BrainRunError("workflow route_request must be a mapping or None")
        route = dict(route_request)
        route["goal"] = task
        raw_needs = route.get("needs")
        if raw_needs is None:
            route["needs"] = [
                {
                    "id": f"stage-{stage.id}",
                    "query": stage.objective,
                    "max_items": 128,
                }
            ]
        elif not isinstance(raw_needs, Sequence) or isinstance(raw_needs, (str, bytes)):
            raise BrainRunError("workflow route_request.needs must be a sequence")
        return route

    @staticmethod
    def _validate_workflow_stage_output(
        stage: AutonomousWorkflowStage,
        structured: Mapping[str, Any] | None,
    ) -> tuple[str | None, tuple[str, ...], tuple[str, ...], tuple[str, ...]]:
        """Return declared status, evidence, uncertainty, and semantic validation errors."""

        if structured is None:
            return None, (), (), ("provider returned no structured stage output",)
        errors: list[str] = []
        if structured.get("stage_id") != stage.id:
            errors.append("provider stage_id does not match the scheduled stage")
        declared = structured.get("status")
        if declared not in AUTONOMOUS_WORKFLOW_STAGE_STATUSES:
            errors.append("provider returned an invalid stage status")
            declared = None
        def value_list(name: str) -> tuple[str, ...]:
            value = structured.get(name, [])
            if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
                errors.append(f"provider stage {name} must be a string list")
                return ()
            try:
                return _sequence(f"provider stage {name}", value, maximum=MAX_AUTONOMOUS_WORKFLOW_STAGE_EVIDENCE)
            except BrainRunError:
                errors.append(f"provider stage {name} is malformed or exceeds its bound")
                return ()
        evidence = value_list("evidence")
        uncertainty = value_list("uncertainty")
        value_list("next_actions")
        notes = structured.get("notes", "")
        if not isinstance(notes, str) or len(notes.encode("utf-8")) > MAX_AUTONOMY_TEXT_BYTES:
            errors.append("provider stage notes are malformed or exceed their bound")
        # Evidence presence is evaluated by the digest-bound stage response evaluator below.
        # Keeping it out of the syntactic validator means a missing-evidence completion is
        # recorded as a quality-gated block with replayable evaluator feedback, rather than
        # disappearing as an unscored provider parse failure.
        if declared == "completed" and not stage.evidence_outputs:
            errors.append("workflow stage has no declared evidence outputs")
        return declared, evidence, uncertainty, tuple(errors)

    def _workflow_checkpoint(
        self,
        *,
        run_id: str,
        blueprint: AutonomousTaskBlueprint,
        snapshots: Sequence[Mapping[str, Any]],
        plan_refinement_digest: str | None = None,
    ) -> AutonomousWorkflowCheckpoint:
        return AutonomousWorkflowCheckpoint(
            run_id=run_id,
            task_digest=blueprint.spec.task_digest,
            workflow_id=blueprint.workflow.workflow_id,
            workflow_digest=blueprint.workflow.workflow_digest,
            stages=tuple(dict(snapshot) for snapshot in snapshots),
            plan_refinement_digest=plan_refinement_digest,
        )

    @staticmethod
    def _accepted_workflow_plan(
        blueprint: AutonomousTaskBlueprint,
        refinement: AutonomousPlanRefinementResult | None,
    ) -> tuple[dict[str, int], str | None, tuple[str, ...]]:
        """Validate an explicitly accepted planner proposal before it affects scheduling."""

        if refinement is None:
            return {}, None, ()
        if not isinstance(refinement, AutonomousPlanRefinementResult):
            raise BrainRunError("accepted_plan_refinement must be an AutonomousPlanRefinementResult or None")
        if refinement.status != "completed" or refinement.review_required:
            raise BrainRunError("only a completed, non-review plan refinement may be accepted")
        if refinement.task_digest != blueprint.spec.task_digest:
            raise BrainRunError("accepted plan refinement task does not match the prepared blueprint")
        if refinement.base_plan_digest != content_digest(blueprint.plan):
            raise BrainRunError("accepted plan refinement base plan does not match the prepared blueprint")
        if refinement.workflow_digest != blueprint.workflow.workflow_digest:
            raise BrainRunError("accepted plan refinement workflow does not match the prepared blueprint")
        stage_ids = tuple(stage.id for stage in blueprint.workflow.stages)
        priority = tuple(refinement.priority_stage_ids)
        if len(priority) != len(stage_ids) or len(set(priority)) != len(priority) or set(priority) != set(stage_ids):
            raise BrainRunError("accepted plan refinement must contain every workflow stage exactly once")
        positions = {stage_id: index for index, stage_id in enumerate(priority)}
        for stage in blueprint.workflow.stages:
            if any(positions[dependency] > positions[stage.id] for dependency in stage.depends_on):
                raise BrainRunError("accepted plan refinement violates workflow dependencies")
        return positions, content_digest(refinement.to_dict()), tuple(refinement.focus_stage_ids)

    @staticmethod
    def _accepted_cross_domain_plan(
        blueprint: AutonomousCrossDomainBlueprint,
        refinement: AutonomousCrossDomainPlanRefinementResult | None,
    ) -> tuple[dict[str, int], str | None, tuple[str, ...]]:
        """Validate an explicitly accepted child-priority proposal before fan-out."""

        if refinement is None:
            return {}, None, ()
        if not isinstance(refinement, AutonomousCrossDomainPlanRefinementResult):
            raise BrainRunError(
                "accepted cross-domain plan refinement must be an AutonomousCrossDomainPlanRefinementResult or None"
            )
        if refinement.status != "completed" or refinement.review_required:
            raise BrainRunError("only a completed, non-review cross-domain plan may be accepted")
        if refinement.task_digest != blueprint.task_digest:
            raise BrainRunError("accepted cross-domain plan task does not match the prepared blueprint")
        if refinement.base_plan_digest != _cross_domain_plan_digest(blueprint):
            raise BrainRunError("accepted cross-domain plan base does not match the prepared blueprint")
        child_ids = tuple(blueprint.child_ids)
        priority = tuple(refinement.priority_child_ids)
        if len(priority) != len(child_ids) or len(set(priority)) != len(priority) or set(priority) != set(child_ids):
            raise BrainRunError("accepted cross-domain plan must contain every child exactly once")
        return (
            {child_id: index for index, child_id in enumerate(priority)},
            content_digest(refinement.to_dict()),
            tuple(refinement.focus_child_ids),
        )

    def run_cross_domain_step(
        self,
        *,
        blueprint: AutonomousCrossDomainBlueprint,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        completed_child_results: Mapping[str, BrainRunResult | BrainToolLoopResult | BrainMissionResult] | None = None,
        completed_synthesis_result: BrainRunResult | BrainToolLoopResult | BrainMissionResult | None = None,
        next_child_id: str | None = None,
        accepted_plan_refinement: AutonomousCrossDomainPlanRefinementResult | None = None,
        ledger: BrainLearningLedger | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
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
        bandit_state: Mapping[str, Any] | None = None,
        response_alignments: Sequence[Mapping[str, Any]] = (),
        require_response_alignment: bool = False,
        minimum_response_reward: float = 0.8,
        minimum_response_alignment_confidence: float = 0.75,
        response_contradiction_confidence_threshold: float = 0.75,
        retry_synthesis_after_response_review: bool = False,
        execution_controller: AutonomousExecutionController | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        reserve_cost: AutonomousCostReservationCallback | None = None,
    ) -> AutonomousCrossDomainStepResult:
        """Execute exactly one child or the final synthesis for restart-safe fan-out.

        Completed child results are caller-owned rehydrated values. Their outcome digests are
        checked by the durable worker before this method is called; this method also refuses to
        skip a child or synthesize from an incomplete ordered prefix.
        """

        if not isinstance(blueprint, AutonomousCrossDomainBlueprint):
            raise BrainRunError("cross-domain step requires an AutonomousCrossDomainBlueprint")
        if not isinstance(require_response_alignment, bool):
            raise BrainRunError("require_response_alignment must be a boolean")
        if not isinstance(retry_synthesis_after_response_review, bool):
            raise BrainRunError("retry_synthesis_after_response_review must be a boolean")
        response_alignments = _mapping_sequence("cross-domain response_alignments", response_alignments, maximum=64)
        for name, value in (
            ("minimum_response_reward", minimum_response_reward),
            ("minimum_response_alignment_confidence", minimum_response_alignment_confidence),
            ("response_contradiction_confidence_threshold", response_contradiction_confidence_threshold),
        ):
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or not 0.0 <= float(value) <= 1.0:
                raise BrainRunError(f"{name} must be finite and within [0, 1]")
        plan_priority, plan_refinement_digest, plan_focus_child_ids = self._accepted_cross_domain_plan(
            blueprint,
            accepted_plan_refinement,
        )
        execution_child_ids = tuple(
            sorted(blueprint.child_ids, key=lambda child_id: plan_priority.get(child_id, len(plan_priority)))
        )
        if completed_child_results is None:
            completed_child_results = {}
        if not isinstance(completed_child_results, Mapping):
            raise BrainRunError("cross-domain step completed_child_results must be a mapping")
        child_by_id = dict(zip(blueprint.child_ids, blueprint.child_blueprints))
        prior: dict[str, BrainRunResult | BrainToolLoopResult | BrainMissionResult] = {}
        for child_id, result in completed_child_results.items():
            if child_id not in child_by_id:
                raise BrainRunError("cross-domain step contains an unknown completed child")
            if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                raise BrainRunError("cross-domain step completed child result is unsupported")
            if not result.status.startswith("completed"):
                raise BrainRunError("cross-domain step cannot rehydrate an incomplete child result")
            prior[child_id] = result
        if completed_synthesis_result is not None and not isinstance(
            completed_synthesis_result,
            (BrainRunResult, BrainToolLoopResult, BrainMissionResult),
        ):
            raise BrainRunError("cross-domain step completed synthesis result is unsupported")
        if completed_synthesis_result is not None and not completed_synthesis_result.status.startswith("completed"):
            raise BrainRunError("cross-domain step cannot rehydrate an incomplete synthesis result")

        def child_context_for(child_id: str) -> dict[str, Any]:
            child = child_by_id[child_id]
            child_context = dict(child.spec.context)
            if plan_refinement_digest is not None:
                child_context["accepted_cross_domain_plan"] = {
                    "refinement_digest": plan_refinement_digest,
                    "priority_rank": plan_priority[child_id],
                    "focus": child_id in plan_focus_child_ids,
                }
            return child_context

        def execute_item(
            item: AutonomousTaskBlueprint,
            *,
            item_id: str,
            context: Mapping[str, Any],
            identity_suffix: str,
        ) -> BrainRunResult | BrainToolLoopResult | BrainMissionResult:
            result = self.run(
                task=item.spec.task,
                domain=item.spec.domain,
                model_candidates=model_candidates,
                credentials=credentials,
                capability=item.spec.capability,
                risk_class=item.spec.risk_class,
                constraints=item.spec.constraints,
                desired_outputs=item.spec.desired_outputs,
                context=context,
                max_steps=item.spec.max_steps,
                require_json=item.spec.require_json,
                structured_domain_response=item.spec.structured_domain_response,
                require_response_review=False,
                response_schema=None if item.spec.structured_domain_response else item.spec.response_schema,
                execution_mode=item.spec.execution_mode,
                required_model_capabilities=tuple(
                    capability
                    for capability in item.required_capabilities
                    if capability not in item.profile.required_model_capabilities
                ),
                ledger=ledger,
                memory=memory,
                memory_query=memory_query,
                memory_limit=memory_limit,
                contextual_observations=contextual_observations,
                content_parts=content_parts,
                input_tokens=input_tokens,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                selection_weights=selection_weights,
                selection_observations=selection_observations,
                bandit_state=bandit_state,
                approve_provider_call=approve_provider_call,
                approve_mission_dispatch=approve_mission_dispatch,
                run_id=self._cross_domain_identity(f"cross-{identity_suffix}", run_id, item_id),
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                idempotency_key=self._cross_domain_identity("cross-key", idempotency_key, item_id),
                mission_policy=mission_policy,
                mission_options=mission_options,
                route_request=route_request,
                auto_route=auto_route,
                enforce_route_tools=enforce_route_tools,
                require_resolved_route=require_resolved_route,
                provider_tools=provider_tools,
                tool_choice=tool_choice,
                max_provider_failovers=max_provider_failovers,
                tool_loop_options=tool_loop_options,
                execution_controller=execution_controller,
                authorization_context=authorization_context,
                reserve_cost=reserve_cost,
            )
            if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                raise BrainRunError("cross-domain step returned an unsupported brain result")
            return result

        if next_child_id is not None:
            if next_child_id not in execution_child_ids:
                raise BrainRunError("cross-domain step next_child_id is unknown")
            index = execution_child_ids.index(next_child_id)
            expected_prior = execution_child_ids[:index]
            if set(prior) != set(expected_prior):
                raise BrainRunError("cross-domain step must rehydrate exactly the completed ordered prefix")
            result = execute_item(
                child_by_id[next_child_id],
                item_id=next_child_id,
                context=child_context_for(next_child_id),
                identity_suffix="child",
            )
            next_completed = expected_prior + ((next_child_id,) if result.status.startswith("completed") else ())
            digests = {child_id: _autonomous_result_digest(prior[child_id]) for child_id in expected_prior}
            if result.status.startswith("completed"):
                digests[next_child_id] = _autonomous_result_digest(result)
            return AutonomousCrossDomainStepResult(
                status=result.status,
                phase="child",
                item_id=next_child_id,
                blueprint=blueprint,
                result=result,
                execution_child_ids=execution_child_ids,
                completed_child_ids=next_completed,
                child_result_digests=digests,
                plan_refinement_digest=plan_refinement_digest,
            )

        if set(prior) != set(execution_child_ids):
            raise BrainRunError("cross-domain synthesis requires every completed child result")
        response_assessment = None
        structured_mode = any(
            item.spec.structured_domain_response
            for item in (*blueprint.child_blueprints, blueprint.synthesis_blueprint)
        )
        if completed_synthesis_result is not None and not retry_synthesis_after_response_review:
            if structured_mode:
                entries = self._cross_domain_response_entries(
                    blueprint,
                    execution_child_ids,
                    tuple(prior[child_id] for child_id in execution_child_ids),
                    synthesis_result=completed_synthesis_result,
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
            if response_assessment is not None and response_assessment.status != "completed":
                return AutonomousCrossDomainStepResult(
                    status="synthesis_response_review_required",
                    phase="synthesis",
                    item_id="synthesis",
                    blueprint=blueprint,
                    result=completed_synthesis_result,
                    execution_child_ids=execution_child_ids,
                    completed_child_ids=execution_child_ids,
                    child_result_digests={
                        child_id: _autonomous_result_digest(prior[child_id])
                        for child_id in execution_child_ids
                    },
                    plan_refinement_digest=plan_refinement_digest,
                    response_assessment=response_assessment,
                )
            return AutonomousCrossDomainStepResult(
                status=completed_synthesis_result.status,
                phase="synthesis",
                item_id="synthesis",
                blueprint=blueprint,
                result=completed_synthesis_result,
                execution_child_ids=execution_child_ids,
                completed_child_ids=execution_child_ids,
                child_result_digests={
                    child_id: _autonomous_result_digest(prior[child_id])
                    for child_id in execution_child_ids
                },
                plan_refinement_digest=plan_refinement_digest,
                response_assessment=response_assessment,
            )
        if structured_mode:
            entries = self._cross_domain_response_entries(
                blueprint,
                execution_child_ids,
                tuple(prior[child_id] for child_id in execution_child_ids),
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
        if response_assessment is not None and not response_assessment.ready_to_synthesize:
            return AutonomousCrossDomainStepResult(
                status="response_review_required",
                phase="synthesis",
                item_id="synthesis",
                blueprint=blueprint,
                result=None,
                execution_child_ids=execution_child_ids,
                completed_child_ids=execution_child_ids,
                child_result_digests={
                    child_id: _autonomous_result_digest(prior[child_id])
                    for child_id in execution_child_ids
                },
                plan_refinement_digest=plan_refinement_digest,
                response_assessment=response_assessment,
            )
        child_outputs = [
            {
                "id": child_id,
                "domain": child_by_id[child_id].profile.domain,
                "workflow_id": child_by_id[child_id].workflow.workflow_id,
                "workflow_digest": child_by_id[child_id].workflow.workflow_digest,
                "status": prior[child_id].status,
                "output": self._cross_domain_output(prior[child_id]),
                "output_digest": content_digest({"output": self._cross_domain_output(prior[child_id])}),
            }
            for child_id in execution_child_ids
        ]
        synthesis = blueprint.synthesis_blueprint
        synthesis_context = dict(synthesis.spec.context)
        synthesis_context["child_outputs"] = child_outputs
        if plan_refinement_digest is not None:
            synthesis_context["accepted_cross_domain_plan"] = {
                "refinement_digest": plan_refinement_digest,
                "priority_child_ids": list(execution_child_ids),
                "focus_child_ids": list(plan_focus_child_ids),
            }
        synthesis_result = execute_item(
            synthesis,
            item_id="synthesis",
            context=synthesis_context,
            identity_suffix="synthesis",
        )
        if structured_mode and synthesis_result.status.startswith("completed"):
            entries = self._cross_domain_response_entries(
                blueprint,
                execution_child_ids,
                tuple(prior[child_id] for child_id in execution_child_ids),
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
                if response_assessment.status != "completed":
                    return AutonomousCrossDomainStepResult(
                        status="synthesis_response_review_required",
                        phase="synthesis",
                        item_id="synthesis",
                        blueprint=blueprint,
                        result=synthesis_result,
                        execution_child_ids=execution_child_ids,
                        completed_child_ids=execution_child_ids,
                        child_result_digests={
                            child_id: _autonomous_result_digest(prior[child_id])
                            for child_id in execution_child_ids
                        },
                        plan_refinement_digest=plan_refinement_digest,
                        response_assessment=response_assessment,
                    )
        return AutonomousCrossDomainStepResult(
            status=synthesis_result.status,
            phase="synthesis",
            item_id="synthesis",
            blueprint=blueprint,
            result=synthesis_result,
            execution_child_ids=execution_child_ids,
            completed_child_ids=execution_child_ids,
            child_result_digests={
                child_id: _autonomous_result_digest(prior[child_id])
                for child_id in execution_child_ids
            },
            plan_refinement_digest=plan_refinement_digest,
            response_assessment=response_assessment,
        )

    def run_workflow(
        self,
        *,
        blueprint: AutonomousTaskBlueprint,
        model_candidates: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle],
        checkpoint: AutonomousWorkflowCheckpoint | Mapping[str, Any] | None = None,
        accepted_plan_refinement: AutonomousPlanRefinementResult | None = None,
        retry_blocked: bool = False,
        max_stage_calls: int | None = None,
        stage_execution_mode: str | None = None,
        ledger: BrainLearningLedger | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        memory: BrainEpisodicMemory | None = None,
        memory_query: MemoryQuery | Mapping[str, Any] | None = None,
        memory_limit: int = 8,
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
        domain_policy_mode: str = "audit",
        domain_policy_evidence_ready: bool | None = None,
        domain_policy_evaluator_configured: bool | None = None,
        domain_policy_plan_accepted: bool | None = None,
        domain_policy_effects_requested: bool | None = None,
        domain_policy_effects_approved: bool | None = None,
        context: Mapping[str, Any] | None = None,
        content_parts: Sequence[ProviderContentPart | Mapping[str, Any]] | None = None,
        prompt_template: AutonomousPromptTemplate | None = None,
        prompt_registry: AutonomousPromptRegistry | None = None,
        prompt_selection: AutonomousPromptSelectionPlan | Mapping[str, Any] | None = None,
        prompt_stage: str | None = None,
        prompt_learning_state: AutonomousPromptLearningState | Mapping[str, Any] | None = None,
        prompt_learning_exploration: float = 0.35,
        execution_plan_context: Mapping[str, Any] | None = None,
        execution_controller: AutonomousExecutionController | None = None,
        invocation_observer: ProviderInvocationObserver | None = None,
        trace_event_callback: Callable[..., Any] | None = None,
        authorization_context: AutonomousAuthorizationContext | None = None,
        reserve_cost: AutonomousCostReservationCallback | None = None,
    ) -> AutonomousWorkflowRun:
        """Execute a prepared domain workflow as a resumable, dependency-checked stage DAG.

        Each stage is a separate structured model decision. Only a stage that returned
        ``completed`` with evidence can unlock its dependents. Approval refusals, malformed
        structured output, or a model-declared blocked/proposed stage stop the DAG and produce a checkpoint;
        resuming with that checkpoint never replays completed stages.
        """

        if not isinstance(blueprint, AutonomousTaskBlueprint):
            raise BrainRunError("workflow execution requires an AutonomousTaskBlueprint")
        _assert_task_decision_allows_provider(
            blueprint.task_decision,
            scope="workflow execution",
        )
        if invocation_observer is not None and not all(
            callable(getattr(invocation_observer, name, None)) for name in ("before", "after")
        ):
            raise BrainRunError("workflow invocation_observer must implement before and after")
        if trace_event_callback is not None and not callable(trace_event_callback):
            raise BrainRunError("workflow trace_event_callback must be callable or None")
        if bandit_state is not None:
            if not isinstance(bandit_state, Mapping):
                raise BrainRunError("workflow bandit_state must be a mapping or None")
            BrainLearningLedger._assert_safe(bandit_state)
        if not isinstance(retry_blocked, bool):
            raise BrainRunError("retry_blocked must be a boolean")
        if stage_execution_mode is not None and stage_execution_mode not in AUTONOMOUS_EXECUTION_MODES:
            raise BrainRunError("stage_execution_mode must be a supported autonomous execution mode")
        if max_stage_calls is None:
            max_stage_calls = len(blueprint.workflow.stages)
        if not isinstance(max_stage_calls, int) or isinstance(max_stage_calls, bool) or not 1 <= max_stage_calls <= 16:
            raise BrainRunError("max_stage_calls must be between 1 and 16")
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
        if execution_plan_context is not None:
            if not isinstance(execution_plan_context, Mapping):
                raise BrainRunError("workflow execution_plan_context must be a mapping or None")
            _safe_json("workflow execution_plan_context", execution_plan_context, maximum=MAX_AUTONOMOUS_EXECUTION_PLAN_BYTES)
        if context is not None:
            if not isinstance(context, Mapping):
                raise BrainRunError("workflow context must be a mapping or None")
            _safe_json("workflow context", context, maximum=MAX_AUTONOMY_CONTEXT_BYTES)
        plan_priority, plan_refinement_digest, plan_focus_stage_ids = self._accepted_workflow_plan(
            blueprint,
            accepted_plan_refinement,
        )
        if checkpoint is None:
            current_checkpoint = None
        elif isinstance(checkpoint, AutonomousWorkflowCheckpoint):
            current_checkpoint = checkpoint
        elif isinstance(checkpoint, Mapping):
            current_checkpoint = AutonomousWorkflowCheckpoint.from_dict(checkpoint)
        else:
            raise BrainRunError("workflow checkpoint must be a checkpoint, mapping, or None")
        workflow_run_id = run_id or (
            current_checkpoint.run_id if current_checkpoint is not None else f"workflow-{uuid.uuid4().hex}"
        )
        _identifier("workflow run_id", workflow_run_id)
        if current_checkpoint is None:
            current_checkpoint = self._workflow_checkpoint(
                run_id=workflow_run_id,
                blueprint=blueprint,
                snapshots=(),
                plan_refinement_digest=plan_refinement_digest,
            )
        if current_checkpoint.task_digest != blueprint.spec.task_digest:
            raise BrainRunError("workflow checkpoint task does not match the prepared blueprint")
        if current_checkpoint.workflow_id != blueprint.workflow.workflow_id or current_checkpoint.workflow_digest != blueprint.workflow.workflow_digest:
            raise BrainRunError("workflow checkpoint workflow does not match the prepared blueprint")
        if current_checkpoint.plan_refinement_digest != plan_refinement_digest:
            raise BrainRunError("workflow checkpoint plan refinement does not match the requested execution")
        if current_checkpoint.run_id != workflow_run_id:
            raise BrainRunError("workflow checkpoint run_id does not match the requested run")
        _emit_trace_event(
            trace_event_callback,
            phase="plan_compiled",
            status="running",
            plan_digest=blueprint.plan.get("plan_digest"),
            detail_digest=content_digest({"workflow_digest": blueprint.workflow.workflow_digest}),
        )
        stage_by_id = {stage.id: stage for stage in blueprint.workflow.stages}
        if any(row["stage_id"] not in stage_by_id for row in current_checkpoint.stages):
            raise BrainRunError("workflow checkpoint contains a stage outside the prepared workflow")
        snapshots: dict[str, dict[str, Any]] = {row["stage_id"]: dict(row) for row in current_checkpoint.stages}
        if any(row["status"] in {"blocked", "proposed", "not_attempted"} for row in snapshots.values()) and not retry_blocked:
            blocked_ids = tuple(
                row["stage_id"] for row in snapshots.values() if row["status"] in {"blocked", "proposed", "not_attempted"}
            )
            next_ids = tuple(sorted(blocked_ids))
            return AutonomousWorkflowRun(
                workflow_run_id,
                "stage_blocked" if any(row["status"] == "blocked" for row in snapshots.values()) else "stage_proposed",
                blueprint,
                (),
                self._workflow_checkpoint(
                    run_id=workflow_run_id,
                    blueprint=blueprint,
                    snapshots=tuple(snapshots.values()),
                    plan_refinement_digest=plan_refinement_digest,
                ),
                next_ids,
            )
        if retry_blocked:
            for stage_id in tuple(snapshots):
                if snapshots[stage_id]["status"] in {"blocked", "proposed", "not_attempted"}:
                    del snapshots[stage_id]
        stage_results: list[AutonomousWorkflowStageResult] = []
        calls = 0
        while calls < max_stage_calls:
            completed = {
                stage_id for stage_id, snapshot in snapshots.items()
                if snapshot.get("status") == "completed" and snapshot.get("execution_status") == "completed"
            }
            ready_candidates = [
                stage for stage in blueprint.workflow.stages
                if stage.id not in snapshots and set(stage.depends_on).issubset(completed)
            ]
            ready = min(
                ready_candidates,
                key=lambda stage: plan_priority.get(stage.id, len(blueprint.workflow.stages)),
                default=None,
            )
            if ready is None:
                remaining = [stage.id for stage in blueprint.workflow.stages if stage.id not in snapshots]
                status = "completed" if not remaining else "stage_blocked"
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    status,
                    blueprint,
                    tuple(stage_results),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    tuple(remaining),
                )
            stage_execution_plan = compile_autonomous_workflow_stage_execution_plan(
                blueprint,
                ready,
                execution_plan_context=execution_plan_context,
                provider_tools=provider_tools,
            )
            _emit_trace_event(
                trace_event_callback,
                phase="plan_compiled",
                status="running",
                plan_digest=stage_execution_plan.stage_plan_digest,
                detail_digest=content_digest({"stage_id": ready.id, "workflow_digest": blueprint.workflow.workflow_digest}),
            )
            if ready.approval_required and ready.id not in set(approved_stage_ids):
                stage_report = AutonomousWorkflowStageResult(
                    stage=ready,
                    execution_status="approval_required",
                    declared_status=None,
                    result=None,
                    structured=None,
                    validation_errors=("workflow_stage_approval_required",),
                    stage_execution_plan=stage_execution_plan.to_dict(),
                )
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    "approval_required",
                    blueprint,
                    (stage_report,),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    (ready.id,),
                )
            calls += 1
            stage_task = _text(
                "workflow stage task",
                f"{blueprint.spec.task}\n\nExecute workflow stage {ready.id}: {ready.objective}",
                maximum=MAX_AUTONOMY_TEXT_BYTES,
            )
            dependency_outputs = {
                dependency: snapshots[dependency]["structured"]
                for dependency in ready.depends_on
                if dependency in snapshots
            }
            stage_context = {
                "workflow_id": blueprint.workflow.workflow_id,
                "workflow_digest": blueprint.workflow.workflow_digest,
                "parent_task_digest": blueprint.spec.task_digest,
                "stage": ready.to_dict(),
                "dependency_outputs": dependency_outputs,
                "completed_stage_ids": sorted(completed),
                "accepted_plan": None
                if plan_refinement_digest is None
                else {
                    "refinement_digest": plan_refinement_digest,
                    "priority_rank": plan_priority[ready.id],
                    "focus_stage": ready.id in plan_focus_stage_ids,
                },
                "checkpoint_digest": current_checkpoint.checkpoint_digest,
                "does_not_authorize": [
                    "skipping caller approval",
                    "claiming an external effect",
                    "widening the workflow or tool policy",
                ],
            }
            if execution_plan_context is not None:
                stage_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = dict(execution_plan_context)
            if context is not None:
                stage_context["caller_context"] = dict(context)
            stage_context[_AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY] = stage_execution_plan.to_dict()
            stage_provider_tools = tuple(
                tool for tool in provider_tools
                if tool.name in set(stage_execution_plan.selected_tool_names)
            )
            stage_result = self.run(
                task=stage_task,
                domain=blueprint.spec.domain,
                model_candidates=model_candidates,
                credentials=credentials,
                capability=ready.required_capabilities[0],
                risk_class=blueprint.spec.risk_class,
                constraints=blueprint.spec.constraints,
                desired_outputs=ready.evidence_outputs,
                context=stage_context,
                max_steps=blueprint.spec.max_steps,
                require_json=True,
                response_schema=blueprint.workflow.stage_response_schema(ready.id),
                execution_mode=stage_execution_mode or blueprint.spec.execution_mode,
                required_model_capabilities=blueprint.required_capabilities,
                ledger=ledger,
                bandit_state=bandit_state,
                memory=memory,
                memory_query=memory_query,
                memory_limit=memory_limit,
                contextual_observations=contextual_observations,
                content_parts=content_parts,
                prompt_template=prompt_template,
                prompt_registry=prompt_registry,
                prompt_selection=prompt_selection,
                prompt_stage=ready.id if prompt_stage is None else prompt_stage,
                prompt_learning_state=prompt_learning_state,
                prompt_learning_exploration=prompt_learning_exploration,
                input_tokens=input_tokens,
                requested_output_tokens=requested_output_tokens,
                max_cost_per_million_tokens=max_cost_per_million_tokens,
                max_latency_ms=max_latency_ms,
                min_quality=min_quality,
                selection_overrides=selection_overrides,
                selection_weights=selection_weights,
                selection_observations=selection_observations,
                approve_provider_call=approve_provider_call,
                approve_mission_dispatch=approve_mission_dispatch,
                run_id=f"{workflow_run_id}-stage-{ready.id}",
                max_output_tokens=max_output_tokens,
                temperature=temperature,
                idempotency_key=None if idempotency_key is None else f"{idempotency_key}-stage-{ready.id}",
                mission_policy=mission_policy,
                mission_options=mission_options,
                route_request=self._workflow_stage_route_request(route_request, task=stage_task, stage=ready),
                auto_route=auto_route,
                enforce_route_tools=enforce_route_tools,
                require_resolved_route=require_resolved_route,
                provider_tools=stage_provider_tools,
                tool_choice=tool_choice,
                max_provider_failovers=max_provider_failovers,
                tool_loop_options=tool_loop_options,
                domain_policy_mode=domain_policy_mode,
                domain_policy_evidence_ready=domain_policy_evidence_ready,
                domain_policy_evaluator_configured=domain_policy_evaluator_configured,
                domain_policy_plan_accepted=domain_policy_plan_accepted,
                domain_policy_effects_requested=domain_policy_effects_requested,
                domain_policy_effects_approved=domain_policy_effects_approved,
                execution_controller=execution_controller,
                invocation_observer=invocation_observer,
                trace_event_callback=trace_event_callback,
                authorization_context=authorization_context,
                reserve_cost=reserve_cost,
            )
            if not isinstance(stage_result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
                raise BrainRunError("workflow stage returned an unsupported result")
            execution_status = self._workflow_execution_status(stage_result)
            structured = self._workflow_structured_output(stage_result)
            declared, evidence, uncertainty, errors = self._validate_workflow_stage_output(ready, structured)
            response = self._workflow_provider_response(stage_result)
            response_digest = None if response is None else content_digest(response.to_dict())
            if errors and execution_status == "completed":
                execution_status = "provider_failed"
            response_evaluation = None
            if structured is not None and not errors:
                response_evaluation = evaluate_autonomous_workflow_stage_response(
                    structured,
                    domain=blueprint.domain_pack.domain,
                    workflow_id=blueprint.workflow.workflow_id,
                    workflow_digest=blueprint.workflow.workflow_digest,
                    stage_id=ready.id,
                ).to_dict()
            quality_gate_failed = (
                execution_status == "completed"
                and not errors
                and declared == "completed"
                and response_evaluation is not None
                and response_evaluation["passed"] is False
            )
            stage_validation_errors = tuple(errors)
            if quality_gate_failed:
                missing = response_evaluation.get("missing_signals", ())
                stage_validation_errors = (
                    *stage_validation_errors,
                    "workflow stage quality gate failed: "
                    + (", ".join(str(item) for item in missing) or "integrity evaluation"),
                )
                execution_status = "blocked"
            stage_report = AutonomousWorkflowStageResult(
                stage=ready,
                execution_status=execution_status,
                declared_status=declared,
                result=stage_result,
                structured=structured,
                evidence=evidence,
                uncertainty=uncertainty,
                validation_errors=stage_validation_errors,
                response_digest=response_digest,
                attempt=1,
                stage_execution_plan=stage_execution_plan.to_dict(),
                response_evaluation=response_evaluation,
            )
            _emit_trace_event(
                trace_event_callback,
                phase="evaluation_settled",
                status="running",
                plan_digest=stage_execution_plan.stage_plan_digest,
                detail_digest=response_digest,
                failure_code=(
                    "workflow_stage_quality_gate_failed"
                    if quality_gate_failed
                    else "workflow_stage_validation_failed"
                    if errors
                    else None
                ),
            )
            stage_results.append(stage_report)
            snapshot = stage_report.checkpoint_snapshot()
            if snapshot is not None and not errors:
                if quality_gate_failed:
                    # Preserve the evaluator and transient structured output for an explicit
                    # retry, while making the stage non-completable until retry_blocked=True.
                    snapshot = {
                        **snapshot,
                        "status": "blocked",
                        "execution_status": "blocked",
                    }
                snapshots[ready.id] = snapshot
            if execution_status == "approval_required":
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    "approval_required",
                    blueprint,
                    tuple(stage_results),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    (ready.id,),
                )
            if quality_gate_failed:
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    "stage_blocked",
                    blueprint,
                    tuple(stage_results),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    (ready.id,),
                )
            if execution_status != "completed":
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    "stage_failed",
                    blueprint,
                    tuple(stage_results),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    (ready.id,),
                )
            if declared != "completed" or errors:
                status = {
                    "blocked": "stage_blocked",
                    "proposed": "stage_proposed",
                    "not_attempted": "stage_not_attempted",
                }.get(declared, "stage_failed")
                return AutonomousWorkflowRun(
                    workflow_run_id,
                    status,
                    blueprint,
                    tuple(stage_results),
                    self._workflow_checkpoint(
                        run_id=workflow_run_id,
                        blueprint=blueprint,
                        snapshots=tuple(snapshots.values()),
                        plan_refinement_digest=plan_refinement_digest,
                    ),
                    (ready.id,),
                )
            current_checkpoint = self._workflow_checkpoint(
                run_id=workflow_run_id,
                blueprint=blueprint,
                snapshots=tuple(snapshots.values()),
                plan_refinement_digest=plan_refinement_digest,
            )
        completed = {
            stage_id for stage_id, snapshot in snapshots.items()
            if snapshot.get("status") == "completed" and snapshot.get("execution_status") == "completed"
        }
        next_ids = tuple(
            stage.id for stage in blueprint.workflow.stages
            if stage.id not in snapshots and set(stage.depends_on).issubset(completed)
        )
        remaining = tuple(stage.id for stage in blueprint.workflow.stages if stage.id not in snapshots)
        final_status = "completed" if not remaining else ("paused" if next_ids else "stage_blocked")
        return AutonomousWorkflowRun(
            workflow_run_id,
            final_status,
            blueprint,
            tuple(stage_results),
            self._workflow_checkpoint(
                run_id=workflow_run_id,
                blueprint=blueprint,
                snapshots=tuple(snapshots.values()),
                plan_refinement_digest=plan_refinement_digest,
            ),
            next_ids,
        )

    @staticmethod
    def _workflow_stage_evidence(
        blueprint: AutonomousTaskBlueprint,
        stage: AutonomousWorkflowStage,
        raw: Mapping[str, Any] | None,
        stage_execution_plan: Mapping[str, Any] | None = None,
    ) -> dict[str, Any] | None:
        if raw is None:
            return None
        if not isinstance(raw, Mapping):
            raise BrainRunError(f"workflow stage evidence for {stage.id} must be a mapping")
        unknown = sorted(set(raw).difference({"signals", "references", "limitations"}))
        if unknown:
            raise BrainRunError(
                f"workflow stage evidence for {stage.id} contains unsupported fields: {', '.join(unknown)}"
            )
        signals = raw.get("signals", {})
        if not isinstance(signals, Mapping):
            raise BrainRunError(f"workflow stage evidence signals for {stage.id} must be a mapping")
        normalized_signals: dict[str, float | bool] = {}
        for signal, value in signals.items():
            _identifier("workflow stage evidence signal", signal)
            if isinstance(value, bool):
                normalized_signals[signal] = value
            elif isinstance(value, (int, float)) and not isinstance(value, bool):
                if value < 0 or value > 1:
                    raise BrainRunError("workflow stage evidence signal values must be within [0, 1]")
                normalized_signals[signal] = float(value)
            else:
                raise BrainRunError("workflow stage evidence signal values must be booleans or numbers")
        references = raw.get("references", ())
        limitations = raw.get("limitations", ())
        references = _sequence(f"workflow stage {stage.id} references", references, maximum=32)
        for reference in references:
            _workflow_digest(reference, f"workflow stage {stage.id} reference")
        limitations = _sequence(f"workflow stage {stage.id} limitations", limitations, maximum=32)
        evidence = {
            "schema": AUTONOMOUS_WORKFLOW_EVALUATOR_SCHEMA,
            "workflow_id": blueprint.workflow.workflow_id,
            "workflow_digest": blueprint.workflow.workflow_digest,
            "stage_id": stage.id,
            "required_signals": list(stage.evaluator_signals),
            "domain": blueprint.domain_pack.domain,
            "capability": stage.required_capabilities[0] if stage.required_capabilities else blueprint.spec.capability,
            "risk_class": blueprint.spec.risk_class,
            "signals": normalized_signals,
            "references": list(references),
            "limitations": list(limitations),
        }
        if stage_execution_plan is not None:
            if not isinstance(stage_execution_plan, Mapping):
                raise BrainRunError("workflow stage execution plan evidence must be a mapping")
            stage_plan_digest = stage_execution_plan.get("stage_plan_digest")
            contract_digests = stage_execution_plan.get("capability_contract_digests", ())
            selected_tool_names = stage_execution_plan.get("selected_tool_names", ())
            if not isinstance(stage_plan_digest, str):
                raise BrainRunError("workflow stage evidence is missing stage_plan_digest")
            _workflow_digest(stage_plan_digest, "workflow stage evidence stage_plan_digest")
            if not isinstance(contract_digests, Sequence) or isinstance(contract_digests, (str, bytes)):
                raise BrainRunError("workflow stage evidence capability_contract_digests must be a sequence")
            for digest in contract_digests:
                _workflow_digest(digest, "workflow stage evidence capability contract digest")
            if not isinstance(selected_tool_names, Sequence) or isinstance(selected_tool_names, (str, bytes)):
                raise BrainRunError("workflow stage evidence selected_tool_names must be a sequence")
            evidence["stage_plan_digest"] = stage_plan_digest
            evidence["capability_contract_digests"] = list(contract_digests)
            evidence["selected_tool_names"] = list(
                _sequence("workflow stage evidence selected_tool_names", selected_tool_names, maximum=64)
            )
        return _safe_json(
            f"workflow stage {stage.id} evidence",
            evidence,
            maximum=250_000,
        )

    def run_workflow_learning(
        self,
        *,
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        stage_evidence: Mapping[str, Mapping[str, Any]] | None = None,
        memory_tags: Sequence[str] = (),
        memory: BrainEpisodicMemory | None = None,
        **workflow_kwargs: Any,
    ) -> AutonomousWorkflowLearningResult:
        """Execute a workflow and record one explicit bandit update per completed stage.

        Learning is intentionally separate from execution. A stage is evaluated only after its
        structured result is complete; missing evidence produces a failed reward rather than a
        default success. Replanning is reported to the caller and never silently replays a stage
        that may have crossed an external boundary.
        """

        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("workflow learning bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if stage_evidence is not None:
            if not isinstance(stage_evidence, Mapping):
                raise BrainRunError("workflow stage_evidence must be a mapping or None")
            if any(not isinstance(stage_id, str) or not isinstance(value, Mapping) for stage_id, value in stage_evidence.items()):
                raise BrainRunError("workflow stage_evidence must map stage ids to mappings")
            _safe_json("workflow stage_evidence", stage_evidence, maximum=1_000_000)
        memory_store = memory if memory is not None else self.brain.memory
        if memory_store is not None and not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("workflow learning memory must be a BrainEpisodicMemory or None")
        authorization_context = workflow_kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("workflow learning authorization_context must be an AutonomousAuthorizationContext or None")
        normalized_tags = _sequence("workflow learning memory_tags", memory_tags, maximum=32)
        blueprint = workflow_kwargs.get("blueprint")
        if not isinstance(blueprint, AutonomousTaskBlueprint):
            raise BrainRunError("workflow learning requires a prepared AutonomousTaskBlueprint")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("workflow learning evaluator must be a BrainOutcomeEvaluator or None")
        if evaluator_registry is not None and not isinstance(evaluator_registry, DomainEvaluatorRegistry):
            raise BrainRunError("workflow evaluator_registry must be a DomainEvaluatorRegistry or None")
        resolved_evaluator = evaluator
        if resolved_evaluator is None and evaluator_registry is not None:
            resolved_evaluator = evaluator_registry.resolve_for_autonomous_domain(
                blueprint.domain_pack.domain,
                fallback_domain=blueprint.domain_pack.evaluator_domain,
            )
        if resolved_evaluator is None:
            resolved_evaluator = AutonomousWorkflowEvaluator(blueprint.workflow)
        state: Mapping[str, Any] = dict(bandit_state)
        evaluations: list[AutonomousWorkflowStageEvaluation] = []
        receipts: list[Mapping[str, Any]] = []
        should_replan = False
        requested_calls = workflow_kwargs.get("max_stage_calls")
        if requested_calls is None:
            requested_calls = len(blueprint.workflow.stages)
        if not isinstance(requested_calls, int) or isinstance(requested_calls, bool) or not 1 <= requested_calls <= 16:
            raise BrainRunError("workflow learning max_stage_calls must be between 1 and 16")
        continuation_kwargs = dict(workflow_kwargs)
        continuation_kwargs.pop("bandit_state", None)
        checkpoint = continuation_kwargs.get("checkpoint")
        workflow_run: AutonomousWorkflowRun | None = None
        all_stage_results: list[AutonomousWorkflowStageResult] = []
        for _ in range(requested_calls):
            call_kwargs = dict(continuation_kwargs)
            call_kwargs["max_stage_calls"] = 1
            call_kwargs["bandit_state"] = dict(state)
            if checkpoint is not None:
                call_kwargs["checkpoint"] = checkpoint
            workflow_run = self.run_workflow(memory=memory_store, **call_kwargs)
            all_stage_results.extend(workflow_run.stage_results)
            checkpoint = workflow_run.checkpoint
            for stage_result in workflow_run.stage_results:
                if (
                    stage_result.result is None
                    or stage_result.execution_status != "completed"
                    or stage_result.declared_status != "completed"
                ):
                    continue
                evidence = self._workflow_stage_evidence(
                    blueprint,
                    stage_result.stage,
                    None if stage_evidence is None else stage_evidence.get(stage_result.stage.id),
                    stage_result.stage_execution_plan,
                )
                decision, report = resolved_evaluator.evaluate_and_record_with_decision(
                    self.brain,
                    stage_result.result,
                    bandit_state=state,
                    evidence=evidence,
                    ledger=continuation_kwargs.get("ledger"),
                )
                next_state = report.get("next_state")
                if isinstance(next_state, Mapping):
                    state = dict(next_state)
                structured_feedback = _record_workflow_stage_response_feedback(
                    self.brain,
                    stage_result,
                    bandit_state=state,
                    ledger=continuation_kwargs.get("ledger"),
                )
                structured_record = None
                if structured_feedback is not None:
                    state, structured_record = structured_feedback
                should_replan = should_replan or decision.replan_requested
                evaluation = AutonomousWorkflowStageEvaluation(
                    stage_id=stage_result.stage.id,
                    stage_status=stage_result.declared_status,
                    decision=decision,
                    recording={
                        "status": report.get("status"),
                        "next_state": report.get("next_state"),
                        "learning_evidence": report.get("learning_evidence"),
                        "structured_response": structured_record,
                    },
                    evidence_digest=decision.evidence_digest,
                    structured_response=structured_record,
                )
                evaluations.append(evaluation)
                if memory_store is not None:
                    episode_id = f"{workflow_run.run_id}-{stage_result.stage.id}"
                    if len(episode_id.encode("utf-8")) > 256:
                        episode_id = "episode-" + content_digest({"run_id": workflow_run.run_id, "stage_id": stage_result.stage.id})
                    receipt = self.brain.remember_result(
                        stage_result.result,
                        task=blueprint.spec.task,
                        episode_id=episode_id,
                        context=_memory_selection_context(blueprint),
                        tags=[
                            *normalized_tags,
                            f"domain:{blueprint.spec.domain}",
                            f"workflow:{blueprint.workflow.workflow_id}",
                            f"stage:{stage_result.stage.id}",
                        ],
                        lesson=decision.replan_instruction if decision.replan_requested else None,
                        provenance={
                            "workflow_id": blueprint.workflow.workflow_id,
                            "workflow_digest": blueprint.workflow.workflow_digest,
                            "stage_id": stage_result.stage.id,
                            "evaluator_id": decision.evaluator_id,
                            "evaluator_version": decision.evaluator_version,
                            "task_lens_digest": _memory_task_lens_digest(blueprint),
                            "task_intent_digest": _memory_task_intent_digest(blueprint),
                            "task_decision_digest": _memory_task_decision_digest(blueprint),
                        },
                        memory=memory_store,
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
                        evaluation_receipt = memory_store.record_evaluation(
                            episode_id,
                            {
                                **decision.to_dict(),
                                "decision_digest": decision_digest,
                            },
                        ).to_dict()
                    except BrainMemoryError as error:
                        raise BrainRunError("workflow stage evaluation memory record failed") from error
                    receipts.extend((receipt, evaluation_receipt))
            if workflow_run.status != "paused" or should_replan or not workflow_run.next_stage_ids:
                break
        if workflow_run is None:
            raise BrainRunError("workflow learning did not produce a workflow run")
        if len(all_stage_results) != len(workflow_run.stage_results):
            workflow_run = AutonomousWorkflowRun(
                workflow_run.run_id,
                workflow_run.status,
                workflow_run.blueprint,
                tuple(all_stage_results),
                workflow_run.checkpoint,
                workflow_run.next_stage_ids,
            )
        if should_replan:
            learning_status = "learning_replan_requested"
        elif workflow_run.status == "completed":
            learning_status = "completed"
        else:
            learning_status = workflow_run.status
        return AutonomousWorkflowLearningResult(
            status=learning_status,
            workflow=workflow_run,
            evaluations=tuple(evaluations),
            bandit_state=state,
            memory_receipts=tuple(receipts),
            replan_requested=should_replan,
        )

    def run_workflow_cycle(self, **kwargs: Any) -> "AutonomousWorkflowCycleResult":
        """Run a bounded evaluator-guided workflow retry cycle.

        The implementation lives in a small companion module to keep this composition class
        readable and to avoid importing the cycle result types into the hot execution path.
        Importing lazily also keeps the public package import graph acyclic.
        """

        from .workflow_cycle import run_workflow_cycle

        return run_workflow_cycle(self, **kwargs)

    def run_workflow_trajectory_learning(
        self,
        *,
        bandit_state: Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator | None = None,
        evaluator_registry: DomainEvaluatorRegistry | None = None,
        stage_evidence: Mapping[str, Mapping[str, Any]] | None = None,
        trajectory_id: str | None = None,
        trajectory_discount: float = 0.90,
        trajectory_terminal_reward: float | None = None,
        memory_tags: Sequence[str] = (),
        memory: BrainEpisodicMemory | None = None,
        **workflow_kwargs: Any,
    ) -> AutonomousWorkflowTrajectoryLearningResult:
        """Execute a workflow, then assign delayed return-to-go credit across its stages.

        Unlike :meth:`run_workflow_learning`, this mode deliberately postpones all bandit writes
        until the completed stage sequence has been assembled. Model routing therefore reflects
        the supplied starting state during this run, while a final evaluator or terminal review
        can teach earlier stages what downstream success required without double-counting an
        immediate reward.
        """

        if not isinstance(bandit_state, Mapping):
            raise BrainRunError("workflow trajectory bandit_state must be a mapping")
        BrainLearningLedger._assert_safe(bandit_state)
        if stage_evidence is not None:
            if not isinstance(stage_evidence, Mapping) or any(
                not isinstance(stage_id, str) or not isinstance(value, Mapping)
                for stage_id, value in stage_evidence.items()
            ):
                raise BrainRunError("workflow trajectory stage_evidence must map stage ids to mappings")
            _safe_json("workflow trajectory stage_evidence", stage_evidence, maximum=1_000_000)
        memory_store = memory if memory is not None else self.brain.memory
        if memory_store is not None and not isinstance(memory_store, BrainEpisodicMemory):
            raise BrainRunError("workflow trajectory memory must be a BrainEpisodicMemory or None")
        authorization_context = workflow_kwargs.get("authorization_context")
        if authorization_context is not None and not isinstance(authorization_context, AutonomousAuthorizationContext):
            raise BrainRunError("workflow trajectory authorization_context must be an AutonomousAuthorizationContext or None")
        normalized_tags = _sequence("workflow trajectory memory_tags", memory_tags, maximum=32)
        blueprint = workflow_kwargs.get("blueprint")
        if not isinstance(blueprint, AutonomousTaskBlueprint):
            raise BrainRunError("workflow trajectory learning requires a prepared AutonomousTaskBlueprint")
        if evaluator is not None and not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("workflow trajectory evaluator must be a BrainOutcomeEvaluator or None")
        if evaluator_registry is not None and not isinstance(evaluator_registry, DomainEvaluatorRegistry):
            raise BrainRunError("workflow trajectory evaluator_registry must be a DomainEvaluatorRegistry or None")
        resolved_evaluator = evaluator
        if resolved_evaluator is None and evaluator_registry is not None:
            resolved_evaluator = evaluator_registry.resolve_for_autonomous_domain(
                blueprint.domain_pack.domain,
                fallback_domain=blueprint.domain_pack.evaluator_domain,
            )
        if resolved_evaluator is None:
            resolved_evaluator = AutonomousWorkflowEvaluator(blueprint.workflow)
        if not isinstance(resolved_evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("workflow trajectory evaluator must resolve to a BrainOutcomeEvaluator")

        requested_calls = workflow_kwargs.get("max_stage_calls")
        if requested_calls is None:
            requested_calls = len(blueprint.workflow.stages)
        if not isinstance(requested_calls, int) or isinstance(requested_calls, bool) or not 1 <= requested_calls <= 16:
            raise BrainRunError("workflow trajectory max_stage_calls must be between 1 and 16")
        continuation_kwargs = dict(workflow_kwargs)
        continuation_kwargs.pop("bandit_state", None)
        continuation_kwargs.pop("memory", None)
        checkpoint = continuation_kwargs.get("checkpoint")
        workflow_run: AutonomousWorkflowRun | None = None
        all_stage_results: list[AutonomousWorkflowStageResult] = []
        for _ in range(requested_calls):
            call_kwargs = dict(continuation_kwargs)
            call_kwargs["max_stage_calls"] = 1
            call_kwargs["bandit_state"] = dict(bandit_state)
            if checkpoint is not None:
                call_kwargs["checkpoint"] = checkpoint
            workflow_run = self.run_workflow(memory=memory_store, **call_kwargs)
            all_stage_results.extend(workflow_run.stage_results)
            checkpoint = workflow_run.checkpoint
            if workflow_run.status != "paused" or not workflow_run.next_stage_ids:
                break
        if workflow_run is None:
            raise BrainRunError("workflow trajectory learning did not produce a workflow run")
        if len(all_stage_results) != len(workflow_run.stage_results):
            workflow_run = AutonomousWorkflowRun(
                workflow_run.run_id,
                workflow_run.status,
                workflow_run.blueprint,
                tuple(all_stage_results),
                workflow_run.checkpoint,
                workflow_run.next_stage_ids,
            )

        completed: list[AutonomousWorkflowStageResult] = []
        evidence_packets: list[Mapping[str, Any]] = []
        for stage_result in all_stage_results:
            if (
                stage_result.result is None
                or stage_result.execution_status != "completed"
                or stage_result.declared_status != "completed"
            ):
                continue
            completed.append(stage_result)
            evidence_packets.append(
                self._workflow_stage_evidence(
                    blueprint,
                    stage_result.stage,
                    None if stage_evidence is None else stage_evidence.get(stage_result.stage.id),
                    stage_result.stage_execution_plan,
                )
            )

        state: Mapping[str, Any] = dict(bandit_state)
        trajectory_result: BrainLearningTrajectoryResult | None = None
        evaluations: list[AutonomousWorkflowStageEvaluation] = []
        receipts: list[Mapping[str, Any]] = []
        should_replan = False
        if completed:
            ledger = continuation_kwargs.get("ledger")
            if ledger is not None and not isinstance(ledger, BrainLearningLedger):
                raise BrainRunError("workflow trajectory ledger must be a BrainLearningLedger or None")
            trajectory = self.brain.prepare_learning_trajectory(
                [stage.result for stage in completed if stage.result is not None],
                evidence_by_step=evidence_packets,
                trajectory_id=trajectory_id or f"workflow-{workflow_run.run_id}",
                discount=trajectory_discount,
                terminal_reward=trajectory_terminal_reward,
                ledger=ledger,
            )
            trajectory_result = resolved_evaluator.evaluate_trajectory(
                self.brain,
                trajectory,
                bandit_state=state,
                evidence_by_step=evidence_packets,
                ledger=ledger,
            )
            state = dict(trajectory_result.bandit_state)
            for index, stage_result in enumerate(completed):
                decision = trajectory_result.decisions[index]
                should_replan = should_replan or decision.replan_requested
                recording = trajectory_result.recordings[index]
                structured_feedback = _record_workflow_stage_response_feedback(
                    self.brain,
                    stage_result,
                    bandit_state=state,
                    ledger=ledger,
                )
                structured_record = None
                if structured_feedback is not None:
                    state, structured_record = structured_feedback
                evaluation = AutonomousWorkflowStageEvaluation(
                    stage_id=stage_result.stage.id,
                    stage_status=stage_result.declared_status or "completed",
                    decision=decision,
                    recording={
                        "status": recording.get("status"),
                        "next_state": recording.get("next_state"),
                        "learning_evidence": recording.get("learning_evidence"),
                        "trajectory_id": trajectory_result.trajectory.trajectory_id,
                        "trajectory_step": index,
                        "credited_reward": trajectory_result.credited_rewards[index],
                        "structured_response": structured_record,
                    },
                    evidence_digest=decision.evidence_digest,
                    structured_response=structured_record,
                )
                evaluations.append(evaluation)
                if memory_store is not None:
                    episode_id = trajectory_result.trajectory.episodes[index].episode_id
                    receipt = self.brain.remember_result(
                        stage_result.result,
                        task=blueprint.spec.task,
                        episode_id=episode_id,
                        context=_memory_selection_context(blueprint),
                        tags=[
                            *normalized_tags,
                            f"domain:{blueprint.spec.domain}",
                            f"workflow:{blueprint.workflow.workflow_id}",
                            f"stage:{stage_result.stage.id}",
                            "learning:trajectory",
                        ],
                        lesson=decision.replan_instruction if decision.replan_requested else None,
                        provenance={
                            "workflow_id": blueprint.workflow.workflow_id,
                            "workflow_digest": blueprint.workflow.workflow_digest,
                            "stage_id": stage_result.stage.id,
                            "trajectory_id": trajectory_result.trajectory.trajectory_id,
                            "trajectory_step": index,
                            "credited_reward": trajectory_result.credited_rewards[index],
                            "evaluator_id": decision.evaluator_id,
                            "evaluator_version": decision.evaluator_version,
                            "task_lens_digest": _memory_task_lens_digest(blueprint),
                            "task_intent_digest": _memory_task_intent_digest(blueprint),
                            "task_decision_digest": _memory_task_decision_digest(blueprint),
                        },
                        memory=memory_store,
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
                            trajectory_id=trajectory_result.trajectory.trajectory_id,
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
                        raise BrainRunError("workflow trajectory evaluation memory record failed") from error
                    receipts.extend((receipt, evaluation_receipt))

        if should_replan:
            status = "trajectory_learning_replan_requested"
        elif workflow_run.status == "completed":
            status = "completed"
        else:
            status = workflow_run.status
        return AutonomousWorkflowTrajectoryLearningResult(
            status=status,
            workflow=workflow_run,
            trajectory_result=trajectory_result,
            evaluations=tuple(evaluations),
            bandit_state=state,
            memory_receipts=tuple(receipts),
            replan_requested=should_replan,
        )

    @staticmethod
    def _provider_failure_result(
        blueprint: AutonomousTaskBlueprint,
        *,
        child_id: str,
        error: ProviderError | CredentialError,
        run_id: str | None,
    ) -> BrainRunResult:
        """Convert a provider-boundary exception into a safe child result envelope.

        A provider or credential failure is an expected operational outcome for a bounded
        fan-out, not a reason to lose every sibling result.  Only stable class/transport
        metadata crosses this boundary.  In particular, ``str(error)`` is never copied because
        provider implementations and credential stores may include sensitive diagnostics there.
        ``BrainRunError`` and other programming/configuration errors remain exceptions so an
        invalid request cannot be disguised as an unavailable child.
        """

        failure: dict[str, Any] = {
            "error_class": type(error).__name__,
            "retryable": bool(getattr(error, "retryable", False)),
            "circuit_open": bool(getattr(error, "circuit_open", False)),
            "status_code": getattr(error, "status_code", None),
            "retention": "metadata_only;provider_error_message_and_payloads_not_retained",
            "secret_material": "never_returned",
        }
        identity = {
            "schema": "bioprism-python-autonomous-provider-failure/0.1",
            "child_id": child_id,
            "plan_digest": blueprint.plan.get("plan_digest"),
            "error_class": failure["error_class"],
            "retryable": failure["retryable"],
            "circuit_open": failure["circuit_open"],
            "status_code": failure["status_code"],
        }
        failure_run_id = run_id or f"cross-child-failure-{content_digest(identity)[:48]}"
        return BrainRunResult(
            run_id=failure_run_id,
            status="provider_failed",
            selection={
                "status": "provider_failed",
                "selected_model": None,
                "retention": "selection_metadata_only;provider_failure_no_model_payload",
            },
            prompt={
                "prompt_digest": content_digest(blueprint.prompt),
                "retention": "provider_prompt_not_dispatched;digest_only",
            },
            plan={
                "plan_digest": blueprint.plan.get("plan_digest"),
                "status": "provider_failed",
                "retention": "plan_metadata_only",
            },
            response=None,
            outcome_digest=content_digest({**identity, "run_id": failure_run_id}),
            failure=failure,
        )

    @classmethod
    def _provider_failure_call(
        cls,
        blueprint: AutonomousTaskBlueprint,
        *,
        child_id: str,
        run_id: str | None,
        invoke: Callable[[], BrainRunResult | BrainToolLoopResult | BrainMissionResult],
    ) -> BrainRunResult | BrainToolLoopResult | BrainMissionResult:
        """Run one provider-bound item while preserving a typed failure envelope."""

        try:
            result = invoke()
        except (ProviderError, CredentialError) as error:
            result = cls._provider_failure_result(
                blueprint,
                child_id=child_id,
                error=error,
                run_id=run_id,
            )
        if not isinstance(result, (BrainRunResult, BrainToolLoopResult, BrainMissionResult)):
            raise BrainRunError("provider-bound cross-domain call returned an unsupported result")
        return result

    @staticmethod
    def _cross_domain_output(result: BrainRunResult | BrainToolLoopResult | BrainMissionResult) -> str:
        response = None
        if isinstance(result, BrainRunResult):
            response = result.response
        elif isinstance(result, BrainToolLoopResult):
            response = None if result.provider_loop is None else result.provider_loop.final_response
            if response is None:
                response = result.brain_run.response
        elif isinstance(result, BrainMissionResult):
            response = result.brain_run.response
        if response is None or not isinstance(response.text, str):
            return ""
        encoded = response.text.encode("utf-8")[:32_000]
        return encoded.decode("utf-8", errors="ignore")

    def _cross_domain_response_entries(
        self,
        blueprint: AutonomousCrossDomainBlueprint,
        execution_child_ids: Sequence[str],
        child_results: Sequence[BrainRunResult | BrainToolLoopResult | BrainMissionResult],
        *,
        synthesis_result: BrainRunResult | BrainToolLoopResult | BrainMissionResult | None = None,
    ) -> list[dict[str, Any]] | None:
        """Build transient structured-response entries for the synthesis admission gate."""

        if not any(child.response_contract is not None for child in blueprint.child_blueprints) and blueprint.synthesis_blueprint.response_contract is None:
            return None
        child_by_id = dict(zip(blueprint.child_ids, blueprint.child_blueprints))
        entries: list[dict[str, Any]] = []
        for child_id, result in zip(execution_child_ids, child_results):
            child = child_by_id.get(child_id)
            if child is None or child.response_contract is None:
                continue
            provider_response = self._workflow_provider_response(result)
            response = getattr(provider_response, "structured", None)
            if response is None and isinstance(provider_response, Mapping):
                response = provider_response.get("structured")
            if response is None:
                continue
            entries.append({
                "domain": child.profile.domain,
                "contract": child.response_contract,
                "response": response,
                "role": "specialist",
            })
        if synthesis_result is not None and blueprint.synthesis_blueprint.response_contract is not None:
            provider_response = self._workflow_provider_response(synthesis_result)
            response = getattr(provider_response, "structured", None)
            if response is None and isinstance(provider_response, Mapping):
                response = provider_response.get("structured")
            if response is not None:
                entries.append({
                    "domain": blueprint.synthesis_blueprint.profile.domain,
                    "contract": blueprint.synthesis_blueprint.response_contract,
                    "response": response,
                    "role": "synthesis",
                })
        return entries or None

    @staticmethod
    def _cross_domain_identity(prefix: str, parent: str | None, child_id: str) -> str | None:
        if parent is None:
            return None
        return f"{prefix}-{content_digest({'parent': parent, 'child': child_id})}"

    @staticmethod
    def _cross_domain_tool_loop_options(
        options: Mapping[str, Any] | None,
        *,
        execution_id: str | None,
        domain: str,
    ) -> Mapping[str, Any] | None:
        """Bind a shared domain-tool runtime to one specialist without widening authority."""

        if options is None or not isinstance(options, Mapping):
            return options
        runtime = options.get("authorize_and_execute")
        if not isinstance(runtime, AutonomousDomainToolRuntime):
            return options
        scoped = dict(options)
        scoped["authorize_and_execute"] = runtime.scoped(
            execution_id=execution_id or f"cross-tool-{uuid.uuid4().hex}",
            domain=domain,
        )
        return scoped

    @staticmethod
    def _cross_domain_trajectory_items(
        cross_domain: AutonomousCrossDomainResult,
        *,
        allow_partial: bool,
    ) -> list[tuple[str, str, AutonomousTaskBlueprint, BrainRunResult | BrainToolLoopResult | BrainMissionResult]]:
        """Return only completed fan-out/fan-in results that may receive delayed credit.

        Provider, approval, route, and execution failures are useful control-plane evidence, but
        they are not task outcomes. Keeping them out of a trajectory is especially important for
        delayed settlement: ``prepare_learning_trajectory`` intentionally accepts typed result
        envelopes without deciding whether an individual envelope is creditable. This boundary
        therefore performs the status admission once and preserves accepted execution order; the
        original ``cross_domain`` envelope remains available to the caller for control-plane
        inspection without exposing provider payloads through the trajectory.
        """

        if not isinstance(cross_domain, AutonomousCrossDomainResult):
            raise BrainRunError("cross-domain trajectory requires an execution result")
        if not isinstance(allow_partial, bool):
            raise BrainRunError("cross-domain trajectory allow_partial must be a boolean")
        child_by_id = dict(zip(cross_domain.blueprint.child_ids, cross_domain.blueprint.child_blueprints))
        raw_items: list[
            tuple[str, str, AutonomousTaskBlueprint, BrainRunResult | BrainToolLoopResult | BrainMissionResult]
        ] = []
        for child_id, result in zip(cross_domain.execution_child_ids, cross_domain.child_results):
            child = child_by_id[child_id]
            raw_items.append(("child", child_id, child, result))
        if cross_domain.synthesis_result is not None:
            raw_items.append(
                (
                    "synthesis",
                    "synthesis",
                    cross_domain.blueprint.synthesis_blueprint,
                    cross_domain.synthesis_result,
                )
            )
        if not raw_items:
            raise BrainRunError("cross-domain trajectory contains no results to evaluate")
        if any(not item[3].status.startswith("completed") for item in raw_items) and not allow_partial:
            raise BrainRunError(
                "cross-domain trajectory cannot settle a non-completed run in strict mode; "
                "set allow_partial=True to settle completed items only"
            )
        completed_items = [item for item in raw_items if item[3].status.startswith("completed")]
        if not completed_items:
            raise BrainRunError("cross-domain trajectory contains no completed results to evaluate")
        return completed_items

# Keep methods discoverable through the public AutonomousTaskOrchestrator facade.
for _name, _descriptor in vars(AutonomousOrchestratorWorkflowMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousTaskOrchestrator.{_name}"
