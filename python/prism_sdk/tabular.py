"""Typed SDK boundary for the Rust CSV/TSV adapter and its conformance evidence."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


TABULAR_MAX_ITEMS = 1_000
TABULAR_MAX_BYTES = 10_000_000


def _tabular_bytes(name: str, value: str, maximum: int) -> None:
    _route_text(name, value)
    if len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} exceeds the {maximum}-byte limit")


def _tabular_bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _tabular_payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct MCP text, structured MCP, or REST tool content for tabular ingest."""

    raw = _route_mapping("tabular ingest response", value)
    if "source_id" in raw and "conformance" in raw:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and "source_id" in structured and "conformance" in structured:
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"tabular response text is not JSON: {error}") from error
                    decoded_mapping = _route_mapping("decoded tabular response", decoded)
                    if "source_id" in decoded_mapping and "conformance" in decoded_mapping:
                        return decoded_mapping
    raise ArgumentError("response does not contain a tabular ingest projection")


@dataclass(frozen=True)
class TabularIngestRequest:
    """Bounded explicit CSV/TSV ingestion request owned by the caller."""

    source_id: str
    profile: Mapping[str, Any]
    csv: str | None = None
    document: str | None = None
    format: str | None = None
    provenance: Mapping[str, Any] | None = None
    include_facts: bool = False
    max_items: int = 100
    max_bytes: int = TABULAR_MAX_BYTES

    def __post_init__(self) -> None:
        _tabular_bytes("source_id", self.source_id, 512)
        if not isinstance(self.profile, Mapping):
            raise ArgumentError("profile must be a mapping serialized from TabularProfile")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= TABULAR_MAX_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {TABULAR_MAX_ITEMS}")
        if isinstance(self.max_bytes, bool) or not isinstance(self.max_bytes, int) or not 1 <= self.max_bytes <= TABULAR_MAX_BYTES:
            raise ArgumentError(f"max_bytes must be between 1 and {TABULAR_MAX_BYTES}")
        if (self.csv is None) == (self.document is None):
            raise ArgumentError("exactly one of csv or document is required")
        if self.csv is not None:
            _tabular_bytes("csv", self.csv, self.max_bytes)
        if self.document is not None:
            _tabular_bytes("document", self.document, 512)
        if self.format is not None:
            _route_text("format", self.format)
        if self.provenance is not None and not isinstance(self.provenance, Mapping):
            raise ArgumentError("provenance must be a mapping when supplied")
        if not isinstance(self.include_facts, bool):
            raise ArgumentError("include_facts must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "source_id": self.source_id,
            "profile": dict(self.profile),
            "include_facts": self.include_facts,
            "max_items": self.max_items,
            "max_bytes": self.max_bytes,
        }
        if self.csv is not None:
            arguments["csv"] = self.csv
        if self.document is not None:
            arguments["document"] = self.document
        if self.format is not None:
            arguments["format"] = self.format
        if self.provenance is not None:
            arguments["provenance"] = dict(self.provenance)
        return arguments


@dataclass(frozen=True)
class TabularCheckReport:
    """One independent conformance check and its human-actionable detail."""

    raw: dict[str, Any]
    check: str
    status: str
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TabularCheckReport":
        raw = _route_mapping("tabular conformance check", value)
        status = _route_text("tabular conformance check status", raw.get("status"))
        if status not in {"pass", "fail", "not_applicable"}:
            raise ArgumentError(f"unknown tabular conformance status: {status!r}")
        return cls(
            raw=raw,
            check=_route_text("tabular conformance check", raw.get("check")),
            status=status,
            detail=_route_text("tabular conformance detail", raw.get("detail")),
        )

    @property
    def passed(self) -> bool:
        return self.status == "pass"


