"""Bind cross-domain acquisition routes to bounded source-to-adapter projection.

The acquisition catalogue is a routing observation, not permission to interpret arbitrary bytes.
This module adds the missing binding step: a caller must name the exact group, domain, adapter,
catalogue digest, and source-plan digest. A truncated catalogue, scope mismatch, stale catalogue,
or cross-domain source envelope is refused before a parser sees any body.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from typing import Any, Mapping

from .adapter_runtime import AdapterRuntime
from .authoring import content_digest
from .domain_acquisition import DomainAcquisitionReport, domain_acquisition_report
from .errors import ArgumentError
from .source_adapter import (
    SourceAdapterProjectionRequest,
    SourceAdapterProjectionResult,
    SourceAdapterProjectionStatus,
    project_source_execution,
)

DOMAIN_EVIDENCE_PIPELINE_SCHEMA = "bioprism-python-domain-evidence-pipeline/0.1"
DOMAIN_EVIDENCE_PIPELINE_WORKFLOW = "domain_evidence_source_project_for_domain"
MAX_PIPELINE_LABEL_BYTES = 512


class DomainEvidencePipelineStatus(str, Enum):
    PROJECTED = "projected"
    SOURCE_PARTIAL = "source_partial"
    INVALID = "invalid"
    LOSSY = "lossy"
    BLOCKED = "blocked"
    REFUSED = "refused"


def _label(name: str, value: Any) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ArgumentError(f"{name} must be a non-empty string")
    if len(value.encode("utf-8")) > MAX_PIPELINE_LABEL_BYTES:
        raise ArgumentError(f"{name} exceeds the {MAX_PIPELINE_LABEL_BYTES}-byte bound")
    if any(ord(character) < 0x20 and character not in "\t " for character in value):
        raise ArgumentError(f"{name} must not contain control characters")
    return value


def _digest(name: str, value: Any) -> str:
    value = _label(name, value)
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _catalogue(value: DomainAcquisitionReport | Mapping[str, Any]) -> DomainAcquisitionReport:
    if isinstance(value, DomainAcquisitionReport):
        return value
    if not isinstance(value, Mapping):
        raise ArgumentError("domain acquisition catalogue must be an object")
    return domain_acquisition_report(value)


@dataclass(frozen=True)
class DomainEvidencePipelineRequest:
    """A fully explicit domain-bound source projection request."""

    group_id: str
    domain: str
    adapter_id: str
    catalogue_digest: str
    source_plan_digest: str
    source_id: str
    adapter_options: Mapping[str, Any] | None = None
    provenance: Mapping[str, Any] | None = None
    max_items: int = 1_000
    expected_raw_content_digest: str | None = None

    def __post_init__(self) -> None:
        _label("group_id", self.group_id)
        _label("domain", self.domain)
        _label("adapter_id", self.adapter_id)
        _digest("catalogue digest", self.catalogue_digest)
        _digest("source plan digest", self.source_plan_digest)
        _label("source_id", self.source_id)
        if self.adapter_options is not None and not isinstance(self.adapter_options, Mapping):
            raise ArgumentError("adapter_options must be an object")
        if self.provenance is not None and not isinstance(self.provenance, Mapping):
            raise ArgumentError("provenance must be an object")
        if isinstance(self.max_items, bool) or not isinstance(self.max_items, int) or not 1 <= self.max_items <= 1_000:
            raise ArgumentError("max_items must be between 1 and 1000")
        if self.expected_raw_content_digest is not None:
            _digest("expected raw content digest", self.expected_raw_content_digest)

    def to_projection_request(self) -> SourceAdapterProjectionRequest:
        return SourceAdapterProjectionRequest(
            adapter_id=self.adapter_id,
            source_id=self.source_id,
            adapter_options=self.adapter_options,
            provenance=self.provenance,
            max_items=self.max_items,
            expected_raw_content_digest=self.expected_raw_content_digest,
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "group_id": self.group_id,
            "domain": self.domain,
            "adapter_id": self.adapter_id,
            "catalogue_digest": self.catalogue_digest,
            "source_plan_digest": self.source_plan_digest,
            "source_id": self.source_id,
            "adapter_option_keys": sorted(str(key) for key in (self.adapter_options or {})),
            "provenance_present": self.provenance is not None,
            "max_items": self.max_items,
            "expected_raw_content_digest": self.expected_raw_content_digest,
        }


@dataclass(frozen=True)
class DomainEvidencePipelineResult:
    """Route-bound projection outcome with a digest of the complete decision envelope."""

    request: DomainEvidencePipelineRequest
    status: DomainEvidencePipelineStatus
    catalogue_digest: str
    source_plan_digest: str
    route: Mapping[str, Any] | None = None
    projection: SourceAdapterProjectionResult | None = None
    error: Mapping[str, Any] | None = None

    @property
    def projected(self) -> bool:
        return self.projection is not None and self.projection.projected and self.status in {
            DomainEvidencePipelineStatus.PROJECTED,
            DomainEvidencePipelineStatus.SOURCE_PARTIAL,
            DomainEvidencePipelineStatus.LOSSY,
        }

    @property
    def projection_digest(self) -> str:
        return content_digest(
            {
                "schema": DOMAIN_EVIDENCE_PIPELINE_SCHEMA,
                "request": self.request.to_wire(),
                "status": self.status.value,
                "catalogue_digest": self.catalogue_digest,
                "source_plan_digest": self.source_plan_digest,
                "route": self.route,
                "projection": self.projection.to_wire() if self.projection else None,
                "error": self.error,
            }
        )

    def to_wire(self) -> dict[str, Any]:
        return {
            "schema": DOMAIN_EVIDENCE_PIPELINE_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PIPELINE_WORKFLOW,
            "request": self.request.to_wire(),
            "status": self.status.value,
            "projected": self.projected,
            "catalogue_digest": self.catalogue_digest,
            "source_plan_digest": self.source_plan_digest,
            "route": dict(self.route) if self.route else None,
            "projection": self.projection.to_wire() if self.projection else None,
            "projection_digest": self.projection_digest,
            "error": dict(self.error) if self.error else None,
        }


def _route_for(
    report: DomainAcquisitionReport,
    request: DomainEvidencePipelineRequest,
) -> Mapping[str, Any] | None:
    if not report.complete or report.truncated:
        return None
    matches = [
        route
        for route in report.routes
        if route.group_id == request.group_id and route.domain == request.domain
    ]
    if len(matches) != 1:
        return None
    route = matches[0]
    if request.adapter_id not in route.adapter_ids:
        return None
    return route.to_dict()


def _refused(
    request: DomainEvidencePipelineRequest,
    *,
    error: Mapping[str, Any],
    route: Mapping[str, Any] | None = None,
) -> DomainEvidencePipelineResult:
    return DomainEvidencePipelineResult(
        request=request,
        status=DomainEvidencePipelineStatus.REFUSED,
        catalogue_digest=request.catalogue_digest,
        source_plan_digest=request.source_plan_digest,
        route=route,
        error=dict(error),
    )


def project_domain_source_execution(
    catalogue: DomainAcquisitionReport | Mapping[str, Any],
    execution: Mapping[str, Any],
    request: DomainEvidencePipelineRequest,
    *,
    runtime: AdapterRuntime | None = None,
) -> DomainEvidencePipelineResult:
    """Validate the exact catalogue route and source envelope before adapter dispatch."""

    if not isinstance(request, DomainEvidencePipelineRequest):
        raise ArgumentError("request must be a DomainEvidencePipelineRequest")
    if not isinstance(execution, Mapping):
        raise ArgumentError("source execution must be an object")
    report = _catalogue(catalogue)
    if report.digest != request.catalogue_digest:
        return _refused(
            request,
            error={
                "kind": "catalogue_digest_mismatch",
                "expected": request.catalogue_digest,
                "actual": report.digest,
            },
        )
    route = _route_for(report, request)
    if route is None:
        kind = "catalogue_incomplete_or_truncated" if not report.complete or report.truncated else "adapter_route_not_declared"
        return _refused(request, error={"kind": kind})

    source_plan_digest = execution.get("source_plan_digest")
    if source_plan_digest != request.source_plan_digest:
        return _refused(
            request,
            route=route,
            error={
                "kind": "source_plan_digest_mismatch",
                "expected": request.source_plan_digest,
                "actual": source_plan_digest,
            },
        )
    if execution.get("catalogue_digest") != request.catalogue_digest:
        return _refused(
            request,
            route=route,
            error={
                "kind": "source_catalogue_digest_mismatch",
                "expected": request.catalogue_digest,
                "actual": execution.get("catalogue_digest"),
            },
        )
    if execution.get("group_id") != request.group_id:
        return _refused(request, route=route, error={"kind": "source_group_mismatch"})
    domains = execution.get("domains")
    if not isinstance(domains, list) or request.domain not in domains:
        return _refused(request, route=route, error={"kind": "source_domain_mismatch"})

    projection = project_source_execution(
        execution,
        request.to_projection_request(),
        runtime=runtime,
    )
    if projection.status is SourceAdapterProjectionStatus.SOURCE_PARTIAL:
        status = DomainEvidencePipelineStatus.SOURCE_PARTIAL
    elif projection.status is SourceAdapterProjectionStatus.INVALID:
        status = DomainEvidencePipelineStatus.INVALID
    elif projection.status is SourceAdapterProjectionStatus.BLOCKED:
        status = DomainEvidencePipelineStatus.BLOCKED
    elif projection.status is SourceAdapterProjectionStatus.LOSSY:
        status = DomainEvidencePipelineStatus.LOSSY
    elif projection.status is SourceAdapterProjectionStatus.PROJECTED:
        status = DomainEvidencePipelineStatus.PROJECTED
    else:
        status = DomainEvidencePipelineStatus.REFUSED
    return DomainEvidencePipelineResult(
        request=request,
        status=status,
        catalogue_digest=request.catalogue_digest,
        source_plan_digest=request.source_plan_digest,
        route=route,
        projection=projection,
        error=projection.error,
    )


__all__ = [
    "DOMAIN_EVIDENCE_PIPELINE_SCHEMA",
    "DOMAIN_EVIDENCE_PIPELINE_WORKFLOW",
    "MAX_PIPELINE_LABEL_BYTES",
    "DomainEvidencePipelineRequest",
    "DomainEvidencePipelineResult",
    "DomainEvidencePipelineStatus",
    "project_domain_source_execution",
]
