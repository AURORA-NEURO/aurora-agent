"""Typed matched counterfactual cell validation and contrast.

The endpoint validates caller-constructed source/follow-up DecisionCells against one declared
intervention and grades two caller-supplied verdicts against the declared response.  It does not
apply mutations, execute a world, or claim realism review at the transport boundary.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_COUNTERFACTUAL_SCHEMA = "bioprism-mcp/benchmark-counterfactual/0.1"
COUNTERFACTUAL_OUTCOMES = frozenset({"as_predicted", "spurious_sensitivity", "missed_the_change", "wrong_direction"})
COUNTERFACTUAL_CELL_FIELDS = ("world", "query", "acceptable_verdicts", "required_witnesses", "require_protected_closure")
MAX_COUNTERFACTUAL_INPUT_BYTES = 20_000_000


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark counterfactual response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BENCHMARK_COUNTERFACTUAL_SCHEMA and isinstance(candidate.get("pair"), Mapping) and isinstance(candidate.get("outcome"), Mapping)
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
                        raise ArgumentError(f"benchmark counterfactual response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark counterfactual projection")


@dataclass(frozen=True)
class BenchmarkCounterfactualCheckArgs:
    source: Mapping[str, Any]
    followup: Mapping[str, Any]
    intervention: Mapping[str, Any]
    expected: Mapping[str, Any]
    source_verdict: str
    followup_verdict: str

    def __post_init__(self) -> None:
        source = dict(_route_mapping("benchmark counterfactual source", self.source))
        followup = dict(_route_mapping("benchmark counterfactual followup", self.followup))
        intervention = dict(_route_mapping("benchmark counterfactual intervention", self.intervention))
        expected = dict(_route_mapping("benchmark counterfactual expected", self.expected))
        source_verdict = _route_text("benchmark source_verdict", self.source_verdict)
        followup_verdict = _route_text("benchmark followup_verdict", self.followup_verdict)
        if not source_verdict or not followup_verdict:
            raise ArgumentError("benchmark source_verdict and followup_verdict must not be empty")
        arguments = {"source": source, "followup": followup, "intervention": intervention, "expected": expected, "source_verdict": source_verdict, "followup_verdict": followup_verdict}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark counterfactual arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_COUNTERFACTUAL_INPUT_BYTES:
            raise ArgumentError("benchmark counterfactual input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "source", source)
        object.__setattr__(self, "followup", followup)
        object.__setattr__(self, "intervention", intervention)
        object.__setattr__(self, "expected", expected)
        object.__setattr__(self, "source_verdict", source_verdict)
        object.__setattr__(self, "followup_verdict", followup_verdict)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCounterfactualCheckArgs":
        raw = _route_mapping("benchmark counterfactual arguments", value)
        return cls(
            _route_mapping("benchmark counterfactual source", raw.get("source")),
            _route_mapping("benchmark counterfactual followup", raw.get("followup")),
            _route_mapping("benchmark counterfactual intervention", raw.get("intervention")),
            _route_mapping("benchmark counterfactual expected", raw.get("expected")),
            _route_text("benchmark source_verdict", raw.get("source_verdict")),
            _route_text("benchmark followup_verdict", raw.get("followup_verdict")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "source": dict(self.source),
            "followup": dict(self.followup),
            "intervention": dict(self.intervention),
            "expected": dict(self.expected),
            "source_verdict": self.source_verdict,
            "followup_verdict": self.followup_verdict,
        }


@dataclass(frozen=True)
class BenchmarkCounterfactualCheckReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    pair: Mapping[str, Any] | None
    outcome: Mapping[str, Any] | None
    satisfied: bool | None
    source_verdict: str | None
    followup_verdict: str | None
    cell_digests: Mapping[str, str]
    allowed_cell_fields: tuple[str, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCounterfactualCheckReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark counterfactual refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, None, None, {}, tuple(), _route_strings("counterfactual refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("counterfactual refusal stage", raw.get("stage")), _route_text("counterfactual refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_COUNTERFACTUAL_SCHEMA:
            raise ArgumentError("benchmark counterfactual projection has an invalid schema")
        outcome = _route_mapping("benchmark counterfactual outcome", raw.get("outcome"))
        outcome_kind = _route_text("benchmark counterfactual outcome kind", outcome.get("outcome"))
        if outcome_kind not in COUNTERFACTUAL_OUTCOMES:
            raise ArgumentError(f"unknown benchmark counterfactual outcome {outcome_kind!r}")
        satisfied = raw.get("satisfied")
        if not isinstance(satisfied, bool):
            raise ArgumentError("benchmark counterfactual satisfied must be a boolean")
        digests_raw = _route_mapping("benchmark counterfactual cell_digests", raw.get("cell_digests"))
        digests = {key: _route_text(f"benchmark counterfactual digest {key}", value) for key, value in digests_raw.items()}
        if set(digests) != {"source", "followup"}:
            raise ArgumentError("benchmark counterfactual cell_digests must contain source and followup")
        allowed = _route_strings("benchmark allowed cell fields", raw.get("allowed_cell_fields", []))
        if tuple(allowed) != COUNTERFACTUAL_CELL_FIELDS:
            raise ArgumentError("benchmark allowed cell fields do not match the declared compiler contract")
        return cls(raw, True, BENCHMARK_COUNTERFACTUAL_SCHEMA, _route_mapping("benchmark counterfactual pair", raw.get("pair")), outcome, satisfied, _route_text("benchmark source_verdict", raw.get("source_verdict")), _route_text("benchmark followup_verdict", raw.get("followup_verdict")), digests, allowed, _route_strings("benchmark counterfactual guarantees", raw.get("guarantees", [])), _route_strings("benchmark counterfactual limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def outcome_kind(self) -> str | None:
        return None if self.outcome is None else str(self.outcome.get("outcome"))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_counterfactual_check_report(value: Mapping[str, Any]) -> BenchmarkCounterfactualCheckReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkCounterfactualCheckReport.from_wire(value)


__all__ = [
    "BENCHMARK_COUNTERFACTUAL_SCHEMA",
    "COUNTERFACTUAL_OUTCOMES",
    "COUNTERFACTUAL_CELL_FIELDS",
    "MAX_COUNTERFACTUAL_INPUT_BYTES",
    "BenchmarkCounterfactualCheckArgs",
    "BenchmarkCounterfactualCheckReport",
    "benchmark_counterfactual_check_report",
]
