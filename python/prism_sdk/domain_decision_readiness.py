"""Typed cross-domain structural readiness policy.

The server evaluates only the caller's explicit coverage, link-role, contradiction, refusal,
review, support, qualification, and lineage policy.  ``ready_for_human_review`` is deliberately
not a scientific, clinical, release, or execution conclusion.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .artifacts import _count, _digest, _mapping, _text
from .capability import _tool_payload
from .errors import ArgumentError

DOMAIN_DECISION_READINESS_SCHEMA = "bioprism-devplat-domain-decision-readiness/0.1"
DOMAIN_DECISION_READINESS_WORKFLOW = "domain_decision_readiness_audit"
DOMAIN_DECISION_READINESS_STATES = (
    "ready_for_human_review",
    "review_required",
    "incomplete",
    "blocked",
)
MAX_DOMAIN_DECISION_READINESS_REPORTS = 64
MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS = 64
DOMAIN_DECISION_READINESS_QUERY_SCHEMA = "bioprism-devplat-artifact-domain-decision-readiness-query/0.1"


def _bounded_texts(name: str, value: Any, maximum: int = MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS) -> tuple[str, ...]:
    if value is None:
        return ()
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array of strings")
    if len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} strings")
    result = tuple(_text(name, item) for item in value)
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicate strings")
    return result


def _policy_bool(policy: Mapping[str, Any], name: str, default: bool) -> bool:
    value = policy.get(name, default)
    if not isinstance(value, bool):
        raise ArgumentError(f"domain decision-readiness policy {name} must be a boolean")
    return value


def _policy_int(policy: Mapping[str, Any], name: str, minimum: int, default: int) -> int:
    value = policy.get(name, default)
    if isinstance(value, bool) or not isinstance(value, int) or not minimum <= value <= MAX_DOMAIN_DECISION_READINESS_REPORTS:
        raise ArgumentError(
            f"domain decision-readiness policy {name} must be between {minimum} and {MAX_DOMAIN_DECISION_READINESS_REPORTS}"
        )
    return value


@dataclass(frozen=True)
class DomainDecisionReadinessRequest:
    """Caller-owned structural policy over canonical cross-domain reports."""

    subject_id: str
    claim: Mapping[str, Any]
    reports: tuple[Mapping[str, Any], ...]
    links: tuple[Mapping[str, Any], ...]
    policy: Mapping[str, Any]

    def __post_init__(self) -> None:
        _text("domain decision-readiness subject_id", self.subject_id)
        if not isinstance(self.claim, Mapping) or not self.claim.get("id"):
            raise ArgumentError("domain decision-readiness claim must be an object with a non-empty id")
        _text("domain decision-readiness claim id", self.claim.get("id"))
        if not 1 <= len(self.reports) <= MAX_DOMAIN_DECISION_READINESS_REPORTS:
            raise ArgumentError("domain decision-readiness reports must contain between 1 and 64 objects")
        if any(not isinstance(report, Mapping) for report in self.reports):
            raise ArgumentError("domain decision-readiness reports must contain only objects")
        if not 1 <= len(self.links) <= 256:
            raise ArgumentError("domain decision-readiness links must contain between 1 and 256 objects")
        if any(not isinstance(link, Mapping) for link in self.links):
            raise ArgumentError("domain decision-readiness links must contain only objects")
        if not isinstance(self.policy, Mapping):
            raise ArgumentError("domain decision-readiness policy must be an object")
        _bounded_texts("domain decision-readiness policy required_group_ids", self.policy.get("required_group_ids"))
        _bounded_texts("domain decision-readiness policy required_domains", self.policy.get("required_domains"))
        _policy_int(self.policy, "minimum_supporting_reports", 1, 1)
        _policy_int(self.policy, "minimum_qualifying_reports", 0, 0)
        for name, default in (
            ("require_all_reports_linked", True),
            ("reject_contradictions", True),
            ("reject_refused_reports", True),
            ("allow_review_required", False),
            ("require_lineage_parents", False),
        ):
            _policy_bool(self.policy, name, default)

    def to_arguments(self) -> dict[str, Any]:
        return {
            "subject_id": self.subject_id,
            "claim": dict(self.claim),
            "reports": [dict(report) for report in self.reports],
            "links": [dict(link) for link in self.links],
            "policy": dict(self.policy),
        }


@dataclass(frozen=True)
class DomainDecisionReadinessReport:
    """Typed response preserving structural state and all blockers."""

    raw: dict[str, Any]
    audit: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str
    audit_digest: str
    decision_state: str
    policy_satisfied: bool
    counts: Mapping[str, Any]
    blockers: tuple[Mapping[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainDecisionReadinessReport":
        raw = _tool_payload(value, DOMAIN_DECISION_READINESS_WORKFLOW)
        if raw.get("schema") != DOMAIN_DECISION_READINESS_SCHEMA:
            raise ArgumentError("domain decision-readiness schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain decision-readiness must not claim readiness")
        if raw.get("execution") != "not_started":
            raise ArgumentError("domain decision-readiness execution must be not_started")
        audit = _mapping("domain decision-readiness audit", raw.get("audit"))
        artifact_registry = _mapping("domain decision-readiness artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain decision-readiness artifact registry projection is not indexed")
        state = _text("domain decision-readiness decision_state", audit.get("decision_state"))
        if state not in DOMAIN_DECISION_READINESS_STATES:
            raise ArgumentError("domain decision-readiness decision_state is invalid")
        policy_satisfied = audit.get("policy_satisfied")
        if not isinstance(policy_satisfied, bool) or policy_satisfied != (state == "ready_for_human_review"):
            raise ArgumentError("domain decision-readiness policy_satisfied is inconsistent")
        blockers = audit.get("blockers", [])
        if not isinstance(blockers, Sequence) or isinstance(blockers, (str, bytes)):
            raise ArgumentError("domain decision-readiness blockers must be an array")
        return cls(
            raw=raw,
            audit=audit,
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain decision-readiness catalogue digest", raw.get("catalogue_digest")),
            audit_digest=_digest("domain decision-readiness audit digest", audit.get("digest")),
            decision_state=state,
            policy_satisfied=policy_satisfied,
            counts=_mapping("domain decision-readiness counts", audit.get("counts")),
            blockers=tuple(_mapping("domain decision-readiness blocker", blocker) for blocker in blockers),
        )

    @property
    def is_ready_for_human_review(self) -> bool:
        return self.decision_state == "ready_for_human_review"

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_decision_readiness_report(value: Mapping[str, Any]) -> DomainDecisionReadinessReport:
    """Parse a direct MCP result or REST tool envelope."""

    return DomainDecisionReadinessReport.from_wire(value)


@dataclass(frozen=True)
class DomainDecisionReadinessQueryRequest:
    """Bounded lookup over retained structural readiness audits."""

    subject_id: str | None = None
    decision_state: str | None = None
    policy_satisfied: bool | None = None
    after: str | None = None
    max_items: int = 100
    include_audits: bool = False

    def __post_init__(self) -> None:
        for name, value in (("subject_id", self.subject_id), ("decision_state", self.decision_state), ("after", self.after)):
            if value is not None:
                _text(f"readiness query {name}", value)
        if self.decision_state is not None and self.decision_state not in DOMAIN_DECISION_READINESS_STATES:
            raise ArgumentError("readiness query decision_state is invalid")
        if self.policy_satisfied is not None and not isinstance(self.policy_satisfied, bool):
            raise ArgumentError("readiness query policy_satisfied must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("readiness query max_items must be between 1 and 256")
        if not isinstance(self.include_audits, bool):
            raise ArgumentError("readiness query include_audits must be a boolean")

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"max_items": self.max_items, "include_audits": self.include_audits}
        for name in ("subject_id", "decision_state", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        if self.policy_satisfied is not None:
            result["policy_satisfied"] = self.policy_satisfied
        return result

    def to_query_params(self) -> dict[str, str]:
        params = {
            "limit": str(self.max_items),
            "include_audits": str(self.include_audits).lower(),
        }
        for name in ("subject_id", "decision_state", "after"):
            value = getattr(self, name)
            if value is not None:
                params[name] = value
        if self.policy_satisfied is not None:
            params["policy_satisfied"] = str(self.policy_satisfied).lower()
        return params


@dataclass(frozen=True)
class DomainDecisionReadinessQueryReport:
    """Digest-ordered retained readiness rows; absence is not a negative scientific result."""

    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainDecisionReadinessQueryReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_domain_decision_readiness_query":
            raise ArgumentError("readiness query workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("readiness query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _digest("readiness query next cursor", next_after)
        if not isinstance(raw.get("has_more"), bool):
            raise ArgumentError("readiness query has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_mapping("readiness query row", row) for row in rows),
            next_after=next_after,
            has_more=raw["has_more"],
            registry_generation=_count("readiness query generation", raw.get("registry_generation")),
            registry_size=_count("readiness query size", raw.get("registry_size")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


__all__ = [
    "DOMAIN_DECISION_READINESS_SCHEMA",
    "DOMAIN_DECISION_READINESS_WORKFLOW",
    "DOMAIN_DECISION_READINESS_STATES",
    "MAX_DOMAIN_DECISION_READINESS_REPORTS",
    "MAX_DOMAIN_DECISION_READINESS_REQUIREMENTS",
    "DomainDecisionReadinessRequest",
    "DomainDecisionReadinessReport",
    "DOMAIN_DECISION_READINESS_QUERY_SCHEMA",
    "DomainDecisionReadinessQueryRequest",
    "DomainDecisionReadinessQueryReport",
    "domain_decision_readiness_report",
]
