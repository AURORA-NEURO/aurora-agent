"""Typed models for exact-digest raw domain evidence intake."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence
from .artifacts import _digest, _mapping, _text
from .domain_reports import DOMAIN_REPORT_CLAIM_STATUSES, _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_INTAKE_SCHEMA = "bioprism-devplat-domain-evidence-intake/0.1"
DOMAIN_EVIDENCE_INTAKE_WORKFLOW = "domain_evidence_intake"
DOMAIN_EVIDENCE_INTAKE_OUTCOMES = ("observed", "partial", "refused", "error", "unknown")
DOMAIN_EVIDENCE_INTAKE_COVERAGE_SCHEMA = "bioprism-devplat-domain-evidence-intake-coverage/0.1"
DOMAIN_EVIDENCE_INTAKE_COVERAGE_WORKFLOW = "domain_evidence_intake_coverage"

_MISSING = object()


def _json_value(name: str, value: Any) -> None:
    try:
        json.dumps(value, separators=(",", ":"), ensure_ascii=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable") from error


@dataclass(frozen=True)
class DomainEvidenceIntakeRequest:
    """One caller-supplied raw request/response envelope."""

    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    response: Any
    outcome: str
    claim_posture: Mapping[str, Any]
    request: Any = _MISSING
    parent_digests: tuple[str, ...] = ()
    source_plan_digest: str | None = None

    def __post_init__(self) -> None:
        _text("domain evidence intake group_id", self.group_id)
        _bounded_text_list("domain evidence intake domains", self.domains, required=True)
        _text("domain evidence intake subject_id", self.subject_id)
        _text("domain evidence intake source_tool", self.source_tool)
        if self.outcome not in DOMAIN_EVIDENCE_INTAKE_OUTCOMES:
            raise ArgumentError(
                "domain evidence intake outcome must be one of "
                + ", ".join(DOMAIN_EVIDENCE_INTAKE_OUTCOMES)
            )
        if not isinstance(self.claim_posture, Mapping):
            raise ArgumentError("domain evidence intake claim_posture must be an object")
        if self.claim_posture.get("status") not in DOMAIN_REPORT_CLAIM_STATUSES:
            raise ArgumentError("domain evidence intake claim_posture.status is invalid")
        _bounded_text_list(
            "domain evidence intake claim_posture.does_not_claim",
            self.claim_posture.get("does_not_claim"),
            required=True,
        )
        _bounded_text_list(
            "domain evidence intake claim_posture.limitations",
            self.claim_posture.get("limitations"),
        )
        _json_value("domain evidence intake response", self.response)
        if self.request is not _MISSING:
            _json_value("domain evidence intake request", self.request)
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence intake parent_digests must contain at most 128 values")
        for digest in self.parent_digests:
            _digest("domain evidence intake parent digest", digest)
        if self.source_plan_digest is not None:
            _digest("domain evidence intake source plan digest", self.source_plan_digest)

    @property
    def request_supplied(self) -> bool:
        return self.request is not _MISSING

    def to_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "source_tool": self.source_tool,
            "response": self.response,
            "outcome": self.outcome,
            "claim_posture": dict(self.claim_posture),
            "parent_digests": list(self.parent_digests),
        }
        if self.source_plan_digest is not None:
            result["source_plan_digest"] = self.source_plan_digest
        if self.request is not _MISSING:
            result["request"] = self.request
        return result


@dataclass(frozen=True)
class DomainEvidenceIntakeReport:
    raw: dict[str, Any]
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    outcome: str
    source_plan_digest: str | None
    request_supplied: bool
    request_digest: str
    response_digest: str
    intake_digest: str
    report: Mapping[str, Any]
    intake: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceIntakeReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_INTAKE_WORKFLOW:
            raise ArgumentError("domain evidence intake workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_INTAKE_SCHEMA:
            raise ArgumentError("domain evidence intake schema is invalid")
        if raw.get("readiness_claimed") is not False:
            raise ArgumentError("domain evidence intake must not claim readiness")
        if raw.get("execution") != "not_started":
            raise ArgumentError("domain evidence intake execution must be not_started")
        request_supplied = raw.get("request_supplied")
        if not isinstance(request_supplied, bool):
            raise ArgumentError("domain evidence intake request_supplied must be a boolean")
        artifact_registry = _mapping("domain evidence intake artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence intake artifact registry projection is not indexed")
        domains = raw.get("domains")
        if not isinstance(domains, Sequence) or isinstance(domains, (str, bytes)):
            raise ArgumentError("domain evidence intake domains must be an array")
        intake = _mapping("domain evidence intake normalized envelope", raw.get("intake"))
        outcome = _text("domain evidence intake outcome", raw.get("outcome"))
        if outcome not in DOMAIN_EVIDENCE_INTAKE_OUTCOMES:
            raise ArgumentError("domain evidence intake outcome is invalid")
        return cls(
            raw=raw,
            group_id=_text("domain evidence intake group_id", raw.get("group_id")),
            domains=tuple(_text("domain evidence intake domain", domain) for domain in domains),
            subject_id=_text("domain evidence intake subject_id", raw.get("subject_id")),
            source_tool=_text("domain evidence intake source_tool", raw.get("source_tool")),
            outcome=outcome,
            source_plan_digest=(
                None
                if raw.get("source_plan_digest") is None
                else _digest("domain evidence intake source plan digest", raw.get("source_plan_digest"))
            ),
            request_supplied=request_supplied,
            request_digest=_digest("domain evidence intake request digest", raw.get("request_digest")),
            response_digest=_digest("domain evidence intake response digest", raw.get("response_digest")),
            intake_digest=_digest("domain evidence intake digest", raw.get("intake_digest")),
            report=_mapping("domain evidence intake report", raw.get("report")),
            intake=intake,
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence intake catalogue digest", raw.get("catalogue_digest")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainEvidenceIntakeCoverageRequest:
    group_id: str | None = None
    domain: str | None = None
    max_groups: int = 64
    include_intake_digests: bool = False

    def __post_init__(self) -> None:
        if self.group_id is not None:
            _text("domain evidence intake coverage group_id", self.group_id)
        if self.domain is not None:
            _text("domain evidence intake coverage domain", self.domain)
        if isinstance(self.max_groups, bool) or not isinstance(self.max_groups, int) or not 1 <= self.max_groups <= 128:
            raise ArgumentError("domain evidence intake coverage max_groups must be between 1 and 128")
        if not isinstance(self.include_intake_digests, bool):
            raise ArgumentError("domain evidence intake coverage include_intake_digests must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params = {
            "max_groups": str(self.max_groups),
            "include_intake_digests": str(self.include_intake_digests).lower(),
        }
        if self.group_id is not None:
            params["group_id"] = self.group_id
        if self.domain is not None:
            params["domain"] = self.domain
        return params

    def to_arguments(self) -> dict[str, Any]:
        return {
            "max_groups": self.max_groups,
            "include_intake_digests": self.include_intake_digests,
            **({"group_id": self.group_id} if self.group_id is not None else {}),
            **({"domain": self.domain} if self.domain is not None else {}),
        }


@dataclass(frozen=True)
class DomainEvidenceIntakeCoverageReport:
    raw: dict[str, Any]
    complete: bool
    group_count: int
    reported_group_count: int
    missing_group_count: int
    missing_group_ids: tuple[str, ...]
    tool_coverage_complete: bool
    missing_tool_group_ids: tuple[str, ...]
    domain_coverage_complete: bool
    missing_domain_group_ids: tuple[str, ...]
    groups: tuple[Mapping[str, Any], ...]
    catalogue_digest: str
    coverage_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceIntakeCoverageReport":
        raw = dict(value)
        if raw.get("workflow") != DOMAIN_EVIDENCE_INTAKE_COVERAGE_WORKFLOW:
            raise ArgumentError("domain evidence intake coverage workflow is invalid")
        if raw.get("schema") != DOMAIN_EVIDENCE_INTAKE_COVERAGE_SCHEMA:
            raise ArgumentError("domain evidence intake coverage schema is invalid")
        complete = raw.get("complete")
        if not isinstance(complete, bool):
            raise ArgumentError("domain evidence intake coverage complete must be a boolean")
        tool_coverage_complete = raw.get("tool_coverage_complete")
        if not isinstance(tool_coverage_complete, bool):
            raise ArgumentError("domain evidence intake tool_coverage_complete must be a boolean")
        domain_coverage_complete = raw.get("domain_coverage_complete")
        if not isinstance(domain_coverage_complete, bool):
            raise ArgumentError("domain evidence intake domain_coverage_complete must be a boolean")
        missing = _bounded_text_list(
            "domain evidence intake missing_group_ids", raw.get("missing_group_ids")
        )
        groups = raw.get("groups", [])
        if not isinstance(groups, Sequence) or isinstance(groups, (str, bytes)):
            raise ArgumentError("domain evidence intake coverage groups must be an array")
        return cls(
            raw=raw,
            complete=complete,
            group_count=_count("domain evidence intake group_count", raw.get("group_count")),
            reported_group_count=_count(
                "domain evidence intake reported_group_count", raw.get("reported_group_count")
            ),
            missing_group_count=_count(
                "domain evidence intake missing_group_count", raw.get("missing_group_count")
            ),
            missing_group_ids=missing,
            tool_coverage_complete=tool_coverage_complete,
            missing_tool_group_ids=_bounded_text_list(
                "domain evidence intake missing_tool_group_ids", raw.get("missing_tool_group_ids")
            ),
            domain_coverage_complete=domain_coverage_complete,
            missing_domain_group_ids=_bounded_text_list(
                "domain evidence intake missing_domain_group_ids", raw.get("missing_domain_group_ids")
            ),
            groups=tuple(_mapping("domain evidence intake coverage group", group) for group in groups),
            catalogue_digest=_digest(
                "domain evidence intake catalogue digest", raw.get("catalogue_digest")
            ),
            coverage_digest=_digest(
                "domain evidence intake coverage digest", raw.get("coverage_digest")
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value
