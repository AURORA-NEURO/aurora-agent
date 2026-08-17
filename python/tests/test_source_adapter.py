from __future__ import annotations

import asyncio

import pytest

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    AdapterExecutionEvidenceRequest,
    ApiClient,
    ArgumentError,
    SourceAdapterProjectionRequest,
    SourceAdapterProjectionStatus,
    Workspace,
    project_source_execution,
)


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def execution(body_encoding: str = "utf8", *, outcome: str = "observed", truncated: bool = False) -> dict:
    retrieval = {
        "status": outcome,
        "body_encoding": body_encoding,
        "body_truncated": truncated,
        "raw_content_digest": "b" * 64,
    }
    if body_encoding == "utf8":
        retrieval["body"] = VCF
    return {
        "source_plan_digest": "a" * 64,
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "source-subject",
        "source_tool": "modality_catalog",
        "outcome": outcome,
        "retrieval_status": outcome,
        "raw_content_digest": "b" * 64,
        "response_digest": "c" * 64,
        "execution_result": {
            "response": {"retrieval": retrieval},
        },
    }


def request(**kwargs: object) -> SourceAdapterProjectionRequest:
    return SourceAdapterProjectionRequest(
        adapter_id="bioprism.python.vcf_text",
        source_id="vcf-source",
        **kwargs,
    )


def test_bridge_binds_transport_digests_into_a_real_adapter_projection() -> None:
    result = project_source_execution(execution(), request(provenance={"accession": "vcf-accession"}))
    assert result.status is SourceAdapterProjectionStatus.PROJECTED
    assert result.projected is True
    assert result.adapter_result is not None
    assert result.adapter_result.document_digest is not None
    assert result.adapter_result.request.source_context["source_plan_digest"] == "a" * 64
    assert len(result.projection_digest) == 64


def test_source_projection_bridges_parser_and_transport_lineage_into_evidence() -> None:
    result = project_source_execution(execution(), request())
    evidence = result.to_adapter_execution_evidence_request(
        "biological_domains",
        ("oncology",),
        subject_id="source-subject",
        input_digest="b" * 64,
        parent_digests=("d" * 64, "a" * 64),
        attempt_id="source-attempt-1",
    )
    assert isinstance(evidence, AdapterExecutionEvidenceRequest)
    assert evidence.adapter_id == "bioprism.python.vcf_text"
    assert evidence.execution_status == "partial"
    assert evidence.input_digest == "b" * 64
    assert evidence.parent_digests == ("a" * 64, "c" * 64, "d" * 64)
    assert evidence.attempt_id == "source-attempt-1"


def test_source_refusals_remain_evidence_bearing_without_parser_execution() -> None:
    result = project_source_execution(execution(truncated=True), request())
    evidence = result.to_adapter_execution_evidence_request(
        "biological_domains",
        ("oncology",),
        subject_id="source-subject",
        input_digest="b" * 64,
        adapter_version="0.1.0",
    )
    assert evidence.execution_status == "refused"
    assert evidence.conformance_status == "refused"
    assert evidence.semantic_loss_status == "unknown"
    assert evidence.error_code == "source_body_refused"
    with pytest.raises(ArgumentError):
        result.to_adapter_execution_evidence_request(
            "biological_domains", ("oncology",), subject_id="source-subject", input_digest="b" * 64
        )
    with pytest.raises(ArgumentError):
        result.to_adapter_execution_evidence_request(
            "biological_domains",
            ("oncology",),
            subject_id="source-subject",
            input_digest="d" * 64,
            adapter_version="0.1.0",
        )


def test_partial_source_stays_partial_even_when_the_parser_succeeds() -> None:
    result = project_source_execution(
        execution(outcome="partial"),
        request(provenance={"accession": "vcf-accession"}),
    )
    assert result.status is SourceAdapterProjectionStatus.SOURCE_PARTIAL
    assert result.source_outcome == "partial"


def test_parser_rejection_is_not_reported_as_a_successful_projection() -> None:
    result = project_source_execution(
        execution(),
        request(provenance={"not_allowed_by_vcf": "value"}),
    )
    assert result.status is SourceAdapterProjectionStatus.REFUSED
    assert result.adapter_result is not None
    assert result.adapter_result.status.value == "rejected"


def test_truncated_and_binary_bodies_are_explicit_refusals() -> None:
    truncated = project_source_execution(execution(truncated=True), request())
    assert truncated.status is SourceAdapterProjectionStatus.REFUSED
    assert truncated.error["kind"] == "source_body_refused"
    binary = project_source_execution(execution(body_encoding="binary"), request())
    assert binary.status is SourceAdapterProjectionStatus.REFUSED
    assert binary.error["kind"] == "source_body_refused"


def test_expected_raw_digest_mismatch_fails_before_adapter_dispatch() -> None:
    result = project_source_execution(
        execution(),
        request(expected_raw_content_digest="d" * 64),
    )
    assert result.status is SourceAdapterProjectionStatus.REFUSED
    assert result.error["kind"] == "raw_content_digest_mismatch"
    assert result.adapter_result is None


def test_refused_source_outcome_never_reaches_a_parser() -> None:
    result = project_source_execution(execution(body_encoding="omitted", outcome="refused"), request())
    assert result.status is SourceAdapterProjectionStatus.REFUSED
    assert result.error["kind"] == "source_outcome_not_projectable"


def test_http_and_workspace_facades_keep_projection_local() -> None:
    projection_request = request(provenance={"accession": "facade-accession"})
    api = ApiClient("http://127.0.0.1:8787")
    assert api.domain_evidence_source_project(execution(), projection_request).projected is True
    assert Workspace(None).domain_evidence_source_project(execution(), projection_request).projected is True


def test_async_facades_keep_projection_local() -> None:
    async def run() -> None:
        projection_request = request(provenance={"accession": "async-accession"})
        api = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        assert (await api.domain_evidence_source_project(execution(), projection_request)).projected is True
        assert (await AsyncWorkspace(None).domain_evidence_source_project(execution(), projection_request)).projected is True

    asyncio.run(run())
