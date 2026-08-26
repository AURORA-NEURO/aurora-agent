"""Provider-free claim-integrity fusion for the autonomous brain.

The repository already has excellent, focused evidence contracts: grounding, acquisition,
reconciliation, temporal validity, contradiction review, and reproducibility checks.  A model
still needs one deterministic decision surface that answers a narrower operational question:
"what may this agent rely on for this claim, and what should it acquire next?"

This module is that surface.  It fuses caller-supplied metadata only.  It never dereferences a
source, invokes an LLM, evaluates a biological result, or reads a clock.  Freshness is always
relative to the explicit ``reference_time`` supplied by the caller.  Raw claim text, evidence
values, prompts, locators, and credentials stay caller-owned; only bounded digests and decisions
are retained in the projection.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from datetime import datetime, timezone
import math
from typing import Any, Mapping, Sequence

from .authoring import content_digest
from .autonomous_information_acquisition import (
    AutonomousInformationAcquisitionCandidate,
    AutonomousInformationAcquisitionPlan,
    AutonomousInformationAcquisitionPolicy,
    plan_autonomous_information_acquisition,
)
from .domain_tools import AUTONOMOUS_DOMAIN_NAMES
from .errors import ArgumentError


AUTONOMOUS_CLAIM_INTEGRITY_SCHEMA = "bioprism-python-autonomous-claim-integrity/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA = "bioprism-python-autonomous-claim-integrity-policy/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA = "bioprism-python-autonomous-claim-integrity-claim/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA = "bioprism-python-autonomous-claim-integrity-evidence/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA = "bioprism-python-autonomous-claim-integrity-assessment/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA = "bioprism-python-autonomous-claim-integrity-action/0.1"
AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA = "bioprism-python-autonomous-claim-integrity-acquisition-bridge/0.1"

AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIMS = 128
AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE = 512
AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS = 128
AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIM_LINKS = 32
AUTONOMOUS_CLAIM_INTEGRITY_MAX_MODALITIES = 16
AUTONOMOUS_CLAIM_INTEGRITY_MAX_TEXT_BYTES = 256
AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES = 16_384
AUTONOMOUS_CLAIM_INTEGRITY_MAX_AGE_SECONDS = 31_536_000

AUTONOMOUS_CLAIM_INTEGRITY_STATUSES = (
    "supported",
    "partially_supported",
    "missing",
    "stale",
    "conflicted",
    "contradicted",
    "insufficient_independence",
    "insufficient_modalities",
    "unreproducible",
    "blocked",
)
AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES = (
    "accepted",
    "partial",
    "rejected",
    "stale",
    "failed",
    "reconciliation_required",
)
AUTONOMOUS_CLAIM_INTEGRITY_STANCES = ("support", "contradict", "neutral")
AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY = (
    "reproduced",
    "observed",
    "declared",
    "unverified",
    "failed",
)
AUTONOMOUS_CLAIM_INTEGRITY_TEMPORAL_STATES = (
    "valid",
    "stale",
    "future",
    "not_yet_valid",
    "expired",
)
AUTONOMOUS_CLAIM_INTEGRITY_ACTION_TYPES = (
    "acquire_evidence",
    "acquire_fresh_evidence",
    "acquire_independent_source",
    "acquire_cross_modal_evidence",
    "resolve_contradiction",
    "reproduce_evidence",
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


def _text(name: str, value: Any, maximum: int = AUTONOMOUS_CLAIM_INTEGRITY_MAX_TEXT_BYTES) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise ArgumentError(f"{name} must be bounded non-empty text")
    return value.strip()


def _identifier(name: str, value: Any, maximum: int = AUTONOMOUS_CLAIM_INTEGRITY_MAX_TEXT_BYTES) -> str:
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


def _sequence(name: str, value: Any, maximum: int) -> tuple[Any, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence) or len(value) > maximum:
        raise ArgumentError(f"{name} is outside its bounds")
    return tuple(value)


def _safe_metadata(value: Any, *, name: str = "metadata", depth: int = 0) -> None:
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
    digest = content_digest(dict(value))
    if len(digest.encode("utf-8")) > AUTONOMOUS_CLAIM_INTEGRITY_MAX_METADATA_BYTES:
        raise ArgumentError("metadata digest is outside its bound")
    return digest


def _timestamp(name: str, value: Any) -> str:
    candidate = _text(name, value, 64)
    try:
        parsed = datetime.fromisoformat(candidate.replace("Z", "+00:00"))
    except ValueError as error:
        raise ArgumentError(f"{name} must be an RFC3339 timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ArgumentError(f"{name} must include a timezone")
    return candidate


def _timestamp_seconds(value: str) -> float:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc).timestamp()


def _round(value: float) -> float:
    return round(float(value), 8)


def _domains(name: str, value: Sequence[str]) -> tuple[str, ...]:
    normalized = tuple(_identifier(f"{name}[{index}]", item, 64) for index, item in enumerate(_sequence(name, value, len(AUTONOMOUS_DOMAIN_NAMES))))
    if not normalized:
        raise ArgumentError(f"{name} must contain at least one domain")
    if len(set(normalized)) != len(normalized) or any(item not in AUTONOMOUS_DOMAIN_NAMES for item in normalized):
        raise ArgumentError(f"{name} contains duplicate or unsupported domains")
    return normalized


def _identifiers(name: str, value: Sequence[str], maximum: int) -> tuple[str, ...]:
    normalized = tuple(_identifier(f"{name}[{index}]", item) for index, item in enumerate(_sequence(name, value, maximum)))
    if len(set(normalized)) != len(normalized):
        raise ArgumentError(f"{name} contains duplicate identifiers")
    return normalized


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityPolicy:
    """Explicit decision policy for fusing evidence metadata."""

    max_age_seconds: int = 86_400
    min_reliability: float = 0.5
    min_support: float = 0.5
    require_independent_sources: bool = False
    min_independent_sources: int = 1
    require_cross_modal_agreement: bool = False
    contradiction_veto: bool = True
    require_reproducibility: bool = False
    allow_partial: bool = False
    max_actions: int = 32

    def __post_init__(self) -> None:
        _integer("integrity policy max_age_seconds", self.max_age_seconds, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_AGE_SECONDS)
        _finite("integrity policy min_reliability", self.min_reliability, 0.0, 1.0)
        _finite("integrity policy min_support", self.min_support, 0.0, 1.0)
        for name in ("require_independent_sources", "require_cross_modal_agreement", "contradiction_veto", "require_reproducibility", "allow_partial"):
            if not isinstance(getattr(self, name), bool):
                raise ArgumentError(f"integrity policy {name} must be boolean")
        _integer("integrity policy min_independent_sources", self.min_independent_sources, 1, 16)
        _integer("integrity policy max_actions", self.max_actions, 1, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | None) -> "AutonomousClaimIntegrityPolicy":
        if value is None:
            return cls()
        if not isinstance(value, Mapping):
            raise ArgumentError("integrity policy must be a mapping")
        return cls(
            max_age_seconds=value.get("max_age_seconds", value.get("maxAgeSeconds", 86_400)),
            min_reliability=value.get("min_reliability", value.get("minReliability", 0.5)),
            min_support=value.get("min_support", value.get("minSupport", 0.5)),
            require_independent_sources=value.get("require_independent_sources", value.get("requireIndependentSources", False)),
            min_independent_sources=value.get("min_independent_sources", value.get("minIndependentSources", 1)),
            require_cross_modal_agreement=value.get("require_cross_modal_agreement", value.get("requireCrossModalAgreement", False)),
            contradiction_veto=value.get("contradiction_veto", value.get("contradictionVeto", True)),
            require_reproducibility=value.get("require_reproducibility", value.get("requireReproducibility", False)),
            allow_partial=value.get("allow_partial", value.get("allowPartial", False)),
            max_actions=value.get("max_actions", value.get("maxActions", 32)),
        )

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA,
            "max_age_seconds": self.max_age_seconds,
            "min_reliability": _round(self.min_reliability),
            "min_support": _round(self.min_support),
            "require_independent_sources": self.require_independent_sources,
            "min_independent_sources": self.min_independent_sources,
            "require_cross_modal_agreement": self.require_cross_modal_agreement,
            "contradiction_veto": self.contradiction_veto,
            "require_reproducibility": self.require_reproducibility,
            "allow_partial": self.allow_partial,
            "max_actions": self.max_actions,
        }

    @property
    def policy_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "policy_digest": self.policy_digest,
            "execution": "provider_free_metadata_fusion;no_source_or_provider_dispatch",
            "retention": "metadata_only;raw_claim_and_evidence_values_caller_owned",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityClaim:
    """A digest-bound claim contract; the claim's human text never enters the projection."""

    claim_id: str
    domain: str
    claim_digest: str
    required_support: float = 0.5
    required_independent_sources: int = 1
    required_reproducibility: bool = False
    required_modalities: Sequence[str] = ()
    priority: float = 0.5
    metadata: Mapping[str, Any] = field(default_factory=dict, repr=False, compare=False)

    def __post_init__(self) -> None:
        claim_id = _identifier("integrity claim_id", self.claim_id)
        domain = _identifier("integrity claim domain", self.domain, 64)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("integrity claim domain is unsupported")
        _digest("integrity claim_digest", self.claim_digest)
        _finite("integrity claim required_support", self.required_support, 0.0, 1.0)
        _integer("integrity claim required_independent_sources", self.required_independent_sources, 1, 16)
        if not isinstance(self.required_reproducibility, bool):
            raise ArgumentError("integrity claim required_reproducibility must be boolean")
        modalities = _identifiers("integrity claim required_modalities", self.required_modalities, AUTONOMOUS_CLAIM_INTEGRITY_MAX_MODALITIES)
        _finite("integrity claim priority", self.priority, 0.0, 1.0)
        if not isinstance(self.metadata, Mapping):
            raise ArgumentError("integrity claim metadata must be a mapping")
        metadata = dict(self.metadata)
        _safe_metadata(metadata, name="integrity claim metadata")
        object.__setattr__(self, "claim_id", claim_id)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "claim_digest", self.claim_digest)
        object.__setattr__(self, "required_support", _finite("integrity claim required_support", self.required_support, 0.0, 1.0))
        object.__setattr__(self, "required_independent_sources", self.required_independent_sources)
        object.__setattr__(self, "required_modalities", modalities)
        object.__setattr__(self, "priority", _finite("integrity claim priority", self.priority, 0.0, 1.0))
        object.__setattr__(self, "metadata", metadata)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousClaimIntegrityClaim":
        if not isinstance(value, Mapping):
            raise ArgumentError("integrity claims must be mappings or typed claims")
        return cls(
            claim_id=value.get("claim_id", value.get("claimId")),
            domain=value.get("domain"),
            claim_digest=value.get("claim_digest", value.get("claimDigest")),
            required_support=value.get("required_support", value.get("requiredSupport", 0.5)),
            required_independent_sources=value.get("required_independent_sources", value.get("requiredIndependentSources", 1)),
            required_reproducibility=value.get("required_reproducibility", value.get("requiredReproducibility", False)),
            required_modalities=tuple(value.get("required_modalities", value.get("requiredModalities", ())) or ()),
            priority=value.get("priority", 0.5),
            metadata=value.get("metadata", {}),
        )

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA,
            "claim_id": self.claim_id,
            "domain": self.domain,
            "claim_digest": self.claim_digest,
            "required_support": _round(self.required_support),
            "required_independent_sources": self.required_independent_sources,
            "required_reproducibility": self.required_reproducibility,
            "required_modalities": list(self.required_modalities),
            "priority": _round(self.priority),
            "metadata_digest": _metadata_digest(self.metadata),
        }

    @property
    def claim_contract_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "claim_contract_digest": self.claim_contract_digest,
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityEvidence:
    """A bounded, digest-only evidence observation supplied by a caller or adapter."""

    evidence_id: str
    domain: str
    claim_ids: Sequence[str]
    source_id: str
    evidence_digest: str
    source_digest: str | None = None
    observed_at: str = ""
    valid_from: str | None = None
    valid_until: str | None = None
    reliability: float = 0.5
    support: float = 0.5
    status: str = "accepted"
    stance: str = "support"
    modality: str = "unspecified"
    reproducibility: str = "unverified"
    metadata: Mapping[str, Any] = field(default_factory=dict, repr=False, compare=False)

    def __post_init__(self) -> None:
        evidence_id = _identifier("integrity evidence_id", self.evidence_id)
        domain = _identifier("integrity evidence domain", self.domain, 64)
        if domain not in AUTONOMOUS_DOMAIN_NAMES:
            raise ArgumentError("integrity evidence domain is unsupported")
        claim_ids = _identifiers("integrity evidence claim_ids", self.claim_ids, AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIM_LINKS)
        if not claim_ids:
            raise ArgumentError("integrity evidence claim_ids must not be empty")
        source_id = _identifier("integrity evidence source_id", self.source_id)
        _digest("integrity evidence_digest", self.evidence_digest)
        _digest("integrity source_digest", self.source_digest, allow_none=True)
        observed_at = _timestamp("integrity evidence observed_at", self.observed_at)
        valid_from = None if self.valid_from is None else _timestamp("integrity evidence valid_from", self.valid_from)
        valid_until = None if self.valid_until is None else _timestamp("integrity evidence valid_until", self.valid_until)
        if valid_from is not None and valid_until is not None and _timestamp_seconds(valid_from) >= _timestamp_seconds(valid_until):
            raise ArgumentError("integrity evidence valid_from must precede valid_until")
        _finite("integrity evidence reliability", self.reliability, 0.0, 1.0)
        _finite("integrity evidence support", self.support, 0.0, 1.0)
        if self.status not in AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES:
            raise ArgumentError("integrity evidence status is unsupported")
        if self.stance not in AUTONOMOUS_CLAIM_INTEGRITY_STANCES:
            raise ArgumentError("integrity evidence stance is unsupported")
        modality = _identifier("integrity evidence modality", self.modality)
        if self.reproducibility not in AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY:
            raise ArgumentError("integrity evidence reproducibility is unsupported")
        if not isinstance(self.metadata, Mapping):
            raise ArgumentError("integrity evidence metadata must be a mapping")
        metadata = dict(self.metadata)
        _safe_metadata(metadata, name="integrity evidence metadata")
        object.__setattr__(self, "evidence_id", evidence_id)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "claim_ids", claim_ids)
        object.__setattr__(self, "source_id", source_id)
        object.__setattr__(self, "observed_at", observed_at)
        object.__setattr__(self, "valid_from", valid_from)
        object.__setattr__(self, "valid_until", valid_until)
        object.__setattr__(self, "reliability", _finite("integrity evidence reliability", self.reliability, 0.0, 1.0))
        object.__setattr__(self, "support", _finite("integrity evidence support", self.support, 0.0, 1.0))
        object.__setattr__(self, "modality", modality)
        object.__setattr__(self, "metadata", metadata)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousClaimIntegrityEvidence":
        if not isinstance(value, Mapping):
            raise ArgumentError("integrity evidence must be a mapping or typed evidence")
        return cls(
            evidence_id=value.get("evidence_id", value.get("evidenceId")),
            domain=value.get("domain"),
            claim_ids=tuple(value.get("claim_ids", value.get("claimIds", ())) or ()),
            source_id=value.get("source_id", value.get("sourceId")),
            evidence_digest=value.get("evidence_digest", value.get("evidenceDigest")),
            source_digest=value.get("source_digest", value.get("sourceDigest")),
            observed_at=value.get("observed_at", value.get("observedAt")),
            valid_from=value.get("valid_from", value.get("validFrom")),
            valid_until=value.get("valid_until", value.get("validUntil")),
            reliability=value.get("reliability", 0.5),
            support=value.get("support", 0.5),
            status=value.get("status", "accepted"),
            stance=value.get("stance", "support"),
            modality=value.get("modality", "unspecified"),
            reproducibility=value.get("reproducibility", "unverified"),
            metadata=value.get("metadata", {}),
        )

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA,
            "evidence_id": self.evidence_id,
            "domain": self.domain,
            "claim_ids": list(self.claim_ids),
            "source_id": self.source_id,
            "source_digest": self.source_digest,
            "evidence_digest": self.evidence_digest,
            "observed_at": self.observed_at,
            "valid_from": self.valid_from,
            "valid_until": self.valid_until,
            "reliability": _round(self.reliability),
            "support": _round(self.support),
            "status": self.status,
            "stance": self.stance,
            "modality": self.modality,
            "reproducibility": self.reproducibility,
            "metadata_digest": _metadata_digest(self.metadata),
        }

    @property
    def evidence_contract_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "evidence_contract_digest": self.evidence_contract_digest,
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityEvidenceRow:
    evidence_id: str
    domain: str
    claim_ids: tuple[str, ...]
    status: str
    stance: str
    usable: bool
    temporal_state: str
    source_key: str
    reliability: float
    support: float
    reproducibility: str
    issues: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA,
            "evidence_id": self.evidence_id,
            "domain": self.domain,
            "claim_ids": list(self.claim_ids),
            "status": self.status,
            "stance": self.stance,
            "usable": self.usable,
            "temporal_state": self.temporal_state,
            "source_key": self.source_key,
            "reliability": _round(self.reliability),
            "support": _round(self.support),
            "reproducibility": self.reproducibility,
            "issues": list(self.issues),
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityClaimAssessment:
    claim_id: str
    domain: str
    status: str
    support_score: float
    confidence: float
    supporting_evidence_ids: tuple[str, ...]
    contradicting_evidence_ids: tuple[str, ...]
    usable_evidence_ids: tuple[str, ...]
    independent_source_count: int
    modalities: tuple[str, ...]
    missing_modalities: tuple[str, ...]
    reproducibility: str
    temporal_state: str
    issues: tuple[str, ...]
    next_action_type: str | None
    priority: float

    def to_dict(self) -> dict[str, Any]:
        return {
            "claim_id": self.claim_id,
            "domain": self.domain,
            "status": self.status,
            "support_score": _round(self.support_score),
            "confidence": _round(self.confidence),
            "supporting_evidence_ids": list(self.supporting_evidence_ids),
            "contradicting_evidence_ids": list(self.contradicting_evidence_ids),
            "usable_evidence_ids": list(self.usable_evidence_ids),
            "independent_source_count": self.independent_source_count,
            "modalities": list(self.modalities),
            "missing_modalities": list(self.missing_modalities),
            "reproducibility": self.reproducibility,
            "temporal_state": self.temporal_state,
            "issues": list(self.issues),
            "next_action_type": self.next_action_type,
            "priority": _round(self.priority),
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityAction:
    action_type: str
    domain: str
    claim_ids: tuple[str, ...]
    blocking_evidence_ids: tuple[str, ...]
    reason_codes: tuple[str, ...]
    priority: float
    expected_value: float

    @property
    def action_id(self) -> str:
        return content_digest(self._payload())

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA,
            "action_type": self.action_type,
            "domain": self.domain,
            "claim_ids": list(self.claim_ids),
            "blocking_evidence_ids": list(self.blocking_evidence_ids),
            "reason_codes": list(self.reason_codes),
            "priority": _round(self.priority),
            "expected_value": _round(self.expected_value),
        }

    def to_dict(self) -> dict[str, Any]:
        return {**self._payload(), "action_id": self.action_id, "dispatch": "planning_only;caller_approval_required", "secret_material": "never_returned"}


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityAssessment:
    context_digest: str
    reference_time: str
    policy: AutonomousClaimIntegrityPolicy
    claims: tuple[AutonomousClaimIntegrityClaimAssessment, ...]
    evidence: tuple[AutonomousClaimIntegrityEvidenceRow, ...]
    actions: tuple[AutonomousClaimIntegrityAction, ...]
    omitted_actions: int
    status: str
    summary: Mapping[str, Any]
    prior_assessment_digest: str | None = None
    generation: int = 1

    def __post_init__(self) -> None:
        _digest("integrity assessment context_digest", self.context_digest)
        _timestamp("integrity assessment reference_time", self.reference_time)
        _integer("integrity assessment generation", self.generation, 1, 2_147_483_647)
        _integer("integrity assessment omitted_actions", self.omitted_actions, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS)
        _digest("integrity assessment prior_assessment_digest", self.prior_assessment_digest, allow_none=True)
        if self.status not in ("ready", "partial", "blocked"):
            raise ArgumentError("integrity assessment status is unsupported")
        if not isinstance(self.summary, Mapping):
            raise ArgumentError("integrity assessment summary must be a mapping")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA,
            "context_digest": self.context_digest,
            "reference_time": self.reference_time,
            "policy_digest": self.policy.policy_digest,
            "claims": [claim.to_dict() for claim in self.claims],
            "evidence": [item.to_dict() for item in self.evidence],
            "actions": [action.to_dict() for action in self.actions],
            "omitted_actions": self.omitted_actions,
            "status": self.status,
            "summary": dict(self.summary),
            "prior_assessment_digest": self.prior_assessment_digest,
            "generation": self.generation,
        }

    @property
    def assessment_digest(self) -> str:
        return content_digest(self._payload())

    @property
    def ready(self) -> bool:
        return self.status == "ready"

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "assessment_digest": self.assessment_digest,
            "policy": self.policy.to_dict(),
            "execution": "provider_free_claim_integrity_fusion;no_source_or_provider_dispatch",
            "retention": "metadata_only;claim_text_evidence_values_prompts_locators_credentials_caller_owned",
            "authorization": "actions_are_proposals;acquisition_resolution_and_provider_calls_require_separate_approval",
            "secret_material": "never_returned",
        }


