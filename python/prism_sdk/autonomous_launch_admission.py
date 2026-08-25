"""Digest-bound caller review handoff for the autonomous launch preflight.

The preflight report is intentionally descriptive.  This module adds the next explicit boundary:
the caller can record an ``approve`` or ``hold`` decision against one exact preflight digest and
the twelve domain rows.  The result is still not provider, source, tool, learner, queue, or effect
authority.  It is a restart-safe value-only admission record that an embedding deployment can bind
to its own authorization and execution controller.
"""

from __future__ import annotations

import json
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA = "bioprism-python-autonomous-launch-admission/0.1"
AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA = "bioprism-python-autonomous-launch-admission-domain/0.1"
MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES = 256_000
MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS = 512

_DECISIONS = frozenset({"approve", "hold"})
_ADMISSION_STATES = frozenset({"approved", "held", "blocked", "not_selected"})
_PREFLIGHT_STATES = frozenset({"blocked", "partial", "ready_for_review"})
_RETENTION = "metadata_only;preflight_and_review_digests_only;runtime_values_not_retained"
_EXECUTION = "admission_only;does_not_grant_provider_source_tool_effect_credential_or_learner_authority"
_AUTHORITY = "caller_review_record_only;authorization_digest_is_not_verified_by_sdk"
_SECRET_MATERIAL = "never_returned"
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


def _identifier(name: str, value: Any) -> str:
    result = _text(name, value, 256)
    if not re.fullmatch(r"[A-Za-z0-9_.:+-]+", result):
        raise ArgumentError(f"{name} is not a safe identifier")
    return result


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    result = _text(name, value, 64)
    if len(result) != 64 or any(character not in "0123456789abcdef" for character in result):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return result


def _safe_metadata(value: Any, *, depth: int = 0) -> None:
    if depth > 10:
        raise ArgumentError("launch admission metadata nesting exceeds its bound")
    if isinstance(value, Mapping):
        if len(value) > 512:
            raise ArgumentError("launch admission metadata mapping exceeds its bound")
        for key, child in value.items():
            if not isinstance(key, str):
                raise ArgumentError("launch admission metadata keys must be strings")
            normalized = re.sub(r"[^a-z0-9]", "", key.lower())
            if normalized in _SECRET_KEYS:
                raise ArgumentError("launch admission contains transient or secret-shaped metadata")
            _safe_metadata(child, depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 512:
            raise ArgumentError("launch admission metadata sequence exceeds its bound")
        for child in value:
            _safe_metadata(child, depth=depth + 1)
        return
    if isinstance(value, (bytes, bytearray)):
        raise ArgumentError("launch admission cannot contain binary material")
    if isinstance(value, float) and not (-float("inf") < value < float("inf")):
        raise ArgumentError("launch admission cannot contain non-finite numbers")
    if isinstance(value, str) and len(value.encode("utf-8")) > 8_192:
        raise ArgumentError("launch admission text field exceeds its bound")


def _clone(value: Mapping[str, Any]) -> dict[str, Any]:
    try:
        return json.loads(json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False))
    except (TypeError, ValueError) as error:
        raise ArgumentError("launch admission report is not canonical JSON") from error


def _strings(name: str, value: Any, maximum: int = 512) -> list[str]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounded sequence contract")
    return sorted({_text(f"{name} entry", item, 1_024) for item in value})


def _domains(value: Any) -> tuple[str, ...]:
    if value is None:
        return tuple(AUTONOMOUS_DOMAIN_NAMES)
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or not value:
        raise ArgumentError("launch admission approved_domains must be a non-empty sequence")
    result = tuple(_text("launch admission approved domain", item, 128) for item in value)
    if len(result) > len(AUTONOMOUS_DOMAIN_NAMES) or len(set(result)) != len(result):
        raise ArgumentError("launch admission approved_domains must contain unique built-in domains")
    if any(domain not in AUTONOMOUS_DOMAIN_NAMES for domain in result):
        raise ArgumentError("launch admission approved_domains contains an unsupported domain")
    return result


def _preflight_report(value: Mapping[str, Any]) -> dict[str, Any]:
    from .autonomous_launch_preflight import validate_autonomous_launch_preflight_report

    return validate_autonomous_launch_preflight_report(value)


