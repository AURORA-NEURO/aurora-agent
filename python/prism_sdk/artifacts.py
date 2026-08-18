"""Typed client models for the bounded cross-domain artifact registry.

The registry indexes exact artifact JSON and records verification posture and declared parent
edges.  These models intentionally keep missing parents and non-claims visible; a digest lookup
is not treated as causal, scientific, clinical, publication, or external-effect authority.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .errors import ArgumentError

ARTIFACT_KINDS = (
    "mission_evidence_bundle",
    "workflow_reconciliation",
    "mission_report",
    "evaluator_replay",
    "domain_report",
    "domain_evidence_harmonization",
    "domain_evidence_intake",
    "domain_evidence_provider_handoff",
    "domain_evidence_provider_external_payload",
    "domain_evidence_provider_external_payload_replay",
    "domain_evidence_provider_external_payload_lineage_audit",
    "domain_decision_readiness",
    "adapter_execution_evidence",
    "domain_evidence_source_plan",
    "external_reference",
)


def _text(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > 512:
        raise ArgumentError(f"{name} must be a non-empty string of at most 512 UTF-8 bytes")
    return value


def _digest(name: str, value: Any) -> str:
    value = _text(name, value)
    if len(value) != 64 or any(char not in "0123456789abcdefABCDEF" for char in value):
        raise ArgumentError(f"{name} must be a 64-character SHA-256 digest")
    return value.lower()


@dataclass(frozen=True)
class ArtifactRegistrationRequest:
    """One bounded artifact registration accepted by REST or MCP."""

    kind: str
    subject_id: str
    artifact: Mapping[str, Any] | Sequence[Any] | str | int | float | bool | None
    domains: tuple[str, ...] = ()
    parent_digests: tuple[str, ...] = ()
    declared_digest: str | None = None

    def __post_init__(self) -> None:
        if self.kind not in ARTIFACT_KINDS:
            raise ArgumentError(f"kind must be one of {', '.join(ARTIFACT_KINDS)}")
        _text("artifact subject_id", self.subject_id)
        if len(self.domains) > 128 or any(not isinstance(value, str) for value in self.domains):
            raise ArgumentError("artifact domains must contain at most 128 strings")
        for value in self.domains:
            _text("artifact domain", value)
        if len(self.parent_digests) > 128:
            raise ArgumentError("artifact parent_digests must contain at most 128 values")
        for value in self.parent_digests:
            _digest("artifact parent digest", value)
        if self.declared_digest is not None:
            _digest("artifact declared_digest", self.declared_digest)

    def to_arguments(self) -> dict[str, Any]:
        value: dict[str, Any] = {
            "kind": self.kind,
            "subject_id": self.subject_id,
            "domains": list(self.domains),
            "parent_digests": list(self.parent_digests),
            "artifact": self.artifact,
        }
        if self.declared_digest is not None:
            value["declared_digest"] = self.declared_digest
        return value


@dataclass(frozen=True)
class ArtifactRegistrationReport:
    raw: dict[str, Any]
    content_digest: str
    kind: str
    subject_id: str
    created: bool
    already_present: bool
    registry_generation: int
    registry_size: int
    verification: Mapping[str, Any]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactRegistrationReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_register":
            raise ArgumentError("artifact registration workflow is invalid")
        created, already_present = raw.get("created"), raw.get("already_present")
        if not isinstance(created, bool) or not isinstance(already_present, bool):
            raise ArgumentError("artifact registration flags must be booleans")
        return cls(
            raw=raw,
            content_digest=_digest("artifact content digest", raw.get("content_digest")),
            kind=_text("artifact kind", raw.get("kind")),
            subject_id=_text("artifact subject id", raw.get("subject_id")),
            created=created,
            already_present=already_present,
            registry_generation=_count("artifact registry generation", raw.get("registry_generation")),
            registry_size=_count("artifact registry size", raw.get("registry_size")),
            verification=_mapping("artifact verification", raw.get("verification")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ArtifactQueryRequest:
    kind: str | None = None
    domain: str | None = None
    subject_id: str | None = None
    after: str | None = None
    max_items: int = 100
    include_artifacts: bool = False

    def __post_init__(self) -> None:
        if self.kind is not None and self.kind not in ARTIFACT_KINDS:
            raise ArgumentError(f"kind must be one of {', '.join(ARTIFACT_KINDS)}")
        for name, value in (("domain", self.domain), ("subject_id", self.subject_id), ("after", self.after)):
            if value is not None:
                _text(f"artifact query {name}", value)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("artifact query max_items must be between 1 and 256")
        if not isinstance(self.include_artifacts, bool):
            raise ArgumentError("artifact query include_artifacts must be a boolean")

    def to_query_params(self) -> dict[str, str]:
        params = {"limit": str(self.max_items), "include_artifacts": str(self.include_artifacts).lower()}
        for name in ("kind", "domain", "subject_id", "after"):
            value = getattr(self, name)
            if value is not None:
                params[name] = value
        return params

    def to_arguments(self) -> dict[str, Any]:
        return self.to_query_params()


@dataclass(frozen=True)
class ArtifactQueryReport:
    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactQueryReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_query":
            raise ArgumentError("artifact query workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("artifact query rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _digest("artifact query next cursor", next_after)
        has_more = raw.get("has_more")
        if not isinstance(has_more, bool):
            raise ArgumentError("artifact query has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_mapping("artifact query row", row) for row in rows),
            next_after=next_after,
            has_more=has_more,
            registry_generation=_count("artifact query generation", raw.get("registry_generation")),
            registry_size=_count("artifact query size", raw.get("registry_size")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ArtifactGetRequest:
    content_digest: str

    def __post_init__(self) -> None:
        _digest("artifact content digest", self.content_digest)

    def to_arguments(self) -> dict[str, Any]:
        return {"content_digest": self.content_digest}


@dataclass(frozen=True)
class ArtifactGetReport:
    raw: dict[str, Any]
    record: Mapping[str, Any]
    content_digest: str

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactGetReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_get":
            raise ArgumentError("artifact get workflow is invalid")
        record = _mapping("artifact record", raw.get("record"))
        return cls(raw, record, _digest("artifact content digest", record.get("content_digest")))

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ArtifactLineageReport:
    raw: dict[str, Any]
    root: str
    nodes: tuple[Mapping[str, Any], ...]
    missing_parent_digests: tuple[str, ...]
    cycles: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactLineageReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_lineage":
            raise ArgumentError("artifact lineage workflow is invalid")
        nodes = raw.get("nodes", [])
        missing = raw.get("missing_parent_digests", [])
        cycles = raw.get("cycles", [])
        if not all(isinstance(value, Sequence) and not isinstance(value, (str, bytes)) for value in (nodes, missing, cycles)):
            raise ArgumentError("artifact lineage arrays are invalid")
        return cls(
            raw=raw,
            root=_digest("artifact lineage root", raw.get("root")),
            nodes=tuple(_mapping("artifact lineage node", row) for row in nodes),
            missing_parent_digests=tuple(_digest("missing artifact parent digest", item) for item in missing),
            cycles=tuple(_digest("artifact lineage cycle digest", item) for item in cycles),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ArtifactDomainEvidenceLineageRequest:
    """Filters for the digest-bound domain-evidence intake trace view."""

    content_digest: str | None = None
    group_id: str | None = None
    domain: str | None = None
    subject_id: str | None = None
    source_tool: str | None = None
    outcome: str | None = None
    request_digest: str | None = None
    response_digest: str | None = None
    intake_digest: str | None = None
    source_plan_digest: str | None = None
    after: str | None = None
    max_items: int = 100
    include_children: bool = True

    def __post_init__(self) -> None:
        for name in (
            "content_digest",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
        ):
            value = getattr(self, name)
            if value is not None:
                _digest(f"domain evidence lineage {name}", value)
        for name in ("group_id", "domain", "subject_id", "source_tool", "outcome"):
            value = getattr(self, name)
            if value is not None:
                _text(f"domain evidence lineage {name}", value)
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 256:
            raise ArgumentError("domain evidence lineage max_items must be between 1 and 256")
        if not isinstance(self.include_children, bool):
            raise ArgumentError("domain evidence lineage include_children must be a boolean")

    def to_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "max_items": self.max_items,
            "include_children": self.include_children,
        }
        for name in (
            "content_digest",
            "group_id",
            "domain",
            "subject_id",
            "source_tool",
            "outcome",
            "request_digest",
            "response_digest",
            "intake_digest",
            "source_plan_digest",
            "after",
        ):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = value
        return arguments

    def to_query_params(self) -> dict[str, str]:
        return {key: str(value).lower() if isinstance(value, bool) else str(value) for key, value in self.to_arguments().items()}


@dataclass(frozen=True)
class ArtifactDomainEvidenceLineageReport:
    raw: dict[str, Any]
    rows: tuple[Mapping[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactDomainEvidenceLineageReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_domain_evidence_lineage":
            raise ArgumentError("domain evidence lineage workflow is invalid")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("domain evidence lineage rows must be an array")
        next_after = raw.get("next_after")
        if next_after is not None:
            _digest("domain evidence lineage next cursor", next_after)
        has_more = raw.get("has_more")
        if not isinstance(has_more, bool):
            raise ArgumentError("domain evidence lineage has_more must be a boolean")
        return cls(
            raw=raw,
            rows=tuple(_mapping("domain evidence lineage row", row) for row in rows),
            next_after=next_after,
            has_more=has_more,
            registry_generation=_count("domain evidence lineage generation", raw.get("registry_generation")),
            registry_size=_count("domain evidence lineage size", raw.get("registry_size")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class ArtifactCrossStoreAuditReport:
    """Digest-only consistency diagnostics across the three bounded local registries."""

    raw: dict[str, Any]
    consistent: bool
    truncated: bool
    stores: Mapping[str, Any]
    coverage: Mapping[str, Any]
    findings: tuple[Mapping[str, Any], ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ArtifactCrossStoreAuditReport":
        raw = dict(value)
        if raw.get("workflow") != "artifact_registry_cross_store_audit":
            raise ArgumentError("artifact cross-store audit workflow is invalid")
        consistent, truncated = raw.get("consistent"), raw.get("truncated")
        if not isinstance(consistent, bool) or not isinstance(truncated, bool):
            raise ArgumentError("artifact cross-store audit flags must be booleans")
        findings = raw.get("findings", [])
        if not isinstance(findings, Sequence) or isinstance(findings, (str, bytes)):
            raise ArgumentError("artifact cross-store audit findings must be an array")
        return cls(
            raw=raw,
            consistent=consistent,
            truncated=truncated,
            stores=_mapping("artifact cross-store audit stores", raw.get("stores")),
            coverage=_mapping("artifact cross-store audit coverage", raw.get("coverage")),
            findings=tuple(_mapping("artifact cross-store audit finding", item) for item in findings),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _mapping(name: str, value: Any) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be an object")
    return dict(value)


def _count(name: str, value: Any) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ArgumentError(f"{name} must be a non-negative integer")
    return value
