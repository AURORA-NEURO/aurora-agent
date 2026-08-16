"""Typed evaluator-health audits for the bioevaluation kernel.

This module deliberately keeps three concepts separate:

* ``health`` answers whether the evaluator harness was able to speak;
* ``reached`` is the task predicate reached by a healthy evaluator; and
* ``diagnostic`` explains a negative result and records any hidden-data access.

The distinction is useful in every domain that has a benchmark, validator, grader,
or policy checker. A broken harness is not a failing system, and a passing result
that read hidden fixtures is not an uncomplicated pass.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


BIOEVAL_EVALUATOR_SCHEMA = "bioprism-mcp/bioeval-evaluator-audit/0.1"
BIOEVAL_EVALUATOR_HEALTH_STATES = frozenset({"healthy", "timed_out", "errored", "fixture_broken"})
BIOEVAL_EVALUATOR_TASK_OUTCOMES = frozenset({"met", "not_met", "inapplicable"})
MAX_BIOEVAL_EVALUATOR_RUNS = 1_024
MAX_BIOEVAL_EVALUATOR_OUTPUT_ITEMS = 1_000
MAX_BIOEVAL_EVALUATOR_TEXT_BYTES = 4_096
MAX_BIOEVAL_EVALUATOR_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _text(name: str, value: Any, *, allow_empty: bool = False) -> str:
    if not isinstance(value, str):
        raise ArgumentError(f"{name} must be a string")
    text = value
    if not allow_empty and not text.strip():
        raise ArgumentError(f"{name} must be non-empty text")
    if len(text.encode("utf-8")) > MAX_BIOEVAL_EVALUATOR_TEXT_BYTES:
        raise ArgumentError(f"{name} exceeds {MAX_BIOEVAL_EVALUATOR_TEXT_BYTES} UTF-8 bytes")
    return text


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("bioeval evaluator response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == BIOEVAL_EVALUATOR_SCHEMA and isinstance(candidate.get("panel"), Mapping)
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
                        raise ArgumentError(f"bioeval evaluator response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a bioeval evaluator projection")


@dataclass(frozen=True)
class BioevalEvaluatorHealthArgs:
    """The evaluator's own health, represented as the Rust internal-tag shape."""

    health: str
    after: str | None = None
    detail: str | None = None

    def __post_init__(self) -> None:
        state = _text("bioeval evaluator health", self.health)
        if state not in BIOEVAL_EVALUATOR_HEALTH_STATES:
            raise ArgumentError("bioeval evaluator health must be healthy, timed_out, errored, or fixture_broken")
        after = None if self.after is None else _text("bioeval evaluator timeout", self.after)
        detail = None if self.detail is None else _text("bioeval evaluator health detail", self.detail)
        if state == "timed_out" and after is None:
            raise ArgumentError("timed_out evaluator health requires after")
        if state in {"errored", "fixture_broken"} and detail is None:
            raise ArgumentError(f"{state} evaluator health requires detail")
        if state == "healthy" and (after is not None or detail is not None):
            raise ArgumentError("healthy evaluator health cannot carry timeout or failure detail")
        object.__setattr__(self, "health", state)
        object.__setattr__(self, "after", after)
        object.__setattr__(self, "detail", detail)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEvaluatorHealthArgs":
        raw = _route_mapping("bioeval evaluator health", value)
        return cls(
            _text("bioeval evaluator health", raw.get("health")),
            None if raw.get("after") is None else _text("bioeval evaluator timeout", raw.get("after")),
            None if raw.get("detail") is None else _text("bioeval evaluator health detail", raw.get("detail")),
        )

    def to_wire(self) -> dict[str, str]:
        result = {"health": self.health}
        if self.after is not None:
            result["after"] = self.after
        if self.detail is not None:
            result["detail"] = self.detail
        return result