@dataclass(frozen=True)
class TabularConformanceReport:
    """Independent determinism, manifest, loss, and fact-integrity evidence."""

    raw: dict[str, Any]
    adapter: str
    adapter_version: str
    source_id: str
    checks: tuple[TabularCheckReport, ...]
    passed: bool
    verified: bool
    summary: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TabularConformanceReport":
        raw = _route_mapping("tabular conformance", value)
        report = _route_mapping("tabular conformance report", raw.get("report"))
        checks_raw = report.get("checks")
        if not isinstance(checks_raw, Sequence) or isinstance(checks_raw, (str, bytes)):
            raise ArgumentError("tabular conformance report checks must be an array")
        checks = tuple(TabularCheckReport.from_wire(check) for check in checks_raw)
        passed = _tabular_bool("tabular conformance passed", raw.get("passed"))
        verified = _tabular_bool("tabular conformance verified", raw.get("verified"))
        if verified and not passed:
            raise ArgumentError("tabular conformance cannot be verified when it has failed")
        adapter = _route_text("tabular conformance adapter", report.get("adapter"))
        adapter_version = _route_text("tabular conformance adapter_version", report.get("adapter_version"))
        source_id = _route_text("tabular conformance source_id", report.get("source_id"))
        return cls(
            raw=raw,
            adapter=adapter,
            adapter_version=adapter_version,
            source_id=source_id,
            checks=checks,
            passed=passed,
            verified=verified,
            summary=_route_text("tabular conformance summary", raw.get("summary")),
        )

    @property
    def failed_checks(self) -> tuple[TabularCheckReport, ...]:
        return tuple(check for check in self.checks if check.status == "fail")

    @property
    def not_applicable_checks(self) -> tuple[TabularCheckReport, ...]:
        return tuple(check for check in self.checks if check.status == "not_applicable")


@dataclass(frozen=True)
class TabularSemanticLossReport:
    """Explicit loss variant; unaudited is never treated as lossless."""

    raw: dict[str, Any]
    audit: str
    mapped: tuple[Any, ...]
    lost: tuple[dict[str, Any], ...]
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TabularSemanticLossReport":
        raw = _route_mapping("tabular semantic_loss", value)
        audit = _route_text("tabular semantic_loss audit", raw.get("audit"))
        if audit not in {"unaudited", "lossless", "lossy"}:
            raise ArgumentError(f"unknown tabular semantic-loss audit: {audit!r}")
        mapped_raw = raw.get("mapped", [])
        if not isinstance(mapped_raw, Sequence) or isinstance(mapped_raw, (str, bytes)):
            raise ArgumentError("tabular semantic_loss mapped must be an array")
        lost_raw = raw.get("lost", [])
        if not isinstance(lost_raw, Sequence) or isinstance(lost_raw, (str, bytes)):
            raise ArgumentError("tabular semantic_loss lost must be an array")
        lost = tuple(_route_mapping("tabular semantic-loss entry", entry) for entry in lost_raw)
        reason = raw.get("reason")
        if audit == "unaudited":
            reason = _route_text("tabular semantic_loss reason", reason)
        elif reason is not None:
            raise ArgumentError("tabular semantic_loss reason is only valid for unaudited reports")
        if audit == "lossless" and lost:
            raise ArgumentError("lossless tabular semantic_loss cannot contain lost entries")
        if audit == "lossy" and not lost:
            raise ArgumentError("lossy tabular semantic_loss must contain lost entries")
        return cls(raw=raw, audit=audit, mapped=tuple(mapped_raw), lost=lost, reason=reason)

    @property
    def audited(self) -> bool:
        return self.audit != "unaudited"

    @property
    def lossless(self) -> bool:
        return self.audit == "lossless"


@dataclass(frozen=True)
class TabularManifestReport:
    """Source and transformation identity needed to replay an ingestion."""

    raw: dict[str, Any]
    source_id: str
    declared_format: str | None
    source_digest: str
    byte_length: int | None
    adapter: str
    adapter_version: str
    profile_digest: str | None
    provenance: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TabularManifestReport":
        raw = _route_mapping("tabular manifest", value)
        declared_format = raw.get("declared_format")
        if declared_format is not None:
            declared_format = _route_text("tabular manifest declared_format", declared_format)
        byte_length = raw.get("byte_length")
        if byte_length is not None:
            byte_length = _route_count("tabular manifest byte_length", byte_length)
        profile_digest = raw.get("profile_digest")
        if profile_digest is not None:
            profile_digest = _route_text("tabular manifest profile_digest", profile_digest)
        provenance = raw.get("provenance")
        if provenance is not None:
            provenance = _route_mapping("tabular manifest provenance", provenance)
        return cls(
            raw=raw,
            source_id=_route_text("tabular manifest source_id", raw.get("source_id")),
            declared_format=declared_format,
            source_digest=_route_text("tabular manifest source_digest", raw.get("source_digest")),
            byte_length=byte_length,
            adapter=_route_text("tabular manifest adapter", raw.get("adapter")),
            adapter_version=_route_text("tabular manifest adapter_version", raw.get("adapter_version")),
            profile_digest=profile_digest,
            provenance=provenance,
        )


