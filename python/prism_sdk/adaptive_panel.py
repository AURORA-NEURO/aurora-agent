"""Typed adaptive evaluation panel requests and evidence projections.

An adaptive panel is more than a scalar estimate.  It reports what was run, what counted after
parent clustering, which coverage floors remain open, why stopping did or did not occur, how
candidate selection was constrained, and which estimates were withheld.  These models validate
that evidence-bearing structure at the SDK boundary while leaving all statistical calculations in
the Rust authority.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import math
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ADAPTIVE_PANEL_SCHEMA = "bioprism-mcp/adaptive-panel/0.1"
ADAPTIVE_MAX_CANDIDATES = 10_000
ADAPTIVE_MAX_ITEMS = 1_000


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value


def _number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


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


def _optional_bool(name: str, value: Any) -> bool | None:
    return None if value is None else _bool(name, value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract an adaptive panel report from direct MCP output or an HTTP REST envelope."""

    raw = _mapping("adaptive panel response", value)
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
                        raise ArgumentError(f"adaptive panel response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)

    add_container(raw.get("mcp"))
    add_container(raw.get("result"))
    add_container(raw.get("structuredContent"))
    for candidate in candidates:
        if candidate.get("schema") == ADAPTIVE_PANEL_SCHEMA and "ok" in candidate:
            return dict(candidate)
    raise ArgumentError("response does not contain an adaptive panel report")


@dataclass(frozen=True)
class AdaptivePanelRunArgs:
    """Serialized panel and bounded optional selection/query controls."""

    panel: dict[str, Any]
    candidates: tuple[dict[str, Any], ...] | None = None
    batch_size: int | None = None
    capability: str | None = None
    left: str | None = None
    right: str | None = None
    max_items: int = 100

    def __init__(
        self,
        panel: Mapping[str, Any],
        candidates: Sequence[Mapping[str, Any]] | None = None,
        batch_size: int | None = None,
        capability: str | None = None,
        left: str | None = None,
        right: str | None = None,
        max_items: int = 100,
    ) -> None:
        normalized_panel = _mapping("adaptive panel", panel)
        normalized_candidates = None if candidates is None else tuple(_mapping(f"adaptive candidate[{index}]", item) for index, item in enumerate(_sequence("adaptive candidates", candidates)))
        if normalized_candidates is not None and len(normalized_candidates) > ADAPTIVE_MAX_CANDIDATES:
            raise ArgumentError(f"adaptive candidates exceeds the {ADAPTIVE_MAX_CANDIDATES}-candidate safety bound")
        if batch_size is not None and (isinstance(batch_size, bool) or not isinstance(batch_size, int) or not 1 <= batch_size <= ADAPTIVE_MAX_ITEMS):
            raise ArgumentError(f"batch_size must be between 1 and {ADAPTIVE_MAX_ITEMS}")
        if batch_size is not None and normalized_candidates is None:
            raise ArgumentError("batch_size requires candidates")
        if (left is None) != (right is None):
            raise ArgumentError("left and right must be supplied together")
        for name, value in (("capability", capability), ("left", left), ("right", right)):
            if value is not None:
                _route_text(f"adaptive {name}", value)
        if isinstance(max_items, bool) or not isinstance(max_items, int) or not 1 <= max_items <= ADAPTIVE_MAX_ITEMS:
            raise ArgumentError(f"max_items must be between 1 and {ADAPTIVE_MAX_ITEMS}")
        object.__setattr__(self, "panel", normalized_panel)
        object.__setattr__(self, "candidates", normalized_candidates)
        object.__setattr__(self, "batch_size", batch_size)
        object.__setattr__(self, "capability", capability)
        object.__setattr__(self, "left", left)
        object.__setattr__(self, "right", right)
        object.__setattr__(self, "max_items", max_items)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptivePanelRunArgs":
        raw = _mapping("adaptive panel arguments", value)
        return cls(raw.get("panel"), raw.get("candidates"), raw.get("batch_size"), raw.get("capability"), raw.get("left"), raw.get("right"), raw.get("max_items", 100))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"panel": self.panel, "max_items": self.max_items}
        if self.candidates is not None:
            result["candidates"] = list(self.candidates)
        if self.batch_size is not None:
            result["batch_size"] = self.batch_size
        for name, value in (("capability", self.capability), ("left", self.left), ("right", self.right)):
            if value is not None:
                result[name] = value
        return result