@dataclass(frozen=True)
class BioevalEvaluatorDiagnosticArgs:
    """Evidence accompanying a task outcome or an evaluator review."""

    command: str = ""
    exit_state: str = ""
    diff: str = ""
    logs: tuple[str, ...] = ()
    hidden_data_access: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        command = _text("bioeval evaluator diagnostic command", self.command, allow_empty=True)
        exit_state = _text("bioeval evaluator diagnostic exit_state", self.exit_state, allow_empty=True)
        diff = _text("bioeval evaluator diagnostic diff", self.diff, allow_empty=True)
        logs = tuple(_text(f"bioeval evaluator diagnostic log[{index}]", item, allow_empty=True) for index, item in enumerate(self.logs))
        hidden = tuple(_text(f"bioeval evaluator hidden-data evidence[{index}]", item) for index, item in enumerate(self.hidden_data_access))
        object.__setattr__(self, "command", command)
        object.__setattr__(self, "exit_state", exit_state)
        object.__setattr__(self, "diff", diff)
        object.__setattr__(self, "logs", logs)
        object.__setattr__(self, "hidden_data_access", hidden)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any] | None) -> "BioevalEvaluatorDiagnosticArgs":
        raw = {} if value is None else _route_mapping("bioeval evaluator diagnostic", value)
        return cls(
            "" if raw.get("command") is None else _text("bioeval evaluator diagnostic command", raw.get("command"), allow_empty=True),
            "" if raw.get("exit_state") is None else _text("bioeval evaluator diagnostic exit_state", raw.get("exit_state"), allow_empty=True),
            "" if raw.get("diff") is None else _text("bioeval evaluator diagnostic diff", raw.get("diff"), allow_empty=True),
            tuple(_text(f"bioeval evaluator diagnostic log[{index}]", item, allow_empty=True) for index, item in enumerate(_array("bioeval evaluator diagnostic logs", raw.get("logs", [])))),
            tuple(_text(f"bioeval evaluator hidden-data evidence[{index}]", item) for index, item in enumerate(_array("bioeval evaluator hidden-data access", raw.get("hidden_data_access", [])))),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "command": self.command,
            "exit_state": self.exit_state,
            "diff": self.diff,
            "logs": list(self.logs),
            "hidden_data_access": list(self.hidden_data_access),
        }


@dataclass(frozen=True)
class BioevalEvaluatorRunArgs:
    evaluator: str
    health: BioevalEvaluatorHealthArgs | Mapping[str, Any]
    reached: str | None = None
    diagnostic: BioevalEvaluatorDiagnosticArgs | Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        evaluator = _text("bioeval evaluator id", self.evaluator)
        health = self.health if isinstance(self.health, BioevalEvaluatorHealthArgs) else BioevalEvaluatorHealthArgs.from_wire(self.health)
        reached = None if self.reached is None else _text("bioeval evaluator reached", self.reached)
        if reached is not None and reached not in BIOEVAL_EVALUATOR_TASK_OUTCOMES:
            raise ArgumentError("bioeval evaluator reached must be met, not_met, or inapplicable")
        diagnostic = self.diagnostic if isinstance(self.diagnostic, BioevalEvaluatorDiagnosticArgs) else BioevalEvaluatorDiagnosticArgs.from_wire(self.diagnostic)
        object.__setattr__(self, "evaluator", evaluator)
        object.__setattr__(self, "health", health)
        object.__setattr__(self, "reached", reached)
        object.__setattr__(self, "diagnostic", diagnostic)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEvaluatorRunArgs":
        raw = _route_mapping("bioeval evaluator run", value)
        return cls(
            _text("bioeval evaluator id", raw.get("evaluator")),
            BioevalEvaluatorHealthArgs.from_wire(raw.get("health")),
            None if raw.get("reached") is None else _text("bioeval evaluator reached", raw.get("reached")),
            BioevalEvaluatorDiagnosticArgs.from_wire(raw.get("diagnostic")),
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "evaluator": self.evaluator,
            "health": self.health.to_wire(),  # type: ignore[union-attr]
            "reached": self.reached,
            "diagnostic": self.diagnostic.to_wire(),  # type: ignore[union-attr]
        }


