"""Typed provider-evidence conformance requests and structural reports.

The SDK deliberately models artifact, log, and attestation rows as opaque mappings. Their
provider-specific meaning belongs to the Rust kernel; this layer validates transport shape,
preserves the server's findings, and exposes the digest-bound handoff without claiming that a
remote URI was fetched or that an attestation was cryptographically verified.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import _route_count, _route_mapping, _route_strings, _route_text, _tool_payload
from .errors import ArgumentError

CI_PROVIDER_EVIDENCE_SCHEMA = "bioprism-devplat-ci-provider-evidence/0.1"
MAX_PROVIDER_EVIDENCE_ROWS = 128


def _mapping(name: str, value: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(value, Mapping) or not value:
        raise ArgumentError(f"{name} must be a non-empty mapping")
    return dict(value)


def _rows(name: str, value: Sequence[Mapping[str, Any]] | None) -> tuple[dict[str, Any], ...]:
    if value is None:
        return ()
    if isinstance(value, (str, bytes)) or not isinstance(value, Sequence):
        raise ArgumentError(f"{name} must be an array of mappings")
    if len(value) > MAX_PROVIDER_EVIDENCE_ROWS:
        raise ArgumentError(f"{name} must contain at most {MAX_PROVIDER_EVIDENCE_ROWS} rows")
    result: list[dict[str, Any]] = []
    for index, row in enumerate(value):
        result.append(_mapping(f"{name}[{index}]", row))
    return tuple(result)


def _digest(name: str, value: Any) -> str:
    text = _route_text(name, value)
    if len(text) != 64 or any(character not in "0123456789abcdef" for character in text.lower()):
        raise ArgumentError(f"{name} must be a 64-character hexadecimal digest")
    return text


@dataclass(frozen=True)
class CiProviderEvidenceRequest:
    ci: Mapping[str, Any]
    provider: str
    payload: Mapping[str, Any]
    source: str | None = None
    artifacts: Sequence[Mapping[str, Any]] = ()
    logs: Sequence[Mapping[str, Any]] = ()
    attestations: Sequence[Mapping[str, Any]] = ()

    def __post_init__(self) -> None:
        _mapping("ci", self.ci)
        provider = _route_text("provider", self.provider).lower()
        if provider not in {"github_actions", "gitlab_ci", "generic"}:
            raise ArgumentError("provider must be github_actions, gitlab_ci, or generic")
        _mapping("payload", self.payload)
        if self.source is not None and self.source not in {"caller_attested", "provider_observed"}:
            raise ArgumentError("source must be caller_attested or provider_observed")
        _rows("artifacts", self.artifacts)
        _rows("logs", self.logs)
        _rows("attestations", self.attestations)

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "ci": dict(self.ci),
            "provider": self.provider,
            "payload": dict(self.payload),
            "artifacts": [dict(row) for row in self.artifacts],
            "logs": [dict(row) for row in self.logs],
            "attestations": [dict(row) for row in self.attestations],
        }
        if self.source is not None:
            result["source"] = self.source
        return result


@dataclass(frozen=True)
class CiProviderEvidenceReport:
    raw: dict[str, Any]
    schema: str
    workflow: str
    provider: str
    source: str
    run_id: str
    payload_digest: str
    plan_digest: str
    evidence_digest: str
    artifact_record_digest: str
    log_record_digest: str
    attestation_record_digest: str
    evidence: dict[str, Any]
    artifact_count: int
    log_count: int
    attestation_count: int
    linked_artifact_count: int
    linked_log_count: int
    attestation_subject_count: int
    structurally_valid: bool
    conformance_ready: bool
    execution: str
    verification: str
    ci_evidence: dict[str, Any]
    artifacts: tuple[dict[str, Any], ...]
    logs: tuple[dict[str, Any], ...]
    attestations: tuple[dict[str, Any], ...]
    findings: tuple[dict[str, Any], ...]
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiProviderEvidenceReport":
        raw = _tool_payload(value, "ci_provider_evidence_audit")
        if raw.get("ok") is not True:
            raise ArgumentError("CI provider evidence report is not successful")
        audit = _route_mapping("CI provider evidence audit", raw.get("audit"))
        findings_value = audit.get("findings", [])
        if not isinstance(findings_value, list):
            raise ArgumentError("CI provider evidence findings must be an array")
        rows = {}
        for field in ("artifacts", "logs", "attestations"):
            rows[field] = _rows(f"CI provider evidence {field}", audit.get(field, []))
        return cls(
            raw=raw,
            schema=_route_text("CI provider evidence schema", raw.get("schema")),
            workflow=_route_text("CI provider evidence workflow", audit.get("workflow")),
            provider=_route_text("CI provider evidence provider", audit.get("provider")),
            source=_route_text("CI provider evidence source", audit.get("source")),
            run_id=_route_text("CI provider evidence run_id", audit.get("run_id")),
            payload_digest=_digest("CI provider evidence payload_digest", audit.get("payload_digest")),
            plan_digest=_digest("CI provider evidence plan_digest", audit.get("plan_digest")),
            evidence_digest=_digest("CI provider evidence evidence_digest", audit.get("evidence_digest")),
            artifact_record_digest=_digest("CI provider evidence artifact_record_digest", audit.get("artifact_record_digest")),
            log_record_digest=_digest("CI provider evidence log_record_digest", audit.get("log_record_digest")),
            attestation_record_digest=_digest("CI provider evidence attestation_record_digest", audit.get("attestation_record_digest")),
            evidence=_route_mapping("CI provider canonical run evidence", raw.get("evidence")),
            artifact_count=_route_count("CI provider evidence artifact_count", audit.get("artifact_count")),
            log_count=_route_count("CI provider evidence log_count", audit.get("log_count")),
            attestation_count=_route_count("CI provider evidence attestation_count", audit.get("attestation_count")),
            linked_artifact_count=_route_count("CI provider evidence linked_artifact_count", audit.get("linked_artifact_count")),
            linked_log_count=_route_count("CI provider evidence linked_log_count", audit.get("linked_log_count")),
            attestation_subject_count=_route_count("CI provider evidence attestation_subject_count", audit.get("attestation_subject_count")),
            structurally_valid=audit.get("structurally_valid") is True,
            conformance_ready=raw.get("conformance_ready") is True,
            execution=_route_text("CI provider evidence execution", audit.get("execution")),
            verification=_route_text("CI provider evidence verification", audit.get("verification")),
            ci_evidence=_route_mapping("CI provider canonical evidence", audit.get("ci_evidence")),
            artifacts=rows["artifacts"],
            logs=rows["logs"],
            attestations=rows["attestations"],
            findings=tuple(_route_mapping("CI provider evidence finding", item) for item in findings_value),
            guarantees=_route_strings("CI provider evidence guarantees", audit.get("guarantees", [])),
            limitations=_route_strings("CI provider evidence limitations", audit.get("limitations", [])),
        )

    @property
    def blocking_findings(self) -> tuple[dict[str, Any], ...]:
        return tuple(finding for finding in self.findings if finding.get("severity") == "blocking")

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CiProviderEvidenceRegistryQueryRequest:
    """Bounded query over retained provider evidence and its lineage joins."""

    provider: str | None = None
    run_id: str | None = None
    plan_digest: str | None = None
    structurally_valid: bool | None = None
    conformance_ready: bool | None = None
    after: str | None = None
    max_items: int = 100
    include_records: bool = False

    def __post_init__(self) -> None:
        for name, value in (("provider", self.provider), ("run_id", self.run_id)):
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise ArgumentError(f"{name} must be a non-empty string")
        for name, value in (("plan_digest", self.plan_digest), ("after", self.after)):
            if value is not None:
                _digest(name, value)
        for name, value in (("structurally_valid", self.structurally_valid), ("conformance_ready", self.conformance_ready)):
            if value is not None and not isinstance(value, bool):
                raise ArgumentError(f"{name} must be a boolean")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= 256:
            raise ArgumentError("max_items must be between 1 and 256")
        if not isinstance(self.include_records, bool):
            raise ArgumentError("include_records must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        result: dict[str, Any] = {"max_items": self.max_items, "include_records": self.include_records}
        for name in ("provider", "run_id", "plan_digest", "structurally_valid", "conformance_ready", "after"):
            value = getattr(self, name)
            if value is not None:
                result[name] = value
        return result

    def to_http_query(self) -> dict[str, str]:
        return {key: str(value).lower() if isinstance(value, bool) else str(value) for key, value in self.to_mcp_arguments().items()}


@dataclass(frozen=True)
class CiProviderEvidenceRegistryImportReport:
    raw: dict[str, Any]
    provider_evidence_digest: str
    provider: str
    run_id: str
    plan_digest: str
    evidence_digest: str
    artifact_record_digest: str
    log_record_digest: str
    attestation_record_digest: str
    structurally_valid: bool
    conformance_ready: bool
    created: bool
    already_present: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiProviderEvidenceRegistryImportReport":
        raw = _tool_payload(value, "ci_provider_evidence_import")
        return cls(
            raw=raw,
            provider_evidence_digest=_digest("provider evidence digest", raw.get("provider_evidence_digest")),
            provider=_route_text("provider evidence provider", raw.get("provider")),
            run_id=_route_text("provider evidence run_id", raw.get("run_id")),
            plan_digest=_digest("provider evidence plan_digest", raw.get("plan_digest")),
            evidence_digest=_digest("provider evidence evidence_digest", raw.get("evidence_digest")),
            artifact_record_digest=_digest("provider evidence artifact_record_digest", raw.get("artifact_record_digest")),
            log_record_digest=_digest("provider evidence log_record_digest", raw.get("log_record_digest")),
            attestation_record_digest=_digest("provider evidence attestation_record_digest", raw.get("attestation_record_digest")),
            structurally_valid=raw.get("structurally_valid") is True,
            conformance_ready=raw.get("conformance_ready") is True,
            created=raw.get("created") is True,
            already_present=raw.get("already_present") is True,
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CiProviderEvidenceRegistryQueryReport:
    raw: dict[str, Any]
    rows: tuple[dict[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiProviderEvidenceRegistryQueryReport":
        raw = _tool_payload(value, "ci_provider_evidence_query")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("CI provider evidence query rows must be an array")
        return cls(
            raw=raw,
            rows=tuple(_route_mapping("CI provider evidence query row", item) for item in rows),
            next_after=raw.get("next_after") if isinstance(raw.get("next_after"), str) else None,
            has_more=raw.get("has_more") is True,
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class CiProviderEvidenceRegistryGetReport:
    raw: dict[str, Any]
    provider_evidence_digest: str
    provider: str
    run_id: str
    audit: dict[str, Any]
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "CiProviderEvidenceRegistryGetReport":
        raw = _tool_payload(value, "ci_provider_evidence_get")
        return cls(
            raw=raw,
            provider_evidence_digest=_digest("provider evidence digest", raw.get("provider_evidence_digest")),
            provider=_route_text("provider evidence provider", raw.get("provider")),
            run_id=_route_text("provider evidence run_id", raw.get("run_id")),
            audit=_route_mapping("CI provider evidence retained audit", raw.get("audit")),
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def ci_provider_evidence_report(value: Mapping[str, Any]) -> CiProviderEvidenceReport:
    """Parse a direct MCP result or HTTP REST tool envelope."""

    return CiProviderEvidenceReport.from_wire(value)


def ci_provider_evidence_registry_import_report(value: Mapping[str, Any]) -> CiProviderEvidenceRegistryImportReport:
    return CiProviderEvidenceRegistryImportReport.from_wire(value)


def ci_provider_evidence_registry_query_report(value: Mapping[str, Any]) -> CiProviderEvidenceRegistryQueryReport:
    return CiProviderEvidenceRegistryQueryReport.from_wire(value)


def ci_provider_evidence_registry_get_report(value: Mapping[str, Any]) -> CiProviderEvidenceRegistryGetReport:
    return CiProviderEvidenceRegistryGetReport.from_wire(value)


__all__ = [
    "CI_PROVIDER_EVIDENCE_SCHEMA",
    "MAX_PROVIDER_EVIDENCE_ROWS",
    "CiProviderEvidenceRequest",
    "CiProviderEvidenceReport",
    "CiProviderEvidenceRegistryQueryRequest",
    "CiProviderEvidenceRegistryImportReport",
    "CiProviderEvidenceRegistryQueryReport",
    "CiProviderEvidenceRegistryGetReport",
    "ci_provider_evidence_report",
    "ci_provider_evidence_registry_import_report",
    "ci_provider_evidence_registry_query_report",
    "ci_provider_evidence_registry_get_report",
]
