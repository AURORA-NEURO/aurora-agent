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

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
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


def _report_bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _report_optional_text(name: str, value: Any) -> str | None:
    if value is None:
        return None
    return _route_text(name, value)


def _report_status(name: str, value: Any) -> str | None:
    status = _report_optional_text(name, value)
    if status is not None and status not in EVIDENCE_STATUSES:
        raise ArgumentError(f"{name} must be one of {EVIDENCE_STATUSES!r}")
    return status


def _report_mappings(name: str, value: Any) -> tuple[dict[str, Any], ...]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise ArgumentError(f"{name} must be an array")
    return tuple(_route_mapping(f"{name}[{index}]", item) for index, item in enumerate(value))


@dataclass(frozen=True)
class EvidenceAuditItemReport:
    """One bounded evidence outcome, including fail-closed support diagnostics."""

    raw: dict[str, Any]
    index: int
    ok: bool
    id: str | None
    dimension: str | None
    domain: str | None
    declared_status: str | None
    effective_status: str | None
    issues: tuple[dict[str, Any], ...]
    support: dict[str, Any] | None
    fail_closed: bool
    refusal: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvidenceAuditItemReport":
        raw = _route_mapping("evidence audit item", value)
        return cls(
            raw=raw,
            index=_route_count("evidence audit item index", raw.get("index")),
            ok=_report_bool("evidence audit item ok", raw.get("ok")),
            id=_report_optional_text("evidence audit item id", raw.get("id")),
            dimension=_report_optional_text("evidence audit item dimension", raw.get("dimension")),
            domain=_report_optional_text("evidence audit item domain", raw.get("domain")),
            declared_status=_report_status(
                "evidence audit item declared_status", raw.get("declared_status")
            ),
            effective_status=_report_status(
                "evidence audit item effective_status", raw.get("effective_status")
            ),
            issues=_report_mappings("evidence audit item issues", raw.get("issues", [])),
            support=(
                None
                if raw.get("support") is None
                else _route_mapping("evidence audit item support", raw.get("support"))
            ),
            fail_closed=_report_bool("evidence audit item fail_closed", raw.get("fail_closed")),
            refusal=_report_optional_text("evidence audit item refusal", raw.get("refusal")),
        )

    @property
    def measured(self) -> bool:
        return self.effective_status in {"observed", "reproduced"}

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class EvidenceDimensionReport:
    """Rollup state for one of the nine explicit evidence dimensions."""

    raw: dict[str, Any]
    dimension: str
    state: str
    evidence_count: int
    measured_count: int
    declared_count: int
    blocked_count: int
    missing: bool
    measured: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvidenceDimensionReport":
        raw = _route_mapping("evidence dimension", value)
        state = _route_text("evidence dimension state", raw.get("state"))
        if state not in EVIDENCE_STATUSES:
            raise ArgumentError(f"evidence dimension state must be one of {EVIDENCE_STATUSES!r}")
        evidence_count = _route_count("evidence dimension evidence_count", raw.get("evidence_count"))
        measured_count = _route_count("evidence dimension measured_count", raw.get("measured_count"))
        declared_count = _route_count("evidence dimension declared_count", raw.get("declared_count"))
        blocked_count = _route_count("evidence dimension blocked_count", raw.get("blocked_count"))
        if measured_count + declared_count + blocked_count > evidence_count:
            raise ArgumentError("evidence dimension counts do not reconcile")
        return cls(
            raw=raw,
            dimension=_route_text("evidence dimension name", raw.get("dimension")),
            state=state,
            evidence_count=evidence_count,
            measured_count=measured_count,
            declared_count=declared_count,
            blocked_count=blocked_count,
            missing=_report_bool("evidence dimension missing", raw.get("missing")),
            measured=_report_bool("evidence dimension measured", raw.get("measured")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class EvidenceInventoryReport:
    """Bounded evidence rows, dimension rollups, domain counts, and omission accounting."""

    raw: dict[str, Any]
    items: tuple[EvidenceAuditItemReport, ...]
    omitted_items: int
    item_count: int
    invalid_item_count: int
    dimensions: tuple[EvidenceDimensionReport, ...]
    domains: dict[str, int]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvidenceInventoryReport":
        raw = _route_mapping("evidence inventory", value)
        items = tuple(EvidenceAuditItemReport.from_wire(item) for item in _report_mappings("evidence items", raw.get("items", [])))
        omitted_items = _route_count("evidence omitted_items", raw.get("omitted_items"))
        item_count = _route_count("evidence item_count", raw.get("item_count"))
        invalid_item_count = _route_count("evidence invalid_item_count", raw.get("invalid_item_count"))
        if len(items) + omitted_items != item_count or invalid_item_count > item_count:
            raise ArgumentError("evidence inventory counts do not reconcile")
        dimensions = tuple(
            EvidenceDimensionReport.from_wire(item)
            for item in _report_mappings("evidence dimensions", raw.get("dimensions", []))
        )
        raw_domains = _route_mapping("evidence domains", raw.get("domains", {}))
        domains: dict[str, int] = {}
        for domain, count in raw_domains.items():
            domains[_route_text("evidence domain", domain)] = _route_count(
                f"evidence domain count for {domain}", count
            )
        return cls(
            raw=raw,
            items=items,
            omitted_items=omitted_items,
            item_count=item_count,
            invalid_item_count=invalid_item_count,
            dimensions=dimensions,
            domains=domains,
        )

    @property
    def complete(self) -> bool:
        return self.omitted_items == 0

    @property
    def measured_dimensions(self) -> tuple[str, ...]:
        return tuple(sorted(dimension.dimension for dimension in self.dimensions if dimension.measured))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ClaimAuditRowReport:
    """One explicit claim prerequisite result and its blockers or assumptions."""

    raw: dict[str, Any]
    index: int
    ok: bool
    id: str | None
    claim: str | None
    requires: tuple[str, ...]
    allow_declared: bool | None
    eligible: bool | None
    blockers: tuple[dict[str, Any], ...]
    explicit_assumptions: tuple[dict[str, Any], ...]
    fail_closed: bool
    refusal: str | None

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ClaimAuditRowReport":
        raw = _route_mapping("claim audit row", value)
        raw_allow_declared = raw.get("allow_declared")
        allow_declared = None if raw_allow_declared is None else _report_bool(
            "claim audit allow_declared", raw_allow_declared
        )
        raw_eligible = raw.get("eligible")
        eligible = None if raw_eligible is None else _report_bool("claim audit eligible", raw_eligible)
        return cls(
            raw=raw,
            index=_route_count("claim audit row index", raw.get("index")),
            ok=_report_bool("claim audit row ok", raw.get("ok")),
            id=_report_optional_text("claim audit row id", raw.get("id")),
            claim=_report_optional_text("claim audit row claim", raw.get("claim")),
            requires=_route_strings("claim audit row requires", raw.get("requires", [])),
            allow_declared=allow_declared,
            eligible=eligible,
            blockers=_report_mappings("claim audit row blockers", raw.get("blockers", [])),
            explicit_assumptions=_report_mappings(
                "claim audit row explicit_assumptions", raw.get("explicit_assumptions", [])
            ),
            fail_closed=_report_bool("claim audit row fail_closed", raw.get("fail_closed")),
            refusal=_report_optional_text("claim audit row refusal", raw.get("refusal")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ClaimInventoryReport:
    """Bounded claim rows and explicit eligibility totals."""

    raw: dict[str, Any]
    rows: tuple[ClaimAuditRowReport, ...]
    omitted_rows: int
    requested: int
    eligible: int
    all_requested_claims_eligible: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ClaimInventoryReport":
        raw = _route_mapping("claim inventory", value)
        rows = tuple(
            ClaimAuditRowReport.from_wire(row)
            for row in _report_mappings("claim rows", raw.get("rows", []))
        )
        omitted_rows = _route_count("claim omitted_rows", raw.get("omitted_rows"))
        requested = _route_count("claim requested", raw.get("requested"))
        eligible = _route_count("claim eligible", raw.get("eligible"))
        if len(rows) + omitted_rows != requested or eligible > requested:
            raise ArgumentError("claim inventory counts do not reconcile")
        return cls(
            raw=raw,
            rows=rows,
            omitted_rows=omitted_rows,
            requested=requested,
            eligible=eligible,
            all_requested_claims_eligible=_report_bool(
                "claim all_requested_claims_eligible", raw.get("all_requested_claims_eligible")
            ),
        )

    @property
    def complete(self) -> bool:
        return self.omitted_rows == 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class EvidenceReleasePostureReport:
    """Fail-closed claim-readiness posture emitted by the authoritative kernel."""

    raw: dict[str, Any]
    ready_for_requested_claims: bool
    requires_explicit_claim_request: bool
    numeric_scores_are_not_claims_without_evidence: bool
    declared_evidence_is_visible_but_not_measured_support: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "EvidenceReleasePostureReport":
        raw = _route_mapping("evidence release posture", value)
        return cls(
            raw=raw,
            ready_for_requested_claims=_report_bool(
                "evidence ready_for_requested_claims", raw.get("ready_for_requested_claims")
            ),
            requires_explicit_claim_request=_report_bool(
                "evidence requires_explicit_claim_request",
                raw.get("requires_explicit_claim_request"),
            ),
            numeric_scores_are_not_claims_without_evidence=_report_bool(
                "evidence numeric_scores_are_not_claims_without_evidence",
                raw.get("numeric_scores_are_not_claims_without_evidence"),
            ),
            declared_evidence_is_visible_but_not_measured_support=_report_bool(
                "evidence declared_evidence_is_visible_but_not_measured_support",
                raw.get("declared_evidence_is_visible_but_not_measured_support"),
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class BioCapabilityEvidenceAuditReport:
    """Typed cross-domain evidence inventory, claim gating, and optional subaudits."""

    raw: dict[str, Any]
    workflow: str
    metrics: dict[str, Any]
    metrics_ok: bool
    evidence: EvidenceInventoryReport
    claim_requests: ClaimInventoryReport
    subaudits: dict[str, dict[str, Any] | None]
    release_posture: EvidenceReleasePostureReport
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "BioCapabilityEvidenceAuditReport":
        raw = _route_mapping("biocapability evidence audit report", value)
        if raw.get("ok") is False:
            raise ArgumentError("biocapability evidence audit report is not successful")
        if raw.get("workflow") != "biocapability_evidence_conditioned_profile":
            raise ArgumentError("biocapability evidence audit workflow is invalid")
        raw_subaudits = _route_mapping("evidence subaudits", raw.get("subaudits", {}))
        return cls(
            raw=raw,
            workflow=_route_text("biocapability evidence workflow", raw.get("workflow")),
            metrics=_route_mapping("biocapability evidence metrics", raw.get("metrics", {})),
            metrics_ok=_report_bool("biocapability evidence metrics_ok", raw.get("metrics_ok")),
            evidence=EvidenceInventoryReport.from_wire(raw.get("evidence", {})),
            claim_requests=ClaimInventoryReport.from_wire(raw.get("claim_requests", {})),
            subaudits={
                name: None if raw_subaudits.get(name) is None else _route_mapping(
                    f"evidence subaudit {name}", raw_subaudits.get(name)
                )
                for name in (
                    "information_value",
                    "reference_quality",
                    "temporal_validity",
                    "reproducibility",
                )
            },
            release_posture=EvidenceReleasePostureReport.from_wire(
                raw.get("release_posture", {})
            ),
            guarantees=_route_strings("biocapability evidence guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("biocapability evidence limitations", raw.get("limitations", [])),
        )

    @property
    def ready_for_requested_claims(self) -> bool:
        return self.release_posture.ready_for_requested_claims

    @property
    def domains(self) -> tuple[str, ...]:
        return tuple(sorted(self.evidence.domains))

    @property
    def has_explicit_claim_request(self) -> bool:
        return self.claim_requests.requested > 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def biocapability_evidence_audit_report(
    value: Mapping[str, Any],
) -> BioCapabilityEvidenceAuditReport:
    """Parse a direct evidence audit result or an HTTP tool envelope."""

    return BioCapabilityEvidenceAuditReport.from_wire(
        _tool_payload(value, "biocapability_evidence_conditioned_profile")
    )


__all__ = [
    "BioCapabilityEvidenceAuditRequest",
    "BioCapabilityEvidenceAuditReport",
    "ClaimAuditRowReport",
    "ClaimInventoryReport",
    "ClaimRequest",
    "EvidenceAuditItemReport",
    "EvidenceDimensionReport",
    "EVIDENCE_DIMENSIONS",
    "EVIDENCE_STATUSES",
    "EvidenceInventoryReport",
    "EvidenceItem",
    "EvidenceReleasePostureReport",
    "EvidenceStatus",
    "biocapability_evidence_audit_report",
]
