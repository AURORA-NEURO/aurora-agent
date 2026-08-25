"""Unified, provider-free launch preflight for the autonomous brain.

The SDK already exposes independent audits for domain contracts, model/provider readiness,
evidence routing, and deployment-owned capabilities.  Applications otherwise have to call those
surfaces separately and reconcile twelve domain rows themselves.  This module composes the
existing reports into one bounded handoff.  It is a review artifact only: it never resolves a
credential, calls a provider or source, executes a tool, mutates a learner, or authorizes an
effect.
"""

from __future__ import annotations

import json
import math
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_deployment_readiness import audit_autonomous_deployment_readiness
from .autonomous_domain_audit import validate_autonomous_domain_audit_report
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA = "bioprism-python-autonomous-launch-preflight/0.1"
AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA = "bioprism-python-autonomous-launch-preflight-domain/0.1"
MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_BYTES = 512_000
MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS = 512
_STATES = ("blocked", "partial", "ready_for_review")
_RETENTION = "metadata_only;source_reports_summarized;runtime_values_not_retained"
_EXECUTION = "preflight_only;no_provider_source_tool_queue_credential_or_learner_dispatch"
_SECRET_KEYS = frozenset(
    {
        "apikey",
        "bearer",
        "body",
        "content",
        "credential",
        "credentials",
        "headers",
        "messages",
        "password",
        "prompt",
        "request",
        "response",
        "secret",
        "task",
        "token",
    }
)


def _text(name: str, value: Any, maximum: int = 2_048) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a bounded non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded size")
    return value


