"""Provider-free intent-to-action posture for the autonomous brain.

Task intent makes the first interpretation inspectable.  This module takes that bounded
interpretation one step further: it derives the next safe execution posture from the reviewed
intent, domain policy, and task lens.  The result is guidance and admission metadata, never an
authorization token.  Provider, source, tool, credential, effect, and evaluator boundaries stay
owned by their existing callers.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_domain_policy import AutonomousDomainPolicy
from .autonomous_task_intent import (
    AUTONOMOUS_TASK_INTENT_DOMAINS,
    AUTONOMOUS_TASK_INTENT_EFFECTS,
    AutonomousTaskIntent,
)
from .autonomous_task_lens import AutonomousDomainTaskLens
from .errors import ArgumentError


AUTONOMOUS_TASK_DECISION_SCHEMA = "bioprism-autonomous-task-decision/0.1"
AUTONOMOUS_TASK_DECISION_VERSION = "0.1"
AUTONOMOUS_TASK_DECISION_POSTURES = ("admitted", "review_required", "blocked")
AUTONOMOUS_TASK_DECISION_PATHS = (
    "provider",
    "evidence_first",
    "workflow",
    "planning",
    "cross_domain",
)
AUTONOMOUS_TASK_DECISION_APPROVALS = (
    "provider_call",
    "evidence_dispatch",
    "plan_acceptance",
    "effect_approval",
    "evaluator_settlement",
)
AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES = ("optional", "required_before_provider")
MAX_AUTONOMOUS_TASK_DECISION_ITEMS = 12
MAX_AUTONOMOUS_TASK_DECISION_TEXT_BYTES = 512


def _bounded_text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_AUTONOMOUS_TASK_DECISION_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds its bound")
    return value


def _bounded_items(name: str, values: Sequence[str]) -> tuple[str, ...]:
    if not isinstance(values, Sequence) or isinstance(values, (str, bytes)) or len(values) > MAX_AUTONOMOUS_TASK_DECISION_ITEMS:
        raise ArgumentError(f"{name} exceeds its item bound")
    result = tuple(_bounded_text(f"{name} item", value) for value in values)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} contains duplicate items")
    return result


def _digest(name: str, value: str) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _unique(values: Sequence[str]) -> tuple[str, ...]:
    return tuple(dict.fromkeys(values))


@dataclass(frozen=True, slots=True)
class AutonomousTaskDecision:
    """A digest-bound recommendation for the next autonomous execution boundary."""

    domain: str
    workflow_id: str
    task_digest: str
    intent_id: str
    intent_digest: str
    lens_digest: str
    policy_digest: str
    decision_id: str
    posture: str
    recommended_path: str
    requested_effect: str
    evidence_posture: str
    required_model_capabilities: tuple[str, ...]
    preferred_model_capabilities: tuple[str, ...]
    approval_requirements: tuple[str, ...]
    review_reasons: tuple[str, ...]
    blocking_reasons: tuple[str, ...]
    next_actions: tuple[str, ...]
    decision_version: str = AUTONOMOUS_TASK_DECISION_VERSION

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_TASK_INTENT_DOMAINS:
            raise ArgumentError(f"unsupported task-decision domain: {self.domain}")
        for name, value in (
            ("domain", self.domain),
            ("workflow_id", self.workflow_id),
            ("intent_id", self.intent_id),
            ("decision_id", self.decision_id),
        ):
            _bounded_text(f"task decision {name}", value)
        if self.decision_version != AUTONOMOUS_TASK_DECISION_VERSION:
            raise ArgumentError("unsupported task-decision version")
        for name, value in (
            ("task_digest", self.task_digest),
            ("intent_digest", self.intent_digest),
            ("lens_digest", self.lens_digest),
            ("policy_digest", self.policy_digest),
        ):
            _digest(f"task decision {name}", value)
        if self.posture not in AUTONOMOUS_TASK_DECISION_POSTURES:
            raise ArgumentError("task decision posture is unsupported")
        if self.recommended_path not in AUTONOMOUS_TASK_DECISION_PATHS:
            raise ArgumentError("task decision recommended_path is unsupported")
        if self.requested_effect not in AUTONOMOUS_TASK_INTENT_EFFECTS:
            raise ArgumentError("task decision requested_effect is unsupported")
        if self.evidence_posture not in AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES:
            raise ArgumentError("task decision evidence_posture is unsupported")
        for name, value in (
            ("required_model_capabilities", self.required_model_capabilities),
            ("preferred_model_capabilities", self.preferred_model_capabilities),
            ("approval_requirements", self.approval_requirements),
            ("review_reasons", self.review_reasons),
            ("blocking_reasons", self.blocking_reasons),
            ("next_actions", self.next_actions),
        ):
            _bounded_items(f"task decision {name}", value)
        if any(value not in AUTONOMOUS_TASK_DECISION_APPROVALS for value in self.approval_requirements):
            raise ArgumentError("task decision approval_requirements contains an unsupported gate")

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_TASK_DECISION_SCHEMA,
            "decision_version": self.decision_version,
            "domain": self.domain,
            "workflow_id": self.workflow_id,
            "task_digest": self.task_digest,
            "intent_id": self.intent_id,
            "intent_digest": self.intent_digest,
            "lens_digest": self.lens_digest,
            "policy_digest": self.policy_digest,
            "decision_id": self.decision_id,
            "posture": self.posture,
            "recommended_path": self.recommended_path,
            "requested_effect": self.requested_effect,
            "evidence_posture": self.evidence_posture,
            "required_model_capabilities": list(self.required_model_capabilities),
            "preferred_model_capabilities": list(self.preferred_model_capabilities),
            "approval_requirements": list(self.approval_requirements),
            "review_reasons": list(self.review_reasons),
            "blocking_reasons": list(self.blocking_reasons),
            "next_actions": list(self.next_actions),
        }

    @property
    def decision_digest(self) -> str:
        return content_digest(self._descriptor())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._descriptor(),
            "decision_digest": self.decision_digest,
            "authorization": "guidance_only;provider_source_tool_and_effect_authority_remain_separate",
            "retention": "value_only_decision_metadata;task_text_not_retained",
            "secret_material": "never_returned",
        }

    def prompt_contract(self, *, compact: bool = False) -> dict[str, Any]:
        result: dict[str, Any] = {
            "schema": AUTONOMOUS_TASK_DECISION_SCHEMA,
            "decision_id": self.decision_id,
            "decision_digest": self.decision_digest,
            "intent_digest": self.intent_digest,
            "posture": self.posture,
            "recommended_path": self.recommended_path,
            "requested_effect": self.requested_effect,
            "evidence_posture": self.evidence_posture,
            "approval_requirements": list(self.approval_requirements),
            "review_reasons": list(self.review_reasons),
            "blocking_reasons": list(self.blocking_reasons),
            "authority": "guidance_only;does_not_authorize_provider_source_tool_or_effect_actions",
        }
        if not compact:
            result.update(
                {
                    "required_model_capabilities": list(self.required_model_capabilities),
                    "preferred_model_capabilities": list(self.preferred_model_capabilities),
                    "next_actions": list(self.next_actions),
                }
            )
        result["secret_material"] = "never_returned"
        return result


def infer_autonomous_task_decision(
    *,
    intent: AutonomousTaskIntent,
    lens: AutonomousDomainTaskLens,
    policy: AutonomousDomainPolicy,
    required_model_capabilities: Sequence[str] = (),
) -> AutonomousTaskDecision:
    """Derive an auditable next-step posture without invoking a provider or tool."""

    if not isinstance(intent, AutonomousTaskIntent) or not isinstance(lens, AutonomousDomainTaskLens) or not isinstance(policy, AutonomousDomainPolicy):
        raise ArgumentError("task decision requires a valid intent, lens, and policy")
    if intent.domain != lens.domain or intent.domain != policy.domain:
        raise ArgumentError("task decision intent, lens, and policy domains must agree")
    required = _bounded_items("task decision required_model_capabilities", required_model_capabilities)
    if not required:
        raise ArgumentError("task decision requires at least one model capability")

    action = intent.action_mode
    if intent.domain == "cross_domain" or action in {"coordinate", "synthesize"}:
        path = "cross_domain"
    elif action in {"create", "modify"}:
        path = "workflow"
    elif action in {"plan"}:
        path = "planning"
    elif policy.evidence_mode == "required_before_provider" or action in {"observe", "investigate", "analyze", "compare", "evaluate"}:
        path = "evidence_first"
    else:
        path = "provider"

    preferred = list(lens.model_capability_hints)
    if path == "workflow":
        preferred.append("structured_output")
    if path == "cross_domain":
        preferred.extend(("coordination", "structured_output"))
    if action in {"analyze", "compare", "evaluate", "synthesize"}:
        preferred.append("reasoning")
    preferred = list(_unique(preferred))

    approvals = ["provider_call"]
    review: list[str] = []
    blocked: list[str] = []
    if policy.evidence_mode == "required_before_provider":
        approvals.append("evidence_dispatch")
        review.append("evidence_required_before_provider")
    if path in {"workflow", "planning", "cross_domain"} and policy.plan_acceptance_required:
        approvals.append("plan_acceptance")
        review.append("plan_acceptance_required")
    if intent.requested_effect == "external_effect":
        if policy.effect_mode == "forbidden":
            blocked.append("requested_effect_forbidden_by_domain_policy")
        else:
            approvals.append("effect_approval")
            review.append("external_effect_requires_explicit_approval")
    elif intent.requested_effect == "local_change" and policy.effect_mode == "approval_gated":
        approvals.append("effect_approval")
        review.append("local_change_requires_explicit_approval")
    if policy.evaluator_required:
        approvals.append("evaluator_settlement")
    for flag in intent.ambiguity_flags:
        review.append(f"intent:{flag}")
    if action in {"coordinate", "synthesize"} or intent.domain == "cross_domain":
        review.append("specialist_boundaries_require_review")
    if intent.risk_signals:
        review.append("domain_risk_signals_require_review")

    approvals = list(_unique(approvals))
    review = list(_unique(review))
    blocked = list(_unique(blocked))
    posture = "blocked" if blocked else "review_required" if review else "admitted"
    if blocked:
        next_actions = ["stop_before_provider_dispatch", "resolve_domain_policy_conflict"]
    elif review:
        next_actions = ["review_task_intent_and_decision", "satisfy_required_approval_gates"]
        if "evidence_dispatch" in approvals:
            next_actions.append("acquire_and_review_required_evidence")
    else:
        next_actions = ["select_model", "request_provider_call_approval"]
    if "evaluator_settlement" in approvals:
        next_actions.append("settle_explicit_evaluator_feedback_after_run")
    next_actions = list(_unique(next_actions))
    return AutonomousTaskDecision(
        domain=intent.domain,
        workflow_id=intent.workflow_id,
        task_digest=intent.task_digest,
        intent_id=intent.intent_id,
        intent_digest=intent.intent_digest,
        lens_digest=lens.lens_digest,
        policy_digest=policy.policy_digest,
        decision_id=f"{intent.intent_id}:{posture}:{path}",
        posture=posture,
        recommended_path=path,
        requested_effect=intent.requested_effect,
        evidence_posture=policy.evidence_mode,
        required_model_capabilities=required,
        preferred_model_capabilities=tuple(preferred),
        approval_requirements=tuple(approvals),
        review_reasons=tuple(review),
        blocking_reasons=tuple(blocked),
        next_actions=tuple(next_actions),
    )


def validate_autonomous_task_decision(
    value: AutonomousTaskDecision | Mapping[str, Any],
    *,
    intent: AutonomousTaskIntent | None = None,
    lens: AutonomousDomainTaskLens | None = None,
    policy: AutonomousDomainPolicy | None = None,
    required_model_capabilities: Sequence[str] | None = None,
) -> AutonomousTaskDecision:
    """Validate a persisted decision and optionally replay it against live task artifacts.

    A serialized decision is guidance metadata, not an authorization token.  Structural
    validation checks its digest, markers, bounded fields, and enum values.  When the original
    intent, lens, and policy are available, the deterministic decision is recomputed and every
    descriptor field must match.  This prevents stale or tampered decisions from crossing a
    restart boundary into provider, source, tool, evaluator, or effect execution.
    """

    if isinstance(value, AutonomousTaskDecision):
        decision = value
    else:
        if not isinstance(value, Mapping):
            raise ArgumentError("task decision must be an object")
        allowed = {
            "schema", "decision_version", "domain", "workflow_id", "task_digest", "intent_id",
            "intent_digest", "lens_digest", "policy_digest", "decision_id", "posture",
            "recommended_path", "requested_effect", "evidence_posture",
            "required_model_capabilities", "preferred_model_capabilities", "approval_requirements",
            "review_reasons", "blocking_reasons", "next_actions", "decision_digest",
            "authorization", "retention", "secret_material",
        }
        if set(value).difference(allowed):
            raise ArgumentError("task decision contains unsupported fields")
        if (
            value.get("schema") != AUTONOMOUS_TASK_DECISION_SCHEMA
            or value.get("authorization") != "guidance_only;provider_source_tool_and_effect_authority_remain_separate"
            or value.get("retention") != "value_only_decision_metadata;task_text_not_retained"
            or value.get("secret_material") != "never_returned"
        ):
            raise ArgumentError("task decision markers are invalid")

        def items(name: str) -> tuple[str, ...]:
            raw = value.get(name)
            if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes)):
                raise ArgumentError(f"task decision {name} must be a sequence")
            return tuple(raw)

        try:
            decision = AutonomousTaskDecision(
                domain=value.get("domain"),
                workflow_id=value.get("workflow_id"),
                task_digest=value.get("task_digest"),
                intent_id=value.get("intent_id"),
                intent_digest=value.get("intent_digest"),
                lens_digest=value.get("lens_digest"),
                policy_digest=value.get("policy_digest"),
                decision_id=value.get("decision_id"),
                posture=value.get("posture"),
                recommended_path=value.get("recommended_path"),
                requested_effect=value.get("requested_effect"),
                evidence_posture=value.get("evidence_posture"),
                required_model_capabilities=items("required_model_capabilities"),
                preferred_model_capabilities=items("preferred_model_capabilities"),
                approval_requirements=items("approval_requirements"),
                review_reasons=items("review_reasons"),
                blocking_reasons=items("blocking_reasons"),
                next_actions=items("next_actions"),
                decision_version=value.get("decision_version"),
            )
        except (TypeError, ValueError) as error:
            raise ArgumentError("task decision fields are malformed") from error
        if value.get("decision_digest") != decision.decision_digest:
            raise ArgumentError("task decision digest does not match its metadata")

    bindings = (intent, lens, policy)
    if any(item is not None for item in bindings):
        if not all(isinstance(item, expected) for item, expected in zip(bindings, (AutonomousTaskIntent, AutonomousDomainTaskLens, AutonomousDomainPolicy))):
            raise ArgumentError("task decision replay requires intent, lens, and policy together")
        replay = infer_autonomous_task_decision(
            intent=intent,
            lens=lens,
            policy=policy,
            required_model_capabilities=(
                decision.required_model_capabilities
                if required_model_capabilities is None
                else required_model_capabilities
            ),
        )
        if replay._descriptor() != decision._descriptor():
            raise ArgumentError("task decision does not match the supplied intent, lens, and policy")
    elif required_model_capabilities is not None:
        raise ArgumentError("task decision replay capabilities require intent, lens, and policy")
    return decision


__all__ = [
    "AUTONOMOUS_TASK_DECISION_SCHEMA",
    "AUTONOMOUS_TASK_DECISION_VERSION",
    "AUTONOMOUS_TASK_DECISION_POSTURES",
    "AUTONOMOUS_TASK_DECISION_PATHS",
    "AUTONOMOUS_TASK_DECISION_APPROVALS",
    "AUTONOMOUS_TASK_DECISION_EVIDENCE_POSTURES",
    "MAX_AUTONOMOUS_TASK_DECISION_ITEMS",
    "AutonomousTaskDecision",
    "infer_autonomous_task_decision",
    "validate_autonomous_task_decision",
]
