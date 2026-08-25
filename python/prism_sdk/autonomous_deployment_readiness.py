"""Digest-bound, provider-free deployment readiness auditing for the autonomous agent.

The autonomous runtime already exposes detailed readiness, credential provisioning, evidence,
and learning projections.  This module joins those projections into one review artifact that an
application can show before enabling a deployment.  It deliberately does not create a session,
resolve a credential, contact a provider, initialize a queue, test persistence, acquire evidence,
or grant an external effect.
"""

from __future__ import annotations

from copy import deepcopy
from dataclasses import dataclass
import json
from typing import Any, Mapping, NoReturn, Sequence

from .authoring import canonical_json, content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError
from .llm_runtime import CREDENTIAL_PROVISIONING_SCHEMA


AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA = "bioprism-python-autonomous-deployment-readiness/0.1"
AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA = "bioprism-python-autonomous-deployment-readiness-domain/0.1"
AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA = "bioprism-python-autonomous-deployment-readiness-capability/0.1"
MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BYTES = 512_000
MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BLOCKERS = 512

AUTONOMOUS_DEPLOYMENT_READINESS_STATES = ("ready_for_review", "partial", "blocked")
AUTONOMOUS_DEPLOYMENT_BLOCKER_CODES = (
    "model_catalogue",
    "model_capability",
    "provider_registration",
    "credential",
    "tool_catalogue",
    "evidence_adapter",
    "learning",
    "persistence",
    "queue",
    "approval_authority",
    "external_auth",
    "telemetry",
)
AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES = (
    "persistence",
    "queue",
    "approval_authority",
    "external_auth",
    "telemetry",
)

_EXECUTION = "audit_only;no_provider_source_tool_queue_or_credential_dispatch"
_RETENTION = "metadata_only;digests_capabilities_and_next_actions"
_SECRET_MATERIAL = "never_returned"
_AGENT_SCHEMA = "bioprism-autonomous-agent-readiness/0.1"
_DOMAIN_STATES = {
    "ready_for_caller_approval",
    "model_catalogue_required",
    "provider_registration_required",
    "credential_required",
    "model_capability_gap",
    "partial",
}
_SECRET_KEYS = {
    "apikey",
    "bearer",
    "password",
    "privatekey",
    "rawresponse",
    "rawpayload",
    "refreshtoken",
    "secret",
    "token",
}


def _fail(message: str) -> NoReturn:
    raise ArgumentError(f"autonomous deployment readiness {message}")


def _text(name: str, value: Any, maximum: int = 1_024) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail(f"{name} must be a bounded non-empty string")
    if len(value.encode("utf-8")) > maximum:
        _fail(f"{name} exceeds its bound")
    return value


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= maximum:
        _fail(f"{name} must be between {minimum} and {maximum}")
    return value


def _strings(name: str, value: Any, maximum: int = 1_024) -> list[str]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) > maximum:
        _fail(f"{name} is outside its bound")
    return [_text(f"{name}[{index}]", item) for index, item in enumerate(value)]


def _safe_metadata(value: Any, *, path: str = "$", depth: int = 0) -> None:
    if depth > 32:
        _fail(f"{path} is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail(f"{path} contains a non-string key")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _SECRET_KEYS:
                _fail(f"{path}.{key} contains secret-shaped metadata")
            _safe_metadata(child, path=f"{path}.{key}", depth=depth + 1)
    elif isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        for index, child in enumerate(value):
            _safe_metadata(child, path=f"{path}[{index}]", depth=depth + 1)
    else:
        try:
            json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"autonomous deployment readiness {path} is not JSON-safe") from error


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(canonical_json(dict(value)))
    except (ArgumentError, TypeError, ValueError) as error:
        raise ArgumentError("autonomous deployment readiness value is not canonical JSON") from error


