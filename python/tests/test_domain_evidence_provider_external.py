from __future__ import annotations

import asyncio
import json
from dataclasses import replace
from unittest.mock import AsyncMock, patch

import pytest

from prism_sdk import (
    AsyncApiClient,
    AsyncWorkspace,
    ApiClient,
    ArgumentError,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
    DomainEvidenceProviderExternalPayloadReplayRequest,
    DomainEvidenceProviderExternalPayloadNormalizationRequest,
    DomainEvidenceProviderExternalPayloadLineageAuditRequest,
    Workspace,
    domain_evidence_provider_external_payload_receipt_report,
    domain_evidence_provider_external_payload_replay_verification_report,
    domain_evidence_provider_external_payload_normalization_report,
    domain_evidence_provider_external_payload_lineage_audit_report,
)
from prism_sdk.authoring import content_digest


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


def lineage_request() -> DomainEvidenceProviderExternalPayloadLineageAuditRequest:
    return DomainEvidenceProviderExternalPayloadLineageAuditRequest(receipt=request())


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


def normalization_request() -> DomainEvidenceProviderExternalPayloadNormalizationRequest:
    materialized = {"records": [{"id": "pmid:1", "title": "opaque"}]}
    receipt = replace(
        request(),
        payload_digest=content_digest(materialized),
        byte_length=len(json.dumps(materialized, separators=(",", ":"), ensure_ascii=False).encode()),
        content_encoding=None,
    )
    return DomainEvidenceProviderExternalPayloadNormalizationRequest(
        receipt=receipt,
        payload=materialized,
        outcome="observed",
    )


def normalization_payload() -> dict:
    normalized = normalization_request()
    receipt = normalized.receipt.to_mcp_arguments()
    receipt.update(
        {
            "schema": "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1",
            "workflow": "domain_evidence_provider_external_payload_receipt",
            "receipt_digest": "e" * 64,
            "execution": "not_started",
            "readiness_claimed": False,
            "guarantees": [],
            "limitations": [],
        }
    )
    shape = {
        "schema": "bioprism-devplat-domain-evidence-provider-shape-audit/0.1",
        "status": "structured",
        "connector_kind": "literature",
        "root_kind": "object",
        "recognized_container": "records",
        "record_count": 1,
        "valid_record_count": 1,
        "invalid_record_count": 0,
        "identifier_coverage": {"candidate_fields": ["id"], "present_record_count": 1, "missing_record_count": 0},
        "content_digest_coverage": None,
        "missing_fields": [],
        "warnings": [],
        "limitations": [],
        "shape_digest": "f" * 64,
    }
    record_index = {
        "schema": "bioprism-devplat-domain-evidence-provider-record-index/0.1",
        "connector_kind": "literature",
        "recognized_container": "records",
        "record_count": 1,
        "indexed_record_count": 1,
        "omitted_record_count": 0,
        "row_digests": ["7" * 64],
        "index_digest": "8" * 64,
        "limitations": [],
    }
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-external-payload-normalization/0.1",
        "workflow": "domain_evidence_provider_external_payload_normalize",
        "group_id": "biological_domains",
        "domains": ["genomics", "oncology"],
        "subject_id": "external-provider-subject",
        "source_tool": "literature_bind_check",
        "connector_kind": "literature",
        "provider": "pubmed",
        "outcome": "observed",
        "payload_digest": normalized.receipt.payload_digest,
        "request_digest": None,
        "response": {"provider": "pubmed", "connector_kind": "literature", "source": "caller_supplied", "authenticated": False, "payload": normalized.payload},
        "shape_audit": shape,
        "record_index": record_index,
        "normalization": {"payload_digest": normalized.receipt.payload_digest},
        "receipt": receipt,
        "receipt_digest": "e" * 64,
        "materialized_payload_digest": normalized.receipt.payload_digest,
        "materialization": {"mode": "canonical_json", "matched": True, "materialized_payload_digest": normalized.receipt.payload_digest, "locator_opened": False},
        "intake": {"intake_digest": "i" * 64},
        "artifact_registry": {"created": True, "indexed": True},
        "receipt_artifact_registry": {"ok": True, "created": True, "indexed": True},
        "catalogue_digest": "9" * 64,
        "readiness_claimed": False,
        "execution": "not_started",
        "guarantees": [],
        "does_not_claim": ["provider authenticity"],
    }


