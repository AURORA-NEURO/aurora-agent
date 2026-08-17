"""Typed models for cross-domain report projection and coverage diagnostics."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .artifacts import _digest, _mapping, _text
from .errors import ArgumentError

DOMAIN_REPORT_SCHEMA = "bioprism-devplat-domain-report/0.1"
DOMAIN_REPORT_PROJECT_SCHEMA = "bioprism-devplat-domain-report-project/0.1"
DOMAIN_REPORT_COVERAGE_SCHEMA = "bioprism-devplat-domain-report-coverage/0.1"
DOMAIN_REPORT_CLAIM_STATUSES = (
    "observed",
    "derived",
    "review_required",
    "refused",
    "not_applicable",
)


def _bounded_text_list(name: str, value: Any, *, required: bool = False, maximum: int = 64) -> tuple[str, ...]:
    if value is None and not required:
        return ()
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array of strings")
    if len(value) > maximum:
        raise ArgumentError(f"{name} must contain at most {maximum} strings")
    result = tuple(_text(name, item) for item in value)
    if required and not result:
        raise ArgumentError(f"{name} must contain at least one string")
    if len(set(result)) != len(result):
        raise ArgumentError(f"{name} must not contain duplicate strings")
    return result


@dataclass(frozen=True)
class DomainReportProjectRequest:
    """One explicit report projection request."""

    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    report: Mapping[str, Any]
    claim_posture: Mapping[str, Any]
    parent_digests: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        _text("domain report group_id", self.group_id)
        domains = _bounded_text_list("domain report domains", self.domains, required=True)
        if len(domains) > 64:
            raise ArgumentError("domain report domains must contain at most 64 strings")
        _text("domain report subject_id", self.subject_id)
        _text("domain report source_tool", self.source_tool)
        if not isinstance(self.report, Mapping):
            raise ArgumentError("domain report report must be an object")
        if not isinstance(self.claim_posture, Mapping):
            raise ArgumentError("domain report claim_posture must be an object")
        status = self.claim_posture.get("status")
        if status not in DOMAIN_REPORT_CLAIM_STATUSES:
            raise ArgumentError(
                "domain report claim_posture.status must be one of "
                + ", ".join(DOMAIN_REPORT_CLAIM_STATUSES)
            )
        _bounded_text_list(
            "domain report claim_posture.does_not_claim",
            self.claim_posture.get("does_not_claim"),
            required=True,
        )
        _bounded_text_list(
            "domain report claim_posture.limitations",
            self.claim_posture.get("limitations"),
        )
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain report parent_digests must contain at most 128 values")
        for value in self.parent_digests:
            _digest("domain report parent digest", value)

    def to_arguments(self) -> dict[str, Any]:
        return {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "source_tool": self.source_tool,
            "report": dict(self.report),
            "claim_posture": dict(self.claim_posture),
            "parent_digests": list(self.parent_digests),
        }


@dataclass(frozen=True)
class DomainReportProjectReport:
    raw: dict[str, Any]
    report: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    coverage: Mapping[str, Any]
    content_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainReportProjectReport":
        raw = dict(value)
        if raw.get("workflow") != "domain_report_project":
            raise ArgumentError("domain report project workflow is invalid")
        if raw.get("schema") != DOMAIN_REPORT_PROJECT_SCHEMA:
            raise ArgumentError("domain report project schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain report project must not claim readiness")
        artifact_registry = _mapping("domain report artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain report project artifact registry projection is not indexed")
        return cls(
            raw=raw,
            report=_mapping("domain report payload", raw.get("report")),
            artifact_registry=artifact_registry,
            coverage=_mapping("domain report coverage", raw.get("coverage")),
            content_digest=_digest(
                "domain report content digest", artifact_registry.get("content_digest")
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainReportCoverageRequest:
    group_id: str | None = None
    domain: str | None = None
    max_groups: int = 64
    include_report_digests: bool = False

    def __post_init__(self) -> None:
        if self.group_id is not None:
            _text("domain report coverage group_id", self.group_id)
        if self.domain is not None:
            _text("domain report coverage domain", self.domain)
        if isinstance(self.max_groups, bool) or not isinstance(self.max_groups, int) or not 1 <= self.max_groups <= 128:
            raise ArgumentError("domain report coverage max_groups must be between 1 and 128")
        if not isinstance(self.include_report_digests, bool):
            raise ArgumentError("domain report coverage include_report_digests must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params = {
            "max_groups": str(self.max_groups),
            "include_report_digests": str(self.include_report_digests).lower(),
        }
        if self.group_id is not None:
            params["group_id"] = self.group_id
        if self.domain is not None:
            params["domain"] = self.domain
        return params

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"operation": "coverage", **self.to_query_params()}
        result["max_groups"] = self.max_groups
        result["include_report_digests"] = self.include_report_digests
        return result


@dataclass(frozen=True)
class DomainReportCoverageReport:
    raw: dict[str, Any]
    complete: bool
    group_count: int
    reported_group_count: int
    missing_group_count: int
    missing_group_ids: tuple[str, ...]
    groups: tuple[Mapping[str, Any], ...]
    catalogue_digest: str
    coverage_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainReportCoverageReport":
        raw = dict(value)
        if raw.get("workflow") != "domain_report_coverage":
            raise ArgumentError("domain report coverage workflow is invalid")
        if raw.get("schema") != DOMAIN_REPORT_COVERAGE_SCHEMA:
            raise ArgumentError("domain report coverage schema is invalid")
        complete = raw.get("complete")
        if not isinstance(complete, bool):
            raise ArgumentError("domain report coverage complete must be a boolean")
        missing = _bounded_text_list("domain report missing_group_ids", raw.get("missing_group_ids"))
        groups = raw.get("groups", [])
        if not isinstance(groups, Sequence) or isinstance(groups, (str, bytes)):
            raise ArgumentError("domain report coverage groups must be an array")
        return cls(
            raw=raw,
            complete=complete,
            group_count=_count("domain report group_count", raw.get("group_count")),
            reported_group_count=_count("domain report reported_group_count", raw.get("reported_group_count")),
            missing_group_count=_count("domain report missing_group_count", raw.get("missing_group_count")),
            missing_group_ids=missing,
            groups=tuple(_mapping("domain report coverage group", item) for item in groups),
            catalogue_digest=_digest("domain report catalogue digest", raw.get("catalogue_digest")),
            coverage_digest=_digest("domain report coverage digest", raw.get("coverage_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value