@dataclass(frozen=True, slots=True)
class AutonomousClaimIntegrityAcquisitionBridge:
    """A digest-bound handoff from claim blockers to the reviewed acquisition planner."""

    assessment_digest: str
    action_ids: tuple[str, ...]
    targeted_candidate_ids: tuple[str, ...]
    candidate_action_matches: tuple[Mapping[str, Any], ...]
    acquisition_plan: AutonomousInformationAcquisitionPlan | None
    unmatched_action_count: int
    status: str
    generation: int = 1

    def __post_init__(self) -> None:
        _digest("acquisition bridge assessment_digest", self.assessment_digest)
        _identifiers("acquisition bridge action_ids", self.action_ids, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS)
        _identifiers("acquisition bridge targeted_candidate_ids", self.targeted_candidate_ids, 512)
        _integer("acquisition bridge unmatched_action_count", self.unmatched_action_count, 0, AUTONOMOUS_CLAIM_INTEGRITY_MAX_ACTIONS)
        _integer("acquisition bridge generation", self.generation, 1, 2_147_483_647)
        if self.status not in {"planned", "no_action_required", "blocked"}:
            raise ArgumentError("acquisition bridge status is unsupported")
        if not isinstance(self.candidate_action_matches, Sequence):
            raise ArgumentError("acquisition bridge candidate_action_matches must be a sequence")
        for index, match in enumerate(self.candidate_action_matches):
            if not isinstance(match, Mapping):
                raise ArgumentError(f"acquisition bridge match {index} must be a mapping")
            _safe_metadata(dict(match), name=f"acquisition bridge match {index}")
        if self.status == "planned" and self.acquisition_plan is None:
            raise ArgumentError("planned acquisition bridge requires an acquisition plan")
        if self.status == "no_action_required" and self.unmatched_action_count != 0:
            raise ArgumentError("no-action acquisition bridge cannot have unmatched actions")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA,
            "assessment_digest": self.assessment_digest,
            "action_ids": list(self.action_ids),
            "targeted_candidate_ids": list(self.targeted_candidate_ids),
            "candidate_action_matches": [dict(match) for match in self.candidate_action_matches],
            "acquisition_plan_digest": None if self.acquisition_plan is None else self.acquisition_plan.plan_digest,
            "unmatched_action_count": self.unmatched_action_count,
            "status": self.status,
            "generation": self.generation,
        }

    @property
    def bridge_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "bridge_digest": self.bridge_digest,
            "actions_are": "proposals_only;source_dispatch_requires_reviewed_evidence_approval",
            "acquisition_plan": None if self.acquisition_plan is None else self.acquisition_plan.to_dict(),
            "retention": "metadata_only;raw_claim_text_evidence_values_and_source_payloads_caller_owned",
            "secret_material": "never_returned",
        }


