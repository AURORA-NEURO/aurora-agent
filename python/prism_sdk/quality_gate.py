"""Typed data-quality gate requests and reports.

The quality gate is deliberately not a score.  A check can pass with an examined-value count,
fail with a concrete witness, or be not runnable because the run lacks a usable input.  These
models keep those cases and the composed three-way verdict visible across MCP and HTTP without
reimplementing the Rust quality semantics in Python.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


QUALITY_GATE_SCHEMA = "bioprism-mcp/quality-gate/0.1"
QUALITY_MAX_ROWS = 100_000
QUALITY_MAX_COLUMNS = 1_000
QUALITY_MAX_CHECKS = 1_000
QUALITY_VERDICTS = frozenset({"passed", "failed", "indeterminate"})
QUALITY_OUTCOMES = frozenset({"pass", "fail", "not_runnable"})
QUALITY_NOT_RUNNABLE_REASONS = frozenset(
    {"MissingColumn", "AllValuesNull", "NotComparable", "MissingReferenceSet"}
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _bounded_integer(name: str, value: Any, maximum: int) -> int:
    result = _integer(name, value)
    if result > maximum:
        raise ArgumentError(f"{name} must be at most {maximum}")
    return result


def _mapping(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _sequence(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _texts(name: str, value: Any) -> tuple[str, ...]:
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value)))


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _single_variant(name: str, value: Any, variants: frozenset[str]) -> tuple[str, dict[str, Any]]:
    raw = _mapping(name, value)
    present = [key for key in raw if key in variants]
    if len(present) != 1:
        raise ArgumentError(f"{name} must contain exactly one known variant")
    return present[0], _mapping(f"{name}.{present[0]}", raw[present[0]])


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract a quality report from direct MCP output or an HTTP REST envelope."""

    raw = _mapping("quality gate response", value)
    candidates: list[Mapping[str, Any]] = [raw]

    def add_container(container: Any) -> None:
        if not isinstance(container, Mapping):
            return
        candidates.append(container)
        nested = container.get("result")
        if isinstance(nested, Mapping):
            candidates.append(nested)
            structured = nested.get("structuredContent")
            if isinstance(structured, Mapping):
                candidates.append(structured)
            content = nested.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"quality gate response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == QUALITY_GATE_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain a quality gate report")


@dataclass(frozen=True)
class QualityGateRunArgs:
    """Bounded serialized Dataset, Gate, and optional foreign-key reference sets."""

    dataset: dict[str, Any]
    gate: dict[str, Any]
    references: dict[str, Any] | None = None

    def __init__(
        self,
        dataset: Mapping[str, Any],
        gate: Mapping[str, Any],
        references: Mapping[str, Any] | None = None,
    ) -> None:
        normalized_dataset = _mapping("quality dataset", dataset)
        normalized_gate = _mapping("quality gate", gate)
        columns = _mapping("quality dataset columns", normalized_dataset.get("columns", {}))
        checks = _mapping("quality gate checks", normalized_gate.get("checks", {}))
        rows = _bounded_integer("quality dataset rows", normalized_dataset.get("rows", 0), QUALITY_MAX_ROWS)
        if len(columns) > QUALITY_MAX_COLUMNS:
            raise ArgumentError(f"quality dataset may contain at most {QUALITY_MAX_COLUMNS} columns")
        if not checks:
            raise ArgumentError("quality gate must contain at least one named check")
        if len(checks) > QUALITY_MAX_CHECKS:
            raise ArgumentError(f"quality gate may contain at most {QUALITY_MAX_CHECKS} checks")
        for name, values in columns.items():
            if not isinstance(name, str) or not name.strip():
                raise ArgumentError("quality dataset column names must be non-empty strings")
            if not isinstance(values, Sequence) or isinstance(values, (str, bytes, bytearray)):
                raise ArgumentError(f"quality dataset column {name!r} must be an array")
            if len(values) != rows:
                raise ArgumentError(f"quality dataset column {name!r} does not match the declared row count")
        for name in checks:
            if not isinstance(name, str) or not name.strip():
                raise ArgumentError("quality gate check names must be non-empty strings")
        normalized_references = None if references is None else _mapping("quality reference sets", references)
        object.__setattr__(self, "dataset", normalized_dataset)
        object.__setattr__(self, "gate", normalized_gate)
        object.__setattr__(self, "references", normalized_references)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityGateRunArgs":
        raw = _mapping("quality gate arguments", value)
        return cls(raw.get("dataset"), raw.get("gate"), raw.get("references"))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"dataset": self.dataset, "gate": self.gate}
        if self.references is not None:
            result["references"] = self.references
        return result


