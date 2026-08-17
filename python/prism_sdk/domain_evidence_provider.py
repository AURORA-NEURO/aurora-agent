"""Typed caller-managed provider evidence normalization models."""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .artifacts import _digest, _mapping, _text
from .capability import _route_count, _route_strings, _route_text, _tool_payload
from .domain_reports import DOMAIN_REPORT_CLAIM_STATUSES, _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA = "bioprism-devplat-domain-evidence-provider-normalization/0.1"
DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW = "domain_evidence_provider_normalize"
DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS = ("literature", "clinical_trial", "fhir", "object_store", "provider_api")
DOMAIN_EVIDENCE_PROVIDER_OUTCOMES = ("observed", "partial", "refused", "error", "unknown")
_MISSING = object()


def _json_value(name: str, value: Any) -> None:
    try:
        json.dumps(value, separators=(",", ":"), ensure_ascii=False)
    except (TypeError, ValueError) as error:
        raise ArgumentError(f"{name} must be JSON serializable") from error


@dataclass(frozen=True)
class DomainEvidenceProviderNormalizationRequest:
    """Caller-owned provider payload with explicit domain and connector scope."""

    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    connector_kind: str
    provider: str
    payload: Any
    request: Any = _MISSING
    outcome: str = "unknown"
    claim_posture: Mapping[str, Any] | None = None
    parent_digests: tuple[str, ...] = ()
    source_plan_digest: str | None = None

    def __post_init__(self) -> None:
        _text("domain evidence provider group_id", self.group_id)
        _bounded_text_list("domain evidence provider domains", self.domains, required=True)
        _text("domain evidence provider subject_id", self.subject_id)
        _text("domain evidence provider source_tool", self.source_tool)
        _text("domain evidence provider", self.provider)
        if self.connector_kind not in DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS:
            raise ArgumentError("domain evidence provider connector_kind is invalid")
        if self.outcome not in DOMAIN_EVIDENCE_PROVIDER_OUTCOMES:
            raise ArgumentError("domain evidence provider outcome is invalid")
        if self.claim_posture is not None:
            if not isinstance(self.claim_posture, Mapping):
                raise ArgumentError("domain evidence provider claim_posture must be an object")
            if self.claim_posture.get("status") not in DOMAIN_REPORT_CLAIM_STATUSES:
                raise ArgumentError("domain evidence provider claim_posture.status is invalid")
            _bounded_text_list(
                "domain evidence provider claim_posture.does_not_claim",
                self.claim_posture.get("does_not_claim"),
                required=True,
            )
        if not isinstance(self.payload, (Mapping, Sequence)) or isinstance(self.payload, (str, bytes)):
            raise ArgumentError("domain evidence provider payload must be an object or array")
        _json_value("domain evidence provider payload", self.payload)
        if self.request is not _MISSING:
            _json_value("domain evidence provider request", self.request)
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence provider parent_digests must contain at most 128 values")
        for parent in self.parent_digests:
            _digest("domain evidence provider parent digest", parent)
        if self.source_plan_digest is not None:
            _digest("domain evidence provider source plan digest", self.source_plan_digest)

    @property
    def request_supplied(self) -> bool:
        return self.request is not _MISSING

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "source_tool": self.source_tool,
            "connector_kind": self.connector_kind,
            "provider": self.provider,
            "payload": self.payload,
            "outcome": self.outcome,
            "parent_digests": list(self.parent_digests),
        }
        if self.request is not _MISSING:
            result["request"] = self.request
        if self.claim_posture is not None:
            result["claim_posture"] = dict(self.claim_posture)
        if self.source_plan_digest is not None:
            result["source_plan_digest"] = self.source_plan_digest
        return result


@dataclass(frozen=True)
class DomainEvidenceProviderNormalizationReport:
    raw: dict[str, Any]
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    connector_kind: str
    provider: str
    outcome: str
    payload_digest: str
    request_digest: str | None
    response: Mapping[str, Any]
    intake: Mapping[str, Any]
    artifact_registry: Mapping[str, Any]
    catalogue_digest: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderNormalizationReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("domain evidence provider normalization report is not successful")
        normalization = _mapping("domain evidence provider normalization", raw.get("normalization"))
        artifact_registry = _mapping("domain evidence provider artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("domain evidence provider intake artifact is not indexed")
        request_digest = raw.get("request_digest")
        return cls(
            raw=raw,
            group_id=_route_text("domain evidence provider group_id", raw.get("group_id")),
            domains=_bounded_text_list("domain evidence provider domains", raw.get("domains"), required=True),
            subject_id=_route_text("domain evidence provider subject_id", raw.get("subject_id")),
            source_tool=_route_text("domain evidence provider source_tool", raw.get("source_tool")),
            connector_kind=_route_text("domain evidence provider connector_kind", raw.get("connector_kind")),
            provider=_route_text("domain evidence provider", raw.get("provider")),
            outcome=_route_text("domain evidence provider outcome", raw.get("outcome")),
            payload_digest=_digest("domain evidence provider payload digest", normalization.get("payload_digest")),
            request_digest=(
                None
                if request_digest is None
                else _digest("domain evidence provider request digest", request_digest)
            ),
            response=_mapping("domain evidence provider response", raw.get("response")),
            intake=_mapping("domain evidence provider intake", raw.get("intake")),
            artifact_registry=artifact_registry,
            catalogue_digest=_digest("domain evidence provider catalogue digest", raw.get("catalogue_digest")),
            guarantees=_route_strings("domain evidence provider guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("domain evidence provider limitations", raw.get("does_not_claim", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_evidence_provider_normalization_report(
    value: Mapping[str, Any],
) -> DomainEvidenceProviderNormalizationReport:
    return DomainEvidenceProviderNormalizationReport.from_wire(value)


__all__ = [
    "DOMAIN_EVIDENCE_PROVIDER_CONNECTOR_KINDS",
    "DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_OUTCOMES",
    "DomainEvidenceProviderNormalizationRequest",
    "DomainEvidenceProviderNormalizationReport",
    "domain_evidence_provider_normalization_report",
]
