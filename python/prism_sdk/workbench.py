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


def workbench_verification_report(value: Mapping[str, Any]) -> WorkbenchVerificationReport:
    """Parse a direct MCP projection or an HTTP REST tool envelope."""

    return WorkbenchVerificationReport.from_wire(value)


__all__ = [
    "WorkbenchRequest",
    "WorkbenchVerificationRequest",
    "WorkbenchVerificationReport",
    "workbench_verification_report",
]