def _claim_value(value: AutonomousClaimIntegrityClaim | Mapping[str, Any]) -> AutonomousClaimIntegrityClaim:
    return value if isinstance(value, AutonomousClaimIntegrityClaim) else AutonomousClaimIntegrityClaim.from_mapping(value)


def _evidence_value(value: AutonomousClaimIntegrityEvidence | Mapping[str, Any]) -> AutonomousClaimIntegrityEvidence:
    return value if isinstance(value, AutonomousClaimIntegrityEvidence) else AutonomousClaimIntegrityEvidence.from_mapping(value)


def _temporal_state(item: AutonomousClaimIntegrityEvidence, reference_seconds: float, max_age_seconds: int) -> str:
    observed = _timestamp_seconds(item.observed_at)
    if observed > reference_seconds:
        return "future"
    if item.valid_from is not None and reference_seconds < _timestamp_seconds(item.valid_from):
        return "not_yet_valid"
    if item.valid_until is not None and reference_seconds >= _timestamp_seconds(item.valid_until):
        return "expired"
    if reference_seconds - observed > max_age_seconds or item.status == "stale":
        return "stale"
    return "valid"


def _evidence_row(item: AutonomousClaimIntegrityEvidence, reference_seconds: float, policy: AutonomousClaimIntegrityPolicy, claim_ids: set[str]) -> AutonomousClaimIntegrityEvidenceRow:
    temporal = _temporal_state(item, reference_seconds, policy.max_age_seconds)
    issues: list[str] = []
    if temporal != "valid":
        issues.append(temporal)
    if item.status not in ("accepted", "partial"):
        issues.append(item.status)
    if item.status == "partial" and not policy.allow_partial:
        issues.append("partial_not_allowed")
    if item.reliability < policy.min_reliability:
        issues.append("below_reliability_floor")
    if item.support < policy.min_support:
        issues.append("below_support_floor")
    orphaned = sorted(set(item.claim_ids).difference(claim_ids))
    if orphaned:
        issues.append("orphan_claim_reference")
    usable = temporal == "valid" and item.status == "accepted" and item.reliability >= policy.min_reliability and item.support >= policy.min_support
    if policy.allow_partial and item.status == "partial" and temporal == "valid" and item.reliability >= policy.min_reliability and item.support >= policy.min_support:
        usable = True
    return AutonomousClaimIntegrityEvidenceRow(
        evidence_id=item.evidence_id,
        domain=item.domain,
        claim_ids=item.claim_ids,
        status=item.status,
        stance=item.stance,
        usable=usable,
        temporal_state=temporal,
        source_key=item.source_digest or item.source_id,
        reliability=item.reliability,
        support=item.support,
        reproducibility=item.reproducibility,
        issues=tuple(sorted(set(issues))),
    )


