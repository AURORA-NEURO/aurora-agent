"""Typed projections for the executable FIBER decision contract.

`fiber-query/0.3` adds a decision-relative quotient summary and `fiber-query/0.4` adds a full
bounded rate-distortion summary to ``fiber_compile``. This module validates those projections
without pretending the progressive-disclosure response contains the full certificate. The Rust
compiler remains authoritative; Python only makes the published MCP projections safe and
convenient to consume.
"""

from __future__ import annotations

import json
import math
import re
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


FIBER_DECISION_QUOTIENT_SCHEMA = "bioprism-mcp/epistemic-decision-quotient/0.1"
FIBER_DECISION_QUOTIENT_BASIS = "permitted_loss_difference_profile"
FIBER_DECISION_MAX_ACTIONS = 1_000
FIBER_RATE_DISTORTION_SCHEMA = "bioprism-mcp/epistemic-context-audit/0.2"
FIBER_RATE_DISTORTION_MAX_EVIDENCE = 16
_DIGEST = re.compile(r"^[0-9a-f]{64}$")


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if not _DIGEST.fullmatch(text):
        raise ArgumentError(f"{name} must be a lowercase 64-character SHA-256 digest")
    return text


def _candidate_payloads(value: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    raw = _route_mapping("fiber compile response", value)
    candidates: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        candidates.append(mcp)
        result = mcp.get("result")
        if isinstance(result, Mapping):
            candidates.append(result)
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"fiber compile response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    return candidates


@dataclass(frozen=True)
class FiberDecisionQuotientSummary:
    """Validated L0 quotient summary returned by ``fiber_compile``."""

    raw: dict[str, Any]
    schema: str
    basis: str
    permitted_actions: tuple[str, ...]
    original_model_count: int
    quotient_model_count: int
    merged_model_count: int
    compressed: bool
    compression_fraction: float
    query_sha256: str
    certificate_sha256: str
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FiberDecisionQuotientSummary":
        summary: Mapping[str, Any] | None = None
        for candidate in _candidate_payloads(value):
            possible = candidate.get("decision_quotient")
            if isinstance(possible, Mapping):
                summary = possible
                break
        if summary is None:
            raise ArgumentError("response does not contain a fiber decision quotient summary")

        schema = _route_text("fiber decision quotient schema", summary.get("schema"))
        if schema != FIBER_DECISION_QUOTIENT_SCHEMA:
            raise ArgumentError("fiber decision quotient summary has an invalid schema")
        basis = _route_text("fiber decision quotient basis", summary.get("basis"))
        if basis != FIBER_DECISION_QUOTIENT_BASIS:
            raise ArgumentError("fiber decision quotient summary has an invalid basis")
        actions = _route_strings("fiber decision quotient permitted actions", summary.get("permitted_actions"))
        if not 1 <= len(actions) <= FIBER_DECISION_MAX_ACTIONS or tuple(actions) != tuple(sorted(actions)) or len(actions) != len(set(actions)):
            raise ArgumentError("fiber decision quotient permitted actions must be non-empty, unique, and canonical")
        original = _count("fiber decision quotient original model count", summary.get("original_model_count"))
        quotient = _count("fiber decision quotient model count", summary.get("quotient_model_count"))
        merged = _count("fiber decision quotient merged model count", summary.get("merged_model_count"))
        if original == 0 or quotient == 0 or quotient > original or merged != original - quotient:
            raise ArgumentError("fiber decision quotient counts do not reconcile")
        compressed = summary.get("compressed")
        if not isinstance(compressed, bool) or compressed != (quotient < original):
            raise ArgumentError("fiber decision quotient compressed flag does not reconcile")
        fraction = _finite("fiber decision quotient compression fraction", summary.get("compression_fraction"))
        if fraction != quotient / original:
            raise ArgumentError("fiber decision quotient compression fraction does not reconcile")
        binding = _route_mapping("fiber decision quotient certificate binding", summary.get("certificate_binding"))
        limitations = _route_strings("fiber decision quotient limitations", summary.get("limitations", []))
        return cls(
            dict(summary),
            schema,
            basis,
            tuple(actions),
            original,
            quotient,
            merged,
            compressed,
            fraction,
            _digest("fiber decision quotient query_sha256", binding.get("query_sha256")),
            _digest("fiber decision quotient certificate_sha256", binding.get("certificate_sha256")),
            tuple(limitations),
        )

    @property
    def refused(self) -> bool:
        """This projection is present only for an accepted compile."""

        return False

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def fiber_decision_quotient_summary(value: Mapping[str, Any]) -> FiberDecisionQuotientSummary:
    """Parse direct MCP output or an HTTP REST tool envelope from ``fiber_compile``."""

    return FiberDecisionQuotientSummary.from_wire(value)


@dataclass(frozen=True)
class FiberRateDistortionSummary:
    """Validated L0 observed-context rate-distortion summary from ``fiber_compile``."""

    raw: dict[str, Any]
    schema: str
    criterion: str
    tolerance: float
    compatibility_floor: float
    evidence_count: int
    full_rate: float
    identification: dict[str, Any]
    sufficiency: dict[str, Any]
    frontier: dict[str, Any]
    query_sha256: str
    certificate_sha256: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "FiberRateDistortionSummary":
        summary: Mapping[str, Any] | None = None
        for candidate in _candidate_payloads(value):
            possible = candidate.get("rate_distortion")
            if isinstance(possible, Mapping):
                summary = possible
                break
        if summary is None:
            raise ArgumentError("response does not contain a fiber rate-distortion summary")

        schema = _route_text("fiber rate-distortion schema", summary.get("schema"))
        if schema != FIBER_RATE_DISTORTION_SCHEMA:
            raise ArgumentError("fiber rate-distortion summary has an invalid schema")
        criterion = _route_text("fiber rate-distortion criterion", summary.get("criterion"))
        if criterion not in {"bayes_regret", "minimax_regret"}:
            raise ArgumentError("fiber rate-distortion criterion is invalid")
        tolerance = _finite("fiber rate-distortion tolerance", summary.get("tolerance"))
        if tolerance < 0.0:
            raise ArgumentError("fiber rate-distortion tolerance must be non-negative")
        floor = _finite("fiber rate-distortion compatibility floor", summary.get("compatibility_floor"))
        if not 0.0 <= floor <= 1.0:
            raise ArgumentError("fiber rate-distortion compatibility floor must be between 0 and 1")
        evidence_count = _count("fiber rate-distortion evidence count", summary.get("evidence_count"))
        if evidence_count > FIBER_RATE_DISTORTION_MAX_EVIDENCE:
            raise ArgumentError("fiber rate-distortion evidence count exceeds the exhaustive bound")
        full_rate = _finite("fiber rate-distortion full rate", summary.get("full_rate"))
        if full_rate < 0.0:
            raise ArgumentError("fiber rate-distortion full rate must be non-negative")
        identification = dict(_route_mapping("fiber rate-distortion identification", summary.get("identification")))
        sufficiency = dict(_route_mapping("fiber rate-distortion sufficiency", summary.get("sufficiency")))
        frontier = dict(_route_mapping("fiber rate-distortion frontier", summary.get("frontier")))
        evaluated = _count("fiber rate-distortion evaluated contexts", frontier.get("evaluated"))
        if evaluated == 0 or evaluated > (1 << FIBER_RATE_DISTORTION_MAX_EVIDENCE):
            raise ArgumentError("fiber rate-distortion frontier evaluated count is outside the exhaustive bound")
        binding = _route_mapping("fiber rate-distortion certificate binding", summary.get("certificate_binding"))
        guarantees = _route_strings("fiber rate-distortion guarantees", summary.get("guarantees", []))
        limitations = _route_strings("fiber rate-distortion limitations", summary.get("limitations", []))
        return cls(
            dict(summary),
            schema,
            criterion,
            tolerance,
            floor,
            evidence_count,
            full_rate,
            identification,
            sufficiency,
            frontier,
            _digest("fiber rate-distortion query_sha256", binding.get("query_sha256")),
            _digest("fiber rate-distortion certificate_sha256", binding.get("certificate_sha256")),
            tuple(guarantees),
            tuple(limitations),
        )

    @property
    def refused(self) -> bool:
        return False

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def fiber_rate_distortion_summary(value: Mapping[str, Any]) -> FiberRateDistortionSummary:
    """Parse direct MCP output or an HTTP REST tool envelope from ``fiber_compile``."""

    return FiberRateDistortionSummary.from_wire(value)


__all__ = [
    "FIBER_DECISION_QUOTIENT_SCHEMA",
    "FIBER_DECISION_QUOTIENT_BASIS",
    "FIBER_RATE_DISTORTION_SCHEMA",
    "FIBER_RATE_DISTORTION_MAX_EVIDENCE",
    "FiberDecisionQuotientSummary",
    "FiberRateDistortionSummary",
    "fiber_decision_quotient_summary",
    "fiber_rate_distortion_summary",
]