@dataclass(frozen=True)
class AdaptiveIntervalReport:
    raw: dict[str, Any]
    lo: float
    hi: float
    credibility: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveIntervalReport":
        raw = _mapping("adaptive interval", value)
        lo = _number("adaptive interval lo", raw.get("lo"))
        hi = _number("adaptive interval hi", raw.get("hi"))
        credibility = _number("adaptive interval credibility", raw.get("credibility"))
        if lo > hi or not 0.0 < credibility < 1.0:
            raise ArgumentError("adaptive interval bounds or credibility are invalid")
        return cls(raw, lo, hi, credibility)

    @property
    def width(self) -> float:
        return self.hi - self.lo


@dataclass(frozen=True)
class AdaptiveShortfallReport:
    raw: dict[str, Any]
    kind: str
    have: int | None
    need: int | None
    parent: str | None
    share: float | None
    cap: float | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveShortfallReport":
        raw = _mapping("adaptive coverage shortfall", value)
        kind = _route_text("adaptive shortfall kind", raw.get("kind"))
        return cls(
            raw,
            kind,
            None if raw.get("have") is None else _integer("adaptive shortfall have", raw.get("have")),
            None if raw.get("need") is None else _integer("adaptive shortfall need", raw.get("need")),
            _optional_text("adaptive shortfall parent", raw.get("parent")),
            None if raw.get("share") is None else _number("adaptive shortfall share", raw.get("share")),
            None if raw.get("cap") is None else _number("adaptive shortfall cap", raw.get("cap")),
        )


