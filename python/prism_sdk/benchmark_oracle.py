"""Typed oracle review, grading, and DecisionCell packaging.

The Rust kernel intentionally makes ``ProposedOracle`` and ``ReviewedOracle`` different types.
This module preserves that boundary over MCP/HTTP: a proposal is ordinary JSON, while a report
only treats the server's reviewed projection as evidence after the server has run its gate.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_ORACLE_REVIEW_SCHEMA = "bioprism-mcp/benchmark-oracle-review/0.1"
ORACLE_ACCEPTANCE_OUTCOMES = frozenset({"passed", "wrong_verdict", "missing_witnesses", "closure_incomplete"})
MAX_ORACLE_REVIEW_INPUT_BYTES = 20_000_000


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark oracle review response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BENCHMARK_ORACLE_REVIEW_SCHEMA and isinstance(candidate.get("reviewed_oracle"), Mapping)
        return candidate.get("ok") is False and isinstance(candidate.get("stage"), str) and isinstance(candidate.get("refusal"), str)

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
            if isinstance(content, list):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"benchmark oracle review response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark oracle review projection")


@dataclass(frozen=True)
class BenchmarkOracleReviewArgs:
    proposal: Mapping[str, Any]
    reviewer: str
    grade: Mapping[str, Any] | None = None
    cell: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        proposal = dict(_route_mapping("benchmark oracle proposal", self.proposal))
        reviewer = _route_text("benchmark oracle reviewer", self.reviewer)
        if not reviewer.strip():
            raise ArgumentError("benchmark oracle reviewer must not be empty")
        grade = None if self.grade is None else dict(_route_mapping("benchmark oracle grade", self.grade))
        cell = None if self.cell is None else dict(_route_mapping("benchmark oracle cell", self.cell))
        arguments = {"proposal": proposal, "reviewer": reviewer, "grade": grade, "cell": cell}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark oracle review arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_ORACLE_REVIEW_INPUT_BYTES:
            raise ArgumentError("benchmark oracle review input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "proposal", proposal)
        object.__setattr__(self, "reviewer", reviewer)
        object.__setattr__(self, "grade", grade)
        object.__setattr__(self, "cell", cell)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkOracleReviewArgs":
        raw = _route_mapping("benchmark oracle review arguments", value)
        return cls(
            _route_mapping("benchmark oracle proposal", raw.get("proposal")),
            _route_text("benchmark oracle reviewer", raw.get("reviewer")),
            None if raw.get("grade") is None else _route_mapping("benchmark oracle grade", raw.get("grade")),
            None if raw.get("cell") is None else _route_mapping("benchmark oracle cell", raw.get("cell")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"proposal": dict(self.proposal), "reviewer": self.reviewer}
        if self.grade is not None:
            arguments["grade"] = dict(self.grade)
        if self.cell is not None:
            arguments["cell"] = dict(self.cell)
        return arguments


@dataclass(frozen=True)
class BenchmarkOracleReviewReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    proposal: Mapping[str, Any] | None
    reviewed_oracle: Mapping[str, Any] | None
    reviewer: str | None
    review_digest: str | None
    strength: str | None
    deterministic: bool | None
    grade: Mapping[str, Any] | None
    cell: Mapping[str, Any] | None
    synthesis_order: tuple[str, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkOracleReviewReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark oracle review refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("proposal") if isinstance(raw.get("proposal"), Mapping) else None, None, _route_text("oracle refusal reviewer", raw.get("reviewer")) if raw.get("reviewer") is not None else None, None, None, None, None, None, _route_strings("oracle refusal synthesis order", raw.get("synthesis_order", [])), _route_strings("oracle refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("oracle refusal stage", raw.get("stage")), _route_text("oracle refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_ORACLE_REVIEW_SCHEMA:
            raise ArgumentError("benchmark oracle review projection has an invalid schema")
        deterministic = raw.get("deterministic")
        if not isinstance(deterministic, bool):
            raise ArgumentError("benchmark oracle deterministic must be a boolean")
        reviewer = _route_text("benchmark oracle reviewer", raw.get("reviewer"))
        review_digest = _route_text("benchmark oracle review digest", raw.get("review_digest"))
        if len(review_digest) != 64:
            raise ArgumentError("benchmark oracle review digest must be a SHA-256 hex digest")
        grade = None if raw.get("grade") is None else _route_mapping("benchmark oracle grade", raw.get("grade"))
        if grade is not None:
            acceptance = _route_mapping("benchmark oracle acceptance", grade.get("acceptance"))
            outcome = _route_text("benchmark oracle acceptance outcome", acceptance.get("outcome"))
            if outcome not in ORACLE_ACCEPTANCE_OUTCOMES:
                raise ArgumentError(f"unknown benchmark oracle acceptance outcome {outcome!r}")
            if not isinstance(grade.get("passed"), bool):
                raise ArgumentError("benchmark oracle grade passed must be a boolean")
        cell = None if raw.get("cell") is None else _route_mapping("benchmark oracle cell", raw.get("cell"))
        return cls(raw, True, BENCHMARK_ORACLE_REVIEW_SCHEMA, _route_mapping("benchmark oracle proposal", raw.get("proposal")), _route_mapping("benchmark reviewed oracle", raw.get("reviewed_oracle")), reviewer, review_digest, _route_text("benchmark oracle strength", raw.get("strength")), deterministic, grade, cell, _route_strings("benchmark oracle synthesis order", raw.get("synthesis_order", [])), _route_strings("benchmark oracle guarantees", raw.get("guarantees", [])), _route_strings("benchmark oracle limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def graded(self) -> bool:
        return self.grade is not None

    @property
    def packaged(self) -> bool:
        return self.cell is not None

    @property
    def passed(self) -> bool | None:
        return None if self.grade is None else bool(self.grade.get("passed"))

    @property
    def acceptance_outcome(self) -> str | None:
        if self.grade is None or not isinstance(self.grade.get("acceptance"), Mapping):
            return None
        return str(self.grade["acceptance"].get("outcome"))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_oracle_review_report(value: Mapping[str, Any]) -> BenchmarkOracleReviewReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkOracleReviewReport.from_wire(value)


__all__ = [
    "BENCHMARK_ORACLE_REVIEW_SCHEMA",
    "ORACLE_ACCEPTANCE_OUTCOMES",
    "MAX_ORACLE_REVIEW_INPUT_BYTES",
    "BenchmarkOracleReviewArgs",
    "BenchmarkOracleReviewReport",
    "benchmark_oracle_review_report",
]
