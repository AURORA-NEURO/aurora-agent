"""Typed end-to-end benchmark compilation, review, grading, and cell packaging."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from .benchmark_compile import BenchmarkCompileArgs
from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_COMPILE_REVIEW_SCHEMA = "bioprism-mcp/benchmark-compile-review/0.1"
ORACLE_ACCEPTANCE_OUTCOMES = frozenset({"passed", "wrong_verdict", "missing_witnesses", "closure_incomplete"})
MAX_BENCHMARK_COMPILE_REVIEW_INPUT_BYTES = 20_000_000


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark compile review response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BENCHMARK_COMPILE_REVIEW_SCHEMA and isinstance(candidate.get("compile"), Mapping) and isinstance(candidate.get("reviewed_oracle"), Mapping) and isinstance(candidate.get("cell"), Mapping)
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
                        raise ArgumentError(f"benchmark compile review response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark compile review projection")


@dataclass(frozen=True)
class BenchmarkCompileReviewArgs:
    compile: BenchmarkCompileArgs
    reviewer: str
    world: Mapping[str, Any]
    query: Mapping[str, Any]
    grade: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        compile_args = self.compile if isinstance(self.compile, BenchmarkCompileArgs) else BenchmarkCompileArgs.from_wire(self.compile)
        reviewer = _route_text("benchmark compile reviewer", self.reviewer)
        if not reviewer.strip():
            raise ArgumentError("benchmark compile reviewer must not be empty")
        world = dict(_route_mapping("benchmark compile review world", self.world))
        query = dict(_route_mapping("benchmark compile review query", self.query))
        grade = None if self.grade is None else dict(_route_mapping("benchmark compile review grade", self.grade))
        arguments = compile_args.to_mcp_arguments() | {"reviewer": reviewer, "world": world, "query": query, "grade": grade}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark compile review arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_BENCHMARK_COMPILE_REVIEW_INPUT_BYTES:
            raise ArgumentError("benchmark compile review input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "compile", compile_args)
        object.__setattr__(self, "reviewer", reviewer)
        object.__setattr__(self, "world", world)
        object.__setattr__(self, "query", query)
        object.__setattr__(self, "grade", grade)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCompileReviewArgs":
        raw = _route_mapping("benchmark compile review arguments", value)
        return cls(
            BenchmarkCompileArgs.from_wire(raw),
            _route_text("benchmark compile reviewer", raw.get("reviewer")),
            _route_mapping("benchmark compile review world", raw.get("world")),
            _route_mapping("benchmark compile review query", raw.get("query")),
            None if raw.get("grade") is None else _route_mapping("benchmark compile review grade", raw.get("grade")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result = self.compile.to_mcp_arguments()
        result.update({"reviewer": self.reviewer, "world": dict(self.world), "query": dict(self.query)})
        if self.grade is not None:
            result["grade"] = dict(self.grade)
        return result


@dataclass(frozen=True)
class BenchmarkCompileReviewReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    compile: Mapping[str, Any] | None
    reviewed_oracle: Mapping[str, Any] | None
    reviewer: str | None
    review_digest: str | None
    grade: Mapping[str, Any] | None
    cell: Mapping[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCompileReviewReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark compile review refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("compile") if isinstance(raw.get("compile"), Mapping) else None, None, _route_text("compile review refusal reviewer", raw.get("reviewer")) if raw.get("reviewer") is not None else None, None, None, None, _route_strings("compile review refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("compile review refusal stage", raw.get("stage")), _route_text("compile review refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_COMPILE_REVIEW_SCHEMA:
            raise ArgumentError("benchmark compile review projection has an invalid schema")
        reviewer = _route_text("benchmark compile review reviewer", raw.get("reviewer"))
        digest = _route_text("benchmark compile review digest", raw.get("review_digest"))
        if len(digest) != 64:
            raise ArgumentError("benchmark compile review digest must be a SHA-256 hex digest")
        grade = None if raw.get("grade") is None else _route_mapping("benchmark compile review grade", raw.get("grade"))
        if grade is not None:
            acceptance = _route_mapping("benchmark compile review acceptance", grade.get("acceptance"))
            if _route_text("benchmark compile review acceptance outcome", acceptance.get("outcome")) not in ORACLE_ACCEPTANCE_OUTCOMES:
                raise ArgumentError("benchmark compile review acceptance outcome is invalid")
            if not isinstance(grade.get("passed"), bool):
                raise ArgumentError("benchmark compile review grade passed must be a boolean")
        return cls(raw, True, BENCHMARK_COMPILE_REVIEW_SCHEMA, _route_mapping("benchmark compile review compile", raw.get("compile")), _route_mapping("benchmark compile reviewed oracle", raw.get("reviewed_oracle")), reviewer, digest, grade, _route_mapping("benchmark compile review cell", raw.get("cell")), _route_strings("benchmark compile review guarantees", raw.get("guarantees", [])), _route_strings("benchmark compile review limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

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


def benchmark_compile_review_report(value: Mapping[str, Any]) -> BenchmarkCompileReviewReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkCompileReviewReport.from_wire(value)


__all__ = [
    "BENCHMARK_COMPILE_REVIEW_SCHEMA",
    "MAX_BENCHMARK_COMPILE_REVIEW_INPUT_BYTES",
    "BenchmarkCompileReviewArgs",
    "BenchmarkCompileReviewReport",
    "benchmark_compile_review_report",
]