@dataclass(frozen=True)
class BioevalEvaluatorAuditArgs:
    runs: tuple[BioevalEvaluatorRunArgs, ...]
    require_task_evidence: bool = False
    fail_on_hidden_data: bool = False
    max_items: int = 100

    def __post_init__(self) -> None:
        runs = tuple(item if isinstance(item, BioevalEvaluatorRunArgs) else BioevalEvaluatorRunArgs.from_wire(item) for item in self.runs)
        if len(runs) > MAX_BIOEVAL_EVALUATOR_RUNS:
            raise ArgumentError("bioeval evaluator runs are bounded at 1024 rows")
        require_task_evidence = _bool("bioeval require_task_evidence", self.require_task_evidence)
        fail_on_hidden_data = _bool("bioeval fail_on_hidden_data", self.fail_on_hidden_data)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= MAX_BIOEVAL_EVALUATOR_OUTPUT_ITEMS:
            raise ArgumentError("bioeval evaluator max_items must be between 1 and 1000")
        object.__setattr__(self, "runs", runs)
        object.__setattr__(self, "require_task_evidence", require_task_evidence)
        object.__setattr__(self, "fail_on_hidden_data", fail_on_hidden_data)
        encoded = json.dumps(self.to_mcp_arguments(), ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if len(encoded) > MAX_BIOEVAL_EVALUATOR_INPUT_BYTES:
            raise ArgumentError("bioeval evaluator input exceeds the 20000000-byte safety bound")

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEvaluatorAuditArgs":
        raw = _route_mapping("bioeval evaluator arguments", value)
        return cls(
            tuple(BioevalEvaluatorRunArgs.from_wire(item) for item in _array("bioeval evaluator runs", raw.get("runs"))),
            raw.get("require_task_evidence", False),
            raw.get("fail_on_hidden_data", False),
            raw.get("max_items", 100),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "runs": [item.to_wire() for item in self.runs],
            "require_task_evidence": self.require_task_evidence,
            "fail_on_hidden_data": self.fail_on_hidden_data,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class BioevalEvaluatorAuditReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    workflow: str | None
    runs: Mapping[str, Any] | None
    panel: Mapping[str, Any] | None
    findings: Mapping[str, Any] | None
    stage: str | None
    refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalEvaluatorAuditReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("bioeval evaluator refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), raw.get("workflow"), None, None, None, _route_text("bioeval evaluator refusal stage", raw.get("stage")), _route_text("bioeval evaluator refusal", raw.get("refusal")), _route_strings("bioeval evaluator refusal guarantees", raw.get("guarantees", [])), _route_strings("bioeval evaluator refusal limitations", raw.get("limitations", [])), True)
        if raw.get("ok") is not True or raw.get("schema") != BIOEVAL_EVALUATOR_SCHEMA:
            raise ArgumentError("bioeval evaluator projection has an invalid schema")
        return cls(
            raw,
            True,
            BIOEVAL_EVALUATOR_SCHEMA,
            _route_text("bioeval evaluator workflow", raw.get("workflow")),
            _route_mapping("bioeval evaluator runs projection", raw.get("runs")),
            _route_mapping("bioeval evaluator panel", raw.get("panel")),
            _route_mapping("bioeval evaluator findings", raw.get("findings")),
            None,
            None,
            _route_strings("bioeval evaluator guarantees", raw.get("guarantees", [])),
            _route_strings("bioeval evaluator limitations", raw.get("limitations", [])),
            False,
        )

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def posture(self) -> str | None:
        if self.panel is None:
            return None
        value = self.panel.get("posture")
        return value if isinstance(value, str) else None

    @property
    def task_evidence_count(self) -> int | None:
        if self.panel is None:
            return None
        value = self.panel.get("task_evidence_count")
        return value if isinstance(value, int) and not isinstance(value, bool) else None

    @property
    def hidden_data_evaluators(self) -> tuple[str, ...]:
        if self.findings is None or not isinstance(self.findings.get("hidden_data_evaluators"), Mapping):
            return ()
        values = self.findings["hidden_data_evaluators"].get("ids", [])
        return tuple(value for value in values if isinstance(value, str)) if isinstance(values, Sequence) and not isinstance(values, (str, bytes)) else ()

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def bioeval_evaluator_audit_report(value: Mapping[str, Any]) -> BioevalEvaluatorAuditReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return BioevalEvaluatorAuditReport.from_wire(value)


__all__ = [
    "BIOEVAL_EVALUATOR_SCHEMA",
    "BIOEVAL_EVALUATOR_HEALTH_STATES",
    "BIOEVAL_EVALUATOR_TASK_OUTCOMES",
    "MAX_BIOEVAL_EVALUATOR_RUNS",
    "MAX_BIOEVAL_EVALUATOR_OUTPUT_ITEMS",
    "MAX_BIOEVAL_EVALUATOR_TEXT_BYTES",
    "MAX_BIOEVAL_EVALUATOR_INPUT_BYTES",
    "BioevalEvaluatorHealthArgs",
    "BioevalEvaluatorDiagnosticArgs",
    "BioevalEvaluatorRunArgs",
    "BioevalEvaluatorAuditArgs",
    "BioevalEvaluatorAuditReport",
    "bioeval_evaluator_audit_report",
]
