"""Typed caller evidence for native and Python-delegated adapter execution."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .artifacts import _digest, _mapping, _text
from .capability import _route_text, _tool_payload
from .errors import ArgumentError

ADAPTER_EXECUTION_EVIDENCE_SCHEMA = "bioprism-devplat-adapter-execution-evidence/0.1"
ADAPTER_EXECUTION_EVIDENCE_WORKFLOW = "adapter_execution_evidence"
MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES = 128
MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS = 128
MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS = 2_000_000
MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES = 68_719_476_736

EXECUTION_STATUSES = frozenset({"planned", "started", "succeeded", "partial", "refused", "failed", "unknown"})
CONFORMANCE_STATUSES = frozenset({"verified", "partial", "refused", "not_run", "unknown"})
SEMANTIC_LOSS_STATUSES = frozenset({"lossless", "lossy", "unknown", "not_applicable"})
LOSS_SEVERITIES = frozenset({"info", "warning", "blocking"})


def _bounded_optional_text(name: str, value: Any, maximum: int = 512) -> str | None:
    if value is None:
        return None
    value = _text(name, value)
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte bound")
    if any(ord(character) < 0x20 for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    return value


@dataclass(frozen=True)
class AdapterExecutionLoss:
    kind: str
    severity: str
    detail: str
    source_path: str | None = None
    target_path: str | None = None

    def __post_init__(self) -> None:
        _bounded_optional_text("loss kind", self.kind, 128)
        if self.severity not in LOSS_SEVERITIES:
            raise ArgumentError("loss severity is invalid")
        _bounded_optional_text("loss detail", self.detail, 512)
        _bounded_optional_text("loss source_path", self.source_path, 512)
        _bounded_optional_text("loss target_path", self.target_path, 512)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterExecutionLoss":
        raw = _mapping("adapter execution loss", value)
        allowed = {"kind", "severity", "detail", "source_path", "target_path"}
        unknown = sorted(set(raw) - allowed)
        if unknown:
            raise ArgumentError(f"adapter execution loss contains unsupported fields: {', '.join(unknown)}")
        return cls(
            kind=_bounded_optional_text("loss kind", raw.get("kind"), 128) or "",
            severity=_route_text("loss severity", raw.get("severity")),
            detail=_bounded_optional_text("loss detail", raw.get("detail"), 512) or "",
            source_path=_bounded_optional_text("loss source_path", raw.get("source_path"), 512),
            target_path=_bounded_optional_text("loss target_path", raw.get("target_path"), 512),
        )

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"kind": self.kind, "severity": self.severity, "detail": self.detail}
        if self.source_path is not None:
            result["source_path"] = self.source_path
        if self.target_path is not None:
            result["target_path"] = self.target_path
        return result


@dataclass(frozen=True)
class AdapterExecutionEvidenceRequest:
    group_id: str
    domains: tuple[str, ...]
    subject_id: str
    adapter_id: str
    adapter_version: str
    source_id: str
    input_digest: str
    execution_status: str
    conformance_status: str
    semantic_loss_status: str
    output_digest: str | None = None
    losses: tuple[AdapterExecutionLoss, ...] = ()
    item_count: int | None = None
    byte_length: int | None = None
    error_code: str | None = None
    parent_digests: tuple[str, ...] = ()
    attempt_id: str | None = None

    def __post_init__(self) -> None:
        for name, value in (
            ("group_id", self.group_id),
            ("subject_id", self.subject_id),
            ("adapter_id", self.adapter_id),
            ("adapter_version", self.adapter_version),
            ("source_id", self.source_id),
        ):
            _bounded_optional_text(name, value, 512 if name not in {"adapter_id", "adapter_version"} else 256)
        if not self.domains or len(self.domains) > 64:
            raise ArgumentError("domains must contain 1..=64 entries")
        for domain in self.domains:
            _bounded_optional_text("domain", domain, 512)
        _digest("adapter execution input_digest", self.input_digest)
        if self.output_digest is not None:
            _digest("adapter execution output_digest", self.output_digest)
        if self.execution_status not in EXECUTION_STATUSES:
            raise ArgumentError("execution_status is invalid")
        if self.conformance_status not in CONFORMANCE_STATUSES:
            raise ArgumentError("conformance_status is invalid")
        if self.semantic_loss_status not in SEMANTIC_LOSS_STATUSES:
            raise ArgumentError("semantic_loss_status is invalid")
        if len(self.losses) > MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES or any(
            not isinstance(loss, AdapterExecutionLoss) for loss in self.losses
        ):
            raise ArgumentError("losses must contain at most 128 AdapterExecutionLoss values")
        identities = [loss.to_wire().__repr__() for loss in self.losses]
        if len(identities) != len(set(identities)):
            raise ArgumentError("loss entries must be unique")
        if self.semantic_loss_status in {"lossless", "not_applicable"} and self.losses:
            raise ArgumentError("lossless or not_applicable evidence cannot contain loss entries")
        if self.semantic_loss_status == "lossy" and not self.losses:
            raise ArgumentError("lossy evidence must contain at least one loss entry")
        if self.execution_status == "succeeded" and self.output_digest is None:
            raise ArgumentError("succeeded execution requires output_digest")
        if self.execution_status in {"refused", "failed"} and self.error_code is None:
            raise ArgumentError("refused or failed execution requires error_code")
        _bounded_optional_text("error_code", self.error_code, 128)
        if self.item_count is not None and (
            isinstance(self.item_count, bool)
            or not isinstance(self.item_count, int)
            or not 0 <= self.item_count <= MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS
        ):
            raise ArgumentError("item_count is outside its bound")
        if self.byte_length is not None and (
            isinstance(self.byte_length, bool)
            or not isinstance(self.byte_length, int)
            or not 0 <= self.byte_length <= MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES
        ):
            raise ArgumentError("byte_length is outside its bound")
        if len(self.parent_digests) > MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS:
            raise ArgumentError("parent_digests must contain at most 128 values")
        for parent in self.parent_digests:
            _digest("adapter execution parent_digest", parent)
        _bounded_optional_text("attempt_id", self.attempt_id, 128)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterExecutionEvidenceRequest":
        raw = _mapping("adapter execution evidence request", value)
        allowed = {
            "group_id", "domains", "subject_id", "adapter_id", "adapter_version", "source_id", "input_digest",
            "output_digest", "execution_status", "conformance_status", "semantic_loss_status", "losses",
            "item_count", "byte_length", "error_code", "parent_digests", "attempt_id",
        }
        unknown = sorted(set(raw) - allowed)
        if unknown:
            raise ArgumentError(f"adapter execution evidence request contains unsupported fields: {', '.join(unknown)}")
        required = sorted(set(("group_id", "domains", "subject_id", "adapter_id", "adapter_version", "source_id", "input_digest", "execution_status", "conformance_status", "semantic_loss_status")) - set(raw))
        if required:
            raise ArgumentError(f"adapter execution evidence request is missing: {', '.join(required)}")
        domains = raw.get("domains")
        if not isinstance(domains, list):
            raise ArgumentError("domains must be an array")
        losses = raw.get("losses", [])
        if not isinstance(losses, list):
            raise ArgumentError("losses must be an array")
        parents = raw.get("parent_digests", [])
        if not isinstance(parents, list):
            raise ArgumentError("parent_digests must be an array")
        return cls(
            group_id=_route_text("adapter execution group_id", raw.get("group_id")),
            domains=tuple(_route_text("adapter execution domain", value) for value in domains),
            subject_id=_route_text("adapter execution subject_id", raw.get("subject_id")),
            adapter_id=_route_text("adapter execution adapter_id", raw.get("adapter_id")),
            adapter_version=_route_text("adapter execution adapter_version", raw.get("adapter_version")),
            source_id=_route_text("adapter execution source_id", raw.get("source_id")),
            input_digest=_digest("adapter execution input_digest", raw.get("input_digest")),
            output_digest=None if raw.get("output_digest") is None else _digest("adapter execution output_digest", raw.get("output_digest")),
            execution_status=_route_text("adapter execution execution_status", raw.get("execution_status")),
            conformance_status=_route_text("adapter execution conformance_status", raw.get("conformance_status")),
            semantic_loss_status=_route_text("adapter execution semantic_loss_status", raw.get("semantic_loss_status")),
            losses=tuple(AdapterExecutionLoss.from_wire(loss) for loss in losses),
            item_count=raw.get("item_count"),
            byte_length=raw.get("byte_length"),
            error_code=None if raw.get("error_code") is None else _route_text("adapter execution error_code", raw.get("error_code")),
            parent_digests=tuple(_digest("adapter execution parent_digest", value) for value in parents),
            attempt_id=None if raw.get("attempt_id") is None else _route_text("adapter execution attempt_id", raw.get("attempt_id")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "group_id": self.group_id,
            "domains": list(self.domains),
            "subject_id": self.subject_id,
            "adapter_id": self.adapter_id,
            "adapter_version": self.adapter_version,
            "source_id": self.source_id,
            "input_digest": self.input_digest.lower(),
            "execution_status": self.execution_status,
            "conformance_status": self.conformance_status,
            "semantic_loss_status": self.semantic_loss_status,
            "losses": [loss.to_wire() for loss in self.losses],
            "parent_digests": [parent.lower() for parent in self.parent_digests],
        }
        for name in ("output_digest", "item_count", "byte_length", "error_code", "attempt_id"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value.lower() if name == "output_digest" else value
        return result


@dataclass(frozen=True)
class AdapterExecutionEvidenceReport:
    raw: dict[str, Any]
    evidence: Mapping[str, Any]
    adapter: Mapping[str, Any]
    execution_status: str
    conformance_status: str
    semantic_loss_status: str
    output_digest: str | None
    evidence_digest: str
    artifact_registry: Mapping[str, Any]
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterExecutionEvidenceReport":
        raw = _tool_payload(value, ADAPTER_EXECUTION_EVIDENCE_WORKFLOW)
        if raw.get("ok") is not True or raw.get("readiness_claimed") is not False:
            raise ArgumentError("adapter execution evidence is not successful or ready")
        if raw.get("execution") != "not_started" or raw.get("attestation_posture") != "caller_asserted":
            raise ArgumentError("adapter execution evidence posture is invalid")
        evidence = _mapping("adapter execution evidence", raw.get("evidence"))
        adapter = _mapping("adapter execution adapter", raw.get("adapter"))
        if evidence.get("schema") != ADAPTER_EXECUTION_EVIDENCE_SCHEMA or evidence.get("workflow") != ADAPTER_EXECUTION_EVIDENCE_WORKFLOW:
            raise ArgumentError("adapter execution evidence schema or workflow is invalid")
        execution_status = _route_text("adapter execution evidence execution_status", evidence.get("execution_status"))
        conformance_status = _route_text("adapter execution evidence conformance_status", evidence.get("conformance_status"))
        semantic_loss_status = _route_text("adapter execution evidence semantic_loss_status", evidence.get("semantic_loss_status"))
        if execution_status not in EXECUTION_STATUSES or conformance_status not in CONFORMANCE_STATUSES or semantic_loss_status not in SEMANTIC_LOSS_STATUSES:
            raise ArgumentError("adapter execution evidence contains an invalid status")
        evidence_digest = _digest("adapter execution evidence_digest", raw.get("evidence_digest"))
        if evidence.get("evidence_digest") != evidence_digest:
            raise ArgumentError("adapter execution nested and top-level evidence digests disagree")
        output_digest = evidence.get("output_digest")
        if output_digest is not None:
            output_digest = _digest("adapter execution output_digest", output_digest)
        artifact_registry = _mapping("adapter execution artifact registry", raw.get("artifact_registry"))
        if artifact_registry.get("indexed") is not True:
            raise ArgumentError("adapter execution evidence was not indexed")
        return cls(
            raw=raw,
            evidence=evidence,
            adapter=adapter,
            execution_status=execution_status,
            conformance_status=conformance_status,
            semantic_loss_status=semantic_loss_status,
            output_digest=output_digest,
            evidence_digest=evidence_digest,
            artifact_registry=artifact_registry,
            readiness_claimed=False,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def adapter_execution_evidence_report(value: Mapping[str, Any]) -> AdapterExecutionEvidenceReport:
    return AdapterExecutionEvidenceReport.from_wire(value)


__all__ = [
    "ADAPTER_EXECUTION_EVIDENCE_SCHEMA",
    "ADAPTER_EXECUTION_EVIDENCE_WORKFLOW",
    "MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES",
    "MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS",
    "MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES",
    "MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS",
    "AdapterExecutionLoss",
    "AdapterExecutionEvidenceRequest",
    "AdapterExecutionEvidenceReport",
    "adapter_execution_evidence_report",
]
