"""Typed decision-cell audits over the benchmark compiler.

The decision audit is deliberately more specific than benchmark trace analysis: it selects one
choice/action step, reconstructs the options that were recorded there, applies the hindsight
firewall, and attaches the causal failure card.  This module keeps those layers visible to Python
callers instead of reducing them to a single score.  Bounded projections retain omission counts,
and fail-closed server refusals remain refusals here.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .benchmark_trace import BenchmarkTraceArgs
from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_DECISION_AUDIT_SCHEMA = "bioprism-mcp/benchmark-decision-audit/0.1"
MAX_DECISION_AUDIT_ITEMS = 1_000
MAX_DECISION_AUDIT_ACTIONS = 10_000
MAX_DECISION_AUDIT_RECORDS = 10_000
MAX_DECISION_AUDIT_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _mapping_array(name: str, value: Any, *, limit: int) -> tuple[dict[str, Any], ...]:
    values = _array(name, value)
    if len(values) > limit:
        raise ArgumentError(f"{name} is bounded at {limit} items")
    result: list[dict[str, Any]] = []
    for index, item in enumerate(values):
        result.append(dict(_route_mapping(f"{name}[{index}]", item)))
    return tuple(result)


def _index(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _optional_index(name: str, value: Any) -> int | None:
    return None if value is None else _index(name, value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ArgumentError(f"{name} must be a finite number")
    return parsed


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("benchmark decision audit response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return (
                candidate.get("schema") == BENCHMARK_DECISION_AUDIT_SCHEMA
                and isinstance(candidate.get("decision"), Mapping)
                and isinstance(candidate.get("failure_card"), Mapping)
            )
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
                        raise ArgumentError(f"benchmark decision audit response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark decision audit projection")


@dataclass(frozen=True)
class BenchmarkDecisionAuditArgs:
    """Trace, optional reference, and bounded caller-supplied audit evidence."""

    trace: BenchmarkTraceArgs
    reference: BenchmarkTraceArgs | None = None
    decision_step: int | None = None
    actions: tuple[Mapping[str, Any], ...] = ()
    constraints: tuple[Mapping[str, Any], ...] = ()
    claims: tuple[Mapping[str, Any], ...] = ()
    evaluator_dispute: str | None = None
    max_items: int = 100

    def __post_init__(self) -> None:
        trace = self.trace if isinstance(self.trace, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.trace)
        reference = None if self.reference is None else (self.reference if isinstance(self.reference, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.reference))
        decision_step = _optional_index("benchmark decision_step", self.decision_step)
        actions = _mapping_array("benchmark candidate actions", self.actions, limit=MAX_DECISION_AUDIT_ACTIONS)
        constraints = _mapping_array("benchmark constraints", self.constraints, limit=MAX_DECISION_AUDIT_RECORDS)
        claims = _mapping_array("benchmark claims", self.claims, limit=MAX_DECISION_AUDIT_RECORDS)
        evaluator_dispute = None if self.evaluator_dispute is None else _route_text("benchmark evaluator_dispute", self.evaluator_dispute)
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= MAX_DECISION_AUDIT_ITEMS:
            raise ArgumentError("benchmark decision max_items must be between 1 and 1000")
        arguments = {
            "trace": trace.to_wire(),
            "reference": None if reference is None else reference.to_wire(),
            "decision_step": decision_step,
            "actions": [dict(item) for item in actions],
            "constraints": [dict(item) for item in constraints],
            "claims": [dict(item) for item in claims],
            "evaluator_dispute": evaluator_dispute,
            "max_items": self.max_items,
        }
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark decision audit arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_DECISION_AUDIT_INPUT_BYTES:
            raise ArgumentError("benchmark decision audit input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "trace", trace)
        object.__setattr__(self, "reference", reference)
        object.__setattr__(self, "decision_step", decision_step)
        object.__setattr__(self, "actions", actions)
        object.__setattr__(self, "constraints", constraints)
        object.__setattr__(self, "claims", claims)
        object.__setattr__(self, "evaluator_dispute", evaluator_dispute)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkDecisionAuditArgs":
        raw = _route_mapping("benchmark decision audit arguments", value)
        return cls(
            BenchmarkTraceArgs.from_wire(raw.get("trace")),
            None if raw.get("reference") is None else BenchmarkTraceArgs.from_wire(raw.get("reference")),
            _optional_index("benchmark decision_step", raw.get("decision_step")),
            _mapping_array("benchmark candidate actions", raw.get("actions", []), limit=MAX_DECISION_AUDIT_ACTIONS),
            _mapping_array("benchmark constraints", raw.get("constraints", []), limit=MAX_DECISION_AUDIT_RECORDS),
            _mapping_array("benchmark claims", raw.get("claims", []), limit=MAX_DECISION_AUDIT_RECORDS),
            None if raw.get("evaluator_dispute") is None else _route_text("benchmark evaluator_dispute", raw.get("evaluator_dispute")),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "trace": self.trace.to_wire(),
            "actions": [dict(item) for item in self.actions],
            "constraints": [dict(item) for item in self.constraints],
            "claims": [dict(item) for item in self.claims],
            "max_items": self.max_items,
        }
        if self.reference is not None:
            result["reference"] = self.reference.to_wire()
        if self.decision_step is not None:
            result["decision_step"] = self.decision_step
        if self.evaluator_dispute is not None:
            result["evaluator_dispute"] = self.evaluator_dispute
        return result


@dataclass(frozen=True)
class BenchmarkDecisionCoverageReport:
    raw: dict[str, Any]
    total: int
    visible_at_decision_time: int
    validation_only: int
    feasible: int
    strong: int
    plausible_wrong_alternatives: int
    adequate: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkDecisionCoverageReport":
        raw = _route_mapping("benchmark decision coverage", value)
        counts = {
            key: _route_count(f"benchmark coverage {key}", raw.get(key))
            for key in ("total", "visible_at_decision_time", "validation_only", "feasible", "strong", "plausible_wrong_alternatives")
        }
        adequate = raw.get("adequate")
        if not isinstance(adequate, bool):
            raise ArgumentError("benchmark coverage adequate must be a boolean")
        if counts["visible_at_decision_time"] + counts["validation_only"] != counts["total"]:
            raise ArgumentError("benchmark coverage visibility counts do not reconcile")
        if counts["strong"] > counts["feasible"] or counts["plausible_wrong_alternatives"] > counts["feasible"]:
            raise ArgumentError("benchmark coverage strength counts exceed feasible options")
        return cls(raw, counts["total"], counts["visible_at_decision_time"], counts["validation_only"], counts["feasible"], counts["strong"], counts["plausible_wrong_alternatives"], adequate)


@dataclass(frozen=True)
class BenchmarkFailureCardReport:
    raw: dict[str, Any]
    blame: Mapping[str, Any]
    evidence_ratio: float
    findings: tuple[Mapping[str, Any], ...]
    hypotheses: tuple[Mapping[str, Any], ...]
    violated_constraints: tuple[Mapping[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkFailureCardReport":
        raw = _route_mapping("benchmark failure card", value)
        blame = _route_mapping("benchmark failure card blame", raw.get("blame"))
        evidence_ratio = _finite("benchmark failure card evidence_ratio", raw.get("evidence_ratio"))
        if not 0.0 <= evidence_ratio <= 1.0:
            raise ArgumentError("benchmark failure card evidence_ratio must be between 0 and 1")
        return cls(
            raw,
            blame,
            evidence_ratio,
            _mapping_array("benchmark failure findings", raw.get("findings", []), limit=MAX_DECISION_AUDIT_ITEMS),
            _mapping_array("benchmark failure hypotheses", raw.get("hypotheses", []), limit=MAX_DECISION_AUDIT_ITEMS),
            _mapping_array("benchmark violated constraints", raw.get("violated_constraints", []), limit=MAX_DECISION_AUDIT_ITEMS),
        )


@dataclass(frozen=True)
class BenchmarkDecisionAuditReport:
    """Bounded causal, action-firewall, coverage, and attribution evidence."""

    raw: dict[str, Any]
    ok: bool
    schema: str | None
    trace_id: str | None
    selected_step: int | None
    causal_step: int | None
    causal_alignment: str | None
    coverage: BenchmarkDecisionCoverageReport | None
    failure_card: BenchmarkFailureCardReport | None
    action_counts: Mapping[str, int]
    analysis: Mapping[str, Any] | None
    guarantees: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkDecisionAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark decision audit refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, None, None, None, {}, None, _route_strings("benchmark decision refusal guarantees", raw.get("guarantees", [])), _route_text("benchmark decision refusal stage", raw.get("stage")), _route_text("benchmark decision refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != BENCHMARK_DECISION_AUDIT_SCHEMA:
            raise ArgumentError("benchmark decision audit projection has an invalid schema")
        decision = _route_mapping("benchmark decision projection", raw.get("decision"))
        selected_step = _index("benchmark selected_step", decision.get("selected_step"))
        causal_step = _optional_index("benchmark causal_step", decision.get("causal_step"))
        alignment = _route_text("benchmark causal_alignment", decision.get("causal_alignment"))
        if alignment not in {"aligned", "explicit_override"}:
            raise ArgumentError("benchmark causal_alignment is invalid")
        counts_raw = _route_mapping("benchmark action counts", decision.get("action_counts"))
        action_counts = {key: _route_count(f"benchmark action count {key}", counts_raw.get(key)) for key in ("all", "visible_to_agent", "validation_only", "acceptable")}
        if action_counts["visible_to_agent"] + action_counts["validation_only"] != action_counts["all"]:
            raise ArgumentError("benchmark action counts do not reconcile")
        return cls(
            raw,
            True,
            BENCHMARK_DECISION_AUDIT_SCHEMA,
            _route_text("benchmark decision trace_id", raw.get("trace_id")),
            selected_step,
            causal_step,
            alignment,
            BenchmarkDecisionCoverageReport.from_wire(decision.get("coverage")),
            BenchmarkFailureCardReport.from_wire(raw.get("failure_card")),
            action_counts,
            _route_mapping("benchmark decision analysis", raw.get("analysis")),
            _route_strings("benchmark decision guarantees", raw.get("guarantees", [])),
            None,
            None,
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def localized(self) -> bool:
        return self.causal_step is not None

    @property
    def visible_action_count(self) -> int:
        return self.action_counts.get("visible_to_agent", 0)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_decision_audit_report(value: Mapping[str, Any]) -> BenchmarkDecisionAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkDecisionAuditReport.from_wire(value)


__all__ = [
    "BENCHMARK_DECISION_AUDIT_SCHEMA",
    "MAX_DECISION_AUDIT_ITEMS",
    "MAX_DECISION_AUDIT_ACTIONS",
    "MAX_DECISION_AUDIT_RECORDS",
    "MAX_DECISION_AUDIT_INPUT_BYTES",
    "BenchmarkDecisionAuditArgs",
    "BenchmarkDecisionCoverageReport",
    "BenchmarkFailureCardReport",
    "BenchmarkDecisionAuditReport",
    "benchmark_decision_audit_report",
]
