"""Typed oncology research-boundary contracts.

The OncoWorld boundary is a splitter: safe aggregate research may be released while individual
clinical use is refused and routed to a human process.  This module preserves that partial-release
state and the direct-identifier fail-closed refusal without offering any clinical recommendation.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ONCO_OUTPUT_USES = frozenset(
    {
        "cohort_analysis",
        "method_development",
        "hypothesis_generation",
        "quality_control",
        "individual_diagnosis",
        "individual_prognosis",
        "treatment_recommendation",
        "care_triage",
        "clinical_alerting",
    }
)
ONCO_DISPOSITIONS = frozenset({"release_in_full", "release_partial", "refuse_and_escalate"})
ONCO_TERMINAL_ACTIONS = frozenset({"stop", "abstain", "escalate"})
ONCO_WORLDLINE_SCHEMA = "bioprism-mcp/onco-worldline-view/0.1"
ONCO_WORLDLINE_CLOCK_AXES = ("acquired", "recorded", "released", "visible")
ONCO_WORLDLINE_VISIBILITY_STATES = frozenset({"visible", "hidden_from_agent", "not_filtered"})
ONCO_RESPONSE_SCHEMA = "bioprism-mcp/onco-response-assess/0.1"
ONCO_RESPONSE_CALL_KINDS = frozenset({"complete", "partial", "stable", "progression", "not_evaluable"})
ONCO_RESPONSE_OUTCOME_KINDS = frozenset({"assessment", "refused"})
ONCO_RESPONSE_REFUSAL_KINDS = frozenset({"assessment_error"})
ONCO_BOUNDARY_SCHEMA = "bioprism-mcp/onco-boundary-check/0.1"
ONCO_BOUNDARY_OUTCOME_KINDS = frozenset({"disposition", "refused"})
ONCO_BOUNDARY_REFUSAL_KINDS = frozenset({"identifiers_present"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    raw = _route_mapping("oncology boundary response", value)
    if "ok" in raw and any(key in raw for key in ("disposition", "stage", "permitted")):
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
            if isinstance(structured, Mapping) and "ok" in structured:
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
                    raise ArgumentError(f"oncology boundary response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping("decoded oncology boundary response", decoded)
                if "ok" in decoded_mapping:
                    return decoded_mapping
    raise ArgumentError("response does not contain an oncology boundary projection")


def _projection_payload(
    value: Mapping[str, Any],
    *,
    description: str,
    direct_keys: tuple[str, ...],
) -> dict[str, Any]:
    """Extract one typed oncology projection from direct, MCP, or REST envelopes.

    REST responses carry a transport-level ``ok`` and MCP responses may carry a structured
    result one or two envelopes below it.  Looking for a domain marker as well as ``ok`` keeps
    those transport fields from being mistaken for a successful domain projection.
    """

    raw = _route_mapping(f"{description} response", value)

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
                    raise ArgumentError(f"{description} response text is not JSON: {error}") from error
                decoded_mapping = _route_mapping(f"decoded {description} response", decoded)
                if matches(decoded_mapping):
                    return decoded_mapping
    raise ArgumentError(f"response does not contain a {description} projection")


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    return None if value is None else _route_mapping(name, value)


def _finite_nonnegative(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    result = float(value)
    if result < 0:
        raise ArgumentError(f"{name} must be non-negative")
    return result


@dataclass(frozen=True)
class OncoResponseAssessArgs:
    """Serialized inputs for the criteria-aware response and progression gate."""

    criterion: Mapping[str, Any]
    baseline: Mapping[str, Any]
    current: Mapping[str, Any]
    current_acquired: str
    baseline_clinical: Mapping[str, Any]
    current_clinical: Mapping[str, Any]
    treatment: Mapping[str, Any]
    evidence: Mapping[str, Any] | None = None
    nadir_spd_mm2: float | None = None
    measurement_error_fraction: float = 0.0

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoResponseAssessArgs":
        raw = _route_mapping("oncology response arguments", value)
        return cls(
            raw.get("criterion"),
            raw.get("baseline"),
            raw.get("current"),
            raw.get("current_acquired"),
            raw.get("baseline_clinical"),
            raw.get("current_clinical"),
            raw.get("treatment"),
            raw.get("evidence"),
            raw.get("nadir_spd_mm2"),
            raw.get("measurement_error_fraction", 0.0),
        )

    def __post_init__(self) -> None:
        for name in ("criterion", "baseline", "current", "baseline_clinical", "current_clinical", "treatment"):
            object.__setattr__(self, name, _route_mapping(f"oncology response {name}", getattr(self, name)))
        object.__setattr__(self, "current_acquired", _route_text("oncology current_acquired", self.current_acquired))
        object.__setattr__(self, "evidence", _optional_mapping("oncology progression evidence", self.evidence))
        if self.nadir_spd_mm2 is not None:
            object.__setattr__(self, "nadir_spd_mm2", _finite_nonnegative("oncology nadir_spd_mm2", self.nadir_spd_mm2))
        object.__setattr__(self, "measurement_error_fraction", _finite_nonnegative("oncology measurement_error_fraction", self.measurement_error_fraction))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "criterion": dict(self.criterion),
            "baseline": dict(self.baseline),
            "current": dict(self.current),
            "current_acquired": self.current_acquired,
            "baseline_clinical": dict(self.baseline_clinical),
            "current_clinical": dict(self.current_clinical),
            "treatment": dict(self.treatment),
            "measurement_error_fraction": self.measurement_error_fraction,
        }
        if self.evidence is not None:
            result["evidence"] = dict(self.evidence)
        if self.nadir_spd_mm2 is not None:
            result["nadir_spd_mm2"] = self.nadir_spd_mm2
        return result


ONCO_RESPONSE_SCHEMA = "bioprism-mcp/onco-response-assess/0.1"
ONCO_RESPONSE_CALL_KINDS = frozenset({"complete", "partial", "stable", "progression", "not_evaluable"})
ONCO_RESPONSE_OUTCOME_KINDS = frozenset({"assessment", "refused"})
ONCO_RESPONSE_REFUSAL_KINDS = frozenset({"assessment_error"})
ONCO_RESPONSE_CALL_LABELS = {
    "complete": "complete response",
    "partial": "partial response",
    "stable": "stable disease",
    "progression": "confirmed progression",
    "not_evaluable": "not evaluable",
}


@dataclass(frozen=True)
class OncoResponseAssessmentProjection:
    raw: dict[str, Any]
    criterion_id: str
    criterion_version: str
    unconfirmed_reading: str
    call_kind: str
    call: dict[str, Any]
    sensitivity: dict[str, Any]
    hypotheses: dict[str, Any]
    divergence_from_criterion: dict[str, Any] | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoResponseAssessmentProjection":
        raw = _route_mapping("oncology response assessment", value)
        call = _route_mapping("oncology response call", raw.get("call"))
        call_kind = _route_text("oncology response call kind", call.get("call"))
        if call_kind not in ONCO_RESPONSE_CALL_KINDS:
            raise ArgumentError(f"unknown oncology response call kind: {call_kind!r}")
        hypotheses = _route_mapping("oncology response hypotheses", raw.get("hypotheses"))
        return cls(
            raw,
            _route_text("oncology response criterion id", raw.get("criterion_id")),
            _route_text("oncology response criterion version", raw.get("criterion_version")),
            _route_text("oncology response unconfirmed reading", raw.get("unconfirmed_reading")),
            call_kind,
            call,
            _route_mapping("oncology response sensitivity", raw.get("sensitivity")),
            hypotheses,
            _optional_mapping("oncology response criterion divergence", raw.get("divergence_from_criterion")),
        )


@dataclass(frozen=True)
class OncoResponseReport:
    raw: dict[str, Any]
    ok: bool
    assessment: dict[str, Any] | None
    call_label: str | None
    withheld_progression: bool | None
    hypothesis_count: int | None
    evidence_requests: tuple[Any, ...]
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    refusal_kind: str | None = None
    call_kind: str | None = None
    unconfirmed_reading: str | None = None
    criterion: dict[str, Any] | None = None
    treatment: dict[str, Any] | None = None
    criterion_recognises_post_treatment_change: bool | None = None
    post_treatment_window_days: int | None = None
    pseudoresponse_possible: bool | None = None
    measurement_error_fraction: float | None = None
    evidence_present: bool | None = None
    criterion_divergence_present: bool | None = None
    sensitivity_flips: bool | None = None
    hypothesis_non_identifiable: bool | None = None
    assessment_record: OncoResponseAssessmentProjection | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoResponseReport":
        raw = _projection_payload(value, description="oncology response", direct_keys=("assessment", "stage"))
        ok = _bool("oncology response ok", raw.get("ok"))
        fail_closed = _bool("oncology response fail_closed", raw.get("fail_closed", False))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology response schema", schema_value)
        if schema is not None and schema != ONCO_RESPONSE_SCHEMA:
            raise ArgumentError(f"unknown oncology response schema: {schema!r}")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "assessment" if ok else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("oncology response outcome kind", outcome_kind_value)
            if outcome_kind not in ONCO_RESPONSE_OUTCOME_KINDS or outcome_kind != ("assessment" if ok else "refused"):
                raise ArgumentError("oncology response outcome kind does not reconcile with transport state")
        criterion = None if raw.get("criterion") is None else _route_mapping("oncology response criterion", raw.get("criterion"))
        treatment = None if raw.get("treatment") is None else _route_mapping("oncology response treatment", raw.get("treatment"))
        criterion_recognises = None if raw.get("criterion_recognises_post_treatment_change") is None else _bool("oncology response criterion post-treatment flag", raw.get("criterion_recognises_post_treatment_change"))
        post_treatment_window_days = None if raw.get("post_treatment_window_days") is None else _signed_integer("oncology response post-treatment window", raw.get("post_treatment_window_days"))
        pseudoresponse_possible = None if raw.get("pseudoresponse_possible") is None else _bool("oncology response pseudoresponse flag", raw.get("pseudoresponse_possible"))
        measurement_error_fraction = None if raw.get("measurement_error_fraction") is None else _finite_nonnegative("oncology response measurement error", raw.get("measurement_error_fraction"))
        evidence_present = None if raw.get("evidence_present") is None else _bool("oncology response evidence presence", raw.get("evidence_present"))
        if not ok:
            refusal = _route_text("oncology response refusal", raw.get("refusal"))
            if not fail_closed:
                raise ArgumentError("refused oncology response results must be fail-closed")
            refusal_kind_value = raw.get("refusal_kind")
            refusal_kind = None if refusal_kind_value is None else _route_text("oncology response refusal kind", refusal_kind_value)
            if refusal_kind is not None and refusal_kind not in ONCO_RESPONSE_REFUSAL_KINDS:
                raise ArgumentError(f"unknown oncology response refusal kind: {refusal_kind!r}")
            if schema is not None and refusal_kind is None:
                raise ArgumentError("versioned oncology response refusals require refusal_kind")
            return cls(
                raw,
                False,
                None,
                None,
                None,
                None,
                (),
                _route_text("oncology response stage", raw.get("stage")),
                refusal,
                True,
                None if raw.get("guarantee") is None else _route_text("oncology response guarantee", raw.get("guarantee")),
                (),
                (),
                schema,
                outcome_kind,
                refusal_kind,
                None,
                None,
                criterion,
                treatment,
                criterion_recognises,
                post_treatment_window_days,
                pseudoresponse_possible,
                measurement_error_fraction,
                evidence_present,
                None,
                None,
                None,
                None,
            )
        if fail_closed or raw.get("refusal") is not None or raw.get("stage") is not None:
            raise ArgumentError("successful oncology response results cannot carry refusal evidence")
        assessment = _route_mapping("oncology response assessment", raw.get("assessment"))
        assessment_record = None
        if all(key in assessment for key in ("criterion_id", "criterion_version", "sensitivity", "hypotheses")):
            assessment_record = OncoResponseAssessmentProjection.from_wire(assessment)
        elif schema is not None:
            raise ArgumentError("versioned oncology responses require a complete assessment projection")
        call_label = _route_text("oncology response call_label", raw.get("call_label"))
        call_projection = _route_mapping("oncology response call", assessment.get("call"))
        call_kind_value = raw.get("call_kind")
        call_kind = _route_text("oncology response call kind", call_projection.get("call")) if call_kind_value is None else _route_text("oncology response call kind", call_kind_value)
        if call_kind not in ONCO_RESPONSE_CALL_KINDS or (assessment_record is not None and call_kind != assessment_record.call_kind) or ONCO_RESPONSE_CALL_LABELS[call_kind] != call_label:
            raise ArgumentError("oncology response call kind or label does not reconcile with assessment")
        withheld = _bool("oncology response withheld_progression", raw.get("withheld_progression"))
        if withheld and call_label != "not evaluable":
            raise ArgumentError("withheld progression must have a not evaluable reportable call")
        evidence_requests = _array("oncology response evidence_requests", raw.get("evidence_requests"))
        hypothesis_count = _route_count("oncology response hypothesis_count", raw.get("hypothesis_count"))
        entries = _array("oncology response hypothesis entries", assessment_record.hypotheses.get("entries", [])) if assessment_record is not None else ()
        if assessment_record is not None and hypothesis_count != len(entries):
            raise ArgumentError("oncology response hypothesis count does not reconcile with assessment")
        criterion_divergence_present = _bool("oncology response criterion divergence presence", raw.get("criterion_divergence_present", assessment_record is not None and assessment_record.divergence_from_criterion is not None))
        if assessment_record is not None and criterion_divergence_present != (assessment_record.divergence_from_criterion is not None):
            raise ArgumentError("oncology response criterion divergence does not reconcile with assessment")
        sensitivity_flips = _bool("oncology response sensitivity flips", raw.get("sensitivity_flips", assessment_record is not None and assessment_record.sensitivity.get("flips_within_measurement_error", False)))
        if assessment_record is not None and "flips_within_measurement_error" in assessment_record.sensitivity and sensitivity_flips != _bool("oncology response assessment sensitivity flips", assessment_record.sensitivity.get("flips_within_measurement_error")):
            raise ArgumentError("oncology response sensitivity flip state does not reconcile")
        hypothesis_non_identifiable = _bool("oncology response hypothesis identifiability", raw.get("hypothesis_non_identifiable", len(entries) > 1 if assessment_record is not None else False))
        if assessment_record is not None and hypothesis_non_identifiable != (len(entries) > 1):
            raise ArgumentError("oncology response hypothesis identifiability does not reconcile")
        if schema is not None and ("call_kind" not in raw or criterion is None or treatment is None):
            raise ArgumentError("versioned oncology responses require call, criterion, and treatment projections")
        return cls(
            raw,
            True,
            assessment,
            call_label,
            withheld,
            hypothesis_count,
            evidence_requests,
            None,
            None,
            False,
            None,
            _route_strings("oncology response guarantees", raw.get("guarantees")),
            _route_strings("oncology response limitations", raw.get("limitations")),
            schema,
            outcome_kind,
            None,
            call_kind,
            assessment_record.unconfirmed_reading if assessment_record is not None else (None if assessment.get("unconfirmed_reading") is None else _route_text("oncology response unconfirmed reading", assessment.get("unconfirmed_reading"))),
            criterion,
            treatment,
            criterion_recognises,
            post_treatment_window_days,
            pseudoresponse_possible,
            measurement_error_fraction,
            evidence_present,
            criterion_divergence_present,
            sensitivity_flips,
            hypothesis_non_identifiable,
            assessment_record,
        )


@dataclass(frozen=True)
class OncoWorldlineViewArgs:
    """Serialized worldline plus an optional agent-visibility cutoff."""

    worldline: Mapping[str, Any]
    visible_at: str | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldlineViewArgs":
        raw = _route_mapping("oncology worldline arguments", value)
        return cls(raw.get("worldline"), raw.get("visible_at"))

    def __post_init__(self) -> None:
        object.__setattr__(self, "worldline", _route_mapping("oncology worldline", self.worldline))
        if self.visible_at is not None:
            object.__setattr__(self, "visible_at", _route_text("oncology visible_at", self.visible_at))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"worldline": dict(self.worldline)}
        if self.visible_at is not None:
            result["visible_at"] = self.visible_at
        return result


@dataclass(frozen=True)
class OncoClockProjection:
    """The four distinct clocks carried by one tumour-worldline timepoint."""

    raw: dict[str, Any]
    acquired: str
    recorded: str
    released: str
    visible: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClockProjection":
        raw = _route_mapping("oncology timepoint clocks", value)
        return cls(
            raw,
            _route_text("oncology acquired clock", raw.get("acquired")),
            _route_text("oncology recorded clock", raw.get("recorded")),
            _route_text("oncology released clock", raw.get("released")),
            _route_text("oncology visible clock", raw.get("visible")),
        )

    @property
    def axes(self) -> tuple[str, ...]:
        """Clock names in dependency order, never as a single collapsed timestamp."""

        return ONCO_WORLDLINE_CLOCK_AXES

    @property
    def values(self) -> tuple[str, ...]:
        return self.acquired, self.recorded, self.released, self.visible


def _signed_integer(name: str, value: Any) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ArgumentError(f"{name} must be an integer")
    return value


@dataclass(frozen=True)
class OncoTimepointProjection:
    """One worldline row with explicit order, clock, and visibility evidence."""

    raw: dict[str, Any]
    label: str
    biological_index: int
    record_index: int
    days_from_baseline: int
    clocks: OncoClockProjection
    observation: dict[str, Any]
    visibility_state: str
    visible_at_cutoff: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoTimepointProjection":
        raw = _route_mapping("oncology timepoint", value)
        clocks_value = raw.get("clocks")
        clocks = OncoClockProjection.from_wire(clocks_value)
        for axis in ONCO_WORLDLINE_CLOCK_AXES:
            if axis in raw and raw[axis] != clocks.raw[axis]:
                raise ArgumentError(f"oncology timepoint {axis} clock disagrees with nested clocks")
        visible_at_cutoff_value = raw.get("visible_at_cutoff")
        if visible_at_cutoff_value is not None and not isinstance(visible_at_cutoff_value, bool):
            raise ArgumentError("oncology timepoint visible_at_cutoff must be a boolean or null")
        visibility_state = _route_text("oncology timepoint visibility_state", raw.get("visibility_state"))
        if visibility_state not in ONCO_WORLDLINE_VISIBILITY_STATES:
            raise ArgumentError(f"unknown oncology timepoint visibility state: {visibility_state!r}")
        if visibility_state == "not_filtered" and visible_at_cutoff_value is not None:
            raise ArgumentError("unfiltered oncology timepoints cannot carry cutoff visibility")
        if visibility_state == "visible" and visible_at_cutoff_value is not True:
            raise ArgumentError("visible oncology timepoints must be visible at the cutoff")
        if visibility_state == "hidden_from_agent" and visible_at_cutoff_value is not False:
            raise ArgumentError("hidden oncology timepoints must be absent at the cutoff")
        return cls(
            raw,
            _route_text("oncology timepoint label", raw.get("label")),
            _route_count("oncology biological index", raw.get("biological_index")),
            _route_count("oncology record index", raw.get("record_index")),
            _signed_integer("oncology days_from_baseline", raw.get("days_from_baseline")),
            clocks,
            _route_mapping("oncology timepoint observation", raw.get("observation")),
            visibility_state,
            visible_at_cutoff_value,
        )


@dataclass(frozen=True)
class OncoVisibilityPartitionProjection:
    """The visibility firewall partition, including null evidence when no cutoff was requested."""

    raw: dict[str, Any]
    cutoff: str | None
    filter_applied: bool
    visible: tuple[str, ...] | None
    hidden: tuple[str, ...] | None
    visible_count: int | None
    hidden_count: int | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoVisibilityPartitionProjection":
        raw = _route_mapping("oncology visibility partition", value)
        filter_applied = _bool("oncology visibility partition filter_applied", raw.get("filter_applied"))
        cutoff = None if raw.get("cutoff") is None else _route_text("oncology visibility partition cutoff", raw.get("cutoff"))
        visible_value = raw.get("visible")
        hidden_value = raw.get("hidden")
        if not filter_applied and (cutoff is not None or visible_value is not None or hidden_value is not None):
            raise ArgumentError("unfiltered oncology visibility partitions must be empty")
        visible = None if visible_value is None else _route_strings("oncology partition visible", visible_value)
        hidden = None if hidden_value is None else _route_strings("oncology partition hidden", hidden_value)
        if filter_applied and (cutoff is None or visible is None or hidden is None):
            raise ArgumentError("filtered oncology visibility partitions require cutoff and both sides")
        visible_count_value = raw.get("visible_count")
        hidden_count_value = raw.get("hidden_count")
        visible_count = None if visible_count_value is None else _route_count("oncology partition visible_count", visible_count_value)
        hidden_count = None if hidden_count_value is None else _route_count("oncology partition hidden_count", hidden_count_value)
        if filter_applied and (visible_count != len(visible) or hidden_count != len(hidden)):
            raise ArgumentError("oncology visibility partition counts do not reconcile")
        if not filter_applied and (visible_count is not None or hidden_count is not None):
            raise ArgumentError("unfiltered oncology visibility partitions cannot carry counts")
        return cls(raw, cutoff, filter_applied, visible, hidden, visible_count, hidden_count)


@dataclass(frozen=True)
class OncoWorldlineReport:
    raw: dict[str, Any]
    ok: bool
    subject: str
    baseline: str
    timepoint_count: int
    biological_order: tuple[str, ...]
    record_order: tuple[str, ...]
    record_order_differs: bool
    visibility_cutoff: str | None
    visibility_filter_applied: bool
    visible_timepoints: tuple[str, ...] | None
    hidden_from_agent: tuple[str, ...] | None
    timepoints: tuple[dict[str, Any], ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    clock_axes: tuple[str, ...] = ONCO_WORLDLINE_CLOCK_AXES
    clock_order_guaranteed: bool = False
    baseline_biological_index: int | None = None
    baseline_record_index: int | None = None
    visibility_partition: OncoVisibilityPartitionProjection | None = None
    timepoint_records: tuple[OncoTimepointProjection, ...] = ()
    visible_count: int | None = None
    hidden_count: int | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoWorldlineReport":
        raw = _projection_payload(value, description="oncology worldline", direct_keys=("timepoints", "worldline"))
        if not _bool("oncology worldline ok", raw.get("ok")):
            raise ArgumentError("oncology worldline view is not successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology worldline schema", schema_value)
        if schema is not None and schema != ONCO_WORLDLINE_SCHEMA:
            raise ArgumentError(f"unknown oncology worldline schema: {schema!r}")
        biological = _route_strings("oncology biological_order", raw.get("biological_order"))
        record = _route_strings("oncology record_order", raw.get("record_order"))
        count = _route_count("oncology timepoint_count", raw.get("timepoint_count"))
        rows = tuple(_route_mapping("oncology timepoint", item) for item in _array("oncology timepoints", raw.get("timepoints")))
        if len(biological) != count or len(record) != count or len(rows) != count:
            raise ArgumentError("oncology worldline timepoint counts do not reconcile")
        records = tuple(OncoTimepointProjection.from_wire(row) for row in rows)
        row_labels = tuple(record.label for record in records)
        if set(row_labels) != set(biological) or set(row_labels) != set(record):
            raise ArgumentError("oncology worldline order projections do not reconcile with timepoint rows")
        if tuple(item.label for item in sorted(records, key=lambda item: item.biological_index)) != biological:
            raise ArgumentError("oncology biological indices do not reconcile with biological order")
        if tuple(item.label for item in sorted(records, key=lambda item: item.record_index)) != record:
            raise ArgumentError("oncology record indices do not reconcile with record order")
        if {item.biological_index for item in records} != set(range(count)) or {item.record_index for item in records} != set(range(count)):
            raise ArgumentError("oncology timepoint indices must be complete permutations")
        if _route_text("oncology baseline", raw.get("baseline")) not in set(biological):
            raise ArgumentError("oncology worldline baseline is not present in biological order")
        differs = _bool("oncology record_order_differs", raw.get("record_order_differs"))
        if differs != (biological != record):
            raise ArgumentError("oncology record_order_differs does not reconcile with the order projections")
        clock_axes_value = raw.get("clock_axes", ONCO_WORLDLINE_CLOCK_AXES)
        clock_axes = _route_strings("oncology clock_axes", clock_axes_value)
        if clock_axes != ONCO_WORLDLINE_CLOCK_AXES:
            raise ArgumentError("oncology clock axes must remain in acquisition, recording, release, visibility order")
        clock_order_guaranteed = _bool("oncology clock_order_guaranteed", raw.get("clock_order_guaranteed", False))
        if not clock_order_guaranteed:
            raise ArgumentError("oncology worldline must guarantee four-clock dependency order")
        baseline = _route_text("oncology baseline", raw.get("baseline"))
        baseline_biological_index = _route_count("oncology baseline_biological_index", raw.get("baseline_biological_index", biological.index(baseline)))
        baseline_record_index = _route_count("oncology baseline_record_index", raw.get("baseline_record_index", record.index(baseline)))
        if baseline_biological_index != biological.index(baseline) or baseline_record_index != record.index(baseline):
            raise ArgumentError("oncology baseline indices do not reconcile with order projections")
        filtered = _bool("oncology visibility_filter_applied", raw.get("visibility_filter_applied"))
        cutoff = None if raw.get("visibility_cutoff") is None else _route_text("oncology visibility_cutoff", raw.get("visibility_cutoff"))
        visible_value = raw.get("visible_timepoints")
        hidden_value = raw.get("hidden_from_agent")
        if filtered != (cutoff is not None):
            raise ArgumentError("oncology visibility filter and cutoff do not reconcile")
        if filtered:
            visible = _route_strings("oncology visible_timepoints", visible_value)
            hidden = _route_strings("oncology hidden_from_agent", hidden_value)
            if set(visible).intersection(hidden) or set(visible).union(hidden) != set(row_labels):
                raise ArgumentError("oncology visibility partitions do not cover disjoint timepoint rows")
            if {item.label for item in records if item.visible_at_cutoff} != set(visible) or {item.label for item in records if item.visible_at_cutoff is False} != set(hidden):
                raise ArgumentError("oncology row visibility does not reconcile with the visibility partition")
        elif visible_value is not None or hidden_value is not None:
            raise ArgumentError("unfiltered oncology worldline results cannot carry visibility partitions")
        else:
            visible = hidden = None
        partition_value = raw.get("visibility_partition")
        partition = None if partition_value is None else OncoVisibilityPartitionProjection.from_wire(partition_value)
        if partition is not None:
            if partition.filter_applied != filtered or partition.cutoff != cutoff or partition.visible != visible or partition.hidden != hidden:
                raise ArgumentError("oncology visibility partition does not reconcile with flat visibility fields")
        visible_count_value = raw.get("visible_count")
        hidden_count_value = raw.get("hidden_count")
        visible_count = None if visible_count_value is None else _route_count("oncology visible_count", visible_count_value)
        hidden_count = None if hidden_count_value is None else _route_count("oncology hidden_count", hidden_count_value)
        if filtered and (visible_count != len(visible) or hidden_count != len(hidden)):
            raise ArgumentError("oncology worldline visibility counts do not reconcile")
        if not filtered and (visible_count is not None or hidden_count is not None):
            raise ArgumentError("unfiltered oncology worldline cannot carry visibility counts")
        return cls(
            raw,
            True,
            _route_text("oncology subject", raw.get("subject")),
            _route_text("oncology baseline", raw.get("baseline")),
            count,
            biological,
            record,
            differs,
            cutoff,
            filtered,
            visible,
            hidden,
            rows,
            _route_strings("oncology worldline guarantees", raw.get("guarantees")),
            _route_strings("oncology worldline limitations", raw.get("limitations")),
            schema=schema,
            clock_axes=clock_axes,
            clock_order_guaranteed=clock_order_guaranteed,
            baseline_biological_index=baseline_biological_index,
            baseline_record_index=baseline_record_index,
            visibility_partition=partition,
            timepoint_records=records,
            visible_count=visible_count,
            hidden_count=hidden_count,
        )


@dataclass(frozen=True)
class OncoClassificationArgs:
    histology: str
    panel: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationArgs":
        raw = _route_mapping("oncology classification arguments", value)
        return cls(raw.get("histology"), raw.get("panel"))

    def __post_init__(self) -> None:
        object.__setattr__(self, "histology", _route_text("oncology histology", self.histology))
        object.__setattr__(self, "panel", _route_mapping("oncology marker panel", self.panel))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"histology": self.histology, "panel": dict(self.panel)}


ONCO_CLASSIFICATION_SCHEMA = "bioprism-mcp/onco-classification-check/0.1"
ONCO_CLASSIFICATION_RESOLUTION_KINDS = frozenset({"integrated", "provisional", "unresolved", "mixed", "not_otherwise_resolved"})
ONCO_CLASSIFICATION_MARKERS = frozenset({"idh_mutation", "codeletion1p19q", "tert_promoter_mutation", "egfr_amplification", "chromosome7_gain10_loss", "cdkn2a_cdkn2b_homozygous_deletion", "h3k27_alteration", "h3g34_mutation"})
ONCO_CLASSIFICATION_ROLES = frozenset({"required", "supportive", "exclusionary"})
ONCO_CLASSIFICATION_CALLS = frozenset({"present", "absent"})
ONCO_CLASSIFICATION_STATUSES = frozenset({"missing", "not_collected", "technically_failed", "below_detection", "not_applicable", "redacted"})


@dataclass(frozen=True)
class OncoMarkerObservationProjection:
    """An observed call or an explicit reason why no call exists."""

    raw: dict[str, Any]
    observed: bool
    call: str | None
    status: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoMarkerObservationProjection":
        raw = _route_mapping("oncology marker observation", value)
        has_value = "value" in raw
        has_status = "unobserved" in raw
        if has_value == has_status:
            raise ArgumentError("oncology marker observations must carry exactly one value or unobserved state")
        if has_value:
            call = _route_text("oncology marker call", raw.get("value"))
            if call not in ONCO_CLASSIFICATION_CALLS:
                raise ArgumentError(f"unknown oncology marker call: {call!r}")
            return cls(raw, True, call, None)
        status = _route_text("oncology marker observation status", raw.get("unobserved"))
        if status not in ONCO_CLASSIFICATION_STATUSES:
            raise ArgumentError(f"unknown oncology marker observation status: {status!r}")
        return cls(raw, False, None, status)


@dataclass(frozen=True)
class OncoClassificationPanelStateProjection:
    raw: dict[str, Any]
    marker: str
    state: OncoMarkerObservationProjection

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationPanelStateProjection":
        raw = _route_mapping("oncology panel state", value)
        marker = _route_text("oncology panel marker", raw.get("marker"))
        if marker not in ONCO_CLASSIFICATION_MARKERS:
            raise ArgumentError(f"unknown oncology panel marker: {marker!r}")
        return cls(raw, marker, OncoMarkerObservationProjection.from_wire(raw.get("state")))


@dataclass(frozen=True)
class OncoClassificationObligationProjection:
    raw: dict[str, Any]
    marker: str
    role: str
    state: OncoMarkerObservationProjection
    discriminates: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationObligationProjection":
        raw = _route_mapping("oncology classification obligation", value)
        marker = _route_text("oncology obligation marker", raw.get("marker"))
        role = _route_text("oncology obligation role", raw.get("role"))
        if marker not in ONCO_CLASSIFICATION_MARKERS or role not in ONCO_CLASSIFICATION_ROLES:
            raise ArgumentError("oncology obligation marker or role is outside the classification vocabulary")
        return cls(
            raw,
            marker,
            role,
            OncoMarkerObservationProjection.from_wire(raw.get("state")),
            _route_count("oncology obligation discriminates", raw.get("discriminates")),
        )


@dataclass(frozen=True)
class OncoClassificationSatisfiedEvidenceProjection:
    raw: dict[str, Any]
    marker: str
    role: str
    call: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationSatisfiedEvidenceProjection":
        raw = _route_mapping("oncology satisfied evidence", value)
        marker = _route_text("oncology satisfied evidence marker", raw.get("marker"))
        role = _route_text("oncology satisfied evidence role", raw.get("role"))
        call = _route_text("oncology satisfied evidence call", raw.get("call"))
        if marker not in ONCO_CLASSIFICATION_MARKERS or role not in ONCO_CLASSIFICATION_ROLES or call not in ONCO_CLASSIFICATION_CALLS:
            raise ArgumentError("oncology satisfied evidence is outside the classification vocabulary")
        return cls(raw, marker, role, call)


@dataclass(frozen=True)
class OncoClassificationResolutionProjection:
    """The mutually exclusive classification state and its evidence obligations."""

    raw: dict[str, Any]
    kind: str
    entity: str | None
    candidate: str | None
    candidates: tuple[str, ...]
    excluded: tuple[str, ...]
    histology: str | None
    grade: dict[str, Any] | None
    evidence: tuple[OncoClassificationSatisfiedEvidenceProjection, ...]
    obligations: tuple[OncoClassificationObligationProjection, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationResolutionProjection":
        raw = _route_mapping("oncology classification resolution", value)
        kind = _route_text("oncology classification resolution kind", raw.get("resolution"))
        if kind not in ONCO_CLASSIFICATION_RESOLUTION_KINDS:
            raise ArgumentError(f"unknown oncology classification resolution kind: {kind!r}")
        entity = None if raw.get("entity") is None else _route_text("oncology classification entity", raw.get("entity"))
        candidate = None if raw.get("candidate") is None else _route_text("oncology classification candidate", raw.get("candidate"))
        candidates = _route_strings("oncology classification candidates", raw.get("candidates", []))
        excluded = _route_strings("oncology classification excluded", raw.get("excluded", []))
        histology = None if raw.get("histology") is None else _route_text("oncology classification resolution histology", raw.get("histology"))
        grade_value = raw.get("grade")
        grade = None if grade_value is None else _route_mapping("oncology classification grade", grade_value)
        evidence = tuple(OncoClassificationSatisfiedEvidenceProjection.from_wire(item) for item in _array("oncology classification evidence", raw.get("evidence", [])))
        obligations = tuple(OncoClassificationObligationProjection.from_wire(item) for item in _array("oncology classification obligations", raw.get("obligations", [])))
        if kind == "integrated" and (entity is None or not evidence or candidate is not None or candidates or obligations):
            raise ArgumentError("integrated oncology classification must carry entity/evidence only")
        if kind == "provisional" and (candidate is None or not obligations or entity is not None or candidates):
            raise ArgumentError("provisional oncology classification must carry one candidate and obligations")
        if kind == "unresolved" and (not candidates or not obligations or entity is not None or candidate is not None):
            raise ArgumentError("unresolved oncology classification must carry candidates and obligations")
        if kind == "mixed" and (not candidates or entity is not None or candidate is not None or obligations):
            raise ArgumentError("mixed oncology classification must carry candidates without obligations")
        if kind == "not_otherwise_resolved" and (histology is None or not excluded or entity is not None or candidate is not None or candidates or obligations):
            raise ArgumentError("not-otherwise-resolved classification must carry histology and exclusions")
        return cls(raw, kind, entity, candidate, candidates, excluded, histology, grade, evidence, obligations)

    @property
    def is_integrated(self) -> bool:
        return self.kind == "integrated"


@dataclass(frozen=True)
class OncoClassificationReport:
    raw: dict[str, Any]
    ok: bool
    histology: str
    resolution: dict[str, Any]
    is_integrated: bool
    entity: str | None
    obligations: tuple[dict[str, Any], ...]
    panel_states: tuple[dict[str, Any], ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    resolution_kind: str | None = None
    resolution_record: OncoClassificationResolutionProjection | None = None
    obligation_records: tuple[OncoClassificationObligationProjection, ...] = ()
    panel_state_records: tuple[OncoClassificationPanelStateProjection, ...] = ()
    obligation_count: int | None = None
    panel_state_count: int | None = None
    observed_panel_state_count: int | None = None
    unobserved_panel_state_count: int | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoClassificationReport":
        raw = _projection_payload(value, description="oncology classification", direct_keys=("resolution", "panel_states"))
        if not _bool("oncology classification ok", raw.get("ok")):
            raise ArgumentError("oncology classification is not successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology classification schema", schema_value)
        if schema is not None and schema != ONCO_CLASSIFICATION_SCHEMA:
            raise ArgumentError(f"unknown oncology classification schema: {schema!r}")
        integrated = _bool("oncology classification is_integrated", raw.get("is_integrated"))
        entity_value = raw.get("entity")
        entity = None if entity_value is None else _route_text("oncology classification entity", entity_value)
        if integrated != (entity is not None):
            raise ArgumentError("integrated classification and entity do not reconcile")
        obligations = tuple(_route_mapping("oncology classification obligation", item) for item in _array("oncology classification obligations", raw.get("obligations")))
        states = tuple(_route_mapping("oncology panel state", item) for item in _array("oncology classification panel_states", raw.get("panel_states")))
        resolution_record = OncoClassificationResolutionProjection.from_wire(raw.get("resolution"))
        resolution_kind = _route_text("oncology classification resolution_kind", raw.get("resolution_kind"))
        if resolution_kind != resolution_record.kind or integrated != resolution_record.is_integrated or entity != resolution_record.entity:
            raise ArgumentError("oncology classification summary does not reconcile with its resolution record")
        obligation_records = tuple(OncoClassificationObligationProjection.from_wire(item) for item in obligations)
        if tuple(item.raw for item in obligation_records) != obligations or tuple(item.raw for item in resolution_record.obligations) != obligations:
            raise ArgumentError("oncology classification obligations do not reconcile with the resolution")
        panel_state_records = tuple(OncoClassificationPanelStateProjection.from_wire(item) for item in states)
        obligation_count = _route_count("oncology obligation_count", raw.get("obligation_count"))
        panel_state_count = _route_count("oncology panel_state_count", raw.get("panel_state_count"))
        observed_panel_state_count = _route_count("oncology observed_panel_state_count", raw.get("observed_panel_state_count"))
        unobserved_panel_state_count = _route_count("oncology unobserved_panel_state_count", raw.get("unobserved_panel_state_count"))
        if obligation_count != len(obligations) or panel_state_count != len(states) or observed_panel_state_count + unobserved_panel_state_count != panel_state_count or observed_panel_state_count != sum(item.state.observed for item in panel_state_records):
            raise ArgumentError("oncology classification counts do not reconcile")
        return cls(
            raw,
            True,
            _route_text("oncology classification histology", raw.get("histology")),
            _route_mapping("oncology classification resolution", raw.get("resolution")),
            integrated,
            entity,
            obligations,
            states,
            _route_strings("oncology classification guarantees", raw.get("guarantees")),
            _route_strings("oncology classification limitations", raw.get("limitations")),
            schema=schema,
            resolution_kind=resolution_kind,
            resolution_record=resolution_record,
            obligation_records=obligation_records,
            panel_state_records=panel_state_records,
            obligation_count=obligation_count,
            panel_state_count=panel_state_count,
            observed_panel_state_count=observed_panel_state_count,
            unobserved_panel_state_count=unobserved_panel_state_count,
        )

    @property
    def unresolved(self) -> bool:
        return not self.is_integrated


ONCO_ANALYSIS_UNITS = frozenset({"participant", "lesion", "specimen", "imaging_series"})
ONCO_IDENTITY_SCHEMA = "bioprism-mcp/oncoworlds-identity-join/0.1"
ONCO_IDENTITY_REFUSAL_KINDS = frozenset({"different_participant", "truncated_identifier", "different_lesion", "incompatible_epoch", "different_specimen", "no_identity_evidence", "unlicensed_relation", "undeclared_permissible_use", "no_regional_provenance", "incomparable_coordinates"})
ONCO_BIAS_FLAGS = frozenset({"left_truncation", "informative_loss_to_follow_up", "competing_death", "treatment_switching"})
ONCO_OUTCOME_SCHEMA = "bioprism-mcp/onco-outcome-analyze/0.1"
ONCO_OUTCOME_ENDPOINTS = frozenset({"overall_survival", "progression_free_survival", "time_to_progression", "time_to_treatment_failure"})
ONCO_OUTCOME_POPULATIONS = frozenset({"intention_to_treat", "per_protocol", "evaluable_for_response"})
ONCO_OUTCOME_EVENT_KINDS = frozenset({"death", "confirmed_progression", "progression_or_death", "treatment_failure"})
ONCO_OUTCOME_CENSORING_REASONS = frozenset({"administrative_cutoff", "lost_to_follow_up", "withdrew_consent", "event_free_at_last_contact", "competing_death", "subsequent_therapy"})


@dataclass(frozen=True)
class OncoIdentityJoinArgs:
    left: Mapping[str, Any]
    right: Mapping[str, Any]
    unit: str
    evidence: Mapping[str, Any] | None = None
    epoch_bridge: Mapping[str, Any] | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoIdentityJoinArgs":
        raw = _route_mapping("oncology identity arguments", value)
        return cls(raw.get("left"), raw.get("right"), raw.get("unit"), raw.get("evidence"), raw.get("epoch_bridge"))

    def __post_init__(self) -> None:
        object.__setattr__(self, "left", _route_mapping("oncology identity left", self.left))
        object.__setattr__(self, "right", _route_mapping("oncology identity right", self.right))
        unit = _route_text("oncology identity unit", self.unit)
        if unit not in ONCO_ANALYSIS_UNITS:
            raise ArgumentError(f"unknown oncology identity analysis unit: {unit!r}")
        object.__setattr__(self, "unit", unit)
        object.__setattr__(self, "evidence", _optional_mapping("oncology identity evidence", self.evidence))
        object.__setattr__(self, "epoch_bridge", _optional_mapping("oncology epoch bridge", self.epoch_bridge))

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"left": dict(self.left), "right": dict(self.right), "unit": self.unit}
        if self.evidence is not None:
            result["evidence"] = dict(self.evidence)
        if self.epoch_bridge is not None:
            result["epoch_bridge"] = dict(self.epoch_bridge)
        return result


@dataclass(frozen=True)
class OncoIdentityJoinDecisionProjection:
    raw: dict[str, Any]
    left: str
    right: str
    unit: str
    verdict: str
    refusal: dict[str, Any] | None
    refusal_kind: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoIdentityJoinDecisionProjection":
        raw = _route_mapping("oncology identity join report", value)
        left = _route_text("oncology identity report left", raw.get("left"))
        right = _route_text("oncology identity report right", raw.get("right"))
        unit = _route_text("oncology identity report unit", raw.get("unit"))
        if unit not in ONCO_ANALYSIS_UNITS:
            raise ArgumentError(f"unknown oncology identity report unit: {unit!r}")
        verdict = _route_mapping("oncology identity verdict", raw.get("verdict"))
        verdict_kind = _route_text("oncology identity verdict kind", verdict.get("verdict"))
        if verdict_kind == "joinable":
            return cls(raw, left, right, unit, verdict_kind, None, None)
        if verdict_kind != "declined":
            raise ArgumentError(f"unknown oncology identity verdict: {verdict_kind!r}")
        refusal = _route_mapping("oncology identity refusal", verdict.get("reason"))
        refusal_kind = _route_text("oncology identity refusal kind", refusal.get("refusal"))
        if refusal_kind not in ONCO_IDENTITY_REFUSAL_KINDS:
            raise ArgumentError(f"unknown oncology identity refusal: {refusal_kind!r}")
        return cls(raw, left, right, unit, verdict_kind, refusal, refusal_kind)


@dataclass(frozen=True)
class OncoIdentityJoinReport:
    raw: dict[str, Any]
    ok: bool
    joinable: bool
    report: dict[str, Any]
    bridge_declared: bool
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    verdict_kind: str | None = None
    decision_record: OncoIdentityJoinDecisionProjection | None = None
    refusal_kind: str | None = None
    identity_evidence_present: bool = False
    identity_link_count: int = 0
    epoch_bridge: dict[str, Any] | None = None
    bridge_warrant_present: bool = False
    checked_dimensions: tuple[str, ...] = ()

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoIdentityJoinReport":
        raw = _projection_payload(value, description="oncology identity join", direct_keys=("joinable", "report"))
        if not _bool("oncology identity ok", raw.get("ok")):
            raise ArgumentError("oncology identity join transport projection is not successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology identity schema", schema_value)
        if schema is not None and schema != ONCO_IDENTITY_SCHEMA:
            raise ArgumentError(f"unknown oncology identity schema: {schema!r}")
        joinable = _bool("oncology identity joinable", raw.get("joinable"))
        report = _route_mapping("oncology identity report", raw.get("report"))
        decision_record = OncoIdentityJoinDecisionProjection.from_wire(report)
        if joinable != (decision_record.verdict == "joinable"):
            raise ArgumentError("oncology identity joinable does not reconcile with the tagged verdict")
        bridge_declared = _bool("oncology identity bridge_declared", raw.get("bridge_declared"))
        bridge = _optional_mapping("oncology identity epoch_bridge", raw.get("epoch_bridge"))
        if bridge_declared != (bridge is not None):
            raise ArgumentError("oncology identity bridge declaration does not reconcile with bridge evidence")
        evidence_present = _bool("oncology identity_evidence_present", raw.get("identity_evidence_present"))
        link_count = _route_count("oncology identity_link_count", raw.get("identity_link_count"))
        if evidence_present != (link_count > 0):
            raise ArgumentError("oncology identity evidence presence does not reconcile with link count")
        bridge_warrant_present = _bool("oncology bridge_warrant_present", raw.get("bridge_warrant_present"))
        if bridge_warrant_present != (bridge is not None and isinstance(bridge.get("warrant"), str) and bool(bridge.get("warrant"))):
            raise ArgumentError("oncology bridge warrant presence does not reconcile with bridge evidence")
        checked_dimensions = _route_strings("oncology checked dimensions", raw.get("checked_dimensions"))
        return cls(
            raw,
            True,
            joinable,
            report,
            bridge_declared,
            _route_strings("oncology identity guarantees", raw.get("guarantees")),
            _route_strings("oncology identity limitations", raw.get("limitations")),
            schema=schema,
            verdict_kind=decision_record.verdict,
            decision_record=decision_record,
            refusal_kind=decision_record.refusal_kind,
            identity_evidence_present=evidence_present,
            identity_link_count=link_count,
            epoch_bridge=bridge,
            bridge_warrant_present=bridge_warrant_present,
            checked_dimensions=checked_dimensions,
        )

    @property
    def declined(self) -> bool:
        return not self.joinable


@dataclass(frozen=True)
class OncoOutcomeAnalyzeArgs:
    follow_up: Mapping[str, Any]
    estimand: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoOutcomeAnalyzeArgs":
        raw = _route_mapping("oncology outcome arguments", value)
        return cls(raw.get("follow_up"), raw.get("estimand"))

    def __post_init__(self) -> None:
        object.__setattr__(self, "follow_up", _route_mapping("oncology follow_up", self.follow_up))
        object.__setattr__(self, "estimand", _route_mapping("oncology estimand", self.estimand))

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"follow_up": dict(self.follow_up), "estimand": dict(self.estimand)}


@dataclass(frozen=True)
class OncoEstimandProjection:
    """The estimand carried into a one-subject outcome record."""

    raw: dict[str, Any]
    endpoint: str
    population: str
    variable: str
    summary_measure: str | dict[str, Any]
    intercurrent_event_strategies: tuple[tuple[str, str], ...]
    censoring_assumption: str | dict[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoEstimandProjection":
        raw = _route_mapping("oncology outcome estimand", value)
        endpoint = _route_text("oncology estimand endpoint", raw.get("endpoint"))
        population = _route_text("oncology estimand population", raw.get("population"))
        if endpoint not in ONCO_OUTCOME_ENDPOINTS:
            raise ArgumentError(f"unknown oncology estimand endpoint: {endpoint!r}")
        if population not in ONCO_OUTCOME_POPULATIONS:
            raise ArgumentError(f"unknown oncology estimand population: {population!r}")
        summary_value = raw.get("summary_measure")
        if isinstance(summary_value, str):
            summary_measure: str | dict[str, Any] = _route_text("oncology summary measure", summary_value)
        else:
            summary_measure = _route_mapping("oncology summary measure", summary_value)
        strategies: list[tuple[str, str]] = []
        seen_events: set[str] = set()
        for index, pair in enumerate(_array("oncology intercurrent event strategies", raw.get("intercurrent_event_strategies"))):
            values = _array(f"oncology intercurrent event strategy[{index}]", pair)
            if len(values) != 2:
                raise ArgumentError("oncology intercurrent event strategies must be event/strategy pairs")
            event = _route_text(f"oncology intercurrent event[{index}]", values[0])
            strategy = _route_text(f"oncology intercurrent strategy[{index}]", values[1])
            if event in seen_events:
                raise ArgumentError("oncology estimand cannot declare duplicate intercurrent events")
            seen_events.add(event)
            strategies.append((event, strategy))
        censoring_value = raw.get("censoring_assumption")
        if isinstance(censoring_value, str):
            censoring_assumption: str | dict[str, Any] = _route_text("oncology censoring assumption", censoring_value)
        else:
            censoring_assumption = _route_mapping("oncology censoring assumption", censoring_value)
        return cls(
            raw,
            endpoint,
            population,
            _route_text("oncology estimand variable", raw.get("variable")),
            summary_measure,
            tuple(strategies),
            censoring_assumption,
        )


@dataclass(frozen=True)
class OncoAnalysisOutcomeProjection:
    """Endpoint-specific event/censoring outcome, retaining the tagged wire form."""

    raw: dict[str, Any]
    outcome: str
    event_kind: str | None
    censoring_reason: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoAnalysisOutcomeProjection":
        raw = _route_mapping("oncology analysis outcome", value)
        outcome = _route_text("oncology analysis outcome tag", raw.get("outcome"))
        event_kind = censoring_reason = None
        if outcome == "event":
            event_kind = _route_text("oncology event kind", raw.get("kind"))
            if event_kind not in ONCO_OUTCOME_EVENT_KINDS:
                raise ArgumentError(f"unknown oncology event kind: {event_kind!r}")
        elif outcome == "censored":
            reason_value = raw.get("reason")
            tagged_reasons = [reason for reason in ONCO_OUTCOME_CENSORING_REASONS if reason in raw]
            if reason_value is not None:
                censoring_reason = _route_text("oncology analysis censoring reason", reason_value)
            elif len(tagged_reasons) == 1 and raw[tagged_reasons[0]] is None:
                censoring_reason = tagged_reasons[0]
            else:
                raise ArgumentError("censored oncology outcomes must carry exactly one tagged censoring reason")
            if censoring_reason not in ONCO_OUTCOME_CENSORING_REASONS:
                raise ArgumentError(f"unknown oncology analysis censoring reason: {censoring_reason!r}")
        else:
            raise ArgumentError(f"unknown oncology analysis outcome: {outcome!r}")
        return cls(raw, outcome, event_kind, censoring_reason)

    @property
    def is_event(self) -> bool:
        return self.outcome == "event"


@dataclass(frozen=True)
class OncoAnalysisRecordProjection:
    """The nested `AnalysedFollowUp` record that binds outcome numbers to an estimand."""

    raw: dict[str, Any]
    subject: str
    estimand: OncoEstimandProjection
    at_risk_days: int
    immortal_time_days: int
    outcome: OncoAnalysisOutcomeProjection
    bias_flags: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoAnalysisRecordProjection":
        raw = _route_mapping("oncology outcome analysis", value)
        flags = _route_strings("oncology analysis bias_flags", raw.get("bias_flags"))
        if any(flag not in ONCO_BIAS_FLAGS for flag in flags):
            raise ArgumentError("oncology analysis bias_flags contains an unknown bias")
        return cls(
            raw,
            _route_text("oncology analysis subject", raw.get("subject")),
            OncoEstimandProjection.from_wire(raw.get("estimand")),
            _route_count("oncology analysis at_risk_days", raw.get("at_risk_days")),
            _route_count("oncology analysis immortal_time_days", raw.get("immortal_time_days")),
            OncoAnalysisOutcomeProjection.from_wire(raw.get("outcome")),
            flags,
        )


@dataclass(frozen=True)
class OncoOutcomeReport:
    raw: dict[str, Any]
    ok: bool
    analysis: dict[str, Any]
    at_risk_days: int
    immortal_time_days: int
    event: bool
    censoring_reason: str | None
    informative_bias_flags: tuple[str, ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_record: OncoAnalysisOutcomeProjection | None = None
    analysis_record: OncoAnalysisRecordProjection | None = None
    bias_flags: tuple[str, ...] = ()
    bias_count: int | None = None
    informative_bias_count: int | None = None
    censoring_informative: bool | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoOutcomeReport":
        raw = _projection_payload(value, description="oncology outcome", direct_keys=("analysis", "censoring_reason"))
        if not _bool("oncology outcome ok", raw.get("ok")):
            raise ArgumentError("oncology outcome analysis is not successful")
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology outcome schema", schema_value)
        if schema is not None and schema != ONCO_OUTCOME_SCHEMA:
            raise ArgumentError(f"unknown oncology outcome schema: {schema!r}")
        event = _bool("oncology outcome event", raw.get("event"))
        reason = None if raw.get("censoring_reason") is None else _route_text("oncology censoring_reason", raw.get("censoring_reason"))
        if reason is not None and reason not in ONCO_OUTCOME_CENSORING_REASONS:
            raise ArgumentError(f"unknown oncology censoring_reason: {reason!r}")
        if event and reason is not None:
            raise ArgumentError("an oncology event cannot carry a censoring reason")
        if not event and reason is None:
            raise ArgumentError("a censored oncology outcome must carry a censoring reason")
        flags = _route_strings("oncology informative_bias_flags", raw.get("informative_bias_flags"))
        if any(flag not in ONCO_BIAS_FLAGS for flag in flags):
            raise ArgumentError("oncology informative_bias_flags contains an unknown bias")
        analysis_record = OncoAnalysisRecordProjection.from_wire(raw.get("analysis"))
        outcome_value = OncoAnalysisOutcomeProjection.from_wire(raw.get("outcome"))
        if outcome_value.raw != analysis_record.outcome.raw:
            raise ArgumentError("top-level oncology outcome does not reconcile with the analysis record")
        if outcome_value.is_event != event or outcome_value.censoring_reason != reason:
            raise ArgumentError("oncology event/censoring fields do not reconcile with the tagged outcome")
        at_risk_days = _route_count("oncology at_risk_days", raw.get("at_risk_days"))
        immortal_time_days = _route_count("oncology immortal_time_days", raw.get("immortal_time_days"))
        if (analysis_record.at_risk_days, analysis_record.immortal_time_days) != (at_risk_days, immortal_time_days):
            raise ArgumentError("oncology time-at-risk fields do not reconcile with the analysis record")
        all_flags = _route_strings("oncology bias_flags", raw.get("bias_flags"))
        if all_flags != analysis_record.bias_flags:
            raise ArgumentError("oncology bias_flags do not reconcile with the analysis record")
        informative_expected = tuple(flag for flag in all_flags if flag != "left_truncation")
        if flags != informative_expected:
            raise ArgumentError("oncology informative_bias_flags do not reconcile with all bias flags")
        bias_count = _route_count("oncology bias_count", raw.get("bias_count"))
        informative_bias_count = _route_count("oncology informative_bias_count", raw.get("informative_bias_count"))
        if bias_count != len(all_flags) or informative_bias_count != len(flags):
            raise ArgumentError("oncology bias counts do not reconcile")
        left_truncated = _bool("oncology left_truncated", raw.get("left_truncated"))
        if left_truncated != (immortal_time_days > 0):
            raise ArgumentError("oncology left_truncated does not reconcile with immortal time")
        censoring_informative_value = raw.get("censoring_informative")
        censoring_informative = None if censoring_informative_value is None else _bool("oncology censoring_informative", censoring_informative_value)
        if censoring_informative != (None if reason is None else reason in {"lost_to_follow_up", "withdrew_consent", "competing_death", "subsequent_therapy"}):
            raise ArgumentError("oncology censoring informativeness does not reconcile with the reason")
        return cls(
            raw,
            True,
            analysis_record.raw,
            at_risk_days,
            immortal_time_days,
            event,
            reason,
            flags,
            _route_strings("oncology outcome guarantees", raw.get("guarantees")),
            _route_strings("oncology outcome limitations", raw.get("limitations")),
            schema=schema,
            outcome_record=outcome_value,
            analysis_record=analysis_record,
            bias_flags=all_flags,
            bias_count=bias_count,
            informative_bias_count=informative_bias_count,
            censoring_informative=censoring_informative,
        )

    @property
    def left_truncated(self) -> bool:
        return self.immortal_time_days > 0


@dataclass(frozen=True)
class OncoBoundaryArgs:
    request: Mapping[str, Any]
    boundary: Mapping[str, Any] | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryArgs":
        raw = _route_mapping("oncology boundary arguments", value)
        request = _route_mapping("oncology boundary request", raw.get("request"))
        boundary_value = raw.get("boundary")
        boundary = None if boundary_value is None else _route_mapping("oncology boundary policy", boundary_value)
        return cls(request, boundary)

    def __post_init__(self) -> None:
        request = _route_mapping("oncology boundary request", self.request)
        uses = _array("oncology requested_uses", request.get("requested_uses", []))
        if len(uses) > 100:
            raise ArgumentError("oncology boundary request exceeds the 100-use safety bound")
        for index, use in enumerate(uses):
            name = _route_text(f"oncology requested_uses[{index}]", use)
            if name not in ONCO_OUTPUT_USES:
                raise ArgumentError(f"unknown oncology output use: {name!r}")
        object.__setattr__(self, "request", request)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"request": dict(self.request)}
        if self.boundary is not None:
            result["boundary"] = dict(self.boundary)
        return result


@dataclass(frozen=True)
class OncoEscalationReport:
    raw: dict[str, Any]
    trigger: str
    route: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoEscalationReport":
        raw = _route_mapping("oncology escalation", value)
        trigger = _route_text("oncology escalation trigger", raw.get("trigger"))
        route = _route_text("oncology escalation route", raw.get("route"))
        return cls(raw, trigger, route)


@dataclass(frozen=True)
class OncoBoundaryDispositionReport:
    raw: dict[str, Any]
    kind: str
    released: tuple[str, ...]
    refused: tuple[str, ...]
    escalation: OncoEscalationReport | None
    terminal_action: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryDispositionReport":
        raw = _route_mapping("oncology disposition", value)
        kind = _route_text("oncology disposition kind", raw.get("disposition"))
        if kind not in ONCO_DISPOSITIONS:
            raise ArgumentError(f"unknown oncology disposition: {kind!r}")
        uses = lambda name: tuple(
            _route_text(f"oncology disposition {name}[{index}]", item)
            for index, item in enumerate(_array(f"oncology disposition {name}", raw.get(name, [])))
        )
        if kind == "release_in_full":
            released = uses("uses")
            refused: tuple[str, ...] = ()
            escalation = None
            terminal_action = "abstain"
        elif kind == "release_partial":
            released = uses("released")
            refused = uses("refused")
            escalation_value = _route_mapping("oncology disposition escalation", raw.get("escalation"))
            escalation = OncoEscalationReport.from_wire(escalation_value)
            terminal_action = "escalate"
        else:
            released = ()
            refused = uses("refused")
            escalation_value = _route_mapping("oncology disposition escalation", raw.get("escalation"))
            escalation = OncoEscalationReport.from_wire(escalation_value)
            terminal_action = "stop"
        for name in released + refused:
            if name not in ONCO_OUTPUT_USES:
                raise ArgumentError(f"unknown oncology disposition use: {name!r}")
        return cls(raw, kind, released, refused, escalation, terminal_action)


@dataclass(frozen=True)
class OncoBoundaryReport:
    raw: dict[str, Any]
    ok: bool
    permitted: tuple[str, ...]
    disposition: OncoBoundaryDispositionReport | None
    released: tuple[str, ...]
    refused: tuple[str, ...]
    terminal_action: str | None
    escalation: OncoEscalationReport | None
    research_statement: str | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]
    schema: str | None = None
    outcome_kind: str | None = None
    disposition_kind: str | None = None
    requested_use_count: int | None = None
    released_count: int | None = None
    refused_count: int | None = None
    escalation_present: bool | None = None
    identifier_fields_present: bool | None = None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OncoBoundaryReport":
        raw = _payload(value)
        ok = _bool("oncology boundary ok", raw.get("ok"))
        fail_closed = _bool("oncology boundary fail_closed", raw.get("fail_closed", False))
        schema_value = raw.get("schema")
        schema = None if schema_value is None else _route_text("oncology boundary schema", schema_value)
        if schema is not None and schema != ONCO_BOUNDARY_SCHEMA:
            raise ArgumentError(f"unknown oncology boundary schema: {schema!r}")
        outcome_kind_value = raw.get("outcome_kind")
        outcome_kind = "disposition" if ok else "refused"
        if outcome_kind_value is not None:
            outcome_kind = _route_text("oncology boundary outcome kind", outcome_kind_value)
            if outcome_kind not in ONCO_BOUNDARY_OUTCOME_KINDS or outcome_kind != ("disposition" if ok else "refused"):
                raise ArgumentError("oncology boundary outcome kind does not reconcile with transport state")
        requested_use_count = None if raw.get("requested_use_count") is None else _route_count("oncology requested use count", raw.get("requested_use_count"))
        released_count = None if raw.get("released_count") is None else _route_count("oncology released count", raw.get("released_count"))
        refused_count = None if raw.get("refused_count") is None else _route_count("oncology refused count", raw.get("refused_count"))
        identifier_fields_present = raw.get("identifier_fields_present")
        if identifier_fields_present is not None:
            _bool("oncology identifier fields present", identifier_fields_present)
        stage = None if raw.get("stage") is None else _route_text("oncology boundary stage", raw.get("stage"))
        refusal = None if raw.get("refusal") is None else _route_text("oncology boundary refusal", raw.get("refusal"))
        guarantee = None if raw.get("guarantee") is None else _route_text("oncology boundary guarantee", raw.get("guarantee"))
        if not ok:
            if refusal is None or not fail_closed:
                raise ArgumentError("refused oncology boundary results require a fail-closed refusal")
            refusal_kind_value = raw.get("refusal_kind")
            refusal_kind = None if refusal_kind_value is None else _route_text("oncology boundary refusal kind", refusal_kind_value)
            if refusal_kind is not None and refusal_kind not in ONCO_BOUNDARY_REFUSAL_KINDS:
                raise ArgumentError(f"unknown oncology boundary refusal kind: {refusal_kind!r}")
            if schema is not None and refusal_kind is None:
                raise ArgumentError("versioned oncology boundary refusals require refusal_kind")
            if schema is not None and identifier_fields_present is not True:
                raise ArgumentError("versioned oncology boundary refusals must report identifier presence")
            return cls(raw, False, (), None, (), (), None, None, None, stage, refusal, True, guarantee, (), (), schema, outcome_kind, None, requested_use_count, released_count, refused_count, None, identifier_fields_present)
        if fail_closed or refusal is not None or stage is not None:
            raise ArgumentError("successful oncology boundary results cannot carry refusal evidence")
        permitted = _route_strings("oncology permitted uses", raw.get("permitted"))
        if any(use not in ONCO_OUTPUT_USES for use in permitted):
            raise ArgumentError("oncology permitted contains an unknown output use")
        disposition = OncoBoundaryDispositionReport.from_wire(raw.get("disposition"))
        released = _route_strings("oncology released uses", raw.get("released"))
        refused = _route_strings("oncology refused uses", raw.get("refused"))
        terminal_action = _route_text("oncology terminal action", raw.get("terminal_action"))
        if terminal_action not in ONCO_TERMINAL_ACTIONS or terminal_action != disposition.terminal_action:
            raise ArgumentError("oncology terminal action does not reconcile with disposition")
        if released != disposition.released or refused != disposition.refused:
            raise ArgumentError("oncology released/refused projections do not reconcile with disposition")
        escalation_value = raw.get("escalation")
        escalation = None if escalation_value is None else OncoEscalationReport.from_wire(escalation_value)
        if (escalation is None) != (disposition.escalation is None):
            raise ArgumentError("oncology escalation does not reconcile with disposition")
        disposition_kind_value = raw.get("disposition_kind")
        disposition_kind = disposition.kind if disposition_kind_value is None else _route_text("oncology disposition kind", disposition_kind_value)
        if disposition_kind != disposition.kind:
            raise ArgumentError("oncology disposition kind does not reconcile with disposition")
        calculated_requested = len(released) + len(refused)
        if requested_use_count is not None and requested_use_count != calculated_requested:
            raise ArgumentError("oncology requested use count does not reconcile with disposition")
        if released_count is not None and released_count != len(released):
            raise ArgumentError("oncology released count does not reconcile with disposition")
        if refused_count is not None and refused_count != len(refused):
            raise ArgumentError("oncology refused count does not reconcile with disposition")
        escalation_present = _bool("oncology escalation presence", raw.get("escalation_present", escalation is not None))
        if escalation_present != (escalation is not None):
            raise ArgumentError("oncology escalation presence does not reconcile with disposition")
        if schema is not None and ("disposition_kind" not in raw or "requested_use_count" not in raw or "released_count" not in raw or "refused_count" not in raw):
            raise ArgumentError("versioned oncology boundaries require disposition accounting")
        if schema is not None and identifier_fields_present is not False:
            raise ArgumentError("versioned oncology dispositions must report identifier absence")
        return cls(
            raw,
            True,
            permitted,
            disposition,
            released,
            refused,
            terminal_action,
            escalation,
            _route_text("oncology research statement", raw.get("research_statement")),
            None,
            None,
            False,
            None,
            _route_strings("oncology guarantees", raw.get("guarantees")),
            _route_strings("oncology limitations", raw.get("limitations")),
            schema,
            outcome_kind,
            disposition_kind,
            requested_use_count,
            released_count,
            refused_count,
            escalation_present,
            identifier_fields_present,
        )

    @property
    def refused_individual_use(self) -> bool:
        return any(use.startswith("individual_") or use in {"treatment_recommendation", "care_triage", "clinical_alerting"} for use in self.refused)

    @property
    def research_only(self) -> bool:
        return self.ok and not self.refused_individual_use


def onco_boundary_report(value: Mapping[str, Any]) -> OncoBoundaryReport:
    """Parse direct MCP or HTTP oncology-boundary output."""

    return OncoBoundaryReport.from_wire(value)


def onco_response_report(value: Mapping[str, Any]) -> OncoResponseReport:
    """Parse criteria-aware response assessment while preserving withheld progression."""

    return OncoResponseReport.from_wire(value)


def onco_worldline_report(value: Mapping[str, Any]) -> OncoWorldlineReport:
    """Parse biological, record, and agent-visibility worldline projections."""

    return OncoWorldlineReport.from_wire(value)


def onco_classification_report(value: Mapping[str, Any]) -> OncoClassificationReport:
    """Parse integrated or unresolved molecular classification projections."""

    return OncoClassificationReport.from_wire(value)


def onco_identity_join_report(value: Mapping[str, Any]) -> OncoIdentityJoinReport:
    """Parse an auditable identity join, including a typed declined verdict."""

    return OncoIdentityJoinReport.from_wire(value)


def onco_outcome_report(value: Mapping[str, Any]) -> OncoOutcomeReport:
    """Parse per-subject outcome, censoring, and delayed-entry analysis."""

    return OncoOutcomeReport.from_wire(value)


__all__ = [
    "ONCO_DISPOSITIONS",
    "ONCO_BOUNDARY_OUTCOME_KINDS",
    "ONCO_BOUNDARY_REFUSAL_KINDS",
    "ONCO_BOUNDARY_SCHEMA",
    "ONCO_ANALYSIS_UNITS",
    "ONCO_BIAS_FLAGS",
    "ONCO_IDENTITY_REFUSAL_KINDS",
    "ONCO_IDENTITY_SCHEMA",
    "ONCO_CLASSIFICATION_CALLS",
    "ONCO_CLASSIFICATION_MARKERS",
    "ONCO_CLASSIFICATION_RESOLUTION_KINDS",
    "ONCO_CLASSIFICATION_ROLES",
    "ONCO_CLASSIFICATION_SCHEMA",
    "ONCO_CLASSIFICATION_STATUSES",
    "ONCO_OUTPUT_USES",
    "ONCO_OUTCOME_CENSORING_REASONS",
    "ONCO_OUTCOME_ENDPOINTS",
    "ONCO_OUTCOME_EVENT_KINDS",
    "ONCO_OUTCOME_POPULATIONS",
    "ONCO_OUTCOME_SCHEMA",
    "ONCO_TERMINAL_ACTIONS",
    "ONCO_RESPONSE_CALL_KINDS",
    "ONCO_RESPONSE_OUTCOME_KINDS",
    "ONCO_RESPONSE_REFUSAL_KINDS",
    "ONCO_RESPONSE_SCHEMA",
    "ONCO_WORLDLINE_CLOCK_AXES",
    "ONCO_WORLDLINE_SCHEMA",
    "ONCO_WORLDLINE_VISIBILITY_STATES",
    "OncoBoundaryArgs",
    "OncoBoundaryDispositionReport",
    "OncoBoundaryReport",
    "OncoResponseAssessmentProjection",
    "OncoAnalysisOutcomeProjection",
    "OncoAnalysisRecordProjection",
    "OncoClassificationArgs",
    "OncoClassificationObligationProjection",
    "OncoClassificationPanelStateProjection",
    "OncoClassificationReport",
    "OncoClassificationResolutionProjection",
    "OncoClassificationSatisfiedEvidenceProjection",
    "OncoEstimandProjection",
    "OncoEscalationReport",
    "OncoIdentityJoinArgs",
    "OncoIdentityJoinDecisionProjection",
    "OncoIdentityJoinReport",
    "OncoMarkerObservationProjection",
    "OncoOutcomeAnalyzeArgs",
    "OncoOutcomeReport",
    "OncoResponseAssessArgs",
    "OncoResponseReport",
    "OncoClockProjection",
    "OncoTimepointProjection",
    "OncoVisibilityPartitionProjection",
    "OncoWorldlineReport",
    "OncoWorldlineViewArgs",
    "onco_boundary_report",
    "onco_classification_report",
    "onco_identity_join_report",
    "onco_outcome_report",
    "onco_response_report",
    "onco_worldline_report",
]
