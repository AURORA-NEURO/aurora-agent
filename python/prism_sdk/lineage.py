"""Typed specimen-lineage audit contracts.

The worldfactory lineage kernel deliberately separates material/ancestry findings from identity
evidence.  This module keeps that separation visible at the SDK boundary and validates bounded
projections without attempting to recompute the Rust audit locally.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


LINEAGE_FINGERPRINT_STATES = frozenset({"consistent", "mismatch", "no_evidence_available"})
LINEAGE_FINDING_KINDS = frozenset(
    {
        "lineage_cycle",
        "mass_not_conserved",
        "temporal_implausibility",
        "duplicate_content",
        "identity_mismatch",
        "artifacts_disagree",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    required = (
        "ok",
        "specimen_count",
        "artifact_count",
        "finding_count",
        "clean",
        "identity_complete",
        "fingerprint_count",
        "fingerprints",
        "omitted_fingerprints",
        "unchecked_identity_count",
        "unchecked_identity",
        "finding_count_returned",
        "findings",
        "omitted_findings",
        "guarantees",
        "limitations",
    )
    raw = _route_mapping("lineage audit response", value)
    if all(key in raw for key in required):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and all(key in structured for key in required):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"lineage audit response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded lineage audit response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError("response does not contain a lineage audit projection")


@dataclass(frozen=True)
class LineageAuditArgs:
    """Bounded serialized specimen registry supplied to the authoritative lineage audit."""

    registry: Mapping[str, Any]
    max_items: int = 100

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LineageAuditArgs":
        raw = _route_mapping("lineage audit arguments", value)
        return cls(raw.get("registry"), raw.get("max_items", 100))

    def __post_init__(self) -> None:
        registry = _route_mapping("lineage registry", self.registry)
        nodes = _route_mapping("lineage registry nodes", registry.get("nodes", {}))
        artifacts = _route_mapping("lineage registry artifacts", registry.get("artifacts", {}))
        if len(nodes) > 10_000:
            raise ArgumentError("lineage registry must contain at most 10000 specimens")
        if len(artifacts) > 20_000:
            raise ArgumentError("lineage registry must contain at most 20000 artifacts")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 1_000:
            raise ArgumentError("lineage max_items must be between 1 and 1000")
        object.__setattr__(self, "registry", registry)

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"registry": dict(self.registry), "max_items": self.max_items}


@dataclass(frozen=True)
class LineageFingerprintReport:
    raw: dict[str, Any]
    state: str
    specimen: str
    declared_donor: str | None
    fingerprint_donor: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LineageFingerprintReport":
        raw = _route_mapping("lineage fingerprint", value)
        state = _route_text("lineage fingerprint state", raw.get("fingerprint"))
        if state not in LINEAGE_FINGERPRINT_STATES:
            raise ArgumentError(f"unknown lineage fingerprint state: {state!r}")
        declared = raw.get("declared_donor")
        fingerprint = raw.get("fingerprint_donor")
        if state == "mismatch":
            declared = _route_text("lineage declared donor", declared)
            fingerprint = _route_text("lineage fingerprint donor", fingerprint)
        elif declared is not None or fingerprint is not None:
            raise ArgumentError("non-mismatch lineage fingerprints cannot carry donor comparison fields")
        return cls(
            raw,
            state,
            _route_text("lineage fingerprint specimen", raw.get("specimen")),
            declared,
            fingerprint,
        )

    @property
    def consistent(self) -> bool:
        return self.state == "consistent"

    @property
    def checked(self) -> bool:
        return self.state != "no_evidence_available"


@dataclass(frozen=True)
class LineageFindingReport:
    raw: dict[str, Any]
    kind: str
    specimen: str | None
    parent: str | None
    left: str | None
    right: str | None
    artifacts: tuple[str, ...]
    fingerprint: LineageFingerprintReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LineageFindingReport":
        raw = _route_mapping("lineage finding", value)
        kind = _route_text("lineage finding kind", raw.get("finding"))
        if kind not in LINEAGE_FINDING_KINDS:
            raise ArgumentError(f"unknown lineage finding kind: {kind!r}")
        specimen = raw.get("specimen")
        parent = raw.get("parent")
        left = raw.get("left")
        right = raw.get("right")
        if kind in {"lineage_cycle", "identity_mismatch", "artifacts_disagree"}:
            specimen = _route_text("lineage finding specimen", specimen)
        if kind == "temporal_implausibility":
            specimen = _route_text("lineage finding child", specimen)
            parent = _route_text("lineage finding parent", parent)
        if kind == "mass_not_conserved":
            parent = _route_text("lineage finding parent", parent)
            for field in ("parent_mass_ug", "child_total_ug"):
                _route_count(f"lineage finding {field}", raw.get(field))
        if kind == "duplicate_content":
            left = _route_text("lineage finding left", left)
            right = _route_text("lineage finding right", right)
        artifacts: tuple[str, ...] = ()
        if kind == "artifacts_disagree":
            artifacts = _route_strings("lineage finding artifacts", raw.get("artifacts"))
            if not artifacts:
                raise ArgumentError("artifact disagreement must name at least one artifact")
        nested = raw.get("fingerprint")
        fingerprint = None
        if kind == "identity_mismatch":
            fingerprint = LineageFingerprintReport.from_wire(nested)
            if fingerprint.state != "mismatch" or fingerprint.specimen != specimen:
                raise ArgumentError("identity mismatch finding does not preserve mismatch evidence")
        elif nested is not None:
            raise ArgumentError("only identity mismatch findings may carry fingerprint evidence")
        return cls(raw, kind, specimen, parent, left, right, artifacts, fingerprint)


@dataclass(frozen=True)
class LineageAuditReport:
    raw: dict[str, Any]
    ok: bool
    specimen_count: int
    artifact_count: int
    finding_count: int
    clean: bool
    identity_complete: bool
    fingerprint_count: int
    fingerprints: tuple[LineageFingerprintReport, ...]
    omitted_fingerprints: int
    unchecked_identity_count: int
    unchecked_identity: tuple[str, ...]
    finding_count_returned: int
    findings: tuple[LineageFindingReport, ...]
    omitted_findings: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "LineageAuditReport":
        raw = _payload(value)
        if not _bool("lineage audit ok", raw.get("ok")):
            raise ArgumentError("lineage audit report is not successful")
        specimen_count = _route_count("lineage specimen_count", raw.get("specimen_count"))
        artifact_count = _route_count("lineage artifact_count", raw.get("artifact_count"))
        finding_count = _route_count("lineage finding_count", raw.get("finding_count"))
        clean = _bool("lineage clean", raw.get("clean"))
        identity_complete = _bool("lineage identity_complete", raw.get("identity_complete"))
        fingerprint_count = _route_count("lineage fingerprint_count", raw.get("fingerprint_count"))
        fingerprints_raw = _array("lineage fingerprints", raw.get("fingerprints"))
        fingerprints = tuple(LineageFingerprintReport.from_wire(item) for item in fingerprints_raw)
        omitted_fingerprints = _route_count("lineage omitted_fingerprints", raw.get("omitted_fingerprints"))
        if len(fingerprints) + omitted_fingerprints != fingerprint_count:
            raise ArgumentError("lineage fingerprint counts do not reconcile")
        unchecked_identity_count = _route_count("lineage unchecked_identity_count", raw.get("unchecked_identity_count"))
        unchecked_identity = _route_strings("lineage unchecked_identity", raw.get("unchecked_identity"))
        if len(unchecked_identity) > unchecked_identity_count:
            raise ArgumentError("lineage unchecked identity preview exceeds its total")
        if identity_complete and (unchecked_identity_count or any(not item.consistent for item in fingerprints)):
            raise ArgumentError("lineage identity_complete contradicts visible identity evidence")
        finding_count_returned = _route_count("lineage finding_count_returned", raw.get("finding_count_returned"))
        findings_raw = _array("lineage findings", raw.get("findings"))
        findings = tuple(LineageFindingReport.from_wire(item) for item in findings_raw)
        omitted_findings = _route_count("lineage omitted_findings", raw.get("omitted_findings"))
        if len(findings) != finding_count_returned or len(findings) + omitted_findings != finding_count:
            raise ArgumentError("lineage finding counts do not reconcile")
        if clean != (finding_count == 0):
            raise ArgumentError("lineage clean does not reconcile with finding_count")
        return cls(
            raw,
            True,
            specimen_count,
            artifact_count,
            finding_count,
            clean,
            identity_complete,
            fingerprint_count,
            fingerprints,
            omitted_fingerprints,
            unchecked_identity_count,
            unchecked_identity,
            finding_count_returned,
            findings,
            omitted_findings,
            _route_strings("lineage guarantees", raw.get("guarantees")),
            _route_strings("lineage limitations", raw.get("limitations")),
        )

    @property
    def ready_for_identity_claim(self) -> bool:
        """True only when material audit is clean and every fingerprint is consistent."""

        return self.clean and self.identity_complete

    @property
    def identity_gap(self) -> bool:
        return not self.identity_complete


def lineage_audit_report(value: Mapping[str, Any]) -> LineageAuditReport:
    """Parse direct MCP or HTTP lineage-audit output."""

    return LineageAuditReport.from_wire(value)


__all__ = [
    "LINEAGE_FINDING_KINDS",
    "LINEAGE_FINGERPRINT_STATES",
    "LineageAuditArgs",
    "LineageAuditReport",
    "LineageFindingReport",
    "LineageFingerprintReport",
    "lineage_audit_report",
]