@dataclass(frozen=True)
class TabularIngestReport:
    """Typed result of a real CSV/TSV ingest with bounded fact disclosure."""

    raw: dict[str, Any]
    ok: bool
    source_id: str
    fact_count: int
    ingestion_sha256: str
    manifest: TabularManifestReport
    semantic_loss: TabularSemanticLossReport
    conformance: TabularConformanceReport
    max_items: int
    facts: tuple[dict[str, Any], ...]
    omitted_facts: int
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "TabularIngestReport":
        raw = _tabular_payload(value)
        ok = _tabular_bool("tabular ingest ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("tabular ingest report is not successful")
        source_id = _route_text("tabular ingest source_id", raw.get("source_id"))
        fact_count = _route_count("tabular ingest fact_count", raw.get("fact_count"))
        max_items = _route_count("tabular ingest max_items", raw.get("max_items"))
        if not 1 <= max_items <= TABULAR_MAX_ITEMS:
            raise ArgumentError(f"tabular ingest max_items must be between 1 and {TABULAR_MAX_ITEMS}")
        manifest = TabularManifestReport.from_wire(raw.get("manifest"))
        if manifest.source_id != source_id:
            raise ArgumentError("tabular ingest source_id does not reconcile with its manifest")
        semantic_loss = TabularSemanticLossReport.from_wire(raw.get("semantic_loss"))
        conformance = TabularConformanceReport.from_wire(raw.get("conformance"))
        if conformance.source_id != source_id:
            raise ArgumentError("tabular ingest source_id does not reconcile with conformance")
        facts_raw = raw.get("facts", [])
        if not isinstance(facts_raw, Sequence) or isinstance(facts_raw, (str, bytes)):
            raise ArgumentError("tabular ingest facts must be an array")
        facts = tuple(_route_mapping("tabular fact", fact) for fact in facts_raw)
        omitted_facts = _route_count("tabular ingest omitted_facts", raw.get("omitted_facts", 0))
        if facts and len(facts) + omitted_facts != fact_count:
            raise ArgumentError("tabular ingest fact_count does not reconcile with facts and omitted_facts")
        return cls(
            raw=raw,
            ok=ok,
            source_id=source_id,
            fact_count=fact_count,
            ingestion_sha256=_route_text("tabular ingest ingestion_sha256", raw.get("ingestion_sha256")),
            manifest=manifest,
            semantic_loss=semantic_loss,
            conformance=conformance,
            max_items=max_items,
            facts=facts,
            omitted_facts=omitted_facts,
            limitations=_route_strings("tabular ingest limitations", raw.get("limitations", [])),
        )

    @property
    def conformance_verified(self) -> bool:
        return self.conformance.verified

    @property
    def publishable_candidate(self) -> bool:
        return self.conformance.verified and self.semantic_loss.lossless

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def tabular_ingest_report(value: Mapping[str, Any]) -> TabularIngestReport:
    """Parse direct MCP text, structured MCP, or HTTP REST tabular output."""

    return TabularIngestReport.from_wire(value)


__all__ = [
    "TABULAR_MAX_BYTES",
    "TABULAR_MAX_ITEMS",
    "TabularCheckReport",
    "TabularConformanceReport",
    "TabularIngestReport",
    "TabularIngestRequest",
    "TabularManifestReport",
    "TabularSemanticLossReport",
    "tabular_ingest_report",
]