@dataclass(frozen=True)
class QualityWitnessReport:
    """Concrete row/column evidence for a failed check."""

    raw: dict[str, Any]
    row: int
    column: str
    found: str
    expected: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityWitnessReport":
        raw = _mapping("quality witness", value)
        return cls(
            raw,
            _integer("quality witness row", raw.get("row")),
            _route_text("quality witness column", raw.get("column")),
            _route_text("quality witness found", raw.get("found")),
            _route_text("quality witness expected", raw.get("expected")),
        )


@dataclass(frozen=True)
class QualityNotRunnableReport:
    """Typed reason why a quality check could not be evaluated."""

    raw: dict[str, Any]
    kind: str
    column: str | None
    row: int | None
    found: str | None
    reference: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityNotRunnableReport":
        raw = _mapping("quality not-runnable reason", value)
        variant, payload = _single_variant("quality not-runnable reason", raw, QUALITY_NOT_RUNNABLE_REASONS)
        return cls(
            raw,
            variant,
            _optional_text("quality not-runnable column", payload.get("column")),
            None if payload.get("row") is None else _integer("quality not-runnable row", payload.get("row")),
            _optional_text("quality not-runnable found", payload.get("found")),
            _optional_text("quality not-runnable reference", payload.get("reference")),
        )


@dataclass(frozen=True)
class QualityOutcomeReport:
    """One named check's pass, fail witness, or not-runnable result."""

    raw: dict[str, Any]
    kind: str
    examined: int | None
    witness: QualityWitnessReport | None
    reason: QualityNotRunnableReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityOutcomeReport":
        raw = _mapping("quality check outcome", value)
        variant, payload = _single_variant("quality check outcome", raw, frozenset({"Pass", "Fail", "NotRunnable"}))
        if variant == "Pass":
            return cls(raw, "pass", _integer("quality examined count", payload.get("examined")), None, None)
        if variant == "Fail":
            return cls(raw, "fail", None, QualityWitnessReport.from_wire(payload.get("witness")), None)
        return cls(raw, "not_runnable", None, None, QualityNotRunnableReport.from_wire(payload.get("reason")))

    @property
    def passed(self) -> bool:
        return self.kind == "pass"

    @property
    def failed(self) -> bool:
        return self.kind == "fail"

    @property
    def not_runnable(self) -> bool:
        return self.kind == "not_runnable"


@dataclass(frozen=True)
class QualityVerdictReport:
    """Composed gate verdict, retaining failing and unrunnable names separately."""

    raw: dict[str, Any]
    kind: str
    checks: int | None
    failing: tuple[str, ...]
    not_runnable: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityVerdictReport":
        raw = _mapping("quality gate verdict", value)
        variant, payload = _single_variant("quality gate verdict", raw, frozenset({"Passed", "Failed", "Indeterminate"}))
        if variant == "Passed":
            return cls(raw, "passed", _integer("quality passed check count", payload.get("checks")), (), ())
        if variant == "Failed":
            return cls(raw, "failed", None, _route_strings("quality failing checks", payload.get("failing", [])), _route_strings("quality not-runnable checks", payload.get("not_runnable", [])))
        return cls(raw, "indeterminate", None, (), _route_strings("quality not-runnable checks", payload.get("not_runnable", [])))

    @property
    def passed(self) -> bool:
        return self.kind == "passed"


