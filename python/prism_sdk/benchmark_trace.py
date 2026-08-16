"""Typed benchmark-trace compiler projections.

The benchmark compiler separates observed textual divergence, causal ancestry, decision-boundary
ranking, episode segmentation, and repeated-action progress.  This SDK keeps those layers
separate as well.  A ranked candidate is not a proven cause, an environment divergence is not an
agent blame assignment, and a review proposal is not a replay or a benchmark cell.  The server's
structured fail-closed refusal is preserved so empty or non-decision-bearing traces cannot be
quietly converted into a fabricated diagnosis.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BENCHMARK_TRACE_MAX_EVENTS = 100_000
BENCHMARK_TRACE_MAX_ID_BYTES = 256
BENCHMARK_TRACE_MAX_INPUT_BYTES = 20_000_000
EVENT_KINDS = frozenset({"goal", "observation", "choice", "action", "result", "claim", "termination"})
DECISION_TYPES = frozenset(
    {
        "context_acquisition",
        "evidence_interpretation",
        "hypothesis_update",
        "plan_choice",
        "tool_selection",
        "tool_arguments",
        "memory_access",
        "delegation",
        "verification",
        "recovery",
        "termination",
        "external_side_effect",
        "answer_formulation",
        "unclassified",
    }
)
DIVERGENCE_KINDS = frozenset({"identical", "early_termination", "diverged"})
VERDICT_KINDS = frozenset(
    {"first_causal", "conjunction", "environment_divergence", "no_divergence", "unlocalizable"}
)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ArgumentError(f"{name} must be a finite number")
    parsed = float(value)
    if not math.isfinite(parsed):
        raise ArgumentError(f"{name} must be a finite number")
    return parsed


def _index(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _optional_index(name: str, value: Any) -> int | None:
    return None if value is None else _index(name, value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _text_array(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_array(name, value)))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, structured MCP content, or an HTTP REST tool envelope."""

    raw = _route_mapping("benchmark trace response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return isinstance(candidate.get("analysis"), Mapping) and isinstance(candidate.get("summary"), Mapping)
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
                        raise ArgumentError(f"benchmark trace response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a benchmark trace analysis projection")


@dataclass(frozen=True)
class BenchmarkTraceEventArgs:
    """One serializable trace event used by the benchmark compiler."""

    step: int
    kind: str
    payload: Mapping[str, Any]
    caused_by: int | None = None
    visible: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        step = _index("benchmark trace event step", self.step)
        kind = _route_text("benchmark trace event kind", self.kind)
        if kind not in EVENT_KINDS:
            raise ArgumentError(f"unknown benchmark trace event kind {kind!r}")
        payload = _route_mapping("benchmark trace event payload", self.payload)
        caused_by = _optional_index("benchmark trace event caused_by", self.caused_by)
        visible = _text_array("benchmark trace event visible", self.visible)
        object.__setattr__(self, "step", step)
        object.__setattr__(self, "kind", kind)
        object.__setattr__(self, "payload", payload)
        object.__setattr__(self, "caused_by", caused_by)
        object.__setattr__(self, "visible", visible)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkTraceEventArgs":
        raw = _route_mapping("benchmark trace event", value)
        return cls(
            _index("benchmark trace event step", raw.get("step")),
            _route_text("benchmark trace event kind", raw.get("kind")),
            _route_mapping("benchmark trace event payload", raw.get("payload")),
            _optional_index("benchmark trace event caused_by", raw.get("caused_by")),
            _text_array("benchmark trace event visible", raw.get("visible", [])),
        )

    def to_wire(self) -> dict[str, Any]:
        result: dict[str, Any] = {"step": self.step, "kind": self.kind, "payload": dict(self.payload)}
        if self.caused_by is not None:
            result["caused_by"] = self.caused_by
        if self.visible:
            result["visible"] = list(self.visible)
        return result


@dataclass(frozen=True)
class BenchmarkTraceArgs:
    """A bounded trace with no inferred events or timestamps."""

    trace_id: str
    events: tuple[BenchmarkTraceEventArgs, ...]
    succeeded: bool

    def __post_init__(self) -> None:
        trace_id = _route_text("benchmark trace trace_id", self.trace_id)
        if len(trace_id.encode("utf-8")) > BENCHMARK_TRACE_MAX_ID_BYTES:
            raise ArgumentError("benchmark trace trace_id exceeds 256 UTF-8 bytes")
        events = tuple(
            item if isinstance(item, BenchmarkTraceEventArgs) else BenchmarkTraceEventArgs.from_wire(item)
            for item in self.events
        )
        if len(events) > BENCHMARK_TRACE_MAX_EVENTS:
            raise ArgumentError("benchmark traces are bounded at 100000 events")
        steps = [event.step for event in events]
        if len(steps) != len(set(steps)):
            raise ArgumentError("benchmark trace event steps must be unique")
        succeeded = _bool("benchmark trace succeeded", self.succeeded)
        object.__setattr__(self, "trace_id", trace_id)
        object.__setattr__(self, "events", events)
        object.__setattr__(self, "succeeded", succeeded)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkTraceArgs":
        raw = _route_mapping("benchmark trace", value)
        return cls(
            _route_text("benchmark trace trace_id", raw.get("trace_id")),
            tuple(BenchmarkTraceEventArgs.from_wire(item) for item in _array("benchmark trace events", raw.get("events"))),
            _bool("benchmark trace succeeded", raw.get("succeeded")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {"trace_id": self.trace_id, "events": [event.to_wire() for event in self.events], "succeeded": self.succeeded}


@dataclass(frozen=True)
class BenchmarkTraceAnalyzeArgs:
    """Failing trajectory plus optional better/reference trajectory."""

    failing: BenchmarkTraceArgs
    reference: BenchmarkTraceArgs | None = None

    def __post_init__(self) -> None:
        failing = self.failing if isinstance(self.failing, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.failing)
        reference = None if self.reference is None else (self.reference if isinstance(self.reference, BenchmarkTraceArgs) else BenchmarkTraceArgs.from_wire(self.reference))
        arguments = {"failing": failing.to_wire(), "reference": None if reference is None else reference.to_wire()}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"benchmark trace arguments are not JSON serializable: {error}") from error
        if len(encoded) > BENCHMARK_TRACE_MAX_INPUT_BYTES:
            raise ArgumentError("benchmark trace input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "failing", failing)
        object.__setattr__(self, "reference", reference)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkTraceAnalyzeArgs":
        raw = _route_mapping("benchmark trace analysis arguments", value)
        reference = raw.get("reference")
        return cls(
            BenchmarkTraceArgs.from_wire(raw.get("failing")),
            None if reference is None else BenchmarkTraceArgs.from_wire(reference),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result = {"failing": self.failing.to_wire()}
        if self.reference is not None:
            result["reference"] = self.reference.to_wire()
        return result


@dataclass(frozen=True)
class BenchmarkCandidateScoreReport:
    raw: dict[str, Any]
    alternatives: int
    newly_visible: int
    downstream_steps: int
    is_divergence: bool
    total: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCandidateScoreReport":
        raw = _route_mapping("benchmark candidate score", value)
        total = _finite("benchmark candidate score total", raw.get("total"))
        if total < 0.0:
            raise ArgumentError("benchmark candidate score total cannot be negative")
        return cls(
            raw,
            _route_count("benchmark candidate alternatives", raw.get("alternatives")),
            _route_count("benchmark candidate newly_visible", raw.get("newly_visible")),
            _route_count("benchmark candidate downstream_steps", raw.get("downstream_steps")),
            _bool("benchmark candidate is_divergence", raw.get("is_divergence")),
            total,
        )


@dataclass(frozen=True)
class BenchmarkCausalScoreReport:
    raw: dict[str, Any]
    necessity: float
    counterfactual_effect: float
    irreversibility: float
    explanatory_simplicity: float
    total: float
    irreversibility_declared: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCausalScoreReport":
        raw = _route_mapping("benchmark causal score", value)
        values = {
            name: _finite(f"benchmark causal score {name}", raw.get(name))
            for name in ("necessity", "counterfactual_effect", "irreversibility", "explanatory_simplicity", "total")
        }
        if any(number < 0.0 for number in values.values()):
            raise ArgumentError("benchmark causal score components cannot be negative")
        return cls(raw, **values, irreversibility_declared=_bool("benchmark causal irreversibility_declared", raw.get("irreversibility_declared")))


@dataclass(frozen=True)
class BenchmarkCausalCandidateReport:
    raw: dict[str, Any]
    step: int
    kind: str
    summary: str
    score: BenchmarkCausalScoreReport
    upstream_unresolved: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCausalCandidateReport":
        raw = _route_mapping("benchmark causal candidate", value)
        return cls(
            raw,
            _index("benchmark causal candidate step", raw.get("step")),
            _route_text("benchmark causal candidate kind", raw.get("kind")),
            _route_text("benchmark causal candidate summary", raw.get("summary")),
            BenchmarkCausalScoreReport.from_wire(raw.get("score")),
            _optional_index("benchmark causal candidate upstream_unresolved", raw.get("upstream_unresolved")),
        )


@dataclass(frozen=True)
class BenchmarkDivergenceReport:
    raw: dict[str, Any]
    kind: str
    at_step: int | None
    shorter: str | None
    longer_continued_for: int | None
    failing_step: int | None
    passing_step: int | None
    common_prefix: int | None
    failing_did: str | None
    passing_did: str | None
    visibility_gap: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkDivergenceReport":
        raw = _route_mapping("benchmark textual divergence", value)
        kind = _route_text("benchmark textual divergence kind", raw.get("kind"))
        if kind not in DIVERGENCE_KINDS:
            raise ArgumentError(f"unknown benchmark divergence kind {kind!r}")
        if kind == "identical":
            return cls(raw, kind, None, None, None, None, None, None, None, None, ())
        if kind == "early_termination":
            return cls(
                raw,
                kind,
                _index("benchmark divergence at_step", raw.get("at_step")),
                _route_text("benchmark divergence shorter", raw.get("shorter")),
                _route_count("benchmark divergence longer_continued_for", raw.get("longer_continued_for")),
                None,
                None,
                None,
                None,
                None,
                (),
            )
        return cls(
            raw,
            kind,
            None,
            None,
            None,
            _index("benchmark divergence failing_step", raw.get("failing_step")),
            _index("benchmark divergence passing_step", raw.get("passing_step")),
            _route_count("benchmark divergence common_prefix", raw.get("common_prefix")),
            _route_text("benchmark divergence failing_did", raw.get("failing_did")),
            _route_text("benchmark divergence passing_did", raw.get("passing_did")),
            _route_strings("benchmark divergence visibility_gap", raw.get("visibility_gap", [])),
        )


@dataclass(frozen=True)
class BenchmarkCausalVerdictReport:
    raw: dict[str, Any]
    kind: str
    step: int | None
    score: float | None
    steps: tuple[int, ...]
    at_step: int | None
    event_kind: str | None
    nearest_controlled_ancestor: int | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCausalVerdictReport":
        raw = _route_mapping("benchmark causal verdict", value)
        kind = _route_text("benchmark causal verdict kind", raw.get("verdict"))
        if kind not in VERDICT_KINDS:
            raise ArgumentError(f"unknown benchmark causal verdict {kind!r}")
        if kind == "first_causal":
            return cls(raw, kind, _index("benchmark verdict step", raw.get("step")), _finite("benchmark verdict score", raw.get("score")), (), None, None, None, None)
        if kind == "conjunction":
            return cls(raw, kind, None, None, tuple(_index("benchmark verdict step", item) for item in _array("benchmark verdict steps", raw.get("steps"))), None, None, None, None)
        if kind == "environment_divergence":
            return cls(raw, kind, None, None, (), _index("benchmark environment divergence at_step", raw.get("at_step")), _route_text("benchmark environment divergence kind", raw.get("kind")), _optional_index("benchmark nearest controlled ancestor", raw.get("nearest_controlled_ancestor")), None)
        if kind == "unlocalizable":
            return cls(raw, kind, None, None, (), None, None, None, _route_text("benchmark unlocalizable reason", raw.get("reason")))
        return cls(raw, kind, None, None, (), None, None, None, None)

    @property
    def localized(self) -> bool:
        return self.kind in {"first_causal", "conjunction"}


@dataclass(frozen=True)
class BenchmarkCausalAnalysisReport:
    raw: dict[str, Any]
    trace_id: str
    textual: BenchmarkDivergenceReport
    textual_is_actionable: bool
    reference: str | None
    terminal_step: int
    ancestry: tuple[int, ...]
    candidates: tuple[BenchmarkCausalCandidateReport, ...]
    verdict: BenchmarkCausalVerdictReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkCausalAnalysisReport":
        raw = _route_mapping("benchmark causal analysis", value)
        ancestry = tuple(_index("benchmark causal ancestry step", item) for item in _array("benchmark causal ancestry", raw.get("ancestry")))
        candidates = tuple(BenchmarkCausalCandidateReport.from_wire(item) for item in _array("benchmark causal candidates", raw.get("candidates")))
        return cls(
            raw,
            _route_text("benchmark causal trace_id", raw.get("trace_id")),
            BenchmarkDivergenceReport.from_wire(raw.get("textual")),
            _bool("benchmark textual_is_actionable", raw.get("textual_is_actionable")),
            _optional_text("benchmark causal reference", raw.get("reference")),
            _index("benchmark terminal_step", raw.get("terminal_step")),
            ancestry,
            candidates,
            BenchmarkCausalVerdictReport.from_wire(raw.get("verdict")),
        )

    @property
    def refuses_to_localise(self) -> bool:
        return not self.verdict.localized

    @property
    def first_causal_step(self) -> int | None:
        if self.verdict.kind == "first_causal":
            return self.verdict.step
        if self.verdict.kind == "conjunction":
            return self.verdict.steps[0] if self.verdict.steps else None
        return None


@dataclass(frozen=True)
class BenchmarkReversibilityReport:
    raw: dict[str, Any]
    source: str
    irreversible: bool
    basis: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkReversibilityReport":
        raw = _route_mapping("benchmark reversibility", value)
        source = _route_text("benchmark reversibility source", raw.get("source"))
        if source not in {"declared", "assumed"}:
            raise ArgumentError(f"unknown benchmark reversibility source {source!r}")
        basis = _optional_text("benchmark reversibility basis", raw.get("basis"))
        if source == "assumed" and basis is None:
            raise ArgumentError("assumed benchmark reversibility must include its basis")
        return cls(raw, source, _bool("benchmark reversibility irreversible", raw.get("irreversible")), basis)


@dataclass(frozen=True)
class BenchmarkBoundaryReport:
    raw: dict[str, Any]
    step: int
    summary: str
    decision_type: str
    type_evidence: str
    reversibility: BenchmarkReversibilityReport
    rank: BenchmarkCandidateScoreReport
    no_op_reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkBoundaryReport":
        raw = _route_mapping("benchmark boundary", value)
        decision_type = _route_text("benchmark boundary decision_type", raw.get("decision_type"))
        if decision_type not in DECISION_TYPES:
            raise ArgumentError(f"unknown benchmark decision type {decision_type!r}")
        return cls(
            raw,
            _index("benchmark boundary step", raw.get("step")),
            _route_text("benchmark boundary summary", raw.get("summary")),
            decision_type,
            _route_text("benchmark boundary type_evidence", raw.get("type_evidence")),
            BenchmarkReversibilityReport.from_wire(raw.get("reversibility")),
            BenchmarkCandidateScoreReport.from_wire(raw.get("rank")),
            _optional_text("benchmark boundary no_op_reason", raw.get("no_op_reason")),
        )

    @property
    def extractable(self) -> bool:
        return self.no_op_reason is None


@dataclass(frozen=True)
class BenchmarkEpisodeReport:
    raw: dict[str, Any]
    index: int
    goal_step: int | None
    label: str
    steps: tuple[int, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkEpisodeReport":
        raw = _route_mapping("benchmark episode", value)
        return cls(
            raw,
            _index("benchmark episode index", raw.get("index")),
            _optional_index("benchmark episode goal_step", raw.get("goal_step")),
            _route_text("benchmark episode label", raw.get("label")),
            tuple(_index("benchmark episode step", item) for item in _array("benchmark episode steps", raw.get("steps"))),
        )


@dataclass(frozen=True)
class BenchmarkRepetitionReport:
    raw: dict[str, Any]
    summary: str
    steps: tuple[int, ...]
    classification: str
    evidence_gained: tuple[str, ...]
    repeats: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkRepetitionReport":
        raw = _route_mapping("benchmark repetition", value)
        classification = _route_mapping("benchmark repetition classification", raw.get("classification"))
        kind = _route_text("benchmark repetition classification kind", classification.get("kind"))
        if kind not in {"iterative_refinement", "stuck"}:
            raise ArgumentError(f"unknown benchmark repetition classification {kind!r}")
        evidence = _route_strings("benchmark repetition evidence_gained", classification.get("evidence_gained", []))
        repeats = None if kind == "iterative_refinement" else _route_count("benchmark repetition repeats", classification.get("repeats"))
        return cls(
            raw,
            _route_text("benchmark repetition summary", raw.get("summary")),
            tuple(_index("benchmark repetition step", item) for item in _array("benchmark repetition steps", raw.get("steps"))),
            kind,
            evidence,
            repeats,
        )


@dataclass(frozen=True)
class BenchmarkTraceSummaryReport:
    raw: dict[str, Any]
    episode_count: int
    boundary_count: int
    extractable_boundaries: int
    repetition_groups: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any], *, episodes: int, boundaries: int, repetitions: int) -> "BenchmarkTraceSummaryReport":
        raw = _route_mapping("benchmark trace summary", value)
        episode_count = _route_count("benchmark summary episode_count", raw.get("episode_count"))
        boundary_count = _route_count("benchmark summary boundary_count", raw.get("boundary_count"))
        extractable = _route_count("benchmark summary extractable_boundaries", raw.get("extractable_boundaries"))
        repetition_groups = _route_count("benchmark summary repetition_groups", raw.get("repetition_groups"))
        if (episode_count, boundary_count, repetition_groups) != (episodes, boundaries, repetitions):
            raise ArgumentError("benchmark trace summary counts do not reconcile with returned evidence")
        if extractable > boundary_count:
            raise ArgumentError("benchmark extractable boundary count exceeds boundary count")
        return cls(raw, episode_count, boundary_count, extractable, repetition_groups)


@dataclass(frozen=True)
class BenchmarkTraceAnalysisReport:
    """Complete causal, segmentation, boundary, and repetition projection."""

    raw: dict[str, Any]
    ok: bool
    trace_id: str | None
    succeeded: bool | None
    event_count: int | None
    reference_trace_id: str | None
    analysis: BenchmarkCausalAnalysisReport | None
    episodes: tuple[BenchmarkEpisodeReport, ...]
    boundaries: tuple[BenchmarkBoundaryReport, ...]
    repetitions: tuple[BenchmarkRepetitionReport, ...]
    summary: BenchmarkTraceSummaryReport | None
    guarantees: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BenchmarkTraceAnalysisReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("benchmark trace refusals must be fail-closed")
            return cls(
                raw,
                False,
                None,
                None,
                None,
                None,
                None,
                (),
                (),
                (),
                None,
                _route_strings("benchmark refusal guarantees", raw.get("guarantees", [])),
                _route_text("benchmark refusal stage", raw.get("stage")),
                _route_text("benchmark refusal", raw.get("refusal")),
                True,
            )
        if raw.get("ok") is not True:
            raise ArgumentError("benchmark trace projection must declare ok")
        episodes = tuple(BenchmarkEpisodeReport.from_wire(item) for item in _array("benchmark episodes", raw.get("episodes")))
        boundaries = tuple(BenchmarkBoundaryReport.from_wire(item) for item in _array("benchmark boundaries", raw.get("boundaries")))
        repetitions = tuple(BenchmarkRepetitionReport.from_wire(item) for item in _array("benchmark repetitions", raw.get("repetitions")))
        summary = BenchmarkTraceSummaryReport.from_wire(raw.get("summary"), episodes=len(episodes), boundaries=len(boundaries), repetitions=len(repetitions))
        return cls(
            raw,
            True,
            _route_text("benchmark trace_id", raw.get("trace_id")),
            _bool("benchmark succeeded", raw.get("succeeded")),
            _route_count("benchmark event_count", raw.get("event_count")),
            _optional_text("benchmark reference_trace_id", raw.get("reference_trace_id")),
            BenchmarkCausalAnalysisReport.from_wire(raw.get("analysis")),
            episodes,
            boundaries,
            repetitions,
            summary,
            _route_strings("benchmark guarantees", raw.get("guarantees", [])),
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
    def has_reference(self) -> bool:
        return self.reference_trace_id is not None

    @property
    def localized(self) -> bool | None:
        return None if self.analysis is None else self.analysis.verdict.localized

    @property
    def extractable_boundary_count(self) -> int:
        return sum(boundary.extractable for boundary in self.boundaries)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def benchmark_trace_analysis_report(value: Mapping[str, Any]) -> BenchmarkTraceAnalysisReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BenchmarkTraceAnalysisReport.from_wire(value)


__all__ = [
    "BENCHMARK_TRACE_MAX_EVENTS",
    "BENCHMARK_TRACE_MAX_ID_BYTES",
    "BENCHMARK_TRACE_MAX_INPUT_BYTES",
    "EVENT_KINDS",
    "DECISION_TYPES",
    "DIVERGENCE_KINDS",
    "VERDICT_KINDS",
    "BenchmarkTraceEventArgs",
    "BenchmarkTraceArgs",
    "BenchmarkTraceAnalyzeArgs",
    "BenchmarkCandidateScoreReport",
    "BenchmarkCausalScoreReport",
    "BenchmarkCausalCandidateReport",
    "BenchmarkDivergenceReport",
    "BenchmarkCausalVerdictReport",
    "BenchmarkCausalAnalysisReport",
    "BenchmarkReversibilityReport",
    "BenchmarkBoundaryReport",
    "BenchmarkEpisodeReport",
    "BenchmarkRepetitionReport",
    "BenchmarkTraceSummaryReport",
    "BenchmarkTraceAnalysisReport",
    "benchmark_trace_analysis_report",
]
