"""Typed assembled benchmark compiler projection.

The endpoint exposes the real benchcompiler pipeline while keeping the MCP boundary non-executing:
callers provide an exact table of observed probe signatures, and the server refuses if
minimization asks for a subset that table does not contain.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .benchmark_trace import BenchmarkTraceArgs
from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_COMPILE_SCHEMA = "bioprism-mcp/benchmark-compile/0.1"
MAX_BENCHMARK_COMPILE_CONTEXT = 5_000
MAX_BENCHMARK_COMPILE_OBSERVATIONS = 100_000
MAX_BENCHMARK_COMPILE_RECORDS = 10_000
MAX_BENCHMARK_COMPILE_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mapping_array(name: str, value: Any, *, limit: int) -> tuple[dict[str, Any], ...]:
    values = _array(name, value)
    if len(values) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    return tuple(dict(_route_mapping(f"{name}[{index}]", item)) for index, item in enumerate(values))


def _positive_int(name: str, value: Any, *, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= maximum:
        raise ArgumentError(f"{name} must be between 1 and {maximum}")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark compile response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BENCHMARK_COMPILE_SCHEMA and isinstance(candidate.get("compilation"), Mapping) and isinstance(candidate.get("probe"), Mapping)
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
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"benchmark compile response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark compile projection")


@dataclass(frozen=True)
class BenchmarkCompileArgs:
    trace: BenchmarkTraceArgs
    reference: BenchmarkTraceArgs | None = None
    context: tuple[Mapping[str, Any], ...] = ()
    probe_observations: tuple[Mapping[str, Any], ...] = ()
    budget: Mapping[str, Any] | None = None
    ledger: tuple[Mapping[str, Any], ...] = ()
    claims: tuple[Mapping[str, Any], ...] = ()

    def __post_init__(self) -> None:
        trace = self.trace if isinstance(self.trace, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.trace)
        reference = None if self.reference is None else (self.reference if isinstance(self.reference, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.reference))
        context = _mapping_array("benchmark compile context", self.context, limit=MAX_BENCHMARK_COMPILE_CONTEXT)
        observations = _mapping_array("benchmark compile probe_observations", self.probe_observations, limit=MAX_BENCHMARK_COMPILE_OBSERVATIONS)
        budget = None if self.budget is None else dict(_route_mapping("benchmark compile budget", self.budget))
        if budget is not None:
            _positive_int("benchmark compile budget.max_evaluations", budget.get("max_evaluations"), maximum=100_000)
        ledger = _mapping_array("benchmark compile ledger", self.ledger, limit=MAX_BENCHMARK_COMPILE_RECORDS)
        claims = _mapping_array("benchmark compile claims", self.claims, limit=MAX_BENCHMARK_COMPILE_RECORDS)
        arguments = {
            "trace": trace.to_wire(),
            "reference": None if reference is None else reference.to_wire(),
            "context": [dict(item) for item in context],
            "probe_observations": [dict(item) for item in observations],
            "budget": budget,
            "ledger": [dict(item) for item in ledger],
            "claims": [dict(item) for item in claims],
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark compile arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_BENCHMARK_COMPILE_INPUT_BYTES:
            raise ArgumentError("benchmark compile input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "trace", trace)
        object.__setattr__(self, "reference", reference)
        object.__setattr__(self, "context", context)
        object.__setattr__(self, "probe_observations", observations)
        object.__setattr__(self, "budget", budget)
        object.__setattr__(self, "ledger", ledger)
        object.__setattr__(self, "claims", claims)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCompileArgs":
        raw = _route_mapping("benchmark compile arguments", value)
        return cls(
            BenchmarkTraceArgs.from_wire(raw.get("trace")),
            None if raw.get("reference") is None else BenchmarkTraceArgs.from_wire(raw.get("reference")),
            _mapping_array("benchmark compile context", raw.get("context", []), limit=MAX_BENCHMARK_COMPILE_CONTEXT),
            _mapping_array("benchmark compile probe_observations", raw.get("probe_observations", []), limit=MAX_BENCHMARK_COMPILE_OBSERVATIONS),
            None if raw.get("budget") is None else _route_mapping("benchmark compile budget", raw.get("budget")),
            _mapping_array("benchmark compile ledger", raw.get("ledger", []), limit=MAX_BENCHMARK_COMPILE_RECORDS),
            _mapping_array("benchmark compile claims", raw.get("claims", []), limit=MAX_BENCHMARK_COMPILE_RECORDS),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "trace": self.trace.to_wire(),
            "context": [dict(item) for item in self.context],
            "probe_observations": [dict(item) for item in self.probe_observations],
            "ledger": [dict(item) for item in self.ledger],
            "claims": [dict(item) for item in self.claims],
        }
        if self.reference is not None:
            result["reference"] = self.reference.to_wire()
        if self.budget is not None:
            result["budget"] = dict(self.budget)
        return result


@dataclass(frozen=True)
class BenchmarkCompileReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    trace_id: str | None
    output_class: Mapping[str, Any] | None
    cell_step: int | None
    episodes: int | None
    boundary_count: int | None
    oracle: Mapping[str, Any] | None
    minimization: Mapping[str, Any] | None
    confidence: Mapping[str, Any] | None
    limiting_stage: Any
    unmeasured_stages: tuple[str, ...]
    probe: Mapping[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCompileReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark compile refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), _route_text("benchmark refusal trace_id", raw.get("trace_id")) if raw.get("trace_id") is not None else None, None, None, None, None, None, None, None, None, None, (), _route_strings("benchmark compile refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("benchmark compile refusal stage", raw.get("stage")), _route_text("benchmark compile refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_COMPILE_SCHEMA:
            raise ArgumentError("benchmark compile projection has an invalid schema")
        output_class = _route_mapping("benchmark compile class", raw.get("class"))
        for name in ("episodes", "boundary_count"):
            _route_count(f"benchmark compile {name}", raw.get(name))
        cell_step = raw.get("cell_step")
        if cell_step is not None and (isinstance(cell_step, bool) or not isinstance(cell_step, int) or cell_step < 0):
            raise ArgumentError("benchmark compile cell_step must be a non-negative integer or null")
        return cls(raw, True, BENCHMARK_COMPILE_SCHEMA, _route_text("benchmark compile trace_id", raw.get("trace_id")), output_class, cell_step, int(raw["episodes"]), int(raw["boundary_count"]), None if raw.get("oracle") is None else _route_mapping("benchmark compile oracle", raw.get("oracle")), None if raw.get("minimization") is None else _route_mapping("benchmark compile minimization", raw.get("minimization")), _route_mapping("benchmark compile confidence", raw.get("confidence")), raw.get("limiting_stage"), _route_strings("benchmark compile unmeasured stages", raw.get("unmeasured_stages", [])), _route_mapping("benchmark compile probe", raw.get("probe")), _route_strings("benchmark compile guarantees", raw.get("guarantees", [])), _route_strings("benchmark compile limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def class_name(self) -> str | None:
        return None if self.output_class is None else str(self.output_class.get("class"))

    @property
    def has_oracle(self) -> bool:
        return self.oracle is not None

    @property
    def reduction_ratio(self) -> float | None:
        return None if self.minimization is None else float(self.minimization["reduction_ratio"])

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_compile_report(value: Mapping[str, Any]) -> BenchmarkCompileReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkCompileReport.from_wire(value)


__all__ = [
    "BENCHMARK_COMPILE_SCHEMA",
    "MAX_BENCHMARK_COMPILE_CONTEXT",
    "MAX_BENCHMARK_COMPILE_OBSERVATIONS",
    "MAX_BENCHMARK_COMPILE_RECORDS",
    "MAX_BENCHMARK_COMPILE_INPUT_BYTES",
    "BenchmarkCompileArgs",
    "BenchmarkCompileReport",
    "benchmark_compile_report",
]
