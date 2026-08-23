"""Typed caller-managed provider connector handoff models.

The handoff is a boundary object, not a provider client. It lets a plugin declare its scope,
capabilities, authentication posture, and digest identities before the Rust core receives a
provider payload. Secret references remain opaque labels and credential material is intentionally
not representable by these request models.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .artifacts import _digest, _mapping, _text
from .capability import _route_strings, _route_text, _tool_payload
from .domain_reports import _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA = "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1"
DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW = "domain_evidence_provider_connector_handoff"
DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA = "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1"
DOMAIN_EVIDENCE_PROVIDER_HANDOFF_STATUSES = (
    "prepared", "submitted", "observed", "partial", "refused", "error", "unknown"
)
DOMAIN_EVIDENCE_PROVIDER_AUTH_STATUSES = ("none", "caller_asserted", "delegated", "unknown")
DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CONNECTOR_KINDS = (
    "literature", "clinical_trial", "fhir", "object_store", "provider_api"
)
MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS = 32


def _reject_unknown(name: str, raw: Mapping[str, Any], allowed: set[str]) -> None:
    unknown = sorted(set(raw) - allowed)
    if unknown:
        raise ArgumentError(f"{name} contains unsupported fields: {', '.join(unknown)}")


def _coerce_strings(name: str, value: Any, *, required: bool = False, maximum: int = 64) -> tuple[str, ...]:
    return _bounded_text_list(name, value, required=required, maximum=maximum)


@dataclass(frozen=True)
class DomainEvidenceProviderAuthPosture:
    status: str = "unknown"
    secret_refs: tuple[str, ...] = ()
    does_not_claim: tuple[str, ...] = (
        "credential material is not retained by the core",
        "provider authentication or authorization is not verified",
    )

    def __post_init__(self) -> None:
        if self.status not in DOMAIN_EVIDENCE_PROVIDER_AUTH_STATUSES:
            raise ArgumentError("domain evidence provider auth posture status is invalid")
        _coerce_strings("domain evidence provider secret_refs", self.secret_refs, maximum=MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS)
        if len(self.secret_refs) > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS:
            raise ArgumentError("domain evidence provider secret_refs exceeds its bound")
        _coerce_strings("domain evidence provider auth non-claims", self.does_not_claim, required=True)

    @classmethod
    def from_wire(cls, value: Any) -> "DomainEvidenceProviderAuthPosture":
        raw = _mapping("domain evidence provider auth posture", value)
        _reject_unknown("domain evidence provider auth posture", raw, {"status", "secret_refs", "does_not_claim"})
        return cls(
            status=_route_text("domain evidence provider auth posture status", raw.get("status")),
            secret_refs=_coerce_strings("domain evidence provider secret_refs", raw.get("secret_refs", []), maximum=MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS),
            does_not_claim=_coerce_strings("domain evidence provider auth non-claims", raw.get("does_not_claim"), required=True),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status,
            "secret_refs": list(self.secret_refs),
            "does_not_claim": list(self.does_not_claim),
        }


@dataclass(frozen=True)
class DomainEvidenceProviderConnectorManifest:
    connector_id: str
    version: str
    provider: str
    connector_kind: str
    domains: tuple[str, ...]
    capabilities: tuple[str, ...]
    auth_posture: DomainEvidenceProviderAuthPosture = DomainEvidenceProviderAuthPosture()
    schema: str = DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA
    transport: str = "caller_managed"

    def __post_init__(self) -> None:
        if self.schema != DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA:
            raise ArgumentError("domain evidence provider manifest schema is unsupported")
        if self.transport != "caller_managed":
            raise ArgumentError("domain evidence provider manifest transport must be caller_managed")
        for name, value in (("connector_id", self.connector_id), ("version", self.version), ("provider", self.provider)):
            _text(f"domain evidence provider manifest {name}", value)
        if self.connector_kind not in DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CONNECTOR_KINDS:
            raise ArgumentError("domain evidence provider manifest connector_kind is invalid")
        _coerce_strings("domain evidence provider manifest domains", self.domains, required=True)
        _coerce_strings("domain evidence provider manifest capabilities", self.capabilities, required=True)
        if not isinstance(self.auth_posture, DomainEvidenceProviderAuthPosture):
            raise ArgumentError("domain evidence provider manifest auth_posture must be typed")

    @classmethod
    def from_wire(cls, value: Any) -> "DomainEvidenceProviderConnectorManifest":
        raw = _mapping("domain evidence provider connector manifest", value)
        _reject_unknown(
            "domain evidence provider connector manifest",
            raw,
            {"schema", "connector_id", "version", "provider", "connector_kind", "domains", "capabilities", "transport", "auth_posture"},
        )
        return cls(
            connector_id=_route_text("domain evidence provider connector id", raw.get("connector_id")),
            version=_route_text("domain evidence provider connector version", raw.get("version")),
            provider=_route_text("domain evidence provider manifest provider", raw.get("provider")),
            connector_kind=_route_text("domain evidence provider manifest connector kind", raw.get("connector_kind")),
            domains=_coerce_strings("domain evidence provider manifest domains", raw.get("domains"), required=True),
            capabilities=_coerce_strings("domain evidence provider manifest capabilities", raw.get("capabilities"), required=True),
            auth_posture=DomainEvidenceProviderAuthPosture.from_wire(raw.get("auth_posture")),
            schema=_route_text("domain evidence provider manifest schema", raw.get("schema")),
            transport=_route_text("domain evidence provider manifest transport", raw.get("transport")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": self.schema,
            "connector_id": self.connector_id,
            "version": self.version,
            "provider": self.provider,
            "connector_kind": self.connector_kind,
            "domains": list(self.domains),
            "capabilities": list(self.capabilities),
            "transport": self.transport,
            "auth_posture": self.auth_posture.to_dict(),
        }


@dataclass(frozen=True)
class DomainEvidenceProviderHandoffRequest:
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    provider: str
    connector_kind: str
    manifest: DomainEvidenceProviderConnectorManifest
    status: str = "unknown"
    request_digest: str | None = None
    payload_digest: str | None = None
    source_plan_digest: str | None = None
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None

    def __post_init__(self) -> None:
        for name, value in (("group_id", self.group_id), ("subject_id", self.subject_id), ("source_tool", self.source_tool), ("provider", self.provider)):
            _text(f"domain evidence provider handoff {name}", value)
        _coerce_strings("domain evidence provider handoff domains", self.domains, required=True)
        if self.connector_kind not in DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CONNECTOR_KINDS:
            raise ArgumentError("domain evidence provider handoff connector_kind is invalid")
        if self.status not in DOMAIN_EVIDENCE_PROVIDER_HANDOFF_STATUSES:
            raise ArgumentError("domain evidence provider handoff status is invalid")
        if not isinstance(self.manifest, DomainEvidenceProviderConnectorManifest):
            raise ArgumentError("domain evidence provider handoff manifest must be typed")
        if self.manifest.provider != self.provider or self.manifest.connector_kind != self.connector_kind:
            raise ArgumentError("domain evidence provider handoff manifest scope does not match request")
        if any(domain not in self.manifest.domains for domain in self.domains):
            raise ArgumentError("domain evidence provider handoff domains are outside manifest scope")
        for name, value in (("request_digest", self.request_digest), ("payload_digest", self.payload_digest), ("source_plan_digest", self.source_plan_digest)):
            if value is not None:
                _digest(f"domain evidence provider handoff {name}", value)
        if len(self.parent_digests) > 128:
            raise ArgumentError("domain evidence provider handoff parent_digests exceeds its bound")
        for parent in self.parent_digests:
            _digest("domain evidence provider handoff parent digest", parent)
        if self.attempt_id is not None:
            _text("domain evidence provider handoff attempt_id", self.attempt_id)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderHandoffRequest":
        raw = _mapping("domain evidence provider handoff request", value)
        _reject_unknown(
            "domain evidence provider handoff request",
            raw,
            {"group_id", "domains", "subject_id", "source_tool", "provider", "connector_kind", "manifest", "status", "request_digest", "payload_digest", "source_plan_digest", "parent_digests", "attempt_id"},
        )
        return cls(
            group_id=_route_text("domain evidence provider handoff group_id", raw.get("group_id")),
            domains=_coerce_strings("domain evidence provider handoff domains", raw.get("domains"), required=True),
            subject_id=_route_text("domain evidence provider handoff subject_id", raw.get("subject_id")),
            source_tool=_route_text("domain evidence provider handoff source_tool", raw.get("source_tool")),
            provider=_route_text("domain evidence provider handoff provider", raw.get("provider")),
            connector_kind=_route_text("domain evidence provider handoff connector_kind", raw.get("connector_kind")),
            manifest=DomainEvidenceProviderConnectorManifest.from_wire(raw.get("manifest")),
            status=_route_text("domain evidence provider handoff status", raw.get("status", "unknown")),
            request_digest=raw.get("request_digest"),
            payload_digest=raw.get("payload_digest"),
            source_plan_digest=raw.get("source_plan_digest"),
            parent_digests=_coerce_strings("domain evidence provider handoff parent_digests", raw.get("parent_digests", []), maximum=128),
            attempt_id=raw.get("attempt_id"),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "source_tool": self.source_tool,
            "provider": self.provider,
            "connector_kind": self.connector_kind,
            "manifest": self.manifest.to_dict(),
            "status": self.status,
            "parent_digests": list(self.parent_digests),
        }
        for name in ("request_digest", "payload_digest", "source_plan_digest", "attempt_id"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class DomainEvidenceProviderHandoffReport:
    raw: dict[str, Any]
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    provider: str
    connector_kind: str
    status: str
    manifest: DomainEvidenceProviderConnectorManifest
    manifest_digest: str
    handoff_digest: str
    execution: str
    readiness_claimed: bool
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    artifact_registry: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderHandoffReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("domain evidence provider handoff report is not successful")
        handoff = _mapping("domain evidence provider handoff", raw.get("handoff"))
        readiness = raw.get("readiness_claimed")
        if readiness is not False:
            raise ArgumentError("domain evidence provider handoff readiness must remain false")
        registry = _mapping("domain evidence provider handoff artifact registry", raw.get("artifact_registry"))
        if registry.get("indexed") is not True:
            raise ArgumentError("domain evidence provider handoff artifact is not indexed")
        return cls(
            raw=raw,
            group_id=_route_text("domain evidence provider handoff group_id", handoff.get("group_id")),
            domains=_coerce_strings("domain evidence provider handoff domains", handoff.get("domains"), required=True),
            subject_id=_route_text("domain evidence provider handoff subject_id", handoff.get("subject_id")),
            source_tool=_route_text("domain evidence provider handoff source_tool", handoff.get("source_tool")),
            provider=_route_text("domain evidence provider handoff provider", handoff.get("provider")),
            connector_kind=_route_text("domain evidence provider handoff connector_kind", handoff.get("connector_kind")),
            status=_route_text("domain evidence provider handoff status", handoff.get("status")),
            manifest=DomainEvidenceProviderConnectorManifest.from_wire(handoff.get("manifest")),
            manifest_digest=_digest("domain evidence provider manifest digest", handoff.get("manifest_digest")),
            handoff_digest=_digest("domain evidence provider handoff digest", handoff.get("handoff_digest")),
            execution=_route_text("domain evidence provider handoff execution", handoff.get("execution")),
            readiness_claimed=readiness,
            guarantees=_route_strings("domain evidence provider handoff guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("domain evidence provider handoff limitations", raw.get("does_not_claim", [])),
            artifact_registry=registry,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_evidence_provider_handoff_report(value: Mapping[str, Any]) -> DomainEvidenceProviderHandoffReport:
    return DomainEvidenceProviderHandoffReport.from_wire(value)


__all__ = [
    "DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_HANDOFF_STATUSES",
    "DOMAIN_EVIDENCE_PROVIDER_AUTH_STATUSES",
    "DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CONNECTOR_KINDS",
    "MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS",
    "DomainEvidenceProviderAuthPosture",
    "DomainEvidenceProviderConnectorManifest",
    "DomainEvidenceProviderHandoffRequest",
    "DomainEvidenceProviderHandoffReport",
    "domain_evidence_provider_handoff_report",
]