def _digest(name: str, value: Any) -> str:
    value = _text(name, value, 64)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _safe_metadata(value: Any, *, depth: int = 0) -> None:
    if depth > 10:
        raise ArgumentError("launch preflight metadata nesting exceeds its bound")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ArgumentError("launch preflight metadata mapping exceeds its bound")
        for key, child in value.items():
            if not isinstance(key, str):
                raise ArgumentError("launch preflight metadata keys must be strings")
            normalized = re.sub(r"[^a-z0-9]", "", key.lower())
            if normalized in _SECRET_KEYS:
                raise ArgumentError("launch preflight contains transient or secret-shaped metadata")
            _safe_metadata(child, depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise ArgumentError("launch preflight metadata sequence exceeds its bound")
        for child in value:
            _safe_metadata(child, depth=depth + 1)
        return
    if isinstance(value, (bytes, bytearray)):
        raise ArgumentError("launch preflight cannot contain binary material")
    if isinstance(value, float) and not math.isfinite(value):
        raise ArgumentError("launch preflight cannot contain non-finite numbers")
    if isinstance(value, str) and len(value.encode("utf-8")) > 8_192:
        raise ArgumentError("launch preflight text field exceeds its bound")


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(
            json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        )
    except (TypeError, ValueError) as error:
        raise ArgumentError("launch preflight report is not canonical JSON") from error


def _strings(name: str, values: Any, maximum: int = 512) -> list[str]:
    if isinstance(values, (str, bytes)) or not isinstance(values, Sequence) or len(values) > maximum:
        raise ArgumentError(f"{name} is outside its bounded sequence contract")
    result = {_text(f"{name} entry", value, 1_024) for value in values}
    return sorted(result)


def _source_digest(report: Mapping[str, Any], key: str) -> str:
    value = report.get(key)
    if not isinstance(value, str):
        raise ArgumentError(f"launch preflight source report is missing {key}")
    return _digest(f"launch preflight source {key}", value)


def _readiness_summary(report: Mapping[str, Any]) -> dict[str, Any]:
    domains = report.get("domains")
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence):
        raise ArgumentError("launch preflight readiness domains are malformed")
    rows: list[dict[str, Any]] = []
    for raw in domains:
        if not isinstance(raw, Mapping) or raw.get("domain") not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("launch preflight readiness domain row is malformed")
        rows.append(
            {
                "domain": raw["domain"],
                "state": _text("launch preflight readiness state", raw.get("state"), 128),
                "compatible_model_count": raw.get("compatible_model_count", 0),
                "eligible_model_count": raw.get("eligible_model_count", 0),
                "next_actions": _strings("launch preflight readiness next_actions", raw.get("next_actions", ()), 64),
            }
        )
    if {row["domain"] for row in rows} != set(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch preflight readiness must cover all twelve domains")
    providers = report.get("providers")
    if isinstance(providers, (str, bytes)) or not isinstance(providers, Sequence):
        raise ArgumentError("launch preflight readiness providers are malformed")
    provider_rows = []
    for raw in providers:
        if not isinstance(raw, Mapping):
            raise ArgumentError("launch preflight provider row is malformed")
        provider_rows.append(
            {
                "provider": _text("launch preflight provider", raw.get("provider"), 128),
                "provider_registered": raw.get("provider_registered") is True,
                "credential_ready": raw.get("credential_ready", raw.get("ready", False)) is True,
                "next_action": _text("launch preflight provider next_action", raw.get("next_action", "ready"), 512),
            }
        )
    models = report.get("models")
    if isinstance(models, (str, bytes)) or not isinstance(models, Sequence):
        raise ArgumentError("launch preflight readiness models are malformed")
    return {
        "readiness_digest": content_digest(report),
        "readiness_state": _text("launch preflight readiness_state", report.get("readiness_state"), 128),
        "provider_count": len(provider_rows),
        "ready_provider_count": sum(row["credential_ready"] for row in provider_rows),
        "providers": provider_rows,
        "model_count": len(models),
        "domains": sorted(rows, key=lambda row: row["domain"]),
    }


def _deployment_summary(report: Mapping[str, Any]) -> dict[str, Any]:
    domains = report.get("domains")
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence):
        raise ArgumentError("launch preflight deployment domains are malformed")
    rows = []
    for raw in domains:
        if not isinstance(raw, Mapping) or raw.get("domain") not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("launch preflight deployment domain row is malformed")
        rows.append(
            {
                "domain": raw["domain"],
                "state": _text("launch preflight deployment state", raw.get("state"), 128),
                "agent_state": _text("launch preflight deployment agent_state", raw.get("agent_state"), 128),
                "blockers": _bounded_blockers(raw.get("blockers", ()), "deployment blockers"),
                "warnings": _bounded_blockers(raw.get("warnings", ()), "deployment warnings"),
                "next_actions": _strings("launch preflight deployment next_actions", raw.get("next_actions", ()), 64),
            }
        )
    if {row["domain"] for row in rows} != set(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch preflight deployment must cover all twelve domains")
    providers = report.get("provider_gate")
    if not isinstance(providers, Mapping):
        raise ArgumentError("launch preflight provider gate is malformed")
    capabilities = report.get("capabilities")
    if isinstance(capabilities, (str, bytes)) or not isinstance(capabilities, Sequence):
        raise ArgumentError("launch preflight deployment capabilities are malformed")
    capability_rows = []
    for raw in capabilities:
        if not isinstance(raw, Mapping):
            raise ArgumentError("launch preflight capability row is malformed")
        capability_rows.append(
            {
                "name": _text("launch preflight capability name", raw.get("name"), 128),
                "required": raw.get("required") is True,
                "satisfies_requirement": raw.get("satisfies_requirement") is True,
            }
        )
    return {
        "readiness_digest": _source_digest(report, "readiness_digest"),
        "state": _text("launch preflight deployment overall state", report.get("state"), 128),
        "ready_domain_count": report.get("ready_domain_count", 0),
        "partial_domain_count": report.get("partial_domain_count", 0),
        "blocked_domain_count": report.get("blocked_domain_count", 0),
        "provider_gate": {
            "candidate_provider_count": providers.get("candidate_provider_count", 0),
            "ready_provider_count": providers.get("ready_provider_count", 0),
            "unresolved_provider_count": providers.get("unresolved_provider_count", 0),
        },
        "capabilities": sorted(capability_rows, key=lambda row: row["name"]),
        "domains": sorted(rows, key=lambda row: row["domain"]),
        "global_blocker_count": len(_bounded_blockers(report.get("global_blockers", ()), "global blockers")),
        "warning_count": len(_bounded_blockers(report.get("warnings", ()), "global warnings")),
    }


def _bounded_blockers(value: Any, name: str) -> list[dict[str, Any]]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) > 512:
        raise ArgumentError(f"launch preflight {name} are outside their bound")
    result = []
    for raw in value:
        if not isinstance(raw, Mapping):
            raise ArgumentError(f"launch preflight {name} contain a malformed row")
        result.append(
            {
                "code": _text(f"launch preflight {name} code", raw.get("code"), 128),
                "severity": _text(f"launch preflight {name} severity", raw.get("severity", "blocking"), 32),
                "scope": _text(f"launch preflight {name} scope", raw.get("scope"), 32),
                "domain": raw.get("domain"),
                "next_action": _text(f"launch preflight {name} next_action", raw.get("next_action"), 1_024),
            }
        )
    return result


