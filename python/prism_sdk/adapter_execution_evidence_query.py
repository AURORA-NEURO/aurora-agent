"""Typed read-only queries over retained adapter execution evidence."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping

from .artifacts import _digest, _mapping
from .capability import _route_text, _tool_payload
from .errors import ArgumentError

ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA = "bioprism-devplat-adapter-execution-evidence-query/0.1"
ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW = "adapter_execution_evidence_query"
MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS = 128
EXECUTION_STATUSES = frozenset({"planned", "started", "succeeded", "partial", "refused", "failed", "unknown"})
CONFORMANCE_STATUSES = frozenset({"verified", "partial", "refused", "not_run", "unknown"})
SEMANTIC_LOSS_STATUSES = frozenset({"lossless", "lossy", "unknown", "not_applicable"})


@dataclass(frozen=True)
class AdapterExecutionEvidenceQueryRequest:
    group_id: str | None = None
    domain: str | None = None
    subject_id: str | None = None
    adapter_id: str | None = None
    source_id: str | None = None
    execution_status: str | None = None
    conformance_status: str | None = None
    semantic_loss_status: str | None = None
    after: str | None = None
    max_items: int = 100
    include_artifacts: bool = False

    def __post_init__(self) -> None:
        for name, value in (
            ("group_id", self.group_id),
            ("domain", self.domain),
            ("subject_id", self.subject_id),
            ("adapter_id", self.adapter_id),
            ("source_id", self.source_id),
            ("execution_status", self.execution_status),
            ("conformance_status", self.conformance_status),
            ("semantic_loss_status", self.semantic_loss_status),
        ):
            if value is not None:
                _route_text(f"adapter evidence query {name}", value)
        for name, value, choices in (
            ("execution_status", self.execution_status, EXECUTION_STATUSES),
            ("conformance_status", self.conformance_status, CONFORMANCE_STATUSES),
            ("semantic_loss_status", self.semantic_loss_status, SEMANTIC_LOSS_STATUSES),
        ):
            if value is not None and value not in choices:
                raise ArgumentError(f"adapter evidence query {name} is invalid")
        if self.after is not None:
            _digest("adapter evidence query after", self.after)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS:
            raise ArgumentError("adapter evidence query max_items must be between 1 and 128")
        if not isinstance(self.include_artifacts, bool):
            raise ArgumentError("adapter evidence query include_artifacts must be boolean")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterExecutionEvidenceQueryRequest":
        raw = _mapping("adapter execution evidence query request", value)
        allowed = {
            "group_id", "domain", "subject_id", "adapter_id", "source_id",
            "execution_status", "conformance_status", "semantic_loss_status",
            "after", "max_items", "include_artifacts",
        }
        unknown = sorted(set(raw) - allowed)
        if unknown:
            raise ArgumentError(f"adapter execution evidence query request contains unsupported fields: {', '.join(unknown)}")
        optional_text = lambda name: None if raw.get(name) is None else _route_text(f"adapter evidence query {name}", raw.get(name))
        return cls(
            group_id=optional_text("group_id"),
            domain=optional_text("domain"),
            subject_id=optional_text("subject_id"),
            adapter_id=optional_text("adapter_id"),
            source_id=optional_text("source_id"),
            execution_status=optional_text("execution_status"),
            conformance_status=optional_text("conformance_status"),
            semantic_loss_status=optional_text("semantic_loss_status"),
            after=None if raw.get("after") is None else _digest("adapter evidence query after", raw.get("after")),
            max_items=raw.get("max_items", 100),
            include_artifacts=raw.get("include_artifacts", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"max_items": self.max_items, "include_artifacts": self.include_artifacts}
        for name in (
            "group_id", "domain", "subject_id", "adapter_id", "source_id",
            "execution_status", "conformance_status", "semantic_loss_status", "after",
        ):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class AdapterExecutionEvidenceQueryReport:
    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    query_digest: str
    registry_generation: int
    registry_size: int
    readiness_claimed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterExecutionEvidenceQueryReport":
        raw = _tool_payload(value, ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW)
        if raw.get("ok") is not True or raw.get("readiness_claimed") is not False:
            raise ArgumentError("adapter execution evidence query is not successful or ready")
        if raw.get("execution") != "not_started":
            raise ArgumentError("adapter execution evidence query execution posture is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, list):
            raise ArgumentError("adapter execution evidence query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            next_after = _digest("adapter evidence query next_after", next_after)
        if not isinstance(raw.get("has_more"), bool):
            raise ArgumentError("adapter execution evidence query has_more must be boolean")
        for name in ("registry_generation", "registry_size"):
            number = raw.get(name)
            if isinstance(number, bool) or not isinstance(number, int) or number < 0:
                raise ArgumentError(f"adapter execution evidence query {name} must be a non-negative integer")
        return cls(
            raw=raw,
            rows=tuple(_mapping("adapter execution evidence query row", row) for row in rows),
            next_after=next_after,
            has_more=raw["has_more"],
            query_digest=_digest("adapter execution evidence query_digest", raw.get("query_digest")),
            registry_generation=raw["registry_generation"],
            registry_size=raw["registry_size"],
            readiness_claimed=False,
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def adapter_execution_evidence_query_report(value: Mapping[str, Any]) -> AdapterExecutionEvidenceQueryReport:
    return AdapterExecutionEvidenceQueryReport.from_wire(value)


__all__ = [
    "ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA",
    "ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW",
    "MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS",
    "AdapterExecutionEvidenceQueryRequest",
    "AdapterExecutionEvidenceQueryReport",
    "adapter_execution_evidence_query_report",
]
