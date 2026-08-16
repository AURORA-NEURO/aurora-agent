"""Typed WeaveLang compilation and local replay projections.

The compiler returns two identities for a program: a whole-document digest and a semantic digest
that excludes provenance and assigned program identity.  Execution is a separate, explicit local
semantic phase.  Replay is the default, world-mutating transitions fail closed there, and the
gateway never invokes a network, model, or tool.  The SDK keeps compilation, execution status,
liveness, invariant violations, and optional IR/trace disclosure distinct.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


WEAVELANG_MAX_SOURCE_BYTES = 2_000_000
WEAVELANG_MAX_THREAD_ID_BYTES = 256
EXECUTION_MODES = frozenset({"replay", "live"})
EXECUTION_STATUSES = frozenset({"not_requested", "completed", "refused"})
INVARIANTS = frozenset(
    {
        "authority-safety",
        "delegation-attenuation",
        "budget-conservation",
        "commitment-accountability",
        "epistemic-integrity",
        "information-non-escalation",
        "causal-integrity",
        "replay-safety",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_bool(name: str, value: Any) -> bool | None:
    return None if value is None else _bool(name, value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a successful WeaveLang projection from MCP or REST transport envelopes."""

    raw = _route_mapping("WeaveLang response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return candidate.get("ok") is True and isinstance(candidate.get("program"), Mapping)

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
                        raise ArgumentError(f"WeaveLang response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a WeaveLang compilation projection")


@dataclass(frozen=True)
class WeaveLangCompileArgs:
    """Bounded compiler request with explicit execution and disclosure controls."""

    source: str
    execute: bool = False
    mode: str = "replay"
    thread_id: str = "mcp-weavelang"
    include_ir: bool = False
    include_trace: bool = False

    def __post_init__(self) -> None:
        if not isinstance(self.source, str) or not self.source:
            raise ArgumentError("WeaveLang source must be a non-empty string")
        if len(self.source.encode("utf-8")) > WEAVELANG_MAX_SOURCE_BYTES:
            raise ArgumentError(
                f"WeaveLang source must be at most {WEAVELANG_MAX_SOURCE_BYTES} bytes"
            )
        if not isinstance(self.execute, bool):
            raise ArgumentError("WeaveLang execute must be a boolean")
        if not isinstance(self.mode, str) or self.mode not in EXECUTION_MODES:
            raise ArgumentError("WeaveLang mode must be replay or live")
        if not isinstance(self.thread_id, str) or not 1 <= len(self.thread_id.encode("utf-8")) <= WEAVELANG_MAX_THREAD_ID_BYTES:
            raise ArgumentError(
                f"WeaveLang thread_id must contain between 1 and {WEAVELANG_MAX_THREAD_ID_BYTES} bytes"
            )
        if not isinstance(self.include_ir, bool) or not isinstance(self.include_trace, bool):
            raise ArgumentError("WeaveLang disclosure flags must be booleans")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangCompileArgs":
        raw = _route_mapping("WeaveLang arguments", value)
        return cls(
            _route_text("WeaveLang source", raw.get("source")),
            raw.get("execute", False),
            raw.get("mode", "replay"),
            raw.get("thread_id", "mcp-weavelang"),
            raw.get("include_ir", False),
            raw.get("include_trace", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "source": self.source,
            "execute": self.execute,
            "mode": self.mode,
            "thread_id": self.thread_id,
            "include_ir": self.include_ir,
            "include_trace": self.include_trace,
        }


@dataclass(frozen=True)
class WeaveLangInvariantViolationReport:
    raw: dict[str, Any]
    invariant: str
    detail: str
    at_event: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangInvariantViolationReport":
        raw = _route_mapping("WeaveLang invariant violation", value)
        invariant = _route_text("WeaveLang invariant", raw.get("invariant"))
        if invariant not in INVARIANTS:
            raise ArgumentError(f"unknown WeaveLang invariant {invariant!r}")
        return cls(
            raw,
            invariant,
            _route_text("WeaveLang invariant detail", raw.get("detail")),
            _route_count("WeaveLang invariant at_event", raw.get("at_event")),
        )


@dataclass(frozen=True)
class WeaveLangLivenessReport:
    raw: dict[str, Any]
    messages_left_unconsumed: int
    commitments_left_open: tuple[str, ...]
    states_without_exit: tuple[str, ...]
    unreachable_states: tuple[str, ...]
    deadlock_freedom_proven: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangLivenessReport":
        raw = _route_mapping("WeaveLang liveness", value)
        return cls(
            raw,
            _route_count("WeaveLang messages_left_unconsumed", raw.get("messages_left_unconsumed")),
            _route_strings("WeaveLang commitments_left_open", raw.get("commitments_left_open", [])),
            _route_strings("WeaveLang states_without_exit", raw.get("states_without_exit", [])),
            _route_strings("WeaveLang unreachable_states", raw.get("unreachable_states", [])),
            _bool("WeaveLang deadlock_freedom_proven", raw.get("deadlock_freedom_proven")),
        )

    @property
    def has_open_obligations(self) -> bool:
        return bool(self.commitments_left_open)

    @property
    def has_structural_holes(self) -> bool:
        return bool(self.states_without_exit or self.unreachable_states)


@dataclass(frozen=True)
class WeaveLangProgramReport:
    raw: dict[str, Any]
    program_id: str
    digest: str
    semantic_digest: str
    weave_ir_version: str
    roles: int
    participants: int
    interfaces: int
    policies: int
    state_nodes: int
    transitions: int
    monitors: int
    initial_state: str
    terminal_states: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangProgramReport":
        raw = _route_mapping("WeaveLang program", value)
        return cls(
            raw,
            _route_text("WeaveLang program_id", raw.get("program_id")),
            _route_text("WeaveLang digest", raw.get("digest")),
            _route_text("WeaveLang semantic_digest", raw.get("semantic_digest")),
            _route_text("WeaveLang weave_ir_version", raw.get("weave_ir_version")),
            _route_count("WeaveLang roles", raw.get("roles")),
            _route_count("WeaveLang participants", raw.get("participants")),
            _route_count("WeaveLang interfaces", raw.get("interfaces")),
            _route_count("WeaveLang policies", raw.get("policies")),
            _route_count("WeaveLang state_nodes", raw.get("state_nodes")),
            _route_count("WeaveLang transitions", raw.get("transitions")),
            _route_count("WeaveLang monitors", raw.get("monitors")),
            _route_text("WeaveLang initial_state", raw.get("initial_state")),
            _route_strings("WeaveLang terminal_states", raw.get("terminal_states", [])),
        )


@dataclass(frozen=True)
class WeaveLangExecutionReport:
    raw: dict[str, Any]
    status: str
    mode: str
    state: str
    liveness: WeaveLangLivenessReport
    invariant_violations: tuple[WeaveLangInvariantViolationReport, ...]
    event_count: int | None
    trace_digest: str | None
    trace: dict[str, Any] | None
    error: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangExecutionReport":
        raw = _route_mapping("WeaveLang execution", value)
        status = _route_text("WeaveLang execution status", raw.get("status"))
        if status not in EXECUTION_STATUSES:
            raise ArgumentError(f"unknown WeaveLang execution status {status!r}")
        mode = _route_text("WeaveLang execution mode", raw.get("mode"))
        if mode not in EXECUTION_MODES:
            raise ArgumentError(f"unknown WeaveLang execution mode {mode!r}")
        violations = tuple(
            WeaveLangInvariantViolationReport.from_wire(row)
            for row in _array("WeaveLang invariant_violations", raw.get("invariant_violations", []))
        )
        event_count = None if raw.get("event_count") is None else _route_count("WeaveLang event_count", raw.get("event_count"))
        trace_digest = _optional_text("WeaveLang trace_digest", raw.get("trace_digest"))
        trace_raw = raw.get("trace")
        trace = None if trace_raw is None else _route_mapping("WeaveLang trace", trace_raw)
        error = _optional_text("WeaveLang execution error", raw.get("error"))
        fail_closed = _optional_bool("WeaveLang execution fail_closed", raw.get("fail_closed"))
        if status == "completed" and (event_count is None or trace_digest is None):
            raise ArgumentError("completed WeaveLang executions must include event_count and trace_digest")
        if status == "refused" and (error is None or fail_closed is not True):
            raise ArgumentError("refused WeaveLang executions must include an error and fail_closed=true")
        return cls(
            raw,
            status,
            mode,
            _route_text("WeaveLang execution state", raw.get("state")),
            WeaveLangLivenessReport.from_wire(raw.get("liveness", {})),
            violations,
            event_count,
            trace_digest,
            trace,
            error,
            fail_closed,
        )

    @property
    def requested(self) -> bool:
        return self.status != "not_requested"

    @property
    def completed(self) -> bool:
        return self.status == "completed"

    @property
    def refused(self) -> bool:
        return self.status == "refused"

    @property
    def invariant_clean(self) -> bool:
        return not self.invariant_violations

    @property
    def replay_safe(self) -> bool:
        return self.mode != "replay" or not any(
            violation.invariant == "replay-safety" for violation in self.invariant_violations
        )


@dataclass(frozen=True)
class WeaveLangCompileReport:
    """Validated compilation plus explicit local execution outcome."""

    raw: dict[str, Any]
    ok: bool
    program: WeaveLangProgramReport
    execution: WeaveLangExecutionReport
    ir: dict[str, Any] | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WeaveLangCompileReport":
        raw = _payload(value)
        ok = _bool("WeaveLang ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("WeaveLang compilation projection must be successful")
        ir_raw = raw.get("ir")
        ir = None if ir_raw is None else _route_mapping("WeaveLang IR", ir_raw)
        return cls(
            raw,
            ok,
            WeaveLangProgramReport.from_wire(raw.get("program", {})),
            WeaveLangExecutionReport.from_wire(raw.get("execution", {})),
            ir,
            _route_strings("WeaveLang guarantees", raw.get("guarantees", [])),
        )

    @property
    def compiled(self) -> bool:
        return self.ok

    @property
    def execution_requested(self) -> bool:
        return self.execution.requested

    @property
    def execution_local_only(self) -> bool:
        return "execution is a local semantic trace; it performs no network, model, or tool call" in self.guarantees

    @property
    def replay_defaulted(self) -> bool:
        return self.execution.mode == "replay"

    @property
    def disclosure_includes_ir(self) -> bool:
        return self.ir is not None

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def weavelang_compile_report(value: Mapping[str, Any]) -> WeaveLangCompileReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return WeaveLangCompileReport.from_wire(value)


__all__ = [
    "WEAVELANG_MAX_SOURCE_BYTES",
    "WEAVELANG_MAX_THREAD_ID_BYTES",
    "EXECUTION_MODES",
    "EXECUTION_STATUSES",
    "INVARIANTS",
    "WeaveLangCompileArgs",
    "WeaveLangInvariantViolationReport",
    "WeaveLangLivenessReport",
    "WeaveLangProgramReport",
    "WeaveLangExecutionReport",
    "WeaveLangCompileReport",
    "weavelang_compile_report",
]
