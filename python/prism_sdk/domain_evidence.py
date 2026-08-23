"""Typed models for cross-domain evidence harmonization.

The harmonizer joins exact domain-report projections into a traceability artifact.  These models
keep the caller's link roles and review posture visible; they do not turn a support link into a
scientific, clinical, causal, publication, release, or readiness conclusion.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .artifacts import _digest, _mapping, _text
from .errors import ArgumentError

DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA = "bioprism-devplat-domain-evidence-harmonization/0.1"
DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW = "domain_evidence_harmonize"
DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_SCHEMA = "bioprism-devplat-domain-evidence-harmonization-coverage/0.1"
DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_WORKFLOW = "domain_evidence_harmonization_coverage"
DOMAIN_EVIDENCE_LINK_ROLES = ("supports", "qualifies", "contradicts", "context")
DOMAIN_EVIDENCE_HARMONIZATION_TRACEABILITY_STATES = (
    "complete",
    "requirements_missing",
    "links_missing",
)


def _bounded_texts(name: str, value: Any, maximum: int = 64) -> tuple[str, ...]:
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


@dataclass(frozen=True)
class DomainEvidenceLink:
    """One explicit caller assertion about a report's role in the join."""

    report_index: int
    role: str
    note: str = ""
    report_digest: str | None = None

    def __post_init__(self) -> None:
        if isinstance(self.report_index, bool) or not isinstance(self.report_index, int) or self.report_index < 0:
            raise ArgumentError("domain evidence report_index must be a non-negative integer")
        if self.role not in DOMAIN_EVIDENCE_LINK_ROLES:
            raise ArgumentError(
                "domain evidence role must be one of " + ", ".join(DOMAIN_EVIDENCE_LINK_ROLES)
            )
        if not isinstance(self.note, str) or len(self.note.encode("utf-8")) > 512:
            raise ArgumentError("domain evidence note must be a string of at most 512 UTF-8 bytes")
        if self.role in ("qualifies", "contradicts") and not self.note.strip():
            raise ArgumentError(f"domain evidence {self.role} links require a note")
        if self.report_digest is not None:
            _digest("domain evidence report digest", self.report_digest)

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {"report_index": self.report_index, "role": self.role}
        if self.note:
            result["note"] = self.note
        if self.report_digest is not None:
            result["report_digest"] = self.report_digest
        return result


@dataclass(frozen=True)
class DomainEvidenceHarmonizeRequest:
    """Bounded request for joining canonical domain-report bodies or projection wrappers."""

    subject_id: str
    claim: Mapping[str, Any]
    reports: tuple[Mapping[str, Any], ...]
    links: tuple[Mapping[str, Any] | DomainEvidenceLink, ...]
    required_group_ids: tuple[str, ...] = ()
    required_domains: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("domain evidence subject_id", self.subject_id)
        if not isinstance(self.claim, Mapping) or not self.claim.get("id"):
            raise ArgumentError("domain evidence claim must be an object with a non-empty id")
        _text("domain evidence claim id", self.claim.get("id"))
        if len(self.reports) < 1 or len(self.reports) > 64:
            raise ArgumentError("domain evidence reports must contain between 1 and 64 objects")
        for report in self.reports:
            if not isinstance(report, Mapping):
                raise ArgumentError("domain evidence reports must contain only objects")
        if len(self.links) < 1 or len(self.links) > 256:
            raise ArgumentError("domain evidence links must contain between 1 and 256 objects")
        for link in self.links:
            if isinstance(link, DomainEvidenceLink):
                continue
            if not isinstance(link, Mapping):
                raise ArgumentError("domain evidence links must contain only objects")
            DomainEvidenceLink(
                report_index=link.get("report_index"),
                role=link.get("role"),
                note=link.get("note", ""),
                report_digest=link.get("report_digest"),
            )
        _bounded_texts("domain evidence required_group_ids", self.required_group_ids)
        _bounded_texts("domain evidence required_domains", self.required_domains)

    def to_arguments(self) -> dict[str, Any]:
        links = [link.to_dict() if isinstance(link, DomainEvidenceLink) else dict(link) for link in self.links]
        return {
            "subject_id": self.subject_id,
            "claim": dict(self.claim),
            "reports": [dict(report) for report in self.reports],
            "links": links,
            "required_group_ids": list(self.required_group_ids),
            "required_domains": list(self.required_domains),
        }


