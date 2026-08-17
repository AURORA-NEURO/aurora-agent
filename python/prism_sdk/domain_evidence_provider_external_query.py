"""Typed joined projections for external payload receipt, lineage, and execution evidence."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .artifacts import _digest, _mapping, _text
from .capability import _route_text, _tool_payload
from .errors import ArgumentError

DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA = "bioprism-devplat-domain-evidence-provider-external-payload-query/0.1"
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW = "domain_evidence_provider_external_payload_evidence_query"
MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS = 128


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadEvidenceQueryRequest:
    group_id: str | None = None
    domain: str | None = None
    subject_id: str | None = None
    after: str | None = None
    max_items: int = 100
    include_artifacts: bool = False

    def __post_init__(self) -> None:
        for name, value in (("group_id", self.group_id), ("domain", self.domain), ("subject_id", self.subject_id)):
            if value is not None:
                _text(f"external payload evidence query {name}", value)
        if self.after is not None:
            _digest("external payload evidence query after", self.after)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS:
            raise ArgumentError("external payload evidence query max_items must be between 1 and 128")
        if not isinstance(self.include_artifacts, bool):
            raise ArgumentError("external payload evidence query include_artifacts must be boolean")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadEvidenceQueryRequest":
        raw = _mapping("external payload evidence query request", value)
        allowed = {"group_id", "domain", "subject_id", "after", "max_items", "include_artifacts"}
        unknown = sorted(set(raw) - allowed)
        if unknown:
            raise ArgumentError(f"external payload evidence query request contains unsupported fields: {', '.join(unknown)}")
        return cls(
            group_id=None if raw.get("group_id") is None else _route_text("external payload evidence query group_id", raw.get("group_id")),
            domain=None if raw.get("domain") is None else _route_text("external payload evidence query domain", raw.get("domain")),
            subject_id=None if raw.get("subject_id") is None else _route_text("external payload evidence query subject_id", raw.get("subject_id")),
            after=None if raw.get("after") is None else _digest("external payload evidence query after", raw.get("after")),
            max_items=raw.get("max_items", 100),
            include_artifacts=raw.get("include_artifacts", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "max_items": self.max_items,
            "include_artifacts": self.include_artifacts,
        }
        for name in ("group_id", "domain", "subject_id", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value.lower() if name == "after" else value
        return result


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    query_digest: str
    registry_generation: int
    registry_size: int
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadEvidenceQueryReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW)
        if raw.get("ok") is not True or raw.get("readiness_claimed") is not False:
            raise ArgumentError("external payload evidence query is not successful or ready")
        rows = raw.get("rows", [])
        if not isinstance(rows, list):
            raise ArgumentError("external payload evidence query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            next_after = _digest("external payload evidence query next_after", next_after)
        if not isinstance(raw.get("has_more"), bool):
            raise ArgumentError("external payload evidence query has_more must be boolean")
        for name in ("registry_generation", "registry_size"):
            value_for_name = raw.get(name)
            if isinstance(value_for_name, bool) or not isinstance(value_for_name, int) or value_for_name < 0:
                raise ArgumentError(f"external payload evidence query {name} must be a non-negative integer")
        return cls(
            raw=raw,
            rows=tuple(_mapping("external payload evidence query row", row) for row in rows),
            next_after=next_after,
            has_more=raw["has_more"],
            query_digest=_digest("external payload evidence query_digest", raw.get("query_digest")),
            registry_generation=raw["registry_generation"],
            registry_size=raw["registry_size"],
            readiness_claimed=False,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_evidence_provider_external_payload_evidence_query_report(value: Mapping[str, Any]) -> DomainEvidenceProviderExternalPayloadEvidenceQueryReport:
    return DomainEvidenceProviderExternalPayloadEvidenceQueryReport.from_wire(value)


__all__ = [
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW",
    "MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS",
    "DomainEvidenceProviderExternalPayloadEvidenceQueryRequest",
    "DomainEvidenceProviderExternalPayloadEvidenceQueryReport",
    "domain_evidence_provider_external_payload_evidence_query_report",
]
