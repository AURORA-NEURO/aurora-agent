"""Focused autonomous-agent runs methods."""

from __future__ import annotations

from .autonomy import (
    Any,
    AutonomousAutoReplanResult,
    AutonomousAutoResult,
    AutonomousCrossDomainResult,
    AutonomousMissionReplanCheckpoint,
    AutonomousMissionReplanRehydrationContext,
    AutonomousMissionReplanResult,
    AutonomousMissionReplanStateStore,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
    Callable,
    CredentialHandle,
    CredentialSession,
    DomainEvaluatorRegistry,
    MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS,
    Mapping,
    MissionPolicy,
    ModelCandidate,
    Sequence,
    _apply_versioned_prompt,
    _authorize_launch_admission_domains,
    _provider_messages_with_content_parts,
    content_digest,
    extract_autonomous_prompt_learning_selections,
    normalize_provider_content_parts,
    run_autonomous_mission_replan_cycle,
)

class AutonomousAgentRunsMixin:
    def run(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run a task using the registered catalogue unless an explicit candidate slice is given.

        All existing orchestrator options remain available, including ``learn=True``,
        ``execution_mode="tool_loop"``/``"mission"``, evaluator evidence, bandit state, and
        provider/mission approval.  No option here widens those authorization boundaries.
        """

        run_options = self._prompt_learning_options(kwargs)
        candidates, resolved_credentials, options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=run_options,
            tool_domains=(domain,),
            task=task,
            resume_learning=bool(kwargs.get("learn")),
            execution_id=execution_id,
            resume_execution=resume_execution,
        )
        try:
            result = self.orchestrator.run(
                task=task,
                domain=domain,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **options,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._record_model_quality_from_learning_result(result)
        self._finish_execution(execution_controller, result=result)
        return result

    def run_stream(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run one reviewed domain task as a lazy, provider-neutral event stream.

        The method performs the normal route, blueprint, model-selection, prompt, and plan
        preflight immediately with ``approve_provider_call=False``.  The returned handle starts
        provider transport only when its ``events`` iterator is consumed and only when the
        caller supplied ``approve_provider_call=True``.  Streaming is intentionally limited to
        the direct provider mode: tool-loop, mission, evaluator settlement, and memory writes
        remain on their existing execution APIs, where their separate authority boundaries can
        be represented faithfully.

        ``execution_id``/``resume_execution`` are rejected here because a streamed provider
        response cannot be atomically checkpointed by this synchronous façade.  Deployments that
        need restart recovery should use the existing resumable result controllers and rehydrate
        the caller-owned response explicitly rather than replaying a partial stream.
        """

        from .autonomous_agent_stream import (
            AutonomousAgentStreamHandle,
            build_autonomous_agent_stream_request,
        )
        from .autonomous_stream import AutonomousStreamArm, AutonomousStreamRuntime

        if execution_id is not None or resume_execution:
            raise BrainRunError(
                "run_stream does not support execution persistence; use a resumable result controller"
            )
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("stream task must be a non-empty string")
        if not isinstance(domain, str) or not domain.strip():
            raise BrainRunError("stream domain must be a non-empty string")
        requested_approval = kwargs.pop("approve_provider_call", False)
        if not isinstance(requested_approval, bool):
            raise BrainRunError("stream approve_provider_call must be a boolean")
        execution_mode = kwargs.get("execution_mode", "provider")
        if execution_mode not in (None, "provider"):
            raise BrainRunError("run_stream supports only execution_mode='provider'")
        if kwargs.get("mission_policy") is not None or kwargs.get("tool_loop_options") is not None:
            raise BrainRunError(
                "run_stream cannot execute mission or tool-loop continuations; use run() for those modes"
            )
        if kwargs.get("learn") is True or kwargs.get("evaluator") is not None or kwargs.get("evidence") is not None:
            raise BrainRunError(
                "run_stream cannot settle evaluator learning; use run_learning() or run()"
            )

        run_options = self._prompt_learning_options(kwargs)
        candidates, resolved_credentials, resolved_options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=run_options,
            tool_domains=(domain,),
            task=task,
            resume_learning=False,
        )
        if execution_controller is not None:
            raise BrainRunError(
                "run_stream cannot attach a durable execution controller; use a resumable result controller"
            )

        # Compile the same blueprint used by the ordinary direct runner.  This second provider-
        # free compilation gives the stream handle a stable public plan digest without putting
        # transient task/prompt values into its completion receipt.
        prepare_options = {
            key: resolved_options[key]
            for key in (
                "capability",
                "risk_class",
                "constraints",
                "desired_outputs",
                "context",
                "max_steps",
                "require_json",
                "structured_domain_response",
                "response_schema",
                "execution_mode",
                "max_input_tokens",
                "required_model_capabilities",
                "memory_episodes",
            )
            if key in resolved_options
        }
        blueprint = self.orchestrator.prepare(task=task, domain=domain, **prepare_options)
        route_binding = blueprint.selection_context.get("autonomous_route")
        blueprint = _apply_versioned_prompt(
            blueprint,
            route=route_binding if isinstance(route_binding, Mapping) else None,
            prompt_template=resolved_options.get("prompt_template"),
            prompt_registry=resolved_options.get("prompt_registry"),
            prompt_selection=resolved_options.get("prompt_selection"),
            prompt_stage=resolved_options.get("prompt_stage", "answer"),
            prompt_learning_state=resolved_options.get("prompt_learning_state"),
            prompt_learning_exploration=resolved_options.get("prompt_learning_exploration", 0.35),
        )
        blueprint_public = blueprint.to_dict()
        blueprint_digest = content_digest(blueprint_public)
        task_digest = content_digest({"task": task})
        resolved_options["approve_provider_call"] = False
        try:
            preflight = self.orchestrator.run(
                task=task,
                domain=domain,
                model_candidates=candidates,
                credentials=resolved_credentials,
                **resolved_options,
            )
        except Exception:
            # Do not translate malformed/configuration errors into a stream receipt.  This is
            # the same hard-failure posture as the non-streaming API and avoids hiding contract
            # bugs behind a deferred iterator.
            raise
        if not isinstance(preflight, BrainRunResult):
            raise BrainRunError("run_stream preflight returned a non-direct result")

        route = blueprint.selection_context.get("autonomous_route")
        route_public = dict(route) if isinstance(route, Mapping) else None
        selected = preflight.selection.get("selected_model")
        if not isinstance(selected, Mapping):
            return AutonomousAgentStreamHandle(
                selection=preflight.selection,
                route=route_public,
                blueprint=blueprint_public,
                task_digest=task_digest,
                blueprint_digest=blueprint_digest,
                inner=None,
                initial_status="selection_refused",
            )
        provider = selected.get("provider")
        model = selected.get("model")
        if not isinstance(provider, str) or not isinstance(model, str):
            raise BrainRunError("run_stream preflight selected malformed provider metadata")
        if preflight.status != "approval_required":
            status = preflight.status if preflight.status in {"plan_refused", "route_review_required"} else "selection_refused"
            return AutonomousAgentStreamHandle(
                selection=preflight.selection,
                route=route_public,
                blueprint=blueprint_public,
                task_digest=task_digest,
                blueprint_digest=blueprint_digest,
                inner=None,
                initial_status=status,
            )
        if not requested_approval:
            return AutonomousAgentStreamHandle(
                selection=preflight.selection,
                route=route_public,
                blueprint=blueprint_public,
                task_digest=task_digest,
                blueprint_digest=blueprint_digest,
                inner=None,
                initial_status="approval_required",
            )

        raw_messages = preflight.prompt.get("messages")
        if not isinstance(raw_messages, Sequence) or isinstance(raw_messages, (str, bytes)) or not raw_messages:
            raise BrainRunError("run_stream preflight did not retain provider messages")
        normalized_content_parts = resolved_options.get("content_parts")
        if normalized_content_parts is not None:
            normalized_content_parts = normalize_provider_content_parts(normalized_content_parts)
        provider_messages = _provider_messages_with_content_parts(
            [dict(message) for message in raw_messages],
            () if normalized_content_parts is None else normalized_content_parts,
        )
        tools = resolved_options.get("provider_tools", ())
        if not isinstance(tools, Sequence) or isinstance(tools, (str, bytes)):
            raise BrainRunError("run_stream provider_tools must be a sequence")
        max_output_tokens = resolved_options.get("max_output_tokens", 2_048)
        if not isinstance(max_output_tokens, int) or isinstance(max_output_tokens, bool):
            raise BrainRunError("run_stream max_output_tokens must be an integer")
        max_provider_failovers = resolved_options.get("max_provider_failovers", 2)
        if (
            not isinstance(max_provider_failovers, int)
            or isinstance(max_provider_failovers, bool)
            or not 0 <= max_provider_failovers <= 8
        ):
            raise BrainRunError("run_stream max_provider_failovers must be within [0, 8]")
        request = build_autonomous_agent_stream_request(
            model=model,
            messages=provider_messages,
            max_output_tokens=max_output_tokens,
            temperature=resolved_options.get("temperature"),
            require_json=blueprint.spec.require_json,
            response_schema=blueprint.spec.response_schema,
            idempotency_key=resolved_options.get("idempotency_key"),
            tools=tools,
            tool_choice=resolved_options.get("tool_choice"),
        )
        selection_models = preflight.selection.get("models", ())
        if not isinstance(selection_models, Sequence) or isinstance(selection_models, (str, bytes)):
            selection_models = ()
        fallbacks: list[AutonomousStreamArm] = []
        seen = {f"{provider}/{model}"}
        for candidate in selection_models:
            if not isinstance(candidate, Mapping):
                continue
            fallback_provider = candidate.get("provider")
            fallback_model = candidate.get("model")
            if not isinstance(fallback_provider, str) or not isinstance(fallback_model, str):
                continue
            arm_id = f"{fallback_provider}/{fallback_model}"
            if arm_id in seen:
                continue
            if len(fallbacks) >= max_provider_failovers:
                break
            seen.add(arm_id)
            cost = candidate.get("cost_per_million_tokens", 0.0)
            if isinstance(cost, bool) or not isinstance(cost, (int, float)):
                cost = 0.0
            fallbacks.append(
                AutonomousStreamArm(
                    provider=fallback_provider,
                    model=fallback_model,
                    cost_per_million_tokens=float(cost),
                )
            )
        inner = AutonomousStreamRuntime(self.runtime).open(
            request,
            provider=provider,
            model=model,
            fallbacks=fallbacks,
            credential=resolved_credentials.get(provider),
            credential_for=lambda name: resolved_credentials.get(name),
            max_provider_failovers=max_provider_failovers,
            context_budget=resolved_options.get("context_budget"),
            observer=resolved_options.get("invocation_observer"),
            invocation_kind="autonomous_agent_stream",
            selection=preflight.selection,
            reserve_cost=resolved_options.get("reserve_cost"),
            authorization_context=resolved_options.get("authorization_context"),
            authorization_domain=domain,
        )
        return AutonomousAgentStreamHandle(
            selection=preflight.selection,
            route=route_public,
            blueprint=blueprint_public,
            task_digest=task_digest,
            blueprint_digest=blueprint_digest,
            inner=inner,
        )

    def run_auto_stream(
        self,
        *,
        task: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Route deterministically, then expose the selected single-domain stream.

        Provider-assisted semantic routing, provider planning, learning loops, and cross-domain
        fan-out are intentionally rejected until their separate stream lifecycle contracts are
        available.  This prevents an automatic stream from silently making an unreviewed second
        provider call or collapsing several child completions into one ambiguous transcript.
        """

        from .autonomous_agent_stream import AutonomousAgentStreamHandle

        if kwargs.get("semantic_routing") is True:
            raise BrainRunError("run_auto_stream requires provider-free routing; semantic routing is a separate provider boundary")
        if kwargs.get("planning_mode", "deterministic") != "deterministic":
            raise BrainRunError("run_auto_stream supports only deterministic planning")
        if any(
            kwargs.get(name) is True
            for name in (
                "workflow_execution",
                "workflow_learning",
                "workflow_trajectory_learning",
                "cross_domain_learning",
                "cross_domain_trajectory_learning",
                "cross_domain_replan_learning",
            )
        ) or kwargs.get("learn") is True:
            raise BrainRunError("run_auto_stream cannot combine automatic learning or workflow execution")
        route_options = self._automatic_route_options(kwargs)
        automatic = self.prepare_auto(task=task, **route_options)
        route = automatic.route
        task_digest = content_digest({"task": task})
        if route.abstained:
            return AutonomousAgentStreamHandle(
                selection={},
                route=route.to_dict(),
                blueprint=None,
                task_digest=task_digest,
                blueprint_digest=None,
                inner=None,
                initial_status="route_review_required",
            )
        if len(route.selected_domains) != 1:
            cross_blueprint = automatic.cross_domain_blueprint
            if cross_blueprint is None:
                raise BrainRunError("run_auto_stream route selected multiple domains without a cross-domain blueprint")
            subtasks = [
                {
                    "id": child_id,
                    "task": child.spec.task,
                    "domain": child.profile.domain,
                    "capability": child.spec.capability,
                    "risk_class": child.spec.risk_class,
                    "constraints": child.spec.constraints,
                    "desired_outputs": child.spec.desired_outputs,
                    "context": child.spec.context,
                    "max_steps": child.spec.max_steps,
                    "require_json": child.spec.require_json,
                    "structured_domain_response": child.spec.structured_domain_response,
                    "response_schema": child.spec.response_schema,
                    "execution_mode": "provider",
                    "required_model_capabilities": child.required_capabilities,
                }
                for child_id, child in zip(cross_blueprint.child_ids, cross_blueprint.child_blueprints)
            ]
            cross_options = dict(kwargs)
            for name in (
                "hints",
                "min_confidence",
                "min_margin",
                "max_domains",
                "allow_cross_domain",
                "semantic_routing",
                "semantic_weight",
                "planning_mode",
                "route_override",
            ):
                cross_options.pop(name, None)
            handle = self.run_cross_domain_stream(
                task=task,
                subtasks=subtasks,
                credentials=credentials,
                model_candidates=model_candidates,
                **cross_options,
            )
            if not hasattr(handle, "route"):
                raise BrainRunError("run_auto_stream cross-domain stream returned an invalid handle")
            handle.route = route.to_dict()
            return handle
        selected_domain = route.selected_domains[0]
        direct_options = dict(kwargs)
        for name in (
            "hints",
            "min_confidence",
            "min_margin",
            "max_domains",
            "allow_cross_domain",
            "semantic_routing",
            "semantic_weight",
            "planning_mode",
        ):
            direct_options.pop(name, None)
        handle = self.run_stream(
            task=task,
            domain=selected_domain,
            credentials=credentials,
            model_candidates=model_candidates,
            **direct_options,
        )
        if not isinstance(handle, AutonomousAgentStreamHandle):
            raise BrainRunError("run_auto_stream direct stream returned an invalid handle")
        handle.route = route.to_dict()
        return handle

    def run_cross_domain_stream(
        self,
        *,
        task: str,
        subtasks: Sequence[Mapping[str, Any]],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Stream bounded specialist fan-out followed by optional synthesis.

        The parent plan is compiled and run through the normal provider-free cross-domain
        preflight before a handle is returned.  Each child then uses :meth:`run_stream`, so
        model eligibility, prompt assembly, provider failover, caller approval, and secret
        redaction remain identical to a direct stream.  Child and synthesis output is live-only:
        the fan-in buffer is bounded, and only metadata-only completion receipts survive.

        This synchronous façade intentionally does not combine durable execution checkpoints,
        evaluator settlement, missions, tool loops, or provider-assisted planning with a live
        transcript.  Those paths already have explicit resumable APIs; replaying a partial stream
        would risk duplicating model output or effects.
        """

        from .autonomous_agent_stream import (
            AutonomousCrossDomainStreamHandle,
            MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN,
        )

        if execution_id is not None or resume_execution:
            raise BrainRunError(
                "run_cross_domain_stream does not support execution persistence; use a resumable cross-domain controller"
            )
        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("cross-domain stream task must be a non-empty string")
        if not isinstance(subtasks, Sequence) or isinstance(subtasks, (str, bytes)) or not subtasks:
            raise BrainRunError("cross-domain stream subtasks must be a non-empty sequence")
        if len(subtasks) > MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN:
            raise BrainRunError("cross-domain stream exceeds its bounded child count")
        if any(not isinstance(item, Mapping) for item in subtasks):
            raise BrainRunError("cross-domain stream subtasks must contain mappings")

        requested_approval = kwargs.pop("approve_provider_call", False)
        if not isinstance(requested_approval, bool):
            raise BrainRunError("cross-domain stream approve_provider_call must be a boolean")
        child_execution_mode = kwargs.pop("child_execution_mode", "provider")
        synthesis_execution_mode = kwargs.pop("synthesis_execution_mode", "provider")
        if child_execution_mode not in (None, "provider") or synthesis_execution_mode not in (None, "provider"):
            raise BrainRunError("cross-domain streaming supports only provider child and synthesis modes")
        synthesize = kwargs.pop("synthesize", True)
        allow_partial = kwargs.pop("allow_partial", False)
        max_parallelism = kwargs.pop("max_parallelism", 1)
        if not isinstance(synthesize, bool) or not isinstance(allow_partial, bool):
            raise BrainRunError("cross-domain stream synthesize and allow_partial must be booleans")
        if (
            isinstance(max_parallelism, bool)
            or not isinstance(max_parallelism, int)
            or not 1 <= max_parallelism <= MAX_AUTONOMOUS_CROSS_DOMAIN_STREAM_CHILDREN
        ):
            raise BrainRunError("cross-domain stream max_parallelism must be between 1 and 8")
        if kwargs.get("mission_policy") is not None or kwargs.get("tool_loop_options") is not None:
            raise BrainRunError(
                "cross-domain streaming cannot execute missions or tool loops; use run_cross_domain()"
            )
        if kwargs.get("learn") is True or kwargs.get("evaluator") is not None or kwargs.get("evidence") is not None:
            raise BrainRunError(
                "cross-domain streaming cannot settle evaluator learning; use run_cross_domain_learning()"
            )
        if kwargs.get("execution_controller") is not None:
            raise BrainRunError(
                "cross-domain streaming cannot attach a durable execution controller"
            )
        if kwargs.get("semantic_routing") is True:
            raise BrainRunError(
                "cross-domain streaming requires an explicit reviewed subtask route; use prepare_auto() for routing"
            )
        if kwargs.get("auto_route") is True:
            raise BrainRunError("cross-domain streaming does not run automatic route resolution")
        if kwargs.get("execution_mode") not in (None, "provider"):
            raise BrainRunError("cross-domain streaming supports only execution_mode='provider'")

        run_options = self._prompt_learning_options(kwargs)
        child_domains = tuple(
            item.get("domain") for item in subtasks if isinstance(item.get("domain"), str)
        )
        if len(child_domains) != len(subtasks):
            raise BrainRunError("cross-domain stream every subtask requires a string domain")
        candidates, resolved_credentials, resolved_options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=run_options,
            tool_domains=(*child_domains, "cross_domain"),
            task=task,
            resume_learning=False,
        )
        if execution_controller is not None:
            raise BrainRunError(
                "cross-domain streaming cannot attach a durable execution controller; use a resumable controller"
            )

        # Keep only options accepted by the canonical cross-domain runner.  `_execution_inputs`
        # may add reviewed ledger/memory/tool context, while unsupported façade-only flags must
        # never leak into the lower-level call as an accidental override.
        cross_domain_keys = {
            "context",
            "content_parts",
            "prompt_template",
            "prompt_registry",
            "prompt_selection",
            "prompt_stage",
            "prompt_learning_state",
            "prompt_learning_exploration",
            "execution_plan_context",
            "desired_outputs",
            "max_steps",
            "require_json",
            "structured_domain_response",
            "response_schema",
            "ledger",
            "memory",
            "memory_query",
            "memory_limit",
            "memory_consolidator",
            "memory_lesson_resolver",
            "memory_lesson_context_resolver",
            "consolidated_memory_limit",
            "retrieve_consolidated_memory",
            "consolidated_memory_required",
            "contextual_observations",
            "input_tokens",
            "requested_output_tokens",
            "max_cost_per_million_tokens",
            "max_latency_ms",
            "min_quality",
            "selection_overrides",
            "selection_weights",
            "selection_observations",
            "approve_mission_dispatch",
            "run_id",
            "max_output_tokens",
            "temperature",
            "idempotency_key",
            "mission_policy",
            "mission_options",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "provider_tools",
            "tool_choice",
            "max_provider_failovers",
            "domain_policy_mode",
            "domain_policy_evidence_ready",
            "domain_policy_evaluator_configured",
            "domain_policy_plan_accepted",
            "domain_policy_effects_requested",
            "domain_policy_effects_approved",
            "bandit_state",
            "accepted_plan_refinement",
            "response_alignments",
            "require_response_alignment",
            "minimum_response_reward",
            "minimum_response_alignment_confidence",
            "response_contradiction_confidence_threshold",
            "invocation_observer",
            "trace_event_callback",
        }
        preflight_options = {
            key: resolved_options[key]
            for key in cross_domain_keys
            if key in resolved_options
        }
        preflight_options.update({
            "child_execution_mode": "provider",
            "synthesis_execution_mode": "provider",
            "approve_provider_call": False,
            "approve_mission_dispatch": False,
            "synthesize": synthesize,
            "allow_partial": allow_partial,
            "max_parallelism": max_parallelism,
        })
        preflight = self.orchestrator.run_cross_domain(
            task=task,
            subtasks=subtasks,
            model_candidates=candidates,
            credentials=resolved_credentials,
            **preflight_options,
        )
        if not isinstance(preflight, AutonomousCrossDomainResult):
            raise BrainRunError("cross-domain stream preflight returned an invalid result")
        blueprint = preflight.blueprint
        blueprint_public = blueprint.to_dict()
        blueprint_digest = content_digest(blueprint_public)
        task_digest = content_digest({"task": task})
        initial_failure_statuses = {
            "route_review_required",
            "plan_refused",
            "selection_refused",
            "response_review_required",
            "reconciliation_required",
            "provider_failed",
            "child_failed",
            "child_incomplete",
            "children_completed",
            "children_partial",
        }
        if preflight.status != "approval_required":
            status = preflight.status if preflight.status in initial_failure_statuses else "plan_refused"
            return AutonomousCrossDomainStreamHandle(
                selection={
                    "strategy": "cross_domain_preflight",
                    "child_count": len(blueprint.child_ids),
                    "retention": "selection_metadata_only",
                },
                route=None,
                blueprint=blueprint_public,
                task_digest=task_digest,
                blueprint_digest=blueprint_digest,
                child_specs=(),
                synthesis_spec={},
                open_stream=self.run_stream,
                model_candidates=candidates,
                credentials=resolved_credentials,
                base_options={},
                synthesize=synthesize,
                allow_partial=allow_partial,
                max_parallelism=max_parallelism,
                initial_status=status,
            )
        if not requested_approval:
            return AutonomousCrossDomainStreamHandle(
                selection={
                    "strategy": "cross_domain_preflight",
                    "child_count": len(blueprint.child_ids),
                    "retention": "selection_metadata_only",
                },
                route=None,
                blueprint=blueprint_public,
                task_digest=task_digest,
                blueprint_digest=blueprint_digest,
                child_specs=(),
                synthesis_spec={},
                open_stream=self.run_stream,
                model_candidates=candidates,
                credentials=resolved_credentials,
                base_options={},
                synthesize=synthesize,
                allow_partial=allow_partial,
                max_parallelism=max_parallelism,
                initial_status="approval_required",
            )

        def stream_spec(item: AutonomousTaskBlueprint, item_id: str) -> dict[str, Any]:
            return {
                "id": item_id,
                "task": item.spec.task,
                "domain": item.profile.domain,
                "capability": item.spec.capability,
                "risk_class": item.spec.risk_class,
                "constraints": item.spec.constraints,
                "desired_outputs": item.spec.desired_outputs,
                "context": dict(item.spec.context),
                "max_steps": item.spec.max_steps,
                "require_json": item.spec.require_json,
                "structured_domain_response": item.spec.structured_domain_response,
                "response_schema": item.spec.response_schema,
                "required_model_capabilities": item.required_capabilities,
            }

        child_specs = tuple(
            stream_spec(child, child_id)
            for child_id, child in zip(blueprint.child_ids, blueprint.child_blueprints)
        )
        synthesis_spec = stream_spec(blueprint.synthesis_blueprint, "synthesis")
        # The worker receives caller options, not the preflight-enriched context.  Each child
        # must rebuild its own domain execution-plan/tool projection; copying the parent reserved
        # contract would be rejected as a stale override by `_execution_inputs`.
        stream_base_options = dict(run_options)
        stream_base_options.update({
            "child_execution_mode": "provider",
            "synthesis_execution_mode": "provider",
        })
        return AutonomousCrossDomainStreamHandle(
            selection={
                "strategy": "cross_domain_child_streams",
                "child_count": len(child_specs),
                "child_ids": [spec["id"] for spec in child_specs],
                "max_parallelism": max_parallelism,
                "synthesis_enabled": synthesize,
                "retention": "selection_metadata_only;child_choices_transient",
                "secret_material": "never_returned",
            },
            route=None,
            blueprint=blueprint_public,
            task_digest=task_digest,
            blueprint_digest=blueprint_digest,
            child_specs=child_specs,
            synthesis_spec=synthesis_spec,
            open_stream=self.run_stream,
            model_candidates=candidates,
            credentials=resolved_credentials,
            base_options=stream_base_options,
            synthesize=synthesize,
            allow_partial=allow_partial,
            max_parallelism=max_parallelism,
        )

    def run_mission_replan_cycle(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        mission_policy: MissionPolicy | Mapping[str, Any],
        evaluator: BrainOutcomeEvaluator,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        evidence: Mapping[str, Any] | None = None,
        max_replans: int = 1,
        root_mission_id: str | None = None,
        state_store: AutonomousMissionReplanStateStore | None = None,
        resume: bool = False,
        rehydrate_result: Callable[[AutonomousMissionReplanRehydrationContext], BrainMissionResult] | None = None,
        rehydrate_instruction: Callable[[AutonomousMissionReplanRehydrationContext], str] | None = None,
        checkpoint_sink: Callable[[AutonomousMissionReplanCheckpoint], Any] | None = None,
        mission_options: Mapping[str, Any] | None = None,
        **kwargs: Any,
    ) -> AutonomousMissionReplanResult:
        """Run a domain mission through evaluator-guided, restart-safe replanning.

        This is the application façade for the durable mission kernel.  It compiles the reviewed
        domain prompt and plan without contacting a provider, resolves only caller-owned opaque
        credential handles, then delegates provider selection, invocation, mission authorization,
        evaluator credit, and metadata checkpointing to the existing brain boundaries.  Retry
        feedback is transient prompt context; it cannot grant tools, credentials, dispatch, or
        external effects.
        """

        if not isinstance(evaluator, BrainOutcomeEvaluator):
            raise BrainRunError("mission replan evaluator must be a BrainOutcomeEvaluator")
        if mission_options is not None and not isinstance(mission_options, Mapping):
            raise BrainRunError("mission_options must be a mapping or None")
        options = dict(kwargs)
        if mission_options is not None:
            overlap = sorted(set(options).intersection(mission_options))
            if overlap:
                raise BrainRunError("mission_options duplicates runtime options: " + ", ".join(overlap))
            options.update(dict(mission_options))
        options = self._prompt_learning_options(options)
        supplied_execution_mode = options.get("execution_mode")
        if supplied_execution_mode not in (None, "mission"):
            raise BrainRunError("run_mission_replan_cycle requires execution_mode='mission'")
        options["execution_mode"] = "mission"
        resolved_bandit_state = self.learning_state() if bandit_state is None else bandit_state

        candidates, resolved_credentials, resolved_options, execution_controller = self._execution_inputs(
            credentials=credentials,
            model_candidates=model_candidates,
            options=options,
            tool_domains=(domain,),
            task=task,
            resume_learning=False,
        )
        prepare_options = {
            key: resolved_options[key]
            for key in (
                "capability",
                "risk_class",
                "constraints",
                "desired_outputs",
                "context",
                "max_steps",
                "require_json",
                "structured_domain_response",
                "response_schema",
                "required_model_capabilities",
                "memory_episodes",
                "memory_lesson_references",
            )
            if key in resolved_options
        }
        prepare_options["execution_mode"] = "mission"
        blueprint = self.orchestrator.prepare(task=task, domain=domain, **prepare_options)
        route_binding = blueprint.selection_context.get("autonomous_route")
        blueprint = _apply_versioned_prompt(
            blueprint,
            route=route_binding if isinstance(route_binding, Mapping) else None,
            prompt_template=options.get("prompt_template"),
            prompt_registry=options.get("prompt_registry"),
            prompt_selection=options.get("prompt_selection"),
            prompt_stage=options.get("prompt_stage", "answer"),
            prompt_learning_state=options.get("prompt_learning_state"),
            prompt_learning_exploration=options.get("prompt_learning_exploration", 0.35),
        )

        # `_execution_inputs` enriches the ordinary agent request with reviewed tool/catalogue
        # context. Filter that request down to the explicit adaptive-mission kernel contract so
        # orchestration-only keys can never leak into a lower-level call as an implicit override.
        adaptive_keys = {
            "context",
            "content_parts",
            "contextual_observations",
            "input_tokens",
            "requested_output_tokens",
            "max_cost_per_million_tokens",
            "max_latency_ms",
            "min_quality",
            "selection_overrides",
            "approve_provider_call",
            "approve_mission_dispatch",
            "run_id",
            "max_output_tokens",
            "temperature",
            "response_schema",
            "idempotency_key",
            "claim_requests",
            "evaluator_review",
            "workflow_binding",
            "route_review",
            "operations_gate_acceptance",
            "route_request",
            "enforce_route_tools",
            "require_resolved_route",
            "provider_tools",
            "tool_choice",
            "max_provider_failovers",
            "reserve_cost",
        }
        adaptive_options = {
            key: resolved_options[key]
            for key in adaptive_keys
            if key in resolved_options
        }
        adaptive_options["required_capabilities"] = blueprint.required_capabilities
        adaptive_options["response_schema"] = (
            adaptive_options.get("response_schema") or blueprint.spec.response_schema
        )

        def record_mission_model_quality(
            result: BrainMissionResult,
            decision: BrainEvaluatorDecision,
        ) -> Mapping[str, Any]:
            """Record only evaluator-derived model quality; never retain the live result."""

            try:
                episode = self.brain.prepare_learning_episode(result, evidence=evidence)
                quality = self._record_model_quality_feedback(episode, decision)
                coordinator = self.prompt_learning_coordinator
                if coordinator is not None:
                    selections = extract_autonomous_prompt_learning_selections(
                        {"prompt": dict(result.brain_run.prompt)},
                        coordinator.registry,
                    )
                    quality = {
                        **quality,
                        "prompt_learning": {
                            "selection_count": len(selections),
                            "selection_digests": [selection.selection_digest for selection in selections],
                            "selections": [selection.to_dict() for selection in selections],
                            "retention": "selection_metadata_only;rendered_messages_transient",
                            "secret_material": "never_returned",
                        },
                    }
                return quality
            except Exception as error:
                return {
                    "status": "failed",
                    "error_class": type(error).__name__,
                    "retention": "metadata_only_model_quality_no_payloads",
                    "secret_material": "never_returned",
                }

        try:
            result = run_autonomous_mission_replan_cycle(
                self.brain,
                task=task,
                model_candidates=candidates,
                prompt=blueprint.prompt,
                plan=blueprint.plan,
                credentials=resolved_credentials,
                mission_policy=mission_policy,
                evaluator=evaluator,
                bandit_state=resolved_bandit_state,
                evidence=evidence,
                ledger=self.ledger,
                mission_options=adaptive_options,
                max_replans=max_replans,
                root_mission_id=root_mission_id,
                state_store=state_store,
                resume=resume,
                rehydrate_result=rehydrate_result,
                rehydrate_instruction=rehydrate_instruction,
                checkpoint_sink=checkpoint_sink,
                execution_controller=execution_controller,
                invocation_observer=resolved_options.get("invocation_observer"),
                trace_event_callback=resolved_options.get("trace_event_callback"),
                model_quality_callback=record_mission_model_quality,
            )
        except Exception as error:
            self._finish_execution(execution_controller, error=error)
            raise
        self._finish_execution(execution_controller, result=result)
        return result

    def run_with_launch_admission(
        self,
        *,
        task: str,
        domain: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run one domain only when a caller-owned launch admission covers it.

        The admission check happens before credential resolution and orchestration.  It is an
        additional launch gate, not a replacement for provider, tool, learner, or effect approval
        supplied through ``kwargs``.
        """

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (domain,))
        return self.run(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_auto_with_launch_admission(
        self,
        *,
        task: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        **kwargs: Any,
    ) -> AutonomousAutoResult:
        """Route automatically, enforce admission, then enter the ordinary automatic runtime.

        The first route compilation is provider-free and is the route whose selected domains are
        checked.  Provider-assisted semantic routing is intentionally rejected here because its
        classifier call would occur before a final domain-scoped admission; callers can admit that
        separate boundary explicitly through the semantic-routing APIs.
        """

        self.authorize_auto_launch_admission(
            task=task,
            launch_admission=launch_admission,
            **kwargs,
        )
        return self.run_auto(task=task, credentials=credentials, **kwargs)

    def authorize_auto_launch_admission(
        self,
        *,
        task: str,
        launch_admission: Mapping[str, Any],
        **kwargs: Any,
    ) -> dict[str, Any]:
        """Compile one automatic route offline and return its verified admission metadata.

        This is the provider-free process-boundary counterpart to
        :meth:`run_auto_with_launch_admission`.  Hosts can call it before collecting a
        short-lived credential, opening MCP, or constructing a provider client.  The same route
        controls are accepted by the execution wrapper so a caller can use one exact policy at
        both the preview and dispatch boundaries.
        """

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        semantic_routing = kwargs.get("semantic_routing", False)
        if not isinstance(semantic_routing, bool):
            raise BrainRunError("semantic_routing must be a boolean")
        if semantic_routing:
            raise BrainRunError(
                "launch-admitted automatic execution requires provider-free routing; "
                "admit semantic routing separately before enabling it"
            )
        route_options = self._automatic_route_options(kwargs)
        blueprint = self.prepare_auto(task=task, **route_options)
        return authorize_autonomous_launch_domains(launch_admission, blueprint.route.selected_domains)

    @staticmethod
    def _automatic_route_options(options: Mapping[str, Any]) -> dict[str, Any]:
        """Keep provider-free automatic route compilation aligned across launch gates."""

        return {
            key: options[key]
            for key in (
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
            )
            if key in options
        }

    def run_auto_replan_cycle_with_launch_admission(
        self,
        *,
        task: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        evaluator: BrainOutcomeEvaluator | DomainEvaluatorRegistry,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        max_replans: int = 1,
        hints: Sequence[str] = (),
        min_confidence: float = 0.25,
        min_margin: float = 0.10,
        max_domains: int = 3,
        allow_cross_domain: bool = True,
        **kwargs: Any,
    ) -> AutonomousAutoReplanResult:
        """Run the automatic learning/replan loop behind a process-boundary launch gate.

        Routing is compiled once without a provider, the approved route is passed as an exact
        route override, and every evaluator retry reuses that route.  Provider-assisted semantic
        routing is rejected because its classifier is a separate provider boundary that must be
        admitted independently.  Route abstention returns a review result without consuming the
        launch admission or any credential.
        """

        semantic_routing = kwargs.pop("semantic_routing", False)
        if not isinstance(semantic_routing, bool):
            raise BrainRunError("semantic_routing must be a boolean")
        if semantic_routing:
            raise BrainRunError(
                "launch-admitted automatic replan execution requires provider-free routing; "
                "admit semantic routing separately before enabling it"
            )
        if not isinstance(evaluator, (BrainOutcomeEvaluator, DomainEvaluatorRegistry)):
            raise BrainRunError(
                "automatic replan evaluator must be a BrainOutcomeEvaluator or DomainEvaluatorRegistry"
            )
        if (
            isinstance(max_replans, bool)
            or not isinstance(max_replans, int)
            or not 0 <= max_replans <= MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS
        ):
            raise BrainRunError(
                f"automatic replan max_replans must be within [0, {MAX_AUTONOMOUS_REPLAN_CYCLE_REPLANS}]"
            )
        if "route_override" in kwargs:
            raise BrainRunError(
                "run_auto_replan_cycle_with_launch_admission owns the route override; "
                "pass routing controls instead"
            )

        route_options = self._automatic_route_options(kwargs)
        route_options.update(
            {
                "hints": hints,
                "min_confidence": min_confidence,
                "min_margin": min_margin,
                "max_domains": max_domains,
                "allow_cross_domain": allow_cross_domain,
            }
        )
        blueprint = self.prepare_auto(task=task, **route_options)
        route = blueprint.route
        if route.abstained:
            return AutonomousAutoReplanResult(
                status="route_review_required",
                mode=None,
                route=route,
                final=AutonomousAutoResult(status="route_review_required", route=route),
            )

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, route.selected_domains)
        return self.run_auto_replan_cycle(
            task=task,
            credentials=credentials,
            evaluator=evaluator,
            model_candidates=model_candidates,
            max_replans=max_replans,
            route_override=route,
            hints=hints,
            min_confidence=min_confidence,
            min_margin=min_margin,
            max_domains=max_domains,
            allow_cross_domain=allow_cross_domain,
            semantic_routing=False,
            **kwargs,
        )

    def run_learning(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run one application task through explicit evaluator-backed online learning.

        This is the façade-level counterpart to ``AutonomousBrain.run_learning``. It preserves
        the ordinary agent admission, provider, prompt, planning, tool, effect, and execution
        controller boundaries, while forcing the orchestrator's explicit learning mode. The
        caller still supplies the evaluator and evidence; provider transport success never earns
        reward. Cross-domain callers should use ``run_cross_domain_learning`` so delayed credit
        remains trajectory-scoped.
        """

        options = dict(kwargs)
        options["learn"] = True
        return self.run(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **options,
        )

    def run_learning_with_launch_admission(
        self,
        *,
        task: str,
        domain: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run learning only after the requested domain passes the launch gate.

        Learning changes policy state and therefore uses the same process-boundary admission as
        ordinary execution.  The check is deliberately before the learner, credential resolver,
        provider router, and execution controller are constructed.
        """

        _authorize_launch_admission_domains(launch_admission, (domain,))
        return self.run_learning(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentRunsMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
