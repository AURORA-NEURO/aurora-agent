"""Typed request models for cross-domain evidence-conditioned capability audits.

The Rust MCP kernel owns the scientific and release decisions. These models make the input
boundary explicit in Python: evidence status is never confused with a score, claim prerequisites
are named as dimensions, and optional nested audits remain visible instead of being silently
discarded.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Mapping, Sequence

from .errors import ArgumentError


EVIDENCE_DIMENSIONS = (
    "evidence_grounding",
    "information_acquisition",
    "resource_efficiency",
    "temporal_validity",
    "cross_modal_consistency",
    "causal_identification",
    "reproducibility",
    "translation_maturity",
    "multi_agent_coordination",
)
EVIDENCE_STATUSES = (
    "observed",
    "reproduced",
    "declared",
    "missing",
    "blocked",
    "not_applicable",
)
_RESERVED_EVIDENCE_FIELDS = frozenset({"id", "dimension", "status", "domain"})


class EvidenceStatus(str, Enum):
    OBSERVED = "observed"
    REPRODUCED = "reproduced"
    DECLARED = "declared"
    MISSING = "missing"
    BLOCKED = "blocked"
    NOT_APPLICABLE = "not_applicable"


def _text(name: str, value: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    return value


def _mapping(name: str, value: Mapping[str, Any] | None) -> dict[str, Any] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


def _sequence(name: str, value: Sequence[Any], maximum: int) -> tuple[Any, ...]:
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be a sequence")
    if len(value) > maximum:
        raise ArgumentError(f"{name} may contain at most {maximum} items")
    return tuple(value)


def _status(value: EvidenceStatus | str) -> str:
    if isinstance(value, EvidenceStatus):
        return value.value
    if not isinstance(value, str) or value not in EVIDENCE_STATUSES:
        raise ArgumentError(f"status must be one of {EVIDENCE_STATUSES!r}")
    return value


def _dimension(value: str) -> str:
    if not isinstance(value, str) or value not in EVIDENCE_DIMENSIONS:
        raise ArgumentError(f"dimension must be one of {EVIDENCE_DIMENSIONS!r}")
    return value


@dataclass(frozen=True)
class EvidenceItem:
    """One evidence row and its dimension-specific supporting fields."""

    id: str
    dimension: str
    status: EvidenceStatus | str
    domain: str = "unspecified"
    support: Mapping[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        _text("evidence id", self.id)
        _dimension(self.dimension)
        normalized_status = _status(self.status)
        domain = self.domain if self.domain else "unspecified"
        _text("evidence domain", domain)
        if not isinstance(self.support, Mapping):
            raise ArgumentError("evidence support must be a mapping")
        reserved = sorted(_RESERVED_EVIDENCE_FIELDS.intersection(self.support))
        if reserved:
            raise ArgumentError(f"evidence support cannot override reserved fields: {reserved}")
        object.__setattr__(self, "status", normalized_status)
        object.__setattr__(self, "domain", domain)
        object.__setattr__(self, "support", dict(self.support))

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "EvidenceItem":
        if not isinstance(value, Mapping):
            raise ArgumentError("evidence items must be mappings or EvidenceItem values")
        required = {"id", "dimension", "status"}
        missing = sorted(required.difference(value))
        if missing:
            raise ArgumentError(f"evidence item is missing required fields: {missing}")
        support = dict(value)
        item_id = support.pop("id")
        dimension = support.pop("dimension")
        status = support.pop("status")
        domain = support.pop("domain", "unspecified")
        return cls(item_id, dimension, status, domain, support)

    def to_dict(self) -> dict[str, Any]:
        return {
            **dict(self.support),
            "id": self.id,
            "dimension": self.dimension,
            "status": self.status,
            "domain": self.domain,
        }


@dataclass(frozen=True)
class ClaimRequest:
    """A named claim and the evidence dimensions it explicitly requires."""

    id: str
    claim: str
    requires: Sequence[str]
    allow_declared: bool = False

    def __post_init__(self) -> None:
        _text("claim id", self.id)
        _text("claim", self.claim)
        required = _sequence("claim requires", self.requires, len(EVIDENCE_DIMENSIONS))
        if not required:
            raise ArgumentError("claim requires must contain at least one evidence dimension")
        normalized = tuple(_dimension(dimension) for dimension in required)
        if len(set(normalized)) != len(normalized):
            raise ArgumentError("claim requires must not repeat an evidence dimension")
        if not isinstance(self.allow_declared, bool):
            raise ArgumentError("allow_declared must be a boolean")
        object.__setattr__(self, "requires", normalized)

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "ClaimRequest":
        if not isinstance(value, Mapping):
            raise ArgumentError("claim requests must be mappings or ClaimRequest values")
        required = {"id", "claim", "requires"}
        missing = sorted(required.difference(value))
        if missing:
            raise ArgumentError(f"claim request is missing required fields: {missing}")
        return cls(value["id"], value["claim"], value["requires"], value.get("allow_declared", False))

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "claim": self.claim,
            "requires": list(self.requires),
            "allow_declared": self.allow_declared,
        }


def _evidence(value: EvidenceItem | Mapping[str, Any]) -> EvidenceItem:
    return value if isinstance(value, EvidenceItem) else EvidenceItem.from_mapping(value)


def _claim(value: ClaimRequest | Mapping[str, Any]) -> ClaimRequest:
    return value if isinstance(value, ClaimRequest) else ClaimRequest.from_mapping(value)


@dataclass(frozen=True)
class BioCapabilityEvidenceAuditRequest:
    """Bounded arguments for the evidence-conditioned BioCapability profile tool."""

    evidence: Sequence[EvidenceItem | Mapping[str, Any]]
    claim_requests: Sequence[ClaimRequest | Mapping[str, Any]]
    metrics: Mapping[str, Any] | None = None
    vectors: Sequence[Mapping[str, Any]] | None = None
    waived_dimensions: Sequence[str] = ()
    weighting: Mapping[str, Any] | None = None
    information: Mapping[str, Any] | None = None
    reference: Mapping[str, Any] | None = None
    reference_state: str | None = None
    worldline: Mapping[str, Any] | None = None
    at: str | None = None
    reexecution: Mapping[str, Any] | None = None
    biological_claim: str | None = None
    max_items: int = 100

    def __post_init__(self) -> None:
        evidence = tuple(_evidence(item) for item in _sequence("evidence", self.evidence, 512))
        claims = tuple(_claim(item) for item in _sequence("claim_requests", self.claim_requests, 128))
        evidence_ids = [item.id for item in evidence]
        if len(set(evidence_ids)) != len(evidence_ids):
            raise ArgumentError("evidence ids must be unique")
        claim_ids = [item.id for item in claims]
        if len(set(claim_ids)) != len(claim_ids):
            raise ArgumentError("claim request ids must be unique")
        metrics = _mapping("metrics", self.metrics)
        vectors = None if self.vectors is None else _sequence("vectors", self.vectors, 100)
        if metrics is None and (vectors is None or len(vectors) < 2):
            raise ArgumentError("provide metrics or at least two vectors")
        if vectors is not None and any(not isinstance(vector, Mapping) for vector in vectors):
            raise ArgumentError("vectors must contain mappings")
        waived = _sequence("waived_dimensions", self.waived_dimensions, len(EVIDENCE_DIMENSIONS))
        normalized_waived = tuple(_dimension(dimension) for dimension in waived)
        if len(set(normalized_waived)) != len(normalized_waived):
            raise ArgumentError("waived_dimensions must not repeat a dimension")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 1_000:
            raise ArgumentError("max_items must be between 1 and 1000")
        optional_mappings = {
            name: _mapping(name, value)
            for name, value in (
                ("weighting", self.weighting),
                ("information", self.information),
                ("reference", self.reference),
                ("worldline", self.worldline),
                ("reexecution", self.reexecution),
            )
        }
        if self.reference_state is not None:
            _text("reference_state", self.reference_state)
        if self.at is not None:
            _text("at", self.at)
        if self.biological_claim is not None:
            _text("biological_claim", self.biological_claim)
        object.__setattr__(self, "evidence", evidence)
        object.__setattr__(self, "claim_requests", claims)
        object.__setattr__(self, "metrics", metrics)
        object.__setattr__(self, "vectors", None if vectors is None else tuple(dict(vector) for vector in vectors))
        object.__setattr__(self, "waived_dimensions", normalized_waived)
        for name, value in optional_mappings.items():
            object.__setattr__(self, name, value)

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "evidence": [item.to_dict() for item in self.evidence],
            "claim_requests": [claim.to_dict() for claim in self.claim_requests],
            "max_items": self.max_items,
        }
        if self.metrics is not None:
            arguments["metrics"] = dict(self.metrics)
        if self.vectors is not None:
            arguments["vectors"] = [dict(vector) for vector in self.vectors]
        if self.waived_dimensions:
            arguments["waived_dimensions"] = list(self.waived_dimensions)
        for name in ("weighting", "information", "reference", "worldline", "reexecution"):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = dict(value)
        for name in ("reference_state", "at", "biological_claim"):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = value
        return arguments


__all__ = [
    "BioCapabilityEvidenceAuditRequest",
    "ClaimRequest",
    "EVIDENCE_DIMENSIONS",
    "EVIDENCE_STATUSES",
    "EvidenceItem",
    "EvidenceStatus",
]
