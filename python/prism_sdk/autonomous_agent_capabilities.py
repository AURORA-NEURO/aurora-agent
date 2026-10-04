"""Focused autonomous-agent capabilities methods."""

from __future__ import annotations

from .autonomous_tool_selection import (
    AUTONOMOUS_TOOL_RISK_ORDER,
    AUTONOMOUS_TOOL_SELECTION_POLICY,
    AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
    _portfolio_binding_supports_stage,
    _portfolio_candidate_ranking,
    _portfolio_score,
    _portfolio_score_key,
    _portfolio_task_tokens,
    _tool_risk_allowed,
    _tool_selection_arm_for,
    _tool_selection_number,
    _tool_selection_utility,
    _update_autonomous_tool_selection_state,
    normalize_autonomous_tool_selection_state,
)

from .autonomy import (
    AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
    AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
    AUTONOMOUS_CAPABILITY_PORTFOLIO_SCHEMA,
    AUTONOMOUS_DOMAINS,
    AUTONOMOUS_EXECUTION_PLAN_SCHEMA,
    AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA,
    AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
    AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA,
    Any,
    ArgumentError,
    AutonomousActivationError,
    AutonomousCapabilityExecutionResult,
    AutonomousCapabilityRuntime,
    AutonomousDomainTool,
    AutonomousDomainToolBinding,
    AutonomousDomainToolReceipt,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    AutonomousProviderInvocationReceipt,
    AutonomousProviderLearningReport,
    AutonomousProviderOutcomeEvaluator,
    AutonomousRecoveryHandoffLedger,
    AutonomousRecoveryObservation,
    AutonomousRecoveryPlan,
    AutonomousToolLearningReport,
    AutonomousToolOutcomeEvaluator,
    BrainLearningLedger,
    BrainRunError,
    Callable,
    CredentialHandle,
    CredentialSession,
    DOMAIN_TOOL_BINDING_PLAN_SCHEMA,
    MAX_AUTONOMOUS_CAPABILITY_PLAN_BYTES,
    MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TASK_BYTES,
    MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TOOLS,
    MAX_AUTONOMOUS_MODEL_SELECTION_PREVIEW_BYTES,
    Mapping,
    ModelCandidate,
    Sequence,
    ToolCatalogue,
    ToolDefinition,
    _AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY,
    _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY,
    _authorize_launch_admission_domains,
    _capability_request_domains_for_launch_admission,
    _context_identity_digest,
    _identifier,
    _safe_json,
    _selection_overrides_with_weights,
    _sequence,
    _text,
    _valid_digest,
    build_model_selection_audit,
    builtin_autonomous_domain_tool_profiles,
    compile_autonomous_domain_execution_plan,
    content_digest,
    json,
    normalize_autonomous_model_observations,
    normalize_autonomous_selection_weights,
    plan_autonomous_recovery,
    plan_mcp_catalogue_bindings,
    settle_autonomous_provider_model_outcome,
    validate_autonomous_evaluator_calibration_report,
    validate_autonomous_selection_promotion_report,
)

