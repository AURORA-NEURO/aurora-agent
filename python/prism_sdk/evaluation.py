"""Typed oracle and evaluation projections.

The Rust oracle/evaluation crates own the domain records and algorithms.  This module adds a
strict SDK boundary around their top-level evidence ledgers: underdetermination is not a failed
transport, omitted rows remain countable, future leakage is not a rate, reproducibility is not
biological validity, and a vacuous trajectory property is not a passing one.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError


ORACLE_STATUSES = frozenset({"valid", "invalid", "underdetermined"})
ORACLE_EVIDENCE_TIERS = frozenset({"deterministic", "execution", "property", "statistical", "judge"})


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _array(name: str, value: Any) -> tuple[Any, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(value)


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    return None if value is None else _route_mapping(name, value)


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _optional_finite(name: str, value: Any) -> float | None:
    return None if value is None else _finite(name, value)


def _projection_payload(
    value: Mapping[str, Any],
    *,
    description: str,
    direct_keys: tuple[str, ...],
) -> dict[str, Any]:
    """Extract a domain projection from direct, MCP, REST, or text-content envelopes."""

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
    raise ArgumentError(f"response does not contain an {description} projection")


def _refusal(raw: Mapping[str, Any], description: str) -> tuple[str, str, str | None]:
    fail_closed = _bool(f"{description} fail_closed", raw.get("fail_closed"))
    if not fail_closed:
        raise ArgumentError(f"refused {description} results must be fail-closed")
    return (
        _route_text(f"{description} stage", raw.get("stage")),
        _route_text(f"{description} refusal", raw.get("refusal")),
        None if raw.get("guarantee") is None else _route_text(f"{description} guarantee", raw.get("guarantee")),
    )


@dataclass(frozen=True)
class OracleCombineReport:
    raw: dict[str, Any]
    ok: bool
    subject: str
    at: str
    status: str
    underdetermined: bool
    deciding_tier: str | None
    judge_only: bool
    suppressed_override: bool
    acceptable: bool
    basis: Any
    confidence: dict[str, Any] | None
    establishes: tuple[str, ...]
    does_not_establish: tuple[str, ...]
    contributing: tuple[Any, ...]
    omitted_contributing: int
    withheld: tuple[Any, ...]
    omitted_withheld: int
    inadmissible: tuple[Any, ...]
    omitted_inadmissible: int
    suppressed: tuple[Any, ...]
    omitted_suppressed: int
    disagreements: tuple[Any, ...]
    omitted_disagreements: int
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OracleCombineReport":
        raw = _projection_payload(value, description="oracle combination", direct_keys=("status", "contributing"))
        if not _bool("oracle combination ok", raw.get("ok")):
            raise ArgumentError("oracle combination is not successful")
        status = _route_text("oracle combination status", raw.get("status"))
        if status not in ORACLE_STATUSES:
            raise ArgumentError(f"unknown oracle combination status: {status!r}")
        underdetermined = _bool("oracle combination underdetermined", raw.get("underdetermined"))
        if underdetermined != (status == "underdetermined"):
            raise ArgumentError("oracle underdetermination does not reconcile with status")
        deciding_tier = None if raw.get("deciding_tier") is None else _route_text("oracle deciding_tier", raw.get("deciding_tier"))
        if deciding_tier is not None and deciding_tier not in ORACLE_EVIDENCE_TIERS:
            raise ArgumentError(f"unknown oracle deciding tier: {deciding_tier!r}")

        def rows(name: str) -> tuple[tuple[Any, ...], int]:
            values = _array(f"oracle {name}", raw.get(name))
            omitted = _route_count(f"oracle omitted_{name}", raw.get(f"omitted_{name}"))
            return values, omitted

        contributing, omitted_contributing = rows("contributing")
        withheld, omitted_withheld = rows("withheld")
        inadmissible, omitted_inadmissible = rows("inadmissible")
        suppressed, omitted_suppressed = rows("suppressed")
        disagreements, omitted_disagreements = rows("disagreements")
        confidence = _optional_mapping("oracle confidence", raw.get("confidence"))
        if confidence is not None:
            _finite("oracle confidence.low", confidence.get("low"))
            _finite("oracle confidence.high", confidence.get("high"))
        return cls(
            raw,
            True,
            _route_text("oracle subject", raw.get("subject")),
            _route_text("oracle at", raw.get("at")),
            status,
            underdetermined,
            deciding_tier,
            _bool("oracle judge_only", raw.get("judge_only")),
            _bool("oracle suppressed_override", raw.get("suppressed_override")),
            _bool("oracle acceptable", raw.get("acceptable")),
            raw.get("basis"),
            confidence,
            _route_strings("oracle establishes", raw.get("establishes")),
            _route_strings("oracle does_not_establish", raw.get("does_not_establish")),
            contributing,
            omitted_contributing,
            withheld,
            omitted_withheld,
            inadmissible,
            omitted_inadmissible,
            suppressed,
            omitted_suppressed,
            disagreements,
            omitted_disagreements,
            _route_strings("oracle guarantees", raw.get("guarantees")),
            _route_strings("oracle limitations", raw.get("limitations")),
        )

    @property
    def release_blocked(self) -> bool:
        return self.status != "valid" or self.underdetermined or not self.acceptable


@dataclass(frozen=True)
class OracleReferencePanelReport:
    raw: dict[str, Any]
    ok: bool
    rule: Any | None
    rule_label: str | None
    consensus: dict[str, Any] | None
    tally: dict[str, Any] | None
    readers: int | None
    minority_calls: tuple[Any, ...]
    reads: tuple[Any, ...]
    omitted_reads: int
    per_reader: dict[str, Any] | None
    model_call: str | None
    adjudication: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OracleReferencePanelReport":
        raw = _projection_payload(value, description="oracle reference panel", direct_keys=("consensus", "stage"))
        ok = _bool("oracle reference panel ok", raw.get("ok"))
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "oracle reference panel")
            return cls(raw, False, None, None, None, None, None, (), (), 0, None, None, None, stage, refusal, True, guarantee, (), ())
        if raw.get("stage") is not None or raw.get("refusal") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful oracle reference panels cannot carry refusal evidence")
        return cls(
            raw,
            True,
            raw.get("rule"),
            _route_text("oracle rule_label", raw.get("rule_label")),
            _route_mapping("oracle consensus", raw.get("consensus")),
            _route_mapping("oracle tally", raw.get("tally")),
            _route_count("oracle readers", raw.get("readers")),
            _array("oracle minority_calls", raw.get("minority_calls")),
            _array("oracle reads", raw.get("reads")),
            _route_count("oracle omitted_reads", raw.get("omitted_reads")),
            _optional_mapping("oracle per_reader", raw.get("per_reader")),
            None if raw.get("model_call") is None else _route_text("oracle model_call", raw.get("model_call")),
            _optional_mapping("oracle adjudication", raw.get("adjudication")),
            None,
            None,
            False,
            None,
            _route_strings("oracle reference guarantees", raw.get("guarantees")),
            _route_strings("oracle reference limitations", raw.get("limitations")),
        )

    @property
    def unresolved(self) -> bool:
        return not self.ok or (self.consensus is not None and self.consensus.get("determination") == "unresolved")


@dataclass(frozen=True)
class OracleMissingnessReport:
    raw: dict[str, Any]
    ok: bool
    groups: tuple[Any, ...]
    informativeness: dict[str, Any]
    field: dict[str, Any]
    boundary: dict[str, Any]
    small_cell_floor: int
    egress: dict[str, Any]
    mechanism: dict[str, Any] | None
    complete_case: dict[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "OracleMissingnessReport":
        raw = _projection_payload(value, description="oracle missingness", direct_keys=("informativeness", "egress"))
        if not _bool("oracle missingness ok", raw.get("ok")):
            raise ArgumentError("oracle missingness is not successful")
        return cls(
            raw,
            True,
            _array("oracle missingness groups", raw.get("groups")),
            _route_mapping("oracle informativeness", raw.get("informativeness")),
            _route_mapping("oracle missingness field", raw.get("field")),
            _route_mapping("oracle missingness boundary", raw.get("boundary")),
            _route_count("oracle small_cell_floor", raw.get("small_cell_floor")),
            _route_mapping("oracle egress", raw.get("egress")),
            _optional_mapping("oracle missingness mechanism", raw.get("mechanism")),
            _optional_mapping("oracle complete_case", raw.get("complete_case")),
            _route_strings("oracle missingness guarantees", raw.get("guarantees")),
            _route_strings("oracle missingness limitations", raw.get("limitations")),
        )

    @property
    def complete_case_resolved(self) -> bool:
        return self.complete_case is not None


@dataclass(frozen=True)
class BioevalReferenceAuditReport:
    raw: dict[str, Any]
    ok: bool
    reference: dict[str, Any]
    reference_kind: str
    can_certify_clean_pass: bool
    resolution: dict[str, Any] | None
    modal_state: str | None
    modal_mass: float | None
    modal_confidence: float | None
    entropy_bits: float | None
    dispersion: str | None
    queried_state: str | None
    queried_state_mass: float | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioevalReferenceAuditReport":
        raw = _projection_payload(value, description="bioeval reference audit", direct_keys=("reference_kind", "reference"))
        if not _bool("bioeval reference ok", raw.get("ok")):
            raise ArgumentError("bioeval reference audit is not successful")
        return cls(
            raw,
            True,
            _route_mapping("bioeval reference", raw.get("reference")),
            _route_text("bioeval reference_kind", raw.get("reference_kind")),
            _bool("bioeval can_certify_clean_pass", raw.get("can_certify_clean_pass")),
            _optional_mapping("bioeval resolution", raw.get("resolution")),
            None if raw.get("modal_state") is None else _route_text("bioeval modal_state", raw.get("modal_state")),
            _optional_finite("bioeval modal_mass", raw.get("modal_mass")),
            _optional_finite("bioeval modal_confidence", raw.get("modal_confidence")),
            _optional_finite("bioeval entropy_bits", raw.get("entropy_bits")),
            None if raw.get("dispersion") is None else _route_text("bioeval dispersion", raw.get("dispersion")),
            None if raw.get("queried_state") is None else _route_text("bioeval queried_state", raw.get("queried_state")),
            _optional_finite("bioeval queried_state_mass", raw.get("queried_state_mass")),
            _route_strings("bioeval reference guarantees", raw.get("guarantees")),
            _route_strings("bioeval reference limitations", raw.get("limitations")),
        )


@dataclass(frozen=True)
class EvaluationWorldlineReport:
    raw: dict[str, Any]
    ok: bool
    decisions: int
    leak_count: int
    leaks: tuple[dict[str, Any], ...]
    dangling_count: int
    dangling_references: tuple[Any, ...]
    admissible_at: tuple[Any, ...] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvaluationWorldlineReport":
        raw = _projection_payload(value, description="evaluation worldline", direct_keys=("leak_count", "leaks"))
        if not _bool("evaluation worldline ok", raw.get("ok")):
            raise ArgumentError("evaluation worldline audit is not successful")
        leaks = tuple(_route_mapping("evaluation leakage", item) for item in _array("evaluation leaks", raw.get("leaks")))
        leak_count = _route_count("evaluation leak_count", raw.get("leak_count"))
        if len(leaks) != leak_count:
            raise ArgumentError("evaluation leak count does not reconcile")
        dangling = _array("evaluation dangling_references", raw.get("dangling_references"))
        dangling_count = _route_count("evaluation dangling_count", raw.get("dangling_count"))
        if len(dangling) != dangling_count:
            raise ArgumentError("evaluation dangling count does not reconcile")
        admissible = raw.get("admissible_at")
        admissible_at = None if admissible is None else _array("evaluation admissible_at", admissible)
        return cls(
            raw,
            True,
            _route_count("evaluation decisions", raw.get("decisions")),
            leak_count,
            leaks,
            dangling_count,
            dangling,
            admissible_at,
            _route_strings("evaluation worldline guarantees", raw.get("guarantees")),
            _route_strings("evaluation worldline limitations", raw.get("limitations")),
        )

    @property
    def leakage_detected(self) -> bool:
        return self.leak_count > 0

    @property
    def dangling_context_detected(self) -> bool:
        return self.dangling_count > 0


@dataclass(frozen=True)
class EvaluationReproductionReport:
    raw: dict[str, Any]
    ok: bool
    certificate: dict[str, Any] | None
    reproduced: bool | None
    first_divergence: dict[str, Any] | None
    missing_outputs: tuple[Any, ...]
    portability_demonstrated: bool | None
    validity_claim: dict[str, Any] | None
    stage: str | None
    refusal: str | None
    fail_closed: bool
    guarantee: str | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvaluationReproductionReport":
        raw = _projection_payload(value, description="evaluation reproduction", direct_keys=("certificate", "stage"))
        ok = _bool("evaluation reproduction ok", raw.get("ok"))
        if not ok:
            stage, refusal, guarantee = _refusal(raw, "evaluation reproduction")
            return cls(raw, False, None, None, None, (), None, None, stage, refusal, True, guarantee, (), ())
        if raw.get("stage") is not None or raw.get("refusal") is not None or raw.get("fail_closed", False):
            raise ArgumentError("successful evaluation reproduction cannot carry refusal evidence")
        return cls(
            raw,
            True,
            _route_mapping("evaluation certificate", raw.get("certificate")),
            _bool("evaluation reproduced", raw.get("reproduced")),
            _optional_mapping("evaluation first_divergence", raw.get("first_divergence")),
            _array("evaluation missing_outputs", raw.get("missing_outputs")),
            _bool("evaluation portability_demonstrated", raw.get("portability_demonstrated")),
            _optional_mapping("evaluation validity_claim", raw.get("validity_claim")),
            None,
            None,
            False,
            None,
            _route_strings("evaluation reproduction guarantees", raw.get("guarantees")),
            _route_strings("evaluation reproduction limitations", raw.get("limitations")),
        )

    @property
    def reproduced_and_portable(self) -> bool:
        return self.reproduced is True and self.portability_demonstrated is True


@dataclass(frozen=True)
class EvaluationTrajectoryReport:
    raw: dict[str, Any]
    ok: bool
    steps: int
    acts: tuple[Any, ...]
    properties: tuple[Any, ...]
    property_outcomes: tuple[dict[str, Any], ...]
    recovery: tuple[Any, ...]
    bounded_suffix: dict[str, Any] | None
    bounded_suffix_complete: bool | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvaluationTrajectoryReport":
        raw = _projection_payload(value, description="evaluation trajectory", direct_keys=("property_outcomes", "steps"))
        if not _bool("evaluation trajectory ok", raw.get("ok")):
            raise ArgumentError("evaluation trajectory is not successful")
        steps = _route_count("evaluation trajectory steps", raw.get("steps"))
        acts = _array("evaluation trajectory acts", raw.get("acts"))
        outcomes = tuple(_route_mapping("evaluation property outcome", item) for item in _array("evaluation property_outcomes", raw.get("property_outcomes")))
        if len(acts) != steps:
            raise ArgumentError("evaluation trajectory acts do not reconcile with steps")
        suffix = _optional_mapping("evaluation bounded_suffix", raw.get("bounded_suffix"))
        if suffix is None:
            suffix_complete = None
        elif suffix.get("ok") is False:
            if not _bool("evaluation bounded_suffix fail_closed", suffix.get("fail_closed")):
                raise ArgumentError("refused bounded suffix results must be fail-closed")
            _route_text("evaluation bounded suffix refusal", suffix.get("refusal"))
            suffix_complete = None
        else:
            suffix_complete = _bool("evaluation bounded_suffix complete", suffix.get("complete"))
        return cls(
            raw,
            True,
            steps,
            acts,
            _array("evaluation trajectory properties", raw.get("properties")),
            outcomes,
            _array("evaluation trajectory recovery", raw.get("recovery")),
            suffix,
            suffix_complete,
            _route_strings("evaluation trajectory guarantees", raw.get("guarantees")),
            _route_strings("evaluation trajectory limitations", raw.get("limitations")),
        )


def oracle_combine_report(value: Mapping[str, Any]) -> OracleCombineReport:
    return OracleCombineReport.from_wire(value)


def oracle_reference_panel_report(value: Mapping[str, Any]) -> OracleReferencePanelReport:
    return OracleReferencePanelReport.from_wire(value)


def oracle_missingness_report(value: Mapping[str, Any]) -> OracleMissingnessReport:
    return OracleMissingnessReport.from_wire(value)


def bioeval_reference_audit_report(value: Mapping[str, Any]) -> BioevalReferenceAuditReport:
    return BioevalReferenceAuditReport.from_wire(value)


def evaluation_worldline_audit_report(value: Mapping[str, Any]) -> EvaluationWorldlineReport:
    return EvaluationWorldlineReport.from_wire(value)


def evaluation_reproduction_check_report(value: Mapping[str, Any]) -> EvaluationReproductionReport:
    return EvaluationReproductionReport.from_wire(value)


def evaluation_trajectory_check_report(value: Mapping[str, Any]) -> EvaluationTrajectoryReport:
    return EvaluationTrajectoryReport.from_wire(value)


__all__ = [
    "ORACLE_EVIDENCE_TIERS",
    "ORACLE_STATUSES",
    "BioevalReferenceAuditReport",
    "EvaluationReproductionReport",
    "EvaluationTrajectoryReport",
    "EvaluationWorldlineReport",
    "OracleCombineReport",
    "OracleMissingnessReport",
    "OracleReferencePanelReport",
    "bioeval_reference_audit_report",
    "evaluation_reproduction_check_report",
    "evaluation_trajectory_check_report",
    "evaluation_worldline_audit_report",
    "oracle_combine_report",
    "oracle_missingness_report",
    "oracle_reference_panel_report",
]
