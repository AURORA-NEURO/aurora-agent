"""Typed authoring and request contracts for the Rust oracle/evaluation mesh.

The classes in this module serialize the contracts owned by ``bioprism-oracle`` and
``bioprism-bioevalx``.  They validate identity, timestamp, confidence, tier, plane, and
distribution invariants that are safe to check before transport.  They deliberately do not
combine judgements, score predictions, resolve disagreement, or infer biological truth: those
decisions remain in the Rust MCP tools.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import json
import math
import re
from typing import Any, Iterable, Mapping, Sequence

from .authoring import AuthoringError, canonical_json

JsonObject = dict[str, Any]
_TIMESTAMP = re.compile(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$")
_PLANES = {
    "artifact",
    "analytical",
    "measurement",
    "biological",
    "causal",
    "longitudinal",
    "translational",
    "policy",
}
_SHARED_RESOURCES = {
    "training_data",
    "preprocessing_code",
    "labels",
    "model",
    "annotators",
    "sites",
    "assumptions",
}
_UNCERTAINTY_MODELS = {"exact", "acceptable_set", "distribution"}


def _clone(value: Any) -> Any:
    return json.loads(canonical_json(value))


def _text(value: str, path: str, max_bytes: int = 4096) -> str:
    if not isinstance(value, str) or not value.strip():
        raise AuthoringError(f"{path}: expected a non-empty string")
    if "\r" in value or "\n" in value or len(value.encode("utf-8")) > max_bytes:
        raise AuthoringError(f"{path}: value is not line-safe or exceeds {max_bytes} UTF-8 bytes")
    return value


def _nonnegative_int(value: int, path: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AuthoringError(f"{path}: expected a non-negative integer")
    return value


def _timestamp(value: str, path: str) -> str:
    value = _text(value, path, max_bytes=20)
    match = _TIMESTAMP.fullmatch(value)
    if match is None:
        raise AuthoringError(f"{path}: expected YYYY-MM-DDTHH:MM:SSZ")
    year, month, day, hour, minute, second = (int(part) for part in match.groups())
    if not 1 <= month <= 12:
        raise AuthoringError(f"{path}: month is outside 01..=12")
    month_days = (31, 29 if year % 4 == 0 and (year % 100 != 0 or year % 400 == 0) else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
    if not 1 <= day <= month_days[month - 1]:
        raise AuthoringError(f"{path}: day does not exist in that month")
    if hour > 23 or minute > 59 or second > 59:
        raise AuthoringError(f"{path}: time component is outside the supported UTC range")
    return value


def _finite_probability(value: float, path: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise AuthoringError(f"{path}: expected a finite number")
    if not 0.0 <= value <= 1.0:
        raise AuthoringError(f"{path}: expected a value in [0, 1]")
    return float(value)


def _mapping(value: Mapping[str, Any], path: str) -> JsonObject:
    if not isinstance(value, Mapping):
        raise AuthoringError(f"{path}: expected an object")
    return _clone(dict(value))


class EvidenceTier(str, Enum):
    """The Rust evidence ladder, weakest to strongest."""

    JUDGE = "judge"
    STATISTICAL = "statistical"
    PROPERTY = "property"
    EXECUTION = "execution"
    DETERMINISTIC = "deterministic"

    @property
    def rank(self) -> int:
        return (EvidenceTier.JUDGE, EvidenceTier.STATISTICAL, EvidenceTier.PROPERTY, EvidenceTier.EXECUTION, EvidenceTier.DETERMINISTIC).index(self)

    @property
    def is_grounded(self) -> bool:
        return self in {EvidenceTier.EXECUTION, EvidenceTier.DETERMINISTIC}

    @property
    def is_nondeterministic(self) -> bool:
        return self in {EvidenceTier.JUDGE, EvidenceTier.STATISTICAL}

    def demoted(self) -> "EvidenceTier":
        if self is EvidenceTier.DETERMINISTIC:
            return EvidenceTier.PROPERTY
        if self is EvidenceTier.EXECUTION:
            return EvidenceTier.PROPERTY
        if self is EvidenceTier.PROPERTY:
            return EvidenceTier.PROPERTY
        return EvidenceTier.JUDGE


class Position(str, Enum):
    SUPPORTED = "supported"
    CONTRADICTED = "contradicted"
    UNRESOLVED = "unresolved"
    NOT_EVALUABLE = "not_evaluable"

    @property
    def is_abstention(self) -> bool:
        return self in {Position.UNRESOLVED, Position.NOT_EVALUABLE}


@dataclass(frozen=True)
class OracleVersion:
    major: int
    minor: int
    patch: int

    def __post_init__(self) -> None:
        _nonnegative_int(self.major, "oracle.version.major")
        _nonnegative_int(self.minor, "oracle.version.minor")
        _nonnegative_int(self.patch, "oracle.version.patch")

    def to_dict(self) -> JsonObject:
        return {"major": self.major, "minor": self.minor, "patch": self.patch}


@dataclass(frozen=True)
class OracleRef:
    """A versioned ``namespace:name`` oracle identity."""

    id: str
    version: OracleVersion

    def __post_init__(self) -> None:
        identity = _text(self.id, "oracle.id", max_bytes=256)
        if identity.count(":") != 1 or any(not part for part in identity.split(":")):
            raise AuthoringError("oracle.id: expected exactly one ':' separating namespace and name")
        if any(char.isspace() or ord(char) < 32 for char in identity):
            raise AuthoringError("oracle.id: control characters and whitespace are not allowed")
        if not isinstance(self.version, OracleVersion):
            raise AuthoringError("oracle.version: expected OracleVersion")

    @property
    def kind(self) -> str:
        return self.id

    @property
    def rendered(self) -> str:
        return f"biooracle:{self.id}:{self.version.major}.{self.version.minor}.{self.version.patch}"

    def to_dict(self) -> JsonObject:
        return {"id": self.id, "version": self.version.to_dict()}


@dataclass(frozen=True)
class ValidityWindow:
    valid_from: str
    valid_until: str | None = None

    def __post_init__(self) -> None:
        start = _timestamp(self.valid_from, "validity.valid_from")
        end = None if self.valid_until is None else _timestamp(self.valid_until, "validity.valid_until")
        if end is not None and end < start:
            raise AuthoringError("validity: valid_until cannot precede valid_from")

    def contains(self, at: str) -> bool:
        at = _timestamp(at, "at")
        return at >= self.valid_from and (self.valid_until is None or at <= self.valid_until)

    def to_dict(self) -> JsonObject:
        return {"valid_from": self.valid_from, "valid_until": self.valid_until}


@dataclass(frozen=True)
class Independence:
    from_evaluated_system: bool = True
    shared: frozenset[str] = frozenset()

    def __post_init__(self) -> None:
        if not isinstance(self.from_evaluated_system, bool):
            raise AuthoringError("independence.from_evaluated_system: expected boolean")
        shared = frozenset(self.shared)
        if any(resource not in _SHARED_RESOURCES for resource in shared):
            raise AuthoringError("independence.shared: contains an unknown shared resource")
        object.__setattr__(self, "shared", shared)

    @property
    def is_circular(self) -> bool:
        return not self.from_evaluated_system or bool(self.shared)

    def to_dict(self) -> JsonObject:
        return {"from_evaluated_system": self.from_evaluated_system, "shared": sorted(self.shared)}


@dataclass(frozen=True)
class Admissibility:
    """A retained admissibility state for one judgement."""

    state: str
    fields: tuple[tuple[str, Any], ...] = ()

    def __post_init__(self) -> None:
        if self.state not in {"admissible", "not_yet_valid", "expired", "superseded"}:
            raise AuthoringError("admissibility.state: unknown state")
        field_names = {name for name, _ in self.fields}
        if len(field_names) != len(self.fields):
            raise AuthoringError("admissibility: duplicate field")

    @classmethod
    def admissible(cls) -> "Admissibility":
        return cls("admissible")

    @classmethod
    def not_yet_valid(cls, at: str, valid_from: str) -> "Admissibility":
        return cls("not_yet_valid", (("at", _timestamp(at, "admissibility.at")), ("valid_from", _timestamp(valid_from, "admissibility.valid_from"))))

    @classmethod
    def expired(cls, at: str, valid_until: str) -> "Admissibility":
        return cls("expired", (("at", _timestamp(at, "admissibility.at")), ("valid_until", _timestamp(valid_until, "admissibility.valid_until"))))

    @classmethod
    def superseded(cls, by: OracleRef) -> "Admissibility":
        if not isinstance(by, OracleRef):
            raise AuthoringError("admissibility.by: expected OracleRef")
        return cls("superseded", (("by", by.to_dict()),))

    @property
    def is_admissible(self) -> bool:
        return self.state == "admissible"

    def to_dict(self) -> JsonObject:
        result: JsonObject = {"state": self.state}
        result.update({name: _clone(value) for name, value in self.fields})
        return result


@dataclass(frozen=True)
class OracleManifest:
    """A complete pre-decision oracle declaration."""

    oracle: OracleRef
    declared_tier: EvidenceTier
    establishes: frozenset[str]
    cannot_establish: frozenset[str]
    validity: ValidityWindow
    superseded_by: OracleRef | None = None
    independence: Independence = Independence()
    uncertainty_model: str = "exact"
    known_failure_modes: tuple[str, ...] = ()

    def __post_init__(self) -> None:
        if not isinstance(self.oracle, OracleRef):
            raise AuthoringError("manifest.oracle: expected OracleRef")
        if not isinstance(self.declared_tier, EvidenceTier):
            raise AuthoringError("manifest.declared_tier: expected EvidenceTier")
        establishes = frozenset(self.establishes)
        cannot = frozenset(self.cannot_establish)
        if not establishes:
            raise AuthoringError("manifest.establishes: at least one plane is required")
        if any(plane not in _PLANES for plane in establishes | cannot):
            raise AuthoringError("manifest: unknown evidential plane")
        if establishes & cannot:
            raise AuthoringError("manifest: establishes and cannot_establish overlap")
        if self.uncertainty_model not in _UNCERTAINTY_MODELS:
            raise AuthoringError("manifest.uncertainty_model: unknown model")
        if self.superseded_by is not None and not isinstance(self.superseded_by, OracleRef):
            raise AuthoringError("manifest.superseded_by: expected OracleRef or None")
        object.__setattr__(self, "establishes", establishes)
        object.__setattr__(self, "cannot_establish", cannot)
        object.__setattr__(self, "known_failure_modes", tuple(_text(mode, "manifest.known_failure_mode", 4096) for mode in self.known_failure_modes))

    @property
    def effective_tier(self) -> EvidenceTier:
        return self.declared_tier.demoted() if self.independence.is_circular else self.declared_tier

    def admissibility(self, at: str) -> Admissibility:
        at = _timestamp(at, "at")
        if self.superseded_by is not None:
            return Admissibility.superseded(self.superseded_by)
        if at < self.validity.valid_from:
            return Admissibility.not_yet_valid(at, self.validity.valid_from)
        if self.validity.valid_until is not None and at > self.validity.valid_until:
            return Admissibility.expired(at, self.validity.valid_until)
        return Admissibility.admissible()

    def disclaiming_rest(self) -> "OracleManifest":
        return OracleManifest(
            self.oracle,
            self.declared_tier,
            self.establishes,
            frozenset(_PLANES - self.establishes),
            self.validity,
            self.superseded_by,
            self.independence,
            self.uncertainty_model,
            self.known_failure_modes,
        )

    def to_dict(self) -> JsonObject:
        return {
            "oracle": self.oracle.to_dict(),
            "declared_tier": self.declared_tier.value,
            "establishes": sorted(self.establishes),
            "cannot_establish": sorted(self.cannot_establish),
            "validity": self.validity.to_dict(),
            "superseded_by": None if self.superseded_by is None else self.superseded_by.to_dict(),
            "independence": self.independence.to_dict(),
            "uncertainty_model": self.uncertainty_model,
            "known_failure_modes": list(self.known_failure_modes),
        }


@dataclass(frozen=True)
class PositionDistribution:
    """A validated, tie-preserving distribution over oracle positions."""

    mass: tuple[tuple[Position, float], ...]

    @classmethod
    def from_mapping(cls, mass: Mapping[str | Position, float]) -> "PositionDistribution":
        if not isinstance(mass, Mapping) or not mass:
            raise AuthoringError("belief: a non-empty distribution is required")
        pairs: list[tuple[Position, float]] = []
        for raw_position, raw_mass in mass.items():
            try:
                position = raw_position if isinstance(raw_position, Position) else Position(raw_position)
            except ValueError as exc:
                raise AuthoringError(f"belief: unknown position {raw_position!r}") from exc
            pairs.append((position, _finite_probability(raw_mass, f"belief.{position.value}")))
        return cls(tuple(sorted(pairs, key=lambda item: item[0].value)))

    def __post_init__(self) -> None:
        if not self.mass:
            raise AuthoringError("belief: a non-empty distribution is required")
        positions = [position for position, _ in self.mass]
        if len(positions) != len(set(positions)):
            raise AuthoringError("belief: duplicate position")
        total = sum(_finite_probability(value, f"belief.{position.value}") for position, value in self.mass)
        if abs(total - 1.0) > 1e-9:
            raise AuthoringError(f"belief: total mass is {total}, not 1")

    def modes(self) -> frozenset[Position]:
        peak = max(value for _, value in self.mass)
        return frozenset(position for position, value in self.mass if abs(value - peak) <= 1e-9)

    def to_dict(self) -> JsonObject:
        return {position.value: value for position, value in self.mass}


@dataclass(frozen=True)
class Finding:
    """One Rust-tagged finding; factory methods cover the common deterministic variants."""

    kind: str
    fields: tuple[tuple[str, Any], ...] = ()

    def __post_init__(self) -> None:
        _text(self.kind, "finding", max_bytes=128)
        if len({name for name, _ in self.fields}) != len(self.fields):
            raise AuthoringError("finding: duplicate field")

    @classmethod
    def missing_field(cls, pointer: str) -> "Finding":
        return cls("missing_field", (("pointer", _text(pointer, "finding.pointer", 1024)),))

    @classmethod
    def checksum_mismatch(cls, pointer: str, declared: str, computed: str) -> "Finding":
        return cls("checksum_mismatch", (("pointer", _text(pointer, "finding.pointer", 1024)), ("declared", _text(declared, "finding.declared", 256)), ("computed", _text(computed, "finding.computed", 256))))

    @classmethod
    def property_violated(cls, property: str, pointer: str, detail: str) -> "Finding":
        return cls("property_violated", (("property", _text(property, "finding.property", 256)), ("pointer", _text(pointer, "finding.pointer", 1024)), ("detail", _text(detail, "finding.detail", 4096))))

    @classmethod
    def not_applicable(cls, check: str, reason: str) -> "Finding":
        return cls("not_applicable", (("check", _text(check, "finding.check", 256)), ("reason", _text(reason, "finding.reason", 4096))))

    def to_dict(self) -> JsonObject:
        result: JsonObject = {"finding": self.kind}
        result.update({name: _clone(value) for name, value in self.fields})
        return result


@dataclass(frozen=True)
class Judgement:
    """One serialized oracle judgement, retaining all evidence and refusals."""

    oracle: OracleRef
    tier: EvidenceTier
    declared_tier: EvidenceTier
    position: Position
    confidence: float
    belief: PositionDistribution | None = None
    establishes: frozenset[str] = frozenset()
    cannot_establish: frozenset[str] = frozenset()
    findings: tuple[Finding, ...] = ()
    admissibility: Admissibility = Admissibility.admissible()
    rationale: str = ""

    def __post_init__(self) -> None:
        if not isinstance(self.oracle, OracleRef):
            raise AuthoringError("judgement.oracle: expected OracleRef")
        if not isinstance(self.tier, EvidenceTier) or not isinstance(self.declared_tier, EvidenceTier):
            raise AuthoringError("judgement.tier: expected EvidenceTier values")
        if not isinstance(self.position, Position):
            raise AuthoringError("judgement.position: expected Position")
        _finite_probability(self.confidence, "judgement.confidence")
        establishes = frozenset(self.establishes)
        cannot = frozenset(self.cannot_establish)
        if any(plane not in _PLANES for plane in establishes | cannot):
            raise AuthoringError("judgement: unknown evidential plane")
        if establishes & cannot:
            raise AuthoringError("judgement: establishes and cannot_establish overlap")
        if self.belief is not None and not isinstance(self.belief, PositionDistribution):
            raise AuthoringError("judgement.belief: expected PositionDistribution or None")
        if not all(isinstance(finding, Finding) for finding in self.findings):
            raise AuthoringError("judgement.findings: expected Finding values")
        if not isinstance(self.admissibility, Admissibility):
            raise AuthoringError("judgement.admissibility: expected Admissibility")
        object.__setattr__(self, "confidence", float(self.confidence))
        object.__setattr__(self, "establishes", establishes)
        object.__setattr__(self, "cannot_establish", cannot)
        object.__setattr__(self, "findings", tuple(self.findings))
        _text(self.rationale, "judgement.rationale", max_bytes=8192) if self.rationale else None

    @classmethod
    def from_manifest(
        cls,
        manifest: OracleManifest,
        at: str,
        position: Position,
        confidence: float = 1.0,
    ) -> "Judgement":
        if not isinstance(manifest, OracleManifest):
            raise AuthoringError("manifest: expected OracleManifest")
        return cls(
            oracle=manifest.oracle,
            tier=manifest.effective_tier,
            declared_tier=manifest.declared_tier,
            position=position if isinstance(position, Position) else Position(position),
            confidence=confidence,
            establishes=manifest.establishes,
            cannot_establish=manifest.cannot_establish,
            admissibility=manifest.admissibility(at),
        )

    def to_dict(self) -> JsonObject:
        return {
            "oracle": self.oracle.to_dict(),
            "tier": self.tier.value,
            "declared_tier": self.declared_tier.value,
            "position": self.position.value,
            "confidence": self.confidence,
            "belief": None if self.belief is None else self.belief.to_dict(),
            "establishes": sorted(self.establishes),
            "cannot_establish": sorted(self.cannot_establish),
            "findings": [finding.to_dict() for finding in self.findings],
            "admissibility": self.admissibility.to_dict(),
            "rationale": self.rationale,
        }


class JudgementBuilder:
    """Build a judgement from an oracle manifest without allowing tier inflation."""

    def __init__(self, manifest: OracleManifest, at: str, position: Position, confidence: float = 1.0) -> None:
        self.manifest = manifest
        self.at = _timestamp(at, "at")
        self.position = position if isinstance(position, Position) else Position(position)
        self.confidence = _finite_probability(confidence, "confidence")
        self._belief: PositionDistribution | None = None
        self._findings: list[Finding] = []
        self._rationale = ""

    def belief(self, distribution: PositionDistribution) -> "JudgementBuilder":
        if not isinstance(distribution, PositionDistribution):
            raise AuthoringError("belief: expected PositionDistribution")
        self._belief = distribution
        return self

    def finding(self, finding: Finding) -> "JudgementBuilder":
        if not isinstance(finding, Finding):
            raise AuthoringError("finding: expected Finding")
        self._findings.append(finding)
        return self

    def rationale(self, value: str) -> "JudgementBuilder":
        self._rationale = _text(value, "rationale", max_bytes=8192)
        return self

    def build(self) -> Judgement:
        return Judgement(
            oracle=self.manifest.oracle,
            tier=self.manifest.effective_tier,
            declared_tier=self.manifest.declared_tier,
            position=self.position,
            confidence=self.confidence,
            belief=self._belief,
            establishes=self.manifest.establishes,
            cannot_establish=self.manifest.cannot_establish,
            findings=tuple(self._findings),
            admissibility=self.manifest.admissibility(self.at),
            rationale=self._rationale,
        )


def _judgement_wire(value: Judgement | Mapping[str, Any], path: str) -> JsonObject:
    if isinstance(value, Judgement):
        return value.to_dict()
    return _mapping(value, path)


@dataclass(frozen=True)
class OracleCombineRequest:
    subject: str
    at: str
    judgements: tuple[Judgement | Mapping[str, Any], ...]
    minimum_deciding_tier: EvidenceTier = EvidenceTier.JUDGE
    max_items: int = 100

    def __post_init__(self) -> None:
        _text(self.subject, "subject", 1024)
        _timestamp(self.at, "at")
        if not self.judgements or len(self.judgements) > 1000:
            raise AuthoringError("judgements: expected between 1 and 1000 entries")
        if not isinstance(self.minimum_deciding_tier, EvidenceTier):
            raise AuthoringError("minimum_deciding_tier: expected EvidenceTier")
        if not 1 <= _nonnegative_int(self.max_items, "max_items") <= 1000:
            raise AuthoringError("max_items: expected an integer between 1 and 1000")
        object.__setattr__(self, "judgements", tuple(self.judgements))

    def to_mcp_arguments(self) -> JsonObject:
        return {
            "subject": self.subject,
            "at": self.at,
            "judgements": [_judgement_wire(value, f"judgements[{index}]") for index, value in enumerate(self.judgements)],
            "minimum_deciding_tier": self.minimum_deciding_tier.value,
            "max_items": self.max_items,
        }


@dataclass(frozen=True)
class ReferencePanelRequest:
    panel: Mapping[str, Any]
    rule: Mapping[str, Any] | None = None
    model_call: str | None = None
    max_items: int = 100

    def __post_init__(self) -> None:
        _mapping(self.panel, "panel")
        if self.rule is not None:
            _mapping(self.rule, "rule")
        if self.model_call is not None:
            _text(self.model_call, "model_call", 256)
        if not 1 <= _nonnegative_int(self.max_items, "max_items") <= 1000:
            raise AuthoringError("max_items: expected an integer between 1 and 1000")

    def to_mcp_arguments(self) -> JsonObject:
        result: JsonObject = {"panel": _mapping(self.panel, "panel"), "max_items": self.max_items}
        if self.rule is not None:
            result["rule"] = _mapping(self.rule, "rule")
        if self.model_call is not None:
            result["model_call"] = self.model_call
        return result


@dataclass(frozen=True)
class MissingnessAuditRequest:
    pattern: Mapping[str, Any]
    field: Mapping[str, Any]
    boundary: Mapping[str, Any]
    small_cell_floor: int
    mechanism: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        _mapping(self.pattern, "pattern")
        _mapping(self.field, "field")
        _mapping(self.boundary, "boundary")
        _nonnegative_int(self.small_cell_floor, "small_cell_floor")
        if self.mechanism is not None:
            _mapping(self.mechanism, "mechanism")

    def to_mcp_arguments(self) -> JsonObject:
        result: JsonObject = {
            "pattern": _mapping(self.pattern, "pattern"),
            "field": _mapping(self.field, "field"),
            "boundary": _mapping(self.boundary, "boundary"),
            "small_cell_floor": self.small_cell_floor,
        }
        if self.mechanism is not None:
            result["mechanism"] = _mapping(self.mechanism, "mechanism")
        return result


@dataclass(frozen=True)
class EvaluationWorldlineRequest:
    worldline: Mapping[str, Any]
    at: str | None = None

    def to_mcp_arguments(self) -> JsonObject:
        result = {"worldline": _mapping(self.worldline, "worldline")}
        if self.at is not None:
            result["at"] = _timestamp(self.at, "at")
        return result


@dataclass(frozen=True)
class EvaluationReproductionRequest:
    reexecution: Mapping[str, Any]
    biological_claim: str | None = None

    def to_mcp_arguments(self) -> JsonObject:
        result = {"reexecution": _mapping(self.reexecution, "reexecution")}
        if self.biological_claim is not None:
            result["biological_claim"] = _text(self.biological_claim, "biological_claim", 4096)
        return result


@dataclass(frozen=True)
class EvaluationTrajectoryRequest:
    trajectory: Mapping[str, Any]
    step: int | None = None
    horizon: int | None = None

    def __post_init__(self) -> None:
        _mapping(self.trajectory, "trajectory")
        if (self.step is None) != (self.horizon is None):
            raise AuthoringError("step and horizon must be supplied together")
        if self.step is not None:
            _nonnegative_int(self.step, "step")
            if not 1 <= _nonnegative_int(self.horizon or 0, "horizon") <= 1000:
                raise AuthoringError("horizon: expected an integer between 1 and 1000")

    def to_mcp_arguments(self) -> JsonObject:
        result = {"trajectory": _mapping(self.trajectory, "trajectory")}
        if self.step is not None:
            result.update({"step": self.step, "horizon": self.horizon})
        return result


@dataclass(frozen=True)
class ReferenceStandardAuditRequest:
    reference: Mapping[str, Any]
    state: str | None = None

    def to_mcp_arguments(self) -> JsonObject:
        result = {"reference": _mapping(self.reference, "reference")}
        if self.state is not None:
            result["state"] = _text(self.state, "state", 512)
        return result


__all__ = [
    "Admissibility",
    "EvidenceTier",
    "EvaluationReproductionRequest",
    "EvaluationTrajectoryRequest",
    "EvaluationWorldlineRequest",
    "Finding",
    "Independence",
    "Judgement",
    "JudgementBuilder",
    "MissingnessAuditRequest",
    "OracleCombineRequest",
    "OracleManifest",
    "OracleRef",
    "OracleVersion",
    "Position",
    "PositionDistribution",
    "ReferencePanelRequest",
    "ReferenceStandardAuditRequest",
    "ValidityWindow",
]
