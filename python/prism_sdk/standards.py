"""Typed standards-comparability contracts.

The standards kernel refuses silent coercion.  This module keeps that distinction visible at the
SDK boundary: a conversion is a recorded receipt, an ontology or coordinate mismatch is a typed
block, and a digest is accepted only when it covers the returned report.
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


MEASUREMENT_BLOCKING_REASONS = frozenset({
    "kind_mismatch",
    "dimension_mismatch",
    "not_commensurable",
    "conversion_required",
    "unstated_frame",
    "frame_mismatch",
    "orientation_mismatch",
    "space_mismatch",
    "unstated_build",
    "build_mismatch",
    "convention_mismatch",
    "contig_mismatch",
    "unbound_term",
    "unmapped_term",
    "ambiguous_term",
    "namespace_mismatch",
    "ontology_version_drift",
    "granularity_mismatch",
    "term_mismatch",
})
MEASUREMENT_VERDICTS = frozenset({"comparable", "blocked"})
_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _finite_number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a structured measurement result from direct, MCP, or HTTP output."""

    raw = _route_mapping("measurement compare response", value)
    required = ("ok", "comparable", "policy", "report", "report_sha256", "guarantees", "limitations")
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
                    raise ArgumentError(f"measurement compare response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded measurement compare response", decoded)
                if all(key in decoded_mapping for key in required):
                    return decoded_mapping
    raise ArgumentError("response does not contain a measurement compare projection")


@dataclass(frozen=True)
class MeasurementCompareArgs:
    left: Mapping[str, Any]
    right: Mapping[str, Any]
    require_bound_terms: bool = False

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MeasurementCompareArgs":
        raw = _route_mapping("measurement compare arguments", value)
        return cls(raw.get("left"), raw.get("right"), raw.get("require_bound_terms", False))

    def __post_init__(self) -> None:
        if not isinstance(self.left, Mapping):
            raise ArgumentError("left measurement must be an object")
        if not isinstance(self.right, Mapping):
            raise ArgumentError("right measurement must be an object")
        if not isinstance(self.require_bound_terms, bool):
            raise ArgumentError("require_bound_terms must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "left": dict(self.left),
            "right": dict(self.right),
            "require_bound_terms": self.require_bound_terms,
        }


@dataclass(frozen=True)
class MeasurementConversionReport:
    raw: dict[str, Any]
    from_unit: str
    to_unit: str
    factor: float
    exactness: str
    convention: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MeasurementConversionReport":
        raw = _route_mapping("measurement conversion", value)
        exactness = _route_mapping("measurement conversion exactness", raw.get("exactness"))
        kind = _route_text("measurement conversion exactness kind", exactness.get("exactness"))
        if kind not in {"exact", "conventional"}:
            raise ArgumentError(f"unknown measurement conversion exactness: {kind!r}")
        convention = exactness.get("convention")
        if kind == "conventional":
            convention = _route_text("measurement conversion convention", convention)
        elif convention is not None:
            raise ArgumentError("exact measurement conversion cannot carry a convention")
        return cls(
            raw,
            _route_text("measurement conversion from", raw.get("from")),
            _route_text("measurement conversion to", raw.get("to")),
            _finite_number("measurement conversion factor", raw.get("factor")),
            kind,
            convention,
        )

    @property
    def exact(self) -> bool:
        return self.exactness == "exact"


@dataclass(frozen=True)
class MeasurementBlockedReasonReport:
    raw: dict[str, Any]
    blocked_by: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MeasurementBlockedReasonReport":
        raw = _route_mapping("measurement blocked reason", value)
        blocked_by = _route_text("measurement blocked_by", raw.get("blocked_by"))
        if blocked_by not in MEASUREMENT_BLOCKING_REASONS:
            raise ArgumentError(f"unknown measurement blocking reason: {blocked_by!r}")
        return cls(raw, blocked_by)

    @property
    def metadata_silence(self) -> bool:
        return self.blocked_by in {"unstated_frame", "unstated_build", "unbound_term", "unmapped_term"}


