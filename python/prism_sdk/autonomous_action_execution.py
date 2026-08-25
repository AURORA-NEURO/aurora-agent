"""Digest-bound admission and execution handoff for autonomous action plans.

An :class:`AutonomousActionPlan` is intentionally only a recommendation.  This module adds
the small, explicit control surface an application needs to turn that recommendation into an
execution attempt without teaching every caller the same precedence rules.  The admission record
is value-only and is bound to one exact plan digest.  It never contains task text, credentials,
provider responses, connector payloads, or effect authority.

The execution wrapper replays the provider-free action plan from the transient task and routing
options before invoking the existing ``run_auto`` boundary.  This gives callers a single place to
handle route review, policy blocks, evidence-first work, workflow promotion, provider planning,
and explicit approval gates while preserving the lower-level APIs for durable orchestration.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .authoring import content_digest
from .autonomous_action_plan import (
    AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS,
    AUTONOMOUS_ACTION_PLAN_SCHEMA,
    AutonomousActionPlan,
)
from .autonomous_task_decision import AUTONOMOUS_TASK_DECISION_APPROVALS
from .errors import ArgumentError


AUTONOMOUS_ACTION_EXECUTION_SCHEMA = "bioprism-python-autonomous-action-execution/0.1"
AUTONOMOUS_ACTION_EXECUTION_VERSION = "0.1"
AUTONOMOUS_ACTION_EXECUTION_STATUSES = (
    "admitted",
    "review_required",
    "blocked",
    "route_review_required",
)
AUTONOMOUS_ACTION_EXECUTION_RESULT_STATUSES = (
    "review_required",
    "blocked",
    "route_review_required",
    "completed",
)
AUTONOMOUS_ACTION_EXECUTION_PATHS = (
    "provider",
    "evidence_first",
    "workflow",
    "planning",
    "cross_domain",
    "route_review",
)
MAX_AUTONOMOUS_ACTION_EXECUTION_ITEMS = 32
MAX_AUTONOMOUS_ACTION_EXECUTION_TEXT_BYTES = 512

_AUTHORITY = "admission_only;does_not_authorize_provider_source_tool_effect_or_credential_actions"
_RETENTION = "metadata_only;task_prompt_provider_connector_and_credential_values_not_retained"
_SECRET_MATERIAL = "never_returned"
_APPROVAL_TO_ACTION = {
    "provider_call": "approve_provider_call",
    "evidence_dispatch": "acquire_evidence",
    "plan_acceptance": "review_plan",
    "effect_approval": "review_effect",
    "evaluator_settlement": "settle_evaluator",
}
_ROUTE_OPTION_NAMES = {
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
    "domain_policy_mode",
    "domain_policy_evidence_ready",
    "domain_policy_evaluator_configured",
    "domain_policy_effects_requested",
    "domain_policy_effects_approved",
}
_RESERVED_ROUTE_OPTION_NAMES = {
    "credentials",
    "model_candidates",
    "approvals",
    "reviewed",
    "plan",
    "domain",
}


def _text(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_ACTION_EXECUTION_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds its bound")
    return value


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _items(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_ACTION_EXECUTION_ITEMS) -> tuple[str, ...]:
    if not isinstance(value, (list, tuple)) or len(value) > maximum:
        raise ArgumentError(f"{name} exceeds its bounded item contract")
    result = tuple(_text(f"{name} item", item) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate items")
    return result


def _unique(values: tuple[str, ...] | list[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


def _approval_values(approvals: Mapping[str, bool] | None) -> dict[str, bool]:
    if approvals is None:
        return {}
    if not isinstance(approvals, Mapping):
        raise ArgumentError("action-plan approvals must be a mapping")
    result: dict[str, bool] = {}
    for name, value in approvals.items():
        if name not in AUTONOMOUS_TASK_DECISION_APPROVALS:
            raise ArgumentError(f"action-plan approval {name!r} is unsupported")
        if not isinstance(value, bool):
            raise ArgumentError(f"action-plan approval {name!r} must be boolean")
        result[name] = value
    return result


def _next_actions(
    *,
    plan: AutonomousActionPlan,
    missing: tuple[str, ...],
    review_pending: bool,
) -> tuple[str, tuple[str, ...]]:
    if plan.status == "route_review_required":
        return "review_route", ("review_route", "recompute_route")
    if plan.status == "blocked":
        return "resolve_policy_block", ("resolve_policy_block", "stop_before_dispatch")
    actions: list[str] = []
    if review_pending:
        actions.append("review_task_decision")
    for approval in missing:
        action = _APPROVAL_TO_ACTION[approval]
        if action in AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS:
            actions.append(action)
    if not actions:
        actions.append("review_task_decision")
    return actions[0], _unique(actions)


def _admission_descriptor(admission: "AutonomousActionAdmission") -> dict[str, Any]:
    return {
        "schema": AUTONOMOUS_ACTION_EXECUTION_SCHEMA,
        "version": AUTONOMOUS_ACTION_EXECUTION_VERSION,
        "status": admission.status,
        "plan_digest": admission.plan_digest,
        "task_digest": admission.task_digest,
        "selected_domains": list(admission.selected_domains),
        "execution_path": admission.execution_path,
        "reviewed": admission.reviewed,
        "required_approvals": list(admission.required_approvals),
        "approved_approvals": list(admission.approved_approvals),
        "missing_approvals": list(admission.missing_approvals),
        "review_reasons": list(admission.review_reasons),
        "blocking_reasons": list(admission.blocking_reasons),
        "next_action": admission.next_action,
        "next_actions": list(admission.next_actions),
    }


@dataclass(frozen=True, slots=True)
class AutonomousActionAdmission:
    """One caller review decision bound to an exact autonomous action-plan digest."""

    status: str
    plan_digest: str
    task_digest: str
    selected_domains: tuple[str, ...]
    execution_path: str
    reviewed: bool
    required_approvals: tuple[str, ...]
    approved_approvals: tuple[str, ...]
    missing_approvals: tuple[str, ...]
    review_reasons: tuple[str, ...]
    blocking_reasons: tuple[str, ...]
    next_action: str
    next_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_ACTION_EXECUTION_STATUSES:
            raise ArgumentError("action admission status is unsupported")
        _digest("action admission plan_digest", self.plan_digest)
        _digest("action admission task_digest", self.task_digest)
        if not isinstance(self.selected_domains, (list, tuple)) or len(self.selected_domains) > 12:
            raise ArgumentError("action admission selected_domains exceeds its bound")
        if self.execution_path not in AUTONOMOUS_ACTION_EXECUTION_PATHS:
            raise ArgumentError("action admission execution_path is unsupported")
        if not isinstance(self.reviewed, bool):
            raise ArgumentError("action admission reviewed must be boolean")
        for name, values in (
            ("required_approvals", self.required_approvals),
            ("approved_approvals", self.approved_approvals),
            ("missing_approvals", self.missing_approvals),
            ("review_reasons", self.review_reasons),
            ("blocking_reasons", self.blocking_reasons),
            ("next_actions", self.next_actions),
        ):
            _items(f"action admission {name}", values)
        for value in (*self.required_approvals, *self.approved_approvals, *self.missing_approvals):
            if value not in AUTONOMOUS_TASK_DECISION_APPROVALS:
                raise ArgumentError("action admission contains an unsupported approval")
        if any(value not in self.required_approvals for value in self.approved_approvals):
            raise ArgumentError("action admission approved approval is not required by the plan")
        if any(value not in self.required_approvals for value in self.missing_approvals):
            raise ArgumentError("action admission missing approval is not required by the plan")
        if self.next_action not in AUTONOMOUS_ACTION_PLAN_NEXT_ACTIONS:
            raise ArgumentError("action admission next_action is unsupported")
        if self.next_action not in self.next_actions:
            raise ArgumentError("action admission next_action must be present in next_actions")
        if self.status == "blocked" and not self.blocking_reasons:
            raise ArgumentError("blocked action admission requires blocking reasons")
        if self.status == "review_required" and not (self.review_reasons or self.missing_approvals):
            raise ArgumentError("review-required action admission requires a pending gate")
        object.__setattr__(self, "selected_domains", tuple(self.selected_domains))
        object.__setattr__(self, "required_approvals", _unique(tuple(self.required_approvals)))
        object.__setattr__(self, "approved_approvals", _unique(tuple(self.approved_approvals)))
        object.__setattr__(self, "missing_approvals", _unique(tuple(self.missing_approvals)))
        object.__setattr__(self, "review_reasons", _unique(tuple(self.review_reasons)))
        object.__setattr__(self, "blocking_reasons", _unique(tuple(self.blocking_reasons)))
        object.__setattr__(self, "next_actions", _unique(tuple(self.next_actions)))

    @property
    def admission_digest(self) -> str:
        return content_digest(_admission_descriptor(self))

    def to_dict(self) -> dict[str, Any]:
        return {
            **_admission_descriptor(self),
            "admission_digest": self.admission_digest,
            "authority": _AUTHORITY,
            "retention": _RETENTION,
            "execution": "admission_only;caller_must_bind_provider_and_effect_authority_separately",
            "secret_material": _SECRET_MATERIAL,
        }

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> "AutonomousActionAdmission":
        if not isinstance(value, Mapping) or value.get("schema") != AUTONOMOUS_ACTION_EXECUTION_SCHEMA:
            raise ArgumentError("action admission schema is invalid")
        if value.get("version") != AUTONOMOUS_ACTION_EXECUTION_VERSION:
            raise ArgumentError("action admission version is unsupported")
        if value.get("authority") != _AUTHORITY or value.get("retention") != _RETENTION:
            raise ArgumentError("action admission authority posture is invalid")
        if value.get("execution") != "admission_only;caller_must_bind_provider_and_effect_authority_separately" or value.get("secret_material") != _SECRET_MATERIAL:
            raise ArgumentError("action admission retention posture is invalid")
        admission = cls(
            status=value.get("status"),
            plan_digest=value.get("plan_digest"),
            task_digest=value.get("task_digest"),
            selected_domains=tuple(value.get("selected_domains", ())),
            execution_path=value.get("execution_path"),
            reviewed=value.get("reviewed"),
            required_approvals=tuple(value.get("required_approvals", ())),
            approved_approvals=tuple(value.get("approved_approvals", ())),
            missing_approvals=tuple(value.get("missing_approvals", ())),
            review_reasons=tuple(value.get("review_reasons", ())),
            blocking_reasons=tuple(value.get("blocking_reasons", ())),
            next_action=value.get("next_action"),
            next_actions=tuple(value.get("next_actions", ())),
        )
        if value.get("admission_digest") != admission.admission_digest:
            raise ArgumentError("action admission digest is invalid")
        return admission


def admit_autonomous_action_plan(
    plan: AutonomousActionPlan | Mapping[str, Any],
    *,
    approvals: Mapping[str, bool] | None = None,
    reviewed: bool = False,
) -> AutonomousActionAdmission:
    """Admit one action plan only when its review and every named gate are complete."""

    if isinstance(plan, Mapping):
        plan = AutonomousActionPlan.from_dict(plan)
    if not isinstance(plan, AutonomousActionPlan):
        raise ArgumentError("action admission requires an AutonomousActionPlan")
    if not isinstance(reviewed, bool):
        raise ArgumentError("action-plan reviewed must be boolean")
    values = _approval_values(approvals)
    required = tuple(plan.required_approvals)
    approved = tuple(gate for gate in required if values.get(gate) is True)
    missing = tuple(gate for gate in required if values.get(gate) is not True)
    review_reasons = list(plan.review_reasons)
    if plan.status == "review_required" and not reviewed:
        review_reasons.append("caller_review_required_for_plan_decision")
    if plan.status == "review_required" and reviewed:
        review_reasons = []
    review_reasons.extend(f"approval:{gate}:required" for gate in missing)
    blocking_reasons = list(plan.blocking_reasons)
    if plan.status == "route_review_required":
        status = "route_review_required"
        execution_path = "route_review"
    elif plan.status == "blocked":
        status = "blocked"
        execution_path = "cross_domain" if plan.cross_domain else plan.recommended_path
    elif review_reasons or missing:
        status = "review_required"
        execution_path = "cross_domain" if plan.cross_domain else plan.recommended_path
    else:
        status = "admitted"
        execution_path = "cross_domain" if plan.cross_domain else plan.recommended_path
    next_action, next_actions = _next_actions(
        plan=plan,
        missing=missing,
        review_pending=bool(review_reasons and plan.status == "review_required" and not reviewed),
    )
    return AutonomousActionAdmission(
        status=status,
        plan_digest=plan.plan_digest,
        task_digest=plan.task_digest,
        selected_domains=plan.selected_domains,
        execution_path=execution_path,
        reviewed=reviewed,
        required_approvals=required,
        approved_approvals=approved,
        missing_approvals=missing,
        review_reasons=tuple(review_reasons),
        blocking_reasons=tuple(blocking_reasons),
        next_action=next_action,
        next_actions=next_actions,
    )


@dataclass(frozen=True, slots=True)
class AutonomousActionExecution:
    """The caller-owned result of admitting and optionally dispatching an action plan."""

    status: str
    plan: AutonomousActionPlan
    admission: AutonomousActionAdmission
    result: Any | None = None

    def __post_init__(self) -> None:
        if self.status not in AUTONOMOUS_ACTION_EXECUTION_RESULT_STATUSES:
            raise ArgumentError("action execution status is unsupported")
        if not isinstance(self.plan, AutonomousActionPlan):
            raise ArgumentError("action execution requires an action plan")
        if not isinstance(self.admission, AutonomousActionAdmission):
            raise ArgumentError("action execution requires an action admission")
        if self.admission.plan_digest != self.plan.plan_digest:
            raise ArgumentError("action execution admission does not match the action plan")
        if self.status == "completed" and self.result is None:
            raise ArgumentError("completed action execution requires a result")

    @property
    def execution_status(self) -> str:
        if self.result is None:
            return self.admission.status
        status = getattr(self.result, "execution_status", None)
        return status if isinstance(status, str) and status else self.status

    def to_dict(self) -> dict[str, Any]:
        result = None
        if self.result is not None:
            serializer = getattr(self.result, "to_dict", None)
            if callable(serializer):
                result = serializer()
        return {
            "schema": AUTONOMOUS_ACTION_EXECUTION_SCHEMA,
            "version": AUTONOMOUS_ACTION_EXECUTION_VERSION,
            "status": self.status,
            "execution_status": self.execution_status,
            "plan": self.plan.to_dict(),
            "admission": self.admission.to_dict(),
            "result": result,
            "authorization": "caller_owned_execution_result;provider_and_effect_authority_remain_explicit",
            "retention": "plan_and_admission_metadata_only;provider_result_is_caller_owned",
            "secret_material": _SECRET_MATERIAL,
        }


__all__ = [
    "AUTONOMOUS_ACTION_EXECUTION_SCHEMA",
    "AUTONOMOUS_ACTION_EXECUTION_VERSION",
    "AUTONOMOUS_ACTION_EXECUTION_STATUSES",
    "AUTONOMOUS_ACTION_EXECUTION_RESULT_STATUSES",
    "AUTONOMOUS_ACTION_EXECUTION_PATHS",
    "MAX_AUTONOMOUS_ACTION_EXECUTION_ITEMS",
    "AutonomousActionAdmission",
    "AutonomousActionExecution",
    "admit_autonomous_action_plan",
]
