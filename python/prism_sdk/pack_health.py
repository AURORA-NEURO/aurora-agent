"""Typed projections for the observed benchmark-pack health gate.

``pack_health_assess`` is deliberately more than a score endpoint.  The Rust authority binds
observations to a content-addressed pack revision, keeps calibration separate from health defects,
and refuses a numeric score when the pack is saturated, floored, degenerate, contaminated, lacks a
grounded oracle, or otherwise cannot support the claim being made.  This module keeps those
boundaries visible to Python callers and preserves structured refusals as reports rather than
turning them into missing keys or accidental zeroes.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text
from .errors import ArgumentError
from .pack_catalogue import ORACLE_TIERS


PACK_HEALTH_MAX_INPUT_BYTES = 20_000_000
HEALTH_VERDICTS = frozenset({"healthy", "degraded", "unreportable"})
DISCRIMINATION_VERDICTS = frozenset({"undetermined", "saturated", "floored", "discriminating"})
HEALTH_FINDINGS = frozenset(
    {
        "saturated",
        "floored",
        "not_yet_characterised",
        "degenerate",
        "contaminated",
        "no_grounded_oracle",
        "counts_not_materialized",
    }
)
BLOCKING_FINDINGS = frozenset(
    {"saturated", "floored", "degenerate", "contaminated", "no_grounded_oracle"}
)
HEALTH_STAGES = frozenset({"pack_validation", "pack_health_assessment"})
CONTAMINATION_SIGNALS = frozenset(
    {
        "public_answer_key",
        "corpus_membership",
        "released_before_cutoff",
        "memorization_gap",
    }
)


def _bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _finite(name: str, value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be a finite number")
    return float(value)


def _optional_finite(name: str, value: Any) -> float | None:
    return None if value is None else _finite(name, value)


def _optional_text(name: str, value: Any) -> str | None:
    return None if value is None else _route_text(name, value)


def _string_array(name: str, value: Any) -> tuple[str, ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes, bytearray)):
        raise ArgumentError(f"{name} must be an array of strings")
    return tuple(_route_text(f"{name}[{index}]", item) for index, item in enumerate(value))


def _payload(value: Mapping[str, Any]) -> dict[str, Any]:
    """Extract direct JSON, MCP structured content, or an HTTP tool envelope."""

    raw = _route_mapping("pack health response", value)

    def matches(candidate: Mapping[str, Any]) -> bool:
        if not isinstance(candidate.get("ok"), bool):
            return False
        if candidate.get("ok") is True:
            return (
                isinstance(candidate.get("health"), Mapping)
                and isinstance(candidate.get("calibration"), Mapping)
                and isinstance(candidate.get("score_gate"), Mapping)
            )
        return (
            isinstance(candidate.get("stage"), str)
            and isinstance(candidate.get("refusal"), str)
            and candidate.get("fail_closed") is True
        )

    candidates: list[Mapping[str, Any]] = [raw]
    for container in (raw.get("mcp"), raw.get("result")):
        if not isinstance(container, Mapping):
            continue
        candidates.append(container)
        nested_result = container.get("result")
        if isinstance(nested_result, Mapping):
            candidates.append(nested_result)
            nested_structured = nested_result.get("structuredContent")
            if isinstance(nested_structured, Mapping):
                candidates.append(nested_structured)
            nested_content = nested_result.get("content")
            if isinstance(nested_content, list):
                for block in nested_content:
                    if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                        continue
                    try:
                        decoded = json.loads(block["text"])
                    except json.JSONDecodeError as error:
                        raise ArgumentError(f"pack health response text is not JSON: {error}") from error
                    if isinstance(decoded, Mapping):
                        candidates.append(decoded)
        structured = container.get("structuredContent")
        if isinstance(structured, Mapping):
            candidates.append(structured)
        content = container.get("content")
        if isinstance(content, list):
            for block in content:
                if not isinstance(block, Mapping) or not isinstance(block.get("text"), str):
                    continue
                try:
                    decoded = json.loads(block["text"])
                except json.JSONDecodeError as error:
                    raise ArgumentError(f"pack health response text is not JSON: {error}") from error
                if isinstance(decoded, Mapping):
                    candidates.append(decoded)
    for candidate in candidates:
        if matches(candidate):
            return dict(candidate)
    raise ArgumentError("response does not contain a pack-health assessment projection")


@dataclass(frozen=True)
class PackHealthAssessArgs:
    """Wire arguments for the authoritative Rust pack-health assessment."""

    pack: Mapping[str, Any]
    observations: Mapping[str, Any]
    policy: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        pack = _route_mapping("pack health pack", self.pack)
        observations = _route_mapping("pack health observations", self.observations)
        policy = None if self.policy is None else _route_mapping("pack health policy", self.policy)
        arguments: dict[str, Any] = {"pack": pack, "observations": observations}
        if policy is not None:
            arguments["policy"] = policy
        try:
            encoded = json.dumps(arguments, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise ArgumentError(f"pack health arguments are not JSON serializable: {error}") from error
        if len(encoded) > PACK_HEALTH_MAX_INPUT_BYTES:
            raise ArgumentError("pack health input exceeds the 20000000-byte safety bound")
        object.__setattr__(self, "pack", pack)
        object.__setattr__(self, "observations", observations)
        object.__setattr__(self, "policy", policy)

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackHealthAssessArgs":
        raw = _route_mapping("pack health arguments", value)
        return cls(
            _route_mapping("pack health pack", raw.get("pack")),
            _route_mapping("pack health observations", raw.get("observations")),
            None if raw.get("policy") is None else _route_mapping("pack health policy", raw.get("policy")),
        )

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"pack": dict(self.pack), "observations": dict(self.observations)}
        if self.policy is not None:
            arguments["policy"] = dict(self.policy)
        return arguments


@dataclass(frozen=True)
class PackSystemObservationReport:
    """One system's pass/trial counts; a missing trial is not a zero pass rate."""

    raw: dict[str, Any]
    system: str
    trials: int
    passes: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackSystemObservationReport":
        raw = _route_mapping("pack health system observation", value)
        trials = _route_count("pack health observation trials", raw.get("trials"))
        passes = _route_count("pack health observation passes", raw.get("passes"))
        if passes > trials:
            raise ArgumentError("pack health observation passes cannot exceed trials")
        return cls(raw, _route_text("pack health observation system", raw.get("system")), trials, passes)

    @property
    def pass_rate(self) -> float | None:
        return None if self.trials == 0 else self.passes / self.trials

    @property
    def measured(self) -> bool:
        return self.trials > 0


