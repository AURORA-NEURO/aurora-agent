"""Typed projections for the fixture-verified conformance and release-gate workflow."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


CONFORMANCE_MAX_ITEMS = 1_000


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("conformance response", value)
    if "suite" in raw and "release_decision" in raw:
        return raw
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        result = mcp.get("result")
        if isinstance(result, Mapping):
            structured = result.get("structuredContent")
            if isinstance(structured, Mapping) and "suite" in structured and "release_decision" in structured:
                return dict(structured)
            content = result.get("content")
            if isinstance(content, Sequence) and not isinstance(content, (str, bytes)):
                for block in content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"conformance response text is not JSON: {error}") from error
                    decoded_mapping = _route_mapping("decoded conformance response", decoded)
                    if "suite" in decoded_mapping and "release_decision" in decoded_mapping:
                        return decoded_mapping
    raise ArgumentError("response does not contain a conformance-run projection")


@dataclass(frozen=True)
class ConformanceRunArgs:
    """Bounded request for the shipped fixture-verified suite."""

    include_details: bool = False
    max_items: int = 100

    def __post_init__(self) -> None:
        if not isinstance(self.include_details, bool):
            raise ArgumentError("include_details must be a boolean")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= CONFORMANCE_MAX_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {CONFORMANCE_MAX_ITEMS}")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"include_details": self.include_details, "max_items": self.max_items}


@dataclass(frozen=True)
class ConformanceOutcomeReport:
    raw: dict[str, Any]
    outcome: str
    expectation: str | None
    detail: str | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceOutcomeReport":
        raw = _route_mapping("conformance case outcome", value)
        outcome = _route_text("conformance case outcome", raw.get("outcome"))
        if outcome not in {"passed", "failed", "unsupported", "errored"}:
            raise ArgumentError(f"unknown conformance case outcome: {outcome!r}")
        expectation = raw.get("expectation")
        detail = raw.get("detail")
        reason = raw.get("reason")
        if outcome in {"failed", "unsupported"}:
            expectation = _route_text("conformance outcome expectation", expectation)
        if outcome == "failed":
            detail = _route_text("conformance outcome detail", detail)
        if outcome in {"unsupported", "errored"}:
            reason = _route_text("conformance outcome reason", reason)
        return cls(raw, outcome, expectation, detail, reason)

    @property
    def passed(self) -> bool:
        return self.outcome == "passed"


@dataclass(frozen=True)
class ConformanceCaseReport:
    raw: dict[str, Any]
    case_id: str
    title: str
    layer: str
    requirement: str
    enforces: tuple[str, ...]
    invariant: str
    expectations: tuple[str, ...]
    outcome: ConformanceOutcomeReport

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceCaseReport":
        raw = _route_mapping("conformance case", value)
        layer = _route_text("conformance case layer", raw.get("layer"))
        if layer not in {"unit", "property", "golden", "conformance", "end_to_end"}:
            raise ArgumentError(f"unknown conformance case layer: {layer!r}")
        requirement = _route_text("conformance case requirement", raw.get("requirement"))
        if requirement not in {"must", "should"}:
            raise ArgumentError(f"unknown conformance requirement: {requirement!r}")
        return cls(
            raw=raw,
            case_id=_route_text("conformance case id", raw.get("case_id")),
            title=_route_text("conformance case title", raw.get("title")),
            layer=layer,
            requirement=requirement,
            enforces=_route_strings("conformance case enforces", raw.get("enforces", [])),
            invariant=_route_text("conformance case invariant", raw.get("invariant")),
            expectations=_route_strings("conformance case expectations", raw.get("expectations", [])),
            outcome=ConformanceOutcomeReport.from_wire(raw.get("outcome")),
        )


@dataclass(frozen=True)
class ConformancePyramidReport:
    raw: dict[str, Any]
    counts: dict[str, int]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformancePyramidReport":
        raw = _route_mapping("conformance pyramid", value)
        raw_counts = _route_mapping("conformance pyramid counts", raw.get("counts"))
        counts: dict[str, int] = {}
        for layer, count in raw_counts.items():
            counts[_route_text("conformance pyramid layer", layer)] = _route_count(
                f"conformance pyramid count for {layer}", count
            )
        return cls(raw=raw, counts=counts)

    @property
    def total(self) -> int:
        return sum(self.counts.values())


@dataclass(frozen=True)
class ConformanceSuiteReport:
    raw: dict[str, Any]
    id: str
    version: str
    digest: str
    fixture_manifest_id: str
    fixture_count: int
    synthetic_fixture_count: int
    case_count: int
    passed: int
    failed: int
    unsupported: int
    errored: int
    fixture_drift: tuple[dict[str, Any], ...]
    pyramid: ConformancePyramidReport
    fully_conformant: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceSuiteReport":
        raw = _route_mapping("conformance suite", value)
        counts = {name: _route_count(f"conformance suite {name}", raw.get(name)) for name in ("fixture_count", "synthetic_fixture_count", "case_count", "passed", "failed", "unsupported", "errored")}
        if counts["case_count"] != sum(counts[name] for name in ("passed", "failed", "unsupported", "errored")):
            raise ArgumentError("conformance suite case counts do not reconcile")
        drift_raw = raw.get("fixture_drift", [])
        if not isinstance(drift_raw, Sequence) or isinstance(drift_raw, (str, bytes)):
            raise ArgumentError("conformance fixture_drift must be an array")
        return cls(
            raw=raw,
            id=_route_text("conformance suite id", raw.get("id")),
            version=_route_text("conformance suite version", raw.get("version")),
            digest=_route_text("conformance suite digest", raw.get("digest")),
            fixture_manifest_id=_route_text("conformance fixture_manifest_id", raw.get("fixture_manifest_id")),
            fixture_count=counts["fixture_count"],
            synthetic_fixture_count=counts["synthetic_fixture_count"],
            case_count=counts["case_count"],
            passed=counts["passed"],
            failed=counts["failed"],
            unsupported=counts["unsupported"],
            errored=counts["errored"],
            fixture_drift=tuple(_route_mapping("conformance fixture drift", item) for item in drift_raw),
            pyramid=ConformancePyramidReport.from_wire(raw.get("pyramid")),
            fully_conformant=_bool("conformance suite fully_conformant", raw.get("fully_conformant")),
        )


@dataclass(frozen=True)
class ConformanceUnmetGateReport:
    raw: dict[str, Any]
    gate: str
    because: str
    evidence: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceUnmetGateReport":
        raw = _route_mapping("conformance unmet gate", value)
        return cls(
            raw=raw,
            gate=_route_text("conformance unmet gate name", raw.get("gate")),
            because=_route_text("conformance unmet gate reason", raw.get("because")),
            evidence=_route_strings("conformance unmet gate evidence", raw.get("evidence", [])),
        )


@dataclass(frozen=True)
class ConformanceReleaseDecisionReport:
    raw: dict[str, Any]
    decision: str
    suite_id: str
    suite_version: str
    suite_digest: str | None
    implementation: str | None
    gates: tuple[str, ...]
    met: tuple[str, ...]
    unmet: tuple[ConformanceUnmetGateReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceReleaseDecisionReport":
        raw = _route_mapping("conformance release_decision", value)
        decision = _route_text("conformance release decision", raw.get("decision"))
        if decision not in {"release", "blocked"}:
            raise ArgumentError(f"unknown conformance release decision: {decision!r}")
        suite_digest = raw.get("suite_digest")
        if suite_digest is not None:
            suite_digest = _route_text("conformance release suite_digest", suite_digest)
        implementation = raw.get("implementation")
        if implementation is not None:
            implementation = _route_text("conformance release implementation", implementation)
        gates = _route_strings("conformance release gates", raw.get("gates", []))
        met = _route_strings("conformance release met", raw.get("met", []))
        unmet_raw = raw.get("unmet", [])
        if not isinstance(unmet_raw, Sequence) or isinstance(unmet_raw, (str, bytes)):
            raise ArgumentError("conformance release unmet must be an array")
        unmet = tuple(ConformanceUnmetGateReport.from_wire(item) for item in unmet_raw)
        if decision == "release" and unmet:
            raise ArgumentError("release decision cannot contain unmet gates")
        if decision == "blocked" and not unmet:
            raise ArgumentError("blocked conformance decision must name unmet gates")
        return cls(raw, decision, _route_text("conformance decision suite_id", raw.get("suite_id")), _route_text("conformance decision suite_version", raw.get("suite_version")), suite_digest, implementation, gates, met, unmet)

    @property
    def release_ready(self) -> bool:
        return self.decision == "release"

    @property
    def blocking_gates(self) -> tuple[str, ...]:
        return tuple(gate.gate for gate in self.unmet)


@dataclass(frozen=True)
class ConformanceRunReport:
    raw: dict[str, Any]
    ok: bool
    suite: ConformanceSuiteReport
    release_decision: ConformanceReleaseDecisionReport
    summary: str
    results: tuple[ConformanceCaseReport, ...] | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ConformanceRunReport":
        raw = _payload(value)
        ok = _bool("conformance run ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("conformance run report is not successful")
        suite = ConformanceSuiteReport.from_wire(raw.get("suite"))
        decision = ConformanceReleaseDecisionReport.from_wire(raw.get("release_decision"))
        if decision.suite_id != suite.id or decision.suite_version != suite.version:
            raise ArgumentError("conformance release decision does not reconcile with its suite")
        if decision.suite_digest is not None and decision.suite_digest != suite.digest:
            raise ArgumentError("conformance release decision digest does not reconcile with its suite")
        raw_results = raw.get("results")
        results: tuple[ConformanceCaseReport, ...] | None
        if raw_results is None:
            results = None
        else:
            if not isinstance(raw_results, Sequence) or isinstance(raw_results, (str, bytes)):
                raise ArgumentError("conformance results must be an array or null")
            results = tuple(ConformanceCaseReport.from_wire(item) for item in raw_results)
            case_ids = [case.case_id for case in results]
            if len(case_ids) != len(set(case_ids)):
                raise ArgumentError("conformance result case ids must be unique")
        return cls(
            raw=raw,
            ok=ok,
            suite=suite,
            release_decision=decision,
            summary=_route_text("conformance summary", raw.get("summary")),
            results=results,
            guarantees=_route_strings("conformance guarantees", raw.get("guarantees", [])),
        )

    @property
    def details_included(self) -> bool:
        return self.results is not None

    @property
    def release_ready(self) -> bool:
        return self.release_decision.release_ready


def conformance_run_report(value: Mapping[str, Any]) -> ConformanceRunReport:
    """Parse direct MCP or HTTP conformance-run output."""

    return ConformanceRunReport.from_wire(value)


__all__ = [
    "CONFORMANCE_MAX_ITEMS",
    "ConformanceCaseReport",
    "ConformanceOutcomeReport",
    "ConformancePyramidReport",
    "ConformanceReleaseDecisionReport",
    "ConformanceRunArgs",
    "ConformanceRunReport",
    "ConformanceSuiteReport",
    "ConformanceUnmetGateReport",
    "conformance_run_report",
]
