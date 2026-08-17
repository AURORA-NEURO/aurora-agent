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
