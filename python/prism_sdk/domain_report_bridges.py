"""Canonical domain-report bridges for adapter and provider evidence.

These helpers compose caller-owned runtime observations into the existing ``domain_report``
projection request.  They deliberately retain the adapter/provider evidence as a bounded report
payload while keeping claim posture, lineage, refusal state, and non-claims explicit.  They do
not execute adapters, reopen source locators, contact providers, or turn structural checks into
scientific, clinical, or release readiness.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping, Sequence

from .adapter_conformance import AdapterConformanceReport
from .adapter_execution_evidence import (
    AdapterExecutionEvidenceReport,
    AdapterExecutionEvidenceRequest,
    adapter_execution_evidence_report,
)
from .adapter_runtime import AdapterExecutionResult
from .domain_evidence_provider import DomainEvidenceProviderNormalizationReport
from .domain_evidence_provider import DomainEvidenceProviderNormalizationRequest
from .domain_evidence_provider_external import (
    DomainEvidenceProviderExternalPayloadNormalizationReport,
    DomainEvidenceProviderExternalPayloadNormalizationRequest,
)
from .domain_reports import DomainReportProjectReport, DomainReportProjectRequest
from .errors import ArgumentError

ADAPTER_DOMAIN_REPORT_SCHEMA = "bioprism-devplat-adapter-domain-report/0.1"
ADAPTER_DOMAIN_REPORT_WORKFLOW = "adapter_domain_report"
PROVIDER_DOMAIN_REPORT_SCHEMA = "bioprism-devplat-provider-domain-report/0.1"
PROVIDER_DOMAIN_REPORT_WORKFLOW = "provider_domain_report"


def adapter_domain_report_arguments(
    evidence: AdapterExecutionEvidenceRequest | Mapping[str, Any],
    conformance: Mapping[str, Any] | None = None,
) -> dict[str, Any]:
    """Build arguments for the MCP/REST adapter-domain-report operation."""

    normalized = (
        evidence
        if isinstance(evidence, AdapterExecutionEvidenceRequest)
        else AdapterExecutionEvidenceRequest.from_wire(evidence)
    )
    if conformance is not None and not isinstance(conformance, Mapping):
        raise ArgumentError("conformance must be an object")
    result: dict[str, Any] = {
        "operation": "from_adapter_execution",
        "evidence": normalized.to_mcp_arguments(),
    }
    if conformance is not None:
        result["conformance"] = dict(conformance)
    return result


@dataclass(frozen=True)
class AdapterDomainReportResult:
    """Typed result containing both retained adapter evidence and its domain report."""

    raw: dict[str, Any]
    evidence: AdapterExecutionEvidenceReport
    domain_report: DomainReportProjectReport
    readiness_claimed: bool = False

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "AdapterDomainReportResult":
        if not isinstance(value, Mapping):
            raise ArgumentError("adapter domain report response must be an object")
        raw = dict(value)
        if raw.get("ok") is not True:
            raise ArgumentError("adapter domain report response is not successful")
        if raw.get("schema") != ADAPTER_DOMAIN_REPORT_SCHEMA:
            raise ArgumentError("adapter domain report schema is invalid")
        if raw.get("workflow") != ADAPTER_DOMAIN_REPORT_WORKFLOW:
            raise ArgumentError("adapter domain report workflow is invalid")
        if raw.get("readiness_claimed") is not False or raw.get("execution") != "not_started":
            raise ArgumentError("adapter domain report posture is invalid")
        evidence = adapter_execution_evidence_report(raw.get("evidence"))
        domain_report = DomainReportProjectReport.from_wire(raw.get("domain_report"))
        return cls(raw, evidence, domain_report)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def provider_domain_report_arguments(
    normalization: DomainEvidenceProviderNormalizationRequest | Mapping[str, Any],
) -> dict[str, Any]:
    """Build arguments for inline provider normalization report composition."""

    if isinstance(normalization, DomainEvidenceProviderNormalizationRequest):
        normalized = normalization
    else:
        if not isinstance(normalization, Mapping):
            raise ArgumentError("provider normalization must be an object")
        raw = dict(normalization)
        required = {
            name: raw[name]
            for name in (
                "group_id",
                "domains",
                "subject_id",
                "source_tool",
                "connector_kind",
                "provider",
                "payload",
            )
        }
        required["domains"] = tuple(required["domains"])
        for name in (
            "request",
            "outcome",
            "claim_posture",
            "parent_digests",
            "source_plan_digest",
        ):
            if name in raw:
                required[name] = raw[name]
        if "parent_digests" in required:
            required["parent_digests"] = tuple(required["parent_digests"])
        normalized = DomainEvidenceProviderNormalizationRequest(**required)
    return {
        "operation": "from_provider_normalization",
        "normalization": normalized.to_mcp_arguments(),
    }


def external_provider_domain_report_arguments(
    normalization: DomainEvidenceProviderExternalPayloadNormalizationRequest
    | Mapping[str, Any],
) -> dict[str, Any]:
    """Build arguments for receipt-verified external provider report composition."""

    if isinstance(normalization, DomainEvidenceProviderExternalPayloadNormalizationRequest):
        normalized = normalization
    else:
        if not isinstance(normalization, Mapping):
            raise ArgumentError("external provider normalization must be an object")
        normalized = DomainEvidenceProviderExternalPayloadNormalizationRequest.from_wire(
            normalization
        )
    return {
        "operation": "from_external_provider_normalization",
        "normalization": normalized.to_mcp_arguments(),
    }


@dataclass(frozen=True)
class ProviderDomainReportResult:
    """Typed result containing provider normalization and its canonical domain report."""

    raw: dict[str, Any]
    mode: str
    normalization: (
        DomainEvidenceProviderNormalizationReport
        | DomainEvidenceProviderExternalPayloadNormalizationReport
    )
    domain_report: DomainReportProjectReport
    readiness_claimed: bool = False

    @classmethod
    def from_wire(cls, value: Mapping[str, Any]) -> "ProviderDomainReportResult":
        if not isinstance(value, Mapping):
            raise ArgumentError("provider domain report response must be an object")
        raw = dict(value)
        if raw.get("ok") is not True:
            raise ArgumentError("provider domain report response is not successful")
        if raw.get("schema") != PROVIDER_DOMAIN_REPORT_SCHEMA:
            raise ArgumentError("provider domain report schema is invalid")
        if raw.get("workflow") != PROVIDER_DOMAIN_REPORT_WORKFLOW:
            raise ArgumentError("provider domain report workflow is invalid")
        mode = raw.get("mode")
        if mode == "inline":
            normalization = DomainEvidenceProviderNormalizationReport.from_wire(
                raw.get("normalization")
            )
        elif mode == "external_payload":
            normalization = DomainEvidenceProviderExternalPayloadNormalizationReport.from_wire(
                raw.get("normalization")
            )
        else:
            raise ArgumentError("provider domain report mode is invalid")
        if raw.get("readiness_claimed") is not False or raw.get("execution") != "not_started":
            raise ArgumentError("provider domain report posture is invalid")
        return cls(
            raw=raw,
            mode=mode,
            normalization=normalization,
            domain_report=DomainReportProjectReport.from_wire(raw.get("domain_report")),
        )

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)


def _claim_status(execution_status: str) -> str:
    if execution_status in {"succeeded", "partial"}:
        return "observed"
    if execution_status in {"refused", "failed"}:
        return "refused"
    return "review_required"


def _claim_posture(execution_status: str, limitations: Sequence[str]) -> dict[str, Any]:
    return {
        "status": _claim_status(execution_status),
        "does_not_claim": [
            "adapter or provider correctness beyond the caller-supplied observation",
            "scientific, clinical, causal, provenance, regulatory, or release validity",
            "MCP-core execution, provider authenticity, or external-effect completion",
        ],
        "limitations": list(limitations),
    }


def _parent_tuple(values: Sequence[str]) -> tuple[str, ...]:
    if isinstance(values, (str, bytes)):
        raise ArgumentError("domain report bridge parent_digests must be a sequence")
    return tuple(values)


def _adapter_report_payload(
    evidence: AdapterExecutionEvidenceRequest,
    *,
    conformance: AdapterConformanceReport | None,
) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "kind": "adapter_execution",
        "evidence": evidence.to_mcp_arguments(),
    }
    if conformance is not None:
        payload["conformance"] = conformance.to_wire()
    return payload


def domain_report_from_adapter_execution(
    result: AdapterExecutionResult,
    group_id: str,
    domains: Sequence[str],
    *,
    subject_id: str,
    input_digest: str,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
    conformance: AdapterConformanceReport | None = None,
) -> DomainReportProjectRequest:
    """Build a canonical report request from one local adapter result.

    A supplied conformance report is checked against the runtime result and contributes both its
    profile/report lineage and bounded checks to the resulting report payload.  Without one, the
    runtime result remains explicit evidence and the claim posture is review-required where the
    result is not a terminal success/partial observation.
    """

    if not isinstance(result, AdapterExecutionResult):
        raise ArgumentError("result must be an AdapterExecutionResult")
    if conformance is not None:
        evidence = conformance.to_adapter_execution_evidence_request(
            result,
            group_id,
            tuple(domains),
            subject_id=subject_id,
            input_digest=input_digest,
            parent_digests=_parent_tuple(parent_digests),
            attempt_id=attempt_id,
        )
    else:
        evidence = result.to_adapter_execution_evidence_request(
            group_id,
            tuple(domains),
            subject_id=subject_id,
            input_digest=input_digest,
            parent_digests=_parent_tuple(parent_digests),
            attempt_id=attempt_id,
        )
    limitations = (
        "runtime adapter execution is caller-owned and not performed by the MCP core",
        "the report retains structural adapter evidence rather than asserting domain truth",
    )
    return DomainReportProjectRequest(
        group_id=group_id,
        domains=tuple(domains),
        subject_id=subject_id,
        # The canonical report boundary validates this against the capability catalogue. The
        # adapter identity remains inside the evidence payload, while this is the stable
        # cross-domain transport membership.
        source_tool="adapter_execution_evidence",
        report=_adapter_report_payload(evidence, conformance=conformance),
        claim_posture=_claim_posture(evidence.execution_status, limitations),
        parent_digests=_parent_tuple(parent_digests),
    )


def _provider_report_payload(
    evidence: AdapterExecutionEvidenceRequest,
    provider_report: Any,
    *,
    external: bool,
) -> dict[str, Any]:
    return {
        "kind": "provider_normalization",
        "evidence": evidence.to_mcp_arguments(),
        "provider_normalization": provider_report.to_dict(),
        "external_payload": external,
    }


def domain_report_from_provider_normalization(
    report: DomainEvidenceProviderNormalizationReport,
    adapter_id: str,
    adapter_version: str,
    source_id: str,
    *,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
) -> DomainReportProjectRequest:
    """Build a canonical report request from an in-line provider normalization report."""

    if not isinstance(report, DomainEvidenceProviderNormalizationReport):
        raise ArgumentError("report must be a DomainEvidenceProviderNormalizationReport")
    evidence = report.to_adapter_execution_evidence_request(
        adapter_id,
        adapter_version,
        source_id,
        parent_digests=_parent_tuple(parent_digests),
        attempt_id=attempt_id,
    )
    limitations = (
        "provider normalization is caller-supplied and does not authenticate the provider",
        "payload shape and record indexing do not establish scientific or clinical validity",
        "the MCP core does not contact the provider or execute the adapter",
    )
    return DomainReportProjectRequest(
        group_id=report.group_id,
        domains=report.domains,
        subject_id=report.subject_id,
        source_tool="domain_evidence_provider_normalize",
        report=_provider_report_payload(evidence, report, external=False),
        claim_posture=_claim_posture(evidence.execution_status, limitations),
        parent_digests=_parent_tuple(parent_digests),
    )


def domain_report_from_external_provider_normalization(
    report: DomainEvidenceProviderExternalPayloadNormalizationReport,
    adapter_id: str,
    adapter_version: str,
    source_id: str,
    *,
    parent_digests: Sequence[str] = (),
    attempt_id: str | None = None,
) -> DomainReportProjectRequest:
    """Build a report request while retaining receipt/materialization lineage."""

    if not isinstance(report, DomainEvidenceProviderExternalPayloadNormalizationReport):
        raise ArgumentError(
            "report must be a DomainEvidenceProviderExternalPayloadNormalizationReport"
        )
    evidence = report.to_adapter_execution_evidence_request(
        adapter_id,
        adapter_version,
        source_id,
        parent_digests=_parent_tuple(parent_digests),
        attempt_id=attempt_id,
    )
    limitations = (
        "external payload materialization is caller-supplied and the locator remains unopened",
        "receipt and normalization lineage do not establish payload or provider authenticity",
        "the MCP core does not execute a connector, adapter, or external effect",
    )
    return DomainReportProjectRequest(
        group_id=report.normalization.group_id,
        domains=report.normalization.domains,
        subject_id=report.normalization.subject_id,
        source_tool="domain_evidence_provider_external_payload_normalize",
        report=_provider_report_payload(evidence, report, external=True),
        claim_posture=_claim_posture(evidence.execution_status, limitations),
        parent_digests=_parent_tuple(parent_digests),
    )


__all__ = [
    "ADAPTER_DOMAIN_REPORT_SCHEMA",
    "ADAPTER_DOMAIN_REPORT_WORKFLOW",
    "AdapterDomainReportResult",
    "adapter_domain_report_arguments",
    "PROVIDER_DOMAIN_REPORT_SCHEMA",
    "PROVIDER_DOMAIN_REPORT_WORKFLOW",
    "ProviderDomainReportResult",
    "provider_domain_report_arguments",
    "external_provider_domain_report_arguments",
    "domain_report_from_adapter_execution",
    "domain_report_from_provider_normalization",
    "domain_report_from_external_provider_normalization",
]