def _combined_state(contract: Mapping[str, Any], readiness: Mapping[str, Any], deployment: Mapping[str, Any]) -> str:
    if contract.get("contract_status") == "invalid" or contract.get("runtime_status") == "blocked":
        return "blocked"
    if deployment.get("state") == "blocked":
        return "blocked"
    if contract.get("runtime_status") != "ready_for_review":
        return "partial"
    if readiness.get("state") != "ready_for_caller_approval":
        return "partial"
    if deployment.get("state") != "ready_for_review":
        return "partial"
    return "ready_for_review"


def audit_autonomous_agent_launch_preflight(
    agent: Any,
    *,
    available_tool_names: Sequence[str] | None = None,
    available_evidence: Sequence[str] | None = None,
    completed_stages: Mapping[str, Sequence[str]] | None = None,
    readiness_options: Mapping[str, Any] | None = None,
    deployment_policy: Mapping[str, Any] | Any | None = None,
    deployment_capabilities: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Compose the current local gates into a twelve-domain launch review."""

    if not callable(getattr(agent, "domain_audit", None)) or not callable(getattr(agent, "readiness", None)):
        raise ArgumentError("launch preflight agent must expose domain_audit and readiness")
    if not isinstance(readiness_options, Mapping) and readiness_options is not None:
        raise ArgumentError("launch preflight readiness_options must be a mapping")
    if deployment_capabilities is not None and not isinstance(deployment_capabilities, Mapping):
        raise ArgumentError("launch preflight deployment_capabilities must be a mapping")
    if deployment_capabilities is not None:
        _safe_metadata(deployment_capabilities)

    contract_report = validate_autonomous_domain_audit_report(
        agent.domain_audit(
            available_tool_names=available_tool_names,
            available_evidence=available_evidence,
            completed_stages=completed_stages,
        )
    )
    readiness = agent.readiness(**({} if readiness_options is None else dict(readiness_options)))
    if not isinstance(readiness, Mapping):
        raise ArgumentError("launch preflight agent readiness did not return a mapping")
    readiness = dict(readiness)
    if "learning" not in readiness:
        readiness["learning"] = {
            "configured": getattr(agent, "ledger", None) is not None,
            "calibration": readiness.get("evaluator_calibration"),
            "execution": "readiness_projection_only;no_learning_mutation",
            "secret_material": "never_returned",
        }
    provider_plan_factory = getattr(agent, "credential_provisioning_plan", None)
    if not callable(provider_plan_factory):
        raise ArgumentError("launch preflight agent must expose credential_provisioning_plan")
    deployment_report = audit_autonomous_deployment_readiness(
        {
            "agent": readiness,
            "provider_plan": provider_plan_factory(),
            "capabilities": {} if deployment_capabilities is None else dict(deployment_capabilities),
        },
        policy=deployment_policy,
    )
    readiness_summary = _readiness_summary(readiness)
    deployment_summary = _deployment_summary(deployment_report)
    audit_rows = {row["domain"]: row for row in contract_report["rows"]}
    readiness_rows = {row["domain"]: row for row in readiness_summary["domains"]}
    deployment_rows = {row["domain"]: row for row in deployment_summary["domains"]}
    domain_rows: list[dict[str, Any]] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        contract = audit_rows[domain]
        readiness_row = readiness_rows[domain]
        deployment_row = deployment_rows[domain]
        state = _combined_state(contract, readiness_row, deployment_row)
        actions = {
            *contract.get("next_actions", ()),
            *readiness_row.get("next_actions", ()),
            *deployment_row.get("next_actions", ()),
        }
        if state == "blocked":
            actions.add("resolve blocking launch-preflight gates before dispatch")
        elif state == "partial":
            actions.add("complete caller-owned launch-preflight inputs before dispatch review")
        domain_rows.append(
            {
                "schema": AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA,
                "domain": domain,
                "state": state,
                "contract_status": contract["contract_status"],
                "contract_runtime_status": contract["runtime_status"],
                "contract_row_digest": contract["row_digest"],
                "readiness_state": readiness_row["state"],
                "deployment_state": deployment_row["state"],
                "blocker_count": len(deployment_row["blockers"]),
                "warning_count": len(deployment_row["warnings"]),
                "next_actions": sorted(actions)[:64],
                "retention": _RETENTION,
                "execution": _EXECUTION,
                "secret_material": "never_returned",
            }
        )

    blocked_count = sum(row["state"] == "blocked" for row in domain_rows)
    partial_count = sum(row["state"] == "partial" for row in domain_rows)
    ready_count = sum(row["state"] == "ready_for_review" for row in domain_rows)
    state = "blocked" if blocked_count else "partial" if partial_count else "ready_for_review"
    next_actions = sorted(
        {
            *contract_report["next_actions"],
            *readiness.get("next_actions", ()),
            *deployment_report.get("next_actions", ()),
            *(action for row in domain_rows for action in row["next_actions"]),
        }
    )[:MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS]
    body = {
        "schema": AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA,
        "contract_audit": {
            "report_digest": contract_report["report_digest"],
            "static_contract_status": contract_report["summary"]["static_contract_status"],
            "runtime_status": contract_report["summary"]["runtime_status"],
            "domain_count": contract_report["summary"]["domain_count"],
            "valid_domain_count": contract_report["summary"]["valid_domain_count"],
            "runtime_ready_domain_count": contract_report["summary"]["runtime_ready_domain_count"],
            "runtime_partial_domain_count": contract_report["summary"]["runtime_partial_domain_count"],
            "runtime_unassessed_domain_count": contract_report["summary"]["runtime_unassessed_domain_count"],
        },
        "agent_readiness": readiness_summary,
        "deployment_readiness": deployment_summary,
        "domains": domain_rows,
        "summary": {
            "state": state,
            "domain_count": len(domain_rows),
            "ready_domain_count": ready_count,
            "partial_domain_count": partial_count,
            "blocked_domain_count": blocked_count,
            "contract_report_digest": contract_report["report_digest"],
            "readiness_report_digest": readiness_summary["readiness_digest"],
            "deployment_report_digest": deployment_summary["readiness_digest"],
        },
        "next_actions": next_actions,
        "dispatch": {
            "status": "not_started",
            "authorization": "preflight_review_only;does_not_grant_provider_source_tool_or_effect_authority",
            "provider_calls": 0,
            "source_calls": 0,
            "tool_calls": 0,
            "learner_mutations": 0,
            "credential_resolution": 0,
        },
        "retention": _RETENTION,
        "execution": _EXECUTION,
        "credential_posture": "caller_owned_opaque_handles_only;none_consumed",
        "secret_material": "never_returned",
    }
    _safe_metadata(body)
    report = {**body, "report_digest": content_digest(body)}
    if len(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_BYTES:
        raise ArgumentError("launch preflight report exceeds its bounded size")
    return _clone(report)


def validate_autonomous_launch_preflight_report(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate a launch-preflight handoff and its aggregate digest."""

    if not isinstance(value, Mapping):
        raise ArgumentError("launch preflight report must be a mapping")
    _safe_metadata(value)
    report = _clone(value)
    expected = {
        "schema", "contract_audit", "agent_readiness", "deployment_readiness", "domains", "summary",
        "next_actions", "dispatch", "retention", "execution", "credential_posture", "secret_material", "report_digest",
    }
    if set(report) != expected or report["schema"] != AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA:
        raise ArgumentError("launch preflight report contains unsupported or missing fields")
    if report["retention"] != _RETENTION or report["execution"] != _EXECUTION or report["credential_posture"] != "caller_owned_opaque_handles_only;none_consumed" or report["secret_material"] != "never_returned":
        raise ArgumentError("launch preflight report execution posture is unsafe")
    supplied = _digest("launch preflight report_digest", report["report_digest"])
    unsigned = dict(report)
    unsigned.pop("report_digest")
    if content_digest(unsigned) != supplied:
        raise ArgumentError("launch preflight report_digest does not match its metadata")
    domains = report["domains"]
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence) or len(domains) != len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch preflight domains are outside their bound")
    seen: set[str] = set()
    for row in domains:
        if not isinstance(row, Mapping) or row.get("schema") != AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA:
            raise ArgumentError("launch preflight domain row is malformed")
        domain = row.get("domain")
        if domain not in AUTONOMOUS_DOMAIN_NAMES or domain in seen:
            raise ArgumentError("launch preflight domains are duplicated or unsupported")
        seen.add(domain)
        if row.get("state") not in _STATES or row.get("contract_status") not in {"valid", "invalid"}:
            raise ArgumentError("launch preflight domain state is invalid")
        _text("launch preflight contract_runtime_status", row.get("contract_runtime_status"), 64)
        _text("launch preflight readiness_state", row.get("readiness_state"), 128)
        _text("launch preflight deployment_state", row.get("deployment_state"), 128)
        _digest("launch preflight contract_row_digest", row.get("contract_row_digest"))
        _strings("launch preflight domain next_actions", row.get("next_actions"), 64)
    if seen != set(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch preflight does not cover all twelve domains")
    summary = report["summary"]
    if not isinstance(summary, Mapping) or summary.get("domain_count") != len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch preflight summary is malformed")
    for key in ("ready_domain_count", "partial_domain_count", "blocked_domain_count"):
        count = summary.get(key)
        if isinstance(count, bool) or not isinstance(count, int) or not 0 <= count <= len(AUTONOMOUS_DOMAIN_NAMES):
            raise ArgumentError(f"launch preflight summary {key} is malformed")
    if sum(summary[key] for key in ("ready_domain_count", "partial_domain_count", "blocked_domain_count")) != len(domains):
        raise ArgumentError("launch preflight summary counts do not reconcile")
    _strings("launch preflight next_actions", report["next_actions"], MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS)
    dispatch = report["dispatch"]
    if not isinstance(dispatch, Mapping) or dispatch.get("status") != "not_started" or dispatch.get("authorization") != "preflight_review_only;does_not_grant_provider_source_tool_or_effect_authority":
        raise ArgumentError("launch preflight dispatch posture is unsafe")
    for key in ("provider_calls", "source_calls", "tool_calls", "learner_mutations", "credential_resolution"):
        if dispatch.get(key) != 0:
            raise ArgumentError("launch preflight reports unexpected dispatch activity")
    return report


__all__ = [
    "AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA",
    "AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA",
    "MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_BYTES",
    "MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS",
    "audit_autonomous_agent_launch_preflight",
    "validate_autonomous_launch_preflight_report",
]