@dataclass(frozen=True)
class DomainEvidenceHarmonizationReport:
    """Typed response for a traceability-only harmonization operation."""

    raw: dict[str, Any]
    harmonization: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str
    harmonization_digest: str
    traceability_state: str
    contradiction_declared: bool
    bridge_summary: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceHarmonizationReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_HARMONIZATION_WORKFLOW:
            raise ArgumentError("domain evidence harmonization workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA:
            raise ArgumentError("domain evidence harmonization schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain evidence harmonization must not claim readiness")
        if raw.get("execution") != "not_started":
            raise ArgumentError("domain evidence harmonization execution must be not_started")
        harmonization = _mapping("domain evidence harmonization", raw.get("harmonization"))
        artifact_registry = _mapping("domain evidence artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence artifact registry projection is not indexed")
        coverage = _mapping("domain evidence coverage", harmonization.get("coverage"))
        posture = _mapping("domain evidence posture", harmonization.get("posture"))
        bridge_summary = _mapping(
            "domain evidence bridge summary", coverage.get("bridge_summary", {})
        )
        state = _text("domain evidence traceability state", coverage.get("traceability_state"))
        contradiction = posture.get("explicit_contradiction_declared")
        if not isinstance(contradiction, bool):
            raise ArgumentError("domain evidence contradiction posture must be a boolean")
        return cls(
            raw=raw,
            harmonization=harmonization,
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence catalogue digest", raw.get("catalogue_digest")),
            harmonization_digest=_digest(
                "domain evidence harmonization digest", harmonization.get("harmonization_digest")
            ),
            traceability_state=state,
            contradiction_declared=contradiction,
            bridge_summary=bridge_summary,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainEvidenceHarmonizationCoverageRequest:
    """Bounded cursor query over retained harmonization artifacts."""

    subject_id: str | None = None
    domain: str | None = None
    report_class: str | None = None
    bridge_mode: str | None = None
    traceability_state: str | None = None
    after: str | None = None
    max_items: int = 100
    include_report_digests: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("subject_id", self.subject_id),
            ("domain", self.domain),
            ("report_class", self.report_class),
            ("bridge_mode", self.bridge_mode),
        ):
            if value is not None:
                _text(f"domain evidence harmonization coverage {name}", value)
        if self.traceability_state is not None:
            state = _text(
                "domain evidence harmonization coverage traceability_state",
                self.traceability_state,
            )
            if state not in DOMAIN_EVIDENCE_HARMONIZATION_TRACEABILITY_STATES:
                raise ArgumentError("domain evidence harmonization coverage traceability_state is invalid")
        if self.after is not None:
            _digest("domain evidence harmonization coverage after", self.after)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("domain evidence harmonization coverage max_items must be between 1 and 256")
        if not isinstance(self.include_report_digests, bool):
            raise ArgumentError("domain evidence harmonization coverage include_report_digests must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params: dict[str, str] = {
            "max_items": str(self.max_items),
            "include_report_digests": str(self.include_report_digests).lower(),
        }
        for name in ("subject_id", "domain", "report_class", "bridge_mode", "traceability_state", "after"):
            value = getattr(self, name)
            if value is not None:
                params[name] = value
        return params

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "max_items": self.max_items,
            "include_report_digests": self.include_report_digests,
        }
        for name in ("subject_id", "domain", "report_class", "bridge_mode", "traceability_state", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class DomainEvidenceHarmonizationCoverageReport:
    """Typed retained coverage rows; summary fields remain forward-compatible mappings."""

    raw: dict[str, Any]
    matching_count: int
    returned_count: int
    has_more: bool
    next_after: str | None
    rows: tuple[Mapping[str, Any], ...]
    summary: Mapping[str, Any]
    coverage_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceHarmonizationCoverageReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_WORKFLOW:
            raise ArgumentError("domain evidence harmonization coverage workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_HARMONIZATION_COVERAGE_SCHEMA:
            raise ArgumentError("domain evidence harmonization coverage schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain evidence harmonization coverage must not claim readiness")
        if raw.get("execution") != "not_started":
            raise ArgumentError("domain evidence harmonization coverage execution must be not_started")
        matching_count = _coverage_count("matching_count", raw.get("matching_count"))
        returned_count = _coverage_count("returned_count", raw.get("returned_count"))
        has_more = raw.get("has_more")
        if not isinstance(has_more, bool):
            raise ArgumentError("domain evidence harmonization coverage has_more must be a boolean")
        next_after = raw.get("next_after")
        if next_after is not None:
            next_after = _digest("domain evidence harmonization coverage next_after", next_after)
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("domain evidence harmonization coverage rows must be an array")
        summary = raw.get("summary", {})
        if not isinstance(summary, Mapping):
            raise ArgumentError("domain evidence harmonization coverage summary must be an object")
        return cls(
            raw=raw,
            matching_count=matching_count,
            returned_count=returned_count,
            has_more=has_more,
            next_after=next_after,
            rows=tuple(_mapping("domain evidence harmonization coverage row", row) for row in rows),
            summary=summary,
            coverage_digest=_digest("domain evidence harmonization coverage digest", raw.get("coverage_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _coverage_count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"domain evidence harmonization coverage {name} must be a non-negative integer")
    return value