def _action_type(assessment: AutonomousClaimIntegrityClaimAssessment) -> str | None:
    if assessment.status == "supported":
        return None
    if assessment.status in {"conflicted", "contradicted"}:
        return "resolve_contradiction"
    if assessment.status == "stale":
        return "acquire_fresh_evidence"
    if assessment.status == "insufficient_independence":
        return "acquire_independent_source"
    if assessment.status == "insufficient_modalities":
        return "acquire_cross_modal_evidence"
    if assessment.status == "unreproducible":
        return "reproduce_evidence"
    return "acquire_evidence"


def _capability_matches(candidate: AutonomousInformationAcquisitionCandidate, action: AutonomousClaimIntegrityAction) -> tuple[bool, str]:
    capability = candidate.capability.lower().replace("-", "_").replace(" ", "_")
    action_token = action.action_type.replace("acquire_", "")
    generic_evidence = action.action_type == "acquire_evidence" and "evidence" in capability
    direct = action_token in capability or capability in action.action_type or generic_evidence
    metadata_claims = candidate.metadata.get("claim_ids", ()) if isinstance(candidate.metadata, Mapping) else ()
    if isinstance(metadata_claims, Sequence) and not isinstance(metadata_claims, (str, bytes, bytearray)):
        claim_match = bool(set(str(item) for item in metadata_claims).intersection(action.claim_ids))
    else:
        claim_match = False
    if claim_match and direct:
        return True, "claim_and_capability"
    if claim_match:
        return True, "claim_and_domain"
    if direct:
        return True, "capability"
    return True, "domain"


