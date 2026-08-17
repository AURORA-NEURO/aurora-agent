"""Typed content-addressed developer-delivery receipt requests and reports."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError

DELIVERY_RECEIPT_SCHEMA = "bioprism-devplat-delivery-receipt/0.1"


def _mapping(name: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, Mapping) or not value:
        raise ArgumentError(f"{name} must be a non-empty mapping")
    return dict(value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


@dataclass(frozen=True)
class DeveloperDeliveryReceiptRequest:
    receipt_id: str
    delivery: Mapping[str, Any]

    def __post_init__(self) -> None:
        if not isinstance(self.receipt_id, str) or not self.receipt_id.strip() or len(self.receipt_id) > 128:
            raise ArgumentError("receipt_id must be a non-empty string of at most 128 characters")
        if any(character.isspace() and character in "\r\n" for character in self.receipt_id):
            raise ArgumentError("receipt_id must not contain newline characters")
        _mapping("delivery", self.delivery)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperDeliveryReceiptRequest":
        raw = _mapping("developer delivery receipt request", value)
        return cls(receipt_id=raw.get("receipt_id"), delivery=raw.get("delivery"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"receipt_id": self.receipt_id, "delivery": dict(self.delivery)}


@dataclass(frozen=True)
class DeveloperDeliveryReceiptVerificationRequest:
    receipt: Mapping[str, Any]
    delivery: Mapping[str, Any]

    def __post_init__(self) -> None:
        _mapping("receipt", self.receipt)
        _mapping("delivery", self.delivery)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperDeliveryReceiptVerificationRequest":
        raw = _mapping("developer delivery receipt verification request", value)
        return cls(receipt=raw.get("receipt"), delivery=raw.get("delivery"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"receipt": dict(self.receipt), "delivery": dict(self.delivery)}


@dataclass(frozen=True)
class DeliveryReceiptTargetReport:
    raw: dict[str, Any]
    target: str
    available: bool
    eligible: bool
    blockers: tuple[str, ...]
    notes: tuple[str, ...]
    ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReceiptTargetReport":
        raw = _route_mapping("delivery receipt target", value)
        return cls(
            raw=raw,
            target=_route_text("delivery receipt target name", raw.get("target")),
            available=_bool("delivery receipt target available", raw.get("available")),
            eligible=_bool("delivery receipt target eligible", raw.get("eligible")),
            blockers=_route_strings("delivery receipt target blockers", raw.get("blockers", [])),
            notes=_route_strings("delivery receipt target notes", raw.get("notes", [])),
            ready=_bool("delivery receipt target ready", raw.get("ready")),
        )


@dataclass(frozen=True)
class DeliveryReceiptEvidenceReport:
    raw: dict[str, Any]
    name: str
    present: bool
    ready: bool
    digest: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReceiptEvidenceReport":
        raw = _route_mapping("delivery receipt evidence", value)
        digest = raw.get("digest")
        if digest is not None:
            digest = _route_text("delivery receipt evidence digest", digest)
            if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest.lower()):
                raise ArgumentError("delivery receipt evidence digest must be a 64-character hexadecimal digest")
        return cls(
            raw=raw,
            name=_route_text("delivery receipt evidence name", raw.get("name")),
            present=_bool("delivery receipt evidence present", raw.get("present")),
            ready=_bool("delivery receipt evidence ready", raw.get("ready")),
            digest=digest,
        )


@dataclass(frozen=True)
class DeliveryReceiptFindingReport:
    raw: dict[str, Any]
    code: str
    severity: str
    subject: str
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReceiptFindingReport":
        raw = _route_mapping("delivery receipt finding", value)
        return cls(
            raw=raw,
            code=_route_text("delivery receipt finding code", raw.get("code")),
            severity=_route_text("delivery receipt finding severity", raw.get("severity")),
            subject=_route_text("delivery receipt finding subject", raw.get("subject")),
            detail=_route_text("delivery receipt finding detail", raw.get("detail")),
        )


@dataclass(frozen=True)
class DeveloperDeliveryReceiptReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    receipt_id: str
    delivery_digest: str
    target_digest: str
    receipt_digest: str
    valid: bool
    receipt_ready: bool
    release_request_ready: bool
    structurally_valid: bool
    release_candidate: bool
    target_count: int
    available_target_count: int
    ready_target_count: int
    blocked_target_count: int
    ready_evidence_count: int
    targets: tuple[DeliveryReceiptTargetReport, ...]
    evidence: tuple[DeliveryReceiptEvidenceReport, ...]
    findings: tuple[DeliveryReceiptFindingReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperDeliveryReceiptReport":
        raw = _tool_payload(value, "developer_delivery_receipt")
        if raw.get("ok") is not True:
            raise ArgumentError("developer delivery receipt report is not successful")
        targets = raw.get("targets", [])
        evidence = raw.get("evidence", [])
        findings = raw.get("findings", [])
        if not isinstance(targets, list) or not isinstance(evidence, list) or not isinstance(findings, list):
            raise ArgumentError("developer delivery receipt targets, evidence, and findings must be arrays")
        return cls(
            raw=raw,
            schema=_route_text("delivery receipt schema", raw.get("schema")),
            workflow=_route_text("delivery receipt workflow", raw.get("workflow")),
            receipt_id=_route_text("delivery receipt id", raw.get("receipt_id")),
            delivery_digest=_digest("delivery receipt delivery_digest", raw.get("delivery_digest")),
            target_digest=_digest("delivery receipt target_digest", raw.get("target_digest")),
            receipt_digest=_digest("delivery receipt receipt_digest", raw.get("receipt_digest")),
            valid=_bool("delivery receipt valid", raw.get("valid")),
            receipt_ready=_bool("delivery receipt receipt_ready", raw.get("receipt_ready")),
            release_request_ready=_bool("delivery receipt release_request_ready", raw.get("release_request_ready")),
            structurally_valid=_bool("delivery receipt structurally_valid", raw.get("structurally_valid")),
            release_candidate=_bool("delivery receipt release_candidate", raw.get("release_candidate")),
            target_count=_route_count("delivery receipt target_count", raw.get("target_count")),
            available_target_count=_route_count("delivery receipt available_target_count", raw.get("available_target_count")),
            ready_target_count=_route_count("delivery receipt ready_target_count", raw.get("ready_target_count")),
            blocked_target_count=_route_count("delivery receipt blocked_target_count", raw.get("blocked_target_count")),
            ready_evidence_count=_route_count("delivery receipt ready_evidence_count", raw.get("ready_evidence_count")),
            targets=tuple(DeliveryReceiptTargetReport.from_wire(item) for item in targets),
            evidence=tuple(DeliveryReceiptEvidenceReport.from_wire(item) for item in evidence),
            findings=tuple(DeliveryReceiptFindingReport.from_wire(item) for item in findings),
        )

    @property
    def blocking_findings(self) -> tuple[DeliveryReceiptFindingReport, ...]:
        return tuple(item for item in self.findings if item.severity == "blocking")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeveloperDeliveryReceiptVerificationReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    receipt_id: str
    supplied_receipt_digest: str | None
    recomputed_receipt_digest: str
    delivery_digest_match: bool
    target_digest_match: bool
    receipt_digest_match: bool
    targets_match: bool
    evidence_match: bool
    valid: bool
    verified: bool
    structurally_valid: bool
    findings: tuple[DeliveryReceiptFindingReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperDeliveryReceiptVerificationReport":
        raw = _tool_payload(value, "developer_delivery_receipt_verify")
        if raw.get("ok") is not True:
            raise ArgumentError("developer delivery receipt verification is not successful")
        findings = raw.get("findings", [])
        if not isinstance(findings, list):
            raise ArgumentError("developer delivery receipt verification findings must be an array")
        supplied = raw.get("supplied_receipt_digest")
        if supplied is not None:
            supplied = _digest("supplied delivery receipt digest", supplied)
        return cls(
            raw=raw,
            schema=_route_text("delivery receipt verification schema", raw.get("schema")),
            workflow=_route_text("delivery receipt verification workflow", raw.get("workflow")),
            receipt_id=_route_text("delivery receipt verification id", raw.get("receipt_id")),
            supplied_receipt_digest=supplied,
            recomputed_receipt_digest=_digest("recomputed delivery receipt digest", raw.get("recomputed_receipt_digest")),
            delivery_digest_match=_bool("delivery receipt delivery_digest_match", raw.get("delivery_digest_match")),
            target_digest_match=_bool("delivery receipt target_digest_match", raw.get("target_digest_match")),
            receipt_digest_match=_bool("delivery receipt receipt_digest_match", raw.get("receipt_digest_match")),
            targets_match=_bool("delivery receipt targets_match", raw.get("targets_match")),
            evidence_match=_bool("delivery receipt evidence_match", raw.get("evidence_match")),
            valid=_bool("delivery receipt verification valid", raw.get("valid")),
            verified=_bool("delivery receipt verified", raw.get("verified")),
            structurally_valid=_bool("delivery receipt verification structurally_valid", raw.get("structurally_valid")),
            findings=tuple(DeliveryReceiptFindingReport.from_wire(item) for item in findings),
        )

    @property
    def blocking_findings(self) -> tuple[DeliveryReceiptFindingReport, ...]:
        return tuple(item for item in self.findings if item.severity == "blocking")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if len(text) != 64 or any(character not in "0123456789abcdef" for character in text.lower()):
        raise ArgumentError(f"{name} must be a 64-character hexadecimal digest")
    return text


def developer_delivery_receipt_report(value: Mapping[str, Any]) -> DeveloperDeliveryReceiptReport:
    """Parse a direct MCP result or HTTP REST tool envelope."""

    return DeveloperDeliveryReceiptReport.from_wire(value)


def developer_delivery_receipt_verification_report(
    value: Mapping[str, Any],
) -> DeveloperDeliveryReceiptVerificationReport:
    """Parse a direct MCP result or HTTP REST receipt-verification envelope."""

    return DeveloperDeliveryReceiptVerificationReport.from_wire(value)


__all__ = [
    "DELIVERY_RECEIPT_SCHEMA",
    "DeveloperDeliveryReceiptRequest",
    "DeveloperDeliveryReceiptVerificationRequest",
    "DeliveryReceiptTargetReport",
    "DeliveryReceiptEvidenceReport",
    "DeliveryReceiptFindingReport",
    "DeveloperDeliveryReceiptReport",
    "DeveloperDeliveryReceiptVerificationReport",
    "developer_delivery_receipt_report",
    "developer_delivery_receipt_verification_report",
]
