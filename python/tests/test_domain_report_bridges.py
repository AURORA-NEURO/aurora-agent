from __future__ import annotations

from prism_sdk import (
    AdapterConformanceReport,
    AdapterExecutionEvidenceRequest,
    AdapterRuntime,
    ApiClient,
    AsyncApiClient,
    ProjectionRequest,
    Workspace,
    adapter_domain_report_arguments,
    AdapterDomainReportResult,
    domain_evidence_provider_normalization_report,
    domain_report_from_adapter_execution,
    domain_report_from_provider_normalization,
    evaluate_adapter_conformance,
)
from unittest.mock import patch


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def provider_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-normalization/0.1",
        "workflow": "domain_evidence_provider_normalize",
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "provider-subject",
        "source_tool": "literature_bind_check",
        "connector_kind": "literature",
        "provider": "pubmed",
        "outcome": "observed",
        "payload_digest": "b" * 64,
        "request_digest": "c" * 64,
        "response": {"provider": "pubmed", "payload_digest": "b" * 64},
        "normalization": {"payload_digest": "b" * 64},
        "shape_audit": {
            "schema": "bioprism-devplat-domain-evidence-provider-shape-audit/0.1",
            "status": "structured",
            "connector_kind": "literature",
            "root_kind": "object",
            "recognized_container": "records",
            "record_count": 1,
            "valid_record_count": 1,
            "invalid_record_count": 0,
            "identifier_coverage": {
                "candidate_fields": ["id"],
                "present_record_count": 1,
                "missing_record_count": 0,
            },
            "content_digest_coverage": None,
            "missing_fields": [],
            "warnings": [],
            "limitations": ["structural only"],
            "shape_digest": "e" * 64,
        },
        "record_index": {
            "schema": "bioprism-devplat-domain-evidence-provider-record-index/0.1",
            "connector_kind": "literature",
            "recognized_container": "records",
            "record_count": 1,
            "indexed_record_count": 1,
            "omitted_record_count": 0,
            "row_digests": ["9" * 64],
            "index_digest": "a" * 64,
            "limitations": ["digest-only"],
        },
        "intake": {"workflow": "domain_evidence_intake", "outcome": "observed", "intake_digest": "1" * 64},
        "artifact_registry": {"indexed": True},
        "catalogue_digest": "d" * 64,
        "guarantees": ["structural"],
        "does_not_claim": ["provider authenticity"],
    }


def adapter_evidence_request() -> AdapterExecutionEvidenceRequest:
    return AdapterExecutionEvidenceRequest(
        group_id="biological_domains",
        domains=("oncology",),
        subject_id="bridge-subject",
        adapter_id="bioprism.python.vcf_text",
        adapter_version="0.1.0",
        source_id="bridge-vcf",
        input_digest="a" * 64,
        output_digest="b" * 64,
        execution_status="succeeded",
        conformance_status="verified",
        semantic_loss_status="unknown",
        parent_digests=("c" * 64,),
    )


def adapter_domain_report_payload() -> dict:
    evidence = adapter_evidence_request().to_mcp_arguments()
    evidence.update(
        {
            "schema": "bioprism-devplat-adapter-execution-evidence/0.1",
            "workflow": "adapter_execution_evidence",
            "attestation_posture": "caller_asserted",
            "evidence_digest": "d" * 64,
        }
    )
    return {
        "ok": True,
        "schema": "bioprism-devplat-adapter-domain-report/0.1",
        "workflow": "adapter_domain_report",
        "evidence": {
            "ok": True,
            "schema": "bioprism-devplat-adapter-execution-evidence/0.1",
            "workflow": "adapter_execution_evidence",
            "evidence": evidence,
            "adapter": {
                "id": "bioprism.python.vcf_text",
                "version": "0.1.0",
                "execution": "python_delegated",
                "conformance_level": "normalize",
                "declared_loss_kinds": [],
                "scope_dimensions": ["subject", "variant"],
            },
            "evidence_digest": "d" * 64,
            "attestation_posture": "caller_asserted",
            "artifact_registry": {"indexed": True, "created": True},
            "execution": "not_started",
            "readiness_claimed": False,
        },
        "domain_report": {
            "ok": True,
            "schema": "bioprism-devplat-domain-report-project/0.1",
            "workflow": "domain_report_project",
            "report": {"kind": "adapter_execution"},
            "artifact_registry": {
                "indexed": True,
                "kind": "domain_report",
                "subject_id": "bridge-subject",
                "content_digest": "f" * 64,
            },
            "coverage": {"group_id": "biological_domains"},
            "readiness_claimed": False,
            "execution": "not_started",
        },
        "readiness_claimed": False,
        "execution": "not_started",
    }


def test_adapter_bridge_emits_canonical_review_bound_report_request() -> None:
    result = AdapterRuntime().execute(
        ProjectionRequest("bioprism.python.vcf_text", "bridge-vcf", {"text": VCF})
    )
    conformance = evaluate_adapter_conformance(result)
    request = domain_report_from_adapter_execution(
        result,
        "biological_domains",
        ("genomics",),
        subject_id="bridge-subject",
        input_digest="a" * 64,
        parent_digests=("c" * 64,),
        conformance=conformance,
    )
    arguments = request.to_arguments()
    assert request.source_tool == "adapter_execution_evidence"
    assert arguments["claim_posture"]["status"] == "observed"
    assert arguments["report"]["evidence"]["input_digest"] == "a" * 64
    assert arguments["report"]["conformance"]["report_digest"] == conformance.report_digest
    assert arguments["parent_digests"] == ["c" * 64]


def test_provider_bridge_preserves_refusal_posture_and_structural_lineage() -> None:
    report = domain_evidence_provider_normalization_report(provider_payload())
    request = domain_report_from_provider_normalization(
        report,
        "bioprism.python.fhir_manifest",
        "0.1.0",
        "provider-source",
        parent_digests=("2" * 64,),
    )
    arguments = request.to_arguments()
    assert request.source_tool == "domain_evidence_provider_normalize"
    assert arguments["claim_posture"]["status"] == "observed"
    assert arguments["report"]["external_payload"] is False
    assert arguments["report"]["evidence"]["parent_digests"][-1] == "2" * 64
    assert any(
        "provider authenticity" in item
        for item in arguments["claim_posture"]["does_not_claim"]
    )


def test_python_transport_facades_submit_the_cross_language_adapter_bridge() -> None:
    evidence = adapter_evidence_request()
    with patch.object(ApiClient, "call_tool", return_value=adapter_domain_report_payload()) as tool:
        result = ApiClient("http://127.0.0.1:8787").domain_report_from_adapter_execution(
            evidence, {"status": "verified"}
        )
    assert isinstance(result, AdapterDomainReportResult)
    assert result.domain_report.content_digest == "f" * 64
    assert tool.call_args.args[0] == "domain_report_project"
    assert tool.call_args.args[1] == adapter_domain_report_arguments(evidence, {"status": "verified"})

    with patch.object(Workspace, "tool", return_value=adapter_domain_report_payload()):
        assert (
            Workspace(None)
            .domain_report_from_adapter_execution(evidence)
            .evidence.adapter["id"]
            == "bioprism.python.vcf_text"
        )

    async def run() -> None:
        with patch.object(ApiClient, "call_tool", return_value=adapter_domain_report_payload()):
            result = await AsyncApiClient(ApiClient("http://127.0.0.1:8787")).domain_report_from_adapter_execution(evidence)
            assert result.evidence.evidence_digest == "d" * 64

    import asyncio

    asyncio.run(run())