def plan_autonomous_claim_integrity_acquisition(
    assessment: AutonomousClaimIntegrityAssessment,
    *,
    candidates: Sequence[AutonomousInformationAcquisitionCandidate | Mapping[str, Any]],
    policy: AutonomousInformationAcquisitionPolicy | Mapping[str, Any] | None = None,
    requested_domains: Sequence[str] | None = None,
) -> AutonomousClaimIntegrityAcquisitionBridge:
    """Compile integrity blockers into the existing reviewed acquisition planner.

    The bridge only changes bounded candidate priority signals and records why each candidate was
    promoted.  It does not fabricate evidence, add a source, dispatch an adapter, or authorize a
    provider.  A caller can inspect the returned bridge and then pass its acquisition plan through
    the existing reviewed evidence execution boundary.
    """

    if not isinstance(assessment, AutonomousClaimIntegrityAssessment):
        raise ArgumentError("integrity acquisition planning requires a typed assessment")
    validate_autonomous_claim_integrity(assessment)
    normalized_candidates = tuple(
        item if isinstance(item, AutonomousInformationAcquisitionCandidate) else AutonomousInformationAcquisitionCandidate.from_mapping(item)
        for item in _sequence("integrity acquisition candidates", candidates, 512)
    )
    candidate_ids = [item.candidate_id for item in normalized_candidates]
    if len(set(candidate_ids)) != len(candidate_ids):
        raise ArgumentError("integrity acquisition candidates contain duplicate ids")
    actions = tuple(assessment.actions)
    if not actions:
        return AutonomousClaimIntegrityAcquisitionBridge(
            assessment_digest=assessment.assessment_digest,
            action_ids=(),
            targeted_candidate_ids=(),
            candidate_action_matches=(),
            acquisition_plan=None,
            unmatched_action_count=0,
            status="no_action_required",
            generation=assessment.generation,
        )
    if not normalized_candidates:
        return AutonomousClaimIntegrityAcquisitionBridge(
            assessment_digest=assessment.assessment_digest,
            action_ids=tuple(action.action_id for action in actions),
            targeted_candidate_ids=(),
            candidate_action_matches=(),
            acquisition_plan=None,
            unmatched_action_count=len(actions),
            status="blocked",
            generation=assessment.generation,
        )
    action_domains = {action.domain for action in actions}
    selected_domains = (
        tuple(domain for domain in AUTONOMOUS_DOMAIN_NAMES if domain in action_domains)
        if requested_domains is None
        else _domains("integrity acquisition requested_domains", requested_domains)
    )
    action_matches: list[dict[str, Any]] = []
    adjusted: list[AutonomousInformationAcquisitionCandidate] = []
    targeted_ids: list[str] = []
    matched_action_ids: set[str] = set()
    for candidate in normalized_candidates:
        matches: list[tuple[AutonomousClaimIntegrityAction, str]] = []
        for action in actions:
            if candidate.domain != action.domain:
                continue
            matched, strength = _capability_matches(candidate, action)
            if matched:
                matches.append((action, strength))
        if not matches:
            adjusted.append(candidate)
            continue
        targeted_ids.append(candidate.candidate_id)
        for action, _strength in matches:
            matched_action_ids.add(action.action_id)
        strength_rank = {"domain": 1, "capability": 2, "claim_and_domain": 3, "claim_and_capability": 4}
        strongest = max(matches, key=lambda row: (strength_rank[row[1]], row[0].priority, row[0].action_id))
        boost = min(0.4, 0.10 + 0.05 * strength_rank[strongest[1]] + 0.10 * strongest[0].priority)
        adjusted.append(replace(
            candidate,
            information_gain=min(1.0, candidate.information_gain + boost),
            uncertainty_reduction=min(1.0, candidate.uncertainty_reduction + boost),
            coverage=min(1.0, candidate.coverage + boost * 0.5),
            priority=min(1.0, candidate.priority + boost),
        ))
        action_matches.append({
            "candidate_id": candidate.candidate_id,
            "action_ids": sorted(action.action_id for action, _strength in matches),
            "action_types": sorted({action.action_type for action, _strength in matches}),
            "match_strength": strongest[1],
            "priority_boost": _round(boost),
        })
    selected_domains = tuple(domain for domain in selected_domains if domain in action_domains)
    acquisition_plan = plan_autonomous_information_acquisition(
        task_digest=assessment.context_digest,
        candidates=tuple(adjusted),
        requested_domains=selected_domains,
        policy=policy,
    )
    unmatched = len(set(action.action_id for action in actions).difference(matched_action_ids))
    status = "planned" if acquisition_plan.selected else "blocked"
    return AutonomousClaimIntegrityAcquisitionBridge(
        assessment_digest=assessment.assessment_digest,
        action_ids=tuple(action.action_id for action in actions),
        targeted_candidate_ids=tuple(targeted_ids),
        candidate_action_matches=tuple(action_matches),
        acquisition_plan=acquisition_plan,
        unmatched_action_count=unmatched,
        status=status,
        generation=assessment.generation,
    )


