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
    Workspace,
    domain_evidence_provider_external_payload_receipt_report,
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


def test_sync_and_async_external_receipt_helpers_preserve_rest_and_tool_routes() -> None:
    with patch.object(ApiClient, "request", return_value=payload()) as rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_receipt(request()).byte_length == 4096
        assert rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_external_payload_receipt"
    with patch.object(ApiClient, "call_tool", return_value=payload()) as tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_receipt_tool(request()).availability == "available"
        assert tool.call_args.args[0] == "domain_evidence_provider_external_payload_receipt"
    with patch.object(Workspace, "tool", return_value=payload()):
        assert Workspace(None).domain_evidence_provider_external_payload_receipt_report(request()).provider == "pubmed"

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "request", return_value=payload()):
            assert (await client.domain_evidence_provider_external_payload_receipt(request())).retention == "durable"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (
                await AsyncWorkspace(None).domain_evidence_provider_external_payload_receipt_report(request())
            ).byte_length == 4096

    asyncio.run(run())