@dataclass(frozen=True)
class PackCalibrationReport:
    raw: dict[str, Any]
    observations: tuple[PackSystemObservationReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackCalibrationReport":
        raw = _route_mapping("pack health calibration", value)
        observations_raw = raw.get("observations")
        if not isinstance(observations_raw, Sequence) or isinstance(observations_raw, (str, bytes, bytearray)):
            raise ArgumentError("pack health calibration observations must be an array")
        return cls(raw, tuple(PackSystemObservationReport.from_wire(item) for item in observations_raw))

    @property
    def total_trials(self) -> int:
        return sum(item.trials for item in self.observations)

    @property
    def total_passes(self) -> int:
        return sum(item.passes for item in self.observations)

    @property
    def pooled_pass_rate(self) -> float | None:
        return None if self.total_trials == 0 else self.total_passes / self.total_trials

    @property
    def measured_systems(self) -> int:
        return sum(item.measured for item in self.observations)


@dataclass(frozen=True)
class PackDiscriminationReport:
    raw: dict[str, Any]
    verdict: str
    reason: str | None
    pooled_pass_rate: float | None
    systems: int | None
    lowest: float | None
    highest: float | None
    separated: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackDiscriminationReport":
        raw = _route_mapping("pack health discrimination", value)
        verdict = _route_text("pack health discrimination verdict", raw.get("verdict"))
        if verdict not in DISCRIMINATION_VERDICTS:
            raise ArgumentError(f"unknown pack health discrimination verdict {verdict!r}")
        reason = _optional_text("pack health discrimination reason", raw.get("reason"))
        pooled = _optional_finite("pack health pooled_pass_rate", raw.get("pooled_pass_rate"))
        systems_raw = raw.get("systems")
        systems = None if systems_raw is None else _route_count("pack health discrimination systems", systems_raw)
        lowest = _optional_finite("pack health discrimination lowest", raw.get("lowest"))
        highest = _optional_finite("pack health discrimination highest", raw.get("highest"))
        separated_raw = raw.get("separated")
        separated = None if separated_raw is None else _bool("pack health discrimination separated", separated_raw)
        if verdict == "undetermined" and reason is None:
            raise ArgumentError("undetermined pack health discrimination requires a reason")
        if verdict in {"saturated", "floored"} and (pooled is None or systems is None):
            raise ArgumentError(f"{verdict} pack health discrimination requires pooled rate and systems")
        if verdict == "discriminating" and (lowest is None or highest is None or separated is None):
            raise ArgumentError("discriminating pack health output requires bounds and separation")
        return cls(raw, verdict, reason, pooled, systems, lowest, highest, separated)

    @property
    def is_discriminating(self) -> bool:
        return self.verdict == "discriminating"


@dataclass(frozen=True)
class PackContaminationSignalReport:
    raw: dict[str, Any]
    signal: str
    location: str | None
    corpus: str | None
    matched_instances: int | None
    pack_release: str | None
    model_cutoff: str | None
    public: PackSystemObservationReport | None
    held_out: PackSystemObservationReport | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackContaminationSignalReport":
        raw = _route_mapping("pack health contamination signal", value)
        signal = _route_text("pack health contamination signal kind", raw.get("signal"))
        if signal not in CONTAMINATION_SIGNALS:
            raise ArgumentError(f"unknown pack health contamination signal {signal!r}")
        location = _optional_text("pack health contamination location", raw.get("location"))
        corpus = _optional_text("pack health contamination corpus", raw.get("corpus"))
        matched_raw = raw.get("matched_instances")
        matched = None if matched_raw is None else _route_count("pack health matched_instances", matched_raw)
        pack_release = _optional_text("pack health pack_release", raw.get("pack_release"))
        model_cutoff = _optional_text("pack health model_cutoff", raw.get("model_cutoff"))
        public_raw = raw.get("public")
        held_out_raw = raw.get("held_out")
        public = None if public_raw is None else PackSystemObservationReport.from_wire(public_raw)
        held_out = None if held_out_raw is None else PackSystemObservationReport.from_wire(held_out_raw)
        required: dict[str, bool] = {
            "public_answer_key": location is not None,
            "corpus_membership": corpus is not None and matched is not None,
            "released_before_cutoff": pack_release is not None and model_cutoff is not None,
            "memorization_gap": public is not None and held_out is not None,
        }
        if not required[signal]:
            raise ArgumentError(f"{signal} contamination signal is missing required fields")
        return cls(raw, signal, location, corpus, matched, pack_release, model_cutoff, public, held_out)

    @property
    def pass_rate_gap(self) -> float | None:
        if self.public is None or self.held_out is None:
            return None
        if self.public.pass_rate is None or self.held_out.pass_rate is None:
            return None
        return self.public.pass_rate - self.held_out.pass_rate


@dataclass(frozen=True)
class PackHealthFindingReport:
    raw: dict[str, Any]
    finding: str
    severity: str
    pooled_pass_rate: float | None
    systems: int | None
    reason: str | None
    baseline: str | None
    baseline_pass_rate: float | None
    best_system_pass_rate: float | None
    signal: PackContaminationSignalReport | None
    tiers: tuple[str, ...]
    declared: int | None
    validated: int | None
    materialized_fraction: float | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackHealthFindingReport":
        raw = _route_mapping("pack health finding", value)
        finding = _route_text("pack health finding kind", raw.get("finding"))
        if finding not in HEALTH_FINDINGS:
            raise ArgumentError(f"unknown pack health finding {finding!r}")
        pooled = _optional_finite("pack health finding pooled_pass_rate", raw.get("pooled_pass_rate"))
        systems_raw = raw.get("systems")
        systems = None if systems_raw is None else _route_count("pack health finding systems", systems_raw)
        reason = _optional_text("pack health finding reason", raw.get("reason"))
        baseline = _optional_text("pack health finding baseline", raw.get("baseline"))
        baseline_rate = _optional_finite("pack health finding baseline_pass_rate", raw.get("baseline_pass_rate"))
        best_rate = _optional_finite("pack health finding best_system_pass_rate", raw.get("best_system_pass_rate"))
        signal_raw = raw.get("signal")
        signal = None if signal_raw is None else PackContaminationSignalReport.from_wire(signal_raw)
        tiers_raw = raw.get("tiers", [])
        tiers = _string_array("pack health finding tiers", tiers_raw)
        unknown_tiers = set(tiers) - ORACLE_TIERS
        if unknown_tiers:
            raise ArgumentError(f"unknown pack health oracle tier(s): {sorted(unknown_tiers)!r}")
        declared_raw = raw.get("declared")
        validated_raw = raw.get("validated")
        declared = None if declared_raw is None else _route_count("pack health finding declared", declared_raw)
        validated = None if validated_raw is None else _route_count("pack health finding validated", validated_raw)
        fraction = _optional_finite("pack health finding materialized_fraction", raw.get("materialized_fraction"))
        required: dict[str, bool] = {
            "saturated": pooled is not None and systems is not None,
            "floored": pooled is not None and systems is not None,
            "not_yet_characterised": reason is not None,
            "degenerate": baseline is not None and baseline_rate is not None and best_rate is not None,
            "contaminated": signal is not None,
            "no_grounded_oracle": "tiers" in raw,
            "counts_not_materialized": declared is not None and validated is not None and fraction is not None,
        }
        if not required[finding]:
            raise ArgumentError(f"{finding} pack health finding is missing required fields")
        return cls(raw, finding, "blocking" if finding in BLOCKING_FINDINGS else "advisory", pooled, systems, reason, baseline, baseline_rate, best_rate, signal, tiers, declared, validated, fraction)

    @property
    def blocking(self) -> bool:
        return self.severity == "blocking"


@dataclass(frozen=True)
class PackHealthReport:
    raw: dict[str, Any]
    pack: str
    pack_digest: str
    findings: tuple[PackHealthFindingReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackHealthReport":
        raw = _route_mapping("pack health", value)
        findings_raw = raw.get("findings")
        if not isinstance(findings_raw, Sequence) or isinstance(findings_raw, (str, bytes, bytearray)):
            raise ArgumentError("pack health findings must be an array")
        findings = tuple(PackHealthFindingReport.from_wire(item) for item in findings_raw)
        return cls(raw, _route_text("pack health pack", raw.get("pack")), _route_text("pack health pack_digest", raw.get("pack_digest")), findings)

    @property
    def blocking_findings(self) -> tuple[PackHealthFindingReport, ...]:
        return tuple(item for item in self.findings if item.blocking)

    @property
    def advisory_findings(self) -> tuple[PackHealthFindingReport, ...]:
        return tuple(item for item in self.findings if not item.blocking)


@dataclass(frozen=True)
class PackScoreReport:
    raw: dict[str, Any]
    pack: str
    pack_digest: str
    pooled_pass_rate: float
    discrimination: PackDiscriminationReport
    advisories: tuple[PackHealthFindingReport, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackScoreReport":
        raw = _route_mapping("pack health score", value)
        advisories_raw = raw.get("advisories")
        if not isinstance(advisories_raw, Sequence) or isinstance(advisories_raw, (str, bytes, bytearray)):
            raise ArgumentError("pack health score advisories must be an array")
        advisories = tuple(PackHealthFindingReport.from_wire(item) for item in advisories_raw)
        blocking = tuple(item for item in advisories if item.blocking)
        if blocking:
            raise ArgumentError("reportable pack score cannot contain blocking advisories")
        return cls(
            raw,
            _route_text("pack health score pack", raw.get("pack")),
            _route_text("pack health score pack_digest", raw.get("pack_digest")),
            _finite("pack health score pooled_pass_rate", raw.get("pooled_pass_rate")),
            PackDiscriminationReport.from_wire(raw.get("discrimination")),
            advisories,
        )


@dataclass(frozen=True)
class PackScoreGateReport:
    raw: dict[str, Any]
    reportable: bool
    score: PackScoreReport | None
    refusal: str | None
    fail_closed: bool | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackScoreGateReport":
        raw = _route_mapping("pack health score gate", value)
        reportable = _bool("pack health score gate reportable", raw.get("reportable"))
        if reportable:
            score_raw = raw.get("score")
            if not isinstance(score_raw, Mapping):
                raise ArgumentError("reportable pack health score gate requires a score")
            return cls(raw, True, PackScoreReport.from_wire(score_raw), None, None)
        refusal = _route_text("pack health score gate refusal", raw.get("refusal"))
        if raw.get("fail_closed") is not True:
            raise ArgumentError("unreportable pack health score gate must fail closed")
        if raw.get("score") is not None:
            raise ArgumentError("unreportable pack health score gate must withhold score")
        return cls(raw, False, None, refusal, True)

    @property
    def score_withheld(self) -> bool:
        return not self.reportable


@dataclass(frozen=True)
class PackHealthAssessmentReport:
    """Complete health result, including either a score gate or a structured fail-closed refusal."""

    raw: dict[str, Any]
    ok: bool
    stage: str | None
    refusal: str | None
    fail_closed: bool
    pack: str | None
    pack_digest: str | None
    verdict: str | None
    finding_count: int | None
    blocking_finding_count: int | None
    advisory_finding_count: int | None
    health: PackHealthReport | None
    calibration: PackCalibrationReport | None
    score_gate: PackScoreGateReport | None
    guarantees: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "PackHealthAssessmentReport":
        raw = _payload(value)
        ok = _bool("pack health ok", raw.get("ok"))
        guarantees = _route_strings("pack health guarantees", raw.get("guarantees", []))
        if not ok:
            stage = _route_text("pack health refusal stage", raw.get("stage"))
            if stage not in HEALTH_STAGES:
                raise ArgumentError(f"unknown pack health refusal stage {stage!r}")
            if raw.get("score") is not None:
                raise ArgumentError("refused pack health assessment must withhold score")
            return cls(
                raw=raw,
                ok=False,
                stage=stage,
                refusal=_route_text("pack health refusal", raw.get("refusal")),
                fail_closed=_bool("pack health fail_closed", raw.get("fail_closed")),
                pack=None,
                pack_digest=None,
                verdict=None,
                finding_count=None,
                blocking_finding_count=None,
                advisory_finding_count=None,
                health=None,
                calibration=None,
                score_gate=None,
                guarantees=guarantees,
            )

        health = PackHealthReport.from_wire(raw.get("health"))
        calibration = PackCalibrationReport.from_wire(raw.get("calibration"))
        score_gate = PackScoreGateReport.from_wire(raw.get("score_gate"))
        verdict = _route_text("pack health verdict", raw.get("verdict"))
        if verdict not in HEALTH_VERDICTS:
            raise ArgumentError(f"unknown pack health verdict {verdict!r}")
        finding_count = _route_count("pack health finding_count", raw.get("finding_count"))
        blocking_count = _route_count("pack health blocking_findings", raw.get("blocking_findings"))
        advisory_count = _route_count("pack health advisory_findings", raw.get("advisory_findings"))
        if finding_count != len(health.findings) or blocking_count != len(health.blocking_findings) or advisory_count != len(health.advisory_findings):
            raise ArgumentError("pack health finding counts do not match typed findings")
        pack = _route_text("pack health pack", raw.get("pack"))
        pack_digest = _route_text("pack health pack_digest", raw.get("pack_digest"))
        if health.pack != pack or health.pack_digest != pack_digest:
            raise ArgumentError("pack health digest binding is inconsistent across projections")
        if verdict == "unreportable" and score_gate.reportable:
            raise ArgumentError("unreportable pack health verdict cannot expose a reportable score")
        return cls(raw, True, None, None, False, pack, pack_digest, verdict, finding_count, blocking_count, advisory_count, health, calibration, score_gate, guarantees)

    @property
    def accepted(self) -> bool:
        return self.ok

    @property
    def refused(self) -> bool:
        return not self.ok

    @property
    def reportable(self) -> bool:
        return self.score_gate is not None and self.score_gate.reportable

    @property
    def score(self) -> PackScoreReport | None:
        return None if self.score_gate is None else self.score_gate.score

    @property
    def score_withheld(self) -> bool:
        return not self.reportable

    @property
    def blocking_findings(self) -> tuple[PackHealthFindingReport, ...]:
        return () if self.health is None else self.health.blocking_findings

    @property
    def advisory_findings(self) -> tuple[PackHealthFindingReport, ...]:
        return () if self.health is None else self.health.advisory_findings

    @property
    def digest_bound(self) -> bool:
        if self.pack is None or self.pack_digest is None or self.health is None:
            return False
        if self.health.pack != self.pack or self.health.pack_digest != self.pack_digest:
            return False
        return self.score is None or (self.score.pack == self.pack and self.score.pack_digest == self.pack_digest)

    @property
    def declarations_and_observations_separate(self) -> bool:
        return any("declarations, observed outcomes" in guarantee for guarantee in self.guarantees)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def pack_health_assessment_report(value: Mapping[str, Any]) -> PackHealthAssessmentReport:
    """Parse direct MCP output or an HTTP REST tool envelope."""

    return PackHealthAssessmentReport.from_wire(value)


__all__ = [
    "PACK_HEALTH_MAX_INPUT_BYTES",
    "HEALTH_VERDICTS",
    "DISCRIMINATION_VERDICTS",
    "HEALTH_FINDINGS",
    "BLOCKING_FINDINGS",
    "CONTAMINATION_SIGNALS",
    "PackHealthAssessArgs",
    "PackSystemObservationReport",
    "PackCalibrationReport",
    "PackDiscriminationReport",
    "PackContaminationSignalReport",
    "PackHealthFindingReport",
    "PackHealthReport",
    "PackScoreReport",
    "PackScoreGateReport",
    "PackHealthAssessmentReport",
    "pack_health_assessment_report",
]