def validate_autonomous_claim_integrity_acquisition_bridge(value: AutonomousClaimIntegrityAcquisitionBridge) -> AutonomousClaimIntegrityAcquisitionBridge:
    if not isinstance(value, AutonomousClaimIntegrityAcquisitionBridge):
        raise ArgumentError("integrity acquisition bridge validation requires a typed bridge")
    if content_digest(value._payload()) != value.bridge_digest:
        raise ArgumentError("integrity acquisition bridge digest does not match its fields")
    return value


def assess_autonomous_claim_integrity(
    *,
    context_digest: str,
    claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
    evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
    reference_time: str,
    policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
    prior_assessment_digest: str | None = None,
    generation: int = 1,
) -> AutonomousClaimIntegrityAssessment:
    """Fuse bounded evidence metadata into claim decisions and next actions."""

    _digest("integrity context_digest", context_digest)
    reference_time = _timestamp("integrity reference_time", reference_time)
    normalized_claims = tuple(_claim_value(value) for value in _sequence("integrity claims", claims, AUTONOMOUS_CLAIM_INTEGRITY_MAX_CLAIMS))
    normalized_evidence = tuple(_evidence_value(value) for value in _sequence("integrity evidence", evidence, AUTONOMOUS_CLAIM_INTEGRITY_MAX_EVIDENCE))
    if not normalized_claims:
        raise ArgumentError("integrity claims must contain at least one claim")
    claim_ids = [claim.claim_id for claim in normalized_claims]
    evidence_ids = [item.evidence_id for item in normalized_evidence]
    if len(set(claim_ids)) != len(claim_ids):
        raise ArgumentError("integrity claims contain duplicate ids")
    if len(set(evidence_ids)) != len(evidence_ids):
        raise ArgumentError("integrity evidence contains duplicate ids")
    selected_policy = policy if isinstance(policy, AutonomousClaimIntegrityPolicy) else AutonomousClaimIntegrityPolicy.from_mapping(policy)
    reference_seconds = _timestamp_seconds(reference_time)
    rows = tuple(_evidence_row(item, reference_seconds, selected_policy, set(claim_ids)) for item in normalized_evidence)
    by_id = {item.evidence_id: item for item in normalized_evidence}
    row_by_id = {row.evidence_id: row for row in rows}
    assessments: list[AutonomousClaimIntegrityClaimAssessment] = []

    for claim in normalized_claims:
        linked = [item for item in normalized_evidence if claim.claim_id in item.claim_ids]
        usable = [item for item in linked if row_by_id[item.evidence_id].usable and item.domain == claim.domain]
        domain_mismatch = [item for item in linked if item.domain != claim.domain]
        supporting = [item for item in usable if item.stance == "support"]
        contradicting = [item for item in usable if item.stance == "contradict"]
        usable_ids = tuple(item.evidence_id for item in usable)
        supporting_ids = tuple(item.evidence_id for item in supporting)
        contradicting_ids = tuple(item.evidence_id for item in contradicting)
        sources = {item.source_digest or item.source_id for item in supporting}
        modalities = tuple(sorted({item.modality for item in supporting}))
        required_modalities = set(claim.required_modalities)
        if selected_policy.require_cross_modal_agreement and not required_modalities:
            required_modalities = {"__at_least_two_modalities__"}
        missing_modalities = tuple(sorted(required_modalities.difference(modalities))) if "__at_least_two_modalities__" not in required_modalities else ()
        modal_shortfall = len(modalities) < 2 if "__at_least_two_modalities__" in required_modalities else bool(missing_modalities)
        support_score = min(1.0, sum(item.support * item.reliability * (0.5 if item.status == "partial" else 1.0) for item in supporting))
        required_sources = max(claim.required_independent_sources, selected_policy.min_independent_sources if selected_policy.require_independent_sources else 1)
        temporal_states = {_temporal_state(item, reference_seconds, selected_policy.max_age_seconds) for item in linked}
        if usable:
            temporal_state = "valid"
        elif temporal_states and temporal_states.issubset({"stale"}):
            temporal_state = "stale"
        elif temporal_states and temporal_states.intersection({"future", "not_yet_valid", "expired"}):
            temporal_state = "invalid"
        else:
            temporal_state = "unknown"
        reproduced = any(item.reproducibility == "reproduced" for item in supporting)
        observed_reproducibility = "reproduced" if reproduced else "unreproduced" if supporting else "unknown"
        issues: list[str] = []
        if not linked:
            issues.append("no_evidence")
        if domain_mismatch:
            issues.append("domain_mismatch")
        if not usable and linked:
            if temporal_state == "stale":
                issues.append("stale")
            elif temporal_state == "invalid":
                issues.append("temporal_firewall")
            if all(item.status in {"rejected", "failed", "reconciliation_required"} for item in linked):
                issues.append("evidence_not_accepted")
        if support_score < claim.required_support:
            issues.append("insufficient_support")
        if contradicting:
            issues.append("contradiction")
        if len(sources) < required_sources:
            issues.append("insufficient_independence")
        if modal_shortfall:
            issues.append("missing_modality")
        requires_reproduction = claim.required_reproducibility or selected_policy.require_reproducibility
        if requires_reproduction and supporting and not reproduced:
            issues.append("unreproduced")

        if not linked:
            status = "missing"
        elif contradicting and selected_policy.contradiction_veto:
            status = "conflicted" if supporting else "contradicted"
        elif not usable and temporal_state == "stale":
            status = "stale"
        elif not supporting:
            status = "blocked"
        elif requires_reproduction and not reproduced:
            status = "unreproducible"
        elif len(sources) < required_sources:
            status = "insufficient_independence"
        elif modal_shortfall:
            status = "insufficient_modalities"
        elif support_score < claim.required_support:
            status = "partially_supported"
        else:
            status = "supported"
        quality = min(1.0, support_score / max(claim.required_support, 1e-12)) if supporting else 0.0
        independence = min(1.0, len(sources) / max(required_sources, 1)) if supporting else 0.0
        consistency = 0.0 if contradicting and selected_policy.contradiction_veto else 1.0
        modality_factor = 0.0 if modal_shortfall else 1.0
        confidence = _round(quality * independence * consistency * modality_factor)
        next_action_type = _action_type(AutonomousClaimIntegrityClaimAssessment(
            claim.claim_id, claim.domain, status, support_score, confidence, supporting_ids, contradicting_ids,
            usable_ids, len(sources), modalities, missing_modalities, observed_reproducibility, temporal_state,
            tuple(sorted(set(issues))), None, claim.priority,
        ))
        assessments.append(AutonomousClaimIntegrityClaimAssessment(
            claim_id=claim.claim_id,
            domain=claim.domain,
            status=status,
            support_score=_round(support_score),
            confidence=confidence,
            supporting_evidence_ids=supporting_ids,
            contradicting_evidence_ids=contradicting_ids,
            usable_evidence_ids=usable_ids,
            independent_source_count=len(sources),
            modalities=modalities,
            missing_modalities=missing_modalities,
            reproducibility=observed_reproducibility,
            temporal_state=temporal_state,
            issues=tuple(sorted(set(issues))),
            next_action_type=next_action_type,
            priority=_round(claim.priority),
        ))

    action_candidates: list[AutonomousClaimIntegrityAction] = []
    claim_by_id = {claim.claim_id: claim for claim in normalized_claims}
    for item in assessments:
        action_type = item.next_action_type
        if action_type is None:
            continue
        claim = claim_by_id[item.claim_id]
        action_candidates.append(AutonomousClaimIntegrityAction(
            action_type=action_type,
            domain=item.domain,
            claim_ids=(item.claim_id,),
            blocking_evidence_ids=tuple(sorted(set(item.contradicting_evidence_ids + item.supporting_evidence_ids))),
            reason_codes=item.issues,
            priority=_round(min(1.0, claim.priority + (1.0 - item.confidence) * 0.5)),
            expected_value=_round(min(1.0, max(0.0, claim.required_support - item.support_score) + 0.1 * len(item.issues))),
        ))
    action_candidates.sort(key=lambda item: (-item.priority, -item.expected_value, item.domain, item.claim_ids[0], item.action_type))
    actions = tuple(action_candidates[: selected_policy.max_actions])
    omitted_actions = len(action_candidates) - len(actions)
    status_counts = {status: sum(item.status == status for item in assessments) for status in AUTONOMOUS_CLAIM_INTEGRITY_STATUSES}
    evidence_counts = {
        "total": len(rows),
        "usable": sum(row.usable for row in rows),
        "stale": sum(row.temporal_state == "stale" for row in rows),
        "future": sum(row.temporal_state == "future" for row in rows),
        "expired": sum(row.temporal_state == "expired" for row in rows),
        "rejected_or_failed": sum(row.status in {"rejected", "failed", "reconciliation_required"} for row in rows),
    }
    domains = tuple(sorted({claim.domain for claim in normalized_claims}))
    summary = {
        "claim_count": len(assessments),
        "evidence_count": evidence_counts,
        "status_counts": status_counts,
        "supported_claim_count": status_counts["supported"],
        "action_count": len(actions),
        "omitted_action_count": omitted_actions,
        "domains": list(domains),
        "temporal_firewall": "explicit_reference_time;future_and_expired_observations_excluded",
        "source_independence": "source_digest_or_source_id_unique_supporting_sources",
        "contradiction_policy": "veto" if selected_policy.contradiction_veto else "reported_without_veto",
    }
    if status_counts["supported"] == len(assessments):
        overall = "ready"
    elif status_counts["supported"] == 0:
        overall = "blocked"
    else:
        overall = "partial"
    result = AutonomousClaimIntegrityAssessment(
        context_digest=context_digest,
        reference_time=reference_time,
        policy=selected_policy,
        claims=tuple(assessments),
        evidence=rows,
        actions=actions,
        omitted_actions=omitted_actions,
        status=overall,
        summary=summary,
        prior_assessment_digest=prior_assessment_digest,
        generation=generation,
    )
    if len(str(result.to_dict()).encode("utf-8")) > 2_000_000:
        raise ArgumentError("integrity assessment exceeds its byte bound")
    return result


