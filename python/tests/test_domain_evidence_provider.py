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
    DomainEvidenceProviderReplayRequest,
    Workspace,
    domain_evidence_provider_normalization_report,
    domain_evidence_provider_replay_verification_report,
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
                "candidate_fields": ["id", "pmid", "doi", "source_id"],
                "present_record_count": 1,
                "missing_record_count": 0,
            },
            "content_digest_coverage": None,
            "missing_fields": [],
            "warnings": [],
            "limitations": ["structural only"],
            "shape_digest": "e" * 64,
        },
        "intake": {"workflow": "domain_evidence_intake", "outcome": "observed"},
        "artifact_registry": {"indexed": True},
        "catalogue_digest": "d" * 64,
        "guarantees": ["structural"],
        "does_not_claim": ["provider authenticity"],
    }


def replay_payload() -> dict:
    shape = payload()["shape_audit"]
    replay = {
        "schema": "bioprism-devplat-domain-evidence-provider-replay/0.1",
        "workflow": "domain_evidence_provider_replay_verify",
        "replay_status": "matched",
        "matched": True,
        "group_id": "biological_domains",
        "domains": ["oncology"],
        "subject_id": "provider-subject",
        "source_tool": "literature_bind_check",
        "connector_kind": "literature",
        "provider": "pubmed",
        "expected_payload_digest": "b" * 64,
        "observed_payload_digest": "b" * 64,
        "expected_request_digest": "c" * 64,
        "observed_request_digest": "c" * 64,
        "expected_shape_digest": "e" * 64,
        "observed_shape_digest": "e" * 64,
        "expected_normalization_digest": "f" * 64,
        "observed_normalization_digest": "f" * 64,
        "expected_intake_digest": "1" * 64,
        "observed_intake_digest": "1" * 64,
        "matches": {"payload_digest": True, "request_digest": True, "shape_digest": True, "normalization_digest": True, "intake_digest": True},
        "differences": [],
        "shape_audit": shape,
        "replay_digest": "8" * 64,
        "guarantees": ["structural"],
        "limitations": ["no provider contact"],
    }
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-replay/0.1",
        "workflow": "domain_evidence_provider_replay_verify",
        "replay": replay,
        "matched": True,
        "replay_status": "matched",
        "replay_digest": "8" * 64,
        "artifact_registry": {"created": True, "indexed": True},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": ["indexed"],
        "does_not_claim": ["provider authenticity"],
    }


def replay_request() -> DomainEvidenceProviderReplayRequest:
    return DomainEvidenceProviderReplayRequest(
        observation=request(),
        expected_payload_digest="b" * 64,
        expected_request_digest="c" * 64,
        expected_shape_digest="e" * 64,
        expected_normalization_digest="f" * 64,
        expected_intake_digest="1" * 64,
    )


def test_request_is_explicit_and_provider_report_preserves_digests() -> None:
    normalized = request().to_mcp_arguments()
    assert normalized["connector_kind"] == "literature"
    assert normalized["source_plan_digest"] == "a" * 64
    report = domain_evidence_provider_normalization_report(payload())
    assert report.payload_digest == "b" * 64
    assert report.request_digest == "c" * 64
    assert report.artifact_registry["indexed"] is True
    assert report.shape_audit.status == "structured"
    assert report.shape_audit.identifier_coverage.present_record_count == 1
    replay = replay_request().to_mcp_arguments()
    assert replay["expected_intake_digest"] == "1" * 64
    replay_report = domain_evidence_provider_replay_verification_report(replay_payload())
    assert replay_report.matched is True
    assert replay_report.replay_status == "matched"
    assert replay_report.shape_audit.status == "structured"


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


def test_sync_and_async_provider_replay_helpers_parse_value_free_verification() -> None:
    with patch.object(ApiClient, "request", return_value=replay_payload()) as rest:
        report = ApiClient("http://127.0.0.1:8787").domain_evidence_provider_replay_verify(replay_request())
        assert report.matched is True
        assert rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_replay_verify"
    with patch.object(ApiClient, "call_tool", return_value=replay_payload()) as tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_replay_verify_tool(replay_request()).replay_digest == "8" * 64
        assert tool.call_args.args[0] == "domain_evidence_provider_replay_verify"
    with patch.object(Workspace, "tool", return_value=replay_payload()):
        assert Workspace(None).domain_evidence_provider_replay_verification_report(replay_request()).matched

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "request", return_value=replay_payload()):
            assert (await client.domain_evidence_provider_replay_verify(replay_request())).matched
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=replay_payload()):
            assert (
                await AsyncWorkspace(None).domain_evidence_provider_replay_verification_report(replay_request())
            ).replay_status == "matched"

    asyncio.run(run())
