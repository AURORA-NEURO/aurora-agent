"""Provider-free domain execution policies for the autonomous brain.

The policy is deliberately separate from provider selection and authorization.  It supplies
bounded defaults for every built-in domain, binds those defaults into task blueprints, and lets an
application explain why a planned invocation needs review before it spends a provider budget.
The result contains only value metadata and digests; prompts, credentials, evidence, and provider
responses remain outside this module.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
from typing import Any, Mapping

from .authoring import content_digest
from .errors import ArgumentError

AUTONOMOUS_DOMAIN_POLICY_SCHEMA = "bioprism-autonomous-domain-policy/0.1"
AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA = "bioprism-autonomous-domain-policy-admission/0.1"
AUTONOMOUS_DOMAIN_POLICY_VERSION = "0.1"
AUTONOMOUS_DOMAIN_POLICY_DOMAINS = (
    "coding",
    "browser",
    "data",
    "science",
    "biomedical",
    "neuroscience",
    "operations",
    "enterprise",
    "multi_agent",
    "multimodal",
    "cross_domain",
    "evaluation",
)
_RESPONSE_MODES = ("freeform_allowed", "structured_required")
_EVIDENCE_MODES = ("optional", "required_before_provider")
_EFFECT_MODES = ("read_only", "approval_gated", "forbidden")
_LEARNING_MODES = ("health_only", "evaluator_credit", "evaluator_credit_and_trajectory")


class AutonomousDomainPolicyError(ArgumentError):
    """A domain policy or preflight admission is malformed."""


@dataclass(frozen=True, slots=True)
class AutonomousDomainPolicy:
    domain: str
    policy_id: str
    max_input_tokens: int
    max_output_tokens: int
    max_provider_attempts: int
    max_tool_turns: int
    max_total_cost_units: int
    min_route_confidence: float
    min_selection_confidence: float
    min_selection_margin: float
    response_mode: str
    evidence_mode: str
    effect_mode: str
    learning_mode: str
    evaluator_required: bool
    plan_acceptance_required: bool
    policy_digest: str
    policy_version: str = AUTONOMOUS_DOMAIN_POLICY_VERSION

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_POLICY_DOMAINS:
            raise AutonomousDomainPolicyError(f"unsupported autonomous domain policy domain: {self.domain!r}")
        if not self.policy_id or not isinstance(self.policy_id, str):
            raise AutonomousDomainPolicyError("domain policy policy_id must be non-empty")
        for name in (
            "max_input_tokens",
            "max_output_tokens",
            "max_provider_attempts",
            "max_tool_turns",
            "max_total_cost_units",
        ):
            value = getattr(self, name)
            if isinstance(value, bool) or not isinstance(value, int) or value < 1 or value > 1_000_000:
                raise AutonomousDomainPolicyError(f"domain policy {name} is outside its bounds")
        for name in ("min_route_confidence", "min_selection_confidence", "min_selection_margin"):
            value = getattr(self, name)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= float(value) <= 1:
                raise AutonomousDomainPolicyError(f"domain policy {name} is outside its bounds")
        if self.response_mode not in _RESPONSE_MODES:
            raise AutonomousDomainPolicyError("domain policy response_mode is unsupported")
        if self.evidence_mode not in _EVIDENCE_MODES:
            raise AutonomousDomainPolicyError("domain policy evidence_mode is unsupported")
        if self.effect_mode not in _EFFECT_MODES:
            raise AutonomousDomainPolicyError("domain policy effect_mode is unsupported")
        if self.learning_mode not in _LEARNING_MODES:
            raise AutonomousDomainPolicyError("domain policy learning_mode is unsupported")
        if not isinstance(self.evaluator_required, bool) or not isinstance(self.plan_acceptance_required, bool):
            raise AutonomousDomainPolicyError("domain policy boolean controls are malformed")
        if not isinstance(self.policy_digest, str) or len(self.policy_digest) != 64 or any(c not in "0123456789abcdef" for c in self.policy_digest):
            raise AutonomousDomainPolicyError("domain policy digest must be lowercase SHA-256")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_DOMAIN_POLICY_SCHEMA,
            "domain": self.domain,
            "policy_id": self.policy_id,
            "policy_version": self.policy_version,
            "max_input_tokens": self.max_input_tokens,
            "max_output_tokens": self.max_output_tokens,
            "max_provider_attempts": self.max_provider_attempts,
            "max_tool_turns": self.max_tool_turns,
            "max_total_cost_units": self.max_total_cost_units,
            "min_route_confidence": self.min_route_confidence,
            "min_selection_confidence": self.min_selection_confidence,
            "min_selection_margin": self.min_selection_margin,
            "response_mode": self.response_mode,
            "evidence_mode": self.evidence_mode,
            "effect_mode": self.effect_mode,
            "learning_mode": self.learning_mode,
            "evaluator_required": self.evaluator_required,
            "plan_acceptance_required": self.plan_acceptance_required,
            "policy_digest": self.policy_digest,
            "retention": "value_only_policy_metadata",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousDomainPolicyAdmission:
    domain: str
    policy_digest: str
    decision: str
    reasons: tuple[str, ...]
    checked: Mapping[str, bool]
    effective_limits: Mapping[str, int]
    admission_digest: str

    def __post_init__(self) -> None:
        if self.domain not in AUTONOMOUS_DOMAIN_POLICY_DOMAINS:
            raise AutonomousDomainPolicyError("domain policy admission domain is unsupported")
        if self.decision not in {"admitted", "review_required", "blocked"}:
            raise AutonomousDomainPolicyError("domain policy admission decision is unsupported")
        if not isinstance(self.reasons, tuple) or any(not isinstance(reason, str) for reason in self.reasons):
            raise AutonomousDomainPolicyError("domain policy admission reasons are malformed")
        if not isinstance(self.checked, Mapping) or not isinstance(self.effective_limits, Mapping):
            raise AutonomousDomainPolicyError("domain policy admission projections are malformed")
        if not isinstance(self.admission_digest, str) or len(self.admission_digest) != 64:
            raise AutonomousDomainPolicyError("domain policy admission digest is malformed")

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA,
            "domain": self.domain,
            "policy_digest": self.policy_digest,
            "decision": self.decision,
            "reasons": list(self.reasons),
            "checked": dict(self.checked),
            "effective_limits": dict(self.effective_limits),
            "retention": "value_only_admission_metadata",
            "secret_material": "never_returned",
            "admission_digest": self.admission_digest,
        }


_SEEDS: dict[str, dict[str, Any]] = {
    "coding": dict(max_input_tokens=16_000, max_output_tokens=6_000, max_provider_attempts=3, max_tool_turns=12, max_total_cost_units=16, min_route_confidence=0.55, min_selection_confidence=0.58, min_selection_margin=0.06, response_mode="structured_required", evidence_mode="optional", effect_mode="approval_gated", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "browser": dict(max_input_tokens=12_000, max_output_tokens=4_000, max_provider_attempts=3, max_tool_turns=8, max_total_cost_units=12, min_route_confidence=0.62, min_selection_confidence=0.62, min_selection_margin=0.08, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="read_only", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "data": dict(max_input_tokens=16_000, max_output_tokens=6_000, max_provider_attempts=3, max_tool_turns=10, max_total_cost_units=16, min_route_confidence=0.58, min_selection_confidence=0.60, min_selection_margin=0.07, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
    "science": dict(max_input_tokens=16_000, max_output_tokens=7_000, max_provider_attempts=3, max_tool_turns=10, max_total_cost_units=18, min_route_confidence=0.62, min_selection_confidence=0.64, min_selection_margin=0.09, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
    "biomedical": dict(max_input_tokens=14_000, max_output_tokens=5_000, max_provider_attempts=2, max_tool_turns=8, max_total_cost_units=12, min_route_confidence=0.72, min_selection_confidence=0.70, min_selection_margin=0.12, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="forbidden", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "neuroscience": dict(max_input_tokens=14_000, max_output_tokens=5_000, max_provider_attempts=2, max_tool_turns=8, max_total_cost_units=12, min_route_confidence=0.68, min_selection_confidence=0.68, min_selection_margin=0.11, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="read_only", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
    "operations": dict(max_input_tokens=14_000, max_output_tokens=5_000, max_provider_attempts=2, max_tool_turns=8, max_total_cost_units=12, min_route_confidence=0.68, min_selection_confidence=0.70, min_selection_margin=0.12, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "enterprise": dict(max_input_tokens=14_000, max_output_tokens=5_000, max_provider_attempts=3, max_tool_turns=8, max_total_cost_units=14, min_route_confidence=0.62, min_selection_confidence=0.64, min_selection_margin=0.10, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "multi_agent": dict(max_input_tokens=18_000, max_output_tokens=6_000, max_provider_attempts=3, max_tool_turns=12, max_total_cost_units=20, min_route_confidence=0.64, min_selection_confidence=0.66, min_selection_margin=0.10, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
    "multimodal": dict(max_input_tokens=20_000, max_output_tokens=7_000, max_provider_attempts=3, max_tool_turns=10, max_total_cost_units=20, min_route_confidence=0.64, min_selection_confidence=0.66, min_selection_margin=0.10, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit", evaluator_required=True, plan_acceptance_required=True),
    "cross_domain": dict(max_input_tokens=20_000, max_output_tokens=8_000, max_provider_attempts=3, max_tool_turns=14, max_total_cost_units=24, min_route_confidence=0.68, min_selection_confidence=0.68, min_selection_margin=0.12, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="approval_gated", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
    "evaluation": dict(max_input_tokens=16_000, max_output_tokens=6_000, max_provider_attempts=3, max_tool_turns=10, max_total_cost_units=18, min_route_confidence=0.70, min_selection_confidence=0.72, min_selection_margin=0.12, response_mode="structured_required", evidence_mode="required_before_provider", effect_mode="read_only", learning_mode="evaluator_credit_and_trajectory", evaluator_required=True, plan_acceptance_required=True),
}


def _policy_descriptor(policy: AutonomousDomainPolicy) -> dict[str, Any]:
    value = policy.to_dict()
    value.pop("policy_digest", None)
    return value


def _make_policy(domain: str, overrides: Mapping[str, Any] | None = None) -> AutonomousDomainPolicy:
    if domain not in AUTONOMOUS_DOMAIN_POLICY_DOMAINS:
        raise AutonomousDomainPolicyError(f"unsupported autonomous domain policy domain: {domain!r}")
    values = dict(_SEEDS[domain])
    if overrides is not None:
        if not isinstance(overrides, Mapping):
            raise AutonomousDomainPolicyError("domain policy overrides must be a mapping")
        unknown = sorted(set(overrides).difference(values))
        if unknown:
            raise AutonomousDomainPolicyError("unsupported domain policy overrides: " + ", ".join(unknown))
        values.update(overrides)
    policy = AutonomousDomainPolicy(
        domain=domain,
        policy_id=f"builtin-{domain}-execution-policy",
        policy_digest="0" * 64,
        **values,
    )
    return replace(policy, policy_digest=content_digest(_policy_descriptor(policy)))


_BUILTIN_POLICIES = {domain: _make_policy(domain) for domain in AUTONOMOUS_DOMAIN_POLICY_DOMAINS}


def builtin_autonomous_domain_policies() -> tuple[AutonomousDomainPolicy, ...]:
    """Return all twelve immutable built-in policies in canonical domain order."""

    return tuple(_BUILTIN_POLICIES[domain] for domain in AUTONOMOUS_DOMAIN_POLICY_DOMAINS)


def autonomous_domain_policy(domain: str, overrides: Mapping[str, Any] | None = None) -> AutonomousDomainPolicy:
    """Resolve a policy without contacting a provider, source, tool, or evaluator."""

    if overrides:
        return _make_policy(domain, overrides)
    try:
        return _BUILTIN_POLICIES[domain]
    except KeyError as error:
        raise AutonomousDomainPolicyError(f"unsupported autonomous domain policy domain: {domain!r}") from error


def evaluate_autonomous_domain_policy(
    policy: AutonomousDomainPolicy,
    *,
    route_confidence: float | None = None,
    route_abstained: bool | None = None,
    selection_confidence: float | None = None,
    selection_margin: float | None = None,
    estimated_input_tokens: int | None = None,
    requested_output_tokens: int | None = None,
    estimated_cost_units: int | None = None,
    structured_response: bool | None = None,
    evidence_ready: bool | None = None,
    evaluator_configured: bool | None = None,
    plan_accepted: bool | None = None,
    effects_requested: bool | None = None,
    effects_approved: bool | None = None,
) -> AutonomousDomainPolicyAdmission:
    """Evaluate every provider-free gate and return an explainable admission projection."""

    if not isinstance(policy, AutonomousDomainPolicy):
        raise AutonomousDomainPolicyError("domain policy admission requires a valid policy")
    optional_values = {
        "route_confidence": route_confidence,
        "selection_confidence": selection_confidence,
        "selection_margin": selection_margin,
    }
    for name, value in optional_values.items():
        if value is not None and (isinstance(value, bool) or not isinstance(value, (int, float)) or not 0 <= float(value) <= 1):
            raise AutonomousDomainPolicyError(f"domain policy {name} is outside its bounds")
    for name, value in {"estimated_input_tokens": estimated_input_tokens, "requested_output_tokens": requested_output_tokens, "estimated_cost_units": estimated_cost_units}.items():
        if value is not None and (isinstance(value, bool) or not isinstance(value, int) or value < 0 or value > 1_000_000):
            raise AutonomousDomainPolicyError(f"domain policy {name} is outside its bounds")
    for name, value in {"route_abstained": route_abstained, "structured_response": structured_response, "evidence_ready": evidence_ready, "evaluator_configured": evaluator_configured, "plan_accepted": plan_accepted, "effects_requested": effects_requested, "effects_approved": effects_approved}.items():
        if value is not None and not isinstance(value, bool):
            raise AutonomousDomainPolicyError(f"domain policy {name} must be boolean when supplied")
    blocked: list[str] = []
    review: list[str] = []
    if route_abstained is True:
        blocked.append("route_abstained")
    if route_confidence is not None and route_confidence < policy.min_route_confidence:
        review.append("route_confidence_below_policy_floor")
    if selection_confidence is not None and selection_confidence < policy.min_selection_confidence:
        review.append("selection_confidence_below_policy_floor")
    if selection_margin is not None and selection_margin < policy.min_selection_margin:
        review.append("selection_margin_below_policy_floor")
    if estimated_input_tokens is not None and estimated_input_tokens > policy.max_input_tokens:
        blocked.append("input_budget_exceeded")
    if requested_output_tokens is not None and requested_output_tokens > policy.max_output_tokens:
        blocked.append("output_budget_exceeded")
    if estimated_cost_units is not None and estimated_cost_units > policy.max_total_cost_units:
        blocked.append("cost_budget_exceeded")
    if policy.response_mode == "structured_required" and structured_response is not True:
        review.append("structured_response_required")
    if policy.evidence_mode == "required_before_provider" and evidence_ready is not True:
        review.append("evidence_required_before_provider")
    if policy.evaluator_required and evaluator_configured is not True:
        review.append("evaluator_required")
    if policy.plan_acceptance_required and plan_accepted is not True:
        review.append("plan_acceptance_required")
    if effects_requested is True and policy.effect_mode == "forbidden":
        blocked.append("effects_forbidden_by_policy")
    if effects_requested is True and policy.effect_mode == "approval_gated" and effects_approved is not True:
        review.append("effect_approval_required")
    reasons = tuple([*blocked, *review])
    decision = "blocked" if blocked else "review_required" if review else "admitted"
    checked = {
        "route": route_confidence is not None or route_abstained is not None,
        "selection": selection_confidence is not None or selection_margin is not None,
        "budget": estimated_input_tokens is not None or requested_output_tokens is not None or estimated_cost_units is not None,
        "response": policy.response_mode == "structured_required",
        "evidence": policy.evidence_mode == "required_before_provider",
        "evaluator": policy.evaluator_required,
        "plan": policy.plan_acceptance_required,
        "effects": effects_requested is True,
    }
    limits = {
        "max_input_tokens": policy.max_input_tokens,
        "max_output_tokens": policy.max_output_tokens,
        "max_provider_attempts": policy.max_provider_attempts,
        "max_tool_turns": policy.max_tool_turns,
        "max_total_cost_units": policy.max_total_cost_units,
    }
    descriptor = {
        "schema": AUTONOMOUS_DOMAIN_POLICY_ADMISSION_SCHEMA,
        "domain": policy.domain,
        "policy_digest": policy.policy_digest,
        "decision": decision,
        "reasons": list(reasons),
        "checked": checked,
        "effective_limits": limits,
        "retention": "value_only_admission_metadata",
        "secret_material": "never_returned",
    }
    return AutonomousDomainPolicyAdmission(
        domain=policy.domain,
        policy_digest=policy.policy_digest,
        decision=decision,
        reasons=reasons,
        checked=checked,
        effective_limits=limits,
        admission_digest=content_digest(descriptor),
    )
