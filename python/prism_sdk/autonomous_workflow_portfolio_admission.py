"""Provider-free admission for digest-bound autonomous workflow portfolios.

Planning answers "what would this portfolio contain?" and execution answers "what happened?".
Admission is the deliberately separate gate between them: it joins the reviewed plan to current
model/readiness/tool/evidence/learning metadata, computes dependency-closed eligibility, and
returns a redacted artifact that a caller can review before approving provider calls.

This module never resolves credentials, invokes a provider, dispatches a tool or connector, or
authorizes an effect. It persists only digests, model identities, bounded gate metadata, and
next actions. The execution layer binds an optional admission digest into its portfolio input
identity so a restart cannot silently continue with a different admission posture.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from collections.abc import Mapping, Sequence
from typing import Any, TYPE_CHECKING

from .authoring import content_digest
from .autonomy import AUTONOMOUS_DOMAINS, BrainRunError
from .autonomous_workflow_portfolio import (
    AutonomousWorkflowPortfolioItem,
    AutonomousWorkflowPortfolioPlan,
    plan_autonomous_workflow_portfolio,
    verify_autonomous_workflow_portfolio,
)

if TYPE_CHECKING:
    from .autonomy import AutonomousAgent


AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA = (
    "bioprism-python-autonomous-workflow-portfolio-admission/0.1"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_EXECUTION = (
    "admission_only;no_provider_tool_connector_or_effect_dispatch"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_AUTHORIZATION = (
    "admission_does_not_authorize_provider_tools_connectors_or_effects"
)
AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION = (
    "metadata_only_admission_and_plan_digests;tasks_prompts_credentials_and_provider_values_never_persisted"
)
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS = 32
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BLOCKERS = 32
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_MODELS = 128
MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES = 1_000_000
_SECRET_MATERIAL = "never_returned"
_IDENTIFIER_CHARS = frozenset("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:/-")
_READINESS_STATES = {
    "ready_for_caller_approval",
    "model_catalogue_required",
    "provider_registration_required",
    "credential_required",
    "model_capability_gap",
    "partial",
    "not_evaluated",
}
_ITEM_STATUSES = {"eligible", "blocked", "dependency_blocked", "route_review_required"}


def _identifier(label: str, value: Any, *, maximum: int = 256) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value.encode("utf-8")) > maximum
        or any(character not in _IDENTIFIER_CHARS for character in value)
    ):
        raise BrainRunError(f"{label} is outside its identifier contract")
    return value


def _text(label: str, value: Any, *, maximum: int = 512) -> str:
    if not isinstance(value, str) or not value or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise BrainRunError(f"{label} is outside its text contract")
    return value


def _digest(label: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise BrainRunError(f"{label} must be a lowercase SHA-256 digest")
    return value


def _strings(label: str, value: Any, *, maximum: int = 128) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise BrainRunError(f"{label} must contain at most {maximum} entries")
    result = tuple(_text(f"{label} entry", item) for item in value)
    if len(set(result)) != len(result):
        raise BrainRunError(f"{label} must not contain duplicates")
    return result


def _sequence(label: str, value: Any) -> tuple[Any, ...]:
    """Normalize a JSON array without allowing strings to masquerade as arrays."""

    if value is None:
        return ()
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise BrainRunError(f"{label} must be an array")
    return tuple(value)


def _finite_number(label: str, value: Any, *, minimum: float = 0.0) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or float(value) < minimum:
        raise BrainRunError(f"{label} is outside its bounds")
    return float(value)


def _bounded_positive_int(label: str, value: Any, *, maximum: int = 1_000_000) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise BrainRunError(f"{label} must be between 1 and {maximum}")
    return value


def _safe_digest(value: Any, label: str) -> str:
    try:
        json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        raise BrainRunError(f"{label} must be JSON-safe") from error
    return content_digest(value)


def _candidate_mapping(value: Any, index: int) -> dict[str, Any]:
    if hasattr(value, "to_dict") and callable(value.to_dict):
        value = value.to_dict()
    if not isinstance(value, Mapping):
        raise BrainRunError(f"workflow portfolio admission model candidate {index} is malformed")
    provider = _identifier(f"workflow portfolio admission candidate {index} provider", value.get("provider"))
    model = _identifier(f"workflow portfolio admission candidate {index} model", value.get("model"))
    capabilities = _strings(
        f"workflow portfolio admission candidate {index} capabilities",
        value.get("capabilities", ()),
        maximum=128,
    )
    enabled = value.get("enabled", True)
    if not isinstance(enabled, bool):
        raise BrainRunError("workflow portfolio admission candidate enabled must be boolean")
    cost = _finite_number(
        f"workflow portfolio admission candidate {index} cost_per_million_tokens",
        value.get("cost_per_million_tokens"),
    )
    latency = _finite_number(
        f"workflow portfolio admission candidate {index} latency_ms",
        value.get("latency_ms"),
    )
    quality = _finite_number(
        f"workflow portfolio admission candidate {index} quality",
        value.get("quality"),
    )
    return {
        "provider": provider,
        "model": model,
        "capabilities": capabilities,
        "enabled": enabled,
        "cost_per_million_tokens": cost,
        "latency_ms": latency,
        "quality": quality,
        "model_id": f"{provider}/{model}",
    }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioAdmissionPolicy:
    require_all_domains: bool
    allow_partial: bool
    verify_plan: bool
    require_available_tools: bool
    require_calibrated_learning: bool
    input_tokens: int
    output_tokens: int
    max_cost_per_million_tokens: float | None = None
    max_latency_ms: float | None = None
    min_quality: float | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("require_all_domains", self.require_all_domains),
            ("allow_partial", self.allow_partial),
            ("verify_plan", self.verify_plan),
            ("require_available_tools", self.require_available_tools),
            ("require_calibrated_learning", self.require_calibrated_learning),
        ):
            if not isinstance(value, bool):
                raise BrainRunError(f"workflow portfolio admission policy {name} must be boolean")
        _bounded_positive_int("workflow portfolio admission policy input_tokens", self.input_tokens)
        _bounded_positive_int("workflow portfolio admission policy output_tokens", self.output_tokens)
        for name, value in (
            ("max_cost_per_million_tokens", self.max_cost_per_million_tokens),
            ("max_latency_ms", self.max_latency_ms),
            ("min_quality", self.min_quality),
        ):
            _finite_number(f"workflow portfolio admission policy {name}", value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "require_all_domains": self.require_all_domains,
            "allow_partial": self.allow_partial,
            "verify_plan": self.verify_plan,
            "require_available_tools": self.require_available_tools,
            "require_calibrated_learning": self.require_calibrated_learning,
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "max_cost_per_million_tokens": self.max_cost_per_million_tokens,
            "max_latency_ms": self.max_latency_ms,
            "min_quality": self.min_quality,
        }

    @classmethod
    def from_dict(cls, value: Any) -> "AutonomousWorkflowPortfolioAdmissionPolicy":
        if not isinstance(value, Mapping):
            raise BrainRunError("workflow portfolio admission policy must be an object")
        allowed = {
            "require_all_domains", "allow_partial", "verify_plan", "require_available_tools",
            "require_calibrated_learning", "input_tokens", "output_tokens",
            "max_cost_per_million_tokens", "max_latency_ms", "min_quality",
        }
        if set(value).difference(allowed):
            raise BrainRunError("workflow portfolio admission policy contains unsupported fields")
        return cls(
            require_all_domains=value.get("require_all_domains"),
            allow_partial=value.get("allow_partial"),
            verify_plan=value.get("verify_plan"),
            require_available_tools=value.get("require_available_tools"),
            require_calibrated_learning=value.get("require_calibrated_learning"),
            input_tokens=value.get("input_tokens"),
            output_tokens=value.get("output_tokens"),
            max_cost_per_million_tokens=value.get("max_cost_per_million_tokens"),
            max_latency_ms=value.get("max_latency_ms"),
            min_quality=value.get("min_quality"),
        )


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioAdmissionItem:
    item_id: str
    domain: str
    depends_on: tuple[str, ...]
    dependency_statuses: Mapping[str, str]
    plan_status: str
    status: str
    readiness_state: str
    workflow_digest: str | None
    plan_digest: str | None
    request_digest: str
    required_model_capabilities: tuple[str, ...]
    compatible_model_count: int
    eligible_model_count: int
    eligible_model_ids: tuple[str, ...]
    missing_tool_capabilities: tuple[str, ...]
    blockers: tuple[str, ...]
    next_actions: tuple[str, ...]

    def __post_init__(self) -> None:
        _identifier("workflow portfolio admission item_id", self.item_id)
        if self.domain not in AUTONOMOUS_DOMAINS:
            raise BrainRunError("workflow portfolio admission item domain is unsupported")
        dependencies = _strings("workflow portfolio admission depends_on", self.depends_on, maximum=16)
        if any(dependency == self.item_id for dependency in dependencies):
            raise BrainRunError("workflow portfolio admission item cannot depend on itself")
        if not isinstance(self.dependency_statuses, Mapping):
            raise BrainRunError("workflow portfolio admission dependency_statuses must be an object")
        if set(self.dependency_statuses) != set(dependencies):
            raise BrainRunError("workflow portfolio admission dependency_statuses do not match depends_on")
        if any(status not in _ITEM_STATUSES for status in self.dependency_statuses.values()):
            raise BrainRunError("workflow portfolio admission dependency status is invalid")
        if self.plan_status not in {"ready", "blocked", "failed", "route_review_required"}:
            raise BrainRunError("workflow portfolio admission plan_status is invalid")
        if self.status not in _ITEM_STATUSES:
            raise BrainRunError("workflow portfolio admission item status is invalid")
        if self.readiness_state not in _READINESS_STATES:
            raise BrainRunError("workflow portfolio admission readiness_state is invalid")
        if self.workflow_digest is not None:
            _digest("workflow portfolio admission workflow_digest", self.workflow_digest)
        if self.plan_digest is not None:
            _digest("workflow portfolio admission plan_digest", self.plan_digest)
        _digest("workflow portfolio admission request_digest", self.request_digest)
        required = _strings("workflow portfolio admission required_model_capabilities", self.required_model_capabilities)
        model_ids = _strings(
            "workflow portfolio admission eligible_model_ids",
            self.eligible_model_ids,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_MODELS,
        )
        missing_tools = _strings("workflow portfolio admission missing_tool_capabilities", self.missing_tool_capabilities)
        blockers = _strings(
            "workflow portfolio admission blockers",
            self.blockers,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BLOCKERS,
        )
        actions = _strings(
            "workflow portfolio admission next_actions",
            self.next_actions,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS,
        )
        for name, value in (
            ("compatible_model_count", self.compatible_model_count),
            ("eligible_model_count", self.eligible_model_count),
        ):
            if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_MODELS:
                raise BrainRunError(f"workflow portfolio admission {name} is outside its bound")
        if self.eligible_model_count != len(model_ids) or self.eligible_model_count > self.compatible_model_count:
            raise BrainRunError("workflow portfolio admission model counts are inconsistent")
        object.__setattr__(self, "depends_on", dependencies)
        object.__setattr__(self, "required_model_capabilities", required)
        object.__setattr__(self, "eligible_model_ids", model_ids)
        object.__setattr__(self, "missing_tool_capabilities", missing_tools)
        object.__setattr__(self, "blockers", blockers)
        object.__setattr__(self, "next_actions", actions)

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
            "item_id": self.item_id,
            "domain": self.domain,
            "depends_on": list(self.depends_on),
            "dependency_statuses": dict(sorted(self.dependency_statuses.items())),
            "plan_status": self.plan_status,
            "status": self.status,
            "readiness_state": self.readiness_state,
            "workflow_digest": self.workflow_digest,
            "plan_digest": self.plan_digest,
            "request_digest": self.request_digest,
            "required_model_capabilities": list(self.required_model_capabilities),
            "compatible_model_count": self.compatible_model_count,
            "eligible_model_count": self.eligible_model_count,
            "eligible_model_ids": list(self.eligible_model_ids),
            "missing_tool_capabilities": list(self.missing_tool_capabilities),
            "blockers": list(self.blockers),
            "next_actions": list(self.next_actions),
            "approval": "caller_reviews_and_approves_each_provider_call",
            "selection": "runtime_model_selection_rechecks_policy_and_health",
            "retention": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioAdmissionCounts:
    item_count: int
    eligible_count: int
    blocked_count: int
    dependency_blocked_count: int
    route_review_required_count: int
    missing_model_count: int
    missing_provider_count: int
    credential_required_count: int
    calibration_hold_count: int
    evidence_hold_count: int
    tool_gap_count: int

    def to_dict(self) -> dict[str, int]:
        return {
            "item_count": self.item_count,
            "eligible_count": self.eligible_count,
            "blocked_count": self.blocked_count,
            "dependency_blocked_count": self.dependency_blocked_count,
            "route_review_required_count": self.route_review_required_count,
            "missing_model_count": self.missing_model_count,
            "missing_provider_count": self.missing_provider_count,
            "credential_required_count": self.credential_required_count,
            "calibration_hold_count": self.calibration_hold_count,
            "evidence_hold_count": self.evidence_hold_count,
            "tool_gap_count": self.tool_gap_count,
        }


def _counts(items: Sequence[AutonomousWorkflowPortfolioAdmissionItem]) -> AutonomousWorkflowPortfolioAdmissionCounts:
    def contains(items_to_check: Sequence[AutonomousWorkflowPortfolioAdmissionItem], text: str) -> int:
        return sum(text in item.blockers for item in items_to_check)

    def contains_any(items_to_check: Sequence[AutonomousWorkflowPortfolioAdmissionItem], texts: tuple[str, ...]) -> int:
        return sum(any(text in item.blockers for text in texts) for item in items_to_check)

    return AutonomousWorkflowPortfolioAdmissionCounts(
        item_count=len(items),
        eligible_count=sum(item.status == "eligible" for item in items),
        blocked_count=sum(item.status == "blocked" for item in items),
        dependency_blocked_count=sum(item.status == "dependency_blocked" for item in items),
        route_review_required_count=sum(item.status == "route_review_required" for item in items),
        missing_model_count=contains_any(
            items,
            (
                "readiness:model_catalogue_required",
                "readiness:model_capability_gap",
                "selection:no_model_matches_run_constraints",
            ),
        ),
        missing_provider_count=contains(items, "readiness:provider_registration_required"),
        credential_required_count=contains(items, "readiness:credential_required"),
        calibration_hold_count=contains(items, "calibration:hold"),
        evidence_hold_count=contains(items, "evidence:not_ready"),
        tool_gap_count=contains(items, "tools:missing"),
    )


def _admission_status(plan: AutonomousWorkflowPortfolioPlan, items: Sequence[AutonomousWorkflowPortfolioAdmissionItem]) -> str:
    eligible = sum(item.status == "eligible" for item in items)
    if plan.status == "blocked" or eligible == 0:
        return "blocked"
    if plan.status == "partial":
        return "partial"
    if eligible != len(items) and not plan.allow_partial:
        return "blocked"
    return "ready_for_approval" if eligible == len(items) else "partial"


def _next_actions(
    status: str,
    plan: AutonomousWorkflowPortfolioPlan,
    items: Sequence[AutonomousWorkflowPortfolioAdmissionItem],
) -> tuple[str, ...]:
    actions = {action for item in items for action in item.next_actions}
    if plan.status == "partial":
        actions.add("resolve_missing_required_domain_coverage_before_full_portfolio_execution")
    if status == "ready_for_approval":
        actions.add("review_admission_digest_then_approve_provider_calls_per_item")
    elif status == "partial":
        actions.add("resolve_blocked_items_or_explicitly_accept_partial_portfolio_execution")
    else:
        actions.add("resolve_portfolio_admission_blockers_before_dispatch")
    return tuple(sorted(actions)[:MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS])


@dataclass(frozen=True, slots=True)
class AutonomousWorkflowPortfolioAdmission:
    status: str
    plan: AutonomousWorkflowPortfolioPlan
    policy: AutonomousWorkflowPortfolioAdmissionPolicy
    readiness_digest: str
    items: tuple[AutonomousWorkflowPortfolioAdmissionItem, ...]
    counts: AutonomousWorkflowPortfolioAdmissionCounts
    next_actions: tuple[str, ...]
    admission_digest: str

    def __post_init__(self) -> None:
        if self.status not in {"ready_for_approval", "partial", "blocked"}:
            raise BrainRunError("workflow portfolio admission status is invalid")
        if not isinstance(self.plan, AutonomousWorkflowPortfolioPlan):
            raise BrainRunError("workflow portfolio admission plan is invalid")
        if not isinstance(self.policy, AutonomousWorkflowPortfolioAdmissionPolicy):
            raise BrainRunError("workflow portfolio admission policy is invalid")
        _digest("workflow portfolio admission readiness_digest", self.readiness_digest)
        if len(self.items) != len(self.plan.items):
            raise BrainRunError("workflow portfolio admission must cover every plan item")
        if {item.item_id for item in self.items} != {item.item_id for item in self.plan.items}:
            raise BrainRunError("workflow portfolio admission item ids do not match the plan")
        if not isinstance(self.counts, AutonomousWorkflowPortfolioAdmissionCounts):
            raise BrainRunError("workflow portfolio admission counts are invalid")
        if self.counts.to_dict() != _counts(self.items).to_dict():
            raise BrainRunError("workflow portfolio admission counts are inconsistent")
        actions = _strings(
            "workflow portfolio admission next_actions",
            self.next_actions,
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS,
        )
        if tuple(sorted(actions)) != actions:
            raise BrainRunError("workflow portfolio admission next_actions must be sorted")
        _digest("workflow portfolio admission admission_digest", self.admission_digest)
        object.__setattr__(self, "items", tuple(sorted(self.items, key=lambda item: item.item_id)))
        object.__setattr__(self, "next_actions", actions)

    def _descriptor(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
            "status": self.status,
            "plan": self.plan.to_dict(),
            "policy": self.policy.to_dict(),
            "readiness_digest": self.readiness_digest,
            "items": [item.to_dict() for item in self.items],
            "dependency_graph": self.plan.dependency_graph.to_dict(),
            "waves": [list(wave) for wave in self.plan.dependency_graph.waves],
            "counts": self.counts.to_dict(),
            "next_actions": list(self.next_actions),
            "execution": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_EXECUTION,
            "authorization": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_AUTHORIZATION,
            "retention": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._descriptor(), "admission_digest": self.admission_digest}

    @classmethod
    def from_dict(cls, value: Any) -> "AutonomousWorkflowPortfolioAdmission":
        if not isinstance(value, Mapping):
            raise BrainRunError("workflow portfolio admission must be an object")
        allowed = {
            "schema", "status", "plan", "policy", "readiness_digest", "items", "dependency_graph", "waves",
            "counts", "next_actions", "execution", "authorization", "retention", "secret_material", "admission_digest",
        }
        if set(value).difference(allowed):
            raise BrainRunError("workflow portfolio admission contains unsupported fields")
        if (
            value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA
            or value.get("execution") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_EXECUTION
            or value.get("authorization") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_AUTHORIZATION
            or value.get("retention") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION
            or value.get("secret_material") != _SECRET_MATERIAL
        ):
            raise BrainRunError("workflow portfolio admission markers are invalid")
        plan = AutonomousWorkflowPortfolioPlan.from_dict(value.get("plan"))
        policy = AutonomousWorkflowPortfolioAdmissionPolicy.from_dict(value.get("policy"))
        raw_items = value.get("items")
        if isinstance(raw_items, (str, bytes)) or not isinstance(raw_items, Sequence):
            raise BrainRunError("workflow portfolio admission items are invalid")
        items = tuple(_item_from_dict(raw) for raw in _sequence("workflow portfolio admission items", raw_items))
        by_id = {item.item_id: item for item in items}
        plan_by_id = {item.item_id: item for item in plan.items}
        if set(by_id) != set(plan_by_id) or len(by_id) != len(items):
            raise BrainRunError("workflow portfolio admission item ids do not match the plan")
        for item_id, item in by_id.items():
            plan_item = plan_by_id[item_id]
            if (
                item.domain != plan_item.domain
                or item.depends_on != plan_item.depends_on
                or item.plan_status != plan_item.status
                or item.request_digest != plan_item.request_digest
                or item.workflow_digest != plan_item.workflow_digest
                or item.plan_digest != plan_item.plan_digest
            ):
                raise BrainRunError("workflow portfolio admission item identity does not match the plan")
        if value.get("dependency_graph") != plan.dependency_graph.to_dict() or value.get("waves") != [list(wave) for wave in plan.dependency_graph.waves]:
            raise BrainRunError("workflow portfolio admission dependency projection is inconsistent")
        count_value = value.get("counts")
        if not isinstance(count_value, Mapping):
            raise BrainRunError("workflow portfolio admission counts are invalid")
        counts = AutonomousWorkflowPortfolioAdmissionCounts(
            **{
                field: count_value.get(field)
                for field in AutonomousWorkflowPortfolioAdmissionCounts.__dataclass_fields__
            }
        )
        status = value.get("status")
        if status != _admission_status(plan, items):
            raise BrainRunError("workflow portfolio admission status is inconsistent")
        actions = _strings(
            "workflow portfolio admission next_actions",
            value.get("next_actions"),
            maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS,
        )
        if actions != _next_actions(status, plan, items):
            raise BrainRunError("workflow portfolio admission next_actions are inconsistent")
        admission = cls(
            status=status,
            plan=plan,
            policy=policy,
            readiness_digest=value.get("readiness_digest"),
            items=items,
            counts=counts,
            next_actions=actions,
            admission_digest=value.get("admission_digest"),
        )
        if content_digest(admission._descriptor()) != admission.admission_digest:
            raise BrainRunError("workflow portfolio admission digest does not match its contents")
        if policy.require_all_domains != plan.require_all_domains or policy.allow_partial != plan.allow_partial:
            raise BrainRunError("workflow portfolio admission policy does not match the plan")
        return admission


def _item_from_dict(value: Any) -> AutonomousWorkflowPortfolioAdmissionItem:
    if not isinstance(value, Mapping):
        raise BrainRunError("workflow portfolio admission item must be an object")
    if value.get("schema") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA:
        raise BrainRunError("workflow portfolio admission item schema is invalid")
    if (
        value.get("approval") != "caller_reviews_and_approves_each_provider_call"
        or value.get("selection") != "runtime_model_selection_rechecks_policy_and_health"
        or value.get("retention") != AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION
        or value.get("secret_material") != _SECRET_MATERIAL
    ):
        raise BrainRunError("workflow portfolio admission item markers are invalid")
    dependency_statuses = value.get("dependency_statuses")
    if not isinstance(dependency_statuses, Mapping):
        raise BrainRunError("workflow portfolio admission dependency_statuses must be an object")
    return AutonomousWorkflowPortfolioAdmissionItem(
        item_id=value.get("item_id"),
        domain=value.get("domain"),
        depends_on=_sequence("workflow portfolio admission depends_on", value.get("depends_on", ())),
        dependency_statuses=dict(dependency_statuses),
        plan_status=value.get("plan_status"),
        status=value.get("status"),
        readiness_state=value.get("readiness_state"),
        workflow_digest=value.get("workflow_digest"),
        plan_digest=value.get("plan_digest"),
        request_digest=value.get("request_digest"),
        required_model_capabilities=_sequence(
            "workflow portfolio admission required_model_capabilities",
            value.get("required_model_capabilities", ()),
        ),
        compatible_model_count=value.get("compatible_model_count"),
        eligible_model_count=value.get("eligible_model_count"),
        eligible_model_ids=_sequence(
            "workflow portfolio admission eligible_model_ids",
            value.get("eligible_model_ids", ()),
        ),
        missing_tool_capabilities=_sequence(
            "workflow portfolio admission missing_tool_capabilities",
            value.get("missing_tool_capabilities", ()),
        ),
        blockers=_sequence("workflow portfolio admission blockers", value.get("blockers", ())),
        next_actions=_sequence("workflow portfolio admission next_actions", value.get("next_actions", ())),
    )


def _row_by_domain(readiness: Mapping[str, Any]) -> dict[str, Mapping[str, Any]]:
    rows = readiness.get("domains")
    if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence):
        raise BrainRunError("workflow portfolio admission readiness domains are invalid")
    result: dict[str, Mapping[str, Any]] = {}
    for row in rows:
        if not isinstance(row, Mapping) or not isinstance(row.get("domain"), str):
            raise BrainRunError("workflow portfolio admission readiness domain row is invalid")
        if row["domain"] in result:
            raise BrainRunError("workflow portfolio admission readiness contains duplicate domains")
        result[row["domain"]] = row
    return result


def _eligible_model_ids(
    candidates: Sequence[Mapping[str, Any]],
    required: Sequence[str],
    policy: AutonomousWorkflowPortfolioAdmissionPolicy,
) -> tuple[tuple[str, ...], int]:
    compatible: list[str] = []
    eligible: list[str] = []
    for candidate in candidates:
        if not set(required).issubset(set(candidate["capabilities"])):
            continue
        compatible.append(candidate["model_id"])
        if not candidate["enabled"]:
            continue
        if policy.max_cost_per_million_tokens is not None and (
            candidate["cost_per_million_tokens"] is None
            or candidate["cost_per_million_tokens"] > policy.max_cost_per_million_tokens
        ):
            continue
        if policy.max_latency_ms is not None and (
            candidate["latency_ms"] is None
            or candidate["latency_ms"] > policy.max_latency_ms
        ):
            continue
        if policy.min_quality is not None and (
            candidate["quality"] is None
            or candidate["quality"] < policy.min_quality
        ):
            continue
        eligible.append(candidate["model_id"])
    return tuple(sorted(eligible)), len(compatible)


def _learning_calibrated(row: Mapping[str, Any] | None) -> bool:
    if row is None:
        return False
    if "calibration_admit_learning" in row:
        return bool(row.get("calibration_admit_learning"))
    return (
        bool(row.get("observed"))
        and isinstance(row.get("evaluation_count"), int)
        and row.get("evaluation_count", 0) > 0
        and isinstance(row.get("explored_arm_count"), int)
        and row.get("explored_arm_count", 0) > 0
    )


def _build_item(
    plan_item: AutonomousWorkflowPortfolioItem,
    readiness_row: Mapping[str, Any] | None,
    learning_row: Mapping[str, Any] | None,
    candidates: Sequence[Mapping[str, Any]],
    policy: AutonomousWorkflowPortfolioAdmissionPolicy,
    tool_plan: Mapping[str, Any] | None,
) -> AutonomousWorkflowPortfolioAdmissionItem:
    required = plan_item.required_capabilities
    eligible_ids, compatible_count = _eligible_model_ids(candidates, required, policy)
    missing_tools = () if tool_plan is None else _strings(
        "workflow portfolio admission missing_tool_capabilities",
        tool_plan.get("missing_tool_capabilities", ()),
        maximum=128,
    )
    readiness_state = "not_evaluated" if readiness_row is None else readiness_row.get("state", "partial")
    if not isinstance(readiness_state, str) or readiness_state not in _READINESS_STATES:
        readiness_state = "partial"
    blockers: set[str] = set()
    actions: set[str] = set()
    if readiness_row is not None:
        actions.update(
            _strings(
                "workflow portfolio admission readiness next_actions",
                readiness_row.get("next_actions", ()),
                maximum=MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS,
            )
        )
        if readiness_state != "ready_for_caller_approval":
            blockers.add(f"readiness:{readiness_state}")
        evidence = readiness_row.get("evidence_readiness")
        if isinstance(evidence, Mapping) and evidence.get("status") != "ready":
            blockers.add("evidence:not_ready")
        if policy.require_available_tools and missing_tools:
            blockers.add("tools:missing")
        if policy.require_calibrated_learning and not _learning_calibrated(learning_row):
            blockers.add("calibration:hold")
    else:
        blockers.add("readiness:domain_missing")
        actions.add("recompute_all_domain_readiness")
    if plan_item.status == "route_review_required":
        blockers = {"plan:route_review_required"}
        actions = {"review_route_before_model_admission"}
        status = "route_review_required"
    elif plan_item.status != "ready":
        blockers = {f"plan:{plan_item.error_class or 'not_ready'}"}
        actions = {"repair_portfolio_plan_before_admission"}
        status = "blocked"
    else:
        if not eligible_ids and readiness_state == "ready_for_caller_approval":
            blockers.add("selection:no_model_matches_run_constraints")
            actions.add("relax_run_constraints_or_register_another_model_arm")
        status = "blocked" if blockers else "eligible"
    return AutonomousWorkflowPortfolioAdmissionItem(
        item_id=plan_item.item_id,
        domain=plan_item.domain,
        depends_on=plan_item.depends_on,
        dependency_statuses={dependency: "blocked" for dependency in plan_item.depends_on},
        plan_status=plan_item.status,
        status=status,
        readiness_state=readiness_state,
        workflow_digest=plan_item.workflow_digest,
        plan_digest=plan_item.plan_digest,
        request_digest=plan_item.request_digest,
        required_model_capabilities=required,
        compatible_model_count=compatible_count,
        eligible_model_count=len(eligible_ids),
        eligible_model_ids=eligible_ids,
        missing_tool_capabilities=missing_tools,
        blockers=tuple(sorted(blockers)),
        next_actions=tuple(sorted(actions))[:MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS],
    )


def _dependency_closure(
    items: Sequence[AutonomousWorkflowPortfolioAdmissionItem],
    plan: AutonomousWorkflowPortfolioPlan,
) -> tuple[AutonomousWorkflowPortfolioAdmissionItem, ...]:
    by_id = {item.item_id: item for item in items}
    for item_id in plan.dependency_graph.topological_order:
        item = by_id[item_id]
        statuses = {dependency: by_id[dependency].status for dependency in item.depends_on}
        if item.status != "route_review_required" and any(status != "eligible" for status in statuses.values()):
            by_id[item_id] = AutonomousWorkflowPortfolioAdmissionItem(
                item_id=item.item_id,
                domain=item.domain,
                depends_on=item.depends_on,
                dependency_statuses=statuses,
                plan_status=item.plan_status,
                status="dependency_blocked",
                readiness_state=item.readiness_state,
                workflow_digest=item.workflow_digest,
                plan_digest=item.plan_digest,
                request_digest=item.request_digest,
                required_model_capabilities=item.required_model_capabilities,
                compatible_model_count=item.compatible_model_count,
                eligible_model_count=item.eligible_model_count,
                eligible_model_ids=item.eligible_model_ids,
                missing_tool_capabilities=item.missing_tool_capabilities,
                blockers=tuple(sorted(set(item.blockers) | {"dependency:not_eligible"})),
                next_actions=tuple(sorted(set(item.next_actions) | {"resolve_predecessor_admission_before_dispatch"})),
            )
        elif item.depends_on:
            by_id[item_id] = AutonomousWorkflowPortfolioAdmissionItem(
                item_id=item.item_id,
                domain=item.domain,
                depends_on=item.depends_on,
                dependency_statuses=statuses,
                plan_status=item.plan_status,
                status=item.status,
                readiness_state=item.readiness_state,
                workflow_digest=item.workflow_digest,
                plan_digest=item.plan_digest,
                request_digest=item.request_digest,
                required_model_capabilities=item.required_model_capabilities,
                compatible_model_count=item.compatible_model_count,
                eligible_model_count=item.eligible_model_count,
                eligible_model_ids=item.eligible_model_ids,
                missing_tool_capabilities=item.missing_tool_capabilities,
                blockers=item.blockers,
                next_actions=item.next_actions,
            )
    return tuple(sorted(by_id.values(), key=lambda item: item.item_id))


def admit_autonomous_workflow_portfolio(
    agent: "AutonomousAgent",
    requests: Sequence[Any],
    *,
    plan: AutonomousWorkflowPortfolioPlan | Mapping[str, Any] | None = None,
    verify_plan: bool = True,
    model_candidates: Sequence[Any] | None = None,
    require_available_tools: bool = False,
    require_calibrated_learning: bool = False,
    input_tokens: int = 4_096,
    output_tokens: int = 1_024,
    max_cost_per_million_tokens: float | None = None,
    max_latency_ms: float | None = None,
    min_quality: float | None = None,
    readiness_options: Mapping[str, Any] | None = None,
) -> AutonomousWorkflowPortfolioAdmission:
    """Create a metadata-only admission image for a reviewed workflow portfolio."""

    if not hasattr(agent, "models") or not hasattr(agent, "readiness"):
        raise BrainRunError("workflow portfolio admission requires an AutonomousAgent readiness surface")
    if not isinstance(verify_plan, bool) or not isinstance(require_available_tools, bool) or not isinstance(require_calibrated_learning, bool):
        raise BrainRunError("workflow portfolio admission boolean options are invalid")
    expected_plan = plan
    if expected_plan is None:
        expected_plan = plan_autonomous_workflow_portfolio(agent, requests)
    elif not isinstance(expected_plan, AutonomousWorkflowPortfolioPlan):
        expected_plan = AutonomousWorkflowPortfolioPlan.from_dict(expected_plan)
    policy = AutonomousWorkflowPortfolioAdmissionPolicy(
        require_all_domains=expected_plan.require_all_domains,
        allow_partial=expected_plan.allow_partial,
        verify_plan=verify_plan,
        require_available_tools=require_available_tools,
        require_calibrated_learning=require_calibrated_learning,
        input_tokens=_bounded_positive_int("workflow portfolio admission input_tokens", input_tokens),
        output_tokens=_bounded_positive_int("workflow portfolio admission output_tokens", output_tokens),
        max_cost_per_million_tokens=_finite_number("workflow portfolio admission max_cost_per_million_tokens", max_cost_per_million_tokens),
        max_latency_ms=_finite_number("workflow portfolio admission max_latency_ms", max_latency_ms),
        min_quality=_finite_number("workflow portfolio admission min_quality", min_quality),
    )
    if verify_plan:
        verification = verify_autonomous_workflow_portfolio(agent, expected_plan, requests)
        if verification.status != "verified":
            raise BrainRunError("workflow portfolio admission plan verification failed; re-review is required")
    if readiness_options is None:
        readiness_kwargs: dict[str, Any] = {}
    elif isinstance(readiness_options, Mapping):
        readiness_kwargs = dict(readiness_options)
    else:
        raise BrainRunError("workflow portfolio admission readiness_options must be a mapping")
    allowed_readiness = {
        "selection_promotion_report",
        "require_promoted_selection",
        "evidence_readiness",
        "calibration_report",
    }
    unknown_readiness = sorted(set(readiness_kwargs).difference(allowed_readiness))
    if unknown_readiness:
        raise BrainRunError("workflow portfolio admission readiness_options contain unsupported fields: " + ", ".join(unknown_readiness))
    readiness = agent.readiness(**readiness_kwargs)
    if not isinstance(readiness, Mapping):
        raise BrainRunError("workflow portfolio admission readiness did not return a mapping")
    readiness_digest = _safe_digest(dict(readiness), "workflow portfolio admission readiness")
    rows = _row_by_domain(readiness)
    learning_rows: dict[str, Mapping[str, Any]] = {}
    learning = readiness.get("domain_learning_coverage")
    if isinstance(learning, Mapping) and isinstance(learning.get("rows"), Sequence):
        for row in learning["rows"]:
            if isinstance(row, Mapping) and isinstance(row.get("domain"), str):
                learning_rows[row["domain"]] = row
    raw_candidates = agent.models(enabled_only=True) if model_candidates is None else model_candidates
    if isinstance(raw_candidates, (str, bytes)) or not isinstance(raw_candidates, Sequence) or len(raw_candidates) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_MODELS:
        raise BrainRunError("workflow portfolio admission model candidates are outside their bound")
    candidates = tuple(_candidate_mapping(value, index) for index, value in enumerate(raw_candidates))
    if len({candidate["model_id"] for candidate in candidates}) != len(candidates):
        raise BrainRunError("workflow portfolio admission model candidates contain duplicates")
    initial: list[AutonomousWorkflowPortfolioAdmissionItem] = []
    for plan_item in expected_plan.items:
        tool_plan = None
        if hasattr(agent, "domain_pack_tool_plan") and callable(agent.domain_pack_tool_plan):
            tool_plan = agent.domain_pack_tool_plan(plan_item.domain)
        initial.append(
            _build_item(
                plan_item,
                rows.get(plan_item.domain),
                learning_rows.get(plan_item.domain),
                candidates,
                policy,
                tool_plan,
            )
        )
    admission_items = _dependency_closure(initial, expected_plan)
    status = _admission_status(expected_plan, admission_items)
    counts = _counts(admission_items)
    next_actions = _next_actions(status, expected_plan, admission_items)
    descriptor = {
        "schema": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
        "status": status,
        "plan": expected_plan.to_dict(),
        "policy": policy.to_dict(),
        "readiness_digest": readiness_digest,
        "items": [item.to_dict() for item in admission_items],
        "dependency_graph": expected_plan.dependency_graph.to_dict(),
        "waves": [list(wave) for wave in expected_plan.dependency_graph.waves],
        "counts": counts.to_dict(),
        "next_actions": list(next_actions),
        "execution": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_EXECUTION,
        "authorization": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_AUTHORIZATION,
        "retention": AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION,
        "secret_material": _SECRET_MATERIAL,
    }
    admission = AutonomousWorkflowPortfolioAdmission(
        status=status,
        plan=expected_plan,
        policy=policy,
        readiness_digest=readiness_digest,
        items=admission_items,
        counts=counts,
        next_actions=next_actions,
        admission_digest=content_digest(descriptor),
    )
    if len(json.dumps(admission.to_dict(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")) > MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES:
        raise BrainRunError("workflow portfolio admission exceeds its byte bound")
    return admission


def validate_autonomous_workflow_portfolio_admission(value: Any) -> AutonomousWorkflowPortfolioAdmission:
    """Validate a caller-rehydrated metadata-only admission image."""

    return AutonomousWorkflowPortfolioAdmission.from_dict(value)


__all__ = [
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_EXECUTION",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_AUTHORIZATION",
    "AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_RETENTION",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_ACTIONS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BLOCKERS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_MODELS",
    "MAX_AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_BYTES",
    "AutonomousWorkflowPortfolioAdmissionPolicy",
    "AutonomousWorkflowPortfolioAdmissionItem",
    "AutonomousWorkflowPortfolioAdmissionCounts",
    "AutonomousWorkflowPortfolioAdmission",
    "admit_autonomous_workflow_portfolio",
    "validate_autonomous_workflow_portfolio_admission",
]