class AutonomousAgentCapabilitiesMixin:
    def capability_portfolio(
        self,
        task: str,
        *,
        domains: Sequence[str] | None = None,
        capability: str | None = None,
        allowed_tools: Sequence[str] | None = None,
        max_tools: int = 32,
        read_only_only: bool = False,
        max_risk_class: str | None = None,
        tool_learning_state: Mapping[str, Any] | None = None,
        exploration: float = 0.15,
    ) -> dict[str, Any]:
        """Select a bounded exact-name tool portfolio for a transient task.

        Workflow stages and reviewed domain-tool bindings are the authority for candidate
        selection.  Task text is used only for local deterministic ranking and is represented in
        the returned packet by a digest.  This method never invokes a provider, executes a tool,
        or turns a selected binding into authorization; activation and effect approval remain
        independent runtime gates.
        """

        task_text = _text(
            "capability portfolio task",
            task,
            maximum=MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TASK_BYTES,
        )
        selected_domains = tuple(AUTONOMOUS_DOMAINS) if domains is None else _sequence(
            "capability portfolio domains",
            domains,
            maximum=len(AUTONOMOUS_DOMAINS),
        )
        unknown_domains = sorted(set(selected_domains).difference(AUTONOMOUS_DOMAINS))
        if unknown_domains:
            raise BrainRunError(
                "capability portfolio contains unknown domains: " + ", ".join(unknown_domains)
            )
        if isinstance(max_tools, bool) or not isinstance(max_tools, int) or not 1 <= max_tools <= MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TOOLS:
            raise BrainRunError(
                f"capability portfolio max_tools must be between 1 and {MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TOOLS}"
            )
        maximum_risk_class = "high_impact_effect" if max_risk_class is None else max_risk_class
        if maximum_risk_class not in AUTONOMOUS_TOOL_RISK_ORDER:
            raise BrainRunError("capability portfolio max_risk_class is unsupported")
        tool_learning_state = normalize_autonomous_tool_selection_state(tool_learning_state)
        exploration = float(_tool_selection_number("capability portfolio exploration", exploration, 0, 1))
        total_pulls = sum(int(arm["pulls"]) for arm in tool_learning_state["arms"])
        requested_capabilities = () if capability is None else (
            _identifier("capability portfolio capability", capability),
        )
        caller_allowed: set[str] | None = None
        if allowed_tools is not None:
            if not isinstance(allowed_tools, Sequence) or isinstance(allowed_tools, (str, bytes)):
                raise BrainRunError("capability portfolio allowed_tools must be a sequence")
            if len(allowed_tools) > MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TOOLS:
                raise BrainRunError("capability portfolio allowed_tools exceed their bound")
            caller_allowed = {_identifier("capability portfolio allowed tool", name) for name in allowed_tools}

        activation_state = self.activation.state
        if activation_state.status == "revoked":
            effective_allowed: set[str] | None = set()
        elif activation_state.plan_digest is not None:
            effective_allowed = set(activation_state.approved_tools)
        else:
            effective_allowed = None
        if caller_allowed is not None:
            effective_allowed = caller_allowed if effective_allowed is None else effective_allowed.intersection(caller_allowed)

        profile_map = {
            profile.domain: profile
            for profile in builtin_autonomous_domain_tool_profiles()
        }
        workflow_map = {
            domain: self.orchestrator.workflow_registry.resolve(domain)
            for domain in selected_domains
        }
        live_by_name: dict[str, AutonomousDomainTool] = {}
        if self.tool_registry is not None:
            for domain in selected_domains:
                for tool in self.tool_registry.tools_for((domain,)):
                    live_by_name.setdefault(tool.name, tool)

        tokens = _portfolio_task_tokens(task_text)
        stage_rows: list[dict[str, Any]] = []
        for domain in selected_domains:
            profile = profile_map.get(domain)
            if profile is None:
                raise BrainRunError(f"no reviewed domain tool profile is registered for {domain!r}")
            binding_map = {binding.name: binding for binding in profile.bindings}
            workflow = workflow_map[domain]
            for stage in workflow.stages:
                bindings = [
                    binding
                    for binding in binding_map.values()
                    if _portfolio_binding_supports_stage(domain, stage, binding)
                ]
                live_bindings = [binding for binding in bindings if binding.name in live_by_name]
                eligible = [
                    binding
                    for binding in live_bindings
                    if (not read_only_only or binding.read_only)
                    and (not stage.read_only or binding.read_only)
                    and (stage.approval_required or not binding.approval_required)
                    and _tool_risk_allowed(binding.risk_class, maximum_risk_class)
                    and (effective_allowed is None or binding.name in effective_allowed)
                    and not bool((_tool_selection_arm_for(tool_learning_state, domain, stage, binding) or {}).get("disabled", False))
                ]
                ranked = sorted(
                    eligible,
                    key=lambda binding: _portfolio_score_key(
                        _portfolio_score(tokens, requested_capabilities, stage, binding, domain, tool_learning_state, total_pulls, exploration),
                        binding.name,
                    ),
                )
                stage_rows.append(
                    {
                        "domain": domain,
                        "stage": stage,
                        "bindings": bindings,
                        "live_bindings": live_bindings,
                        "eligible": eligible,
                        "ranked": ranked,
                    }
                )

        preferred: dict[str, tuple[AutonomousDomainToolBinding, tuple[int, int, float, int, int], str]] = {}
        for row in stage_rows:
            candidate = row["ranked"][0] if row["ranked"] else None
            if candidate is None:
                continue
            score = _portfolio_score(tokens, requested_capabilities, row["stage"], candidate, row["domain"], tool_learning_state, total_pulls, exploration)
            previous = preferred.get(candidate.name)
            if previous is None or _portfolio_score_key(score, candidate.name) < _portfolio_score_key(previous[1], candidate.name) or (
                score == previous[1] and row["domain"] < previous[2]
            ):
                preferred[candidate.name] = (candidate, score, row["domain"])

        ranked_names = [
            name
            for name, _ in sorted(
                preferred.items(),
                key=lambda item: _portfolio_score_key(item[1][1], item[0]),
            )
        ]
        selected_names: set[str] = set()
        selected_tool_order: list[str] = []
        for row in stage_rows:
            candidate = next(
                (binding for binding in row["ranked"] if binding.name not in selected_names),
                row["ranked"][0] if row["ranked"] else None,
            )
            if candidate is not None and len(selected_names) < max_tools:
                selected_names.add(candidate.name)
                if candidate.name not in selected_tool_order:
                    selected_tool_order.append(candidate.name)
        for name in ranked_names:
            if len(selected_names) >= max_tools:
                break
            selected_names.add(name)
            if name not in selected_tool_order:
                selected_tool_order.append(name)

        selected_tool_names = sorted(selected_names)
        def binding_projection(binding: AutonomousDomainToolBinding) -> dict[str, Any]:
            return {
                "name": binding.name,
                "domains": list(binding.domains),
                "capability": binding.capability,
                "risk_class": binding.risk_class,
                "read_only": binding.read_only,
                "approval_required": binding.approval_required,
                "secret_material": "never_returned",
            }

        selected_bindings = [
            binding_projection(preferred[name][0])
            for name in selected_tool_names
            if name in preferred
        ]
        coverage: list[dict[str, Any]] = []
        for row in stage_rows:
            selected = next(
                (binding for binding in row["ranked"] if binding.name in selected_names),
                None,
            )
            if selected is not None:
                status = "selected"
            elif not row["bindings"]:
                status = "provider_only"
            elif not row["live_bindings"]:
                status = "catalogue_missing"
            elif effective_allowed is not None and not row["eligible"] and all(binding.name not in effective_allowed for binding in row["live_bindings"]):
                status = "activation_required"
            elif row["live_bindings"] and all(not _tool_risk_allowed(binding.risk_class, maximum_risk_class) for binding in row["live_bindings"]):
                status = "risk_budget_blocked"
            elif any(bool((_tool_selection_arm_for(tool_learning_state, row["domain"], row["stage"], binding) or {}).get("disabled", False)) for binding in row["live_bindings"]):
                status = "learning_disabled"
            else:
                status = "capacity_limited"
            selected_arm = None if selected is None else _tool_selection_arm_for(tool_learning_state, row["domain"], row["stage"], selected)
            candidate_ranking = _portfolio_candidate_ranking(
                tokens,
                requested_capabilities,
                row["stage"],
                row["live_bindings"],
                row["domain"],
                tool_learning_state,
                total_pulls,
                exploration,
                effective_allowed,
                read_only_only,
                maximum_risk_class,
            )
            selected_rank = None if selected is None else next((candidate["rank"] for candidate in candidate_ranking if candidate["tool"] == selected.name), None)
            rationale = (
                "highest_ranked_eligible_candidate"
                if selected is not None and selected_rank == 1
                else "portfolio_reuse_lower_rank_candidate"
                if selected is not None
                else "no_reviewed_binding_for_stage"
                if status == "provider_only"
                else "no_live_catalogue_binding"
                if status == "catalogue_missing"
                else "activation_or_allowlist_required"
                if status == "activation_required"
                else "all_candidates_learning_disabled"
                if status == "learning_disabled"
                else "risk_budget_excluded_all_candidates"
                if status == "risk_budget_blocked"
                else "portfolio_capacity_limit"
            )
            coverage.append(
                {
                    "domain": row["domain"],
                    "stage_id": row["stage"].id,
                    "required_capabilities": list(row["stage"].required_capabilities),
                    "candidate_tool_names": sorted(binding.name for binding in row["live_bindings"]),
                    "selected_tool": None if selected is None else selected.name,
                    "selected_capability": None if selected is None else selected.capability,
                    "approval_required": False if selected is None else selected.approval_required,
                    "selected_arm_id": None if selected_arm is None else selected_arm["arm_id"],
                    "selection_utility": None if selected is None else _tool_selection_utility(selected_arm, total_pulls, exploration),
                    "candidate_ranking": candidate_ranking,
                    "selection_rationale": rationale,
                    "status": status,
                }
            )

        all_live_bindings = [
            binding
            for domain in selected_domains
            for binding in profile_map[domain].bindings
            if binding.name in live_by_name
        ]
        binding_by_name: dict[str, AutonomousDomainToolBinding] = {}
        binding_domains: dict[str, set[str]] = {}
        for binding in all_live_bindings:
            binding_by_name.setdefault(binding.name, binding)
            binding_domains.setdefault(binding.name, set()).update(binding.domains)
        omissions = []
        for name in sorted(binding_by_name):
            if name in selected_names:
                continue
            binding = binding_by_name[name]
            disabled = any(
                bool((_tool_selection_arm_for(tool_learning_state, domain, row, binding) or {}).get("disabled", False))
                for domain in selected_domains
                for row in workflow_map[domain].stages
                if _portfolio_binding_supports_stage(domain, row, binding)
            )
            reason = (
                "activation_required"
                if effective_allowed is not None and name not in effective_allowed
                else (
                    "risk_budget_limited"
                    if not _tool_risk_allowed(binding.risk_class, maximum_risk_class)
                    else (
                    "learning_disabled"
                    if disabled
                    else "capacity_limited"
                    if name in preferred
                    else "not_required_for_reviewed_workflow"
                    )
                )
            )
            omissions.append(
                {
                    "name": name,
                    "domains": sorted(binding_domains[name]),
                    "capability": binding_by_name[name].capability,
                    "reason": reason,
                }
            )
        missing_tools = sorted({
            binding.name
            for domain in selected_domains
            for binding in profile_map[domain].bindings
            if binding.name not in live_by_name
        })
        descriptor = {
            "schema": AUTONOMOUS_CAPABILITY_PORTFOLIO_SCHEMA,
            "task_digest": content_digest({"task": task_text}),
            "catalogue_digest": None if self.tool_registry is None else self.tool_registry.digest,
            "profile_digest": content_digest([
                profile_map[domain].to_dict() for domain in selected_domains
            ]),
            "domains": list(selected_domains),
            "requested_capabilities": list(requested_capabilities),
            "max_tools": max_tools,
            "selected_tool_names": selected_tool_names,
            "selected_tool_order": selected_tool_order,
            "selected_bindings": selected_bindings,
            "approval_required_tools": sorted(
                binding["name"] for binding in selected_bindings if binding.get("approval_required") is True
            ),
            "missing_tools": missing_tools,
            "omissions": omissions[:MAX_AUTONOMOUS_CAPABILITY_PORTFOLIO_TOOLS],
            "coverage": coverage,
            "selection_learning": {
                "schema": AUTONOMOUS_TOOL_SELECTION_STATE_SCHEMA,
                "generation": tool_learning_state["generation"],
                "state_digest": content_digest(tool_learning_state),
                "exploration": exploration,
                "total_pulls": total_pulls,
                "known_arm_count": len(tool_learning_state["arms"]),
                "disabled_arm_count": sum(int(arm["disabled"]) for arm in tool_learning_state["arms"]),
                "retention": "value_only;tool_arguments_outputs_prompts_and_credentials_never_returned",
            },
            "selection_constraints": {
                "max_risk_class": maximum_risk_class,
                "read_only_only": read_only_only,
                "allowed_tools_digest": None if caller_allowed is None else content_digest(sorted(caller_allowed)),
                "policy": AUTONOMOUS_TOOL_SELECTION_POLICY,
            },
            "selection_policy": AUTONOMOUS_TOOL_SELECTION_POLICY,
            "execution": "metadata_only; no_provider_or_tool_calls",
            "authorization": "selection_does_not_authorize_tools_or_effects",
            "secret_material": "never_returned",
        }
        result = {
            **descriptor,
            "plan_digest": content_digest(descriptor),
        }
        try:
            encoded = json.dumps(
                result,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            )
        except (TypeError, ValueError) as error:
            raise BrainRunError("autonomous capability portfolio must be JSON-safe") from error
        if len(encoded.encode("utf-8")) > MAX_AUTONOMOUS_CAPABILITY_PLAN_BYTES:
            raise BrainRunError("autonomous capability portfolio exceeds its bounded size")
        return json.loads(encoded)

    def domain_execution_plan(
        self,
        domain: str,
        *,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compile one domain's reviewed contracts into a non-executing runtime blueprint.

        The result is intentionally safe to show in a configuration UI or attach to a
        provider prompt.  It identifies exact registered tools and model arms, but it contains
        no task text, credential handles, keys, tool arguments, provider responses, or effect
        authorization.
        """

        _identifier("execution plan domain", domain)
        if model_candidates is None:
            candidates = self.catalogue.candidates()
        else:
            if not isinstance(model_candidates, Sequence) or isinstance(model_candidates, (str, bytes)):
                raise BrainRunError("execution plan model_candidates must be a sequence")
            candidates = [
                candidate.to_dict()
                if isinstance(candidate, ModelCandidate)
                else ModelCandidate.from_mapping(candidate).to_dict()
                for candidate in model_candidates
            ]
        profile = self.orchestrator.registry.resolve(domain)
        pack = self.orchestrator.pack_registry.resolve(domain)
        workflow = self.orchestrator.workflow_registry.resolve(domain)
        registered = () if self.tool_registry is None else self.tool_registry.tools_for((domain,))
        return compile_autonomous_domain_execution_plan(
            domain,
            profile=profile,
            pack=pack,
            workflow=workflow,
            registered_tools=registered,
            activation=self.activation,
            model_candidates=candidates,
            provider_statuses=self.onboarding.statuses(),
        )

    def domain_capabilities(self, domain: str) -> list[dict[str, Any]]:
        """Return the reviewed capability/evidence adapters for one domain.

        Each row identifies the domain-level capability, exact adapter capability labels,
        evidence outputs, evaluator signals, and currently active tool names.  The rows are
        planning metadata only; they do not authorize a provider call or tool effect.
        """

        plan = self.domain_execution_plan(domain)
        capabilities = plan.get("capabilities", {}).get("contracts", [])
        if not isinstance(capabilities, list):
            raise BrainRunError("domain execution plan capability contracts are malformed")
        return [dict(row) for row in capabilities if isinstance(row, Mapping)]

    def domain_capability_plan(
        self,
        domain: str,
        capability: str,
        *,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compile one focused capability into a non-executing dispatch plan."""

        _identifier("capability plan domain", domain)
        resolved_capability = _identifier("capability plan capability", capability)
        plan = self.domain_execution_plan(domain, model_candidates=model_candidates)
        rows = plan.get("capabilities", {}).get("contracts", [])
        if not isinstance(rows, list):
            raise BrainRunError("domain execution plan capability contracts are malformed")
        row = next(
            (
                value for value in rows
                if isinstance(value, Mapping) and value.get("capability") == resolved_capability
            ),
            None,
        )
        if not isinstance(row, Mapping):
            raise BrainRunError(
                f"no reviewed capability contract is registered for {domain!r}/{resolved_capability!r}"
            )
        base_status = plan.get("status")
        if base_status in {"revoked", "stale", "model_gap", "provider_pending", "activation_review_required"}:
            status = base_status
        elif row.get("approval_required") is True:
            status = "approval_gated"
        elif row.get("active_tool_names"):
            status = "ready"
        else:
            status = "provider_only"
        result = {
            "schema": AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
            "domain": domain,
            "capability": resolved_capability,
            "status": status,
            "domain_plan_digest": plan["plan_digest"],
            "contract_digest": row.get("contract_digest"),
            "contract": dict(row.get("contract", {})),
            "stage_ids": list(row.get("stage_ids", [])),
            "active_tool_names": list(row.get("active_tool_names", [])),
            "withheld_tool_names": list(row.get("withheld_tool_names", [])),
            "matched_active_tool_capabilities": list(row.get("matched_active_tool_capabilities", [])),
            "tool_posture": row.get("tool_posture"),
            "execution_posture": row.get("execution_posture"),
            "evidence_outputs": list(row.get("evidence_outputs", [])),
            "evaluator_signals": list(row.get("evaluator_signals", [])),
            "review_gates": dict(plan.get("review_gates", {})),
            "learning_context_digest": plan.get("learning", {}).get("context_digest"),
            "execution": "planning_only; dispatch_requires_caller_credentials_and_approval",
            "credential_posture": "caller_supplied_opaque_handles; no_keys_or_handles_in_plan",
            "authority_posture": "metadata_only; plan_does_not_grant_authority",
        }
        return _safe_json(
            "autonomous capability plan",
            result,
            maximum=MAX_AUTONOMOUS_CAPABILITY_PLAN_BYTES,
        )

    def capability_plans(
        self,
        domains: Sequence[str] | None = None,
        *,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compile every reviewed capability contract for selected domains."""

        selected = tuple(AUTONOMOUS_DOMAINS) if domains is None else _sequence(
            "capability plan domains", domains, maximum=len(AUTONOMOUS_DOMAINS)
        )
        unknown = sorted(set(selected).difference(AUTONOMOUS_DOMAINS))
        if unknown:
            raise BrainRunError("capability plan contains unknown domains: " + ", ".join(unknown))
        plans = [
            self.domain_capability_plan(domain, row["capability"], model_candidates=model_candidates)
            for domain in selected
            for row in self.domain_capabilities(domain)
        ]
        return {
            "schema": AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
            "status": "multi_domain" if len(selected) > 1 else (plans[0]["status"] if plans else "ready"),
            "domains": list(selected),
            "capability_count": len(plans),
            "plans": plans,
            "plan_digest": content_digest(plans),
            "execution": "planning_only; no_provider_or_tool_invocation",
            "authority_posture": "metadata_only; plans_do_not_grant_authority",
            "secret_material": "never_returned",
        }

    def model_capability_coverage(self, domains: Sequence[str] | None = None) -> dict[str, Any]:
        """Project static model-arm coverage for every selected autonomous domain.

        This joins reviewed domain requirements with caller-declared catalogue capabilities. It
        is intentionally separate from readiness: an arm can be capability-compatible while its
        provider still needs a credential, has an open circuit, or is blocked by another live
        gate.
        """

        selected = tuple(AUTONOMOUS_DOMAINS) if domains is None else _sequence(
            "model capability coverage domains", domains, maximum=len(AUTONOMOUS_DOMAINS)
        )
        unknown = sorted(set(selected).difference(AUTONOMOUS_DOMAINS))
        if unknown:
            raise BrainRunError("model capability coverage contains unknown domains: " + ", ".join(unknown))
        rows: list[dict[str, Any]] = []
        for domain in selected:
            profile = self.orchestrator.registry.resolve(domain)
            report = self.catalogue.compatibility_report(profile.required_model_capabilities)
            rows.append(
                {
                    "domain": domain,
                    "required_model_capabilities": list(profile.required_model_capabilities),
                    "catalogue": report,
                }
            )
        return {
            "schema": "bioprism-autonomous-model-capability-coverage/0.1",
            "domains": list(selected),
            "domain_count": len(rows),
            "rows": rows,
            "evidence_posture": "static_caller_declared_capabilities_only",
            "runtime_gates": "not_projected; readiness and selection apply live provider gates",
            "secret_material": "never_returned",
        }

    def model_selection_preview(
        self,
        *,
        task: str,
        domain: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession | None = None,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        capability: str | None = None,
        risk_class: str | None = None,
        context: Mapping[str, Any] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_model_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        min_selection_confidence: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Preview the live domain-scoped selector without contacting a provider.

        This is the operator/UI boundary for the same decision that an execution call will use.
        It compiles the reviewed domain blueprint and execution-plan identity, joins caller-owned
        health and evaluator state, and asks only the local brain kernel for its bounded ranking.
        Missing credentials therefore appear as eligibility evidence instead of triggering key
        collection or a speculative provider request. The returned projection contains model-arm
        metadata and digests, never task text, prompts, handles, or provider payloads.
        """

        if not isinstance(task, str) or not task.strip():
            raise BrainRunError("model selection preview task must be a non-empty string")
        _identifier("model selection preview domain", domain)
        if domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError(f"model selection preview domain is unsupported: {domain!r}")
        resolved_candidates = self._resolve_candidates(model_candidates)
        resolved_credentials = (
            {}
            if credentials is None
            else self._credential_mapping(credentials)
        )
        blueprint = self.prepare(
            task=task,
            domain=domain,
            capability=capability,
            risk_class=risk_class,
            context=context,
            max_input_tokens=input_tokens,
            required_model_capabilities=required_model_capabilities,
        )
        execution_plan = self.execution_plans(
            (domain,),
            model_candidates=resolved_candidates,
        )
        selection_context = dict(blueprint.selection_context)
        selection_context[_AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY] = execution_plan
        capability_contract_digest: str | None = None
        if capability is not None:
            capability_plan = self.domain_capability_plan(
                domain,
                blueprint.spec.capability,
                model_candidates=resolved_candidates,
            )
            contract = capability_plan.get("contract")
            if not isinstance(contract, Mapping):
                raise BrainRunError("model selection preview capability contract is malformed")
            capability_contract_digest = contract.get("contract_digest")
            if not isinstance(capability_contract_digest, str):
                raise BrainRunError("model selection preview capability contract has no digest")
            selection_context[_AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY] = dict(contract)

        effective_state = bandit_state
        if effective_state is None:
            effective_state = self.domain_learning_state(
                domain,
                capability=blueprint.spec.capability,
                risk_class=blueprint.spec.risk_class,
            )["bandit_state"]
        if not isinstance(effective_state, Mapping):
            raise BrainRunError("model selection preview bandit state is malformed")

        # The approval contract binds only caller-supplied observations. The live request may
        # also merge persisted health-ledger or bandit evidence, but that mutable state must not
        # invalidate a preview merely because it changed between review and dispatch.
        raw_preview_observations = (
            selection_observations
            if selection_observations is not None
            else (
                selection_overrides.get("observations")
                if isinstance(selection_overrides, Mapping)
                else None
            )
        )
        try:
            normalized_preview_observations = normalize_autonomous_model_observations(
                raw_preview_observations
            )
        except ArgumentError as error:
            raise BrainRunError(str(error)) from error

        effective_overrides = (
            None if selection_overrides is None else dict(selection_overrides)
        )
        if self.health_ledger is not None:
            effective_overrides = self._merge_selection_overrides(
                self.health_ledger.selection_overrides(),
                effective_overrides,
            )
        effective_overrides = _selection_overrides_with_weights(
            effective_overrides,
            selection_weights,
            selection_observations,
        )
        merged_overrides = {} if effective_overrides is None else dict(effective_overrides)
        merged_overrides.update(
            {
                "autonomy_execution_plan_digest": content_digest(execution_plan["plans"]),
                "autonomy_execution_plan_statuses": {
                    row["domain"]: row["status"]
                    for row in execution_plan["plans"]
                    if isinstance(row, Mapping)
                    and isinstance(row.get("domain"), str)
                    and isinstance(row.get("status"), str)
                },
            }
        )
        if capability is not None:
            merged_overrides.update(
                {
                    "autonomy_capability_focus": blueprint.spec.capability,
                    "autonomy_capability_contract_digest": capability_contract_digest,
                }
            )
        selection_request = self.brain.build_adaptive_model_selection(
            task=task,
            model_candidates=resolved_candidates,
            credentials=resolved_credentials,
            ledger=self.ledger,
            bandit_state=effective_state,
            context=selection_context,
            contextual_observations=contextual_observations,
            required_capabilities=blueprint.required_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            min_selection_confidence=min_selection_confidence,
            selection_overrides=merged_overrides,
            selection_observations=selection_observations,
        )
        report = self.brain._preview_adaptive_selection(
            task=task,
            selection=selection_request,
            context=selection_context,
        )
        audit = build_model_selection_audit(report)
        eligibility = audit.get("eligibility", {})
        if not isinstance(eligibility, Mapping):
            raise BrainRunError("model selection preview audit eligibility is malformed")
        preview = {
            "schema": AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA,
            "status": audit["selection_status"],
            "task_digest": blueprint.spec.task_digest,
            "domain": blueprint.spec.domain,
            "capability": blueprint.spec.capability,
            "risk_class": blueprint.spec.risk_class,
            "workflow_id": blueprint.workflow.workflow_id,
            "workflow_digest": blueprint.workflow.workflow_digest,
            "domain_pack_digest": blueprint.domain_pack.pack_digest,
            "task_intent_digest": blueprint.task_intent.intent_digest if blueprint.task_intent is not None else None,
            "task_decision_digest": blueprint.task_decision.decision_digest if blueprint.task_decision is not None else None,
            "task_decision_posture": blueprint.task_decision.posture if blueprint.task_decision is not None else None,
            "selection_context_digest": _context_identity_digest(selection_context),
            "execution_plan_digest": content_digest(execution_plan["plans"]),
            "capability_contract_digest": capability_contract_digest,
            "required_model_capabilities": list(blueprint.required_capabilities),
            "candidate_count": len(resolved_candidates),
            "eligible_candidate_count": eligibility.get("eligible_count", 0),
            "selection_contract": {
                "task_digest": blueprint.spec.task_digest,
                "domain": blueprint.spec.domain,
                "capability": blueprint.spec.capability,
                "risk_class": blueprint.spec.risk_class,
                "task_intent_digest": blueprint.task_intent.intent_digest if blueprint.task_intent is not None else None,
                "task_decision_digest": blueprint.task_decision.decision_digest if blueprint.task_decision is not None else None,
                "task_decision_posture": blueprint.task_decision.posture if blueprint.task_decision is not None else None,
                "required_model_capabilities": list(blueprint.required_capabilities),
                "candidate_ids": [
                    f"{candidate['provider']}/{candidate['model']}"
                    for candidate in resolved_candidates
                ],
                "input_tokens": input_tokens,
                "requested_output_tokens": requested_output_tokens,
                "max_cost_per_million_tokens": max_cost_per_million_tokens,
                "max_latency_ms": max_latency_ms,
                "min_quality": min_quality,
                "min_selection_confidence": min_selection_confidence,
                "selection_weights": selection_request["weights"],
                "selection_observations_digest": content_digest(normalized_preview_observations),
            },
            "selection_audit": audit,
            "review": {
                "provider_call": "not_started",
                "domain_tools": "not_started",
                "caller_approval_required": True,
                "next_action": (
                    "resolve_task_decision_block"
                    if blueprint.task_decision is not None and blueprint.task_decision.posture == "blocked"
                    else "review_selection_and_approve_provider_call"
                    if audit["selection_status"] == "selected"
                    else "resolve_model_provider_or_credential_gates"
                ),
            },
            "execution": "preview_only; no_provider_or_domain_tool_invocation",
            "authority_posture": "selection_review_only; preview_does_not_authorize_provider_or_effects",
            "credential_posture": "caller_opaque_handles_only; no_handles_returned",
            "retention": "metadata_only_model_ranking_and_digests",
            "secret_material": "never_returned",
        }
        return _safe_json(
            "autonomous model selection preview",
            preview,
            maximum=MAX_AUTONOMOUS_MODEL_SELECTION_PREVIEW_BYTES,
        )

    def run_approved_model_selection(
        self,
        *,
        task: str,
        domain: str,
        selection_preview: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        capability: str | None = None,
        risk_class: str | None = None,
        context: Mapping[str, Any] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_model_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        min_selection_confidence: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Revalidate a provider-free preview, then invoke only its reviewed model arm."""

        if not isinstance(selection_preview, Mapping):
            raise BrainRunError("approved model selection preview must be a mapping")
        if selection_preview.get("schema") != AUTONOMOUS_MODEL_SELECTION_PREVIEW_SCHEMA:
            raise BrainRunError("approved model selection preview schema is invalid")
        if selection_preview.get("status") != "selected":
            raise BrainRunError("approved model selection preview is not selected")
        if selection_preview.get("task_decision_posture") == "blocked":
            raise BrainRunError("approved model selection is blocked by the task decision posture")
        contract = selection_preview.get("selection_contract")
        audit = selection_preview.get("selection_audit")
        if not isinstance(contract, Mapping) or not isinstance(audit, Mapping):
            raise BrainRunError("approved model selection preview is missing its selection contract")
        try:
            supplied_weights = normalize_autonomous_selection_weights(
                selection_weights
                if selection_weights is not None
                else (
                    selection_overrides.get("weights")
                    if isinstance(selection_overrides, Mapping)
                    else None
                )
            )
            reviewed_weights = normalize_autonomous_selection_weights(
                contract.get("selection_weights")
            )
            raw_supplied_observations = (
                selection_observations
                if selection_observations is not None
                else (
                    selection_overrides.get("observations")
                    if isinstance(selection_overrides, Mapping)
                    else None
                )
            )
            supplied_observations = normalize_autonomous_model_observations(
                raw_supplied_observations
            )
        except ArgumentError as error:
            raise BrainRunError(str(error)) from error
        if supplied_weights != reviewed_weights:
            raise BrainRunError("approved model selection weights changed; re-review required")
        reviewed_observations_digest = contract.get("selection_observations_digest")
        if (
            not isinstance(reviewed_observations_digest, str)
            or not _valid_digest(reviewed_observations_digest)
            or content_digest(supplied_observations) != reviewed_observations_digest
        ):
            raise BrainRunError("approved model selection observations changed; re-review required")
        effective_selection_overrides = _selection_overrides_with_weights(
            selection_overrides,
            supplied_weights,
            supplied_observations,
        )
        selected = audit.get("selected_model")
        selected_id = selected.get("model_id") if isinstance(selected, Mapping) else None
        if not isinstance(selected_id, str) or not selected_id.strip() or "/" not in selected_id:
            raise BrainRunError("approved model selection preview has no exact selected model")
        candidate_ids = contract.get("candidate_ids")
        if (
            not isinstance(candidate_ids, Sequence)
            or isinstance(candidate_ids, (str, bytes))
            or any(not isinstance(candidate_id, str) or not candidate_id.strip() for candidate_id in candidate_ids)
        ):
            raise BrainRunError("approved model selection preview candidate ids are malformed")
        resolved_candidates = self._resolve_candidates(model_candidates)
        resolved_ids = [
            f"{candidate['provider']}/{candidate['model']}"
            for candidate in resolved_candidates
        ]
        if list(candidate_ids) != resolved_ids:
            raise BrainRunError("approved model selection candidate catalogue changed; re-review required")
        selected_candidates = [
            candidate for candidate in resolved_candidates
            if f"{candidate['provider']}/{candidate['model']}" == selected_id
        ]
        if len(selected_candidates) != 1:
            raise BrainRunError("approved model selection selected model is absent or duplicated")
        if contract.get("domain") != domain:
            raise BrainRunError("approved model selection domain does not match the request")
        if capability is not None and capability != contract.get("capability"):
            raise BrainRunError("approved model selection capability does not match the request")
        if risk_class is not None and risk_class != contract.get("risk_class"):
            raise BrainRunError("approved model selection risk class does not match the request")
        expected_contract = {
            "task_digest": selection_preview.get("task_digest"),
            "domain": domain,
            "capability": contract.get("capability"),
            "risk_class": contract.get("risk_class"),
            "task_intent_digest": contract.get("task_intent_digest"),
            "task_decision_digest": contract.get("task_decision_digest"),
            "task_decision_posture": contract.get("task_decision_posture"),
            "required_model_capabilities": list(contract.get("required_model_capabilities", ())),
            "candidate_ids": resolved_ids,
            "input_tokens": input_tokens,
            "requested_output_tokens": requested_output_tokens,
            "max_cost_per_million_tokens": max_cost_per_million_tokens,
            "max_latency_ms": max_latency_ms,
            "min_quality": min_quality,
            "min_selection_confidence": min_selection_confidence,
            "selection_weights": supplied_weights,
            "selection_observations_digest": reviewed_observations_digest,
        }
        if contract != expected_contract:
            raise BrainRunError("approved model selection inputs changed; re-review required")
        fresh = self.model_selection_preview(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=resolved_candidates,
            capability=capability,
            risk_class=risk_class,
            context=context,
            bandit_state=bandit_state,
            contextual_observations=contextual_observations,
            required_model_capabilities=required_model_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            min_selection_confidence=min_selection_confidence,
            selection_overrides=effective_selection_overrides,
            selection_weights=supplied_weights,
            selection_observations=supplied_observations,
        )
        for field_name in (
            "task_digest", "domain", "capability", "risk_class", "workflow_id",
            "workflow_digest", "domain_pack_digest", "task_intent_digest", "task_decision_digest",
            "task_decision_posture", "selection_context_digest",
            "execution_plan_digest", "required_model_capabilities", "selection_contract",
            "selection_audit",
        ):
            if fresh.get(field_name) != selection_preview.get(field_name):
                raise BrainRunError("approved model selection is stale; re-review required")
        fresh_selected = fresh["selection_audit"].get("selected_model")
        if not isinstance(fresh_selected, Mapping) or fresh_selected.get("model_id") != selected_id:
            raise BrainRunError("approved model selection changed; re-review required")
        run_options = dict(kwargs)
        supplied_approval = run_options.pop("approve_provider_call", None)
        if supplied_approval is not None and supplied_approval is not True:
            raise BrainRunError("approved model selection cannot disable provider approval")
        supplied_failovers = run_options.pop("max_provider_failovers", None)
        if supplied_failovers not in (None, 0):
            raise BrainRunError("approved model selection forbids provider failover")
        run_options.update(
            {
                "capability": capability,
                "risk_class": risk_class,
                "context": context,
                "bandit_state": bandit_state,
                "contextual_observations": contextual_observations,
                "required_model_capabilities": required_model_capabilities,
                "input_tokens": input_tokens,
                "requested_output_tokens": requested_output_tokens,
                "max_cost_per_million_tokens": max_cost_per_million_tokens,
                "max_latency_ms": max_latency_ms,
                "min_quality": min_quality,
                "selection_overrides": effective_selection_overrides,
                "selection_weights": supplied_weights,
                "selection_observations": supplied_observations,
                "approve_provider_call": True,
                "max_provider_failovers": 0,
            }
        )
        return self.run(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=selected_candidates,
            **run_options,
        )

    def run_approved_model_selection_with_launch_admission(
        self,
        *,
        task: str,
        domain: str,
        selection_preview: Mapping[str, Any],
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        capability: str | None = None,
        risk_class: str | None = None,
        context: Mapping[str, Any] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        contextual_observations: Sequence[Mapping[str, Any]] = (),
        required_model_capabilities: Sequence[str] = (),
        input_tokens: int = 4_096,
        requested_output_tokens: int = 2_048,
        max_cost_per_million_tokens: int | None = None,
        max_latency_ms: int | None = None,
        min_quality: float | None = None,
        min_selection_confidence: float | None = None,
        selection_overrides: Mapping[str, Any] | None = None,
        selection_weights: Mapping[str, Any] | None = None,
        selection_observations: Sequence[Mapping[str, Any]] | None = None,
        **kwargs: Any,
    ) -> Any:
        """Invoke an approved model arm only after its domain passes launch admission."""

        _authorize_launch_admission_domains(launch_admission, (domain,))
        return self.run_approved_model_selection(
            task=task,
            domain=domain,
            selection_preview=selection_preview,
            credentials=credentials,
            model_candidates=model_candidates,
            capability=capability,
            risk_class=risk_class,
            context=context,
            bandit_state=bandit_state,
            contextual_observations=contextual_observations,
            required_model_capabilities=required_model_capabilities,
            input_tokens=input_tokens,
            requested_output_tokens=requested_output_tokens,
            max_cost_per_million_tokens=max_cost_per_million_tokens,
            max_latency_ms=max_latency_ms,
            min_quality=min_quality,
            min_selection_confidence=min_selection_confidence,
            selection_overrides=selection_overrides,
            selection_weights=selection_weights,
            selection_observations=selection_observations,
            **kwargs,
        )

    def run_capability(
        self,
        *,
        task: str,
        domain: str,
        capability: str,
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        approve_capability: bool = False,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run one reviewed capability with stage-scoped tools and evidence instructions.

        This is the focused dispatch path used when an embedding application already knows
        the domain and wants the brain to make a capability-level decision.  It narrows the
        provider-visible tools to exact adapter aliases, adds the reviewed contract to the
        developer prompt, and still delegates provider approval, tool approval, and credential
        validation to the normal runtime boundary.
        """

        resolved_capability = _identifier("capability", capability)
        capability_plan = self.domain_capability_plan(
            domain,
            resolved_capability,
            model_candidates=model_candidates,
        )
        contract = capability_plan.get("contract")
        if not isinstance(contract, Mapping) or contract.get("capability") != resolved_capability:
            raise BrainRunError("capability dispatch contract is malformed")
        if not isinstance(approve_capability, bool):
            raise BrainRunError("approve_capability must be a boolean")
        if contract.get("approval_required") is True and not approve_capability:
            raise BrainRunError(
                f"capability {domain!r}/{resolved_capability!r} requires explicit capability approval"
            )
        context = kwargs.pop("context", None)
        if context is None:
            dispatch_context: dict[str, Any] = {}
        elif isinstance(context, Mapping):
            dispatch_context = dict(context)
        else:
            raise BrainRunError("context must be a mapping or None")
        if _AUTONOMOUS_CAPABILITY_CONTRACT_CONTEXT_KEY in dispatch_context:
            raise BrainRunError("context cannot override the autonomous capability contract")
        if _AUTONOMOUS_EXECUTION_PLAN_CONTEXT_KEY in dispatch_context:
            raise BrainRunError("context cannot override the autonomous execution plan")
        kwargs["context"] = dispatch_context
        kwargs["capability"] = resolved_capability
        required = kwargs.pop("required_model_capabilities", ())
        if not isinstance(required, Sequence) or isinstance(required, (str, bytes)):
            raise BrainRunError("required_model_capabilities must be a sequence")
        kwargs["required_model_capabilities"] = tuple(
            dict.fromkeys(
                (
                    *contract.get("required_model_capabilities", ()),
                    *required,
                )
            )
        )
        kwargs["_aurora_capability_focus"] = resolved_capability
        kwargs["_aurora_capability_contract"] = dict(contract)
        return self.run(
            task=task,
            domain=domain,
            credentials=credentials,
            model_candidates=model_candidates,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def run_capability_with_launch_admission(
        self,
        *,
        task: str,
        domain: str,
        capability: str,
        launch_admission: Mapping[str, Any],
        credentials: Mapping[str, CredentialHandle] | CredentialSession,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
        approve_capability: bool = False,
        execution_id: str | None = None,
        resume_execution: bool = False,
        **kwargs: Any,
    ) -> Any:
        """Run a reviewed capability only after its domain is launch-admitted.

        The admission check deliberately precedes capability-plan compilation, credential
        resolution, and provider/tool setup.  This makes the capability facade safe to call
        from a host process that has not yet collected credentials or opened external tool
        transports.
        """

        from .autonomous_launch_admission import authorize_autonomous_launch_domains

        authorize_autonomous_launch_domains(launch_admission, (domain,))
        return self.run_capability(
            task=task,
            domain=domain,
            capability=capability,
            credentials=credentials,
            model_candidates=model_candidates,
            approve_capability=approve_capability,
            execution_id=execution_id,
            resume_execution=resume_execution,
            **kwargs,
        )

    def execution_plans(
        self,
        domains: Sequence[str] | None = None,
        *,
        model_candidates: Sequence[ModelCandidate | Mapping[str, Any]] | None = None,
    ) -> dict[str, Any]:
        """Compile deterministic execution plans for one or more autonomous domains."""

        selected = tuple(AUTONOMOUS_DOMAINS) if domains is None else _sequence(
            "execution plan domains", domains, maximum=len(AUTONOMOUS_DOMAINS)
        )
        unknown = sorted(set(selected).difference(AUTONOMOUS_DOMAINS))
        if unknown:
            raise BrainRunError("execution plan contains unknown domains: " + ", ".join(unknown))
        plans = [
            self.domain_execution_plan(domain, model_candidates=model_candidates)
            for domain in selected
        ]
        statuses = {plan["status"] for plan in plans}
        aggregate_status = next(iter(statuses)) if len(statuses) == 1 else "multi_domain"
        return {
            "schema": AUTONOMOUS_EXECUTION_PLAN_SCHEMA,
            "status": aggregate_status,
            "plan_digest": content_digest(plans),
            "domains": list(selected),
            "domain_count": len(selected),
            "plans": plans,
            "catalogue_digest": content_digest(self.catalogue.candidates()),
            "domain_pack_registry_digest": self.orchestrator.pack_registry.digest,
            "activation_id": self.activation.state.activation_id,
            "execution": "planning_only; no_provider_or_tool_invocation",
            "authority_posture": "metadata_only; plans_do_not_grant_authority",
            "secret_material": "never_returned",
        }

    def register_tool(
        self,
        tool: AutonomousDomainTool,
        *,
        replace_existing: bool = False,
    ) -> AutonomousDomainTool:
        """Register one application-owned domain tool without accepting credentials."""

        if self.tool_registry is None:
            raise BrainRunError("register_tool requires an AutonomousDomainToolRegistry")
        if not isinstance(tool, AutonomousDomainTool):
            raise BrainRunError("register_tool accepts an AutonomousDomainTool value")
        registered = self.tool_registry.register(tool, replace_existing=replace_existing)
        if self.tool_runtime is None and hasattr(self.brain.workspace, "tool") and callable(getattr(self.brain.workspace, "tool")):
            self.tool_runtime = AutonomousDomainToolRuntime(
                self.tool_registry,
                executor=lambda resolved, arguments: self.brain.workspace.tool(resolved.name, dict(arguments)),
            )
            self.capability_runtime = AutonomousCapabilityRuntime(
                self.tool_runtime,
                journal=self.capability_journal,
            )
        return registered

    def plan_workspace_tool_bindings(
        self,
        catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition] | None = None,
        *,
        domains: Sequence[str] | None = None,
    ) -> dict[str, Any]:
        """Plan reviewed live-tool bindings without registering or executing anything.

        The plan intersects the workspace's authoritative ``tools/list`` snapshot with exact
        curated profiles for the selected domains.  Unknown tools remain unclassified and
        effectful tools remain review-only; a plan is never an authorization artifact.
        """

        if catalogue is None:
            catalogue_reader = getattr(self.brain.workspace, "tool_catalogue", None)
            if not callable(catalogue_reader):
                raise BrainRunError("plan_workspace_tool_bindings requires a catalogue or workspace.tool_catalogue()")
            catalogue = catalogue_reader()
        try:
            plan = plan_mcp_catalogue_bindings(catalogue, domains=domains)
            if self.activation.state.status != "revoked":
                self.activation.record_provider_statuses(self.onboarding.statuses())
                self.activation.record_binding_plan(plan)
            return plan
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("workspace tool binding plan failed") from error
        except AutonomousActivationError as error:
            raise BrainRunError("workspace tool binding activation plan could not be recorded") from error

    def register_workspace_bindings_from_plan(
        self,
        plan: Mapping[str, Any],
        approved_tools: Sequence[str],
        *,
        catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition] | None = None,
        replace_existing: bool = False,
    ) -> list[dict[str, Any]]:
        """Apply only caller-approved safe proposals from a fresh binding plan.

        The catalogue digest, profile digest, and selected binding rows are recomputed before
        registration.  This prevents a stale or hand-edited plan from changing the curated
        posture.  Applying a safe binding still only exposes a schema to the brain; the runtime
        callback, mission policy, and effect approval boundary remain authoritative.
        """

        if not isinstance(plan, Mapping) or plan.get("schema") != DOMAIN_TOOL_BINDING_PLAN_SCHEMA:
            raise BrainRunError("register_workspace_bindings_from_plan requires a valid binding plan")
        if not isinstance(approved_tools, Sequence) or isinstance(approved_tools, (str, bytes)):
            raise BrainRunError("approved_tools must be a non-empty sequence")
        if not approved_tools:
            raise BrainRunError("approved_tools must contain at least one tool")
        approved: list[str] = []
        seen: set[str] = set()
        for name in approved_tools:
            if not isinstance(name, str) or not name.strip():
                raise BrainRunError("approved tool names must be non-empty strings")
            if name in seen:
                raise BrainRunError(f"approved_tools contains a duplicate tool: {name}")
            seen.add(name)
            approved.append(name)
        raw_domains = plan.get("domains")
        if not isinstance(raw_domains, Sequence) or isinstance(raw_domains, (str, bytes)):
            raise BrainRunError("binding plan domains are missing or malformed")
        if catalogue is None:
            catalogue_reader = getattr(self.brain.workspace, "tool_catalogue", None)
            if not callable(catalogue_reader):
                raise BrainRunError("register_workspace_bindings_from_plan requires a catalogue or workspace.tool_catalogue()")
            catalogue = catalogue_reader()
        try:
            snapshot = catalogue if isinstance(catalogue, ToolCatalogue) else ToolCatalogue.from_definitions(catalogue)
            fresh_plan = plan_mcp_catalogue_bindings(snapshot, domains=tuple(raw_domains))
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("workspace tool binding plan could not be revalidated") from error
        if plan.get("catalogue_digest") != snapshot.digest:
            raise BrainRunError("workspace tool binding plan is stale: catalogue digest changed")
        if plan.get("profile_digest") != fresh_plan.get("profile_digest"):
            raise BrainRunError("workspace tool binding plan is stale: profile digest changed")
        proposed = plan.get("proposed_bindings")
        fresh_proposed = fresh_plan.get("proposed_bindings")
        if not isinstance(proposed, Mapping) or not isinstance(fresh_proposed, Mapping):
            raise BrainRunError("workspace tool binding plan has no proposed bindings")
        bindings: dict[str, Mapping[str, Any]] = {}
        for name in approved:
            row = proposed.get(name)
            fresh_row = fresh_proposed.get(name)
            if not isinstance(row, Mapping) or not isinstance(fresh_row, Mapping) or dict(row) != dict(fresh_row):
                raise BrainRunError(f"approved tool {name!r} is absent or does not match curated policy")
            if row.get("read_only") is not True or row.get("risk_class") != "read_only" or row.get("approval_required") is not False:
                raise BrainRunError(f"approved tool {name!r} is not a safe proposed binding")
            bindings[name] = row
        registered = self.register_workspace_tools(
            bindings,
            catalogue=snapshot,
            require_all=False,
            replace_existing=replace_existing,
        )
        try:
            self.activation.approve_bindings(
                plan,
                approved,
                registered_tool_count=0 if self.tool_registry is None else len(self.tool_registry.catalogue()),
            )
        except AutonomousActivationError as error:
            raise BrainRunError("approved workspace bindings could not be recorded") from error
        return registered

    def register_workspace_tools(
        self,
        bindings: Mapping[str, AutonomousDomainToolBinding | Mapping[str, Any]],
        *,
        catalogue: ToolCatalogue | Sequence[Mapping[str, Any] | ToolDefinition] | None = None,
        require_all: bool = True,
        replace_existing: bool = False,
    ) -> list[dict[str, Any]]:
        """Bind the workspace's live MCP catalogue into the autonomous tool registry.

        The workspace supplies authoritative schemas and the caller supplies every domain,
        capability, and risk decision. If no registry was provided at construction, this
        method creates one; registration still grants no execution authority. When the
        workspace exposes ``tool``, the default runtime is wired to that caller-owned adapter,
        preserving the existing approval and metadata-only receipt boundary.
        """

        if catalogue is None:
            catalogue_reader = getattr(self.brain.workspace, "tool_catalogue", None)
            if not callable(catalogue_reader):
                raise BrainRunError("register_workspace_tools requires a catalogue or workspace.tool_catalogue()")
            catalogue = catalogue_reader()
        if self.tool_registry is None:
            self.tool_registry = AutonomousDomainToolRegistry()
        try:
            registered = self.tool_registry.register_mcp_catalogue(
                catalogue,
                bindings,
                require_all=require_all,
                replace_existing=replace_existing,
            )
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("workspace tool binding failed") from error
        if self.tool_runtime is None and hasattr(self.brain.workspace, "tool") and callable(getattr(self.brain.workspace, "tool")):
            self.tool_runtime = AutonomousDomainToolRuntime(
                self.tool_registry,
                executor=lambda resolved, arguments: self.brain.workspace.tool(resolved.name, dict(arguments)),
            )
            self.capability_runtime = AutonomousCapabilityRuntime(
                self.tool_runtime,
                journal=self.capability_journal,
            )
        if self.activation.state.status != "revoked":
            try:
                self.activation.record_registered_tools(len(self.tool_registry.catalogue()))
            except AutonomousActivationError as error:
                raise BrainRunError("registered workspace tools could not be reflected in activation state") from error
        return [tool.to_dict() for tool in registered]

    def tools(self, domain: str | None = None) -> list[dict[str, Any]]:
        """Return metadata-only domain tools visible to a domain or to the full registry."""

        if self.tool_registry is None:
            return []
        return self.tool_registry.catalogue(None if domain is None else (domain,))

    def tool_receipts(self) -> list[dict[str, Any]]:
        """Return metadata-only receipts from the application-owned domain tool runtime."""

        if self.tool_runtime is None:
            return []
        return [receipt.to_dict() for receipt in self.tool_runtime.receipts]

    def execute_capability(
        self,
        request: Mapping[str, Any],
        *,
        project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None,
    ) -> AutonomousCapabilityExecutionResult:
        """Execute one reviewed capability through the approval-aware domain runtime.

        The request contains only caller-owned identifiers, bounded arguments, and workflow
        digests. The returned value is transient to this call; durable history and journals
        retain execution metadata, digests, evidence labels, and refusal/replay state only.
        """

        if self.capability_runtime is None:
            raise BrainRunError(
                "execute_capability requires a configured domain tool runtime and workspace executor"
            )
        try:
            return self.capability_runtime.execute(
                request,
                project_observations=project_observations,
            )
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("capability execution request was rejected") from error

    def execute_capability_with_launch_admission(
        self,
        request: Mapping[str, Any],
        *,
        launch_admission: Mapping[str, Any],
        project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None,
    ) -> AutonomousCapabilityExecutionResult:
        """Execute one capability only after its workflow domain passes launch admission."""

        _authorize_launch_admission_domains(
            launch_admission,
            _capability_request_domains_for_launch_admission(request),
        )
        return self.execute_capability(
            request,
            project_observations=project_observations,
        )

    def execute_capability_batch(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None,
        max_parallelism: int = 1,
    ) -> tuple[AutonomousCapabilityExecutionResult, ...]:
        """Execute a bounded capability batch in stable input order."""

        if self.capability_runtime is None:
            raise BrainRunError(
                "execute_capability_batch requires a configured domain tool runtime and workspace executor"
            )
        try:
            return self.capability_runtime.execute_batch(
                requests,
                project_observations=project_observations,
                max_parallelism=max_parallelism,
            )
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("capability batch was rejected") from error

    def execute_capability_batch_with_launch_admission(
        self,
        requests: Sequence[Mapping[str, Any]],
        *,
        launch_admission: Mapping[str, Any],
        project_observations: Callable[[Any, Mapping[str, Any]], Sequence[Mapping[str, Any]]] | None = None,
        max_parallelism: int = 1,
    ) -> tuple[AutonomousCapabilityExecutionResult, ...]:
        """Execute a capability batch only after every request domain is admitted."""

        requested_domains: list[str] = []
        for request in requests:
            requested_domains.extend(_capability_request_domains_for_launch_admission(request))
        _authorize_launch_admission_domains(launch_admission, tuple(requested_domains))
        return self.execute_capability_batch(
            requests,
            project_observations=project_observations,
            max_parallelism=max_parallelism,
        )

    def restore_capability_journal(self) -> dict[str, Any]:
        """Rehydrate committed capability replay identities from the configured journal."""

        if self.capability_runtime is None:
            raise BrainRunError("restore_capability_journal requires a configured capability runtime")
        return self.capability_runtime.rehydrate()

    def restore_capability_journal_persistence(self) -> dict[str, Any]:
        """Restore durable capability metadata, then open its replay barrier in this process."""

        if self.capability_journal_persistence is None:
            raise BrainRunError("restore_capability_journal_persistence requires configured persistence")
        persisted = self.capability_journal_persistence.restore()
        runtime = self.capability_runtime
        rehydrated = runtime.rehydrate() if runtime is not None else {
            "restored": 0,
            "replayable": 0,
            "value_retention": "transient_caller_value_only",
        }
        return {
            "schema": AUTONOMOUS_CAPABILITY_JOURNAL_SNAPSHOT_SCHEMA,
            **persisted,
            "rehydrated": rehydrated["restored"],
            "replayable": rehydrated["replayable"],
            "value_retention": rehydrated["value_retention"],
            "retention": "metadata_only;caller_values_not_restored",
        }

    def flush_capability_journal_persistence(self) -> dict[str, Any]:
        """Flush the capability replay barrier without returning journal entries."""

        if self.capability_journal_persistence is None:
            raise BrainRunError("flush_capability_journal_persistence requires configured persistence")
        return self.capability_journal_persistence.flush()

    def restore_decision_cycle_persistence(self) -> dict[str, Any]:
        """Restore metadata-only route/planning/evaluation checkpoints for all persisted cycles."""

        if self.decision_cycle_persistence is None:
            raise BrainRunError("restore_decision_cycle_persistence requires configured persistence")
        snapshot = self.decision_cycle_persistence.restore()
        if snapshot is None:
            return {
                "restored": False,
                "snapshot_digest": None,
                "cycles": 0,
                "terminal_cycles": 0,
                "retention": "metadata_only_hash_bound",
            }
        return {
            "restored": True,
            "schema": snapshot.schema,
            "snapshot_digest": snapshot.snapshot_digest,
            "cycles": len(snapshot.states),
            "terminal_cycles": sum(state.phase == "terminal" for state in snapshot.states),
            "retention": "metadata_only_hash_bound",
        }

    def flush_decision_cycle_persistence(self) -> dict[str, Any]:
        """Flush decision-cycle checkpoints while projecting no task or provider payloads."""

        if self.decision_cycle_persistence is None:
            raise BrainRunError("flush_decision_cycle_persistence requires configured persistence")
        snapshot = self.decision_cycle_persistence.flush()
        return {
            "schema": snapshot.schema,
            "snapshot_digest": snapshot.snapshot_digest,
            "cycles": len(snapshot.states),
            "terminal_cycles": sum(state.phase == "terminal" for state in snapshot.states),
            "retention": "metadata_only_hash_bound",
        }

    def capability_execution_evidence(self) -> list[dict[str, Any]]:
        """Return bounded metadata-only capability records for evaluator integration."""

        if self.capability_runtime is None:
            return []
        return self.capability_runtime.execution_evidence()

    def evaluate_capability_execution(
        self,
        result: AutonomousCapabilityExecutionResult,
        *,
        evaluator: AutonomousToolOutcomeEvaluator,
        evidence: Mapping[str, Any] | None = None,
        allow_reconciliation: bool = False,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
        tool_selection_state: Mapping[str, Any] | None = None,
    ) -> AutonomousToolLearningReport:
        """Settle one capability execution through independent evaluator evidence."""

        if not isinstance(evaluator, AutonomousToolOutcomeEvaluator):
            raise BrainRunError("evaluator must be an AutonomousToolOutcomeEvaluator")
        caller_provided_tool_selection_state = tool_selection_state is not None
        effective_tool_selection_state = tool_selection_state if caller_provided_tool_selection_state else (
            self.tool_selection_state if self._tool_selection_configured else None
        )
        try:
            report = evaluator.evaluate_capability_result(
                result,
                evidence=evidence,
                allow_reconciliation=allow_reconciliation,
                bandit_state=bandit_state,
                bandit_updater=bandit_updater,
                ledger=self.ledger if ledger is None else ledger,
                tool_selection_state=effective_tool_selection_state,
                tool_selection_updater=_update_autonomous_tool_selection_state,
            )
            next_state = report.get("next_tool_selection_state") if isinstance(report, Mapping) else None
            if not caller_provided_tool_selection_state and self._tool_selection_configured and isinstance(next_state, Mapping):
                self._set_tool_selection_state(next_state)
            return report
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("capability execution evaluation failed") from error

    def evaluate_capability_executions(
        self,
        results: Sequence[AutonomousCapabilityExecutionResult],
        *,
        evaluator: AutonomousToolOutcomeEvaluator,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        allow_reconciliation: bool = False,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
        tool_selection_state: Mapping[str, Any] | None = None,
    ) -> AutonomousToolLearningReport:
        """Settle an ordered capability-result batch through one evaluator/state stream."""

        if not isinstance(evaluator, AutonomousToolOutcomeEvaluator):
            raise BrainRunError("evaluator must be an AutonomousToolOutcomeEvaluator")
        caller_provided_tool_selection_state = tool_selection_state is not None
        effective_tool_selection_state = tool_selection_state if caller_provided_tool_selection_state else (
            self.tool_selection_state if self._tool_selection_configured else None
        )
        try:
            report = evaluator.evaluate_capability_results(
                results,
                evidence=evidence,
                allow_reconciliation=allow_reconciliation,
                bandit_state=bandit_state,
                bandit_updater=bandit_updater,
                ledger=self.ledger if ledger is None else ledger,
                tool_selection_state=effective_tool_selection_state,
                tool_selection_updater=_update_autonomous_tool_selection_state,
            )
            next_state = report.get("next_tool_selection_state") if isinstance(report, Mapping) else None
            if not caller_provided_tool_selection_state and self._tool_selection_configured and isinstance(next_state, Mapping):
                self._set_tool_selection_state(next_state)
            return report
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("capability execution batch evaluation failed") from error

    def evaluate_tool_receipts(
        self,
        *,
        evaluator: AutonomousToolOutcomeEvaluator,
        receipts: Sequence[AutonomousDomainToolReceipt] | None = None,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        bandit_state: Mapping[str, Any] | None = None,
        bandit_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
        ledger: BrainLearningLedger | None = None,
        tool_selection_state: Mapping[str, Any] | None = None,
    ) -> AutonomousToolLearningReport:
        """Score selected live tool receipts and return the next online-learning state.

        This is intentionally explicit: an executed tool call is transport evidence, not a
        quality reward.  The caller supplies an independent evaluator and optional safe evidence
        keyed by receipt ``call_id``.  The evaluator sees only domain/capability/risk metadata,
        digests, status, and that bounded evidence; it never receives tool arguments or outputs.
        """

        if self.tool_runtime is None:
            raise BrainRunError("evaluate_tool_receipts requires a configured domain tool runtime")
        if not isinstance(evaluator, AutonomousToolOutcomeEvaluator):
            raise BrainRunError("evaluator must be an AutonomousToolOutcomeEvaluator")
        selected = self.tool_runtime.receipts if receipts is None else tuple(receipts)
        if any(not isinstance(receipt, AutonomousDomainToolReceipt) for receipt in selected):
            raise BrainRunError("receipts must contain AutonomousDomainToolReceipt values")
        caller_provided_tool_selection_state = tool_selection_state is not None
        effective_tool_selection_state = tool_selection_state if caller_provided_tool_selection_state else (
            self.tool_selection_state if self._tool_selection_configured else None
        )
        try:
            report = evaluator.evaluate_receipts(
                selected,
                evidence=evidence,
                bandit_state=bandit_state,
                bandit_updater=bandit_updater,
                ledger=self.ledger if ledger is None else ledger,
                tool_selection_state=effective_tool_selection_state,
                tool_selection_updater=_update_autonomous_tool_selection_state,
            )
            next_state = report.get("next_tool_selection_state") if isinstance(report, Mapping) else None
            if not caller_provided_tool_selection_state and self._tool_selection_configured and isinstance(next_state, Mapping):
                self._set_tool_selection_state(next_state)
            return report
        except (ArgumentError, ValueError) as error:
            raise BrainRunError("domain tool receipt evaluation failed") from error

    def evaluate_provider_receipts(
        self,
        *,
        evaluator: AutonomousProviderOutcomeEvaluator,
        receipts: Sequence[Mapping[str, Any] | AutonomousProviderInvocationReceipt],
        contexts: Mapping[str, Mapping[str, Any]] | None = None,
        evidence: Mapping[str, Mapping[str, Any]] | None = None,
        learning_state: Mapping[str, Any] | None = None,
        learning_updater: Callable[[Mapping[str, Any], Mapping[str, Any], Mapping[str, Any]], Mapping[str, Any]] | None = None,
    ) -> AutonomousProviderLearningReport:
        """Evaluate redacted provider receipts and update explicit model-arm learning.

        Provider success is transport evidence only.  The independent evaluator is the sole
        source of reward, while the default updater applies the value-only result to the same
        portable contextual bandit state used by model selection.  Callers that persist a
        learning ledger should persist ``report.next_learning_state`` through their normal
        caller-owned snapshot/CAS boundary; no credential or provider payload is retained here.
        """

        if not isinstance(evaluator, AutonomousProviderOutcomeEvaluator):
            raise BrainRunError("evaluator must be an AutonomousProviderOutcomeEvaluator")
        if learning_updater is None:
            learning_updater = settle_autonomous_provider_model_outcome
        try:
            return evaluator.evaluate_receipts(
                receipts,
                contexts=contexts,
                evidence=evidence,
                learning_state=self.learning_state() if learning_state is None else learning_state,
                learning_updater=learning_updater,
            )
        except (ArgumentError, ValueError, TypeError) as error:
            raise BrainRunError("provider receipt evaluation failed") from error

    def plan_recovery(self, observation: Mapping[str, Any] | AutonomousRecoveryObservation) -> AutonomousRecoveryPlan:
        """Build a deterministic recovery plan from a caller-owned failure projection."""

        try:
            return plan_autonomous_recovery(observation)
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("autonomous recovery observation was rejected") from error

    def submit_recovery_handoff(
        self,
        ledger: AutonomousRecoveryHandoffLedger,
        observation: Mapping[str, Any] | AutonomousRecoveryObservation,
        *,
        run_id_digest: str,
        attempt: int = 0,
    ) -> dict[str, Any]:
        """Queue a recovery plan in caller-owned metadata without retrying or dispatching it."""

        if not isinstance(ledger, AutonomousRecoveryHandoffLedger):
            raise BrainRunError("autonomous recovery handoff requires an AutonomousRecoveryHandoffLedger")
        try:
            return ledger.submit(self.plan_recovery(observation), run_id_digest=run_id_digest, attempt=attempt)
        except (ArgumentError, TypeError, ValueError) as error:
            raise BrainRunError("autonomous recovery handoff was rejected") from error

    def readiness(
        self,
        *,
        selection_promotion_report: Mapping[str, Any] | None = None,
        require_promoted_selection: bool = False,
        evidence_readiness: Mapping[str, Any] | None = None,
        calibration_report: Mapping[str, Any] | None = None,
        calibration_report_digest: str | None = None,
    ) -> dict[str, Any]:
        """Project provider/model readiness plus optional evidence and calibration gates.

        ``evidence_readiness`` is a caller-owned configuration mapping with a typed adapter
        registry, an optional health store, and optional auditor options.  ``calibration_report``
        is a validated, aggregate-only evaluator report produced by
        :func:`calibrate_autonomous_evaluators`; ``calibration_report_digest`` resolves that
        report from the explicitly configured aggregate registry.  Supplying either option
        performs only a local projection; it never dispatches an evidence source, mutates
        learning, or invokes a provider.
        """

        if not isinstance(require_promoted_selection, bool):
            raise BrainRunError("require_promoted_selection must be a boolean")
        promotion_report = None if selection_promotion_report is None else validate_autonomous_selection_promotion_report(selection_promotion_report)
        promotion_state = None if self.selection_promotion is None else self.selection_promotion.state
        promotion_admitted = bool(
            promotion_state is not None
            and promotion_state.status == "admitted"
            and promotion_state.active_promotion_digest is not None
            and (promotion_report is None or promotion_state.active_promotion_digest == promotion_report["promotion_digest"])
        )
        promotion_blocks = require_promoted_selection and not promotion_admitted

        if calibration_report is not None and calibration_report_digest is not None:
            raise BrainRunError("provide calibration_report or calibration_report_digest, not both")
        stored_calibration_report = None
        if calibration_report_digest is not None:
            if self.evaluator_calibration_registry is None:
                raise BrainRunError("calibration_report_digest requires an evaluator calibration registry")
            try:
                stored_calibration_report = self.evaluator_calibration_registry.get(calibration_report_digest)
            except (ArgumentError, TypeError, ValueError) as error:
                raise BrainRunError("calibration_report_digest was rejected") from error
            if stored_calibration_report is None:
                raise BrainRunError("calibration_report_digest was not found in the registry")
        supplied_calibration_report = calibration_report if calibration_report is not None else stored_calibration_report
        evaluator_calibration_report = None
        if supplied_calibration_report is not None:
            try:
                evaluator_calibration_report = validate_autonomous_evaluator_calibration_report(supplied_calibration_report)
            except (ArgumentError, TypeError, ValueError) as error:
                raise BrainRunError("evaluator calibration report was rejected") from error
        calibration_by_domain = (
            {}
            if evaluator_calibration_report is None
            else {
                row["domain"]: row
                for row in evaluator_calibration_report["domains"]
            }
        )

        evidence_readiness_report = None
        if evidence_readiness is not None:
            from .autonomous_evidence_readiness import AutonomousLLMEvidenceReadinessAuditor

            if not isinstance(evidence_readiness, Mapping):
                raise BrainRunError("evidence_readiness must be a mapping or None")
            evidence_registry = evidence_readiness.get("registry")
            if evidence_registry is None:
                raise BrainRunError("evidence_readiness requires a typed adapter registry")
            evidence_health_store = evidence_readiness.get("health_store")
            evidence_options = evidence_readiness.get("options", {})
            if not isinstance(evidence_options, Mapping):
                raise BrainRunError("evidence_readiness options must be a mapping")
            try:
                evidence_readiness_report = AutonomousLLMEvidenceReadinessAuditor(
                    evidence_registry,
                    evidence_health_store,
                ).audit(AUTONOMOUS_DOMAINS, **dict(evidence_options))
            except (ArgumentError, TypeError, ValueError) as error:
                raise BrainRunError("evidence readiness audit was rejected") from error

        provider_names = {
            candidate["provider"]
            for candidate in self.catalogue.candidates()
            if isinstance(candidate.get("provider"), str)
        }
        providers_by_name = {
            row["provider"]: row
            for row in self.onboarding.statuses()
            if isinstance(row, Mapping) and isinstance(row.get("provider"), str)
        }
        for provider in sorted(provider_names.difference(providers_by_name)):
            providers_by_name[provider] = self.onboarding.status(provider)
        providers = [providers_by_name[provider] for provider in sorted(providers_by_name)]
        status_by_provider = {
            row["provider"]: row
            for row in providers
            if isinstance(row, Mapping) and isinstance(row.get("provider"), str)
        }
        catalogue_candidates = self.catalogue.candidates()
        models: list[dict[str, Any]] = []
        for candidate in catalogue_candidates:
            provider = candidate["provider"]
            provider_status = status_by_provider.get(provider, {})
            models.append(
                {
                    "provider": provider,
                    "model": candidate["model"],
                    "enabled": candidate.get("enabled", True),
                    "provider_registered": bool(provider_status.get("provider_registered", False)),
                    "credential_ready": bool(provider_status.get("ready", False)),
                    "eligible_for_selection": bool(candidate.get("enabled", True))
                    and bool(provider_status.get("ready", False)),
                }
            )
        health = {} if self.health_ledger is None else self.health_ledger.health_snapshot()
        for row in providers:
            provider = row.get("provider") if isinstance(row, Mapping) else None
            if isinstance(provider, str):
                row["health"] = dict(health.get(provider, {}))
        next_actions = sorted(
            {
                str(row.get("next_action"))
                for row in providers
                if isinstance(row, Mapping) and row.get("next_action") not in (None, "ready")
            }
        )
        if self.activation.state.status != "revoked":
            try:
                self.activation.record_provider_statuses(providers)
            except AutonomousActivationError as error:
                raise BrainRunError("activation provider readiness projection failed") from error
        domain_rows = self.domains()
        readiness_states: set[str] = set()
        for row in domain_rows:
            domain = row.get("domain") if isinstance(row, Mapping) else None
            domain_report = None if promotion_report is None else next((item for item in promotion_report["domains"] if item.get("domain") == domain), None)
            if not isinstance(row, dict):
                continue
            profile = self.orchestrator.registry.resolve(domain) if isinstance(domain, str) else None
            required_capabilities = () if profile is None else tuple(profile.required_model_capabilities)
            compatible_candidates = [
                candidate
                for candidate in catalogue_candidates
                if candidate.get("enabled", True)
                and all(capability in candidate.get("capabilities", ()) for capability in required_capabilities)
            ]
            eligible_candidates = [
                candidate
                for candidate in compatible_candidates
                if (
                    status_by_provider.get(candidate["provider"], {}).get("provider_registered", False)
                    and status_by_provider.get(candidate["provider"], {}).get("ready", False)
                    and status_by_provider.get(candidate["provider"], {}).get("circuit", "closed") != "open"
                )
            ]
            provider_missing = any(
                not status_by_provider.get(candidate["provider"], {}).get("provider_registered", False)
                for candidate in compatible_candidates
            )
            credential_missing = any(
                status_by_provider.get(candidate["provider"], {}).get("provider_registered", False)
                and not status_by_provider.get(candidate["provider"], {}).get("ready", False)
                for candidate in compatible_candidates
            )
            base_state = (
                "model_catalogue_required"
                if not catalogue_candidates
                else "model_capability_gap"
                if not compatible_candidates
                else "ready_for_caller_approval"
                if eligible_candidates
                else "credential_required"
                if credential_missing
                else "provider_registration_required"
                if provider_missing
                else "partial"
            )
            evidence_row = None
            if evidence_readiness_report is not None:
                evidence_row = next(
                    (item for item in evidence_readiness_report.domains if item.domain == domain),
                    None,
                )
                if evidence_row is None:
                    raise BrainRunError(f"evidence readiness report does not cover domain: {domain}")
            evidence_blocks = evidence_row is not None and evidence_row.status != "ready"
            calibration_row = calibration_by_domain.get(domain)
            calibration_blocks = evaluator_calibration_report is not None and (
                evaluator_calibration_report["status"] != "ready"
                or calibration_row is None
                or calibration_row.get("status") != "ready"
            )
            if promotion_blocks or (evidence_blocks and base_state == "ready_for_caller_approval") or (
                calibration_blocks and base_state == "ready_for_caller_approval"
            ):
                state = "partial"
            else:
                state = base_state
            readiness_states.add(state)
            row.update(
                {
                    "required_model_capabilities": list(required_capabilities),
                    "compatible_model_count": len(compatible_candidates),
                    "eligible_model_count": len(eligible_candidates),
                    "state": state,
                }
            )
            if evidence_row is not None:
                row["evidence_readiness"] = {
                    "status": evidence_row.status,
                    "coverage_state": evidence_row.coverage_state,
                    "selected_adapter_id": evidence_row.selected_adapter_id,
                    "selected_manifest_digest": evidence_row.selected_manifest_digest,
                    "candidate_count": evidence_row.candidate_count,
                    "eligible_candidate_count": evidence_row.eligible_candidate_count,
                    "selection_reason": evidence_row.selection_reason,
                    "selection_strategy": evidence_row.selection_strategy,
                    "health": evidence_row.health.to_dict(),
                    "failover_policy_digest": evidence_row.failover_policy_digest,
                    "reason": evidence_row.reason,
                    "report_digest": evidence_readiness_report.report_digest,
                    "execution": "readiness_projection_only;does_not_dispatch_source",
                    "secret_material": "never_returned",
                }
            if calibration_row is not None:
                row["evaluator_calibration"] = {
                    "status": calibration_row["status"],
                    "evaluator_id": calibration_row["evaluator_id"],
                    "evaluator_version": calibration_row["evaluator_version"],
                    "pass_threshold": calibration_row["pass_threshold"],
                    "calibration_scored_count": calibration_row["calibration"]["scored_count"],
                    "holdout_scored_count": calibration_row["holdout"]["scored_count"],
                    "holdout_expected_calibration_error": calibration_row["holdout"]["expected_calibration_error"],
                    "holdout_brier_score": calibration_row["holdout"]["brier_score"],
                    "case_set_digest": calibration_row["case_set_digest"],
                    "evaluation_digest": calibration_row["evaluation_digest"],
                    "report_digest": evaluator_calibration_report["report_digest"],
                    "execution": "readiness_projection_only;no_learning_mutation",
                    "secret_material": "never_returned",
                }
            row_next_actions = set(row.get("next_actions", ()))
            if state == "model_catalogue_required":
                row_next_actions.add("register at least one model candidate with the reviewed domain capabilities")
            elif state == "model_capability_gap":
                row_next_actions.add(
                    "register a model declaring: " + ", ".join(required_capabilities)
                )
            elif state == "provider_registration_required":
                row_next_actions.add("register the provider transport before requesting a credential")
            elif state == "credential_required":
                row_next_actions.add("collect a short-lived user credential through ProviderOnboarding")
            if promotion_blocks:
                row_next_actions.add(
                    "attach and apply an admitted all-domain selection promotion report before enabling learned selection"
                    if promotion_state is None
                    else f"resolve selection promotion lifecycle hold: {promotion_state.last_reason or promotion_state.status}"
                )
            if evidence_row is not None and evidence_row.status != "ready":
                row_next_actions.add(
                    "resolve evidence routing readiness before source dispatch: " + evidence_row.reason
                )
            if calibration_blocks:
                row_next_actions.add(
                    "hold learned evaluator updates until calibration and independent holdout evidence are ready"
                )
            row["next_actions"] = sorted(row_next_actions)
            row["selection_promotion"] = {
                "decision": "admit" if promotion_admitted else promotion_report["decision"] if promotion_report is not None else "hold",
                "status": promotion_state.status if promotion_state is not None else "unconfigured",
                "promotion_digest": promotion_report["promotion_digest"] if promotion_report is not None else None if promotion_state is None else promotion_state.promotion_digest,
                "active_promotion_digest": None if promotion_state is None else promotion_state.active_promotion_digest,
                "source_report_digest": promotion_report["source_report_digest"] if promotion_report is not None else None if promotion_state is None else promotion_state.source_report_digest,
                "domain_decision": None if domain_report is None else domain_report["decision"],
                "reasons": list(domain_report["reasons"]) if domain_report is not None else [] if promotion_state is None or promotion_state.last_reason is None else [promotion_state.last_reason],
                "execution": "readiness_projection_only;does_not_mutate_learner_or_invoke_provider",
                "secret_material": "never_returned",
            }
        for row in domain_rows:
            if isinstance(row, Mapping):
                next_actions.extend(
                    action
                    for action in row.get("next_actions", ())
                    if isinstance(action, str)
                )
        if promotion_blocks:
            next_actions.append(
                "attach and apply an admitted all-domain selection promotion report before enabling learned selection"
                if promotion_state is None
                else f"resolve selection promotion lifecycle hold: {promotion_state.last_reason or promotion_state.status}"
            )
        learning = self.domain_learning_coverage()
        if evaluator_calibration_report is not None:
            learning["evaluator_calibration"] = {
                "configured": True,
                "report_digest": evaluator_calibration_report["report_digest"],
                "status": evaluator_calibration_report["status"],
                "decision": evaluator_calibration_report["gate"]["decision"],
                "missing_domains": list(evaluator_calibration_report["missing_domains"]),
                "non_ready_domains": list(evaluator_calibration_report["gate"]["non_ready_domains"]),
                "execution": "readiness_projection_only;no_learning_mutation",
                "secret_material": "never_returned",
            }
            for learning_row in learning["rows"]:
                calibration_row = calibration_by_domain.get(learning_row["domain"])
                learning_row["calibration_status"] = None if calibration_row is None else calibration_row["status"]
                learning_row["calibration_report_digest"] = evaluator_calibration_report["report_digest"]
                learning_row["calibration_admit_learning"] = bool(
                    evaluator_calibration_report["gate"]["decision"] == "admit_learning"
                    and calibration_row is not None
                    and calibration_row["status"] == "ready"
                )
        else:
            learning["evaluator_calibration"] = {
                "configured": False,
                "report_digest": None,
                "status": "unconfigured",
                "decision": "hold_learning",
                "missing_domains": [],
                "non_ready_domains": [],
                "execution": "readiness_projection_only;no_learning_mutation",
                "secret_material": "never_returned",
            }
        model_inventory_readiness = self.model_inventory_readiness()
        learning["selection_promotion"] = {
            "configured": promotion_report is not None or promotion_state is not None,
            "required": require_promoted_selection,
            "report_digest": None if promotion_report is None else promotion_report["promotion_digest"],
            "source_report_digest": promotion_report["source_report_digest"] if promotion_report is not None else None if promotion_state is None else promotion_state.source_report_digest,
            "lifecycle_status": "unconfigured" if promotion_state is None else promotion_state.status,
            "active_promotion_digest": None if promotion_state is None else promotion_state.active_promotion_digest,
            "decision": "admit" if promotion_admitted else promotion_report["decision"] if promotion_report is not None else "hold",
            "admitted_domain_count": 0 if promotion_report is None else sum(row["decision"] == "admit" for row in promotion_report["domains"]),
            "held_domain_count": len(AUTONOMOUS_DOMAINS) if promotion_report is None and promotion_blocks else 0 if promotion_report is None else sum(row["decision"] == "hold" for row in promotion_report["domains"]),
        }
        if promotion_blocks:
            readiness_state = "partial"
        elif len(readiness_states) == 1:
            readiness_state = next(iter(readiness_states))
        else:
            readiness_state = "partial"
        result = {
            "schema": "bioprism-autonomous-agent-readiness/0.1",
            "providers": providers,
            "models": models,
            "credential_provisioning": self.credential_provisioning_plan(
                tuple(sorted(provider_names))
            ),
            "provider_health": health,
            "model_inventory_readiness": model_inventory_readiness,
            "domains": domain_rows,
            "model_capability_coverage": self.model_capability_coverage(),
            "domain_learning_coverage": learning,
            "evaluator_calibration": None if evaluator_calibration_report is None else {
                "report_digest": evaluator_calibration_report["report_digest"],
                "status": evaluator_calibration_report["status"],
                "decision": evaluator_calibration_report["gate"]["decision"],
                "missing_domains": list(evaluator_calibration_report["missing_domains"]),
                "non_ready_domains": list(evaluator_calibration_report["gate"]["non_ready_domains"]),
                "execution": "readiness_projection_only;no_learning_mutation",
                "secret_material": "never_returned",
            },
            "workflows": self.workflows(),
            "domain_packs": self.domain_packs(),
            "domain_pack_registry_digest": self.orchestrator.pack_registry.digest,
            "domain_pack_tool_plans": [
                self.domain_pack_tool_plan(domain)
                for domain in AUTONOMOUS_DOMAINS
            ],
            "domain_execution_plans": self.execution_plans()["plans"],
            "domain_capability_plans": self.capability_plans()["plans"],
            "route_catalogue": self.orchestrator.router.catalogue(),
            "semantic_routing": {
                "schema": AUTONOMOUS_SEMANTIC_ROUTE_SCHEMA,
                "plan_refinement_schema": AUTONOMOUS_PLAN_REFINEMENT_SCHEMA,
                "enabled": True,
                "domain_count": len(AUTONOMOUS_DOMAINS),
                "requires_caller_provider_approval": True,
                "transcript_retention": "classifier_transcript_not_retained",
                "authorization": "routing_evidence_only; no_tools_or_effects_authorized",
            },
            "domain_tools": [] if self.tool_registry is None else self.tool_registry.catalogue(),
            "domain_tool_registry_digest": None if self.tool_registry is None else self.tool_registry.digest,
            "activation": self.activation.to_dict(),
            "next_actions": sorted(set(next_actions)),
            "readiness_state": readiness_state,
            "secret_material": "never_returned",
            "credential_posture": "caller_supplied_opaque_handles",
        }
        if evidence_readiness_report is not None:
            result["evidence"] = {
                "configured": True,
                "registry_digest": evidence_readiness_report.registry_digest,
                "report_digest": evidence_readiness_report.report_digest,
                "status": evidence_readiness_report.status,
                "complete": evidence_readiness_report.complete,
                "ready_count": evidence_readiness_report.ready_count,
                "degraded_count": evidence_readiness_report.degraded_count,
                "blocked_count": evidence_readiness_report.blocked_count,
                "missing_count": evidence_readiness_report.missing_count,
                "domains": [row.to_dict() for row in evidence_readiness_report.domains],
                "execution": "readiness_projection_only;no_source_dispatch",
                "secret_material": "never_returned",
            }
            if evidence_readiness_report.status != "ready":
                result["next_actions"] = sorted(
                    {
                        *result["next_actions"],
                        "resolve evidence routing readiness before source dispatch",
                    }
                )
        return result

# Keep method introspection compatible with the public AutonomousAgent class.
for _name, _descriptor in vars(AutonomousAgentCapabilitiesMixin).items():
    _function = _descriptor.__func__ if isinstance(_descriptor, staticmethod) else _descriptor
    if callable(_function) and hasattr(_function, "__code__"):
        _function.__module__ = "prism_sdk.autonomy"
        _function.__qualname__ = f"AutonomousAgent.{_name}"
