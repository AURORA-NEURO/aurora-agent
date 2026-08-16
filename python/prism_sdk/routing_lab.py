"""Typed offline routing-lab requests and regret-report projections."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ROUTING_LAB_SCHEMA = "bioprism-mcp/routing-lab-run/0.1"
ROUTING_LAB_HOLDOUTS = frozenset({"task", "regime"})
ROUTING_LAB_VERDICTS = frozenset(
    {
        "router_loses_to_fixed_default",
        "no_achievable_gain",
        "router_matches_fixed_default",
        "router_beats_fixed_default",
    }
)
MAX_ROUTING_LAB_TASKS = 256
MAX_ROUTING_LAB_ROWS = 1_000
MAX_ROUTING_LAB_INPUT_BYTES = 20_000_000


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _probability(name: str, value: Any) -> float:
    result = _finite(name, value)
    if not 0.0 <= result <= 1.0:
        raise ArgumentError(f"{name} must lie in [0, 1]")
    return result


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("routing lab response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if candidate.get("ok") is True:
            return candidate.get("schema") == ROUTING_LAB_SCHEMA and isinstance(candidate.get("report"), Mapping)
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
                        raise ArgumentError(f"routing lab response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a routing lab projection")


@dataclass(frozen=True)
class RoutingLabRunArgs:
    tasks: tuple[Mapping[str, Any], ...]
    settings: Mapping[str, Any]
    include_rows: bool = False
    max_rows: int = 100

    def __post_init__(self) -> None:
        tasks = tuple(_route_mapping(f"routing lab tasks[{index}]", task) for index, task in enumerate(_array("routing lab tasks", self.tasks)))
        if not 1 <= len(tasks) <= MAX_ROUTING_LAB_TASKS:
            raise ArgumentError("routing lab tasks must contain between 1 and 256 objects")
        task_ids: list[str] = []
        for index, task in enumerate(tasks):
            task_id = _route_text(f"routing lab tasks[{index}].task_id", task.get("task_id"))
            if len(task_id.encode("utf-8")) > 512:
                raise ArgumentError("routing lab task_id must contain at most 512 bytes")
            _route_mapping(f"routing lab tasks[{index}].world", task.get("world"))
            _route_mapping(f"routing lab tasks[{index}].query", task.get("query"))
            task_ids.append(task_id)
        if len(task_ids) != len(set(task_ids)):
            raise ArgumentError("routing lab task_id values must be unique")
        settings = _route_mapping("routing lab settings", self.settings)
        if not isinstance(self.include_rows, bool):
            raise ArgumentError("routing lab include_rows must be a boolean")
        if not isinstance(self.max_rows, int) or isinstance(self.max_rows, bool) or not 1 <= self.max_rows <= MAX_ROUTING_LAB_ROWS:
            raise ArgumentError("routing lab max_rows must be between 1 and 1000")
        arguments = {"tasks": [dict(task) for task in tasks], "settings": settings, "include_rows": self.include_rows, "max_rows": self.max_rows}
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"routing lab arguments are not JSON serializable: {error}") from error
        if len(encoded) > MAX_ROUTING_LAB_INPUT_BYTES:
            raise ArgumentError("routing lab input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "tasks", tasks)
        object.__setattr__(self, "settings", settings)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RoutingLabRunArgs":
        raw = _route_mapping("routing lab arguments", value)
        return cls(tuple(_route_mapping(f"routing lab tasks[{index}]", item) for index, item in enumerate(_array("routing lab tasks", raw.get("tasks")))), _route_mapping("routing lab settings", raw.get("settings")), raw.get("include_rows", False), raw.get("max_rows", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {
            "tasks": [dict(task) for task in self.tasks],
            "settings": dict(self.settings),
            "include_rows": self.include_rows,
            "max_rows": self.max_rows,
        }


@dataclass(frozen=True)
class RoutingLabRunReport:
    raw: dict[str, Any]
    ok: bool
    schema: str | None
    tasks: int | None
    holdout: str | None
    holdout_label: str | None
    approved_architectures: tuple[str, ...]
    fixed_default: Mapping[str, Any] | None
    include_rows: bool
    report: Mapping[str, Any] | None
    verdict: str | None
    task_rows: tuple[Mapping[str, Any], ...]
    task_rows_omitted: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "RoutingLabRunReport":
        raw = _payload(value)
        if raw.get("ok") is False:
            if raw.get("fail_closed") is not True:
                raise ArgumentError("routing lab refusals must be fail-closed")
            return cls(raw, False, raw.get("schema"), None, None, None, (), None, False, None, None, (), 0, _route_strings("routing lab refusal guarantees", raw.get("guarantees", [])), tuple(), _route_text("routing lab refusal stage", raw.get("stage")), _route_text("routing lab refusal", raw.get("refusal")), True)
        if raw.get("ok") is not True or raw.get("schema") != ROUTING_LAB_SCHEMA:
            raise ArgumentError("routing lab projection has an invalid schema")
        tasks = _route_count("routing lab task count", raw.get("tasks"))
        if not 1 <= tasks <= MAX_ROUTING_LAB_TASKS:
            raise ArgumentError("routing lab task count must be between 1 and 256")
        holdout = _route_text("routing lab holdout", raw.get("holdout"))
        if holdout not in ROUTING_LAB_HOLDOUTS:
            raise ArgumentError("routing lab holdout is not recognized")
        holdout_label = _route_text("routing lab holdout label", raw.get("holdout_label"))
        approved = _route_strings("routing lab approved architectures", raw.get("approved_architectures", []))
        fixed_default = _route_mapping("routing lab fixed default", raw.get("fixed_default"))
        include_rows = raw.get("include_rows")
        if not isinstance(include_rows, bool):
            raise ArgumentError("routing lab include_rows must be a boolean")
        report = _route_mapping("routing lab report", raw.get("report"))
        verdict = _route_text("routing lab verdict", report.get("verdict"))
        if verdict not in ROUTING_LAB_VERDICTS:
            raise ArgumentError("routing lab verdict is not recognized")
        _probability("routing lab abstention rate", report.get("abstention_rate"))
        if report.get("oracle_agreement_rate") is not None:
            _probability("routing lab oracle agreement rate", report.get("oracle_agreement_rate"))
        outcome_counts = tuple(
            _route_count(f"routing lab report.{field}", report.get(field))
            for field in ("tasks_won", "tasks_lost", "tasks_tied")
        )
        if sum(outcome_counts) != tasks:
            raise ArgumentError("routing lab outcome counts do not reconcile with the task count")
        task_rows = tuple(_route_mapping("routing lab task row", item) for item in _array("routing lab task rows", report.get("task_rows", [])))
        task_rows_omitted = _route_count("routing lab task rows omitted", report.get("task_rows_omitted"))
        if len(task_rows) + task_rows_omitted != tasks:
            raise ArgumentError("routing lab task rows do not reconcile with the task count")
        _route_mapping("routing lab regret account", report.get("account"))
        _route_mapping("routing lab calibration", report.get("calibration"))
        _route_strings("routing lab caveats", report.get("caveats", []))
        return cls(raw, True, ROUTING_LAB_SCHEMA, tasks, holdout, holdout_label, approved, fixed_default, include_rows, report, verdict, task_rows, task_rows_omitted, _route_strings("routing lab guarantees", raw.get("guarantees", [])), _route_strings("routing lab limitations", raw.get("limitations", [])), None, None, False)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def negative_result(self) -> bool:
        return self.verdict in {"router_loses_to_fixed_default", "no_achievable_gain", "router_matches_fixed_default"}

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def routing_lab_run_report(value: Mapping[str, Any]) -> RoutingLabRunReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return RoutingLabRunReport.from_wire(value)


__all__ = [
    "ROUTING_LAB_SCHEMA",
    "ROUTING_LAB_HOLDOUTS",
    "ROUTING_LAB_VERDICTS",
    "MAX_ROUTING_LAB_TASKS",
    "MAX_ROUTING_LAB_ROWS",
    "MAX_ROUTING_LAB_INPUT_BYTES",
    "RoutingLabRunArgs",
    "RoutingLabRunReport",
    "routing_lab_run_report",
]
