"""Typed biological-stress profiling contracts.

The Rust stress engine reports a breaking-point profile, not a scalar robustness score.  These
projections keep the declared cohort/stress/procedure inputs opaque to the SDK while making the
evidence-bearing outer contract typed: identifiability, generator postconditions, effective
sample size, unresolved measurements, and required-versus-probed findings remain inspectable.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


STRESS_FAMILIES = frozenset(
    {"prevalence_shift", "batch_effect", "assay_degradation", "segmentation_jitter"}
)
STRESS_IDENTIFIABILITY = frozenset({"not_applicable", "separable", "confounded"})
STRESS_OBLIGATIONS = frozenset({"required", "probed"})


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


def _payload(value: Mapping[str, Any], *, label: str, direct_keys: tuple[str, ...]) -> dict[str, Any]:
    """Extract a domain projection from direct, MCP, REST, or text envelopes."""

    raw = _route_mapping(f"{label} response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        return "ok" in candidate and any(key in candidate for key in direct_keys)

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
    raise ArgumentError(f"response does not contain a {label} projection")


def _mapping_sequence(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    values = _array(name, value)
    result: list[dict[str, Any]] = []
    for index, item in enumerate(values):
        result.append(_object(f"{name}[{index}]", item))
    return tuple(result)


def _identifiability(name: str, value: Any) -> dict[str, Any]:
    result = _object(name, value)
    tag = result.get("identifiability")
    if tag not in STRESS_IDENTIFIABILITY:
        raise ArgumentError(f"{name}.identifiability is not a recognized stress state")
    if tag == "separable":
        _route_text(f"{name}.batch", result.get("batch"))
    if tag == "confounded":
        _route_text(f"{name}.batch", result.get("batch"))
        only = _route_text(f"{name}.only", result.get("only"))
        if only not in {"positive", "negative"}:
            raise ArgumentError(f"{name}.only must identify positive or negative subjects")
    return result


def _validate_profile(name: str, value: Any) -> dict[str, Any]:
    profile = _object(name, value)
    family = profile.get("family")
    if family not in STRESS_FAMILIES:
        raise ArgumentError(f"{name}.family is not a recognized stress family")
    _route_text(f"{name}.blueprint_module", profile.get("blueprint_module"))
    _route_text(f"{name}.stress_id", profile.get("stress_id"))
    _route_text(f"{name}.cohort_id", profile.get("cohort_id"))
    _route_text(f"{name}.parent_digest", profile.get("parent_digest"))
    _identifiability(f"{name}.identifiability", profile.get("identifiability"))
    sweep = _mapping_sequence(f"{name}.sweep", profile.get("sweep"))
    if not sweep:
        raise ArgumentError(f"{name}.sweep must not be empty")
    for index, point in enumerate(sweep):
        _route_count(f"{name}.sweep[{index}].magnitude", point.get("magnitude"))
        effective_n = point.get("effective_n")
        if isinstance(effective_n, bool) or not isinstance(effective_n, (int, float)):
            raise ArgumentError(f"{name}.sweep[{index}].effective_n must be numeric")
        _route_count(f"{name}.sweep[{index}].nominal_n", point.get("nominal_n"))
        _route_count(f"{name}.sweep[{index}].unresolved", point.get("unresolved"))
        prevalence = point.get("analysable_prevalence")
        if isinstance(prevalence, bool) or not isinstance(prevalence, (int, float)):
            raise ArgumentError(f"{name}.sweep[{index}].analysable_prevalence must be numeric")
        _bool(f"{name}.sweep[{index}].abandoned", point.get("abandoned"))
    findings = _mapping_sequence(f"{name}.findings", profile.get("findings"))
    for index, finding in enumerate(findings):
        _route_text(f"{name}.findings[{index}].conclusion_id", finding.get("conclusion_id"))
        character = _route_text(f"{name}.findings[{index}].character", finding.get("character"))
        if character not in {"discriminative", "calibrated", "geometric"}:
            raise ArgumentError(f"{name}.findings[{index}].character is not recognized")
        obligation = _route_text(f"{name}.findings[{index}].obligation", finding.get("obligation"))
        if obligation not in STRESS_OBLIGATIONS:
            raise ArgumentError(f"{name}.findings[{index}].obligation is not recognized")
        _route_text(f"{name}.findings[{index}].relation", finding.get("relation"))
        _route_text(f"{name}.findings[{index}].rationale", finding.get("rationale"))
        for optional in ("held_through", "broke_at"):
            if finding.get(optional) is not None:
                _route_count(f"{name}.findings[{index}].{optional}", finding[optional])
    defects = _mapping_sequence(f"{name}.generator_defects", profile.get("generator_defects"))
    for index, defect in enumerate(defects):
        _route_count(f"{name}.generator_defects[{index}].magnitude", defect.get("magnitude"))
        for field in ("invariant", "expected", "observed"):
            _route_text(f"{name}.generator_defects[{index}].{field}", defect.get(field))
    _route_text(f"{name}.caveat", profile.get("caveat"))
    return profile


@dataclass(frozen=True)
class StressProfileArgs:
    """A bounded single-family stress request."""

    cohort: Mapping[str, Any]
    stress: Mapping[str, Any]
    procedures: tuple[Mapping[str, Any], ...] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "cohort", _object("stress cohort", self.cohort))
        object.__setattr__(self, "stress", _object("stress specification", self.stress))
        if self.procedures is not None:
            procedures = _mapping_sequence("stress procedures", self.procedures)
            if len(procedures) > 100:
                raise ArgumentError("stress procedures must contain at most 100 entries")
            object.__setattr__(self, "procedures", procedures)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StressProfileArgs":
        raw = _object("stress profile arguments", value)
        procedures = None if raw.get("procedures") is None else _mapping_sequence("stress procedures", raw.get("procedures"))
        return cls(raw.get("cohort"), raw.get("stress"), procedures)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"cohort": dict(self.cohort), "stress": dict(self.stress)}
        if self.procedures is not None:
            result["procedures"] = [dict(procedure) for procedure in self.procedures]
        return result


@dataclass(frozen=True)
class StressReportArgs:
    """A bounded multi-family stress program request."""

    cohort: Mapping[str, Any]
    stresses: tuple[Mapping[str, Any], ...]
    procedures: tuple[Mapping[str, Any], ...] | None = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "cohort", _object("stress cohort", self.cohort))
        stresses = _mapping_sequence("stress program", self.stresses)
        if len(stresses) > 100:
            raise ArgumentError("stress program must contain at most 100 entries")
        object.__setattr__(self, "stresses", stresses)
        if self.procedures is not None:
            procedures = _mapping_sequence("stress procedures", self.procedures)
            if len(procedures) > 100:
                raise ArgumentError("stress procedures must contain at most 100 entries")
            object.__setattr__(self, "procedures", procedures)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StressReportArgs":
        raw = _object("stress report arguments", value)
        procedures = None if raw.get("procedures") is None else _mapping_sequence("stress procedures", raw.get("procedures"))
        return cls(raw.get("cohort"), _mapping_sequence("stress program", raw.get("stresses")), procedures)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "cohort": dict(self.cohort),
            "stresses": [dict(stress) for stress in self.stresses],
        }
        if self.procedures is not None:
            result["procedures"] = [dict(procedure) for procedure in self.procedures]
        return result


@dataclass(frozen=True)
class StressProfileReport:
    raw: dict[str, Any]
    ok: bool
    headline: str | None
    profile: dict[str, Any] | None
    family: str | None
    identifiability: dict[str, Any] | None
    sweep: tuple[dict[str, Any], ...]
    findings: tuple[dict[str, Any], ...]
    generator_defects: tuple[dict[str, Any], ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StressProfileReport":
        raw = _payload(value, label="stress profile", direct_keys=("profile", "stage"))
        ok = _bool("stress profile ok", raw.get("ok"))
        headline = None if raw.get("headline") is None else _route_text("stress profile headline", raw.get("headline"))
        stage = None if raw.get("stage") is None else _route_text("stress profile stage", raw.get("stage"))
        refusal = None if raw.get("refusal") is None else _route_text("stress profile refusal", raw.get("refusal"))
        guarantee = None if raw.get("guarantee") is None else _route_text("stress profile guarantee", raw.get("guarantee"))
        if ok:
            if raw.get("profile") is None or stage is not None or refusal is not None:
                raise ArgumentError("successful stress profiles must contain only a profile outcome")
            profile = _validate_profile("stress profile.profile", raw.get("profile"))
            if headline is None:
                raise ArgumentError("successful stress profiles must contain a headline")
            identifiability = _identifiability("stress profile.identifiability", profile["identifiability"])
            sweep = _mapping_sequence("stress profile.sweep", profile["sweep"])
            findings = _mapping_sequence("stress profile.findings", profile["findings"])
            defects = _mapping_sequence("stress profile.generator_defects", profile["generator_defects"])
        else:
            if stage != "stress_profile" or refusal is None or not _bool("stress profile fail_closed", raw.get("fail_closed")):
                raise ArgumentError("refused stress profiles must be fail-closed and identify stress_profile")
            profile = None
            identifiability = None
            sweep = ()
            findings = ()
            defects = ()
        return cls(
            raw=raw,
            ok=ok,
            headline=headline,
            profile=profile,
            family=None if profile is None else profile["family"],
            identifiability=identifiability,
            sweep=sweep,
            findings=findings,
            generator_defects=defects,
            stage=stage,
            refusal=refusal,
            fail_closed=False if ok else True,
            guarantee=guarantee,
            guarantees=_route_strings("stress profile guarantees", raw.get("guarantees", ())) if ok else (),
            limitations=_route_strings("stress profile limitations", raw.get("limitations", ())) if ok else (),
        )

    @property
    def informative(self) -> bool:
        return self.identifiability is not None and self.identifiability.get("identifiability") != "confounded"

    @property
    def generator_sound(self) -> bool:
        return not self.generator_defects


@dataclass(frozen=True)
class StressReportProjection:
    raw: dict[str, Any]
    ok: bool
    headline: str | None
    cohort_id: str | None
    report: dict[str, Any] | None
    profiles: tuple[dict[str, Any], ...]
    worst_family: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "StressReportProjection":
        raw = _payload(value, label="stress report", direct_keys=("report", "stage"))
        ok = _bool("stress report ok", raw.get("ok"))
        headline = None if raw.get("headline") is None else _route_text("stress report headline", raw.get("headline"))
        stage = None if raw.get("stage") is None else _route_text("stress report stage", raw.get("stage"))
        refusal = None if raw.get("refusal") is None else _route_text("stress report refusal", raw.get("refusal"))
        guarantee = None if raw.get("guarantee") is None else _route_text("stress report guarantee", raw.get("guarantee"))
        if ok:
            if raw.get("report") is None or stage is not None or refusal is not None:
                raise ArgumentError("successful stress reports must contain only a report outcome")
            report = _object("stress report.report", raw.get("report"))
            cohort_id = _route_text("stress report.report.cohort_id", report.get("cohort_id"))
            profiles = _mapping_sequence("stress report.report.profiles", report.get("profiles"))
            for index, profile in enumerate(profiles):
                _validate_profile(f"stress report.report.profiles[{index}]", profile)
            worst_family = _optional_mapping("stress report.worst_family", raw.get("worst_family"))
            if worst_family is not None:
                _validate_profile("stress report.worst_family", worst_family)
        else:
            if stage != "stress_report" or refusal is None or not _bool("stress report fail_closed", raw.get("fail_closed")):
                raise ArgumentError("refused stress reports must be fail-closed and identify stress_report")
            report = None
            cohort_id = None
            profiles = ()
            worst_family = None
        return cls(
            raw=raw,
            ok=ok,
            headline=headline,
            cohort_id=cohort_id,
            report=report,
            profiles=profiles,
            worst_family=worst_family,
            stage=stage,
            refusal=refusal,
            fail_closed=False if ok else True,
            guarantee=guarantee,
            guarantees=_route_strings("stress report guarantees", raw.get("guarantees", ())) if ok else (),
            limitations=_route_strings("stress report limitations", raw.get("limitations", ())) if ok else (),
        )

    @property
    def comparable(self) -> bool:
        """Whether the guarded worst-family comparison yielded a usable profile."""

        return self.worst_family is not None


def stress_profile_report(value: Mapping[str, Any]) -> StressProfileReport:
    return StressProfileReport.from_wire(value)


def stress_report_projection(value: Mapping[str, Any]) -> StressReportProjection:
    return StressReportProjection.from_wire(value)
