from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, patch

import pytest

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    ApiClient,
    ArgumentError,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest,
    Workspace,
    domain_evidence_provider_external_payload_receipt_report,
    domain_evidence_provider_external_payload_replay_verification_report,
)


def request() -> DomainEvidenceProviderExternalPayloadReceiptRequest:
    return DomainEvidenceProviderExternalPayloadReceiptRequest(
        group_id="biological_domains",
        domains=("genomics", "oncology"),
        subject_id="external-provider-subject",
        source_tool="literature_bind_check",
        provider="pubmed",
        connector_kind="literature",
        handoff_digest="a" * 64,
        transfer_id="export-1",
        payload_digest="b" * 64,
        byte_length=4096,
        storage_backend="object_store",
        locator_kind="opaque",
        locator="store://caller/pubmed/objects/1",
        content_type="application/json",
        content_encoding="gzip",
        request_digest="c" * 64,
        parent_digests=("d" * 64,),
        availability="available",
        retention="durable",
        attempt_id="attempt-1",
    )


def payload() -> dict:
    receipt = request().to_mcp_arguments()
    receipt.update(
        {
            "schema": "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1",
            "workflow": "domain_evidence_provider_external_payload_receipt",
            "content_type": "application/json",
            "content_encoding": "gzip",
            "receipt_digest": "e" * 64,
            "execution": "not_started",
            "readiness_claimed": False,
            "guarantees": ["digest bound"],
            "limitations": ["caller store"],
        }
    )
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1",
        "workflow": "domain_evidence_provider_external_payload_receipt",
        "receipt": receipt,
        "handoff_digest": "a" * 64,
        "payload_digest": "b" * 64,
        "receipt_digest": "e" * 64,
        "artifact_registry": {"created": True, "indexed": True},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": ["digest bound"],
        "does_not_claim": ["caller store"],
    }


def replay_request() -> DomainEvidenceProviderExternalPayloadReplayRequest:
    receipt = request()
    return DomainEvidenceProviderExternalPayloadReplayRequest(
        receipt=receipt,
        expected_receipt_digest="e" * 64,
        expected_handoff_digest=receipt.handoff_digest,
        expected_payload_digest=receipt.payload_digest,
        expected_byte_length=receipt.byte_length,
    )


def replay_payload() -> dict:
    receipt_payload = payload()
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1",
        "workflow": "domain_evidence_provider_external_payload_replay_verify",
        "replay": {
            "schema": "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1",
            "workflow": "domain_evidence_provider_external_payload_replay_verify",
            "replay_status": "matched",
            "matched": True,
            "group_id": "biological_domains",
            "domains": ["genomics", "oncology"],
            "subject_id": "external-provider-subject",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "expected_receipt_digest": "e" * 64,
            "observed_receipt_digest": "e" * 64,
            "expected_handoff_digest": "a" * 64,
            "observed_handoff_digest": "a" * 64,
            "expected_payload_digest": "b" * 64,
            "observed_payload_digest": "b" * 64,
            "expected_byte_length": 4096,
            "observed_byte_length": 4096,
            "matches": {"byte_length": True, "handoff_digest": True, "payload_digest": True, "receipt_digest": True},
            "differences": [],
            "receipt": receipt_payload["receipt"],
            "replay_digest": "f" * 64,
            "guarantees": [],
            "limitations": [],
        },
        "matched": True,
        "replay_status": "matched",
        "replay_digest": "f" * 64,
        "artifact_registry": {"created": True, "indexed": True},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": [],
        "does_not_claim": ["store accessibility"],
    }


def test_external_receipt_is_out_of_line_and_rejects_payload_material() -> None:
    arguments = request().to_mcp_arguments()
    assert "payload" not in arguments
    assert arguments["byte_length"] == 4096
    report = domain_evidence_provider_external_payload_receipt_report(payload())
    assert report.receipt_digest == "e" * 64
    assert report.retention == "durable"

    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadReceiptRequest.from_wire(
            {**arguments, "credential_material": "never"}
        )
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadReceiptRequest.from_wire(
            {**arguments, "locator": "https://user:pass@example.org/object"}
        )


def test_external_receipt_replay_is_metadata_only_and_digest_bound() -> None:
    replay = replay_request()
    arguments = replay.to_mcp_arguments()
    assert arguments["expected_byte_length"] == 4096
    assert "payload" not in arguments
    report = domain_evidence_provider_external_payload_replay_verification_report(replay_payload())
    assert report.matched
    assert report.replay_status == "matched"
    assert report.matches["receipt_digest"] is True
    assert report.replay_digest == "f" * 64
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadReplayRequest.from_wire(
            {**arguments, "credential_material": "never"}
        )
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadReplayRequest.from_wire(
            {**arguments, "expected_byte_length": 0}
        )
def test_sync_and_async_external_receipt_helpers_preserve_rest_and_tool_routes() -> None:
    with patch.object(ApiClient, "request", return_value=payload()) as rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_receipt(request()).byte_length == 4096
        assert rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_external_payload_receipt"
    with patch.object(ApiClient, "call_tool", return_value=payload()) as tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_receipt_tool(request()).availability == "available"
        assert tool.call_args.args[0] == "domain_evidence_provider_external_payload_receipt"
    with patch.object(Workspace, "tool", return_value=payload()):
        assert Workspace(None).domain_evidence_provider_external_payload_receipt_report(request()).provider == "pubmed"
    with patch.object(ApiClient, "request", return_value=replay_payload()) as replay_rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_replay_verify(replay_request()).matched
        assert replay_rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_external_payload_replay_verify"
    with patch.object(ApiClient, "call_tool", return_value=replay_payload()) as replay_tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_replay_verify_tool(replay_request()).matched
        assert replay_tool.call_args.args[0] == "domain_evidence_provider_external_payload_replay_verify"
    with patch.object(Workspace, "tool", return_value=replay_payload()):
        assert Workspace(None).domain_evidence_provider_external_payload_replay_verify_report(replay_request()).matched

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "request", return_value=payload()):
            assert (await client.domain_evidence_provider_external_payload_receipt(request())).retention == "durable"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (
                await AsyncWorkspace(None).domain_evidence_provider_external_payload_receipt_report(request())
            ).byte_length == 4096
        with patch.object(ApiClient, "request", return_value=replay_payload()):
            assert (await client.domain_evidence_provider_external_payload_replay_verify(replay_request())).matched
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=replay_payload()):
            assert (await AsyncWorkspace(None).domain_evidence_provider_external_payload_replay_verify_report(replay_request())).replay_status == "matched"

    asyncio.run(run())
