from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, patch

import pytest

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    ApiClient,
    ArgumentError,
    DomainEvidenceProviderNormalizationRequest,
    Workspace,
    domain_evidence_provider_normalization_report,
)


def request() -> DomainEvidenceProviderNormalizationRequest:
    return DomainEvidenceProviderNormalizationRequest(
        group_id="biological_domains",
        domains=("oncology",),
        subject_id="provider-subject",
        source_tool="literature_bind_check",
        connector_kind="literature",
        provider="pubmed",
        payload={"records": [{"id": "pmid:1"}]},
        request={"query": "oncology"},
        outcome="observed",
        source_plan_digest="a" * 64,
    )


def payload() -> dict:
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
        "intake": {"workflow": "domain_evidence_intake", "outcome": "observed"},
        "artifact_registry": {"indexed": True},
        "catalogue_digest": "d" * 64,
        "guarantees": ["structural"],
        "does_not_claim": ["provider authenticity"],
    }


def test_request_is_explicit_and_provider_report_preserves_digests() -> None:
    normalized = request().to_mcp_arguments()
    assert normalized["connector_kind"] == "literature"
    assert normalized["source_plan_digest"] == "a" * 64
    report = domain_evidence_provider_normalization_report(payload())
    assert report.payload_digest == "b" * 64
    assert report.request_digest == "c" * 64
    assert report.artifact_registry["indexed"] is True


def test_request_rejects_non_provider_connectors_and_scalar_payloads() -> None:
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderNormalizationRequest(**{**request().__dict__, "connector_kind": "file"})
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderNormalizationRequest(**{**request().__dict__, "payload": "not-an-object"})


def test_sync_http_and_workspace_helpers_parse_provider_normalization() -> None:
    with patch.object(ApiClient, "request", return_value=payload()) as rest:
        report = ApiClient("http://127.0.0.1:8787").domain_evidence_provider_normalize(request())
        assert report.provider == "pubmed"
        assert rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_normalize"
    with patch.object(ApiClient, "call_tool", return_value=payload()) as tool:
        report = ApiClient("http://127.0.0.1:8787").domain_evidence_provider_normalize_tool(request())
        assert report.outcome == "observed"
        assert tool.call_args.args[0] == "domain_evidence_provider_normalize"
    with patch.object(Workspace, "tool", return_value=payload()):
        assert Workspace(None).domain_evidence_provider_normalization_report(request()).provider == "pubmed"


def test_async_http_and_workspace_helpers_parse_provider_normalization() -> None:
    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "request", return_value=payload()):
            assert (await client.domain_evidence_provider_normalize(request())).provider == "pubmed"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (
                await AsyncWorkspace(None).domain_evidence_provider_normalization_report(request())
            ).provider == "pubmed"

    asyncio.run(run())