def lineage_payload() -> dict:
    receipt_payload = payload()
    receipt = receipt_payload["receipt"]
    audit = {
        "schema": "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1",
        "workflow": "domain_evidence_provider_external_payload_lineage_audit",
        "lineage_status": "matched",
        "group_id": receipt["group_id"],
        "domains": receipt["domains"],
        "subject_id": receipt["subject_id"],
        "source_tool": receipt["source_tool"],
        "provider": receipt["provider"],
        "connector_kind": receipt["connector_kind"],
        "receipt": receipt,
        "handoff": {"handoff_digest": receipt["handoff_digest"], "status": "prepared"},
        "matches": {"handoff_present": True, "handoff_digest": True, "payload_digest": True},
        "differences": [],
        "payload_binding_status": "matched",
        "lineage_digest": "1" * 64,
        "guarantees": [],
        "limitations": [],
    }
    return {
        "ok": True,
        "schema": "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1",
        "workflow": "domain_evidence_provider_external_payload_lineage_audit",
        "audit": audit,
        "lineage_status": "matched",
        "payload_binding_status": "matched",
        "lineage_digest": "1" * 64,
        "receipt_registry": {"ok": True, "created": True},
        "artifact_registry": {"ok": True, "created": True},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": [],
        "does_not_claim": ["provider authenticity"],
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


def test_external_materialization_bridge_requires_explicit_digest_match() -> None:
    normalized = normalization_request()
    arguments = normalized.to_mcp_arguments()
    assert arguments["payload_digest"] == content_digest(normalized.payload)
    assert "payload" in arguments
    report = domain_evidence_provider_external_payload_normalization_report(normalization_payload())
    assert report.materialized_payload_digest == report.normalization.payload_digest
    assert report.materialization["locator_opened"] is False
    assert report.receipt_artifact_registry["indexed"] is True
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadNormalizationRequest.from_wire(
            {**arguments, "credential_material": "never"}
        )
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadReplayRequest.from_wire(
            {**arguments, "expected_byte_length": 0}
        )


def test_external_lineage_audit_is_registry_bound_and_preserves_orchestration_boundaries() -> None:
    normalized = DomainEvidenceProviderExternalPayloadLineageAuditRequest.from_wire(request().to_mcp_arguments())
    assert normalized.to_mcp_arguments()["handoff_digest"] == request().handoff_digest
    report = domain_evidence_provider_external_payload_lineage_audit_report(lineage_payload())
    assert report.lineage_status == "matched"
    assert report.payload_binding_status == "matched"
    assert report.matches["payload_digest"] is True
    assert report.readiness_claimed is False
    with pytest.raises(ArgumentError):
        DomainEvidenceProviderExternalPayloadLineageAuditRequest.from_wire(
            {**request().to_mcp_arguments(), "credential_material": "never"}
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
    with patch.object(ApiClient, "request", return_value=normalization_payload()) as normalize_rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_normalize(normalization_request()).normalization.payload_digest == normalization_request().receipt.payload_digest
        assert normalize_rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_external_payload_normalize"
    with patch.object(ApiClient, "call_tool", return_value=normalization_payload()) as normalize_tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_normalize_tool(normalization_request()).readiness_claimed is False
        assert normalize_tool.call_args.args[0] == "domain_evidence_provider_external_payload_normalize"
    with patch.object(Workspace, "tool", return_value=normalization_payload()):
        assert Workspace(None).domain_evidence_provider_external_payload_normalize_report(normalization_request()).receipt_digest == "e" * 64
    with patch.object(ApiClient, "request", return_value=lineage_payload()) as lineage_rest:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_lineage_audit(lineage_request()).lineage_status == "matched"
        assert lineage_rest.call_args.args[1] == "/v1/tools/domain_evidence_provider_external_payload_lineage_audit"
    with patch.object(ApiClient, "call_tool", return_value=lineage_payload()) as lineage_tool:
        assert ApiClient("http://127.0.0.1:8787").domain_evidence_provider_external_payload_lineage_audit_tool(lineage_request()).payload_binding_status == "matched"
        assert lineage_tool.call_args.args[0] == "domain_evidence_provider_external_payload_lineage_audit"
    with patch.object(Workspace, "tool", return_value=lineage_payload()):
        assert Workspace(None).domain_evidence_provider_external_payload_lineage_audit_report(lineage_request()).lineage_status == "matched"

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
        with patch.object(ApiClient, "request", return_value=normalization_payload()):
            assert (await client.domain_evidence_provider_external_payload_normalize(normalization_request())).materialized_payload_digest == normalization_request().receipt.payload_digest
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=normalization_payload()):
            assert (await AsyncWorkspace(None).domain_evidence_provider_external_payload_normalize_report(normalization_request())).receipt_digest == "e" * 64
        with patch.object(ApiClient, "request", return_value=lineage_payload()):
            assert (await client.domain_evidence_provider_external_payload_lineage_audit(lineage_request())).lineage_status == "matched"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=lineage_payload()):
            assert (await AsyncWorkspace(None).domain_evidence_provider_external_payload_lineage_audit_report(lineage_request())).payload_binding_status == "matched"

    asyncio.run(run())
