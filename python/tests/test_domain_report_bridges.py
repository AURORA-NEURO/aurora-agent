from __future__ import annotations

from prism_sdk import (
    AdapterConformanceReport,
    AdapterRuntime,
    ProjectionRequest,
    domain_evidence_provider_normalization_report,
    domain_report_from_adapter_execution,
    domain_report_from_provider_normalization,
    evaluate_adapter_conformance,
)


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
    assert request.source_tool == "bioprism.python.vcf_text"
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
    assert request.source_tool == "literature:pubmed"
    assert arguments["claim_posture"]["status"] == "observed"
    assert arguments["report"]["external_payload"] is False
    assert arguments["report"]["evidence"]["parent_digests"][-1] == "2" * 64
    assert any(
        "provider authenticity" in item
        for item in arguments["claim_posture"]["does_not_claim"]
    )
