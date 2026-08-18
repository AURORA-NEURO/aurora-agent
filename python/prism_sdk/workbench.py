"""Typed transport request for the Rust developer workbench.

The Rust workbench owns session validation, dependency ordering, stale detection, dashboard
projection, and CI YAML generation. This facade validates only that the nested wire objects are
JSON mappings and preserves them unchanged.
"""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .capability import _route_mapping, _route_text, _tool_payload
from .errors import ArgumentError


def _mapping(name: str, value: Mapping[str, Any] | None) -> dict[str, Any] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise ArgumentError(f"{name} must be a mapping")
    return dict(value)


@dataclass(frozen=True)
class WorkbenchRequest:
    """Compose authoring-session audit, dashboard query, and optional CI planning."""

    session: Mapping[str, Any]
    dashboard: Mapping[str, Any] | None = None
    ci: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.session, Mapping):
            raise ArgumentError("session must be a mapping")
        if not self.session:
            raise ArgumentError("session must not be empty")
        _mapping("dashboard", self.dashboard)
        _mapping("ci", self.ci)

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"session": dict(self.session)}
        dashboard = _mapping("dashboard", self.dashboard)
        ci = _mapping("ci", self.ci)
        if dashboard is not None:
            arguments["dashboard"] = dashboard
        if ci is not None:
            arguments["ci"] = ci
        return arguments


@dataclass(frozen=True)
class WorkbenchVerificationRequest:
    """Verify a retained workbench report without executing cells or contacting CI."""

    session: Mapping[str, Any]
    report: Mapping[str, Any]
    expected_report_digest: str | None = None
    ci_replay: Mapping[str, Any] | None = None
    policy: Mapping[str, Any] | None = None

    def __post_init__(self) -> None:
        if not isinstance(self.session, Mapping) or not self.session:
            raise ArgumentError("session must be a non-empty mapping")
        if not isinstance(self.report, Mapping) or not self.report:
            raise ArgumentError("report must be a non-empty mapping")
        if self.expected_report_digest is not None:
            if not isinstance(self.expected_report_digest, str) or not _DIGEST.fullmatch(self.expected_report_digest):
                raise ArgumentError("expected_report_digest must be a lowercase SHA-256 digest")
        _mapping("ci_replay", self.ci_replay)
        _mapping("policy", self.policy)

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {
            "session": dict(self.session),
            "report": dict(self.report),
        }
        if self.expected_report_digest is not None:
            arguments["expected_report_digest"] = self.expected_report_digest
        ci_replay = _mapping("ci_replay", self.ci_replay)
        if ci_replay is not None:
            arguments["ci_replay"] = ci_replay
        policy = _mapping("policy", self.policy)
        if policy is not None:
            arguments["policy"] = policy
        return arguments


@dataclass(frozen=True)
class WorkbenchRegistryImportRequest:
    """Import one direct or transport-wrapped workbench report into retention."""

    report: Mapping[str, Any]

    def __post_init__(self) -> None:
        if not isinstance(self.report, Mapping) or not self.report:
            raise ArgumentError("report must be a non-empty mapping")

    def to_mcp_arguments(self) -> dict[str, Any]:
        return {"report": dict(self.report)}


@dataclass(frozen=True)
class WorkbenchRegistryQueryRequest:
    """Bounded report-index query; report bodies are opt-in."""

    session_digest: str | None = None
    domain: str | None = None
    capability: str | None = None
    state: str | None = None
    release_ready: bool | None = None
    after: str | None = None
    max_items: int = 100
    include_reports: bool = False

    def __post_init__(self) -> None:
        for name, value in (("session_digest", self.session_digest), ("after", self.after)):
            if value is not None and (not isinstance(value, str) or not _DIGEST.fullmatch(value)):
                raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
        for name, value in (("domain", self.domain), ("capability", self.capability), ("state", self.state)):
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise ArgumentError(f"{name} must be a non-empty string")
        if not isinstance(self.max_items, int) or isinstance(self.max_items, bool) or not 1 <= self.max_items <= 256:
            raise ArgumentError("max_items must be between 1 and 256")
        if self.release_ready is not None and not isinstance(self.release_ready, bool):
            raise ArgumentError("release_ready must be a boolean")
        if not isinstance(self.include_reports, bool):
            raise ArgumentError("include_reports must be a boolean")

    def to_mcp_arguments(self) -> dict[str, Any]:
        arguments: dict[str, Any] = {"max_items": self.max_items, "include_reports": self.include_reports}
        for name in ("session_digest", "domain", "capability", "state", "release_ready", "after"):
            value = getattr(self, name)
            if value is not None:
                arguments[name] = value
        return arguments

    def to_http_query(self) -> dict[str, str]:
        return {key: str(value).lower() if isinstance(value, bool) else str(value) for key, value in self.to_mcp_arguments().items()}


_DIGEST = re.compile(r"^[0-9a-f]{64}$")


