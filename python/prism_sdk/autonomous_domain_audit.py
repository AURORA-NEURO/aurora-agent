"""Provider-free audit of the reviewed autonomous domain surface.

The autonomous runtime has several deliberately separate registries: domain profiles describe
reasoning posture, workflow strategies describe the stage DAG, tool profiles describe exact
adapter bindings, and the evidence planner describes what must be observed before a result can
be treated as complete.  This module joins those contracts into one bounded handoff that can be
shown to an operator before routing, model selection, source acquisition, or tool dispatch.

The audit is structural rather than epistemic.  It cannot establish that a source is true, that
a provider is available, or that a tool is safe merely because its declaration looks correct.
It does make missing contracts, dependency errors, tool coverage, and caller-supplied evidence
coverage explicit for every built-in domain.  Report projections contain only metadata and
digests; task text, prompts, credentials, provider values, tool arguments, and evidence bodies
never enter the report.
"""

from __future__ import annotations

import json
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_evidence import build_autonomous_evidence_plan
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES, builtin_autonomous_domain_tool_profiles
from .errors import ArgumentError


AUTONOMOUS_DOMAIN_AUDIT_SCHEMA = "bioprism-python-autonomous-domain-audit/0.1"
AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA = "bioprism-python-autonomous-domain-audit-row/0.1"
MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES = 512_000
MAX_AUTONOMOUS_DOMAIN_AUDIT_ISSUES = 256

_RETENTION = "metadata_only;profile_payloads_and_runtime_values_not_retained"
_EXECUTION = "audit_only;no_provider_source_tool_queue_or_credential_dispatch"
_PROFILE_SCHEMA = "bioprism-python-autonomous-task/0.1"
_RUNTIME_STATUSES = ("unassessed", "ready_for_review", "partial", "blocked")
_CONTRACT_STATUSES = ("valid", "invalid")
_SEVERITIES = ("blocking", "warning")
_EVIDENCE_STATUSES = ("not_evaluated", "missing", "partial", "complete", "unassessed")


def _field(value: Any, name: str, default: Any = None) -> Any:
    if isinstance(value, Mapping):
        return value.get(name, default)
    return getattr(value, name, default)


def _bounded_text(name: str, value: Any, maximum: int = 2_048) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} is outside its bounded text contract")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return value


def _bounded_digest(name: str, value: Any) -> str:
    value = _bounded_text(name, value, 64)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _unique_strings(name: str, value: Any, *, maximum: int) -> tuple[str, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a bounded sequence")
    if len(value) > maximum:
        raise ArgumentError(f"{name} exceeds its bound")
    result: list[str] = []
    seen: set[str] = set()
    for item in value:
        text = _bounded_text(f"{name} entry", item, 512)
        if text in seen:
            raise ArgumentError(f"{name} contains a duplicate entry: {text}")
        seen.add(text)
        result.append(text)
    return tuple(sorted(result))


def _descriptor(value: Any, *, remove: Sequence[str] = ()) -> dict[str, Any]:
    """Return the canonical metadata descriptor for a registry value."""

    for method_name in ("descriptor", "to_dict"):
        method = getattr(value, method_name, None)
        if callable(method):
            try:
                result = method()
            except Exception:
                result = None
            if isinstance(result, Mapping):
                result = dict(result)
                for key in remove:
                    result.pop(key, None)
                return result
    if isinstance(value, Mapping):
        result = dict(value)
        for key in remove:
            result.pop(key, None)
        return result
    return {}


def _json_clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False))
    except (TypeError, ValueError) as error:
        raise ArgumentError("autonomous domain audit report is not canonical JSON") from error


def _issue(code: str, severity: str, message: str, next_action: str) -> dict[str, str]:
    if severity not in _SEVERITIES:
        raise ArgumentError("autonomous domain audit issue severity is invalid")
    return {
        "code": _bounded_text("domain audit issue code", code, 128),
        "severity": severity,
        "message": _bounded_text("domain audit issue message", message),
        "next_action": _bounded_text("domain audit issue next action", next_action, 1_024),
    }


