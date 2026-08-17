"""Typed receipts for large provider payloads kept in caller-managed storage."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .artifacts import _digest, _mapping, _text
from .capability import _route_strings, _route_text, _tool_payload
from .domain_reports import _bounded_text_list
from .errors import ArgumentError

DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA = "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1"
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW = "domain_evidence_provider_external_payload_receipt"
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA = "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1"
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW = "domain_evidence_provider_external_payload_replay_verify"
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_CONNECTOR_KINDS = (
    "literature", "clinical_trial", "fhir", "object_store", "provider_api"
)
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_STORAGE_BACKENDS = (
    "object_store", "file", "database", "caller_managed"
)
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LOCATOR_KINDS = ("opaque", "uri", "path")
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_AVAILABILITIES = ("available", "partial", "missing", "unknown")
DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_RETENTIONS = ("ephemeral", "durable", "unknown")
MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES = 64 * 1024 * 1024 * 1024


def _lower_digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or value != value.lower():
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return _digest(name, value)


def _reject_unknown(name: str, raw: Mapping[str, Any], allowed: set[str]) -> None:
    unknown = sorted(set(raw) - allowed)
    if unknown:
        raise ArgumentError(f"{name} contains unsupported fields: {', '.join(unknown)}")


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadReceiptRequest:
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    provider: str
    connector_kind: str
    handoff_digest: str
    transfer_id: str
    payload_digest: str
    byte_length: int
    storage_backend: str
    locator_kind: str
    locator: str
    content_type: str | None = None
    content_encoding: str | None = None
    request_digest: str | None = None
    parent_digests: tuple[str, ...] = ()
    availability: str = "unknown"
    retention: str = "unknown"
    attempt_id: str | None = None

    def __post_init__(self) -> None:
        for name, value in (("group_id", self.group_id), ("subject_id", self.subject_id), ("source_tool", self.source_tool), ("provider", self.provider), ("transfer_id", self.transfer_id), ("locator", self.locator)):
            _text(f"external provider payload {name}", value)
        _bounded_text_list("external provider payload domains", self.domains, required=True)
        if self.connector_kind not in DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_CONNECTOR_KINDS:
            raise ArgumentError("external provider payload connector_kind is invalid")
        _lower_digest("external provider payload handoff_digest", self.handoff_digest)
        _lower_digest("external provider payload payload_digest", self.payload_digest)
        if isinstance(self.byte_length, bool) or not isinstance(self.byte_length, int) or not 1 <= self.byte_length <= MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES:
            raise ArgumentError("external provider payload byte_length is outside its bound")
        if self.storage_backend not in DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_STORAGE_BACKENDS:
            raise ArgumentError("external provider payload storage_backend is invalid")
        if self.locator_kind not in DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LOCATOR_KINDS:
            raise ArgumentError("external provider payload locator_kind is invalid")
        if "://" in self.locator and "@" in self.locator.split("://", 1)[1].split("/", 1)[0].split("?", 1)[0].split("#", 1)[0]:
            raise ArgumentError("external provider payload locator must not contain embedded credentials")
        for name, value in (("content_type", self.content_type), ("content_encoding", self.content_encoding), ("attempt_id", self.attempt_id)):
            if value is not None:
                _text(f"external provider payload {name}", value)
        if self.request_digest is not None:
            _lower_digest("external provider payload request_digest", self.request_digest)
        if len(self.parent_digests) > 128:
            raise ArgumentError("external provider payload parent_digests exceeds its bound")
        for parent in self.parent_digests:
            _lower_digest("external provider payload parent digest", parent)
        if self.availability not in DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_AVAILABILITIES:
            raise ArgumentError("external provider payload availability is invalid")
        if self.retention not in DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_RETENTIONS:
            raise ArgumentError("external provider payload retention is invalid")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadReceiptRequest":
        raw = _mapping("external provider payload receipt request", value)
        _reject_unknown(
            "external provider payload receipt request",
            raw,
            {"group_id", "domains", "subject_id", "source_tool", "provider", "connector_kind", "handoff_digest", "transfer_id", "payload_digest", "byte_length", "storage_backend", "locator_kind", "locator", "content_type", "content_encoding", "request_digest", "parent_digests", "availability", "retention", "attempt_id"},
        )
        return cls(
            group_id=_route_text("external provider payload group_id", raw.get("group_id")),
            domains=_bounded_text_list("external provider payload domains", raw.get("domains"), required=True),
            subject_id=_route_text("external provider payload subject_id", raw.get("subject_id")),
            source_tool=_route_text("external provider payload source_tool", raw.get("source_tool")),
            provider=_route_text("external provider payload provider", raw.get("provider")),
            connector_kind=_route_text("external provider payload connector_kind", raw.get("connector_kind")),
            handoff_digest=_route_text("external provider payload handoff_digest", raw.get("handoff_digest")),
            transfer_id=_route_text("external provider payload transfer_id", raw.get("transfer_id")),
            payload_digest=_route_text("external provider payload payload_digest", raw.get("payload_digest")),
            byte_length=raw.get("byte_length"),
            storage_backend=_route_text("external provider payload storage_backend", raw.get("storage_backend")),
            locator_kind=_route_text("external provider payload locator_kind", raw.get("locator_kind")),
            locator=_route_text("external provider payload locator", raw.get("locator")),
            content_type=raw.get("content_type"),
            content_encoding=raw.get("content_encoding"),
            request_digest=raw.get("request_digest"),
            parent_digests=_bounded_text_list("external provider payload parent_digests", raw.get("parent_digests", []), maximum=128),
            availability=_route_text("external provider payload availability", raw.get("availability", "unknown")),
            retention=_route_text("external provider payload retention", raw.get("retention", "unknown")),
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
            "handoff_digest": self.handoff_digest,
            "transfer_id": self.transfer_id,
            "payload_digest": self.payload_digest,
            "byte_length": self.byte_length,
            "storage_backend": self.storage_backend,
            "locator_kind": self.locator_kind,
            "locator": self.locator,
            "parent_digests": list(self.parent_digests),
            "availability": self.availability,
            "retention": self.retention,
        }
        for name in ("content_type", "content_encoding", "request_digest", "attempt_id"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadReceiptReport:
    raw: dict[str, Any]
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    provider: str
    connector_kind: str
    handoff_digest: str
    transfer_id: str
    payload_digest: str
    byte_length: int
    storage_backend: str
    locator_kind: str
    locator: str
    availability: str
    retention: str
    receipt_digest: str
    execution: str
    readiness_claimed: bool
    artifact_registry: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadReceiptReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("external provider payload receipt report is not successful")
        receipt = _mapping("external provider payload receipt", raw.get("receipt"))
        if raw.get("readiness_claimed") is not False or receipt.get("readiness_claimed") is not False:
            raise ArgumentError("external provider payload receipt readiness must remain false")
        registry = _mapping("external provider payload receipt artifact registry", raw.get("artifact_registry"))
        if registry.get("indexed") is not True:
            raise ArgumentError("external provider payload receipt artifact is not indexed")
        return cls(
            raw=raw,
            group_id=_route_text("external provider payload group_id", receipt.get("group_id")),
            domains=_bounded_text_list("external provider payload domains", receipt.get("domains"), required=True),
            subject_id=_route_text("external provider payload subject_id", receipt.get("subject_id")),
            source_tool=_route_text("external provider payload source_tool", receipt.get("source_tool")),
            provider=_route_text("external provider payload provider", receipt.get("provider")),
            connector_kind=_route_text("external provider payload connector_kind", receipt.get("connector_kind")),
            handoff_digest=_lower_digest("external provider payload handoff_digest", receipt.get("handoff_digest")),
            transfer_id=_route_text("external provider payload transfer_id", receipt.get("transfer_id")),
            payload_digest=_lower_digest("external provider payload payload_digest", receipt.get("payload_digest")),
            byte_length=receipt.get("byte_length"),
            storage_backend=_route_text("external provider payload storage_backend", receipt.get("storage_backend")),
            locator_kind=_route_text("external provider payload locator_kind", receipt.get("locator_kind")),
            locator=_route_text("external provider payload locator", receipt.get("locator")),
            availability=_route_text("external provider payload availability", receipt.get("availability")),
            retention=_route_text("external provider payload retention", receipt.get("retention")),
            receipt_digest=_lower_digest("external provider payload receipt_digest", receipt.get("receipt_digest")),
            execution=_route_text("external provider payload execution", receipt.get("execution")),
            readiness_claimed=False,
            artifact_registry=registry,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadReplayRequest:
    """Compare caller-retained external payload metadata without fetching the payload."""

    receipt: DomainEvidenceProviderExternalPayloadReceiptRequest
    expected_receipt_digest: str
    expected_handoff_digest: str
    expected_payload_digest: str
    expected_byte_length: int

    def __post_init__(self) -> None:
        _lower_digest("external provider payload expected_receipt_digest", self.expected_receipt_digest)
        _lower_digest("external provider payload expected_handoff_digest", self.expected_handoff_digest)
        _lower_digest("external provider payload expected_payload_digest", self.expected_payload_digest)
        if isinstance(self.expected_byte_length, bool) or not isinstance(self.expected_byte_length, int) or not 1 <= self.expected_byte_length <= MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES:
            raise ArgumentError("external provider payload expected_byte_length is outside its bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadReplayRequest":
        raw = _mapping("external provider payload replay request", value)
        expected_names = {"expected_receipt_digest", "expected_handoff_digest", "expected_payload_digest", "expected_byte_length"}
        missing = sorted(expected_names - set(raw))
        if missing:
            raise ArgumentError(f"external provider payload replay request is missing: {', '.join(missing)}")
        receipt_raw = {name: item for name, item in raw.items() if name not in expected_names}
        return cls(
            receipt=DomainEvidenceProviderExternalPayloadReceiptRequest.from_wire(receipt_raw),
            expected_receipt_digest=_route_text("external provider payload expected_receipt_digest", raw.get("expected_receipt_digest")),
            expected_handoff_digest=_route_text("external provider payload expected_handoff_digest", raw.get("expected_handoff_digest")),
            expected_payload_digest=_route_text("external provider payload expected_payload_digest", raw.get("expected_payload_digest")),
            expected_byte_length=raw.get("expected_byte_length"),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            **self.receipt.to_mcp_arguments(),
            "expected_receipt_digest": self.expected_receipt_digest,
            "expected_handoff_digest": self.expected_handoff_digest,
            "expected_payload_digest": self.expected_payload_digest,
            "expected_byte_length": self.expected_byte_length,
        }


@dataclass(frozen=True)
class DomainEvidenceProviderExternalPayloadReplayVerificationReport:
    raw: dict[str, Any]
    replay_status: str
    matched: bool
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    source_tool: str
    provider: str
    connector_kind: str
    expected_receipt_digest: str
    observed_receipt_digest: str
    expected_handoff_digest: str
    observed_handoff_digest: str
    expected_payload_digest: str
    observed_payload_digest: str
    expected_byte_length: int
    observed_byte_length: int
    matches: Mapping[str, bool]
    differences: tuple[str, ...]
    receipt: Mapping[str, Any]
    replay_digest: str
    artifact_registry: Mapping[str, Any]
    execution: str
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DomainEvidenceProviderExternalPayloadReplayVerificationReport":
        raw = _tool_payload(value, DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW)
        if raw.get("ok") is not True:
            raise ArgumentError("external provider payload replay report is not successful")
        replay = _mapping("external provider payload replay", raw.get("replay"))
        receipt = _mapping("external provider payload replay receipt", replay.get("receipt"))
        registry = _mapping("external provider payload replay artifact registry", raw.get("artifact_registry"))
        if raw.get("readiness_claimed") is not False or replay.get("readiness_claimed") is not None:
            raise ArgumentError("external provider payload replay readiness must remain false")
        if registry.get("indexed") is not True:
            raise ArgumentError("external provider payload replay artifact is not indexed")
        matches_raw = _mapping("external provider payload replay matches", replay.get("matches"))
        matches = {name: value for name, value in matches_raw.items() if isinstance(name, str) and isinstance(value, bool)}
        if len(matches) != len(matches_raw):
            raise ArgumentError("external provider payload replay matches must be boolean fields")
        differences = _bounded_text_list("external provider payload replay differences", replay.get("differences", []), maximum=4)
        replay_status = _route_text("external provider payload replay status", replay.get("replay_status"))
        if replay_status not in {"matched", "mismatch"}:
            raise ArgumentError("external provider payload replay status is invalid")
        matched = replay.get("matched")
        if not isinstance(matched, bool) or matched != (replay_status == "matched"):
            raise ArgumentError("external provider payload replay matched status is inconsistent")
        return cls(
            raw=raw,
            replay_status=replay_status,
            matched=matched,
            group_id=_route_text("external provider payload replay group_id", replay.get("group_id")),
            domains=_bounded_text_list("external provider payload replay domains", replay.get("domains"), required=True),
            subject_id=_route_text("external provider payload replay subject_id", replay.get("subject_id")),
            source_tool=_route_text("external provider payload replay source_tool", replay.get("source_tool")),
            provider=_route_text("external provider payload replay provider", replay.get("provider")),
            connector_kind=_route_text("external provider payload replay connector_kind", replay.get("connector_kind")),
            expected_receipt_digest=_lower_digest("external provider payload replay expected_receipt_digest", replay.get("expected_receipt_digest")),
            observed_receipt_digest=_lower_digest("external provider payload replay observed_receipt_digest", replay.get("observed_receipt_digest")),
            expected_handoff_digest=_lower_digest("external provider payload replay expected_handoff_digest", replay.get("expected_handoff_digest")),
            observed_handoff_digest=_lower_digest("external provider payload replay observed_handoff_digest", replay.get("observed_handoff_digest")),
            expected_payload_digest=_lower_digest("external provider payload replay expected_payload_digest", replay.get("expected_payload_digest")),
            observed_payload_digest=_lower_digest("external provider payload replay observed_payload_digest", replay.get("observed_payload_digest")),
            expected_byte_length=replay.get("expected_byte_length"),
            observed_byte_length=replay.get("observed_byte_length"),
            matches=matches,
            differences=differences,
            receipt=receipt,
            replay_digest=_lower_digest("external provider payload replay replay_digest", replay.get("replay_digest")),
            artifact_registry=registry,
            execution=_route_text("external provider payload replay execution", raw.get("execution")),
            readiness_claimed=False,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def domain_evidence_provider_external_payload_receipt_report(value: Mapping[str, Any]) -> DomainEvidenceProviderExternalPayloadReceiptReport:
    return DomainEvidenceProviderExternalPayloadReceiptReport.from_wire(value)


def domain_evidence_provider_external_payload_replay_verification_report(value: Mapping[str, Any]) -> DomainEvidenceProviderExternalPayloadReplayVerificationReport:
    return DomainEvidenceProviderExternalPayloadReplayVerificationReport.from_wire(value)


__all__ = [
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_CONNECTOR_KINDS",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_STORAGE_BACKENDS",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LOCATOR_KINDS",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_AVAILABILITIES",
    "DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_RETENTIONS",
    "MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES",
    "DomainEvidenceProviderExternalPayloadReceiptRequest",
    "DomainEvidenceProviderExternalPayloadReceiptReport",
    "domain_evidence_provider_external_payload_receipt_report",
    "DomainEvidenceProviderExternalPayloadReplayRequest",
    "DomainEvidenceProviderExternalPayloadReplayVerificationReport",
    "domain_evidence_provider_external_payload_replay_verification_report",
]
