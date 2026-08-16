"""Typed influence-analysis contracts.

Influence analysis is a soundness boundary, not a generic numeric helper.  The Rust kernel emits
either a bounded total-variation estimate with method and validity provenance or a named unknown
reason.  This module validates the request envelope and keeps those two states distinct across
MCP and HTTP transports while leaving the factor graph itself authoritative in Rust.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


INFLUENCE_METHODS = frozenset(
    {
        "exact_removal",
        "dynamic_range",
        "ratio_composition",
        "chain_contraction",
        "structural_zero",
        "abstract_interpretation",
        "widened_abstract_interpretation",
    }
)
INFLUENCE_METRICS = frozenset({"total_variation_on_normalised_answer"})
INFLUENCE_APPROXIMATIONS = frozenset({"exact", "conservative_upper_bound"})
INFLUENCE_PERTURBATIONS = frozenset({"removal", "multiplicative_range"})


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _object(name: str, value: Any) -> dict[str, Any]:
    return _route_mapping(name, value)


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    return None if value is None else _route_mapping(name, value)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _finite_number(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _positive_integer(name: str, value: Any, *, maximum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ArgumentError(f"{name} must be a positive integer")
    if maximum is not None and value > maximum:
        raise ArgumentError(f"{name} must be at most {maximum}")
    return value


def _strings(name: str, value: Any) -> tuple[str, ...]:
    values = _array(name, value)
    result: list[str] = []
    for index, item in enumerate(values):
        result.append(_route_text(f"{name}[{index}]", item))
    return tuple(result)


def _mapping_sequence(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    values = _array(name, value)
    return tuple(_object(f"{name}[{index}]", item) for index, item in enumerate(values))


def _payload(value: Mapping[str, Any], *, label: str) -> dict[str, Any]:
    raw = _route_mapping(f"{label} response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and "analysis" in candidate

    if matches(raw):
        return raw
    envelopes: list[Mapping[str, Any]] = [raw]
    mcp = raw.get("mcp")
    if isinstance(mcp, Mapping):
        envelopes.append(mcp)
    for envelope in envelopes:
        result = envelope.get("result")
        candidates: list[Mapping[str, Any]] = [envelope]
        if isinstance(result, Mapping):
            candidates.append(result)
        for candidate in candidates:
            structured = candidate.get("structuredContent")
            if isinstance(structured, Mapping) and matches(structured):
                return dict(structured)
            content = candidate.get("content")
            if not isinstance(content, Sequence) or isinstance(content, (str, bytes)):
                continue
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"{label} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {label} response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain an {label} projection")


def _validate_perturbation(name: str, value: Any) -> dict[str, Any]:
    perturbation = _object(name, value)
    class_name = _route_text(f"{name}.class", perturbation.get("class"))
    if class_name not in INFLUENCE_PERTURBATIONS:
        raise ArgumentError(f"{name}.class is not a supported influence perturbation")
    if class_name == "multiplicative_range":
        ratio = _object(f"{name}.range", perturbation.get("range"))
        lo = _finite_number(f"{name}.range.lo", ratio.get("lo"))
        hi = _finite_number(f"{name}.range.hi", ratio.get("hi"))
        if lo < 0 or hi < 0 or lo > hi:
            raise ArgumentError(f"{name}.range must satisfy 0 <= lo <= hi")
    return perturbation


def _validate_factor(name: str, value: Any) -> dict[str, Any]:
    factor = _object(name, value)
    _route_text(f"{name}.id", factor.get("id"))
    scope = _strings(f"{name}.scope", factor.get("scope"))
    if not 1 <= len(scope) <= 128:
        raise ArgumentError(f"{name}.scope must contain between 1 and 128 variables")
    if factor.get("table") is not None:
        table = _array(f"{name}.table", factor.get("table"))
        if len(table) > 4_194_304:
            raise ArgumentError(f"{name}.table must contain at most 4194304 entries")
        for index, item in enumerate(table):
            _finite_number(f"{name}.table[{index}]", item)
    return factor


@dataclass(frozen=True)
class InfluenceAnalyzeArgs:
    """A bounded caller-declared factor-region influence request."""

    label: str
    variables: Mapping[str, int]
    factors: tuple[Mapping[str, Any], ...]
    free: tuple[str, ...]
    perturbation: Mapping[str, Any]
    factor: str | None = None
    factor_group: tuple[str, ...] | None = None
    assumed_variables: tuple[str, ...] = ()
    budget: Mapping[str, Any] | None = None
    execute: bool = False

    def __post_init__(self) -> None:
        label = _route_text("influence label", self.label)
        if not label.strip():
            raise ArgumentError("influence label must not be empty")
        object.__setattr__(self, "label", label)
        variables = _object("influence variables", self.variables)
        if not 1 <= len(variables) <= 10_000:
            raise ArgumentError("influence variables must contain between 1 and 10000 entries")
        normalized_variables: dict[str, int] = {}
        for name, cardinality in variables.items():
            if not isinstance(name, str) or not name:
                raise ArgumentError("influence variable names must be non-empty strings")
            normalized_variables[name] = _positive_integer(
                f"influence variables[{name!r}]", cardinality, maximum=1_000_000
            )
        object.__setattr__(self, "variables", normalized_variables)

        factors = tuple(_validate_factor(f"influence factors[{index}]", factor) for index, factor in enumerate(_mapping_sequence("influence factors", self.factors)))
        if not 1 <= len(factors) <= 1_000:
            raise ArgumentError("influence factors must contain between 1 and 1000 entries")
        object.__setattr__(self, "factors", factors)

        assumed = _strings("influence assumed_variables", self.assumed_variables)
        if len(set(assumed)) != len(assumed):
            raise ArgumentError("influence assumed_variables must not contain duplicates")
        unknown_assumed = set(assumed) - set(normalized_variables)
        if unknown_assumed:
            raise ArgumentError(f"assumed_variables names undeclared variables: {sorted(unknown_assumed)!r}")
        object.__setattr__(self, "assumed_variables", assumed)

        free = _strings("influence free", self.free)
        if not 1 <= len(free) <= len(normalized_variables):
            raise ArgumentError("influence free must contain at least one declared variable")
        if len(set(free)) != len(free):
            raise ArgumentError("influence free must not contain duplicate variables")
        if set(free) - set(normalized_variables):
            raise ArgumentError("influence free must contain only declared variables")
        object.__setattr__(self, "free", free)
        object.__setattr__(self, "perturbation", _validate_perturbation("influence perturbation", self.perturbation))

        if (self.factor is None) == (self.factor_group is None):
            raise ArgumentError("provide exactly one of factor or factor_group")
        if self.factor is not None:
            factor = _route_text("influence factor", self.factor)
            if not factor:
                raise ArgumentError("influence factor must not be empty")
            object.__setattr__(self, "factor", factor)
        else:
            group = _strings("influence factor_group", self.factor_group)
            if not 1 <= len(group) <= 1_000:
                raise ArgumentError("influence factor_group must contain between 1 and 1000 ids")
            if len(set(group)) != len(group):
                raise ArgumentError("influence factor_group must not contain duplicate ids")
            object.__setattr__(self, "factor_group", group)
        object.__setattr__(self, "budget", _optional_mapping("influence budget", self.budget))
        if self.budget is not None:
            for key, maximum in (("max_induced_width", 1024),):
                if key in self.budget:
                    _positive_integer(f"influence budget.{key}", self.budget[key], maximum=maximum)
            for key in ("max_peak_entries", "max_ops"):
                if key in self.budget and _finite_number(f"influence budget.{key}", self.budget[key]) <= 0:
                    raise ArgumentError(f"influence budget.{key} must be positive")
        object.__setattr__(self, "execute", _bool("influence execute", self.execute))

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "InfluenceAnalyzeArgs":
        raw = _object("influence arguments", value)
        return cls(
            raw.get("label"),
            raw.get("variables"),
            _mapping_sequence("influence factors", raw.get("factors")),
            _strings("influence free", raw.get("free")),
            raw.get("perturbation"),
            raw.get("factor"),
            None if raw.get("factor_group") is None else _strings("influence factor_group", raw.get("factor_group")),
            _strings("influence assumed_variables", raw.get("assumed_variables", ())),
            raw.get("budget"),
            raw.get("execute", False),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "label": self.label,
            "variables": dict(self.variables),
            "assumed_variables": list(self.assumed_variables),
            "factors": [dict(factor) for factor in self.factors],
            "free": list(self.free),
            "perturbation": dict(self.perturbation),
            "execute": self.execute,
        }
        if self.factor is not None:
            result["factor"] = self.factor
        if self.factor_group is not None:
            result["factor_group"] = list(self.factor_group)
        if self.budget is not None:
            result["budget"] = dict(self.budget)
        return result


def _validate_estimate(name: str, value: Any) -> dict[str, Any]:
    estimate = _object(name, value)
    kind = _route_text(f"{name}.kind", estimate.get("kind"))
    if kind == "bounded":
        bound = estimate
        amount = _finite_number(f"{name}.value", bound.get("value"))
        if not 0.0 <= amount <= 1.0:
            raise ArgumentError(f"{name}.value must lie in [0, 1]")
        metric = _route_text(f"{name}.metric", bound.get("metric"))
        if metric not in INFLUENCE_METRICS:
            raise ArgumentError(f"{name}.metric is not recognized")
        method = _route_text(f"{name}.method", bound.get("method"))
        if method not in INFLUENCE_METHODS:
            raise ArgumentError(f"{name}.method is not recognized")
        approximation = _route_text(f"{name}.approximation", bound.get("approximation"))
        if approximation not in INFLUENCE_APPROXIMATIONS:
            raise ArgumentError(f"{name}.approximation is not recognized")
        _route_text(f"{name}.validity", bound.get("validity"))
    elif kind == "unknown":
        reason = _object(f"{name}.reason", estimate.get("reason"))
        _route_text(f"{name}.reason.reason", reason.get("reason"))
    else:
        raise ArgumentError(f"{name}.kind must be bounded or unknown")
    return estimate


def _validate_analysis(value: Any) -> dict[str, Any]:
    analysis = _object("influence analysis", value)
    subjects = _strings("influence analysis.subject", analysis.get("subject"))
    perturbation = _validate_perturbation("influence analysis.perturbation", analysis.get("perturbation"))
    _validate_estimate("influence analysis.estimate", analysis.get("estimate"))
    attempted = _mapping_sequence("influence analysis.attempted", analysis.get("attempted"))
    for index, outcome in enumerate(attempted):
        method = _route_text(f"influence analysis.attempted[{index}].method", outcome.get("method"))
        if method not in INFLUENCE_METHODS:
            raise ArgumentError(f"influence analysis.attempted[{index}].method is not recognized")
        has_value = outcome.get("value") is not None
        has_declined = outcome.get("declined") is not None
        if has_value == has_declined:
            raise ArgumentError(f"influence analysis.attempted[{index}] must contain exactly one outcome")
        if has_value:
            amount = _finite_number(f"influence analysis.attempted[{index}].value", outcome["value"])
            if not 0.0 <= amount <= 1.0:
                raise ArgumentError(f"influence analysis.attempted[{index}].value must lie in [0, 1]")
        else:
            _object(f"influence analysis.attempted[{index}].declined", outcome["declined"])
    return {**analysis, "subject": list(subjects), "perturbation": perturbation}


@dataclass(frozen=True)
class InfluenceAnalysisReport:
    raw: dict[str, Any]
    ok: bool
    region: dict[str, Any]
    subjects: tuple[str, ...]
    perturbation: dict[str, Any]
    analysis: dict[str, Any]
    estimate: dict[str, Any]
    attempted: tuple[dict[str, Any], ...]
    execute: bool
    looseness: float | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "InfluenceAnalysisReport":
        raw = _payload(value, label="influence analysis")
        ok = _bool("influence analysis ok", raw.get("ok"))
        if not ok:
            raise ArgumentError("influence analysis projection must be successful; transport errors should remain raw")
        region = _object("influence region", raw.get("region"))
        _route_text("influence region.label", region.get("label"))
        variables = _object("influence region.variables", region.get("variables"))
        free = _strings("influence region.free", region.get("free"))
        bound = _strings("influence region.bound", region.get("bound"))
        factors = _mapping_sequence("influence region.factors", region.get("factors"))
        for index, factor in enumerate(factors):
            _route_text(f"influence region.factors[{index}].id", factor.get("id"))
            _strings(f"influence region.factors[{index}].scope", factor.get("scope"))
            _positive_integer(f"influence region.factors[{index}].arity", factor.get("arity"))
            _bool(f"influence region.factors[{index}].has_table", factor.get("has_table"))
        for name, item in (("has_tables", region.get("has_tables")),):
            _bool(f"influence region.{name}", item)
        for name in ("joint_entries", "free_entries"):
            _route_count(f"influence region.{name}", region.get(name))
        fraction = _finite_number("influence region.assumed_cardinality_fraction", region.get("assumed_cardinality_fraction"))
        if not 0.0 <= fraction <= 1.0:
            raise ArgumentError("influence region.assumed_cardinality_fraction must lie in [0, 1]")
        analysis = _validate_analysis(raw.get("analysis"))
        estimate = _validate_estimate("influence analysis.estimate", analysis["estimate"])
        attempted = _mapping_sequence("influence analysis.attempted", analysis["attempted"])
        execute = _bool("influence execute", raw.get("execute"))
        looseness = None if raw.get("looseness") is None else _finite_number("influence looseness", raw.get("looseness"))
        return cls(
            raw=raw,
            ok=ok,
            region={**region, "variables": variables, "free": list(free), "bound": list(bound), "factors": list(factors)},
            subjects=tuple(analysis["subject"]),
            perturbation=analysis["perturbation"],
            analysis=analysis,
            estimate=estimate,
            attempted=attempted,
            execute=execute,
            looseness=looseness,
            guarantees=_route_strings("influence guarantees", raw.get("guarantees", ())),
        )

    @property
    def bounded(self) -> bool:
        return self.estimate.get("kind") == "bounded"

    @property
    def unknown(self) -> bool:
        return self.estimate.get("kind") == "unknown"

    @property
    def bound_value(self) -> float | None:
        value = self.estimate.get("value") if self.bounded else None
        return None if value is None else float(value)

    @property
    def method(self) -> str | None:
        value = self.estimate.get("method") if self.bounded else None
        return value if isinstance(value, str) else None

    @property
    def exact(self) -> bool:
        return self.estimate.get("approximation") == "exact"


def influence_analysis_report(value: Mapping[str, Any]) -> InfluenceAnalysisReport:
    return InfluenceAnalysisReport.from_wire(value)