@dataclass(frozen=True)
class QualityGateExecutionReport:
    """The complete serialized Rust GateReport nested inside the MCP envelope."""

    raw: dict[str, Any]
    gate: str
    dataset: str
    rows: int
    outcomes: dict[str, QualityOutcomeReport]
    verdict: QualityVerdictReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityGateExecutionReport":
        raw = _mapping("quality gate execution report", value)
        outcome_values = _mapping("quality gate outcomes", raw.get("outcomes"))
        outcomes = {name: QualityOutcomeReport.from_wire(outcome) for name, outcome in outcome_values.items()}
        verdict = QualityVerdictReport.from_wire(raw.get("verdict"))
        if verdict.kind == "passed" and verdict.checks != len(outcomes):
            raise ArgumentError("quality passed check count does not reconcile with outcomes")
        actual_failing = tuple(name for name, outcome in outcomes.items() if outcome.failed)
        actual_blocked = tuple(name for name, outcome in outcomes.items() if outcome.not_runnable)
        if tuple(verdict.failing) != actual_failing:
            raise ArgumentError("quality failing check set does not reconcile with outcomes")
        if tuple(verdict.not_runnable) != actual_blocked:
            raise ArgumentError("quality not-runnable check set does not reconcile with outcomes")
        return cls(
            raw,
            _route_text("quality gate name", raw.get("gate")),
            _route_text("quality report dataset", raw.get("dataset")),
            _integer("quality report rows", raw.get("rows")),
            outcomes,
            verdict,
        )

    @property
    def passed_checks(self) -> tuple[str, ...]:
        return tuple(name for name, outcome in self.outcomes.items() if outcome.passed)

    @property
    def failed_checks(self) -> tuple[str, ...]:
        return tuple(name for name, outcome in self.outcomes.items() if outcome.failed)

    @property
    def not_runnable_checks(self) -> tuple[str, ...]:
        return tuple(name for name, outcome in self.outcomes.items() if outcome.not_runnable)


@dataclass(frozen=True)
class QualityGateRunReport:
    """Validated top-level quality gate result returned by MCP or REST."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    verdict: str
    passed: bool
    dataset: str
    rows: int
    check_count: int
    report: QualityGateExecutionReport
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "QualityGateRunReport":
        raw = _payload(value)
        ok = _bool("quality gate ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("quality gate structured reports must be successful; refusals remain transport errors")
        schema = _route_text("quality gate schema", raw.get("schema"))
        if schema != QUALITY_GATE_SCHEMA:
            raise ArgumentError(f"unsupported quality gate schema {schema!r}")
        verdict = _route_text("quality gate verdict", raw.get("verdict"))
        if verdict not in QUALITY_VERDICTS:
            raise ArgumentError(f"unknown quality gate verdict {verdict!r}")
        report = QualityGateExecutionReport.from_wire(raw.get("report"))
        passed = _bool("quality gate passed", raw.get("passed"))
        if passed != (verdict == "passed") or report.verdict.kind != verdict:
            raise ArgumentError("quality gate top-level verdict does not reconcile with report verdict")
        check_count = _integer("quality gate check count", raw.get("check_count"))
        if check_count != len(report.outcomes):
            raise ArgumentError("quality gate check count does not reconcile with outcomes")
        dataset = _route_text("quality gate dataset", raw.get("dataset"))
        rows = _integer("quality gate rows", raw.get("rows"))
        if dataset != report.dataset or rows != report.rows:
            raise ArgumentError("quality gate dataset projection does not reconcile with the report")
        return cls(raw, True, schema, verdict, passed, dataset, rows, check_count, report, _texts("quality gate guarantees", raw.get("guarantees", [])))

    @property
    def ready_for_release(self) -> bool:
        """True only when every named check ran and passed."""

        return self.passed

    @property
    def has_data_failures(self) -> bool:
        return bool(self.report.failed_checks)

    @property
    def has_run_obstructions(self) -> bool:
        return bool(self.report.not_runnable_checks)

    @property
    def failures_and_obstructions_are_separate(self) -> bool:
        return set(self.report.failed_checks).isdisjoint(self.report.not_runnable_checks)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def quality_gate_run(value: Mapping[str, Any]) -> QualityGateRunReport:
    """Parse a direct MCP result or HTTP envelope into a typed quality report."""

    return QualityGateRunReport.from_wire(value)


__all__ = [
    "QUALITY_GATE_SCHEMA",
    "QUALITY_MAX_ROWS",
    "QUALITY_MAX_COLUMNS",
    "QUALITY_MAX_CHECKS",
    "QualityGateRunArgs",
    "QualityWitnessReport",
    "QualityNotRunnableReport",
    "QualityOutcomeReport",
    "QualityVerdictReport",
    "QualityGateExecutionReport",
    "QualityGateRunReport",
    "quality_gate_run",
]