def reassess_autonomous_claim_integrity(
    previous: AutonomousClaimIntegrityAssessment,
    *,
    claims: Sequence[AutonomousClaimIntegrityClaim | Mapping[str, Any]],
    evidence: Sequence[AutonomousClaimIntegrityEvidence | Mapping[str, Any]],
    reference_time: str,
    policy: AutonomousClaimIntegrityPolicy | Mapping[str, Any] | None = None,
) -> AutonomousClaimIntegrityAssessment:
    """Recompute after new value-only evidence while fencing the previous decision chain."""

    if not isinstance(previous, AutonomousClaimIntegrityAssessment):
        raise ArgumentError("integrity reassessment requires a typed previous assessment")
    validate_autonomous_claim_integrity(previous)
    return assess_autonomous_claim_integrity(
        context_digest=previous.context_digest,
        claims=claims,
        evidence=evidence,
        reference_time=reference_time,
        policy=policy if policy is not None else previous.policy,
        prior_assessment_digest=previous.assessment_digest,
        generation=previous.generation + 1,
    )


def validate_autonomous_claim_integrity(value: AutonomousClaimIntegrityAssessment) -> AutonomousClaimIntegrityAssessment:
    """Validate a typed snapshot before a caller resumes or persists the decision loop."""

    if not isinstance(value, AutonomousClaimIntegrityAssessment):
        raise ArgumentError("integrity validation requires a typed assessment")
    expected = content_digest(value._payload())
    if expected != value.assessment_digest:
        raise ArgumentError("integrity assessment digest does not match its fields")
    return value


