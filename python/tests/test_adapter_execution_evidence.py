from __future__ import annotations

import asyncio
import json
from pathlib import Path
from unittest.mock import AsyncMock, patch

import pytest

from prism_sdk import (
    AdapterExecutionEvidenceReport,
    AdapterExecutionEvidenceRequest,
    AdapterExecutionLoss,
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    ArgumentError,
    Workspace,
    adapter_execution_evidence_report,
)


def request() -> AdapterExecutionEvidenceRequest:
    return AdapterExecutionEvidenceRequest(
        group_id="biological_domains",
        domains=("genomics",),
        subject_id="adapter-subject-1",
        adapter_id="bioprism.python.vcf_text",
        adapter_version="0.1.0",
        source_id="vcf-source-1",
        input_digest="a" * 64,
        output_digest="b" * 64,
        execution_status="succeeded",
        conformance_status="verified",
        semantic_loss_status="lossless",
        item_count=4,
        byte_length=128,
        parent_digests=("c" * 64,),
        attempt_id="attempt-1",
    )


def payload() -> dict:
    evidence = request().to_mcp_arguments()
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
        "schema": "bioprism-devplat-adapter-execution-evidence/0.1",
        "workflow": "adapter_execution_evidence",
        "evidence": evidence,
        "adapter": {
            "id": "bioprism.python.vcf_text",
            "version": "0.1.0",
            "execution": "python_delegated",
            "conformance_level": "normalize",
            "optional_dependency": None,
            "declared_loss_kinds": [],
            "scope_dimensions": ["subject", "sample", "variant", "genome"],
        },
        "evidence_digest": "d" * 64,
        "attestation_posture": "caller_asserted",
        "artifact_registry": {"indexed": True, "created": True, "kind": "adapter_execution_evidence"},
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": [],
        "does_not_claim": ["adapter execution by the MCP core"],
    }


def test_adapter_execution_request_preserves_loss_and_digest_boundaries() -> None:
    normalized = AdapterExecutionEvidenceRequest.from_wire(request().to_mcp_arguments())
    assert normalized.input_digest == "a" * 64
    assert normalized.to_mcp_arguments()["parent_digests"] == ["c" * 64]
    report = adapter_execution_evidence_report(payload())
    assert isinstance(report, AdapterExecutionEvidenceReport)
    assert report.execution_status == "succeeded"
    assert report.conformance_status == "verified"
    assert report.semantic_loss_status == "lossless"
    assert report.readiness_claimed is False

    lossy = request()
    with pytest.raises(ArgumentError):
        AdapterExecutionEvidenceRequest(
            **{**lossy.__dict__, "semantic_loss_status": "lossy"}
        )
    with pytest.raises(ArgumentError):
        AdapterExecutionEvidenceRequest(
            **{**lossy.__dict__, "execution_status": "refused", "error_code": None}
        )


def test_provider_normalization_fixture_round_trips_the_shared_request_contract() -> None:
    fixture_path = Path(__file__).parents[2] / "fixtures" / "adapter-execution-evidence" / "provider-normalization-request.json"
    fixture = json.loads(fixture_path.read_text(encoding="utf-8"))
    normalized = AdapterExecutionEvidenceRequest.from_wire(fixture)
    assert normalized.to_mcp_arguments() == fixture
    assert normalized.parent_digests[-1] == "2" * 64
    assert normalized.semantic_loss_status == "unknown"


def test_loss_entries_are_typed_and_bounded() -> None:
    loss = AdapterExecutionLoss("coordinate_frame_not_carried", "warning", "reference build was not retained")
    lossy = AdapterExecutionEvidenceRequest(
        **{
            **request().__dict__,
            "semantic_loss_status": "lossy",
            "losses": (loss,),
        }
    )
    assert lossy.to_mcp_arguments()["losses"][0]["severity"] == "warning"
    with pytest.raises(ArgumentError):
        AdapterExecutionLoss("loss", "invalid", "detail")


def test_sync_async_workspace_and_http_helpers_preserve_the_tool_route() -> None:
    with patch.object(ApiClient, "call_tool", return_value=payload()) as call_tool:
        report = ApiClient("http://127.0.0.1:8787").adapter_execution_evidence_report(request())
        assert report.evidence_digest == "d" * 64
        assert call_tool.call_args.args[0] == "adapter_execution_evidence"
    with patch.object(Workspace, "tool", return_value=payload()) as tool:
        assert Workspace(None).adapter_execution_evidence_report(request()).adapter["id"] == "bioprism.python.vcf_text"
        assert tool.call_args.args[0] == "adapter_execution_evidence"

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "call_tool", return_value=payload()):
            assert (await client.adapter_execution_evidence_report(request())).execution_status == "succeeded"
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (await AsyncWorkspace(None).adapter_execution_evidence_report(request())).semantic_loss_status == "lossless"

    asyncio.run(run())