@dataclass(frozen=True)
class MeasurementVerdictReport:
    raw: dict[str, Any]
    verdict: str
    reason: MeasurementBlockedReasonReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MeasurementVerdictReport":
        raw = _route_mapping("measurement verdict", value)
        verdict = _route_text("measurement verdict kind", raw.get("verdict"))
        if verdict not in MEASUREMENT_VERDICTS:
            raise ArgumentError(f"unknown measurement verdict: {verdict!r}")
        reason_value = raw.get("reason")
        reason = MeasurementBlockedReasonReport.from_wire(reason_value) if reason_value is not None else None
        if verdict == "blocked" and reason is None:
            raise ArgumentError("blocked measurement verdict must carry its reason")
        if verdict == "comparable" and reason is not None:
            raise ArgumentError("comparable measurement verdict cannot carry a blocking reason")
        return cls(raw, verdict, reason)

    @property
    def comparable(self) -> bool:
        return self.verdict == "comparable"


@dataclass(frozen=True)
class MeasurementCompareReport:
    raw: dict[str, Any]
    ok: bool
    comparable: bool
    policy_require_bound_terms: bool
    left: str
    right: str
    verdict: MeasurementVerdictReport
    conversions: tuple[MeasurementConversionReport, ...]
    caveats: tuple[str, ...]
    report_sha256: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "MeasurementCompareReport":
        raw = _payload(value)
        required = ("ok", "comparable", "policy", "report", "report_sha256", "guarantees", "limitations")
        if any(key not in raw for key in required):
            raise ArgumentError("measurement compare response is missing a required projection")
        if not _bool("measurement compare ok", raw.get("ok")):
            raise ArgumentError("measurement compare report is not successful")
        comparable = _bool("measurement compare comparable", raw.get("comparable"))
        policy = _route_mapping("measurement compare policy", raw.get("policy"))
        require_bound_terms = _bool("measurement compare policy require_bound_terms", policy.get("require_bound_terms"))
        report = _route_mapping("measurement comparability report", raw.get("report"))
        verdict = MeasurementVerdictReport.from_wire(report.get("verdict"))
        if comparable != verdict.comparable:
            raise ArgumentError("measurement comparable does not reconcile with verdict")
        digest = _route_text("measurement compare report_sha256", raw.get("report_sha256"))
        if not _SHA256_RE.fullmatch(digest):
            raise ArgumentError("measurement compare report_sha256 must be a lowercase SHA-256 digest")
        conversions_value = report.get("conversions", [])
        if not isinstance(conversions_value, Sequence) or isinstance(conversions_value, (str, bytes)):
            raise ArgumentError("measurement conversions must be an array")
        conversions = tuple(MeasurementConversionReport.from_wire(item) for item in conversions_value if isinstance(item, Mapping))
        if len(conversions) != len(conversions_value):
            raise ArgumentError("measurement conversions must contain only objects")
        if not comparable and conversions:
            raise ArgumentError("blocked measurement comparison cannot carry conversions")
        return cls(
            raw,
            True,
            comparable,
            require_bound_terms,
            _route_text("measurement report left", report.get("left")),
            _route_text("measurement report right", report.get("right")),
            verdict,
            conversions,
            _route_strings("measurement report caveats", report.get("caveats", [])),
            digest,
            _route_strings("measurement compare guarantees", raw.get("guarantees")),
            _route_strings("measurement compare limitations", raw.get("limitations")),
        )

    @property
    def blocked(self) -> bool:
        return not self.comparable

    @property
    def report_digest(self) -> str:
        return self.report_sha256


def measurement_compare_report(value: Mapping[str, Any]) -> MeasurementCompareReport:
    """Parse direct MCP or HTTP measurement-comparability output."""

    return MeasurementCompareReport.from_wire(value)


__all__ = [
    "MEASUREMENT_BLOCKING_REASONS",
    "MEASUREMENT_VERDICTS",
    "MeasurementBlockedReasonReport",
    "MeasurementCompareArgs",
    "MeasurementCompareReport",
    "MeasurementConversionReport",
    "MeasurementVerdictReport",
    "measurement_compare_report",
]