@dataclass(frozen=True)
class WorkbenchVerificationReport:
    """Typed digest, dashboard, CI replay, and mismatch posture."""

    raw: dict[str, Any]
    schema_version: str
    workflow: str
    valid: bool
    status: str
    retained_report_digest: str
    expected_report_digest: str | None
    report_digest_matched: bool | None
    retained_audit_digest: str
    observed_audit_digest: str
    dashboard_present: bool
    dashboard_verified: bool
    ci_present: bool
    ci_replay_supplied: bool
    ci_verified: bool
    mismatches: tuple[dict[str, Any], ...]
    execution: str
    network_access: str
    verification_digest: str
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkbenchVerificationReport":
        raw = _tool_payload(value, "developer_workbench_verify")
        mismatches = raw.get("mismatches", [])
        if not isinstance(mismatches, Sequence) or isinstance(mismatches, (str, bytes)):
            raise ArgumentError("workbench verification mismatches must be an array")
        guarantees = raw.get("guarantees", [])
        limitations = raw.get("limitations", [])
        if not isinstance(guarantees, Sequence) or isinstance(guarantees, (str, bytes)):
            raise ArgumentError("workbench verification guarantees must be an array")
        if not isinstance(limitations, Sequence) or isinstance(limitations, (str, bytes)):
            raise ArgumentError("workbench verification limitations must be an array")
        return cls(
            raw=raw,
            schema_version=_route_text("workbench verification schema_version", raw.get("schema_version")),
            workflow=_route_text("workbench verification workflow", raw.get("workflow")),
            valid=raw.get("valid") is True,
            status=_route_text("workbench verification status", raw.get("status")),
            retained_report_digest=_route_text("retained report digest", raw.get("retained_report_digest")),
            expected_report_digest=raw.get("expected_report_digest") if isinstance(raw.get("expected_report_digest"), str) else None,
            report_digest_matched=raw.get("report_digest_matched") if isinstance(raw.get("report_digest_matched"), bool) else None,
            retained_audit_digest=_route_text("retained audit digest", raw.get("retained_audit_digest")),
            observed_audit_digest=_route_text("observed audit digest", raw.get("observed_audit_digest")),
            dashboard_present=raw.get("dashboard_present") is True,
            dashboard_verified=raw.get("dashboard_verified") is True,
            ci_present=raw.get("ci_present") is True,
            ci_replay_supplied=raw.get("ci_replay_supplied") is True,
            ci_verified=raw.get("ci_verified") is True,
            mismatches=tuple(_route_mapping("workbench verification mismatch", item) for item in mismatches),
            execution=_route_text("workbench verification execution", raw.get("execution")),
            network_access=_route_text("workbench verification network_access", raw.get("network_access")),
            verification_digest=_route_text("workbench verification digest", raw.get("verification_digest")),
            guarantees=tuple(_route_text("workbench verification guarantee", item) for item in guarantees),
            limitations=tuple(_route_text("workbench verification limitation", item) for item in limitations),
        )

    @property
    def verified(self) -> bool:
        return self.valid and self.status in {"verified", "verified_without_replay"}

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class WorkbenchRegistryImportReport:
    raw: dict[str, Any]
    workbench_report_digest: str
    created: bool
    already_present: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkbenchRegistryImportReport":
        raw = _tool_payload(value, "developer_workbench_import")
        return cls(
            raw=raw,
            workbench_report_digest=_route_text("workbench report digest", raw.get("workbench_report_digest")),
            created=raw.get("created") is True,
            already_present=raw.get("already_present") is True,
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class WorkbenchRegistryQueryReport:
    raw: dict[str, Any]
    rows: tuple[dict[str, Any], ...]
    next_after: str | None
    has_more: bool
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkbenchRegistryQueryReport":
        raw = _tool_payload(value, "developer_workbench_query")
        rows = raw.get("rows", [])
        if not isinstance(rows, Sequence) or isinstance(rows, (str, bytes)):
            raise ArgumentError("workbench query rows must be an array")
        return cls(
            raw=raw,
            rows=tuple(_route_mapping("workbench query row", item) for item in rows),
            next_after=raw.get("next_after") if isinstance(raw.get("next_after"), str) else None,
            has_more=raw.get("has_more") is True,
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class WorkbenchRegistryGetReport:
    raw: dict[str, Any]
    workbench_report_digest: str
    report: dict[str, Any]
    registry_generation: int
    registry_size: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "WorkbenchRegistryGetReport":
        raw = _tool_payload(value, "developer_workbench_get")
        report = raw.get("report")
        if not isinstance(report, Mapping):
            raise ArgumentError("workbench get report must be a mapping")
        return cls(
            raw=raw,
            workbench_report_digest=_route_text("workbench report digest", raw.get("workbench_report_digest")),
            report=dict(report),
            registry_generation=int(raw.get("registry_generation", 0)),
            registry_size=int(raw.get("registry_size", 0)),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def workbench_verification_report(value: Mapping[str, Any]) -> WorkbenchVerificationReport:
    """Parse a direct MCP projection or an HTTP REST tool envelope."""

    return WorkbenchVerificationReport.from_wire(value)


def workbench_registry_import_report(value: Mapping[str, Any]) -> WorkbenchRegistryImportReport:
    return WorkbenchRegistryImportReport.from_wire(value)


def workbench_registry_query_report(value: Mapping[str, Any]) -> WorkbenchRegistryQueryReport:
    return WorkbenchRegistryQueryReport.from_wire(value)


def workbench_registry_get_report(value: Mapping[str, Any]) -> WorkbenchRegistryGetReport:
    return WorkbenchRegistryGetReport.from_wire(value)


__all__ = [
    "WorkbenchRequest",
    "WorkbenchVerificationRequest",
    "WorkbenchVerificationReport",
    "WorkbenchRegistryImportRequest",
    "WorkbenchRegistryQueryRequest",
    "WorkbenchRegistryImportReport",
    "WorkbenchRegistryQueryReport",
    "WorkbenchRegistryGetReport",
    "workbench_verification_report",
    "workbench_registry_import_report",
    "workbench_registry_query_report",
    "workbench_registry_get_report",
]
