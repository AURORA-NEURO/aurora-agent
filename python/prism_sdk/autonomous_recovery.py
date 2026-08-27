"""Deterministic, metadata-only recovery planning for autonomous runs.

The provider runtime and execution facades deliberately return held, failed, and reconciliation
states instead of hiding them behind a generic exception.  This module turns that small
value-only observation into a bounded operator/queue handoff.  It never retries, resolves a key,
calls a provider, executes a tool, settles an evaluator, or reconciles an external effect; those
authorities stay with the embedding deployment.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .authoring import canonical_json, content_digest
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
AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA = "bioprism-python-autonomous-recovery-handoff/0.1"
AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA = "bioprism-python-autonomous-recovery-handoff-snapshot/0.1"
AUTONOMOUS_RECOVERY_HANDOFF_RETENTION = (
    "metadata_only_recovery_handoff;run_identity_is_digest_bound;tasks_prompts_credentials_provider_values_and_effects_not_retained"
)
AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY = (
    "review_queue_only;review_does_not_execute_retry_reconcile_provider_tool_or_effect"
)
AUTONOMOUS_RECOVERY_HANDOFF_STATUSES = (
    "queued",
    "retry_approved",
    "reconciliation_required",
    "escalated",
    "closed",
)
AUTONOMOUS_RECOVERY_REVIEW_DECISIONS = (
    "approve_retry",
    "approve_reconciliation",
    "escalate",
    "close",
)
MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS = 4096
MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES = 10_000_000

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


def _count(name: str, value: Any, default: int, maximum: int = 64) -> int:
    result = default if value is None else value
    if isinstance(result, bool) or not isinstance(result, int) or not 0 <= result <= maximum:
        _fail(f"{name} must be an integer within [0, {maximum}]")
    return result


def _digest(name: str, value: Any, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _exact_keys(name: str, value: Mapping[str, Any], expected: Sequence[str]) -> None:
    if set(value) != set(expected):
        _fail(f"{name} contains unsupported or missing fields")


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


def _handoff_identity_digest(run_id_digest: str, attempt: int, plan_digest: str) -> str:
    return content_digest(
        {
            "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
            "run_id_digest": run_id_digest,
            "attempt": attempt,
            "plan_digest": plan_digest,
        }
    )


def _transition_digest(
    handoff_id: str,
    previous_handoff_digest: str | None,
    decision: str | None,
    reviewer_digest: str | None,
    status: str,
    revision: int,
) -> str:
    return content_digest(
        {
            "handoff_id": handoff_id,
            "previous_handoff_digest": previous_handoff_digest,
            "decision": decision,
            "reviewer_digest": reviewer_digest,
            "status": status,
            "revision": revision,
        }
    )


@dataclass(frozen=True, slots=True)
class AutonomousRecoveryHandoff:
    """A digest-bound recovery queue row with no transient run values."""

    handoff_id: str
    run_id_digest: str
    attempt: int
    plan_digest: str
    domain: str
    capability: str
    plan_status: str
    recommended_action: str
    actions: tuple[str, ...]
    retry_count: int
    max_retries: int
    status: str
    selected_action: str | None
    revision: int
    last_decision: str | None
    reviewer_digest: str | None
    transition_digest: str
    handoff_digest: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
            "handoff_id": self.handoff_id,
            "run_id_digest": self.run_id_digest,
            "attempt": self.attempt,
            "plan_digest": self.plan_digest,
            "domain": self.domain,
            "capability": self.capability,
            "plan_status": self.plan_status,
            "recommended_action": self.recommended_action,
            "actions": list(self.actions),
            "retry_count": self.retry_count,
            "max_retries": self.max_retries,
            "status": self.status,
            "selected_action": self.selected_action,
            "revision": self.revision,
            "last_decision": self.last_decision,
            "reviewer_digest": self.reviewer_digest,
            "transition_digest": self.transition_digest,
            "authority": AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY,
            "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
            "secret_material": "never_returned",
            "handoff_digest": self.handoff_digest,
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousRecoveryHandoff":
        return _parse_handoff(value)


def _handoff_body(value: Mapping[str, Any]) -> dict[str, Any]:
    return {key: value[key] for key in value if key != "handoff_digest"}


def _parse_handoff(value: Mapping[str, Any]) -> AutonomousRecoveryHandoff:
    if not isinstance(value, Mapping):
        _fail("handoff must be a mapping")
    expected = {
        "schema",
        "handoff_id",
        "run_id_digest",
        "attempt",
        "plan_digest",
        "domain",
        "capability",
        "plan_status",
        "recommended_action",
        "actions",
        "retry_count",
        "max_retries",
        "status",
        "selected_action",
        "revision",
        "last_decision",
        "reviewer_digest",
        "transition_digest",
        "authority",
        "retention",
        "secret_material",
        "handoff_digest",
    }
    _exact_keys("handoff", value, expected)
    _secret_free(value)
    if (
        value["schema"] != AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA
        or value["authority"] != AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY
        or value["retention"] != AUTONOMOUS_RECOVERY_HANDOFF_RETENTION
        or value["secret_material"] != "never_returned"
    ):
        _fail("handoff markers are invalid")
    run_id_digest = _digest("handoff run_id_digest", value["run_id_digest"]) or ""
    attempt = _count("handoff attempt", value["attempt"], 0)
    plan_digest = _digest("handoff plan_digest", value["plan_digest"]) or ""
    handoff_id = _digest("handoff handoff_id", value["handoff_id"]) or ""
    if handoff_id != _handoff_identity_digest(run_id_digest, attempt, plan_digest):
        _fail("handoff identity does not match its digests")
    domain = value["domain"]
    if domain not in AUTONOMOUS_DOMAIN_NAMES:
        _fail("handoff domain is unsupported")
    capability = _identifier("handoff capability", value["capability"], MAX_AUTONOMOUS_RECOVERY_CAPABILITY_BYTES)
    plan_status = _identifier("handoff plan_status", value["plan_status"], 64)
    if plan_status not in {"completed", "retryable", "held", "reconciliation_required", "blocked"}:
        _fail("handoff plan_status is invalid")
    recommended_action = _identifier("handoff recommended_action", value["recommended_action"])
    if recommended_action not in AUTONOMOUS_RECOVERY_ACTIONS:
        _fail("handoff recommended_action is invalid")
    actions = _actions("handoff actions", value["actions"])
    if actions[0] != recommended_action:
        _fail("handoff recommended_action must be the first action")
    retry_count = _count("handoff retry_count", value["retry_count"], 0)
    max_retries = _count("handoff max_retries", value["max_retries"], 2)
    if retry_count > max_retries:
        _fail("handoff retry_count exceeds max_retries")
    status = _identifier("handoff status", value["status"], 64)
    if status not in AUTONOMOUS_RECOVERY_HANDOFF_STATUSES:
        _fail("handoff status is invalid")
    selected_action = value["selected_action"]
    if selected_action is not None:
        selected_action = _identifier("handoff selected_action", selected_action)
        if selected_action not in AUTONOMOUS_RECOVERY_ACTIONS or selected_action not in actions:
            _fail("handoff selected_action is not available in actions")
    revision = _count("handoff revision", value["revision"], 1, maximum=2_147_483_647)
    if revision < 1:
        _fail("handoff revision must be positive")
    last_decision = value["last_decision"]
    if last_decision is not None:
        last_decision = _identifier("handoff last_decision", last_decision)
        if last_decision not in AUTONOMOUS_RECOVERY_REVIEW_DECISIONS:
            _fail("handoff last_decision is invalid")
    reviewer_digest = _digest("handoff reviewer_digest", value["reviewer_digest"], allow_none=True)
    if status == "queued" and (last_decision is not None or reviewer_digest is not None or selected_action is not None):
        _fail("queued handoff contains a review decision")
    if status == "retry_approved" and (
        last_decision != "approve_retry"
        or reviewer_digest is None
        or selected_action not in {"retry_provider", "retry_after_review"}
    ):
        _fail("retry-approved handoff is inconsistent")
    if status == "reconciliation_required" and (
        last_decision != "approve_reconciliation"
        or reviewer_digest is None
        or selected_action != "reconcile_external_effect"
    ):
        _fail("reconciliation handoff is inconsistent")
    if status == "escalated" and (
        last_decision != "escalate" or reviewer_digest is None or selected_action != "stop_and_escalate"
    ):
        _fail("escalated handoff is inconsistent")
    if status == "closed" and last_decision is not None and (last_decision != "close" or reviewer_digest is None):
        _fail("closed handoff decision is inconsistent")
    transition_digest = _digest("handoff transition_digest", value["transition_digest"]) or ""
    handoff_digest = _digest("handoff handoff_digest", value["handoff_digest"]) or ""
    body = {
        "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
        "handoff_id": handoff_id,
        "run_id_digest": run_id_digest,
        "attempt": attempt,
        "plan_digest": plan_digest,
        "domain": domain,
        "capability": capability,
        "plan_status": plan_status,
        "recommended_action": recommended_action,
        "actions": list(actions),
        "retry_count": retry_count,
        "max_retries": max_retries,
        "status": status,
        "selected_action": selected_action,
        "revision": revision,
        "last_decision": last_decision,
        "reviewer_digest": reviewer_digest,
        "transition_digest": transition_digest,
        "authority": AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY,
        "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
        "secret_material": "never_returned",
    }
    if handoff_digest != content_digest(_handoff_body({**body, "handoff_digest": handoff_digest})):
        _fail("handoff digest does not match metadata")
    return AutonomousRecoveryHandoff(
        handoff_id=handoff_id,
        run_id_digest=run_id_digest,
        attempt=attempt,
        plan_digest=plan_digest,
        domain=domain,
        capability=capability,
        plan_status=plan_status,
        recommended_action=recommended_action,
        actions=actions,
        retry_count=retry_count,
        max_retries=max_retries,
        status=status,
        selected_action=selected_action,
        revision=revision,
        last_decision=last_decision,
        reviewer_digest=reviewer_digest,
        transition_digest=transition_digest,
        handoff_digest=handoff_digest,
    )


def _make_handoff(plan: AutonomousRecoveryPlan, run_id_digest: str, attempt: int) -> AutonomousRecoveryHandoff:
    handoff_id = _handoff_identity_digest(run_id_digest, attempt, plan.plan_digest)
    status = "closed" if plan.status == "completed" else "queued"
    selected_action = "complete" if plan.status == "completed" else None
    transition = _transition_digest(handoff_id, None, None, None, status, 1)
    body = {
        "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
        "handoff_id": handoff_id,
        "run_id_digest": run_id_digest,
        "attempt": attempt,
        "plan_digest": plan.plan_digest,
        "domain": plan.domain,
        "capability": plan.capability,
        "plan_status": plan.status,
        "recommended_action": plan.next_action,
        "actions": list(plan.actions),
        "retry_count": plan.retry_count,
        "max_retries": plan.max_retries,
        "status": status,
        "selected_action": selected_action,
        "revision": 1,
        "last_decision": None,
        "reviewer_digest": None,
        "transition_digest": transition,
        "authority": AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY,
        "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
        "secret_material": "never_returned",
    }
    return _parse_handoff({**body, "handoff_digest": content_digest(body)})


def _validate_handoff_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping):
        _fail("handoff snapshot must be a mapping")
    expected = {
        "schema",
        "entries",
        "generation",
        "previous_snapshot_digest",
        "retention",
        "secret_material",
        "snapshot_digest",
    }
    _exact_keys("handoff snapshot", value, expected)
    _secret_free(value)
    if (
        value["schema"] != AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA
        or value["retention"] != AUTONOMOUS_RECOVERY_HANDOFF_RETENTION
        or value["secret_material"] != "never_returned"
    ):
        _fail("handoff snapshot markers are invalid")
    entries_raw = value["entries"]
    if not isinstance(entries_raw, Sequence) or isinstance(entries_raw, (str, bytes, bytearray)) or len(entries_raw) > MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS:
        _fail("handoff snapshot entries are outside their bound")
    entries = tuple(_parse_handoff(entry) for entry in entries_raw)
    if len({entry.handoff_id for entry in entries}) != len(entries) or tuple(sorted(entries, key=lambda entry: entry.handoff_id)) != entries:
        _fail("handoff snapshot entries are not unique and ordered")
    generation = _count("handoff snapshot generation", value["generation"], 1, maximum=2_147_483_647)
    if generation < 1:
        _fail("handoff snapshot generation must be positive")
    previous = _digest("handoff snapshot previous_snapshot_digest", value["previous_snapshot_digest"], allow_none=True)
    if (generation == 1) != (previous is None):
        _fail("handoff snapshot generation and predecessor are inconsistent")
    snapshot_digest = _digest("handoff snapshot snapshot_digest", value["snapshot_digest"]) or ""
    body = {
        "schema": AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA,
        "entries": [entry.to_dict() for entry in entries],
        "generation": generation,
        "previous_snapshot_digest": previous,
        "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
        "secret_material": "never_returned",
    }
    if snapshot_digest != content_digest(body):
        _fail("handoff snapshot digest does not match metadata")
    normalized = {**body, "snapshot_digest": snapshot_digest}
    if len(canonical_json(normalized).encode("utf-8")) > MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES:
        _fail("handoff snapshot exceeds its byte bound")
    return normalized


class AutonomousRecoveryHandoffLedger:
    """Bounded, idempotent recovery handoffs with explicit review transitions."""

    def __init__(self) -> None:
        self._entries: dict[str, AutonomousRecoveryHandoff] = {}
        self._generation = 0
        self._previous_snapshot_digest: str | None = None
        self._cached_snapshot: dict[str, Any] | None = None
        self._cached_signature: tuple[str, ...] | None = None

    def get(self, handoff_id: str) -> AutonomousRecoveryHandoff | None:
        identity = _digest("handoff id", handoff_id) or ""
        entry = self._entries.get(identity)
        return None if entry is None else AutonomousRecoveryHandoff.from_mapping(entry.to_dict())

    def entries(
        self,
        *,
        status: str | None = None,
        domain: str | None = None,
        limit: int = MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS,
    ) -> tuple[AutonomousRecoveryHandoff, ...]:
        limit = _count("handoff list limit", limit, MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS, maximum=MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS)
        if limit < 1:
            _fail("handoff list limit must be positive")
        if status is not None and status not in AUTONOMOUS_RECOVERY_HANDOFF_STATUSES:
            _fail("handoff list status is invalid")
        if domain is not None and domain not in AUTONOMOUS_DOMAIN_NAMES:
            _fail("handoff list domain is invalid")
        selected = [entry for entry in self._entries.values() if (status is None or entry.status == status) and (domain is None or entry.domain == domain)]
        return tuple(AutonomousRecoveryHandoff.from_mapping(entry.to_dict()) for entry in sorted(selected, key=lambda item: item.handoff_id)[:limit])

    def submit(
        self,
        plan: Mapping[str, Any] | AutonomousRecoveryPlan,
        *,
        run_id_digest: str,
        attempt: int = 0,
    ) -> dict[str, Any]:
        normalized_plan = plan if isinstance(plan, AutonomousRecoveryPlan) else AutonomousRecoveryPlan.from_mapping(plan)
        run_id = _digest("handoff submission run_id_digest", run_id_digest) or ""
        attempt = _count("handoff submission attempt", attempt, 0)
        handoff = _make_handoff(normalized_plan, run_id, attempt)
        existing = self._entries.get(handoff.handoff_id)
        if existing is not None:
            if existing.handoff_digest != handoff.handoff_digest:
                _fail("handoff identity conflicts with retained metadata")
            result_status = "duplicate"
            returned = existing
        else:
            if len(self._entries) >= MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS:
                _fail("handoff ledger is at capacity")
            self._entries[handoff.handoff_id] = handoff
            self._invalidate()
            result_status = "accepted"
            returned = handoff
        return {
            "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
            "status": result_status,
            "handoff": returned.to_dict(),
            "retained_count": len(self._entries),
            "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
            "secret_material": "never_returned",
        }

    def review(
        self,
        handoff_id: str,
        *,
        decision: str,
        expected_revision: int,
        reviewer_digest: str,
    ) -> dict[str, Any]:
        identity = _digest("handoff review handoff_id", handoff_id) or ""
        if decision not in AUTONOMOUS_RECOVERY_REVIEW_DECISIONS:
            _fail("handoff review decision is invalid")
        expected_revision = _count("handoff review expected_revision", expected_revision, 1, maximum=2_147_483_647)
        reviewer = _digest("handoff review reviewer_digest", reviewer_digest) or ""
        current = self._entries.get(identity)
        if current is None:
            _fail("handoff is not retained")
        if current.revision != expected_revision:
            _fail("handoff review revision is stale")
        if current.status != "queued":
            _fail("handoff is already reviewed")
        if decision == "approve_retry" and (
            current.recommended_action == "collect_credential"
            or not ({"retry_provider", "retry_after_review"} & set(current.actions))
        ):
            _fail("handoff does not authorize a retry review")
        if decision == "approve_reconciliation" and "reconcile_external_effect" not in current.actions:
            _fail("handoff does not require reconciliation")
        status = {
            "approve_retry": "retry_approved",
            "approve_reconciliation": "reconciliation_required",
            "escalate": "escalated",
            "close": "closed",
        }[decision]
        selected_action = (
            "retry_provider"
            if decision == "approve_retry" and "retry_provider" in current.actions
            else "retry_after_review"
            if decision == "approve_retry"
            else "reconcile_external_effect"
            if decision == "approve_reconciliation"
            else "stop_and_escalate"
            if decision == "escalate"
            else None
        )
        body = {
            **current.to_dict(),
            "status": status,
            "selected_action": selected_action,
            "revision": current.revision + 1,
            "last_decision": decision,
            "reviewer_digest": reviewer,
            "transition_digest": _transition_digest(identity, current.handoff_digest, decision, reviewer, status, current.revision + 1),
        }
        next_entry = _parse_handoff({**body, "handoff_digest": content_digest(_handoff_body(body))})
        self._entries[identity] = next_entry
        self._invalidate()
        return {
            "schema": AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA,
            "status": "reviewed",
            "decision": decision,
            "handoff": next_entry.to_dict(),
            "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
            "secret_material": "never_returned",
        }

    def snapshot(self) -> dict[str, Any]:
        entries = self.entries()
        signature = tuple(entry.handoff_digest for entry in entries)
        if self._cached_snapshot is not None and self._cached_signature == signature:
            return copy.deepcopy(self._cached_snapshot)
        body = {
            "schema": AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA,
            "entries": [entry.to_dict() for entry in entries],
            "generation": self._generation + 1,
            "previous_snapshot_digest": self._previous_snapshot_digest,
            "retention": AUTONOMOUS_RECOVERY_HANDOFF_RETENTION,
            "secret_material": "never_returned",
        }
        snapshot = _validate_handoff_snapshot({**body, "snapshot_digest": content_digest(body)})
        self._generation = snapshot["generation"]
        self._previous_snapshot_digest = snapshot["snapshot_digest"]
        self._cached_snapshot = copy.deepcopy(snapshot)
        self._cached_signature = signature
        return copy.deepcopy(snapshot)

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _validate_handoff_snapshot(snapshot)
        entries = tuple(_parse_handoff(entry) for entry in normalized["entries"])
        self._entries = {entry.handoff_id: entry for entry in entries}
        self._generation = normalized["generation"]
        self._previous_snapshot_digest = normalized["snapshot_digest"]
        self._cached_snapshot = copy.deepcopy(normalized)
        self._cached_signature = tuple(entry.handoff_digest for entry in entries)

    def _invalidate(self) -> None:
        self._cached_snapshot = None
        self._cached_signature = None


class JsonAutonomousRecoveryHandoffPersistence:
    """Canonical JSON persistence for the recovery handoff ledger."""

    def __init__(self, store: Any, *, max_bytes: int = MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES) -> None:
        if not all(callable(getattr(store, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous recovery handoff JSON persistence requires a text store")
        self.store = store
        self.max_bytes = _count("handoff persistence max_bytes", max_bytes, MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES, maximum=MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES)

    def read(self) -> dict[str, Any] | None:
        raw_value = self.store.read()
        if raw_value is None:
            return None
        if not isinstance(raw_value, str) or len(raw_value.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("autonomous recovery handoff JSON exceeds its byte bound")
        import json

        try:
            raw = json.loads(raw_value)
        except (TypeError, json.JSONDecodeError) as error:
            raise ArgumentError("autonomous recovery handoff JSON is invalid") from error
        if canonical_json(raw) != raw_value:
            raise ArgumentError("autonomous recovery handoff JSON is not canonical")
        return _validate_handoff_snapshot(raw)

    def write(self, snapshot: Mapping[str, Any]) -> None:
        normalized = _validate_handoff_snapshot(snapshot)
        encoded = canonical_json(normalized)
        if len(encoded.encode("utf-8")) > self.max_bytes:
            raise ArgumentError("autonomous recovery handoff JSON exceeds its byte bound")
        self.store.write(encoded)


class TransactionalJsonAutonomousRecoveryHandoffPersistence(JsonAutonomousRecoveryHandoffPersistence):
    def __init__(self, store: Any, **kwargs: Any) -> None:
        super().__init__(store, **kwargs)
        if not callable(getattr(store, "write_if_unchanged", None)):
            raise ArgumentError("transactional autonomous recovery handoff persistence requires write_if_unchanged")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, snapshot: Mapping[str, Any]) -> bool:
        _digest("handoff expected_snapshot_digest", expected_snapshot_digest, allow_none=True)
        normalized = _validate_handoff_snapshot(snapshot)
        return bool(self.store.write_if_unchanged(expected_snapshot_digest, canonical_json(normalized)))


class AutonomousRecoveryHandoffPersistenceCoordinator:
    """Restore/flush ordering with an optional compare-and-swap fence."""

    def __init__(self, ledger: AutonomousRecoveryHandoffLedger, persistence: Any) -> None:
        if not isinstance(ledger, AutonomousRecoveryHandoffLedger):
            raise ArgumentError("autonomous recovery handoff coordinator requires a handoff ledger")
        if not all(callable(getattr(persistence, name, None)) for name in ("read", "write")):
            raise ArgumentError("autonomous recovery handoff persistence is malformed")
        self.ledger = ledger
        self.persistence = persistence
        self._expected_snapshot_digest: str | None = None

    def restore(self) -> dict[str, Any] | None:
        snapshot = self.persistence.read()
        if snapshot is None:
            self._expected_snapshot_digest = None
            return None
        self.ledger.restore(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return copy.deepcopy(snapshot)

    def flush(self) -> dict[str, Any]:
        snapshot = self.ledger.snapshot()
        writer = getattr(self.persistence, "write_if_unchanged", None)
        if callable(writer):
            if not writer(self._expected_snapshot_digest, snapshot):
                raise ArgumentError("autonomous recovery handoff persistence compare-and-swap conflict")
        else:
            self.persistence.write(snapshot)
        self._expected_snapshot_digest = snapshot["snapshot_digest"]
        return copy.deepcopy(snapshot)


def validate_autonomous_recovery_handoff(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and return a canonical metadata-only recovery handoff."""

    return _parse_handoff(value).to_dict()


