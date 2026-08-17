"""Typed delivery-readiness projections for the cross-domain release boundary."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .capability import (
    _route_count,
    _route_mapping,
    _route_strings,
    _route_text,
    _tool_payload,
)
from .errors import ArgumentError


def _delivery_bool(name: str, value: Any) -> bool:
    if not isinstance(value, bool):
        raise ArgumentError(f"{name} must be a boolean")
    return value


def _optional_mapping(name: str, value: Any) -> dict[str, Any] | None:
    if value is None:
        return None
    return _route_mapping(name, value)


@dataclass(frozen=True)
class DeliveryTargetReport:
    """One explicitly requested release target and its fail-closed blockers."""

    raw: dict[str, Any]
    target: str
    available: bool
    eligible: bool
    blockers: tuple[str, ...]
    notes: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryTargetReport":
        raw = _route_mapping("delivery target", value)
        return cls(
            raw=raw,
            target=_route_text("delivery target name", raw.get("target")),
            available=_delivery_bool("delivery target available", raw.get("available")),
            eligible=_delivery_bool("delivery target eligible", raw.get("eligible")),
            blockers=_route_strings("delivery target blockers", raw.get("blockers", [])),
            notes=_route_strings("delivery target notes", raw.get("notes", [])),
        )

    @property
    def ready(self) -> bool:
        return self.available and self.eligible and not self.blockers

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryReadinessReport:
    """Composed readiness gates from platform through release audit."""

    raw: dict[str, Any]
    platform_checks_clean: bool
    unguarded_claims: int
    developer_claims_ready: bool
    repository_scope_clean: bool
    repository_impact_clean: bool
    sdk_admission_clean: bool
    conformance_release: bool
    provider_capability_gate_cleared: bool
    governance_document_clean: bool
    release_audit_ready: bool
    ci_execution_evidence_ready: bool
    ci_provider_evidence_ready: bool
    execution_provenance_ready: bool
    local_delivery_ready: bool

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReadinessReport":
        raw = _route_mapping("delivery readiness", value)
        return cls(
            raw=raw,
            platform_checks_clean=_delivery_bool(
                "delivery platform_checks_clean", raw.get("platform_checks_clean")
            ),
            unguarded_claims=_route_count("delivery unguarded_claims", raw.get("unguarded_claims")),
            developer_claims_ready=_delivery_bool(
                "delivery developer_claims_ready", raw.get("developer_claims_ready")
            ),
            repository_scope_clean=_delivery_bool(
                "delivery repository_scope_clean", raw.get("repository_scope_clean")
            ),
            repository_impact_clean=_delivery_bool(
                "delivery repository_impact_clean", raw.get("repository_impact_clean")
            ),
            sdk_admission_clean=_delivery_bool(
                "delivery sdk_admission_clean", raw.get("sdk_admission_clean")
            ),
            conformance_release=_delivery_bool(
                "delivery conformance_release", raw.get("conformance_release")
            ),
            provider_capability_gate_cleared=_delivery_bool(
                "delivery provider_capability_gate_cleared",
                raw.get("provider_capability_gate_cleared"),
            ),
            governance_document_clean=_delivery_bool(
                "delivery governance_document_clean", raw.get("governance_document_clean")
            ),
            release_audit_ready=_delivery_bool(
                "delivery release_audit_ready", raw.get("release_audit_ready")
            ),
            ci_execution_evidence_ready=_delivery_bool(
                "delivery ci_execution_evidence_ready",
                raw.get("ci_execution_evidence_ready", False),
            ),
            ci_provider_evidence_ready=_delivery_bool(
                "delivery ci_provider_evidence_ready",
                raw.get("ci_provider_evidence_ready", False),
            ),
            execution_provenance_ready=_delivery_bool(
                "delivery execution_provenance_ready",
                raw.get("execution_provenance_ready", False),
            ),
            local_delivery_ready=_delivery_bool(
                "delivery local_delivery_ready", raw.get("local_delivery_ready")
            ),
        )

    @property
    def claims_guarded(self) -> bool:
        return self.unguarded_claims == 0

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryExternalSurfaceReport:
    """Explicit foreign-artifact and unverified-surface posture."""

    raw: dict[str, Any]
    foreign_subject_count: int
    foreign_artifacts_present: bool
    foreign_artifacts_are_not_inferred: bool
    local_integration_foundations: tuple[dict[str, Any], ...]
    unverified_surface_families: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryExternalSurfaceReport":
        raw = _route_mapping("delivery external surface", value)
        raw_foundations = raw.get("local_integration_foundations", [])
        if not isinstance(raw_foundations, Sequence) or isinstance(raw_foundations, (str, bytes)):
            raise ArgumentError("delivery local_integration_foundations must be an array")
        return cls(
            raw=raw,
            foreign_subject_count=_route_count(
                "delivery foreign_subject_count", raw.get("foreign_subject_count")
            ),
            foreign_artifacts_present=_delivery_bool(
                "delivery foreign_artifacts_present", raw.get("foreign_artifacts_present")
            ),
            foreign_artifacts_are_not_inferred=_delivery_bool(
                "delivery foreign_artifacts_are_not_inferred",
                raw.get("foreign_artifacts_are_not_inferred"),
            ),
            local_integration_foundations=tuple(
                _route_mapping("delivery local integration foundation", foundation)
                for foundation in raw_foundations
            ),
            unverified_surface_families=_route_strings(
                "delivery unverified_surface_families", raw.get("unverified_surface_families", [])
            ),
        )

    @property
    def foreign_posture_explicit(self) -> bool:
        return not self.foreign_artifacts_present or self.foreign_artifacts_are_not_inferred

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeliveryReleaseRequestReport:
    """Explicit release-request state; absence can never become implicit approval."""

    raw: dict[str, Any]
    present: bool
    ready: bool
    fail_closed: bool | None
    no_implicit_release: bool
    request_id: str | None
    targets: tuple[DeliveryTargetReport, ...]
    reason: str | None
    available_target_count: int

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeliveryReleaseRequestReport":
        raw = _route_mapping("delivery release request", value)
        present = _delivery_bool("delivery release request present", raw.get("present"))
        ready = _delivery_bool("delivery release request ready", raw.get("ready"))
        raw_targets = raw.get("targets", [])
        if not isinstance(raw_targets, Sequence) or isinstance(raw_targets, (str, bytes)):
            raise ArgumentError("delivery release request targets must be an array")
        targets = tuple(DeliveryTargetReport.from_wire(target) for target in raw_targets)
        target_names = tuple(target.target for target in targets)
        if present and not targets:
            raise ArgumentError("present delivery release requests require targets")
        if not present and targets:
            raise ArgumentError("absent delivery release requests cannot contain targets")
        if len(target_names) != len(set(target_names)):
            raise ArgumentError("delivery release request targets must be unique")
        if present and ready != all(target.eligible for target in targets):
            raise ArgumentError("delivery release request readiness does not reconcile with targets")
        raw_fail_closed = raw.get("fail_closed")
        fail_closed = None if raw_fail_closed is None else _delivery_bool(
            "delivery release request fail_closed", raw_fail_closed
        )
        if fail_closed is not None and fail_closed == ready:
            raise ArgumentError("delivery release request fail_closed must be the inverse of ready")
        raw_id = raw.get("id")
        request_id = None if raw_id is None else _route_text("delivery release request id", raw_id)
        raw_reason = raw.get("reason")
        reason = None if raw_reason is None else _route_text("delivery release request reason", raw_reason)
        return cls(
            raw=raw,
            present=present,
            ready=ready,
            fail_closed=fail_closed,
            no_implicit_release=_delivery_bool(
                "delivery release request no_implicit_release", raw.get("no_implicit_release")
            ),
            request_id=request_id,
            targets=targets,
            reason=reason,
            available_target_count=_route_count(
                "delivery available_target_count", raw.get("available_target_count")
            ),
        )

    @property
    def blockers(self) -> tuple[str, ...]:
        return tuple(blocker for target in self.targets for blocker in target.blockers)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


@dataclass(frozen=True)
class DeveloperDeliveryAuditReport:
    """Typed, fail-closed cross-domain delivery audit with raw evidence preserved."""

    raw: dict[str, Any]
    workflow: str
    readiness: DeliveryReadinessReport
    external_surface_posture: DeliveryExternalSurfaceReport
    release_request: DeliveryReleaseRequestReport
    checks: dict[str, dict[str, Any] | None]
    ci_provider_normalization: dict[str, Any] | None
    ci_provider_evidence: dict[str, Any] | None
    guarantees: tuple[str, ...]
    limitations: tuple[str, ...]

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "DeveloperDeliveryAuditReport":
        raw = _route_mapping("developer delivery audit report", value)
        if raw.get("ok") is False:
            raise ArgumentError("developer delivery audit report is not successful")
        if raw.get("workflow") != "developer_delivery_audit":
            raise ArgumentError("developer delivery audit workflow is invalid")
        return cls(
            raw=raw,
            workflow=_route_text("developer delivery audit workflow", raw.get("workflow")),
            readiness=DeliveryReadinessReport.from_wire(raw.get("readiness", {})),
            external_surface_posture=DeliveryExternalSurfaceReport.from_wire(
                raw.get("external_surface_posture", {})
            ),
            release_request=DeliveryReleaseRequestReport.from_wire(
                raw.get("release_request", {})
            ),
            checks={
                name: _optional_mapping(f"developer delivery {name}", raw.get(name))
                for name in (
                    "platform",
                    "repository",
                    "repository_impact",
                    "sdk",
                    "conformance",
                    "provider",
                    "governance",
                    "release",
                    "ci_evidence",
                    "execution_provenance",
                )
            },
            ci_provider_normalization=_optional_mapping(
                "developer delivery ci_provider_normalization",
                raw.get("ci_provider_normalization"),
            ),
            ci_provider_evidence=_optional_mapping(
                "developer delivery ci_provider_evidence",
                raw.get("ci_provider_evidence"),
            ),
            guarantees=_route_strings("developer delivery guarantees", raw.get("guarantees", [])),
            limitations=_route_strings("developer delivery limitations", raw.get("limitations", [])),
        )

    @property
    def explicitly_requested(self) -> bool:
        return self.release_request.present

    @property
    def ready_for_requested_release(self) -> bool:
        return self.release_request.present and self.release_request.ready

    @property
    def evidence_complete(self) -> bool:
        return all(value is not None for value in self.checks.values())

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def developer_delivery_audit_report(value: Mapping[str, Any]) -> DeveloperDeliveryAuditReport:
    """Parse a direct delivery audit result or an HTTP tool envelope."""

    return DeveloperDeliveryAuditReport.from_wire(
        _tool_payload(value, "developer_delivery_audit")
    )


__all__ = [
    "DeliveryTargetReport",
    "DeliveryReadinessReport",
    "DeliveryExternalSurfaceReport",
    "DeliveryReleaseRequestReport",
    "DeveloperDeliveryAuditReport",
    "developer_delivery_audit_report",
]
