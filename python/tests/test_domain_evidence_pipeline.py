from __future__ import annotations

import asyncio

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    ApiClient,
    DomainEvidencePipelineRequest,
    DomainEvidencePipelineStatus,
    Workspace,
    domain_acquisition_report,
    project_domain_source_execution,
)


VCF = """##fileformat=VCFv4.3
##reference=GRCh38
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t10\t.\tA\tG\t50\tPASS\tDP=4
"""


def catalogue(*, complete: bool = True, truncated: bool = False) -> dict:
    route = {
        "group_id": "biological_domains",
        "domain": "oncology",
        "declared_tool_count": 8,
        "transport": {"status": "bounded_file_http"},
        "interpretation": {
            "status": "python_delegated",
            "adapter_ids": ["bioprism.python.vcf_text"],
        },
        "limitations": ["scope labels are not ontology validation"],
    }
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-acquisition/0.1",
        "workflow": "domain_acquisition_catalogue",
        "execution": "not_started",
        "catalogue": {
            "schema": "bioprism-devplat-domain-acquisition/0.1",
            "workflow": "domain_acquisition_catalogue",
            "digest": "a" * 64,
            "complete": complete,
            "truncated": truncated,
            "selected_domain_count": 1,
            "routes": [route],
        },
        "guarantees": [],
        "does_not_claim": ["scientific truth"],
    }


def execution(*, outcome: str = "observed", group_id: str = "biological_domains", domain: str = "oncology") -> dict:
    retrieval = {
        "status": outcome,
        "body_encoding": "utf8",
        "body_truncated": False,
        "raw_content_digest": "b" * 64,
        "body": VCF,
    }
    return {
        "source_plan_digest": "c" * 64,
        "catalogue_digest": "a" * 64,
        "group_id": group_id,
        "domains": [domain],
        "subject_id": "source-subject",
        "source_tool": "modality_catalog",
        "outcome": outcome,
        "retrieval_status": outcome,
        "raw_content_digest": "b" * 64,
        "response_digest": "d" * 64,
        "execution_result": {"response": {"retrieval": retrieval}},
    }


def request(**kwargs: object) -> DomainEvidencePipelineRequest:
    return DomainEvidencePipelineRequest(
        group_id="biological_domains",
        domain="oncology",
        adapter_id="bioprism.python.vcf_text",
        catalogue_digest="a" * 64,
        source_plan_digest="c" * 64,
        source_id="vcf-source",
        provenance={"accession": "pipeline-accession"},
        **kwargs,
    )


def test_pipeline_requires_catalogue_declared_route_and_binds_both_digests() -> None:
    result = project_domain_source_execution(
        domain_acquisition_report(catalogue()),
        execution(),
        request(),
    )
    assert result.status is DomainEvidencePipelineStatus.PROJECTED
    assert result.projected is True
    assert result.route["group_id"] == "biological_domains"
    assert result.projection.source_plan_digest == "c" * 64
    assert len(result.projection_digest) == 64


def test_partial_transport_remains_partial_at_domain_pipeline_boundary() -> None:
    result = project_domain_source_execution(
        domain_acquisition_report(catalogue()),
        execution(outcome="partial"),
        request(),
    )
    assert result.status is DomainEvidencePipelineStatus.SOURCE_PARTIAL


def test_incomplete_catalogue_is_refused_before_parser_dispatch() -> None:
    result = project_domain_source_execution(
        domain_acquisition_report(catalogue(complete=False, truncated=True)),
        execution(),
        request(),
    )
    assert result.status is DomainEvidencePipelineStatus.REFUSED
    assert result.error["kind"] == "catalogue_incomplete_or_truncated"
    assert result.projection is None


def test_scope_and_identity_mismatches_are_refused() -> None:
    wrong_domain = project_domain_source_execution(
        domain_acquisition_report(catalogue()),
        execution(domain="immunology"),
        request(),
    )
    assert wrong_domain.error["kind"] == "source_domain_mismatch"
    wrong_plan = project_domain_source_execution(
        domain_acquisition_report(catalogue()),
        execution(),
        DomainEvidencePipelineRequest(
            **{**request().__dict__, "source_plan_digest": "e" * 64},
        ),
    )
    assert wrong_plan.error["kind"] == "source_plan_digest_mismatch"


def test_facades_expose_the_domain_bound_local_handoff() -> None:
    report = domain_acquisition_report(catalogue())
    api = ApiClient("http://127.0.0.1:8787")
    assert api.domain_evidence_source_project_for_domain(report, execution(), request()).projected is True
    assert Workspace(None).domain_evidence_source_project_for_domain(report, execution(), request()).projected is True


def test_async_facades_expose_the_domain_bound_local_handoff() -> None:
    async def run() -> None:
        report = domain_acquisition_report(catalogue())
        api = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        assert (await api.domain_evidence_source_project_for_domain(report, execution(), request())).projected is True
        assert (await AsyncWorkspace(None).domain_evidence_source_project_for_domain(report, execution(), request())).projected is True

    asyncio.run(run())