@dataclass(frozen=True, slots=True)
class AutonomousDeploymentReadinessPolicy:
    """Deployment-owned requirements; defaults are conservative for credentials and persistence."""

    require_credentials: bool = True
    require_tool_catalogue: bool = False
    require_evidence: bool = False
    require_learning: bool = False
    require_persistence: bool = True
    require_queue: bool = False
    require_approval_authority: bool = True
    require_external_auth: bool = False
    require_telemetry: bool = False

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | None = None) -> "AutonomousDeploymentReadinessPolicy":
        if value is None:
            return cls()
        if not isinstance(value, Mapping):
            _fail("policy must be a mapping or None")
        aliases = {
            "require_credentials": "require_credentials",
            "requireCredentials": "require_credentials",
            "require_tool_catalogue": "require_tool_catalogue",
            "requireToolCatalogue": "require_tool_catalogue",
            "require_evidence": "require_evidence",
            "requireEvidence": "require_evidence",
            "require_learning": "require_learning",
            "requireLearning": "require_learning",
            "require_persistence": "require_persistence",
            "requirePersistence": "require_persistence",
            "require_queue": "require_queue",
            "requireQueue": "require_queue",
            "require_approval_authority": "require_approval_authority",
            "requireApprovalAuthority": "require_approval_authority",
            "require_external_auth": "require_external_auth",
            "requireExternalAuth": "require_external_auth",
            "require_telemetry": "require_telemetry",
            "requireTelemetry": "require_telemetry",
        }
        values: dict[str, bool] = {}
        for key, raw in value.items():
            if key not in aliases:
                _fail(f"policy contains unsupported field {key}")
            if not isinstance(raw, bool):
                _fail(f"policy {key} must be boolean")
            values[aliases[key]] = raw
        return cls(**values)

    def to_dict(self) -> dict[str, bool]:
        return {
            "require_credentials": self.require_credentials,
            "require_tool_catalogue": self.require_tool_catalogue,
            "require_evidence": self.require_evidence,
            "require_learning": self.require_learning,
            "require_persistence": self.require_persistence,
            "require_queue": self.require_queue,
            "require_approval_authority": self.require_approval_authority,
            "require_external_auth": self.require_external_auth,
            "require_telemetry": self.require_telemetry,
        }


def _policy(value: AutonomousDeploymentReadinessPolicy | Mapping[str, Any] | None) -> AutonomousDeploymentReadinessPolicy:
    if isinstance(value, AutonomousDeploymentReadinessPolicy):
        return value
    return AutonomousDeploymentReadinessPolicy.from_mapping(value)


def _validate_agent_report(value: Mapping[str, Any]) -> tuple[dict[str, Any], str]:
    if not isinstance(value, Mapping):
        _fail("agent readiness report must be a mapping")
    _safe_metadata(value)
    report = _clone(value)
    if report.get("schema") != _AGENT_SCHEMA:
        _fail("agent readiness report schema is unsupported")
    domains = report.get("domains")
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence) or len(domains) != len(AUTONOMOUS_DOMAIN_NAMES):
        _fail("agent readiness report must cover every built-in domain")
    seen: set[str] = set()
    for index, row in enumerate(domains):
        if not isinstance(row, Mapping):
            _fail(f"agent readiness domain {index} is malformed")
        domain = row.get("domain")
        if domain not in AUTONOMOUS_DOMAIN_NAMES or domain in seen:
            _fail(f"agent readiness domain {index} is unsupported or duplicated")
        seen.add(domain)
        if row.get("state") not in _DOMAIN_STATES:
            _fail(f"agent readiness domain {domain} state is invalid")
        compatible = _integer(f"agent readiness {domain} compatible_model_count", row.get("compatible_model_count"), 0, 1_000_000)
        _integer(f"agent readiness {domain} eligible_model_count", row.get("eligible_model_count"), 0, compatible)
        _strings(f"agent readiness {domain} required_model_capabilities", row.get("required_model_capabilities", ()), 256)
        _strings(f"agent readiness {domain} next_actions", row.get("next_actions", ()), 128)
    if seen != set(AUTONOMOUS_DOMAIN_NAMES):
        _fail("agent readiness report does not cover every built-in domain")
    supplied = report.get("readiness_digest")
    if supplied is None:
        digest = content_digest(report)
    else:
        _digest("agent readiness_digest", supplied)
        unsigned = dict(report)
        unsigned.pop("readiness_digest", None)
        if content_digest(unsigned) != supplied:
            _fail("agent readiness_digest does not match its metadata")
        digest = supplied
    return report, digest