def validate_autonomous_claim_integrity_snapshot(value: Mapping[str, Any]) -> dict[str, Any]:
    """Validate the digest-bearing JSON projection without rehydrating raw caller values."""

    if not isinstance(value, Mapping):
        raise ArgumentError("integrity snapshot must be a mapping")
    provided = value.get("assessment_digest")
    _digest("integrity snapshot assessment_digest", provided)
    fields = (
        "schema", "context_digest", "reference_time", "policy_digest", "claims", "evidence", "actions",
        "omitted_actions", "status", "summary", "prior_assessment_digest", "generation",
    )
    descriptor = {key: value[key] for key in fields if key in value}
    if set(descriptor) != set(fields):
        raise ArgumentError("integrity snapshot is missing digest-bound fields")
    if content_digest(descriptor) != provided:
        raise ArgumentError("integrity snapshot digest does not match its fields")
    return dict(value)


__all__ = [
    "AUTONOMOUS_CLAIM_INTEGRITY_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_POLICY_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_CLAIM_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_ASSESSMENT_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_ACTION_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_ACQUISITION_BRIDGE_SCHEMA",
    "AUTONOMOUS_CLAIM_INTEGRITY_STATUSES",
    "AUTONOMOUS_CLAIM_INTEGRITY_EVIDENCE_STATUSES",
    "AUTONOMOUS_CLAIM_INTEGRITY_STANCES",
    "AUTONOMOUS_CLAIM_INTEGRITY_REPRODUCIBILITY",
    "AUTONOMOUS_CLAIM_INTEGRITY_TEMPORAL_STATES",
    "AUTONOMOUS_CLAIM_INTEGRITY_ACTION_TYPES",
    "AutonomousClaimIntegrityPolicy",
    "AutonomousClaimIntegrityClaim",
    "AutonomousClaimIntegrityEvidence",
    "AutonomousClaimIntegrityEvidenceRow",
    "AutonomousClaimIntegrityClaimAssessment",
    "AutonomousClaimIntegrityAction",
    "AutonomousClaimIntegrityAssessment",
    "AutonomousClaimIntegrityAcquisitionBridge",
    "assess_autonomous_claim_integrity",
    "reassess_autonomous_claim_integrity",
    "plan_autonomous_claim_integrity_acquisition",
    "validate_autonomous_claim_integrity",
    "validate_autonomous_claim_integrity_snapshot",
    "validate_autonomous_claim_integrity_acquisition_bridge",
]
