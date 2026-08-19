"""Typed client contracts for the cross-domain control-plane evidence join.

The server composes independently retained domain, route, operations, release, and workflow
packets. These models preserve the distinction between structural completeness and authority:
``ready_for_human_review`` is never an execution, deployment, scientific, clinical, or release
approval.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping

from .artifacts import _count, _digest, _mapping, _text
from .capability import _tool_payload
from .errors import ArgumentError

CONTROL_PLANE_READINESS_SCHEMA = "bioprism-control-plane-readiness/0.1"
CONTROL_PLANE_READINESS_WORKFLOW = "control_plane_readiness_audit"
CONTROL_PLANE_READINESS_STATES = (
    "ready_for_human_review",
    "review_required",
    "incomplete",
    "blocked",
)
CONTROL_PLANE_READINESS_QUERY_SCHEMA = "bioprism-devplat-artifact-control-plane-readiness-query/0.1"
CONTROL_PLANE_READINESS_COMPARE_SCHEMA = "bioprism-control-plane-readiness-compare/0.1"
CONTROL_PLANE_READINESS_RETAINED_COMPARE_SCHEMA = "bioprism-control-plane-readiness-compare-retained/0.1"


@dataclass(frozen=True)
class ControlPlaneReadinessRequest:
    """Caller-owned policy and explicitly supplied evidence packets."""

    subject_id: str
    policy: Mapping[str, Any] = field(default_factory=dict)
    readiness_audit: Mapping[str, Any] | None = None
    route_review: Mapping[str, Any] | None = None
    route_plan: Mapping[str, Any] | None = None
    operations_gate_projection: Mapping[str, Any] | None = None
    operations_gate_review: Mapping[str, Any] | None = None
    release_audit: Mapping[str, Any] | None = None
    workflow_evidence: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _text("control-plane readiness subject_id", self.subject_id)
        if not isinstance(self.policy, Mapping):
            raise ArgumentError("control-plane readiness policy must be an object")
        for name in (
            "require_domain_readiness",
            "require_route_review",
            "require_route_plan",
            "require_operations_acceptance",
            "require_release_ready",
            "require_workflow_evidence",
        ):
            value = self.policy.get(name)
            if value is not None and not isinstance(value, bool):
                raise ArgumentError(f"control-plane readiness policy {name} must be a boolean")
        for name in (
            "readiness_audit",
            "route_review",
            "route_plan",
            "operations_gate_projection",
            "operations_gate_review",
            "release_audit",
            "workflow_evidence",
        ):
            value = getattr(self, name)
            if value is not None and not isinstance(value, Mapping):
                raise ArgumentError(f"control-plane readiness {name} must be an object")

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"subject_id": self.subject_id, "policy": dict(self.policy)}
        for name in (
            "readiness_audit",
            "route_review",
            "route_plan",
            "operations_gate_projection",
            "operations_gate_review",
            "release_audit",
            "workflow_evidence",
        ):
            value = getattr(self, name)
            if value is not None:
                result[name] = dict(value)
        return result


@dataclass(frozen=True)
class ControlPlaneReadinessReport:
    """Typed structural projection with component-level evidence preserved."""

    raw: dict[str, Any]
    audit: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    subject_id: str
    digest: str
    control_plane_state: str
    policy_satisfied: bool
    components: Mapping[str, Any]
    blockers: tuple[Mapping[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ControlPlaneReadinessReport":
        raw = _tool_payload(value, CONTROL_PLANE_READINESS_WORKFLOW)
        if raw.get("schema") != CONTROL_PLANE_READINESS_SCHEMA:
            raise ArgumentError("control-plane readiness schema is invalid")
        if raw.get("readiness_claimed") is not False or raw.get("execution") != "not_started":
            raise ArgumentError("control-plane readiness must remain non-executing and non-claiming")
        audit = _mapping("control-plane readiness audit", raw.get("audit"))
        registry = _mapping("control-plane readiness artifact registry", raw.get("artifact_registry"))
        if registry.get("indexed") is not True:
            raise ArgumentError("control-plane readiness artifact registry projection is not indexed")
        state = _text("control-plane readiness state", audit.get("control_plane_state"))
        if state not in CONTROL_PLANE_READINESS_STATES:
            raise ArgumentError("control-plane readiness state is invalid")
        policy_satisfied = audit.get("policy_satisfied")
        if not isinstance(policy_satisfied, bool) or policy_satisfied != (state == "ready_for_human_review"):
            raise ArgumentError("control-plane readiness policy_satisfied is inconsistent")
        components = _mapping("control-plane readiness components", audit.get("components"))
        blockers = audit.get("blockers", [])
        if not isinstance(blockers, (list, tuple)):
            raise ArgumentError("control-plane readiness blockers must be an array")
        return cls(
            raw=raw,
            audit=audit,
            artifact_registry=registry,
            subject_id=_text("control-plane readiness subject_id", audit.get("subject_id")),
            digest=_digest("control-plane readiness digest", audit.get("digest")),
            control_plane_state=state,
            policy_satisfied=policy_satisfied,
            components=components,
            blockers=tuple(_mapping("control-plane readiness blocker", item) for item in blockers),
        )

    @property
    def ready_for_human_review(self) -> bool:
        return self.control_plane_state == "ready_for_human_review"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def control_plane_readiness_report(value: Mapping[str, Any]) -> ControlPlaneReadinessReport:
    """Parse a direct MCP result or an HTTP REST tool envelope."""

    return ControlPlaneReadinessReport.from_wire(value)


@dataclass(frozen=True)
class ControlPlaneReadinessCompareRequest:
    """Two complete control-plane snapshots for a digest-verified structural diff."""

    before: Mapping[str, Any]
    after: Mapping[str, Any]
    subject_id: str | None = None

    def __post_init__(self) -> None:
        for name, value in (("before", self.before), ("after", self.after)):
            if not isinstance(value, Mapping):
                raise ArgumentError(f"control-plane readiness comparison {name} must be an object")
        if self.subject_id is not None:
            _text("control-plane readiness comparison subject_id", self.subject_id)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "before": dict(self.before),
            "after": dict(self.after),
        }
        if self.subject_id is not None:
            result["subject_id"] = self.subject_id
        return result


@dataclass(frozen=True)
class ControlPlaneReadinessCompareReport:
    """Typed structural change report; no external authority is inferred."""

    raw: dict[str, Any]
    comparison: Mapping[str, Any]
    subject_id: str
    state_direction: str
    evidence_direction: str
    comparison_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ControlPlaneReadinessCompareReport":
        raw = _tool_payload(value, "control_plane_readiness_compare")
        if raw.get("schema") != CONTROL_PLANE_READINESS_COMPARE_SCHEMA:
            raise ArgumentError("control-plane readiness comparison schema is invalid")
        if raw.get("readiness_claimed") is not False or raw.get("execution") != "not_started":
            raise ArgumentError("control-plane readiness comparison must remain non-executing")
        comparison = _mapping("control-plane readiness comparison", raw.get("comparison"))
        state_direction = _text("control-plane readiness comparison state_direction", comparison.get("state_direction"))
        if state_direction not in {"improved", "regressed", "unchanged"}:
            raise ArgumentError("control-plane readiness comparison state_direction is invalid")
        evidence_direction = _text("control-plane readiness comparison evidence_direction", comparison.get("evidence_direction"))
        if evidence_direction not in {"improved", "regressed", "mixed", "unchanged"}:
            raise ArgumentError("control-plane readiness comparison evidence_direction is invalid")
        for name in ("component_changes", "blockers_added", "blockers_removed", "improvements", "regressions"):
            if not isinstance(comparison.get(name), (list, tuple)):
                raise ArgumentError(f"control-plane readiness comparison {name} must be an array")
        return cls(
            raw=raw,
            comparison=comparison,
            subject_id=_text("control-plane readiness comparison subject_id", comparison.get("subject_id")),
            state_direction=state_direction,
            evidence_direction=evidence_direction,
            comparison_digest=_digest("control-plane readiness comparison digest", comparison.get("comparison_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ControlPlaneReadinessRetainedCompareRequest:
    """Two retained readiness artifacts addressed by exact content digest."""

    before_content_digest: str
    after_content_digest: str
    subject_id: str | None = None

    def __post_init__(self) -> None:
        _digest("retained control-plane comparison before content digest", self.before_content_digest)
        _digest("retained control-plane comparison after content digest", self.after_content_digest)
        if self.subject_id is not None:
            _text("retained control-plane comparison subject_id", self.subject_id)

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "before_content_digest": self.before_content_digest,
            "after_content_digest": self.after_content_digest,
        }
        if self.subject_id is not None:
            result["subject_id"] = self.subject_id
        return result


@dataclass(frozen=True)
class ControlPlaneReadinessRetainedCompareReport:
    """Typed structural diff resolved from the retained artifact registry."""

    raw: dict[str, Any]
    comparison: Mapping[str, Any]
    subject_id: str
    before_content_digest: str
    after_content_digest: str
    state_direction: str
    evidence_direction: str
    comparison_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ControlPlaneReadinessRetainedCompareReport":
        raw = _tool_payload(value, "control_plane_readiness_compare_retained")
        if raw.get("schema") != CONTROL_PLANE_READINESS_RETAINED_COMPARE_SCHEMA:
            raise ArgumentError("retained control-plane comparison schema is invalid")
        if raw.get("readiness_claimed") is not False or raw.get("execution") != "not_started":
            raise ArgumentError("retained control-plane comparison must remain non-executing")
        comparison = _mapping("retained control-plane comparison", raw.get("comparison"))
        state_direction = _text("retained control-plane comparison state_direction", comparison.get("state_direction"))
        if state_direction not in {"improved", "regressed", "unchanged"}:
            raise ArgumentError("retained control-plane comparison state_direction is invalid")
        evidence_direction = _text("retained control-plane comparison evidence_direction", comparison.get("evidence_direction"))
        if evidence_direction not in {"improved", "regressed", "mixed", "unchanged"}:
            raise ArgumentError("retained control-plane comparison evidence_direction is invalid")
        return cls(
            raw=raw,
            comparison=comparison,
            subject_id=_text("retained control-plane comparison subject_id", raw.get("subject_id")),
            before_content_digest=_digest(
                "retained control-plane comparison before content digest",
                raw.get("before_content_digest"),
            ),
            after_content_digest=_digest(
                "retained control-plane comparison after content digest",
                raw.get("after_content_digest"),
            ),
            state_direction=state_direction,
            evidence_direction=evidence_direction,
            comparison_digest=_digest(
                "retained control-plane comparison digest",
                comparison.get("comparison_digest"),
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ControlPlaneReadinessQueryRequest:
    """Bounded lookup over retained control-plane projections."""

    subject_id: str | None = None
    control_plane_state: str | None = None
    policy_satisfied: bool | None = None
    after: str | None = None
    max_items: int = 100
    include_audits: bool = False

    def __post_init__(self) -> None:
        for name, value in (("subject_id", self.subject_id), ("control_plane_state", self.control_plane_state), ("after", self.after)):
            if value is not None:
                _text(f"control-plane readiness query {name}", value)
        if self.control_plane_state is not None and self.control_plane_state not in CONTROL_PLANE_READINESS_STATES:
            raise ArgumentError("control-plane readiness query state is invalid")
        if self.policy_satisfied is not None and not isinstance(self.policy_satisfied, bool):
            raise ArgumentError("control-plane readiness query policy_satisfied must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("control-plane readiness query max_items must be between 1 and 256")
        if not isinstance(self.include_audits, bool):
            raise ArgumentError("control-plane readiness query include_audits must be a boolean")

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"max_items": self.max_items, "include_audits": self.include_audits}
        for name in ("subject_id", "control_plane_state", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        if self.policy_satisfied is not None:
            result["policy_satisfied"] = self.policy_satisfied
        return result

    def to_query_params(self) -> dict[str, str]:
        result = {"limit": str(self.max_items), "include_audits": str(self.include_audits).lower()}
        for name in ("subject_id", "control_plane_state", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        if self.policy_satisfied is not None:
            result["policy_satisfied"] = str(self.policy_satisfied).lower()
        return result


@dataclass(frozen=True)
class ControlPlaneReadinessQueryReport:
    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ControlPlaneReadinessQueryReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_control_plane_readiness_query":
            raise ArgumentError("control-plane readiness query workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, (list, tuple)):
            raise ArgumentError("control-plane readiness query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _digest("control-plane readiness query next cursor", next_after)
        if not isinstance(raw.get("has_more"), bool):
            raise ArgumentError("control-plane readiness query has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_mapping("control-plane readiness query row", row) for row in rows),
            next_after=next_after,
            has_more=raw["has_more"],
            registry_generation=_count("control-plane readiness query generation", raw.get("registry_generation")),
            registry_size=_count("control-plane readiness query size", raw.get("registry_size")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


__all__ = [
    "CONTROL_PLANE_READINESS_SCHEMA",
    "CONTROL_PLANE_READINESS_WORKFLOW",
    "CONTROL_PLANE_READINESS_STATES",
    "CONTROL_PLANE_READINESS_QUERY_SCHEMA",
    "CONTROL_PLANE_READINESS_COMPARE_SCHEMA",
    "ControlPlaneReadinessRequest",
    "ControlPlaneReadinessReport",
    "ControlPlaneReadinessCompareRequest",
    "ControlPlaneReadinessCompareReport",
    "CONTROL_PLANE_READINESS_RETAINED_COMPARE_SCHEMA",
    "ControlPlaneReadinessRetainedCompareRequest",
    "ControlPlaneReadinessRetainedCompareReport",
    "ControlPlaneReadinessQueryRequest",
    "ControlPlaneReadinessQueryReport",
    "control_plane_readiness_report",
]