def validate_autonomous_recovery_handoff_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate and return a canonical recovery handoff snapshot."""

    return _validate_handoff_snapshot(value)


__all__ = [
    "AUTONOMOUS_RECOVERY_ACTIONS",
    "AUTONOMOUS_RECOVERY_AUTHORITY",
    "AUTONOMOUS_RECOVERY_HANDOFF_AUTHORITY",
    "AUTONOMOUS_RECOVERY_HANDOFF_RETENTION",
    "AUTONOMOUS_RECOVERY_HANDOFF_SCHEMA",
    "AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_SCHEMA",
    "AUTONOMOUS_RECOVERY_HANDOFF_STATUSES",
    "AUTONOMOUS_RECOVERY_PLAN_SCHEMA",
    "AUTONOMOUS_RECOVERY_RETENTION",
    "AUTONOMOUS_RECOVERY_REVIEW_DECISIONS",
    "MAX_AUTONOMOUS_RECOVERY_ACTIONS",
    "MAX_AUTONOMOUS_RECOVERY_CAPABILITY_BYTES",
    "MAX_AUTONOMOUS_RECOVERY_HANDOFF_ITEMS",
    "MAX_AUTONOMOUS_RECOVERY_HANDOFF_SNAPSHOT_BYTES",
    "MAX_AUTONOMOUS_RECOVERY_REASON_CODES",
    "AutonomousRecoveryHandoff",
    "AutonomousRecoveryHandoffLedger",
    "AutonomousRecoveryHandoffPersistenceCoordinator",
    "AutonomousRecoveryPlan",
    "AutonomousRecoveryObservation",
    "JsonAutonomousRecoveryHandoffPersistence",
    "TransactionalJsonAutonomousRecoveryHandoffPersistence",
    "plan_autonomous_recovery",
    "validate_autonomous_recovery_handoff",
    "validate_autonomous_recovery_handoff_snapshot",
    "validate_autonomous_recovery_plan",
]