@dataclass(frozen=True)
class AdaptiveCoverageReport:
    raw: dict[str, Any]
    capability: str
    trials: int
    parents: int
    qualifying_parents: int
    abstentions: int
    shortfalls: tuple[AdaptiveShortfallReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveCoverageReport":
        raw = _mapping("adaptive coverage status", value)
        return cls(raw, _route_text("adaptive coverage capability", raw.get("capability")), _integer("adaptive coverage trials", raw.get("trials")), _integer("adaptive coverage parents", raw.get("parents")), _integer("adaptive qualifying parents", raw.get("qualifying_parents")), _integer("adaptive coverage abstentions", raw.get("abstentions")), tuple(AdaptiveShortfallReport.from_wire(item) for item in _sequence("adaptive coverage shortfalls", raw.get("shortfalls", []))))

    @property
    def met(self) -> bool:
        return not self.shortfalls


@dataclass(frozen=True)
class AdaptiveIccReport:
    raw: dict[str, Any]
    kind: str
    rho: float | None
    raw_estimate: float | None
    assumed: float | None
    reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveIccReport":
        raw = _mapping("adaptive ICC", value)
        return cls(raw, _route_text("adaptive ICC kind", raw.get("kind")), None if raw.get("rho") is None else _number("adaptive ICC rho", raw.get("rho")), None if raw.get("raw") is None else _number("adaptive ICC raw", raw.get("raw")), None if raw.get("assumed") is None else _number("adaptive ICC assumed", raw.get("assumed")), _optional_text("adaptive ICC reason", raw.get("reason")))


@dataclass(frozen=True)
class AdaptiveBetaPosteriorReport:
    raw: dict[str, Any]
    alpha: float
    beta: float

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveBetaPosteriorReport":
        raw = _mapping("adaptive beta posterior", value)
        return cls(raw, _number("adaptive beta alpha", raw.get("alpha")), _number("adaptive beta beta", raw.get("beta")))


@dataclass(frozen=True)
class AdaptiveEstimateReport:
    raw: dict[str, Any]
    capability: str
    trials: int
    successes: int
    abstentions: int
    parents: int
    posterior_mean: float
    icc: AdaptiveIccReport
    design_effect: float
    effective_trials: float
    naive_posterior: AdaptiveBetaPosteriorReport
    clustered_posterior: AdaptiveBetaPosteriorReport
    naive_interval: AdaptiveIntervalReport
    clustered_interval: AdaptiveIntervalReport
    bootstrap_interval: AdaptiveIntervalReport | None
    inflation: float
    caveat: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveEstimateReport":
        raw = _mapping("adaptive estimate", value)
        bootstrap_raw = raw.get("bootstrap_interval")
        return cls(
            raw,
            _route_text("adaptive estimate capability", raw.get("capability")),
            _integer("adaptive estimate trials", raw.get("trials")),
            _integer("adaptive estimate successes", raw.get("successes")),
            _integer("adaptive estimate abstentions", raw.get("abstentions")),
            _integer("adaptive estimate parents", raw.get("parents")),
            _number("adaptive posterior mean", raw.get("posterior_mean")),
            AdaptiveIccReport.from_wire(raw.get("icc")),
            _number("adaptive design effect", raw.get("design_effect")),
            _number("adaptive effective trials", raw.get("effective_trials")),
            AdaptiveBetaPosteriorReport.from_wire(raw.get("naive_posterior")),
            AdaptiveBetaPosteriorReport.from_wire(raw.get("clustered_posterior")),
            AdaptiveIntervalReport.from_wire(raw.get("naive_interval")),
            AdaptiveIntervalReport.from_wire(raw.get("clustered_interval")),
            None if bootstrap_raw is None else AdaptiveIntervalReport.from_wire(bootstrap_raw),
            _number("adaptive inflation", raw.get("inflation")),
            _route_text("adaptive estimate caveat", raw.get("caveat")),
        )

    @property
    def clustering_inflates_interval(self) -> bool:
        return self.clustered_interval.width >= self.naive_interval.width


@dataclass(frozen=True)
class AdaptiveStoppingReport:
    raw: dict[str, Any]
    capability: str
    reason: str
    stop: bool
    conclusive: bool
    trials: int
    effective_trials: float
    design_effect: float
    remaining_budget: int
    interval: AdaptiveIntervalReport
    best_case_width: float
    detail: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveStoppingReport":
        raw = _mapping("adaptive stopping verdict", value)
        return cls(raw, _route_text("adaptive stopping capability", raw.get("capability")), _route_text("adaptive stopping reason", raw.get("reason")), _bool("adaptive stopping stop", raw.get("stop")), _bool("adaptive stopping conclusive", raw.get("conclusive")), _integer("adaptive stopping trials", raw.get("trials")), _number("adaptive stopping effective trials", raw.get("effective_trials")), _number("adaptive stopping design effect", raw.get("design_effect")), _integer("adaptive stopping remaining budget", raw.get("remaining_budget")), AdaptiveIntervalReport.from_wire(raw.get("interval")), _number("adaptive stopping best-case width", raw.get("best_case_width")), _route_text("adaptive stopping detail", raw.get("detail")))


@dataclass(frozen=True)
class AdaptiveCapabilityAuditReport:
    raw: dict[str, Any]
    capability: str
    cost: float
    coverage: AdaptiveCoverageReport
    stopping: AdaptiveStoppingReport
    estimate: AdaptiveEstimateReport | None
    withheld: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveCapabilityAuditReport":
        raw = _mapping("adaptive capability audit", value)
        estimate_raw = raw.get("estimate")
        estimate = None if estimate_raw is None else AdaptiveEstimateReport.from_wire(estimate_raw)
        withheld = _optional_text("adaptive withheld reason", raw.get("withheld"))
        if estimate is None and withheld is None:
            raise ArgumentError("adaptive capability audit must explain a missing estimate")
        return cls(raw, _route_text("adaptive audit capability", raw.get("capability")), _number("adaptive capability cost", raw.get("cost")), AdaptiveCoverageReport.from_wire(raw.get("coverage")), AdaptiveStoppingReport.from_wire(raw.get("stopping")), estimate, withheld)


@dataclass(frozen=True)
class AdaptivePanelAuditReport:
    raw: dict[str, Any]
    trials: int
    scored_trials: int
    abstentions: int
    total_cost: float
    capabilities: tuple[AdaptiveCapabilityAuditReport, ...]
    caveat: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptivePanelAuditReport":
        raw = _mapping("adaptive panel audit", value)
        return cls(raw, _integer("adaptive audit trials", raw.get("trials")), _integer("adaptive audit scored trials", raw.get("scored_trials")), _integer("adaptive audit abstentions", raw.get("abstentions")), _number("adaptive audit total cost", raw.get("total_cost")), tuple(AdaptiveCapabilityAuditReport.from_wire(item) for item in _sequence("adaptive audit capabilities", raw.get("capabilities", []))), _route_text("adaptive audit caveat", raw.get("caveat")))

    @property
    def reported(self) -> int:
        return sum(estimate is not None for estimate in (item.estimate for item in self.capabilities))

    @property
    def withheld(self) -> int:
        return len(self.capabilities) - self.reported

    @property
    def effective_trials(self) -> float:
        return sum(item.estimate.effective_trials for item in self.capabilities if item.estimate is not None)


@dataclass(frozen=True)
class AdaptiveScoredCandidateReport:
    raw: dict[str, Any]
    instance: str
    capability: str
    parent: str
    score: float
    expected_variance_reduction: float
    independence_weight: float
    cost: float
    parent_trials_before: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveScoredCandidateReport":
        raw = _mapping("adaptive scored candidate", value)
        return cls(raw, _route_text("adaptive candidate instance", raw.get("instance")), _route_text("adaptive candidate capability", raw.get("capability")), _route_text("adaptive candidate parent", raw.get("parent")), _number("adaptive candidate score", raw.get("score")), _number("adaptive variance reduction", raw.get("expected_variance_reduction")), _number("adaptive independence weight", raw.get("independence_weight")), _number("adaptive candidate cost", raw.get("cost")), _integer("adaptive parent trials", raw.get("parent_trials_before")))


@dataclass(frozen=True)
class AdaptiveSelectionRecordReport:
    raw: dict[str, Any]
    chosen: AdaptiveScoredCandidateReport
    eligible: int
    already_run: int
    coverage_gated_out: int
    gated_by: dict[str, Any] | None
    runners_up: tuple[AdaptiveScoredCandidateReport, ...]
    icc_used: float
    icc_source: str
    caveat: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveSelectionRecordReport":
        raw = _mapping("adaptive selection record", value)
        gated_raw = raw.get("gated_by")
        return cls(raw, AdaptiveScoredCandidateReport.from_wire(raw.get("chosen")), _integer("adaptive eligible candidates", raw.get("eligible")), _integer("adaptive already-run candidates", raw.get("already_run")), _integer("adaptive coverage-gated candidates", raw.get("coverage_gated_out")), None if gated_raw is None else _mapping("adaptive coverage gate", gated_raw), tuple(AdaptiveScoredCandidateReport.from_wire(item) for item in _sequence("adaptive runners-up", raw.get("runners_up", []))), _number("adaptive selected ICC", raw.get("icc_used")), _route_text("adaptive ICC source", raw.get("icc_source")), _route_text("adaptive selection caveat", raw.get("caveat")))


@dataclass(frozen=True)
class AdaptiveSelectionReport:
    raw: dict[str, Any]
    state: str
    mode: str | None
    record: AdaptiveSelectionRecordReport | None
    records: tuple[AdaptiveSelectionRecordReport, ...]
    omitted: int
    refusal: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveSelectionReport":
        raw = _mapping("adaptive selection", value)
        ok = _bool("adaptive selection ok", raw.get("ok"))
        if not ok:
            return cls(raw, "refused", None, None, (), 0, _route_text("adaptive selection refusal", raw.get("refusal")))
        content = _mapping("adaptive selection value", raw.get("value"))
        mode = _route_text("adaptive selection mode", content.get("mode"))
        if mode == "batch":
            records = tuple(AdaptiveSelectionRecordReport.from_wire(item) for item in _sequence("adaptive batch records", content.get("records", [])))
            return cls(raw, "selected", mode, None, records, _integer("adaptive batch omitted", content.get("omitted", 0)), None)
        return cls(raw, "selected", mode, AdaptiveSelectionRecordReport.from_wire(content.get("record")), (), 0, None)


@dataclass(frozen=True)
class AdaptiveCapabilityViewReport:
    raw: dict[str, Any]
    capability: str
    coverage: AdaptiveCoverageReport
    stopping: AdaptiveStoppingReport | None
    stopping_refusal: str | None
    estimate: AdaptiveEstimateReport | None
    estimate_refusal: str | None
    fail_closed: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveCapabilityViewReport":
        raw = _mapping("adaptive capability view", value)
        stopping_raw = raw.get("stopping")
        estimate_raw = raw.get("estimate")
        return cls(raw, _route_text("adaptive view capability", raw.get("capability")), AdaptiveCoverageReport.from_wire(raw.get("coverage")), None if stopping_raw is None else AdaptiveStoppingReport.from_wire(stopping_raw), _optional_text("adaptive stopping refusal", raw.get("stopping_refusal")), None if estimate_raw is None else AdaptiveEstimateReport.from_wire(estimate_raw), _optional_text("adaptive estimate refusal", raw.get("estimate_refusal")), _bool("adaptive capability fail_closed", raw.get("fail_closed")))


@dataclass(frozen=True)
class AdaptiveComparisonReport:
    raw: dict[str, Any]
    left: str
    right: str
    left_mean: float
    right_mean: float
    left_effective_trials: float
    right_effective_trials: float
    probability_left_exceeds_right: float
    naive_probability_left_exceeds_right: float
    intervals_disjoint: bool
    caveat: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptiveComparisonReport":
        raw = _mapping("adaptive comparison", value)
        return cls(raw, _route_text("adaptive comparison left", raw.get("left")), _route_text("adaptive comparison right", raw.get("right")), _number("adaptive left mean", raw.get("left_mean")), _number("adaptive right mean", raw.get("right_mean")), _number("adaptive left effective trials", raw.get("left_effective_trials")), _number("adaptive right effective trials", raw.get("right_effective_trials")), _number("adaptive clustered probability", raw.get("probability_left_exceeds_right")), _number("adaptive naive probability", raw.get("naive_probability_left_exceeds_right")), _bool("adaptive intervals disjoint", raw.get("intervals_disjoint")), _route_text("adaptive comparison caveat", raw.get("caveat")))


@dataclass(frozen=True)
class AdaptivePanelReport:
    """Validated adaptive audit plus selection, query, comparison, and stop projections."""

    raw: dict[str, Any]
    ok: bool
    schema: str
    audit: AdaptivePanelAuditReport
    audit_summary: dict[str, Any]
    audit_digest: str | None
    selection: AdaptiveSelectionReport | None
    capability: AdaptiveCapabilityViewReport | None
    comparison: AdaptiveComparisonReport | None
    comparison_refusal: str | None
    finished: bool | None
    finished_refusal: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdaptivePanelReport":
        raw = _payload(value)
        ok = _bool("adaptive panel ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("adaptive panel structured reports must be successful; refusals remain transport errors")
        schema = _route_text("adaptive panel schema", raw.get("schema"))
        if schema != ADAPTIVE_PANEL_SCHEMA:
            raise ArgumentError(f"unsupported adaptive panel schema {schema!r}")
        audit = AdaptivePanelAuditReport.from_wire(raw.get("audit"))
        summary = _mapping("adaptive audit summary", raw.get("audit_summary"))
        if _integer("adaptive summary trials", summary.get("trials")) != audit.trials or _integer("adaptive summary scored", summary.get("scored_trials")) != audit.scored_trials or _integer("adaptive summary abstentions", summary.get("abstentions")) != audit.abstentions or _integer("adaptive summary capabilities", summary.get("capabilities")) != len(audit.capabilities):
            raise ArgumentError("adaptive audit summary does not reconcile with audit")
        if _integer("adaptive summary reported", summary.get("reported")) != audit.reported or _integer("adaptive summary withheld", summary.get("withheld")) != audit.withheld:
            raise ArgumentError("adaptive audit reportability counts do not reconcile")
        selection_raw = raw.get("selection")
        selection = None if selection_raw is None else AdaptiveSelectionReport.from_wire(selection_raw)
        capability_raw = raw.get("capability")
        comparison_raw = raw.get("comparison")
        comparison = None
        comparison_refusal = None
        if comparison_raw is not None:
            comparison_wrapper = _mapping("adaptive comparison result", comparison_raw)
            if _bool("adaptive comparison ok", comparison_wrapper.get("ok")):
                comparison = AdaptiveComparisonReport.from_wire(comparison_wrapper.get("value"))
            else:
                comparison_refusal = _route_text("adaptive comparison refusal", comparison_wrapper.get("refusal"))
        return cls(raw, True, schema, audit, summary, _optional_text("adaptive audit digest", raw.get("audit_digest")), selection, None if capability_raw is None else AdaptiveCapabilityViewReport.from_wire(capability_raw), comparison, comparison_refusal, _optional_bool("adaptive finished", raw.get("finished")), _optional_text("adaptive finished refusal", raw.get("finished_refusal")), _texts("adaptive guarantees", raw.get("guarantees", [])), _texts("adaptive limitations", raw.get("limitations", [])))

    @property
    def all_touched_capabilities_reported(self) -> bool:
        return self.audit.withheld == 0

    @property
    def selection_was_refused(self) -> bool:
        return self.selection is not None and self.selection.state == "refused"

    @property
    def reportable_estimates_are_clustered(self) -> bool:
        return all(item.estimate is None or item.estimate.clustering_inflates_interval for item in self.audit.capabilities)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def adaptive_panel_report(value: Mapping[str, Any]) -> AdaptivePanelReport:
    """Parse a direct MCP result or HTTP envelope into a typed adaptive report."""

    return AdaptivePanelReport.from_wire(value)


__all__ = [
    "ADAPTIVE_PANEL_SCHEMA",
    "ADAPTIVE_MAX_CANDIDATES",
    "ADAPTIVE_MAX_ITEMS",
    "AdaptivePanelRunArgs",
    "AdaptiveIntervalReport",
    "AdaptiveShortfallReport",
    "AdaptiveCoverageReport",
    "AdaptiveIccReport",
    "AdaptiveBetaPosteriorReport",
    "AdaptiveEstimateReport",
    "AdaptiveStoppingReport",
    "AdaptiveCapabilityAuditReport",
    "AdaptivePanelAuditReport",
    "AdaptiveScoredCandidateReport",
    "AdaptiveSelectionRecordReport",
    "AdaptiveSelectionReport",
    "AdaptiveCapabilityViewReport",
    "AdaptiveComparisonReport",
    "AdaptivePanelReport",
    "adaptive_panel_report",
]