def _unique_sorted(values: Sequence[str]) -> list[str]:
    return sorted(set(values))


def _normalize_registry_values(
    name: str,
    values: Any,
    *,
    maximum: int,
    default_factory: Any,
) -> tuple[Any, ...]:
    if values is None:
        values = default_factory()
    elif hasattr(values, "_profiles"):
        values = tuple(values._profiles.values())
    elif hasattr(values, "_strategies"):
        values = tuple(values._strategies.values())
    if isinstance(values, (str, bytes)) or not isinstance(values, Sequence):
        raise ArgumentError(f"{name} must be a sequence")
    if not 1 <= len(values) <= maximum:
        raise ArgumentError(f"{name} must contain between 1 and {maximum} entries")
    return tuple(values)


def _normalize_optional_strings(name: str, values: Sequence[str] | None) -> tuple[str, ...] | None:
    if values is None:
        return None
    if isinstance(values, (str, bytes, bytearray)) or not isinstance(values, Sequence) or len(values) > 4_096:
        raise ArgumentError(f"{name} is outside its bounded sequence contract")
    return tuple(sorted({
        _bounded_text(f"{name} entry", item, 512)
        for item in values
    }))


def _profile_map(profiles: Sequence[Any]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for profile in profiles:
        domain = _field(profile, "domain")
        if not isinstance(domain, str) or domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("domain audit profile domain is unsupported")
        if domain in result:
            raise ArgumentError("domain audit profiles must use unique domains")
        result[domain] = profile
    return result


def _workflow_map(workflows: Sequence[Any]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for workflow in workflows:
        domain = _field(workflow, "domain")
        if isinstance(domain, str) and domain in AUTONOMOUS_DOMAIN_NAMES and domain not in result:
            result[domain] = workflow
    return result


def _tool_profile_map(profiles: Sequence[Any]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for profile in profiles:
        domain = _field(profile, "domain")
        if isinstance(domain, str) and domain in AUTONOMOUS_DOMAIN_NAMES and domain not in result:
            result[domain] = profile
    return result


def _stage_list(workflow: Any) -> list[Any]:
    stages = _field(workflow, "stages", ())
    if isinstance(stages, (str, bytes)) or not isinstance(stages, Sequence):
        return []
    return list(stages)


def _audit_workflow(domain: str, workflow: Any | None, issues: list[dict[str, str]]) -> tuple[str, str, list[str]]:
    if workflow is None:
        issues.append(_issue("workflow_missing", "blocking", "domain has no reviewed workflow strategy", "register a workflow strategy before enabling autonomous dispatch"))
        return f"missing-{domain}", content_digest({"missing_workflow": domain}), []

    workflow_domain = _field(workflow, "domain")
    if workflow_domain != domain:
        issues.append(_issue("workflow_domain_mismatch", "blocking", "workflow domain does not match its profile domain", "rebuild the reviewed domain profile and workflow as one aligned contract"))
    workflow_id = _field(workflow, "workflow_id")
    if not isinstance(workflow_id, str) or not workflow_id.strip():
        issues.append(_issue("workflow_identity_missing", "blocking", "workflow identity is incomplete", "rebuild the workflow and assign a stable workflow id"))
        workflow_id = f"missing-{domain}"

    descriptor = _descriptor(workflow, remove=("workflow_digest", "execution"))
    expected_digest = content_digest(descriptor if descriptor else {"domain": domain, "workflow_id": workflow_id})
    supplied_digest = _field(workflow, "workflow_digest")
    workflow_digest = supplied_digest if isinstance(supplied_digest, str) and len(supplied_digest) == 64 else expected_digest
    if not isinstance(supplied_digest, str) or len(supplied_digest) != 64:
        issues.append(_issue("workflow_digest_missing", "blocking", "workflow digest is missing or malformed", "rebuild the workflow and recompute its digest"))
    elif supplied_digest != expected_digest:
        issues.append(_issue("workflow_digest_drift", "blocking", "workflow digest does not match its canonical descriptor", "recompute the workflow digest before accepting the handoff"))

    stages = _stage_list(workflow)
    if not 1 <= len(stages) <= 64:
        issues.append(_issue("workflow_stage_count", "blocking", "workflow must contain between one and sixty-four stages", "define a bounded reviewed workflow stage graph"))
        return workflow_id, workflow_digest, []
    stage_ids = [_field(stage, "id") for stage in stages]
    if any(not isinstance(stage_id, str) or not stage_id.strip() for stage_id in stage_ids) or len(set(stage_ids)) != len(stage_ids):
        issues.append(_issue("workflow_stage_identity", "blocking", "workflow stage identifiers must be non-empty and unique", "give every stage a stable unique identifier"))
    known = {stage_id for stage_id in stage_ids if isinstance(stage_id, str)}
    indegree = {stage_id: 0 for stage_id in known}
    outgoing: dict[str, list[str]] = {stage_id: [] for stage_id in known}
    for stage in stages:
        stage_id = _field(stage, "id", "unknown-stage")
        capabilities = _field(stage, "required_capabilities", ())
        evidence_outputs = _field(stage, "evidence_outputs", ())
        evaluator_signals = _field(stage, "evaluator_signals", ())
        dependencies = _field(stage, "depends_on", ())
        if not isinstance(capabilities, Sequence) or isinstance(capabilities, (str, bytes)) or not capabilities:
            issues.append(_issue("stage_capability_contract", "blocking", f"workflow stage {stage_id} has no required capability contract", "bind every stage to at least one reviewed capability"))
        if not isinstance(evidence_outputs, Sequence) or isinstance(evidence_outputs, (str, bytes)) or not evidence_outputs:
            issues.append(_issue("stage_evidence_contract", "blocking", f"workflow stage {stage_id} has no evidence output contract", "declare evidence outputs for every stage"))
        if not isinstance(evaluator_signals, Sequence) or isinstance(evaluator_signals, (str, bytes)) or not evaluator_signals:
            issues.append(_issue("stage_evaluator_contract", "blocking", f"workflow stage {stage_id} has no evaluator signal contract", "declare at least one evaluator signal for every stage"))
        if not isinstance(dependencies, Sequence) or isinstance(dependencies, (str, bytes)):
            issues.append(_issue("workflow_dependency_closure", "blocking", f"workflow stage {stage_id} has malformed dependencies", "close every dependency against the reviewed workflow graph"))
            dependencies = ()
        for dependency in dependencies:
            if dependency == stage_id or dependency not in known:
                issues.append(_issue("workflow_dependency_closure", "blocking", f"workflow stage {stage_id} depends on an unknown or self-referential stage", "close every dependency against the reviewed workflow graph"))
            elif stage_id in indegree and dependency in outgoing:
                indegree[stage_id] += 1
                outgoing[dependency].append(stage_id)
        if _field(stage, "read_only", True) is False and _field(stage, "approval_required", False) is not True:
            issues.append(_issue("effect_approval_contract", "blocking", f"workflow stage {stage_id} permits effects without an approval gate", "require explicit approval before any non-read-only stage"))
    queue = [stage_id for stage_id in known if indegree[stage_id] == 0]
    visited = 0
    while queue:
        current = queue.pop(0)
        visited += 1
        for child in outgoing[current]:
            indegree[child] -= 1
            if indegree[child] == 0:
                queue.append(child)
    if visited != len(known):
        issues.append(_issue("workflow_dependency_cycle", "blocking", "workflow stage dependencies contain a cycle", "replace the cycle with a directed acyclic execution graph"))
    route_intents = _field(workflow, "route_intents", ())
    workflow_signals = _field(workflow, "evaluator_signals", ())
    if not isinstance(route_intents, Sequence) or isinstance(route_intents, (str, bytes)) or not route_intents:
        issues.append(_issue("workflow_route_intents", "blocking", "workflow does not declare routing intents", "declare the task families this workflow can handle"))
    if not isinstance(workflow_signals, Sequence) or isinstance(workflow_signals, (str, bytes)) or not workflow_signals:
        issues.append(_issue("workflow_evaluator_signals", "blocking", "workflow does not declare evaluator signals", "declare the workflow-level completion signals"))
    if not isinstance(_field(workflow, "completion_contract"), str) or not _field(workflow, "completion_contract").strip():
        issues.append(_issue("workflow_completion_contract", "blocking", "workflow does not declare a completion contract", "define what evidence is required before the workflow can claim completion"))
    return workflow_id, workflow_digest, [stage_id for stage_id in stage_ids if isinstance(stage_id, str)]


def _audit_profile(profile: Any, domain: str, issues: list[dict[str, str]]) -> str:
    descriptor = _descriptor(profile)
    if descriptor.get("schema") != _PROFILE_SCHEMA:
        issues.append(_issue("profile_schema", "blocking", "profile schema is not the reviewed autonomy schema", "rebuild the profile through the reviewed built-in profile factory"))
    required = _field(profile, "required_model_capabilities", ())
    if not isinstance(required, Sequence) or isinstance(required, (str, bytes)) or not required:
        issues.append(_issue("model_capability_contract", "blocking", "domain has no required model capabilities", "declare the model capabilities needed to serve this domain"))
    elif len(set(required)) != len(required):
        issues.append(_issue("model_capability_duplicates", "blocking", "domain model capabilities contain duplicates", "deduplicate required model capabilities"))
    capabilities = _field(profile, "capabilities", ())
    if not isinstance(capabilities, Sequence) or isinstance(capabilities, (str, bytes)) or not capabilities:
        issues.append(_issue("domain_capability_catalogue", "blocking", "domain has no capability catalogue", "declare the domain capabilities used by routing and planning"))
    elif _field(profile, "default_capability") not in capabilities:
        issues.append(_issue("default_capability_unlisted", "blocking", "default capability is not present in the domain capability catalogue", "add the default capability or select a declared one"))
    guardrails = _field(profile, "guardrails", ())
    if not isinstance(guardrails, Sequence) or isinstance(guardrails, (str, bytes)) or not guardrails:
        issues.append(_issue("guardrail_contract", "blocking", "domain has no guardrails", "declare domain-specific safety and epistemic guardrails"))
    if not isinstance(_field(profile, "system_instructions"), str) or not _field(profile, "system_instructions").strip():
        issues.append(_issue("system_instruction_contract", "blocking", "domain has no system instruction contract", "define bounded domain instructions for provider prompting"))
    if not isinstance(_field(profile, "evaluator_domain"), str) or not _field(profile, "evaluator_domain").strip():
        issues.append(_issue("evaluator_domain_contract", "blocking", "domain has no evaluator-domain binding", "bind the domain to a reviewed evaluator profile"))
    return content_digest(descriptor if descriptor else {"domain": domain})


def _binding_rows(tool_profile: Any) -> list[Any]:
    bindings = _field(tool_profile, "bindings", ()) if tool_profile is not None else ()
    if isinstance(bindings, (str, bytes)) or not isinstance(bindings, Sequence):
        return []
    return list(bindings)


def _binding_supports_stage(domain: str, stage: Any, binding: Any) -> bool:
    binding_capability = _field(binding, "capability")
    aliases: Mapping[str, Mapping[str, Sequence[str]]] = {}
    try:
        from .autonomy import _AUTONOMOUS_CAPABILITY_TOOL_ALIASES

        aliases = _AUTONOMOUS_CAPABILITY_TOOL_ALIASES
    except Exception:
        aliases = {}
    required = _field(stage, "required_capabilities", ())
    if not isinstance(required, Sequence) or isinstance(required, (str, bytes)):
        return False
    return any(
        binding_capability == capability
        or binding_capability in aliases.get(domain, {}).get(capability, ())
        for capability in required
    )


def _audit_tools(domain: str, workflow: Any | None, tool_profile: Any | None, available: tuple[str, ...] | None, issues: list[dict[str, str]]) -> dict[str, Any]:
    if tool_profile is None:
        issues.append(_issue("tool_profile_contract", "blocking", "domain tool profile is missing", "register a reviewed exact-name domain tool profile"))
    bindings = _binding_rows(tool_profile)
    names = [_field(binding, "name") for binding in bindings]
    if any(not isinstance(name, str) or not name.strip() for name in names) or len(set(names)) != len(names):
        issues.append(_issue("tool_binding_duplicates", "blocking", "domain tool profile contains missing or duplicate binding names", "give every domain tool binding a unique stable name"))
    for binding in bindings:
        binding_name = _field(binding, "name", "unknown-tool")
        domains = _field(binding, "domains", ())
        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)) or domain not in domains:
            issues.append(_issue("tool_binding_domain", "blocking", f"tool {binding_name} is not bound to its profile domain", "attach each tool binding to the domain that can review it"))
        read_only = _field(binding, "read_only", True)
        approval_required = _field(binding, "approval_required", False)
        if not isinstance(read_only, bool) or not isinstance(approval_required, bool):
            issues.append(_issue("tool_safety_flags", "blocking", f"tool {binding_name} has malformed safety flags", "declare boolean read-only and approval posture"))
        elif not read_only and not approval_required:
            issues.append(_issue("tool_effect_without_approval", "blocking", f"effectful tool {binding_name} has no approval requirement", "require approval for every non-read-only tool binding"))
    stage_gaps: list[str] = []
    if workflow is not None:
        for stage in _stage_list(workflow):
            for capability in _field(stage, "required_capabilities", ()) if isinstance(_field(stage, "required_capabilities", ()), Sequence) else ():
                if not any(_binding_supports_stage(domain, stage, binding) for binding in bindings):
                    stage_gaps.append(str(capability))
    stage_gaps = _unique_sorted(stage_gaps)
    if stage_gaps:
        issues.append(_issue("stage_tool_capability_gap", "warning", "some workflow capabilities have no reviewed tool binding: " + ", ".join(stage_gaps), "attach a reviewed tool adapter or explicitly accept provider-only stage execution"))
    missing = [] if available is None else [name for name in names if name not in available]
    return {
        "assessed": available is not None,
        "declared_tool_count": len(bindings),
        "available_tool_count": None if available is None else len([name for name in names if name in available]),
        "missing_tool_names": sorted(set(missing)),
        "read_only_tool_count": sum(_field(binding, "read_only", True) is True for binding in bindings),
        "approval_required_tool_count": sum(_field(binding, "approval_required", False) is True for binding in bindings),
        "exact_stage_capability_gaps": stage_gaps,
    }


def _audit_evidence(domain: str, workflow: Any | None, available: tuple[str, ...] | None, completed: Mapping[str, Sequence[str]] | None, issues: list[dict[str, str]]) -> dict[str, Any]:
    if workflow is None:
        return {
            "assessed": False,
            "plan_digest": content_digest({"missing_workflow": domain}),
            "requirement_count": 0,
            "covered_requirement_count": None,
            "missing_requirement_count": None,
            "coverage_ratio": None,
            "coverage_status": "unassessed",
            "next_stage_ids": [],
        }
    try:
        plan = build_autonomous_evidence_plan(
            (workflow,),
            available_evidence=() if available is None else available,
            completed_stages=completed,
        )
    except Exception as error:
        issues.append(_issue("evidence_plan_invalid", "blocking", "workflow evidence plan could not be compiled", "repair the workflow evidence outputs and dependency contract"))
        return {
            "assessed": False,
            "plan_digest": content_digest({"domain": domain, "evidence_plan_error": error.__class__.__name__}),
            "requirement_count": 0,
            "covered_requirement_count": None,
            "missing_requirement_count": None,
            "coverage_ratio": None,
            "coverage_status": "unassessed",
            "next_stage_ids": [],
        }
    return {
        "assessed": available is not None,
        "plan_digest": plan.plan_digest,
        "requirement_count": len(plan.requirements),
        "covered_requirement_count": None if available is None else len(plan.covered_requirement_ids),
        "missing_requirement_count": None if available is None else len(plan.missing_requirement_ids),
        "coverage_ratio": None if available is None else plan.coverage_ratio,
        "coverage_status": "unassessed" if available is None else plan.coverage_status,
        "next_stage_ids": list(plan.next_stage_ids),
    }


def _runtime_status(contract_status: str, tools: Mapping[str, Any], evidence: Mapping[str, Any]) -> str:
    if contract_status == "invalid":
        return "blocked"
    if tools.get("assessed") and tools.get("missing_tool_names"):
        return "partial"
    if evidence.get("assessed") and evidence.get("coverage_status") != "complete":
        return "partial"
    if not tools.get("assessed") and not evidence.get("assessed"):
        return "unassessed"
    return "ready_for_review"


def audit_autonomous_domain_contracts(
    *,
    profiles: Sequence[Any] | Any | None = None,
    workflows: Sequence[Any] | Any | None = None,
    tool_profiles: Sequence[Any] | Any | None = None,
    available_tool_names: Sequence[str] | None = None,
    available_evidence: Sequence[str] | None = None,
    completed_stages: Mapping[str, Sequence[str]] | None = None,
) -> dict[str, Any]:
    """Audit every selected domain without contacting providers, tools, or evidence sources."""

    from .autonomy import builtin_autonomous_domain_profiles, builtin_autonomous_workflow_strategies

    selected_profiles = _normalize_registry_values(
        "domain audit profiles", profiles, maximum=len(AUTONOMOUS_DOMAIN_NAMES), default_factory=builtin_autonomous_domain_profiles
    )
    selected_workflows = _normalize_registry_values(
        "domain audit workflows", workflows, maximum=len(AUTONOMOUS_DOMAIN_NAMES), default_factory=builtin_autonomous_workflow_strategies
    )
    selected_tool_profiles = _normalize_registry_values(
        "domain audit tool profiles", tool_profiles, maximum=len(AUTONOMOUS_DOMAIN_NAMES), default_factory=builtin_autonomous_domain_tool_profiles
    )
    available_tools = _normalize_optional_strings("domain audit available_tool_names", available_tool_names)
    available_values = _normalize_optional_strings("domain audit available_evidence", available_evidence)
    if completed_stages is not None:
        if not isinstance(completed_stages, Mapping) or len(completed_stages) > len(AUTONOMOUS_DOMAIN_NAMES):
            raise ArgumentError("domain audit completed_stages is malformed")
        for domain, stages in completed_stages.items():
            if domain not in AUTONOMOUS_DOMAIN_NAMES or isinstance(stages, (str, bytes)) or not isinstance(stages, Sequence):
                raise ArgumentError("domain audit completed_stages must map built-in domains to sequences")
        completed_stages = {str(domain): tuple(stages) for domain, stages in completed_stages.items()}

    profile_by_domain = _profile_map(selected_profiles)
    workflow_by_domain = _workflow_map(selected_workflows)
    tool_profile_by_domain = _tool_profile_map(selected_tool_profiles)
    rows: list[dict[str, Any]] = []
    for domain in sorted(profile_by_domain):
        profile = profile_by_domain[domain]
        issues: list[dict[str, str]] = []
        profile_digest = _audit_profile(profile, domain, issues)
        workflow = workflow_by_domain.get(domain)
        workflow_id, workflow_digest, stage_ids = _audit_workflow(domain, workflow, issues)
        tool_surface = _audit_tools(domain, workflow, tool_profile_by_domain.get(domain), available_tools, issues)
        evidence_surface = _audit_evidence(domain, workflow, available_values, completed_stages, issues)
        if len(issues) > MAX_AUTONOMOUS_DOMAIN_AUDIT_ISSUES:
            raise ArgumentError(f"domain audit {domain} produced too many issues")
        contract_status = "invalid" if any(item["severity"] == "blocking" for item in issues) else "valid"
        runtime_status = _runtime_status(contract_status, tool_surface, evidence_surface)
        next_actions = _unique_sorted([item["next_action"] for item in issues])
        if runtime_status == "partial":
            next_actions.append("resolve the missing live tool or evidence coverage before dispatch")
        if runtime_status == "unassessed":
            next_actions.append("provide caller-owned live tool and evidence inventories for runtime coverage assessment")
        descriptor = {
            "schema": AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA,
            "domain": domain,
            "profile_digest": profile_digest,
            "workflow_id": workflow_id,
            "workflow_digest": workflow_digest,
            "stage_ids": stage_ids,
            "stage_count": len(stage_ids),
            "required_model_capabilities": list(_field(profile, "required_model_capabilities", ())) if isinstance(_field(profile, "required_model_capabilities", ()), Sequence) else [],
            "declared_capability_count": len(_field(profile, "capabilities", ())) if isinstance(_field(profile, "capabilities", ()), Sequence) and not isinstance(_field(profile, "capabilities", ()), (str, bytes)) else 0,
            "evaluator_domain": _field(profile, "evaluator_domain", "unknown"),
            "workflow_evaluator_signals": list(_field(workflow, "evaluator_signals", ())) if workflow is not None and isinstance(_field(workflow, "evaluator_signals", ()), Sequence) and not isinstance(_field(workflow, "evaluator_signals", ()), (str, bytes)) else [],
            "contract_status": contract_status,
            "runtime_status": runtime_status,
            "tool_surface": tool_surface,
            "evidence_surface": evidence_surface,
            "issues": issues,
            "next_actions": _unique_sorted(next_actions),
            "retention": _RETENTION,
            "execution": _EXECUTION,
            "secret_material": "never_returned",
        }
        rows.append({**descriptor, "row_digest": content_digest(descriptor)})

    valid_count = sum(row["contract_status"] == "valid" for row in rows)
    runtime_ready = sum(row["runtime_status"] == "ready_for_review" for row in rows)
    runtime_partial = sum(row["runtime_status"] == "partial" for row in rows)
    runtime_blocked = sum(row["runtime_status"] == "blocked" for row in rows)
    runtime_unassessed = sum(row["runtime_status"] == "unassessed" for row in rows)
    evidence_rows = [row for row in rows if row["evidence_surface"]["assessed"]]
    evidence_covered = None if not evidence_rows else sum(row["evidence_surface"]["covered_requirement_count"] or 0 for row in evidence_rows)
    static_status = "valid" if valid_count == len(rows) else "invalid"
    runtime_status = "blocked" if runtime_blocked else "partial" if runtime_partial else "unassessed" if runtime_unassessed == len(rows) else "ready_for_review" if runtime_ready == len(rows) else "partial"
    summary = {
        "domain_count": len(rows),
        "valid_domain_count": valid_count,
        "invalid_domain_count": len(rows) - valid_count,
        "runtime_ready_domain_count": runtime_ready,
        "runtime_partial_domain_count": runtime_partial,
        "runtime_blocked_domain_count": runtime_blocked,
        "runtime_unassessed_domain_count": runtime_unassessed,
        "declared_tool_count": sum(row["tool_surface"]["declared_tool_count"] for row in rows),
        "missing_tool_count": sum(len(row["tool_surface"]["missing_tool_names"]) for row in rows),
        "evidence_requirement_count": sum(row["evidence_surface"]["requirement_count"] for row in rows),
        "evidence_covered_requirement_count": evidence_covered,
        "static_contract_status": static_status,
        "runtime_status": runtime_status,
    }
    next_actions = _unique_sorted([
        action for row in rows for action in row["next_actions"]
    ])
    if static_status == "invalid":
        next_actions.append("repair blocking domain contract issues before enabling autonomous dispatch")
    if runtime_status == "unassessed":
        next_actions.append("supply caller-owned tool and evidence inventories to complete the runtime audit")
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_AUDIT_SCHEMA,
        "rows": rows,
        "summary": summary,
        "next_actions": _unique_sorted(next_actions),
        "retention": _RETENTION,
        "execution": _EXECUTION,
        "credential_posture": "caller_owned_opaque_handles_only;no_credentials_consumed",
        "secret_material": "never_returned",
    }
    report = {**descriptor, "report_digest": content_digest(descriptor)}
    if len(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES:
        raise ArgumentError("domain audit report exceeds its bounded size")
    return _json_clone(report)


def validate_autonomous_domain_audit_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate row and report digests before accepting an audit handoff."""

    if not isinstance(value, Mapping):
        raise ArgumentError("autonomous domain audit report must be a mapping")
    if value.get("schema") != AUTONOMOUS_DOMAIN_AUDIT_SCHEMA or value.get("retention") != _RETENTION or value.get("execution") != _EXECUTION or value.get("credential_posture") != "caller_owned_opaque_handles_only;no_credentials_consumed" or value.get("secret_material") != "never_returned":
        raise ArgumentError("autonomous domain audit report is malformed")
    rows = value.get("rows")
    if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence) or not 1 <= len(rows) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("autonomous domain audit rows are outside their bounds")
    domains: list[str] = []
    for raw in rows:
        if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA or raw.get("domain") not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("autonomous domain audit row is malformed")
        row_digest = _bounded_digest("autonomous domain audit row digest", raw.get("row_digest"))
        descriptor = dict(raw)
        descriptor.pop("row_digest", None)
        if content_digest(descriptor) != row_digest:
            raise ArgumentError("autonomous domain audit row digest does not match its metadata")
        domains.append(raw["domain"])
    if len(set(domains)) != len(domains):
        raise ArgumentError("autonomous domain audit rows contain duplicate domains")
    report_digest = _bounded_digest("autonomous domain audit report digest", value.get("report_digest"))
    descriptor = dict(value)
    descriptor.pop("report_digest", None)
    if content_digest(descriptor) != report_digest:
        raise ArgumentError("autonomous domain audit report digest does not match its metadata")
    if len(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES:
        raise ArgumentError("autonomous domain audit report exceeds its bounded size")
    return _json_clone(dict(value))


def audit_autonomous_agent_domain_contracts(
    agent: Any,
    *,
    available_tool_names: Sequence[str] | None = None,
    available_evidence: Sequence[str] | None = None,
    completed_stages: Mapping[str, Sequence[str]] | None = None,
) -> dict[str, Any]:
    """Audit the registries currently bound to an ``AutonomousAgent``."""

    orchestrator = getattr(agent, "orchestrator", None)
    if orchestrator is None:
        raise ArgumentError("autonomous domain audit agent must expose an orchestrator")
    tool_names = available_tool_names
    if tool_names is None:
        tool_registry = getattr(agent, "tool_registry", None)
        if tool_registry is not None and callable(getattr(tool_registry, "catalogue", None)):
            tool_names = tuple(
                row["name"]
                for row in tool_registry.catalogue()
                if isinstance(row, Mapping) and isinstance(row.get("name"), str)
            )
    return audit_autonomous_domain_contracts(
        profiles=getattr(orchestrator, "registry", None),
        workflows=getattr(orchestrator, "workflow_registry", None),
        available_tool_names=tool_names,
        available_evidence=available_evidence,
        completed_stages=completed_stages,
    )


__all__ = [
    "AUTONOMOUS_DOMAIN_AUDIT_SCHEMA",
    "AUTONOMOUS_DOMAIN_AUDIT_ROW_SCHEMA",
    "MAX_AUTONOMOUS_DOMAIN_AUDIT_BYTES",
    "MAX_AUTONOMOUS_DOMAIN_AUDIT_ISSUES",
    "audit_autonomous_domain_contracts",
    "audit_autonomous_agent_domain_contracts",
    "validate_autonomous_domain_audit_report",
]
