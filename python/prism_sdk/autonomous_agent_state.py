"""Focused autonomous-agent state methods."""

from __future__ import annotations

from .autonomy import (
    AUTONOMOUS_DOMAINS,
    Any,
    ArgumentError,
    AutonomousExecutionController,
    AutonomousExecutionPolicy,
    AutonomousToolSelectionSnapshot,
    AutonomyPersistenceError,
    BrainRunError,
    CredentialHandle,
    CredentialSession,
    MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TASK_BYTES,
    Mapping,
    ModelCandidate,
    Sequence,
    _AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY,
    _AUTONOMOUS_CAPABILITY_PORTFOLIO_CONTEXT_KEY,
    _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
    _AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY,
    _build_domain_capability_contracts,
    _identifier,
    _resolve_domain_capability_contract,
    _text,
    _update_autonomous_tool_selection_state,
    builtin_autonomous_domain_tool_profiles,
    content_digest,
    normalize_autonomous_tool_selection_state,
    uuid,
)

class AutonomousAgentStateMixin:
    def execution_state(self, execution_id: str) -> dict[str, Any] | None:
        """Read one restart-safe execution state without returning task or provider content."""

        if self.execution_journal is None:
            return None
        state = self.execution_journal.state(execution_id)
        return None if state is None else state.to_dict()

    def execution_events(
        self,
        execution_id: str,
        *,
        after_sequence: int = 0,
        limit: int = 256,
    ) -> list[dict[str, Any]]:
        """Read hash-verified metadata events for an execution."""

        if self.execution_journal is None:
            return []
        return [dict(row) for row in self.execution_journal.events(execution_id=execution_id, after_sequence=after_sequence, limit=limit)]

    def restore_execution_persistence(self) -> dict[str, Any]:
        """Restore the metadata-only execution checkpoint used by long-horizon recovery."""

        if self.execution_persistence is None:
            raise BrainRunError("restore_execution_persistence requires configured persistence")
        snapshot = self.execution_persistence.restore()
        if snapshot is None:
            return {"restored": False, "snapshot_digest": None, "events": 0, "retention": "metadata_only"}
        rows = snapshot.get("rows")
        return {
            "restored": True,
            "schema": snapshot.get("schema"),
            "snapshot_digest": snapshot.get("snapshot_digest"),
            "head_digest": snapshot.get("head_digest"),
            "events": len(rows) if isinstance(rows, list) else 0,
            "retention": "metadata_only_hash_chained",
        }

    def flush_execution_persistence(self) -> dict[str, Any]:
        """Flush the execution journal while projecting only checkpoint metadata."""

        if self.execution_persistence is None:
            raise BrainRunError("flush_execution_persistence requires configured persistence")
        snapshot = self.execution_persistence.flush()
        rows = snapshot.get("rows")
        return {
            "schema": snapshot.get("schema"),
            "snapshot_digest": snapshot.get("snapshot_digest"),
            "head_digest": snapshot.get("head_digest"),
            "events": len(rows) if isinstance(rows, list) else 0,
            "retention": "metadata_only_hash_chained",
        }

    def _resolve_candidates(
        self,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None,
        *,
        allow_empty: bool = False,
    ) -> list[dict[str, Any]]:
        candidates = self.catalogue.candidates() if model_candidates is None else [
            candidate.to_dict()
            if isinstance(candidate, ModelCandidate)
            else ModelCandidate.from_mapping(candidate).to_dict()
            for candidate in model_candidates
        ]
        if not candidates and not allow_empty:
            raise BrainRunError("the autonomous agent has no model candidates")
        return candidates

    def _execution_inputs(
        self,
        *,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None,
        options: Mapping[str, Any],
        tool_domains: Sequence[str] = (),
        task: str | None = None,
        resume_learning: bool = False,
        attach_execution_plan_context: bool = True,
        execution_id: str | None = None,
        resume_execution: bool = False,
    ) -> tuple[list[dict[str, Any]], dict[str, CredentialHandle], dict[str, Any], AutonomousExecutionController | None]:
        self._assert_selection_promotion_admitted()
        resolved_credentials = self._credential_mapping(credentials)
        resolved_candidates = self._resolve_candidates(model_candidates)
        resolved_options = dict(options)
        task_text = None if task is None else _text(
            "execution task",
            task,
            maximum=MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TASK_BYTES,
        )
        capability_focus = resolved_options.pop("_aurora_capability_focus", None)
        capability_contract = resolved_options.pop("_aurora_capability_contract", None)
        tool_learning_state = resolved_options.pop("tool_learning_state", resolved_options.pop("toolSelectionState", None))
        if tool_learning_state is None and self._tool_selection_configured:
            tool_learning_state = self.tool_selection_state
        tool_selection_exploration = resolved_options.pop("tool_selection_exploration", resolved_options.pop("toolSelectionExploration", 0.15))
        max_tool_risk_class = resolved_options.pop("max_tool_risk_class", resolved_options.pop("maxToolRiskClass", None))
        if capability_focus is not None:
            capability_focus = _identifier("capability focus", capability_focus)
            if not tool_domains:
                raise BrainRunError("capability focus requires a domain execution scope")
            if "provider_tools" in resolved_options:
                raise BrainRunError(
                    "provider_tools cannot override capability-scoped adapter selection"
                )
        if capability_contract is not None and not isinstance(capability_contract, Mapping):
            raise BrainRunError("capability contract must be a mapping")
        for reserved_key in (
            _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
            _AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY,
            _AUTONOMOUS_CAPABILITY_PORTFOLIO_CONTEXT_KEY,
            _AUTONOMOUS_WORKFLOW_STAGE_PLAN_CONTEXT_KEY,
        ):
            caller_context = resolved_options.get("context")
            if isinstance(caller_context, Mapping) and reserved_key in caller_context:
                raise BrainRunError("context cannot override an autonomous runtime contract")
        if not isinstance(resume_execution, bool):
            raise BrainRunError("resume_execution must be a boolean")
        resolved_options.setdefault("ledger", self.ledger)
        resolved_options.setdefault("memory", self.memory)
        activation_state = self.activation.state
        activation_guarded = activation_state.status == "revoked" or activation_state.plan_digest is not None
        if activation_guarded and "provider_tools" in resolved_options:
            raise BrainRunError(
                "provider_tools cannot bypass the activation-approved domain tool set"
            )
        if self.tool_registry is not None and "provider_tools" not in resolved_options:
            selected_tools = self.tool_registry.tools_for(tool_domains or None)
            reviewed_tool_bindings = {
                (binding.name, binding.capability)
                for profile in builtin_autonomous_domain_tool_profiles()
                for binding in profile.bindings
            }
            custom_tool_names = {
                tool.name
                for tool in selected_tools
                if (tool.name, tool.capability) not in reviewed_tool_bindings
            }
            if activation_state.status == "revoked":
                selected_tools = ()
            elif activation_state.plan_digest is not None:
                approved = set(activation_state.approved_tools)
                selected_tools = tuple(tool for tool in selected_tools if tool.name in approved)
            portfolio_packet = None
            if task_text is not None and tool_domains:
                portfolio_packet = self.capability_portfolio(
                    task_text,
                    domains=tuple(dict.fromkeys(tool_domains)),
                    capability=capability_focus,
                    tool_learning_state=tool_learning_state,
                    exploration=tool_selection_exploration,
                    max_risk_class=max_tool_risk_class,
                )
                selected_names = set(portfolio_packet["selected_tool_names"])
                # A caller-owned binding may intentionally describe a capability that is not
                # present in the reviewed built-in pack. Keep those explicitly registered tools
                # visible to the provider; the runtime still applies schema validation and the
                # separate read-only/effect approval gate at execution time.
                if capability_focus is None:
                    selected_names.update(custom_tool_names)
                # A non-empty portfolio is an explicit narrowing decision, including the
                # learned-disabled case. Preserve only caller-registered compatibility tools;
                # never silently restore every reviewed tool when the adaptive planner selects
                # an empty set.
                selected_tools = tuple(
                    tool for tool in selected_tools
                    if tool.name in selected_names or tool.name in custom_tool_names
                )
            pack_capabilities: set[str] = set()
            for domain in tool_domains:
                if domain in AUTONOMOUS_DOMAINS:
                    profile = self.orchestrator.registry.resolve(domain)
                    pack = self.orchestrator.pack_registry.resolve(domain)
                    workflow = self.orchestrator.workflow_registry.resolve(domain)
                    pack_capabilities.update(pack.tool_capabilities)
                    for contract in _build_domain_capability_contracts(profile, pack, workflow):
                        pack_capabilities.update(contract.tool_capabilities)
            pack_tools = tuple(
                tool for tool in selected_tools
                if tool.capability in pack_capabilities
            )
            caller_tools = tuple(
                tool for tool in selected_tools
                if tool.name in custom_tool_names and tool.capability not in pack_capabilities
            )
            if capability_focus is not None:
                focused_tools: list[AutonomousDomainTool] = []
                for domain in tool_domains:
                    if domain not in AUTONOMOUS_DOMAINS:
                        continue
                    profile = self.orchestrator.registry.resolve(domain)
                    pack = self.orchestrator.pack_registry.resolve(domain)
                    workflow = self.orchestrator.workflow_registry.resolve(domain)
                    contract = _resolve_domain_capability_contract(
                        profile,
                        pack,
                        workflow,
                        capability_focus,
                    )
                    focused_tools.extend(
                        tool for tool in selected_tools
                        if tool.capability in contract.tool_capabilities
                    )
                pack_tools = tuple(focused_tools)
            # A caller may register an intentionally application-specific capability that is not
            # in the reviewed built-in pack. Keep it visible rather than silently dropping it;
            # pack matching is a narrowing aid when there is at least one reviewed match, never
            # an authorization mechanism or a way to hide caller-registered tools.
            provider_surface = (
                pack_tools
                if capability_focus is not None
                else (*pack_tools, *caller_tools)
            )
            resolved_options["provider_tools"] = tuple(
                tool.to_provider_tool() for tool in provider_surface
            )
            if portfolio_packet is not None:
                caller_context = resolved_options.get("context")
                if caller_context is None:
                    portfolio_context: dict[str, Any] = {}
                elif isinstance(caller_context, Mapping):
                    portfolio_context = dict(caller_context)
                else:
                    raise BrainRunError("context must be a mapping or None")
                portfolio_context[_AUTONOMOUS_CAPABILITY_PORTFOLIO_CONTEXT_KEY] = {
                    "schema": portfolio_packet["schema"],
                    "plan_digest": portfolio_packet["plan_digest"],
                    "catalogue_digest": portfolio_packet["catalogue_digest"],
                    "profile_digest": portfolio_packet["profile_digest"],
                    "domains": list(portfolio_packet["domains"]),
                    "selected_tool_names": list(portfolio_packet["selected_tool_names"]),
                    "selected_tool_order": list(portfolio_packet["selected_tool_order"]),
                    "selection_learning": dict(portfolio_packet["selection_learning"]),
                    "execution": portfolio_packet["execution"],
                    "authority_posture": portfolio_packet["authorization"],
                }
                resolved_options["context"] = portfolio_context
        plan_domains = tuple(
            dict.fromkeys(domain for domain in tool_domains if domain in AUTONOMOUS_DOMAINS)
        )
        if plan_domains:
            execution_plan_packet = self.execution_plans(
                plan_domains,
                model_candidates=resolved_candidates,
            )
            if attach_execution_plan_context:
                caller_context = resolved_options.get("context")
                if caller_context is None:
                    merged_context: dict[str, Any] = {}
                elif isinstance(caller_context, Mapping):
                    if _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY in caller_context:
                        raise BrainRunError("context cannot override the autonomous execution plan")
                    merged_context = dict(caller_context)
                else:
                    raise BrainRunError("context must be a mapping or None")
                merged_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = execution_plan_packet
                resolved_options["context"] = merged_context
            else:
                resolved_options["execution_plan_context"] = execution_plan_packet
            if capability_focus is not None:
                if not isinstance(capability_contract, Mapping):
                    raise BrainRunError("capability focus requires its reviewed capability contract")
                capability_rows = execution_plan_packet["plans"][0].get("capabilities", {}).get("contracts", [])
                matching = next(
                    (
                        row for row in capability_rows
                        if isinstance(row, Mapping) and row.get("capability") == capability_focus
                    ),
                    None,
                )
                if not isinstance(matching, Mapping) or dict(matching.get("contract", {})) != dict(capability_contract):
                    raise BrainRunError("capability contract is stale or does not match the execution plan")
                caller_context = resolved_options.get("context")
                merged_context = {} if caller_context is None else dict(caller_context)
                merged_context[_AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY] = dict(capability_contract)
                resolved_options["context"] = merged_context
            selection_overrides = resolved_options.get("selection_overrides")
            if selection_overrides is None:
                merged_overrides: dict[str, Any] = {}
            elif isinstance(selection_overrides, Mapping):
                merged_overrides = dict(selection_overrides)
            else:
                raise BrainRunError("selection_overrides must be a mapping or None")
            merged_overrides["autonomy_execution_plan_digest"] = content_digest(execution_plan_packet["plans"])
            merged_overrides["autonomy_execution_plan_statuses"] = {
                plan["domain"]: plan["status"]
                for plan in execution_plan_packet["plans"]
            }
            if capability_focus is not None:
                merged_overrides["autonomy_capability_focus"] = capability_focus
                merged_overrides["autonomy_capability_contract_digest"] = capability_contract.get("contract_digest")
            resolved_options["selection_overrides"] = merged_overrides
        execution_controller: AutonomousExecutionController | None = None
        session_runtime: AutonomousDomainToolRuntime | None = None
        persistence_requested = self.execution_journal is not None or self.execution_policy is not None or execution_id is not None
        if resume_execution and self.execution_journal is None:
            raise BrainRunError("resume_execution requires execution_journal")
        if persistence_requested:
            selected_domain = next((value for value in tool_domains if value in AUTONOMOUS_DOMAINS), "cross_domain")
            try:
                profile = self.orchestrator.registry.resolve(selected_domain)
                default_capability = profile.default_capability
                default_risk_class = profile.risk_class
            except BrainRunError:
                default_capability = "tool_execution"
                default_risk_class = "cross_domain"
            selected_capability = resolved_options.get("capability")
            selected_risk_class = resolved_options.get("risk_class")
            if not isinstance(selected_capability, str) or not selected_capability.strip():
                selected_capability = default_capability
            if not isinstance(selected_risk_class, str) or not selected_risk_class.strip():
                selected_risk_class = default_risk_class
            resolved_execution_id = execution_id or resolved_options.get("run_id") or f"execution-{uuid.uuid4().hex}"
            if not isinstance(resolved_execution_id, str):
                raise BrainRunError("execution_id must be a string")
            resolved_options.setdefault("run_id", resolved_execution_id)
            execution_policy = self.execution_policy or AutonomousExecutionPolicy()
            try:
                if self.tool_runtime is not None:
                    session_runtime = self.tool_runtime.session(
                        execution_id=resolved_execution_id,
                        domain=selected_domain,
                        capability=selected_capability,
                        risk_class=selected_risk_class,
                        policy=execution_policy,
                        journal=self.execution_journal,
                        resume=resume_execution,
                        authorization_context=resolved_options.get("authorization_context"),
                    )
                    execution_controller = session_runtime.controller
                else:
                    execution_controller = AutonomousExecutionController(
                        execution_id=resolved_execution_id,
                        domain=selected_domain,
                        capability=selected_capability,
                        risk_class=selected_risk_class,
                        policy=execution_policy,
                        journal=self.execution_journal,
                        resume=resume_execution,
                    )
            except AutonomyPersistenceError as error:
                raise BrainRunError("autonomous execution persistence could not start") from error
        if self.tool_runtime is not None and session_runtime is None and self.tool_runtime.controller is None:
            # Even a non-durable run needs a domain identity on its tool receipts.  Without this
            # ephemeral scope, the later evaluator would have to guess ``cross_domain`` and could
            # credit the wrong domain arm.  No journal or checkpoint is created here.
            selected_domain = next((value for value in tool_domains if value in AUTONOMOUS_DOMAINS), "cross_domain")
            resolved_execution_id = execution_id or resolved_options.get("run_id") or f"execution-{uuid.uuid4().hex}"
            resolved_options.setdefault("run_id", resolved_execution_id)
            session_runtime = self.tool_runtime.scoped(
                execution_id=resolved_execution_id,
                domain=selected_domain,
                authorization_context=resolved_options.get("authorization_context"),
            )
        if execution_controller is not None:
            # This is an internal capability, never a caller/model option.  The orchestrator
            # forwards it only to adaptive provider boundaries so every provider turn shares the
            # same count, cost, journal, and resume state as domain tools.
            resolved_options["execution_controller"] = execution_controller
        if self.tool_runtime is not None or session_runtime is not None:
            loop_options = resolved_options.get("tool_loop_options")
            if loop_options is None:
                loop_options = {}
            elif not isinstance(loop_options, Mapping):
                raise BrainRunError("tool_loop_options must be a mapping or None")
            else:
                loop_options = dict(loop_options)
            if session_runtime is not None:
                loop_options["authorize_and_execute"] = session_runtime
            elif self.tool_runtime is not None:
                loop_options.setdefault("authorize_and_execute", self.tool_runtime)
            resolved_options["tool_loop_options"] = loop_options
        if self.health_ledger is not None:
            historical = self.health_ledger.selection_overrides()
            supplied = resolved_options.get("selection_overrides")
            resolved_options["selection_overrides"] = self._merge_selection_overrides(
                historical, supplied
            )
        if resume_learning and resolved_options.get("bandit_state") is None:
            resolved_options["bandit_state"] = self.learning_state()
        return resolved_candidates, resolved_credentials, resolved_options, execution_controller

    @staticmethod
    def _finish_execution(
        controller: AutonomousExecutionController | None,
        result: Any = None,
        error: BaseException | None = None,
    ) -> None:
        if controller is None:
            return
        if error is not None:
            try:
                controller.fail(reason="execution_error")
            except Exception:
                pass
            return
        status = getattr(result, "status", None)
        if not isinstance(status, str):
            status = "completed"
        if status.startswith("completed"):
            controller.complete()
        elif "approval" in status or status in {"paused", "stage_blocked", "stage_proposed", "stage_not_attempted", "stage_failed"}:
            controller.checkpoint(status="paused", reason="execution_paused")
        else:
            controller.fail(reason="execution_failed")

    def restore_prompt_learning(self) -> Any:
        """Restore the configured prompt learner before accepting new high-level work."""

        coordinator = self.prompt_learning_coordinator
        if coordinator is None:
            raise BrainRunError("prompt learning coordinator is not configured")
        return coordinator.restore()

    def flush_prompt_learning(self) -> Any:
        """Flush the current prompt learner without inventing evaluator credit."""

        coordinator = self.prompt_learning_coordinator
        if coordinator is None:
            raise BrainRunError("prompt learning coordinator is not configured")
        return coordinator.flush()

    def restore_learning(self) -> Any:
        """Restore the evaluator/bandit ledger from its caller-owned persistence boundary."""

        if self.ledger is None:
            raise BrainRunError("AutonomousAgent has no learning ledger")
        coordinator = self.learning_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent learning persistence is not configured")
        return coordinator.restore()

    def flush_learning(self) -> Any:
        """Flush evaluator/bandit state through its caller-owned CAS boundary."""

        if self.ledger is None:
            raise BrainRunError("AutonomousAgent has no learning ledger")
        coordinator = self.learning_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent learning persistence is not configured")
        return coordinator.flush()

    def restore_health(self) -> Any:
        """Restore provider/model health priors from the caller-owned persistence boundary."""

        if self.health_ledger is None:
            raise BrainRunError("AutonomousAgent has no provider health ledger")
        coordinator = self.health_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent health persistence is not configured")
        return coordinator.restore()

    def flush_health(self) -> Any:
        """Flush provider/model health observations through their caller-owned CAS boundary."""

        if self.health_ledger is None:
            raise BrainRunError("AutonomousAgent has no provider health ledger")
        coordinator = self.health_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent health persistence is not configured")
        return coordinator.flush()

    def restore_provider_health(self) -> Any:
        """Compatibility alias for restoring the agent's provider/model health ledger."""

        return self.restore_health()

    def flush_provider_health(self) -> Any:
        """Compatibility alias for flushing the agent's provider/model health ledger."""

        return self.flush_health()

    def restore_runtime_health(self) -> Any:
        """Restore process-local provider circuits and transport observations after restart."""

        coordinator = self.runtime_health_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent runtime health persistence is not configured")
        return coordinator.restore()

    def flush_runtime_health(self) -> Any:
        """Flush process-local provider circuits and transport observations through CAS storage."""

        coordinator = self.runtime_health_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent runtime health persistence is not configured")
        return coordinator.flush()

    def restore_transport_health(self) -> Any:
        """Compatibility alias for restoring runtime transport health."""

        return self.restore_runtime_health()

    def flush_transport_health(self) -> Any:
        """Compatibility alias for flushing runtime transport health."""

        return self.flush_runtime_health()

    def register_evaluator_calibration(self, report: Mapping[str, Any]) -> str:
        """Register one validated aggregate evaluator calibration report by digest."""

        registry = self.evaluator_calibration_registry
        if registry is None:
            raise BrainRunError("AutonomousAgent evaluator calibration registry is not configured")
        try:
            return registry.register(report)
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("evaluator calibration report was rejected") from error

    def evaluator_calibration_report(self, report_digest: str) -> dict[str, Any] | None:
        """Return one aggregate calibration report without exposing source evaluation cases."""

        registry = self.evaluator_calibration_registry
        if registry is None:
            raise BrainRunError("AutonomousAgent evaluator calibration registry is not configured")
        try:
            return registry.get(report_digest)
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("evaluator calibration report digest was rejected") from error

    def evaluator_calibration_reports(self) -> list[dict[str, Any]]:
        """Return all registered aggregate calibration reports in deterministic digest order."""

        registry = self.evaluator_calibration_registry
        if registry is None:
            raise BrainRunError("AutonomousAgent evaluator calibration registry is not configured")
        return registry.reports()

    def restore_evaluator_calibration(self) -> Any:
        """Restore aggregate evaluator calibration before admitting readiness or learning."""

        if self.evaluator_calibration_registry is None:
            raise BrainRunError("AutonomousAgent evaluator calibration registry is not configured")
        coordinator = self.evaluator_calibration_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent evaluator calibration persistence is not configured")
        return coordinator.restore()

    def flush_evaluator_calibration(self) -> Any:
        """Flush aggregate evaluator calibration through its caller-owned CAS boundary."""

        if self.evaluator_calibration_registry is None:
            raise BrainRunError("AutonomousAgent evaluator calibration registry is not configured")
        coordinator = self.evaluator_calibration_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent evaluator calibration persistence is not configured")
        return coordinator.flush()

    def restore_online_learning(self) -> Any:
        """Compatibility alias for restoring the agent's value-only online-learning ledger."""

        return self.restore_learning()

    def flush_online_learning(self) -> Any:
        """Compatibility alias for flushing the agent's value-only online-learning ledger."""

        return self.flush_learning()

    def _set_tool_selection_state(self, state: Mapping[str, Any]) -> None:
        self.tool_selection_state = normalize_autonomous_tool_selection_state(state)

    def tool_selection_state_snapshot(self) -> dict[str, Any]:
        """Return a fresh normalized copy of the agent-owned tool selector state."""

        return normalize_autonomous_tool_selection_state(self.tool_selection_state)

    def restore_tool_selection(self) -> AutonomousToolSelectionSnapshot | None:
        """Restore evaluator-approved tool-arm statistics before planning resumes."""

        if self._tool_selection_persistence_coordinator is None:
            raise BrainRunError("AutonomousAgent tool selection persistence is not configured")
        return self._tool_selection_persistence_coordinator.restore()

    def flush_tool_selection(self) -> AutonomousToolSelectionSnapshot:
        """Flush evaluator-approved tool-arm statistics through the caller-owned CAS boundary."""

        if self._tool_selection_persistence_coordinator is None:
            raise BrainRunError("AutonomousAgent tool selection persistence is not configured")
        return self._tool_selection_persistence_coordinator.flush()

    def record_tool_selection_reward(self, outcome: Mapping[str, Any]) -> dict[str, Any]:
        """Apply one independent evaluator reward to the agent-owned adaptive selector."""

        if not self._tool_selection_configured:
            raise BrainRunError("AutonomousAgent tool selection state is not configured")
        try:
            self._set_tool_selection_state(_update_autonomous_tool_selection_state(self.tool_selection_state, outcome))
        except (ArgumentError, BrainRunError, TypeError, ValueError) as error:
            raise BrainRunError("tool selection reward could not be recorded") from error
        return self.tool_selection_state_snapshot()

    def restore_memory(self) -> Any:
        """Restore episodic memory from its caller-owned persistence boundary."""

        if self.memory is None:
            raise BrainRunError("AutonomousAgent has no episodic memory")
        coordinator = self.memory_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent memory persistence is not configured")
        return coordinator.restore()

    def flush_memory(self) -> Any:
        """Flush episodic memory through its caller-owned CAS boundary."""

        if self.memory is None:
            raise BrainRunError("AutonomousAgent has no episodic memory")
        coordinator = self.memory_persistence
        if coordinator is None:
            raise BrainRunError("AutonomousAgent memory persistence is not configured")
        return coordinator.flush()

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentStateMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, (staticmethod, classmethod)) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
