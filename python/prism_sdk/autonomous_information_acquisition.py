"""Provider-free information-acquisition planning for the autonomous brain.

The model-selection and evidence runtimes already know how to execute a reviewed request.  This
module supplies the missing decision boundary before that work queue: given a bounded catalogue of
caller-owned acquisition candidates, choose the next observations that buy the most context per
unit cost while preserving explicit uncertainty, freshness, risk, conflict, approval, and
all-domain coverage signals.

The planner is deliberately not a source router, evaluator, authorization oracle, or truth
engine.  It never calls a provider or source and never retains task text, prompts, credentials,
locators, evidence values, or tool arguments.  Its output can be passed to the existing reviewed
evidence planner; source approval and evidence truth remain separate boundaries.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import math
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_INFORMATION_ACQUISITION_SCHEMA = "bioprism-python-autonomous-information-acquisition/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_POLICY_SCHEMA = "bioprism-python-autonomous-information-acquisition-policy/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_SCHEMA = "bioprism-python-autonomous-information-acquisition-candidate/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_SELECTION_SCHEMA = "bioprism-python-autonomous-information-acquisition-selection/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_OMISSION_SCHEMA = "bioprism-python-autonomous-information-acquisition-omission/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_PLAN_SCHEMA = "bioprism-python-autonomous-information-acquisition-plan/0.1"
AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_SCHEMA = "bioprism-python-autonomous-information-acquisition-observation/0.1"

AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES = 512
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED = 64
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_DEPENDENCIES = 16
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_OBSERVATIONS = 512
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_IDENTIFIER_BYTES = 256
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_TEXT_BYTES = 2_048
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_PLAN_BYTES = 1_000_000
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS = 86_400_000
AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST = 1_000_000.0
AUTONOMOUS_INFORMATION_ACQUISITION_EPSILON = 1e-12

AUTONOMOUS_INFORMATION_ACQUISITION_STATUSES = (
    "ready",
    "partial",
    "blocked",
    "empty",
    "review_required",
)
AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_STATUSES = (
    "available",
    "partial",
    "stale",
    "unavailable",
    "requires_approval",
    "conflicted",
)
AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_STATUSES = (
    "accepted",
    "partial",
    "rejected",
    "stale",
    "failed",
    "reconciliation_required",
)

_SECRET_MARKERS = frozenset(
    {
        "apikey",
        "authorization",
        "bearer",
        "credential",
        "credentials",
        "password",
        "privatekey",
        "secret",
        "secretkey",
        "token",
        "accesstoken",
        "refreshtoken",
        "clientsecret",
        "gsk",
        "sk",
    }
)

_DEFAULT_WEIGHTS = {
    "information_gain": 0.30,
    "uncertainty_reduction": 0.25,
    "reliability": 0.15,
    "freshness": 0.10,
    "coverage": 0.10,
    "priority": 0.10,
    "cost": 0.10,
    "latency": 0.05,
    "risk": 0.20,
    "conflict": 0.15,
}
_WEIGHT_NAMES = tuple(_DEFAULT_WEIGHTS)


def _text(name: str, value: Any, maximum: int = AUTONOMOUS_INFORMATION_ACQUISITION_MAX_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} must be bounded non-empty text")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = AUTONOMOUS_INFORMATION_ACQUISITION_MAX_IDENTIFIER_BYTES) -> str:
    candidate = _text(name, value, maximum)
    if any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:+-/ " for character in candidate):
        raise ArgumentError(f"{name} contains unsupported identifier characters")
    return candidate


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if value is None and allow_none:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _finite(name: str, value: Any, minimum: float, maximum: float) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise ArgumentError(f"{name} must be finite")
    number = float(value)
    if number < minimum or number > maximum:
        raise ArgumentError(f"{name} is outside its bounds")
    return number


def _integer(name: str, value: Any, minimum: int, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum or value > maximum:
        raise ArgumentError(f"{name} is outside its integer bounds")
    return value


def _digest_or_none(name: str, value: Any) -> str | None:
    return _digest(name, value, allow_none=True)


def _safe_metadata(value: Any, *, name: str = "metadata", depth: int = 0) -> None:
    """Reject credential-shaped metadata before it can influence a plan digest."""

    if depth > 8:
        raise ArgumentError(f"{name} is too deeply nested")
    if isinstance(value, Mapping):
        if len(value) > 64:
            raise ArgumentError(f"{name} contains too many fields")
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip() or "\x00" in key:
                raise ArgumentError(f"{name} contains an invalid key")
            marker = "".join(character for character in key.lower() if character.isalnum())
            if marker in _SECRET_MARKERS or "secret" in marker or "credential" in marker or "token" in marker:
                raise ArgumentError(f"{name}.{key} is credential-shaped metadata")
            _safe_metadata(child, name=f"{name}.{key}", depth=depth + 1)
        return
    if isinstance(value, (list, tuple)):
        if len(value) > 128:
            raise ArgumentError(f"{name} contains too many entries")
        for index, child in enumerate(value):
            _safe_metadata(child, name=f"{name}[{index}]", depth=depth + 1)
        return
    if value is None or isinstance(value, (str, bool, int)):
        return
    if isinstance(value, float) and math.isfinite(value):
        return
    raise ArgumentError(f"{name} contains unsupported metadata")


def _metadata_digest(value: Mapping[str, Any]) -> str:
    _safe_metadata(value)
    return content_digest(dict(value))


def _domains(name: str, value: Sequence[str] | None, *, default_all: bool = False) -> tuple[str, ...]:
    if value is None:
        return tuple(AUTONOMOUS_DOMAIN_NAMES) if default_all else ()
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a sequence of domains")
    normalized = tuple(_identifier(f"{name}[{index}]", item, 64) for index, item in enumerate(value))
    if not 1 <= len(normalized) <= len(AUTONOMOUS_DOMAIN_NAMES):
        raise ArgumentError(f"{name} must contain between 1 and {len(AUTONOMOUS_DOMAIN_NAMES)} domains")
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicate domains")
    if any(item not in AUTONOMOUS_DOMAIN_NAMES for item in normalized):
        raise ArgumentError(f"{name} contains an unsupported autonomous domain")
    return normalized


def _identifiers(name: str, value: Sequence[str] | None, maximum: int) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounds")
    normalized = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(value))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicate identifiers")
    return normalized


def _round(value: float) -> float:
    return round(float(value), 8)


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionPolicy:
    """Caller-owned utility policy for bounded context acquisition."""

    max_cost: float = 1.0
    max_items: int = 8
    max_latency_ms: int = 300_000
    min_score: float = 0.0
    min_reliability: float = 0.0
    require_domain_coverage: bool = False
    allow_partial: bool = False
    allow_stale: bool = False
    allow_unavailable: bool = False
    exploration: float = 0.15
    coverage_bonus: float = 0.20
    weights: Mapping[str, float] = field(default_factory=lambda: dict(_DEFAULT_WEIGHTS))

    def __post_init__(self) -> None:
        max_cost = _finite("information policy max_cost", self.max_cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST)
        if max_cost <= 0.0:
            raise ArgumentError("information policy max_cost must be positive")
        max_items = _integer("information policy max_items", self.max_items, 1, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED)
        max_latency = _integer("information policy max_latency_ms", self.max_latency_ms, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS)
        min_score = _finite("information policy min_score", self.min_score, -10.0, 10.0)
        min_reliability = _finite("information policy min_reliability", self.min_reliability, 0.0, 1.0)
        for name in ("require_domain_coverage", "allow_partial", "allow_stale", "allow_unavailable"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"information policy {name} must be boolean")
        exploration = _finite("information policy exploration", self.exploration, 0.0, 2.0)
        coverage_bonus = _finite("information policy coverage_bonus", self.coverage_bonus, 0.0, 2.0)
        if not isinstance(self.weights, Mapping) or set(self.weights) != set(_WEIGHT_NAMES):
            raise ArgumentError("information policy weights must contain exactly the known score dimensions")
        weights = {name: _finite(f"information policy weight {name}", self.weights[name], 0.0, 4.0) for name in _WEIGHT_NAMES}
        if sum(weights.values()) <= 0.0:
            raise ArgumentError("information policy weights must contain a positive value")
        object.__setattr__(self, "max_cost", max_cost)
        object.__setattr__(self, "max_items", max_items)
        object.__setattr__(self, "max_latency_ms", max_latency)
        object.__setattr__(self, "min_score", min_score)
        object.__setattr__(self, "min_reliability", min_reliability)
        object.__setattr__(self, "exploration", exploration)
        object.__setattr__(self, "coverage_bonus", coverage_bonus)
        object.__setattr__(self, "weights", weights)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_POLICY_SCHEMA,
            "max_cost": _round(self.max_cost),
            "max_items": self.max_items,
            "max_latency_ms": self.max_latency_ms,
            "min_score": _round(self.min_score),
            "min_reliability": _round(self.min_reliability),
            "require_domain_coverage": self.require_domain_coverage,
            "allow_partial": self.allow_partial,
            "allow_stale": self.allow_stale,
            "allow_unavailable": self.allow_unavailable,
            "exploration": _round(self.exploration),
            "coverage_bonus": _round(self.coverage_bonus),
            "weights": {name: _round(self.weights[name]) for name in _WEIGHT_NAMES},
        }

    @property
    def policy_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "policy_digest": self.policy_digest,
            "execution": "provider_free_candidate_prioritization;no_source_dispatch",
            "retention": "metadata_only;candidate_values_and_source_payloads_caller_owned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionCandidate:
    """One source/experiment/context candidate described only by bounded metadata."""

    candidate_id: str
    domain: str
    capability: str
    source_id: str
    information_gain: float
    uncertainty_reduction: float
    reliability: float
    freshness: float
    coverage: float
    cost: float
    latency_ms: int
    risk: float
    conflict_risk: float
    priority: float = 0.5
    status: str = "available"
    depends_on: tuple[str, ...] = ()
    source_digest: str | None = None
    metadata: Mapping[str, Any] = field(default_factory=dict, repr=False, compare=False)

    def __post_init__(self) -> None:
        candidate_id = _identifier("information candidate_id", self.candidate_id)
        domain = _identifier("information candidate domain", self.domain, 64)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("information candidate domain is unsupported")
        capability = _identifier("information candidate capability", self.capability)
        source_id = _identifier("information candidate source_id", self.source_id)
        for name in ("information_gain", "uncertainty_reduction", "reliability", "freshness", "coverage", "risk", "conflict_risk", "priority"):
            _finite(f"information candidate {name}", getattr(self, name), 0.0, 1.0)
        cost = _finite("information candidate cost", self.cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST)
        if cost <= 0.0:
            raise ArgumentError("information candidate cost must be positive")
        latency_ms = _integer("information candidate latency_ms", self.latency_ms, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS)
        if self.status not in AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_STATUSES:
            raise ArgumentError("information candidate status is unsupported")
        depends_on = _identifiers("information candidate depends_on", self.depends_on, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_DEPENDENCIES)
        if candidate_id in depends_on:
            raise ArgumentError("information candidate cannot depend on itself")
        source_digest = _digest_or_none("information candidate source_digest", self.source_digest)
        if not isinstance(self.metadata, Mapping):
            raise ArgumentError("information candidate metadata must be a mapping")
        metadata = dict(self.metadata)
        _safe_metadata(metadata, name="information candidate metadata")
        object.__setattr__(self, "candidate_id", candidate_id)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "capability", capability)
        object.__setattr__(self, "source_id", source_id)
        object.__setattr__(self, "information_gain", _finite("information candidate information_gain", self.information_gain, 0.0, 1.0))
        object.__setattr__(self, "uncertainty_reduction", _finite("information candidate uncertainty_reduction", self.uncertainty_reduction, 0.0, 1.0))
        object.__setattr__(self, "reliability", _finite("information candidate reliability", self.reliability, 0.0, 1.0))
        object.__setattr__(self, "freshness", _finite("information candidate freshness", self.freshness, 0.0, 1.0))
        object.__setattr__(self, "coverage", _finite("information candidate coverage", self.coverage, 0.0, 1.0))
        object.__setattr__(self, "cost", _finite("information candidate cost", self.cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST))
        object.__setattr__(self, "latency_ms", latency_ms)
        object.__setattr__(self, "risk", _finite("information candidate risk", self.risk, 0.0, 1.0))
        object.__setattr__(self, "conflict_risk", _finite("information candidate conflict_risk", self.conflict_risk, 0.0, 1.0))
        object.__setattr__(self, "priority", _finite("information candidate priority", self.priority, 0.0, 1.0))
        object.__setattr__(self, "depends_on", depends_on)
        object.__setattr__(self, "source_digest", source_digest)
        object.__setattr__(self, "metadata", metadata)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousInformationAcquisitionCandidate":
        if not isinstance(value, Mapping):
            raise ArgumentError("information candidate must be a mapping")
        return cls(
            candidate_id=value.get("candidate_id", value.get("candidateId")),
            domain=value.get("domain"),
            capability=value.get("capability"),
            source_id=value.get("source_id", value.get("sourceId")),
            information_gain=value.get("information_gain", value.get("informationGain")),
            uncertainty_reduction=value.get("uncertainty_reduction", value.get("uncertaintyReduction")),
            reliability=value.get("reliability"),
            freshness=value.get("freshness"),
            coverage=value.get("coverage"),
            cost=value.get("cost"),
            latency_ms=value.get("latency_ms", value.get("latencyMs")),
            risk=value.get("risk"),
            conflict_risk=value.get("conflict_risk", value.get("conflictRisk")),
            priority=value.get("priority", 0.5),
            status=value.get("status", "available"),
            depends_on=tuple(value.get("depends_on", value.get("dependsOn", ())) or ()),
            source_digest=value.get("source_digest", value.get("sourceDigest")),
            metadata=value.get("metadata", {}),
        )

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_CANDIDATE_SCHEMA,
            "candidate_id": self.candidate_id,
            "domain": self.domain,
            "capability": self.capability,
            "source_id": self.source_id,
            "information_gain": _round(self.information_gain),
            "uncertainty_reduction": _round(self.uncertainty_reduction),
            "reliability": _round(self.reliability),
            "freshness": _round(self.freshness),
            "coverage": _round(self.coverage),
            "cost": _round(self.cost),
            "latency_ms": self.latency_ms,
            "risk": _round(self.risk),
            "conflict_risk": _round(self.conflict_risk),
            "priority": _round(self.priority),
            "status": self.status,
            "depends_on": list(self.depends_on),
            "source_digest": self.source_digest,
            "metadata_digest": _metadata_digest(self.metadata),
        }

    @property
    def candidate_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "candidate_digest": self.candidate_digest,
            "retention": "metadata_only;candidate_values_and_source_payloads_caller_owned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionObservation:
    """Value-only result metadata used to replan without replaying a source."""

    candidate_id: str
    status: str
    observed_information_gain: float | None = None
    observed_uncertainty_reduction: float | None = None
    actual_cost: float | None = None
    actual_latency_ms: int | None = None
    value_digest: str | None = None
    evaluator_digest: str | None = None

    def __post_init__(self) -> None:
        _identifier("information observation candidate_id", self.candidate_id)
        if self.status not in AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_STATUSES:
            raise ArgumentError("information observation status is unsupported")
        for name in ("observed_information_gain", "observed_uncertainty_reduction"):
            value = getattr(self, name)
            if value is not None:
                _finite(f"information observation {name}", value, 0.0, 1.0)
        if self.actual_cost is not None:
            _finite("information observation actual_cost", self.actual_cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST)
        if self.actual_latency_ms is not None:
            _integer("information observation actual_latency_ms", self.actual_latency_ms, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS)
        _digest_or_none("information observation value_digest", self.value_digest)
        _digest_or_none("information observation evaluator_digest", self.evaluator_digest)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousInformationAcquisitionObservation":
        if not isinstance(value, Mapping):
            raise ArgumentError("information observation must be a mapping")
        return cls(
            candidate_id=value.get("candidate_id", value.get("candidateId")),
            status=value.get("status"),
            observed_information_gain=value.get("observed_information_gain", value.get("observedInformationGain")),
            observed_uncertainty_reduction=value.get("observed_uncertainty_reduction", value.get("observedUncertaintyReduction")),
            actual_cost=value.get("actual_cost", value.get("actualCost")),
            actual_latency_ms=value.get("actual_latency_ms", value.get("actualLatencyMs")),
            value_digest=value.get("value_digest", value.get("valueDigest")),
            evaluator_digest=value.get("evaluator_digest", value.get("evaluatorDigest")),
        )

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_OBSERVATION_SCHEMA,
            "candidate_id": self.candidate_id,
            "status": self.status,
            "observed_information_gain": None if self.observed_information_gain is None else _round(self.observed_information_gain),
            "observed_uncertainty_reduction": None if self.observed_uncertainty_reduction is None else _round(self.observed_uncertainty_reduction),
            "actual_cost": None if self.actual_cost is None else _round(self.actual_cost),
            "actual_latency_ms": self.actual_latency_ms,
            "value_digest": self.value_digest,
            "evaluator_digest": self.evaluator_digest,
        }
        return {**payload, "observation_digest": content_digest(payload), "retention": "value_only_observation_metadata", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionSelection:
    candidate_id: str
    domain: str
    capability: str
    source_id: str
    candidate_digest: str
    rank: int
    score: float
    utility_per_cost: float
    projected_information_gain: float
    projected_uncertainty_reduction: float
    projected_cost: float
    projected_latency_ms: int
    selection_reason: str

    def __post_init__(self) -> None:
        _identifier("information selection candidate_id", self.candidate_id)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("information selection domain is unsupported")
        _identifier("information selection capability", self.capability)
        _identifier("information selection source_id", self.source_id)
        _digest("information selection candidate_digest", self.candidate_digest)
        _integer("information selection rank", self.rank, 1, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED)
        _finite("information selection score", self.score, -100.0, 100.0)
        _finite("information selection utility_per_cost", self.utility_per_cost, -100_000.0, 100_000.0)
        _finite("information selection projected_information_gain", self.projected_information_gain, 0.0, 1.0)
        _finite("information selection projected_uncertainty_reduction", self.projected_uncertainty_reduction, 0.0, 1.0)
        _finite("information selection projected_cost", self.projected_cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST)
        _integer("information selection projected_latency_ms", self.projected_latency_ms, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS)
        _identifier("information selection selection_reason", self.selection_reason, 128)

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_SELECTION_SCHEMA,
            "candidate_id": self.candidate_id,
            "domain": self.domain,
            "capability": self.capability,
            "source_id": self.source_id,
            "candidate_digest": self.candidate_digest,
            "rank": self.rank,
            "score": _round(self.score),
            "utility_per_cost": _round(self.utility_per_cost),
            "projected_information_gain": _round(self.projected_information_gain),
            "projected_uncertainty_reduction": _round(self.projected_uncertainty_reduction),
            "projected_cost": _round(self.projected_cost),
            "projected_latency_ms": self.projected_latency_ms,
            "selection_reason": self.selection_reason,
        }
        return {**payload, "retention": "metadata_only;source_dispatch_requires_review", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionOmission:
    candidate_id: str
    domain: str
    candidate_digest: str
    reason: str
    score: float | None = None

    def __post_init__(self) -> None:
        _identifier("information omission candidate_id", self.candidate_id)
        if self.domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("information omission domain is unsupported")
        _digest("information omission candidate_digest", self.candidate_digest)
        _identifier("information omission reason", self.reason, 128)
        if self.score is not None:
            _finite("information omission score", self.score, -100.0, 100.0)

    def to_dict(self) -> dict[str, Any]:
        payload = {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_OMISSION_SCHEMA,
            "candidate_id": self.candidate_id,
            "domain": self.domain,
            "candidate_digest": self.candidate_digest,
            "reason": self.reason,
            "score": None if self.score is None else _round(self.score),
        }
        return {**payload, "retention": "metadata_only;omitted_candidate_payload_not_retained", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousInformationAcquisitionPlan:
    task_digest: str
    route_digest: str | None
    requested_domains: tuple[str, ...]
    selected: tuple[AutonomousInformationAcquisitionSelection, ...]
    omissions: tuple[AutonomousInformationAcquisitionOmission, ...]
    policy: AutonomousInformationAcquisitionPolicy
    candidate_count: int
    consumed_cost: float
    consumed_latency_ms: int
    status: str
    missing_domains: tuple[str, ...]
    prior_plan_digest: str | None = None
    observations_digest: str | None = None
    generation: int = 1
    plan_digest: str | None = None

    def __post_init__(self) -> None:
        _digest("information plan task_digest", self.task_digest)
        _digest_or_none("information plan route_digest", self.route_digest)
        requested = _domains("information plan requested_domains", self.requested_domains)
        if not isinstance(self.selected, Sequence) or len(self.selected) > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED:
            raise ArgumentError("information plan selections are outside their bounds")
        if any(not isinstance(item, AutonomousInformationAcquisitionSelection) for item in self.selected):
            raise ArgumentError("information plan selections are malformed")
        if len({item.candidate_id for item in self.selected}) != len(self.selected):
            raise ArgumentError("information plan selections contain duplicate candidates")
        if tuple(item.rank for item in self.selected) != tuple(range(1, len(self.selected) + 1)):
            raise ArgumentError("information plan selection ranks must be contiguous")
        if not isinstance(self.omissions, Sequence) or len(self.omissions) > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES:
            raise ArgumentError("information plan omissions are outside their bounds")
        if any(not isinstance(item, AutonomousInformationAcquisitionOmission) for item in self.omissions):
            raise ArgumentError("information plan omissions are malformed")
        if len({item.candidate_id for item in self.omissions}) != len(self.omissions):
            raise ArgumentError("information plan omissions contain duplicate candidates")
        if set(item.candidate_id for item in self.selected) & set(item.candidate_id for item in self.omissions):
            raise ArgumentError("information plan candidate cannot be both selected and omitted")
        if _integer("information plan candidate_count", self.candidate_count, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES) != len(self.selected) + len(self.omissions):
            raise ArgumentError("information plan candidate_count does not reconcile selected and omitted candidates")
        consumed_cost = _finite("information plan consumed_cost", self.consumed_cost, 0.0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_COST)
        consumed_latency = _integer("information plan consumed_latency_ms", self.consumed_latency_ms, 0, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_LATENCY_MS * AUTONOMOUS_INFORMATION_ACQUISITION_MAX_SELECTED)
        if consumed_cost > self.policy.max_cost + 1e-8:
            raise ArgumentError("information plan consumed cost exceeds policy")
        if self.status not in AUTONOMOUS_INFORMATION_ACQUISITION_STATUSES:
            raise ArgumentError("information plan status is unsupported")
        missing = _domains("information plan missing_domains", self.missing_domains) if self.missing_domains else ()
        if any(domain not in requested for domain in missing):
            raise ArgumentError("information plan missing domain is outside requested domains")
        _digest_or_none("information plan prior_plan_digest", self.prior_plan_digest)
        _digest_or_none("information plan observations_digest", self.observations_digest)
        generation = _integer("information plan generation", self.generation, 1, 2_147_483_647)
        payload = self._payload()
        expected = content_digest(payload)
        if self.plan_digest is not None and self.plan_digest != expected:
            raise ArgumentError("information plan digest does not match its fields")
        object.__setattr__(self, "requested_domains", requested)
        object.__setattr__(self, "consumed_cost", consumed_cost)
        object.__setattr__(self, "consumed_latency_ms", consumed_latency)
        object.__setattr__(self, "generation", generation)
        object.__setattr__(self, "plan_digest", expected)

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_INFORMATION_ACQUISITION_PLAN_SCHEMA,
            "task_digest": self.task_digest,
            "route_digest": self.route_digest,
            "requested_domains": list(self.requested_domains),
            "selected": [item.to_dict() for item in self.selected],
            "omissions": [item.to_dict() for item in self.omissions],
            "policy_digest": self.policy.policy_digest,
            "candidate_count": self.candidate_count,
            "consumed_cost": _round(self.consumed_cost),
            "consumed_latency_ms": self.consumed_latency_ms,
            "status": self.status,
            "missing_domains": list(self.missing_domains),
            "prior_plan_digest": self.prior_plan_digest,
            "observations_digest": self.observations_digest,
            "generation": self.generation,
        }

    @property
    def selected_domains(self) -> tuple[str, ...]:
        selected = {item.domain for item in self.selected}
        return tuple(domain for domain in self.requested_domains if domain in selected)

    @property
    def coverage_ratio(self) -> float:
        return 0.0 if not self.requested_domains else len(self.selected_domains) / len(self.requested_domains)

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "plan_digest": self.plan_digest,
            "policy": self.policy.to_dict(),
            "selected_domains": list(self.selected_domains),
            "coverage_ratio": _round(self.coverage_ratio),
            "remaining_cost": _round(max(0.0, self.policy.max_cost - self.consumed_cost)),
            "remaining_items": self.policy.max_items - len(self.selected),
            "execution": "planning_only;source_dispatch_requires_reviewed_evidence_boundary",
            "retention": "metadata_only;task_text_prompts_source_values_credentials_and_locators_caller_owned",
            "secret_material": "never_returned",
        }


def _normalize_candidates(candidates: Sequence[AutonomousInformationAcquisitionCandidate | Mapping[str, Any]]) -> tuple[AutonomousInformationAcquisitionCandidate, ...]:
    if isinstance(candidates, (str, bytes, bytearray)) or not isinstance(candidates, Sequence):
        raise ArgumentError("information candidates must be a sequence")
    if not 1 <= len(candidates) <= AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES:
        raise ArgumentError("information candidates must contain between 1 and 512 entries")
    normalized = tuple(item if isinstance(item, AutonomousInformationAcquisitionCandidate) else AutonomousInformationAcquisitionCandidate.from_mapping(item) for item in candidates)
    ids = [item.candidate_id for item in normalized]
    if len(set(ids)) != len(ids):
        raise ArgumentError("information candidates contain duplicate ids")
    return normalized


def _normalize_policy(policy: AutonomousInformationAcquisitionPolicy | Mapping[str, Any] | None) -> AutonomousInformationAcquisitionPolicy:
    if policy is None:
        return AutonomousInformationAcquisitionPolicy()
    if isinstance(policy, AutonomousInformationAcquisitionPolicy):
        return policy
    if not isinstance(policy, Mapping):
        raise ArgumentError("information acquisition policy must be typed or a mapping")
    return AutonomousInformationAcquisitionPolicy(
        max_cost=policy.get("max_cost", policy.get("maxCost", 1.0)),
        max_items=policy.get("max_items", policy.get("maxItems", 8)),
        max_latency_ms=policy.get("max_latency_ms", policy.get("maxLatencyMs", 300_000)),
        min_score=policy.get("min_score", policy.get("minScore", 0.0)),
        min_reliability=policy.get("min_reliability", policy.get("minReliability", 0.0)),
        require_domain_coverage=policy.get("require_domain_coverage", policy.get("requireDomainCoverage", False)),
        allow_partial=policy.get("allow_partial", policy.get("allowPartial", False)),
        allow_stale=policy.get("allow_stale", policy.get("allowStale", False)),
        allow_unavailable=policy.get("allow_unavailable", policy.get("allowUnavailable", False)),
        exploration=policy.get("exploration", 0.15),
        coverage_bonus=policy.get("coverage_bonus", policy.get("coverageBonus", 0.20)),
        weights=policy.get("weights", dict(_DEFAULT_WEIGHTS)),
    )


def _normalize_observations(observations: Sequence[AutonomousInformationAcquisitionObservation | Mapping[str, Any]]) -> tuple[AutonomousInformationAcquisitionObservation, ...]:
    if isinstance(observations, (str, bytes, bytearray)) or not isinstance(observations, Sequence) or len(observations) > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_OBSERVATIONS:
        raise ArgumentError("information observations are outside their bounds")
    normalized = tuple(item if isinstance(item, AutonomousInformationAcquisitionObservation) else AutonomousInformationAcquisitionObservation.from_mapping(item) for item in observations)
    if len({item.candidate_id for item in normalized}) != len(normalized):
        raise ArgumentError("information observations contain duplicate candidate ids")
    return normalized


def _score(candidate: AutonomousInformationAcquisitionCandidate, policy: AutonomousInformationAcquisitionPolicy, *, domain_missing: bool, observation_count: int = 0) -> tuple[float, float]:
    weights = policy.weights
    latency_ratio = 1.0 if policy.max_latency_ms == 0 and candidate.latency_ms > 0 else (0.0 if policy.max_latency_ms == 0 else min(1.0, candidate.latency_ms / policy.max_latency_ms))
    cost_ratio = min(1.0, candidate.cost / policy.max_cost)
    value = (
        weights["information_gain"] * candidate.information_gain
        + weights["uncertainty_reduction"] * candidate.uncertainty_reduction
        + weights["reliability"] * candidate.reliability
        + weights["freshness"] * candidate.freshness
        + weights["coverage"] * candidate.coverage
        + weights["priority"] * candidate.priority
    )
    penalties = (
        weights["cost"] * cost_ratio
        + weights["latency"] * latency_ratio
        + weights["risk"] * candidate.risk
        + weights["conflict"] * candidate.conflict_risk
    )
    exploration = policy.exploration / math.sqrt(1.0 + observation_count)
    if observation_count == 0:
        exploration *= 1.0 - 0.5 * candidate.reliability
    score = value + (policy.coverage_bonus if domain_missing else 0.0) + exploration - penalties
    return _round(score), _round(score / max(candidate.cost, AUTONOMOUS_INFORMATION_ACQUISITION_EPSILON))


def _candidate_omission(candidate: AutonomousInformationAcquisitionCandidate, reason: str, score: float | None = None) -> AutonomousInformationAcquisitionOmission:
    return AutonomousInformationAcquisitionOmission(candidate.candidate_id, candidate.domain, candidate.candidate_digest, reason, score)


def _candidate_with(candidate: AutonomousInformationAcquisitionCandidate, **updates: Any) -> AutonomousInformationAcquisitionCandidate:
    """Clone a slots dataclass without depending on a private ``__dict__``."""

    values: dict[str, Any] = {
        "candidate_id": candidate.candidate_id,
        "domain": candidate.domain,
        "capability": candidate.capability,
        "source_id": candidate.source_id,
        "information_gain": candidate.information_gain,
        "uncertainty_reduction": candidate.uncertainty_reduction,
        "reliability": candidate.reliability,
        "freshness": candidate.freshness,
        "coverage": candidate.coverage,
        "cost": candidate.cost,
        "latency_ms": candidate.latency_ms,
        "risk": candidate.risk,
        "conflict_risk": candidate.conflict_risk,
        "priority": candidate.priority,
        "status": candidate.status,
        "depends_on": candidate.depends_on,
        "source_digest": candidate.source_digest,
        "metadata": candidate.metadata,
    }
    values.update(updates)
    return AutonomousInformationAcquisitionCandidate(**values)


def _plan(
    *,
    task_digest: str,
    route_digest: str | None,
    candidates: Sequence[AutonomousInformationAcquisitionCandidate | Mapping[str, Any]],
    requested_domains: Sequence[str] | None,
    policy: AutonomousInformationAcquisitionPolicy | Mapping[str, Any] | None,
    satisfied_candidate_ids: Sequence[str] = (),
    prior_plan_digest: str | None = None,
    observations: Sequence[AutonomousInformationAcquisitionObservation | Mapping[str, Any]] = (),
    generation: int = 1,
) -> AutonomousInformationAcquisitionPlan:
    task_digest = _digest("information task_digest", task_digest)  # type: ignore[assignment]
    route_digest = _digest_or_none("information route_digest", route_digest)
    normalized_candidates = _normalize_candidates(candidates)
    normalized_policy = _normalize_policy(policy)
    domains = _domains("information requested_domains", requested_domains, default_all=requested_domains is None)
    satisfied = set(_identifiers("information satisfied_candidate_ids", satisfied_candidate_ids, AUTONOMOUS_INFORMATION_ACQUISITION_MAX_CANDIDATES))
    normalized_observations = _normalize_observations(observations)
    observation_counts: dict[str, int] = {}
    for observation in normalized_observations:
        observation_counts[observation.candidate_id] = observation_counts.get(observation.candidate_id, 0) + 1
    by_id = {item.candidate_id: item for item in normalized_candidates}
    selected_ids: set[str] = set()
    selected_domains: set[str] = set()
    selected: list[AutonomousInformationAcquisitionSelection] = []
    omissions: dict[str, AutonomousInformationAcquisitionOmission] = {}
    consumed_cost = 0.0
    consumed_latency = 0

    def eligible(candidate: AutonomousInformationAcquisitionCandidate) -> str | None:
        if candidate.domain not in domains:
            return "domain_not_requested"
        if candidate.reliability < normalized_policy.min_reliability:
            return "below_reliability_floor"
        if candidate.latency_ms > normalized_policy.max_latency_ms:
            return "latency_budget_exceeded"
        if candidate.status == "partial" and not normalized_policy.allow_partial:
            return "partial_not_allowed"
        if candidate.status == "stale" and not normalized_policy.allow_stale:
            return "stale_not_allowed"
        if candidate.status == "unavailable" and not normalized_policy.allow_unavailable:
            return "unavailable"
        if candidate.status in {"requires_approval", "conflicted"}:
            return "approval_or_conflict_review_required"
        if any(dependency not in satisfied and dependency not in selected_ids for dependency in candidate.depends_on):
            return "dependency_unavailable"
        return None

    remaining = {item.candidate_id for item in normalized_candidates}
    while remaining and len(selected) < normalized_policy.max_items:
        eligible_rows: list[tuple[float, float, AutonomousInformationAcquisitionCandidate]] = []
        for candidate in (by_id[item] for item in sorted(remaining)):
            reason = eligible(candidate)
            if reason is not None:
                omissions.setdefault(candidate.candidate_id, _candidate_omission(candidate, reason))
                continue
            score, ratio = _score(candidate, normalized_policy, domain_missing=candidate.domain not in selected_domains, observation_count=observation_counts.get(candidate.candidate_id, 0))
            eligible_rows.append((ratio, score, candidate))
        if not eligible_rows:
            break
        eligible_rows.sort(key=lambda row: (-row[0], -row[1], row[2].domain, row[2].candidate_id))
        ratio, score, candidate = eligible_rows[0]
        if score < normalized_policy.min_score:
            for _, blocked_score, blocked in eligible_rows:
                omissions.setdefault(blocked.candidate_id, _candidate_omission(blocked, "below_score_floor", blocked_score))
            break
        if consumed_cost + candidate.cost > normalized_policy.max_cost + 1e-8:
            omissions.setdefault(candidate.candidate_id, _candidate_omission(candidate, "budget_exceeded", score))
            remaining.remove(candidate.candidate_id)
            continue
        omissions.pop(candidate.candidate_id, None)
        selected.append(
            AutonomousInformationAcquisitionSelection(
                candidate_id=candidate.candidate_id,
                domain=candidate.domain,
                capability=candidate.capability,
                source_id=candidate.source_id,
                candidate_digest=candidate.candidate_digest,
                rank=len(selected) + 1,
                score=score,
                utility_per_cost=ratio,
                projected_information_gain=candidate.information_gain,
                projected_uncertainty_reduction=candidate.uncertainty_reduction,
                projected_cost=candidate.cost,
                projected_latency_ms=candidate.latency_ms,
                selection_reason="domain_coverage_priority" if candidate.domain not in selected_domains else "utility_per_cost",
            )
        )
        selected_ids.add(candidate.candidate_id)
        selected_domains.add(candidate.domain)
        consumed_cost += candidate.cost
        consumed_latency += candidate.latency_ms
        remaining.remove(candidate.candidate_id)

    for candidate in normalized_candidates:
        if candidate.candidate_id in selected_ids or candidate.candidate_id in omissions:
            continue
        reason = "max_items_reached" if len(selected) >= normalized_policy.max_items else "dependency_unavailable"
        omissions[candidate.candidate_id] = _candidate_omission(candidate, reason)
    missing_domains = tuple(domain for domain in domains if domain not in selected_domains)
    if not selected:
        status = "blocked" if omissions and all(item.reason in {"unavailable", "stale_not_allowed", "partial_not_allowed", "approval_or_conflict_review_required", "below_reliability_floor", "latency_budget_exceeded", "dependency_unavailable", "domain_not_requested"} for item in omissions.values()) else "empty"
    elif normalized_policy.require_domain_coverage and missing_domains:
        status = "partial"
    elif len(selected) < min(normalized_policy.max_items, len(normalized_candidates)) and any(item.reason in {"budget_exceeded", "below_score_floor", "max_items_reached"} for item in omissions.values()):
        status = "partial"
    else:
        status = "ready"
    if normalized_policy.require_domain_coverage and missing_domains and not selected:
        status = "blocked"
    observation_digest = content_digest([item.to_dict() for item in normalized_observations]) if normalized_observations else None
    plan = AutonomousInformationAcquisitionPlan(
        task_digest=task_digest,
        route_digest=route_digest,
        requested_domains=domains,
        selected=tuple(selected),
        omissions=tuple(omissions[item.candidate_id] for item in normalized_candidates if item.candidate_id in omissions),
        policy=normalized_policy,
        candidate_count=len(normalized_candidates),
        consumed_cost=consumed_cost,
        consumed_latency_ms=consumed_latency,
        status=status,
        missing_domains=missing_domains,
        prior_plan_digest=_digest_or_none("information prior_plan_digest", prior_plan_digest),
        observations_digest=observation_digest,
        generation=_integer("information plan generation", generation, 1, 2_147_483_647),
    )
    if len(str(plan.to_dict()).encode("utf-8")) > AUTONOMOUS_INFORMATION_ACQUISITION_MAX_PLAN_BYTES:
        raise ArgumentError("information acquisition plan exceeds its byte bound")
    return plan


def plan_autonomous_information_acquisition(
    *,
    task_digest: str,
    candidates: Sequence[AutonomousInformationAcquisitionCandidate | Mapping[str, Any]],
    requested_domains: Sequence[str] | None = None,
    route_digest: str | None = None,
    policy: AutonomousInformationAcquisitionPolicy | Mapping[str, Any] | None = None,
    satisfied_candidate_ids: Sequence[str] = (),
) -> AutonomousInformationAcquisitionPlan:
    """Rank the next bounded context acquisitions without contacting any external system."""

    return _plan(
        task_digest=task_digest,
        route_digest=route_digest,
        candidates=candidates,
        requested_domains=requested_domains,
        policy=policy,
        satisfied_candidate_ids=satisfied_candidate_ids,
    )


def replan_autonomous_information_acquisition(
    previous_plan: AutonomousInformationAcquisitionPlan,
    *,
    candidates: Sequence[AutonomousInformationAcquisitionCandidate | Mapping[str, Any]],
    observations: Sequence[AutonomousInformationAcquisitionObservation | Mapping[str, Any]],
    policy: AutonomousInformationAcquisitionPolicy | Mapping[str, Any] | None = None,
    satisfied_candidate_ids: Sequence[str] = (),
) -> AutonomousInformationAcquisitionPlan:
    """Reprioritize from value-only observations; never reacquire or replay a source implicitly."""

    if not isinstance(previous_plan, AutonomousInformationAcquisitionPlan):
        raise ArgumentError("information replan requires a typed previous plan")
    normalized_candidates = list(_normalize_candidates(candidates))
    normalized_observations = _normalize_observations(observations)
    by_id = {item.candidate_id: item for item in normalized_candidates}
    if any(item.candidate_id not in by_id for item in normalized_observations):
        raise ArgumentError("information observation references an unknown candidate")
    selected_digest_by_id = {item.candidate_id: item.candidate_digest for item in previous_plan.selected}
    for candidate_id, expected_digest in selected_digest_by_id.items():
        current = by_id.get(candidate_id)
        if current is not None and current.candidate_digest != expected_digest:
            raise ArgumentError(f"information candidate {candidate_id} changed since the previous plan")
    observed_by_id = {item.candidate_id: item for item in normalized_observations}
    adjusted: list[AutonomousInformationAcquisitionCandidate] = []
    for candidate in normalized_candidates:
        observation = observed_by_id.get(candidate.candidate_id)
        if observation is None:
            adjusted.append(candidate)
            continue
        if observation.status in {"rejected", "failed", "reconciliation_required"}:
            adjusted.append(_candidate_with(candidate, status="unavailable", reliability=max(0.0, candidate.reliability * 0.5)))
            continue
        gain = candidate.information_gain if observation.observed_information_gain is None else observation.observed_information_gain
        uncertainty = candidate.uncertainty_reduction if observation.observed_uncertainty_reduction is None else observation.observed_uncertainty_reduction
        reliability_delta = 0.10 if observation.status == "accepted" else -0.05
        adjusted.append(_candidate_with(candidate, information_gain=gain, uncertainty_reduction=uncertainty, reliability=min(1.0, max(0.0, candidate.reliability + reliability_delta)), status="available" if observation.status == "accepted" else "partial"))
    return _plan(
        task_digest=previous_plan.task_digest,
        route_digest=previous_plan.route_digest,
        candidates=adjusted,
        requested_domains=previous_plan.requested_domains,
        policy=policy or previous_plan.policy,
        satisfied_candidate_ids=satisfied_candidate_ids,
        prior_plan_digest=previous_plan.plan_digest,
        observations=normalized_observations,
        generation=previous_plan.generation + 1,
    )


def validate_autonomous_information_acquisition_plan(value: AutonomousInformationAcquisitionPlan | Mapping[str, Any]) -> AutonomousInformationAcquisitionPlan:
    """Validate a typed plan boundary; mappings must be recompiled by the caller with candidates."""

    if not isinstance(value, AutonomousInformationAcquisitionPlan):
        raise ArgumentError("information acquisition plan validation requires a typed plan; recompile mappings with candidates")
    # Re-run the constructor-level digest and bounds checks without exposing mutable internals.
    return AutonomousInformationAcquisitionPlan(**{field_name: getattr(value, field_name) for field_name in (
        "task_digest", "route_digest", "requested_domains", "selected", "omissions", "policy", "candidate_count", "consumed_cost", "consumed_latency_ms", "status", "missing_domains", "prior_plan_digest", "observations_digest", "generation", "plan_digest"
    )})