def _validate_provider_plan(value: Mapping[str, Any] | None) -> tuple[dict[str, Any], str]:
    if value is None:
        value = {
            "schema": CREDENTIAL_PROVISIONING_SCHEMA,
            "providers": [],
            "provider_count": 0,
            "execution": "process_local_resolution_into_short_lived_session",
            "restart_posture": "re-register_sources_and_resolve_fresh_handles",
            "retention": "metadata_only_no_keys_references_or_callbacks",
            "secret_material": _SECRET_MATERIAL,
        }
    if not isinstance(value, Mapping):
        _fail("provider plan must be a mapping")
    _safe_metadata(value)
    plan = _clone(value)
    if plan.get("schema") not in {CREDENTIAL_PROVISIONING_SCHEMA, "bioprism-typescript-provider-setup/0.1"}:
        _fail("provider plan schema is unsupported")
    providers = plan.get("providers")
    if isinstance(providers, (str, bytes)) or not isinstance(providers, Sequence) or len(providers) > 128:
        _fail("provider plan providers are outside their bound")
    if plan.get("provider_count") != len(providers):
        _fail("provider plan provider_count is inconsistent")
    for index, row in enumerate(providers):
        if not isinstance(row, Mapping):
            _fail(f"provider plan provider {index} is malformed")
        _text(f"provider plan provider {index}", row.get("provider"), 128)
        for field in ("provider_registered", "ready", "credential_ready"):
            if field in row and not isinstance(row[field], bool):
                _fail(f"provider plan provider {index} {field} must be boolean")
        _text(f"provider plan provider {index} next_action", row.get("next_action", "register_provider"), 256)
    return plan, content_digest(plan)


def _blocker(code: str, scope: str, domain: str | None, message: str, next_action: str, severity: str = "blocking") -> dict[str, Any]:
    if code not in AUTONOMOUS_DEPLOYMENT_BLOCKER_CODES or scope not in {"global", "domain"} or severity not in {"blocking", "warning"}:
        _fail("blocker contains an unsupported code, scope, or severity")
    if scope == "domain" and domain not in AUTONOMOUS_DOMAIN_NAMES:
        _fail("domain blocker has an unsupported domain")
    if scope == "global" and domain is not None:
        _fail("global blocker cannot carry a domain")
    return {
        "code": code,
        "scope": scope,
        "domain": domain,
        "severity": severity,
        "message": _text("blocker message", message, 2_048),
        "next_action": _text("blocker next_action", next_action, 1_024),
    }