def _row(value: Mapping[str, Any]) -> dict[str, Any]:
    if value.get("schema") != "bioprism-python-autonomous-launch-preflight-domain/0.1":
        raise ArgumentError("launch admission preflight domain row has an unsupported schema")
    domain = value.get("domain")
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        raise ArgumentError("launch admission preflight domain is unsupported")
    state = _text("launch admission preflight domain state", value.get("state"), 64)
    if state not in _PREFLIGHT_STATES:
        raise ArgumentError("launch admission preflight domain state is invalid")
    return {
        "domain": domain,
        "state": state,
        "contract_status": _text("launch admission contract status", value.get("contract_status"), 32),
        "contract_row_digest": _digest("launch admission contract row digest", value.get("contract_row_digest")),
        "readiness_state": _text("launch admission readiness state", value.get("readiness_state"), 128),
        "deployment_state": _text("launch admission deployment state", value.get("deployment_state"), 128),
        "next_actions": _strings("launch admission preflight next actions", value.get("next_actions", ()), 64),
    }


def create_autonomous_launch_admission(
    preflight_report: Mapping[str, Any],
    *,
    decision: str,
    approved_domains: Sequence[str] | None = None,
    authorization_digest: str | None = None,
    reason: str | None = None,
    admission_id: str = "autonomous-launch-admission",
) -> dict[str, Any]:
    """Record an explicit caller decision against one exact launch-preflight digest."""

    report = _preflight_report(preflight_report)
    decision = _text("launch admission decision", decision, 16)
    if decision not in _DECISIONS:
        raise ArgumentError("launch admission decision must be approve or hold")
    selected = set(_domains(approved_domains))
    normalized_admission_id = _identifier("launch admission admission_id", admission_id)
    if reason is not None:
        _text("launch admission reason", reason, 4_096)
    normalized_authorization = _digest("launch admission authorization_digest", authorization_digest, allow_none=True)
    if decision == "approve" and normalized_authorization is None:
        raise ArgumentError("launch admission approve requires an authorization_digest")
    rows = []
    for raw in report["domains"]:
        normalized = _row(raw)
        domain = normalized["domain"]
        preflight_state = normalized["state"]
        if domain not in selected:
            admission_state = "not_selected"
        elif preflight_state == "blocked":
            admission_state = "blocked"
        elif decision == "approve" and preflight_state == "ready_for_review":
            admission_state = "approved"
        else:
            admission_state = "held"
        actions = set(normalized["next_actions"])
        if admission_state == "blocked":
            actions.add("resolve the blocked preflight gate before requesting launch approval")
        elif admission_state == "held":
            actions.add("complete the preflight or obtain an explicit deployment decision before launch")
        elif admission_state == "not_selected":
            actions.add("select this domain explicitly before dispatch")
        rows.append(
            {
                "schema": AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA,
                "domain": domain,
                "preflight_state": preflight_state,
                "admission_state": admission_state,
                "contract_row_digest": normalized["contract_row_digest"],
                "readiness_state": normalized["readiness_state"],
                "deployment_state": normalized["deployment_state"],
                "next_actions": sorted(actions)[:64],
                "retention": _RETENTION,
                "execution": _EXECUTION,
                "secret_material": _SECRET_MATERIAL,
            }
        )
    approved_count = sum(row["admission_state"] == "approved" for row in rows)
    held_count = sum(row["admission_state"] == "held" for row in rows)
    blocked_count = sum(row["admission_state"] == "blocked" for row in rows)
    selected_count = sum(row["admission_state"] != "not_selected" for row in rows)
    status = "approved" if decision == "approve" and selected_count > 0 and approved_count == selected_count else "held"
    next_actions = sorted({
        *report["next_actions"],
        *(action for row in rows for action in row["next_actions"] if row["admission_state"] != "approved"),
    })[:MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS]
    body = {
        "schema": AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA,
        "admission_id": normalized_admission_id,
        "preflight_report_digest": report["report_digest"],
        "decision": decision,
        "status": status,
        "authorization_digest": normalized_authorization,
        "reason_digest": None if reason is None else content_digest(reason),
        "domains": rows,
        "summary": {
            "domain_count": len(rows),
            "selected_domain_count": selected_count,
            "approved_domain_count": approved_count,
            "held_domain_count": held_count,
            "blocked_domain_count": blocked_count,
            "not_selected_domain_count": len(rows) - selected_count,
        },
        "next_actions": next_actions,
        "authority": _AUTHORITY,
        "retention": _RETENTION,
        "execution": _EXECUTION,
        "credential_posture": "caller_owned_opaque_handles_only;none_consumed",
        "secret_material": _SECRET_MATERIAL,
    }
    _safe_metadata(body)
    report_out = {**body, "admission_digest": content_digest(body)}
    encoded = json.dumps(report_out, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    if len(encoded) > MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES:
        raise ArgumentError("launch admission report exceeds its bounded size")
    return _clone(report_out)


def validate_autonomous_launch_admission(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate an admission record before a caller binds it to another gate."""

    if not isinstance(value, Mapping):
        raise ArgumentError("launch admission report must be a mapping")
    _safe_metadata(value)
    report = _clone(value)
    expected = {
        "schema", "admission_id", "preflight_report_digest", "decision", "status", "authorization_digest",
        "reason_digest", "domains", "summary", "next_actions", "authority", "retention", "execution",
        "credential_posture", "secret_material", "admission_digest",
    }
    if set(report) != expected or report["schema"] != AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA:
        raise ArgumentError("launch admission report contains unsupported or missing fields")
    if report["authority"] != _AUTHORITY or report["retention"] != _RETENTION or report["execution"] != _EXECUTION or report["credential_posture"] != "caller_owned_opaque_handles_only;none_consumed" or report["secret_material"] != _SECRET_MATERIAL:
        raise ArgumentError("launch admission report execution posture is unsafe")
    supplied = _digest("launch admission admission_digest", report["admission_digest"])
    unsigned = dict(report)
    unsigned.pop("admission_digest")
    if content_digest(unsigned) != supplied:
        raise ArgumentError("launch admission admission_digest does not match its metadata")
    _identifier("launch admission admission_id", report["admission_id"])
    _digest("launch admission preflight_report_digest", report["preflight_report_digest"])
    decision = _text("launch admission decision", report["decision"], 16)
    status = _text("launch admission status", report["status"], 16)
    if decision not in _DECISIONS or status not in {"approved", "held"}:
        raise ArgumentError("launch admission decision or status is invalid")
    _digest("launch admission authorization_digest", report["authorization_digest"], allow_none=True)
    _digest("launch admission reason_digest", report["reason_digest"], allow_none=True)
    domains = report["domains"]
    if isinstance(domains, (str, bytes)) or not isinstance(domains, Sequence) or len(domains) != len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch admission domains are outside their bound")
    seen: set[str] = set()
    counts = {"approved": 0, "held": 0, "blocked": 0, "not_selected": 0}
    for raw in domains:
        if not isinstance(raw, Mapping) or raw.get("schema") != AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA:
            raise ArgumentError("launch admission domain row is malformed")
        domain = raw.get("domain")
        if domain not in AUTONOMOUS_DOMAIN_NAMES or domain in seen:
            raise ArgumentError("launch admission domains are duplicated or unsupported")
        seen.add(domain)
        state = _text("launch admission domain admission_state", raw.get("admission_state"), 32)
        if state not in _ADMISSION_STATES:
            raise ArgumentError("launch admission domain admission_state is invalid")
        preflight_state = _text("launch admission domain preflight_state", raw.get("preflight_state"), 64)
        if preflight_state not in _PREFLIGHT_STATES:
            raise ArgumentError("launch admission domain preflight_state is invalid")
        _digest("launch admission domain contract_row_digest", raw.get("contract_row_digest"))
        _text("launch admission domain readiness_state", raw.get("readiness_state"), 128)
        _text("launch admission domain deployment_state", raw.get("deployment_state"), 128)
        _strings("launch admission domain next_actions", raw.get("next_actions"), 64)
        if raw.get("retention") != _RETENTION or raw.get("execution") != _EXECUTION or raw.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("launch admission domain row execution posture is unsafe")
        counts[state] += 1
    if seen != set(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError("launch admission does not cover all twelve domains")
    summary = report["summary"]
    if not isinstance(summary, Mapping) or summary.get("domain_count") != len(domains):
        raise ArgumentError("launch admission summary is malformed")
    for key in ("selected_domain_count", "approved_domain_count", "held_domain_count", "blocked_domain_count", "not_selected_domain_count"):
        value_int = summary.get(key)
        if isinstance(value_int, bool) or not isinstance(value_int, int) or not 0 <= value_int <= len(domains):
            raise ArgumentError(f"launch admission summary {key} is malformed")
    if summary["approved_domain_count"] != counts["approved"] or summary["held_domain_count"] != counts["held"] or summary["blocked_domain_count"] != counts["blocked"] or summary["not_selected_domain_count"] != counts["not_selected"] or summary["selected_domain_count"] != len(domains) - counts["not_selected"]:
        raise ArgumentError("launch admission summary counts do not reconcile")
    if status == "approved" and (decision != "approve" or counts["approved"] != summary["selected_domain_count"] or summary["selected_domain_count"] == 0):
        raise ArgumentError("launch admission approved status is inconsistent")
    if decision == "approve" and report["authorization_digest"] is None:
        raise ArgumentError("launch admission approval is missing its authorization digest")
    _strings("launch admission next_actions", report["next_actions"], MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS)
    return report


__all__ = [
    "AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA",
    "AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA",
    "MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES",
    "MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS",
    "create_autonomous_launch_admission",
    "validate_autonomous_launch_admission",
]
