"""Deterministic, metadata-only recovery planning for autonomous runs.

The provider runtime and execution facades deliberately return held, failed, and reconciliation
states instead of hiding them behind a generic exception.  This module turns that small
value-only observation into a bounded operator/queue handoff.  It never retries, resolves a key,
calls a provider, executes a tool, settles an evaluator, or reconciles an external effect; those
authorities stay with the embedding deployment.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_RECOVERY_PLAN_SCHEMA = "bioprism-python-autonomous-recovery-plan/0.1"
AUTONOMOUS_RECOVERY_RETENTION = (
    "metadata_only_recovery_handoff;task_prompt_provider_response_credentials_and_effect_values_not_retained"
)
AUTONOMOUS_RECOVERY_AUTHORITY = (
    "guidance_only;does_not_authorize_retry_provider_source_tool_evaluator_or_effect"
)
AUTONOMOUS_RECOVERY_ACTIONS = (
    "complete",
    "retry_provider",
    "refresh_provider_health",
    "collect_credential",
    "approve_provider_call",
    "review_route",
    "review_domain_policy",
    "review_response_quality",
    "review_tool_authorization",
    "reconcile_external_effect",
    "retry_after_review",
    "stop_and_escalate",
)
MAX_AUTONOMOUS_RECOVERY_ACTIONS = 16
MAX_AUTONOMOUS_RECOVERY_REASON_CODES = 16
MAX_AUTONOMOUS_RECOVERY_CAPABILITY_BYTES = 256

_IDENTIFIER = re.compile(r"^[A-Za-z0-9_.:-]+$")
_SECRET_KEYS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
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
        "privatekey",
        "rawpayload",
        "arguments",
        "output",
    }
)

_DOMAIN_GUARDRAILS: dict[str, tuple[str, ...]] = {
    "coding": ("report_verification_that_actually_ran", "preserve_rollback_and_diff_review"),
    "browser": ("recheck_source_identity_and_freshness", "do_not_treat_page_access_as_truth"),
    "data": ("recheck_schema_and_provenance", "report_missingness_before_interpretation"),
    "science": ("separate_hypothesis_from_observation", "preserve_uncertainty_and_reproduction"),
    "biomedical": (
        "require_qualified_human_review_for_high_impact_claims",
        "do_not_diagnose_or_prescribe",
    ),
    "neuroscience": ("preserve_specimen_and_coordinate_scope", "escalate_interpretive_uncertainty"),
    "operations": ("require_operator_approval_before_effects", "preserve_stop_conditions_and_rollback"),
    "enterprise": ("recheck_owner_and_policy_scope", "keep_external_effects_separately_authorized"),
    "multi_agent": ("retain_one_accountable_coordinator", "reconcile_specialist_dissent_before_synthesis"),
    "multimodal": ("identify_uninspected_modalities", "do_not_infer_absent_observations"),
    "cross_domain": ("reconcile_domain_scopes_before_synthesis", "keep_claims_attached_to_specialists"),
    "evaluation": ("keep_evaluator_independent_of_subject", "preserve_holdout_and_replay_evidence"),
}


def _fail(message: str) -> None:
    raise ArgumentError(f"autonomous recovery {message}")


def _text(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        _fail(f"{name} is outside its bounded text contract")
    if len(value) > maximum:
        _fail(f"{name} exceeds its bounded text contract")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    result = _text(name, value, maximum)
    if not _IDENTIFIER.fullmatch(result):
        _fail(f"{name} must be a bounded identifier")
    return result


def _boolean(name: str, value: Any, default: bool) -> bool:
    if value is None:
        return default
    if not isinstance(value, bool):
        _fail(f"{name} must be boolean")
    return value


def _count(name: str, value: Any, default: int) -> int:
    result = default if value is None else value
    if isinstance(result, bool) or not isinstance(result, int) or not 0 <= result <= 64:
        _fail(f"{name} must be an integer within [0, 64]")
    return result


def _string_list(name: str, value: Any, maximum: int) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or len(value) > maximum:
        _fail(f"{name} exceeds its bounded list size")
    result = tuple(_identifier(f"{name} item", item) for item in value)
    if len(set(result)) != len(result):
        _fail(f"{name} contains duplicate items")
    return result


def _actions(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)) or not 1 <= len(value) <= MAX_AUTONOMOUS_RECOVERY_ACTIONS:
        _fail(f"{name} must contain 1..{MAX_AUTONOMOUS_RECOVERY_ACTIONS} actions")
    result = tuple(_identifier(f"{name} item", item) for item in value)
    if any(item not in AUTONOMOUS_RECOVERY_ACTIONS for item in result):
        _fail(f"{name} contains an unsupported action")
    if len(set(result)) != len(result):
        _fail(f"{name} contains duplicate actions")
    return result


def _secret_free(value: Any, depth: int = 0) -> None:
    if depth > 8:
        _fail("metadata is too deeply nested")
    if isinstance(value, Mapping):
        for key, child in value.items():
            if not isinstance(key, str):
                _fail("metadata contains a non-string key")
            normalized = re.sub(r"[^a-z0-9]", "", key.lower())
            if normalized in _SECRET_KEYS:
                _fail("metadata contains transient or secret-shaped fields")
            _secret_free(child, depth + 1)
    elif isinstance(value, (list, tuple)):
        for child in value:
            _secret_free(child, depth + 1)


def _unique(values: Sequence[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


@dataclass(frozen=True, slots=True)
class AutonomousRecoveryObservation:
    domain: str
    capability: str
    status: str
    failure_class: str | None = None
    failure_code: str | None = None
    retryable: bool = False
    retry_count: int = 0
    max_retries: int = 2
    approval_required: bool = False
    reconciliation_required: bool = False
    provider_configured: bool = True
    credential_ready: bool = True
    route_reviewed: bool = True
    policy_admitted: bool = True
    response_quality_passed: bool | None = None
    tool_authorization_ready: bool = True

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | "AutonomousRecoveryObservation") -> "AutonomousRecoveryObservation":
        if isinstance(value, cls):
            return value
        if not isinstance(value, Mapping):
            _fail("observation must be a mapping")
        allowed = {
            "domain",
            "capability",
            "status",
            "failure_class",
            "failure_code",
            "retryable",
            "retry_count",
            "max_retries",
            "approval_required",
            "reconciliation_required",
            "provider_configured",
            "credential_ready",
            "route_reviewed",
            "policy_admitted",
            "response_quality_passed",
            "tool_authorization_ready",
        }
        if set(value).difference(allowed):
            _fail("observation contains unsupported fields")
        _secret_free(value)
        domain = value.get("domain")
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            _fail("observation domain is unsupported")
        failure_class = None if value.get("failure_class") is None else _identifier("observation failure_class", value.get("failure_class"), 128)
        failure_code = None if value.get("failure_code") is None else _identifier("observation failure_code", value.get("failure_code"), 128)
        retry_count = _count("observation retry_count", value.get("retry_count"), 0)
        max_retries = _count("observation max_retries", value.get("max_retries"), 2)
        if retry_count > max_retries:
            _fail("observation retry_count exceeds max_retries")
        quality = value.get("response_quality_passed")
        if quality is not None and not isinstance(quality, bool):
            _fail("observation response_quality_passed must be boolean or null")
        return cls(
            domain=domain,
            capability=_identifier("observation capability", value.get("capability"), MAX_AUTONOMOUS_RECOVERY_CAPABILITY_BYTES),
            status=_identifier("observation status", value.get("status"), 128),
            failure_class=failure_class,
            failure_code=failure_code,
            retryable=_boolean("observation retryable", value.get("retryable"), False),
            retry_count=retry_count,
            max_retries=max_retries,
            approval_required=_boolean("observation approval_required", value.get("approval_required"), False),
            reconciliation_required=_boolean("observation reconciliation_required", value.get("reconciliation_required"), False),
            provider_configured=_boolean("observation provider_configured", value.get("provider_configured"), True),
            credential_ready=_boolean("observation credential_ready", value.get("credential_ready"), True),
            route_reviewed=_boolean("observation route_reviewed", value.get("route_reviewed"), True),
            policy_admitted=_boolean("observation policy_admitted", value.get("policy_admitted"), True),
            response_quality_passed=quality,
            tool_authorization_ready=_boolean("observation tool_authorization_ready", value.get("tool_authorization_ready"), True),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "domain": self.domain,
            "capability": self.capability,
            "status": self.status,
            "failure_class": self.failure_class,
            "failure_code": self.failure_code,
            "retryable": self.retryable,
            "retry_count": self.retry_count,
            "max_retries": self.max_retries,
            "approval_required": self.approval_required,
            "reconciliation_required": self.reconciliation_required,
            "provider_configured": self.provider_configured,
            "credential_ready": self.credential_ready,
            "route_reviewed": self.route_reviewed,
            "policy_admitted": self.policy_admitted,
            "response_quality_passed": self.response_quality_passed,
            "tool_authorization_ready": self.tool_authorization_ready,
        }


@dataclass(frozen=True, slots=True)
class AutonomousRecoveryPlan:
    domain: str
    capability: str
    observed_status: str
    status: str
    next_action: str
    actions: tuple[str, ...]
    retryable: bool
    retry_count: int
    max_retries: int
    reason_codes: tuple[str, ...]
    domain_guardrails: tuple[str, ...]
    plan_digest: str

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            _fail("plan domain is unsupported")
        _identifier("plan capability", self.capability, MAX_AUTONOMOUS_RECOVERY_CAPABILITY_BYTES)
        _identifier("plan observed_status", self.observed_status, 128)
        if self.status not in {"completed", "retryable", "held", "reconciliation_required", "blocked"}:
            _fail("plan status is invalid")
        if self.next_action not in AUTONOMOUS_RECOVERY_ACTIONS:
            _fail("plan next_action is invalid")
        actions = _actions("plan actions", self.actions)
        if actions[0] != self.next_action:
            _fail("plan next_action must be the first action")
        if not isinstance(self.retryable, bool):
            _fail("plan retryable must be boolean")
        retry_count = _count("plan retry_count", self.retry_count, 0)
        max_retries = _count("plan max_retries", self.max_retries, 2)
        if retry_count > max_retries:
            _fail("plan retry_count exceeds max_retries")
        _string_list("plan reason_codes", self.reason_codes, MAX_AUTONOMOUS_RECOVERY_REASON_CODES)
        guardrails = _string_list("plan domain_guardrails", self.domain_guardrails, 8)
        if any(guardrail not in _DOMAIN_GUARDRAILS[self.domain] for guardrail in guardrails):
            _fail("plan domain_guardrails do not match the domain contract")
        _identifier("plan_digest", self.plan_digest)

    def _body(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_RECOVERY_PLAN_SCHEMA,
            "domain": self.domain,
            "capability": self.capability,
            "observed_status": self.observed_status,
            "status": self.status,
            "next_action": self.next_action,
            "actions": list(self.actions),
            "retryable": self.retryable,
            "retry_count": self.retry_count,
            "max_retries": self.max_retries,
            "reason_codes": list(self.reason_codes),
            "domain_guardrails": list(self.domain_guardrails),
            "authority": AUTONOMOUS_RECOVERY_AUTHORITY,
            "retention": AUTONOMOUS_RECOVERY_RETENTION,
            "secret_material": "never_returned",
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._body(), "plan_digest": self.plan_digest}

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousRecoveryPlan":
        if not isinstance(value, Mapping):
            _fail("plan must be a mapping")
        allowed = {
            "schema",
            "domain",
            "capability",
            "observed_status",
            "status",
            "next_action",
            "actions",
            "retryable",
            "retry_count",
            "max_retries",
            "reason_codes",
            "domain_guardrails",
            "authority",
            "retention",
            "secret_material",
            "plan_digest",
        }
        if set(value).difference(allowed):
            _fail("plan contains unsupported fields")
        _secret_free(value)
        if value.get("schema") != AUTONOMOUS_RECOVERY_PLAN_SCHEMA or value.get("authority") != AUTONOMOUS_RECOVERY_AUTHORITY or value.get("retention") != AUTONOMOUS_RECOVERY_RETENTION or value.get("secret_material") != "never_returned":
            _fail("plan retention markers are invalid")
        plan = cls(
            domain=value.get("domain"),
            capability=value.get("capability"),
            observed_status=value.get("observed_status"),
            status=value.get("status"),
            next_action=value.get("next_action"),
            actions=tuple(value.get("actions", ())),
            retryable=value.get("retryable"),
            retry_count=value.get("retry_count"),
            max_retries=value.get("max_retries"),
            reason_codes=tuple(value.get("reason_codes", ())),
            domain_guardrails=tuple(value.get("domain_guardrails", ())),
            plan_digest=value.get("plan_digest"),
        )
        if plan.plan_digest != content_digest(plan._body()):
            _fail("plan digest does not match metadata")
        return plan


def plan_autonomous_recovery(
    observation: Mapping[str, Any] | AutonomousRecoveryObservation,
) -> AutonomousRecoveryPlan:
    """Build a deterministic recovery handoff without performing recovery."""

    value = AutonomousRecoveryObservation.from_mapping(observation)
    failure = f"{value.failure_class or ''} {value.failure_code or ''}".lower()
    reasons: list[str] = []
    if value.status in {"completed", "children_completed"}:
        status, next_action, actions = "completed", "complete", ("complete",)
        reasons.append("run_completed")
    elif value.reconciliation_required or value.status == "reconciliation_required" or "reconcil" in failure:
        status, next_action, actions = "reconciliation_required", "reconcile_external_effect", ("reconcile_external_effect", "stop_and_escalate")
        reasons.append("external_state_is_uncertain")
    elif value.approval_required or value.status == "approval_required" or "approval" in failure or "approve" in failure:
        status, next_action, actions = "held", "approve_provider_call", ("approve_provider_call", "review_tool_authorization", "stop_and_escalate")
        reasons.append("explicit_approval_is_missing")
    elif not value.policy_admitted or value.status in {"policy_blocked", "policy_review_required"} or "policy" in failure:
        status, next_action, actions = "held", "review_domain_policy", ("review_domain_policy", "stop_and_escalate")
        reasons.append("domain_policy_is_not_admitted")
    elif not value.route_reviewed or value.status in {"route_review_required", "abstained"}:
        status, next_action, actions = "held", "review_route", ("review_route", "stop_and_escalate")
        reasons.append("route_requires_review")
    elif not value.provider_configured or any(term in failure for term in ("configuration", "provider_missing", "not_configured")):
        status, next_action, actions = "blocked", "stop_and_escalate", ("stop_and_escalate",)
        reasons.append("provider_configuration_is_missing")
    elif not value.credential_ready or any(term in failure for term in ("credential", "authentication", "unauthorized", "forbidden")):
        status, next_action, actions = "blocked", "collect_credential", ("collect_credential", "retry_provider", "stop_and_escalate")
        reasons.append("caller_credential_is_not_ready")
    elif value.response_quality_passed is False or value.status == "response_review_required" or "quality" in failure or "response_review" in failure:
        status, next_action, actions = "held", "review_response_quality", ("review_response_quality", "retry_after_review", "stop_and_escalate")
        reasons.append("response_quality_requires_explicit_review")
    elif not value.tool_authorization_ready or "tool" in failure and "author" in failure:
        status, next_action, actions = "held", "review_tool_authorization", ("review_tool_authorization", "retry_after_review", "stop_and_escalate")
        reasons.append("tool_authorization_is_not_ready")
    elif value.retryable and value.retry_count >= value.max_retries:
        status, next_action, actions = "blocked", "stop_and_escalate", ("stop_and_escalate",)
        reasons.append("retry_budget_exhausted")
    elif value.retryable and value.retry_count < value.max_retries:
        status, next_action, actions = "retryable", "retry_provider", ("retry_provider", "refresh_provider_health", "stop_and_escalate")
        reasons.append("bounded_retry_budget_remains")
    elif any(code in failure for code in ("timeout", "transport", "http_5xx", "circuit_open", "provider_error")):
        status, next_action, actions = "blocked", "refresh_provider_health", ("refresh_provider_health", "stop_and_escalate")
        reasons.append("provider_failure_is_not_retryable_in_this_context")
    elif value.status in {"turn_limit_reached", "child_failed", "cross_domain_partial"}:
        status, next_action, actions = "held", "retry_after_review", ("retry_after_review", "review_route", "stop_and_escalate")
        reasons.append("bounded_execution_did_not_reach_a_complete_result")
    else:
        status, next_action, actions = "blocked", "stop_and_escalate", ("stop_and_escalate",)
        reasons.append("unclassified_failure_requires_review")

    body = {
        "schema": AUTONOMOUS_RECOVERY_PLAN_SCHEMA,
        "domain": value.domain,
        "capability": value.capability,
        "observed_status": value.status,
        "status": status,
        "next_action": next_action,
        "actions": list(actions),
        "retryable": status == "retryable" or "retry_provider" in actions or "retry_after_review" in actions,
        "retry_count": value.retry_count,
        "max_retries": value.max_retries,
        "reason_codes": list(_unique(reasons)),
        "domain_guardrails": list(_DOMAIN_GUARDRAILS[value.domain]),
        "authority": AUTONOMOUS_RECOVERY_AUTHORITY,
        "retention": AUTONOMOUS_RECOVERY_RETENTION,
        "secret_material": "never_returned",
    }
    return AutonomousRecoveryPlan(
        domain=body["domain"],
        capability=body["capability"],
        observed_status=body["observed_status"],
        status=body["status"],
        next_action=body["next_action"],
        actions=tuple(body["actions"]),
        retryable=body["retryable"],
        retry_count=body["retry_count"],
        max_retries=body["max_retries"],
        reason_codes=tuple(body["reason_codes"]),
        domain_guardrails=tuple(body["domain_guardrails"]),
        plan_digest=content_digest(body),
    )


def validate_autonomous_recovery_plan(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and return a canonical metadata-only recovery plan mapping."""

    return AutonomousRecoveryPlan.from_mapping(value).to_dict()