def _sorted_blockers(rows: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    return [
        dict(row)
        for row in sorted(
            rows,
            key=lambda row: f"{row.get('scope')}:{row.get('domain') or ''}:{row.get('code')}:{row.get('message')}",
        )
    ]


def _workflow_maps(agent: Mapping[str, Any]) -> tuple[dict[str, Mapping[str, Any]], dict[str, Mapping[str, Any]]]:
    workflows: dict[str, Mapping[str, Any]] = {}
    packs: dict[str, Mapping[str, Any]] = {}
    for row in agent.get("workflows", ()) if isinstance(agent.get("workflows", ()), Sequence) else ():
        if isinstance(row, Mapping) and isinstance(row.get("domain"), str):
            workflows[row["domain"]] = row
    for row in agent.get("domain_packs", ()) if isinstance(agent.get("domain_packs", ()), Sequence) else ():
        if isinstance(row, Mapping) and isinstance(row.get("domain"), str):
            packs[row["domain"]] = row
    return workflows, packs


def _tool_gate(agent: Mapping[str, Any], domain: str, row: Mapping[str, Any]) -> tuple[list[str], list[str], list[str]]:
    plans = agent.get("domain_pack_tool_plans", {})
    if isinstance(plans, Mapping):
        plan = plans.get(domain)
    elif isinstance(plans, Sequence) and not isinstance(plans, (str, bytes)):
        plan = next((item for item in plans if isinstance(item, Mapping) and item.get("domain") == domain), None)
    else:
        plan = None
    if isinstance(plan, Mapping):
        required = _strings(f"deployment readiness {domain} required tools", plan.get("required_tool_capabilities", ()), 512)
        available = _strings(f"deployment readiness {domain} available tools", plan.get("covered_tool_capabilities", plan.get("available_tool_capabilities", ())), 512)
        missing = _strings(f"deployment readiness {domain} missing tools", plan.get("missing_tool_capabilities", ()), 512)
        return required, available, missing
    required = _strings(f"deployment readiness {domain} required tools", row.get("required_tools", ()), 512)
    missing = _strings(f"deployment readiness {domain} missing tools", row.get("missing_tools", ()), 512)
    return required, [item for item in required if item not in missing], missing


def _evidence_gate(agent: Mapping[str, Any], domain: str, row: Mapping[str, Any]) -> tuple[str, str | None]:
    evidence = row.get("evidence_readiness")
    if not isinstance(evidence, Mapping):
        report = agent.get("evidence")
        if isinstance(report, Mapping):
            rows = report.get("domains", ())
            if isinstance(rows, Sequence):
                evidence = next((item for item in rows if isinstance(item, Mapping) and item.get("domain") == domain), None)
    if not isinstance(evidence, Mapping):
        return "not_requested", None
    status = _text(f"deployment readiness {domain} evidence status", evidence.get("status", "not_requested"), 128)
    digest = evidence.get("report_digest")
    return status, _digest(f"deployment readiness {domain} evidence report_digest", digest, allow_none=True)


def _learning_projection(agent: Mapping[str, Any], row: Mapping[str, Any]) -> tuple[bool, str | None]:
    learning = agent.get("learning")
    if not isinstance(learning, Mapping):
        learning = agent.get("domain_learning_coverage")
    configured = learning.get("configured") is True if isinstance(learning, Mapping) else False
    calibration: Any = None
    if isinstance(row.get("evaluator_calibration"), Mapping):
        calibration_row = row["evaluator_calibration"]
        status = calibration_row.get("status")
        calibration = "admit_learning" if status == "ready" else "hold_learning" if isinstance(status, str) else None
    if calibration is None and isinstance(learning, Mapping):
        calibration_value = learning.get("calibration", learning.get("evaluator_calibration"))
        if isinstance(calibration_value, Mapping):
            if calibration_value.get("configured", True) is not False:
                calibration = calibration_value.get("decision")
    if calibration is None:
        calibration_value = agent.get("evaluator_calibration")
        if isinstance(calibration_value, Mapping):
            calibration = calibration_value.get("decision")
    if calibration is not None:
        calibration = _text("deployment readiness calibration decision", calibration, 64)
    return configured, calibration


def _domain_row(
    agent: Mapping[str, Any],
    row: Mapping[str, Any],
    policy: AutonomousDeploymentReadinessPolicy,
) -> dict[str, Any]:
    domain = row["domain"]
    workflows, packs = _workflow_maps(agent)
    workflow = workflows.get(domain)
    pack = packs.get(domain)
    workflow_id = workflow.get("workflow_id") if isinstance(workflow, Mapping) else pack.get("workflow_id") if isinstance(pack, Mapping) else None
    workflow_digest = workflow.get("workflow_digest") if isinstance(workflow, Mapping) else None
    if workflow_digest is None and isinstance(pack, Mapping):
        workflow_digest = pack.get("pack_digest") or pack.get("pack_id")
    workflow_id = _text(f"deployment readiness {domain} workflow_id", workflow_id or f"{domain}_workflow", 256)
    workflow_digest = _digest(f"deployment readiness {domain} workflow_digest", workflow_digest) if workflow_digest and len(str(workflow_digest)) == 64 else content_digest({"domain": domain, "workflow_id": workflow_id})
    domain_blockers: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    state = row["state"]
    if state == "model_catalogue_required":
        domain_blockers.append(_blocker("model_catalogue", "domain", domain, "no model catalogue is available for this domain", "register a reviewed candidate model for this domain"))
    elif state == "model_capability_gap":
        domain_blockers.append(_blocker("model_capability", "domain", domain, "available models do not declare the required domain capabilities", "register a model with the required capabilities"))
    elif state == "provider_registration_required":
        domain_blockers.append(_blocker("provider_registration", "domain", domain, "compatible model providers are not registered", "register the provider transport before invocation"))
    elif state == "credential_required":
        domain_blockers.append(_blocker("credential", "domain", domain, "compatible providers have no active caller credential", "collect a short-lived user credential through protected onboarding"))
    elif state == "partial":
        actions = _strings(f"deployment readiness {domain} next_actions", row.get("next_actions", ()), 128)
        domain_blockers.append(_blocker("model_capability", "domain", domain, "domain readiness is partial and requires review before deployment", actions[0] if actions else "resolve the domain readiness next action"))

    required_tools, available_tools, missing_tools = _tool_gate(agent, domain, row)
    if policy.require_tool_catalogue and missing_tools:
        domain_blockers.append(_blocker("tool_catalogue", "domain", domain, "required domain tools are missing: " + ", ".join(missing_tools), "attach and review the live tool catalogue"))
    elif missing_tools:
        warnings.append(_blocker("tool_catalogue", "domain", domain, "optional domain tools are not currently attached: " + ", ".join(missing_tools), "attach a reviewed tool catalogue for richer execution", "warning"))

    evidence_status, evidence_digest = _evidence_gate(agent, domain, row)
    if policy.require_evidence and evidence_status != "ready":
        domain_blockers.append(_blocker("evidence_adapter", "domain", domain, f"evidence readiness is {evidence_status}", "register and health-check a source adapter before evidence dispatch"))
    elif evidence_status in {"blocked", "missing"}:
        warnings.append(_blocker("evidence_adapter", "domain", domain, f"evidence readiness is {evidence_status}", "resolve source adapter coverage before evidence-backed work", "warning"))

    configured, calibration = _learning_projection(agent, row)
    if policy.require_learning and not configured:
        domain_blockers.append(_blocker("learning", "domain", domain, "online learning is required but no learner is attached", "attach persisted online learning and evaluator settlement"))
    if policy.require_learning and calibration is not None and calibration != "admit_learning":
        domain_blockers.append(_blocker("learning", "domain", domain, f"learning calibration is {calibration}", "resolve evaluator calibration before enabling learning"))

    final_state = "blocked" if domain_blockers else "ready_for_review" if state == "ready_for_caller_approval" else "partial"
    actions = set(_strings(f"deployment readiness {domain} next_actions", row.get("next_actions", ()), 128))
    actions.update(item["next_action"] for item in (*domain_blockers, *warnings))
    return {
        "schema": AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA,
        "domain": domain,
        "workflow_id": workflow_id,
        "workflow_digest": workflow_digest,
        "agent_state": state,
        "state": final_state,
        "model_gate": {
            "compatible_model_count": row["compatible_model_count"],
            "eligible_model_count": row["eligible_model_count"],
            "required_model_capabilities": _strings(f"deployment readiness {domain} required model capabilities", row.get("required_model_capabilities", ()), 256),
        },
        "tool_gate": {
            "required_tool_count": len(required_tools),
            "available_tool_count": len(available_tools),
            "missing_tools": missing_tools,
        },
        "evidence_gate": {"requested": policy.require_evidence, "status": evidence_status, "report_digest": evidence_digest},
        "learning_gate": {"required": policy.require_learning, "configured": configured, "calibration_decision": calibration},
        "blockers": _sorted_blockers(domain_blockers),
        "warnings": _sorted_blockers(warnings),
        "next_actions": sorted(actions),
        "execution": _EXECUTION,
        "retention": _RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }


class AutonomousDeploymentReadinessAuditor:
    """Join readiness projections and deployment-owned capability assertions without side effects."""

    def __init__(self, policy: AutonomousDeploymentReadinessPolicy | Mapping[str, Any] | None = None, **overrides: Any) -> None:
        if overrides:
            base = policy.to_dict() if isinstance(policy, AutonomousDeploymentReadinessPolicy) else dict(policy or {})
            base.update(overrides)
            policy = base
        self.policy = _policy(policy)

    def audit(self, value: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(value, Mapping):
            _fail("input must be a mapping")
        if "agent" not in value:
            _fail("input requires an agent readiness report")
        agent, agent_digest = _validate_agent_report(value["agent"])
        provider_plan, provider_digest = _validate_provider_plan(value.get("provider_plan", agent.get("credential_provisioning")))
        capabilities = value.get("capabilities", {})
        if not isinstance(capabilities, Mapping):
            _fail("capabilities must be a mapping")

        used_providers = set()
        models = agent.get("models", ())
        if isinstance(models, (str, bytes)) or not isinstance(models, Sequence):
            _fail("agent models are malformed")
        for index, model in enumerate(models):
            if not isinstance(model, Mapping):
                _fail(f"agent model {index} is malformed")
            used_providers.add(_text(f"agent model {index} provider", model.get("provider"), 128))
        plan_rows = provider_plan.get("providers", ())
        plan_by_provider = {row["provider"]: row for row in plan_rows if isinstance(row, Mapping) and isinstance(row.get("provider"), str)}
        provider_rows = []
        global_blockers: list[dict[str, Any]] = []
        if not used_providers:
            global_blockers.append(_blocker("model_catalogue", "global", None, "the agent readiness report contains no model candidates", "register reviewed model candidates with domain capabilities"))
        for provider in sorted(used_providers):
            row = plan_by_provider.get(provider)
            registered = row is not None and row.get("provider_registered") is True
            ready = row is not None and row.get("ready", row.get("credential_ready", False)) is True
            provider_rows.append({"provider": provider, "registered": registered, "ready": ready, "next_action": row.get("next_action", "register_provider") if isinstance(row, Mapping) else "register_provider_transport"})
            if not registered:
                global_blockers.append(_blocker("provider_registration", "global", None, f"provider transport {provider} is not registered", f"register_provider_transport: {provider}"))
            elif self.policy.require_credentials and not ready:
                global_blockers.append(_blocker("credential", "global", None, f"provider {provider} is registered but not credential-ready", f"collect_user_credential through a protected onboarding boundary: {provider}"))

        capabilities_out = []
        for name in AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES:
            raw = capabilities.get(name)
            if raw is None:
                raw = {"configured": False, "operational": False, "restart_safe": False, "integrity_fenced": False, "caller_owned": True, "next_actions": [f"configure {name} through the deployment owner"]}
            if not isinstance(raw, Mapping):
                _fail(f"capability {name} is malformed")
            for field in ("configured", "operational", "restart_safe", "integrity_fenced", "caller_owned"):
                if not isinstance(raw.get(field), bool):
                    _fail(f"capability {name} {field} must be boolean")
            actions = _strings(f"capability {name} next_actions", raw.get("next_actions", ()), 32)
            required = bool(self.policy.to_dict()[f"require_{name}"])
            satisfies = all(raw[field] for field in ("configured", "operational", "restart_safe", "integrity_fenced"))
            if required and not satisfies:
                global_blockers.append(_blocker(name, "global", None, f"{name} is required but its deployment contract is incomplete", actions[0] if actions else f"configure {name} with restart-safe integrity fencing"))
            capabilities_out.append({"schema": AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA, "name": name, "required": required, "configured": raw["configured"], "operational": raw["operational"], "restart_safe": raw["restart_safe"], "integrity_fenced": raw["integrity_fenced"], "caller_owned": raw["caller_owned"], "satisfies_requirement": not required or satisfies, "next_actions": actions, "execution": "projection_only;does_not_initialize_or_test_capability", "retention": _RETENTION, "secret_material": _SECRET_MATERIAL})

        domain_by_name = {row["domain"]: row for row in agent["domains"]}
        domain_rows = [_domain_row(agent, domain_by_name[domain], self.policy) for domain in AUTONOMOUS_DOMAIN_NAMES]
        global_blockers = _sorted_blockers(global_blockers)
        warnings = _sorted_blockers([warning for row in domain_rows for warning in row["warnings"]])
        ready_count = sum(row["state"] == "ready_for_review" for row in domain_rows)
        partial_count = sum(row["state"] == "partial" for row in domain_rows)
        blocked_count = sum(row["state"] == "blocked" for row in domain_rows)
        state = "blocked" if global_blockers or blocked_count == len(domain_rows) else "ready_for_review" if ready_count == len(domain_rows) else "partial"
        actions = set(_strings("agent next_actions", agent.get("next_actions", ()), 512))
        actions.update(item["next_action"] for item in (*global_blockers, *warnings))
        body = {
            "schema": AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA,
            "agent_readiness_digest": agent_digest,
            "provider_setup_digest": provider_digest,
            "policy": self.policy.to_dict(),
            "provider_gate": {"candidate_provider_count": len(provider_rows), "ready_provider_count": sum(row["ready"] for row in provider_rows), "unresolved_provider_count": sum(not row["ready"] for row in provider_rows), "providers": provider_rows},
            "capabilities": capabilities_out,
            "domains": domain_rows,
            "global_blockers": global_blockers,
            "warnings": warnings,
            "ready_domain_count": ready_count,
            "partial_domain_count": partial_count,
            "blocked_domain_count": blocked_count,
            "state": state,
            "next_actions": sorted(actions),
            "readiness_claimed": False,
            "execution": _EXECUTION,
            "authority": "audit_does_not_grant_dispatch_authority",
            "credential_posture": "caller_owned_protected_input;opaque_runtime_handles_only",
            "retention": _RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        if len(global_blockers) + len(warnings) > MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BLOCKERS:
            _fail("blocker count exceeds its bound")
        report = {**body, "readiness_digest": content_digest(body)}
        if len(canonical_json(report).encode("utf-8")) > MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BYTES:
            _fail("report exceeds its byte bound")
        return _clone(report)


def _validate_blocker(value: Mapping[str, Any]) -> None:
    if not isinstance(value, Mapping):
        _fail("report blocker is malformed")
    _blocker(value.get("code"), value.get("scope"), value.get("domain"), value.get("message"), value.get("next_action"), value.get("severity"))


def validate_autonomous_deployment_readiness_report(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("report must be a mapping")
    _safe_metadata(value)
    report = _clone(value)
    expected = {"schema", "agent_readiness_digest", "provider_setup_digest", "policy", "provider_gate", "capabilities", "domains", "global_blockers", "warnings", "ready_domain_count", "partial_domain_count", "blocked_domain_count", "state", "next_actions", "readiness_claimed", "execution", "authority", "credential_posture", "retention", "secret_material", "readiness_digest"}
    if set(report) != expected:
        _fail("report contains unsupported or missing fields")
    if report["schema"] != AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA or report["readiness_claimed"] is not False or report["execution"] != _EXECUTION or report["authority"] != "audit_does_not_grant_dispatch_authority" or report["secret_material"] != _SECRET_MATERIAL:
        _fail("report execution posture is unsafe")
    _digest("report agent_readiness_digest", report["agent_readiness_digest"])
    _digest("report provider_setup_digest", report["provider_setup_digest"])
    _digest("report readiness_digest", report["readiness_digest"])
    unsigned = dict(report)
    unsigned.pop("readiness_digest")
    if content_digest(unsigned) != report["readiness_digest"]:
        _fail("report readiness_digest does not match its metadata")
    if report["state"] not in AUTONOMOUS_DEPLOYMENT_READINESS_STATES:
        _fail("report state is invalid")
    _integer("report ready_domain_count", report["ready_domain_count"], 0, len(AUTONOMOUS_DOMAIN_NAMES))
    _integer("report partial_domain_count", report["partial_domain_count"], 0, len(AUTONOMOUS_DOMAIN_NAMES))
    _integer("report blocked_domain_count", report["blocked_domain_count"], 0, len(AUTONOMOUS_DOMAIN_NAMES))
    _strings("report next_actions", report["next_actions"], 512)
    if not isinstance(report["policy"], Mapping) or set(report["policy"]) != set(AutonomousDeploymentReadinessPolicy().to_dict()):
        _fail("report policy is malformed")
    _policy(report["policy"])
    domains = report["domains"]
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence) or len(domains) != len(AUTONOMOUS_DOMAIN_NAMES):
        _fail("report domains are outside their bound")
    seen: set[str] = set()
    for row in domains:
        if not isinstance(row, Mapping) or row.get("schema") != AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA:
            _fail("report domain row is malformed")
        domain = row.get("domain")
        if domain not in AUTONOMOUS_DOMAIN_NAMES or domain in seen:
            _fail("report domains are duplicated or unsupported")
        seen.add(domain)
        _text("report workflow_id", row.get("workflow_id"), 256)
        _digest("report workflow_digest", row.get("workflow_digest"))
        if row.get("state") not in AUTONOMOUS_DEPLOYMENT_READINESS_STATES or row.get("agent_state") not in _DOMAIN_STATES:
            _fail("report domain state is invalid")
        model_gate = row.get("model_gate")
        tool_gate = row.get("tool_gate")
        if not isinstance(model_gate, Mapping) or not isinstance(tool_gate, Mapping):
            _fail("report domain gates are malformed")
        compatible = _integer("report compatible model count", model_gate.get("compatible_model_count"), 0, 1_000_000)
        _integer("report eligible model count", model_gate.get("eligible_model_count"), 0, compatible)
        _strings("report required model capabilities", model_gate.get("required_model_capabilities"), 256)
        required = _integer("report required tool count", tool_gate.get("required_tool_count"), 0, 1_000_000)
        _integer("report available tool count", tool_gate.get("available_tool_count"), 0, required)
        _strings("report missing tools", tool_gate.get("missing_tools"), 512)
        evidence = row.get("evidence_gate")
        learning = row.get("learning_gate")
        if not isinstance(evidence, Mapping) or not isinstance(learning, Mapping) or not isinstance(evidence.get("requested"), bool) or not isinstance(learning.get("required"), bool) or not isinstance(learning.get("configured"), bool):
            _fail("report evidence or learning gate is malformed")
        _digest("report evidence digest", evidence.get("report_digest"), allow_none=True)
        for key in ("blockers", "warnings"):
            rows = row.get(key)
            if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence):
                _fail(f"report domain {key} are malformed")
            for blocker in rows:
                _validate_blocker(blocker)
        _strings("report domain next_actions", row.get("next_actions"), 512)
    if seen != set(AUTONOMOUS_DOMAIN_NAMES):
        _fail("report does not cover every built-in domain")
    for key in ("global_blockers", "warnings"):
        rows = report[key]
        if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence):
            _fail(f"report {key} are malformed")
        for blocker in rows:
            _validate_blocker(blocker)
    capabilities = report["capabilities"]
    if isinstance(capabilities, (str, bytes)) or not isinstance(capabilities, Sequence) or len(capabilities) != len(AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES):
        _fail("report capabilities are outside their bound")
    if {row.get("name") for row in capabilities if isinstance(row, Mapping)} != set(AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES):
        _fail("report capabilities do not cover every deployment capability")
    for row in capabilities:
        if not isinstance(row, Mapping) or row.get("schema") != AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA:
            _fail("report capability row is malformed")
        for field in ("required", "configured", "operational", "restart_safe", "integrity_fenced", "caller_owned", "satisfies_requirement"):
            if not isinstance(row.get(field), bool):
                _fail(f"report capability {field} must be boolean")
        _strings("report capability next_actions", row.get("next_actions"), 32)
    providers = report["provider_gate"]
    if not isinstance(providers, Mapping) or not isinstance(providers.get("providers"), Sequence):
        _fail("report provider gate is malformed")
    _integer("report candidate_provider_count", providers.get("candidate_provider_count"), 0, 128)
    _integer("report ready_provider_count", providers.get("ready_provider_count"), 0, providers["candidate_provider_count"])
    _integer("report unresolved_provider_count", providers.get("unresolved_provider_count"), 0, providers["candidate_provider_count"])
    for row in providers["providers"]:
        if not isinstance(row, Mapping) or not isinstance(row.get("registered"), bool) or not isinstance(row.get("ready"), bool):
            _fail("report provider row is malformed")
        _text("report provider", row.get("provider"), 128)
        _text("report provider next_action", row.get("next_action"), 256)
    return _clone(report)


def audit_autonomous_deployment_readiness(value: Mapping[str, Any], policy: AutonomousDeploymentReadinessPolicy | Mapping[str, Any] | None = None) -> dict[str, Any]:
    return AutonomousDeploymentReadinessAuditor(policy).audit(value)


def audit_autonomous_agent_deployment_readiness(
    agent: Any,
    *,
    policy: AutonomousDeploymentReadinessPolicy | Mapping[str, Any] | None = None,
    capabilities: Mapping[str, Any] | None = None,
    readiness_options: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    if not callable(getattr(agent, "readiness", None)) or not callable(getattr(agent, "credential_provisioning_plan", None)):
        _fail("agent must expose readiness and credential_provisioning_plan")
    options = {} if readiness_options is None else dict(readiness_options)
    readiness = agent.readiness(**options)
    if not isinstance(readiness, Mapping):
        _fail("agent readiness did not return a mapping")
    readiness = dict(readiness)
    if "learning" not in readiness:
        readiness["learning"] = {
            "configured": getattr(agent, "ledger", None) is not None,
            "calibration": readiness.get("evaluator_calibration"),
            "execution": "readiness_projection_only;no_learning_mutation",
            "secret_material": _SECRET_MATERIAL,
        }
    return AutonomousDeploymentReadinessAuditor(policy).audit({"agent": readiness, "provider_plan": agent.credential_provisioning_plan(), "capabilities": {} if capabilities is None else capabilities})


__all__ = [
    "AUTONOMOUS_DEPLOYMENT_READINESS_SCHEMA",
    "AUTONOMOUS_DEPLOYMENT_READINESS_DOMAIN_SCHEMA",
    "AUTONOMOUS_DEPLOYMENT_READINESS_CAPABILITY_SCHEMA",
    "MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BYTES",
    "MAX_AUTONOMOUS_DEPLOYMENT_READINESS_BLOCKERS",
    "AUTONOMOUS_DEPLOYMENT_READINESS_STATES",
    "AUTONOMOUS_DEPLOYMENT_BLOCKER_CODES",
    "AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES",
    "AutonomousDeploymentReadinessPolicy",
    "AutonomousDeploymentReadinessAuditor",
    "validate_autonomous_deployment_readiness_report",
    "audit_autonomous_deployment_readiness",
    "audit_autonomous_agent_deployment_readiness",
]
