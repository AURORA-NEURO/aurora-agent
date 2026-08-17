from __future__ import annotations

import asyncio
from unittest.mock import AsyncMock, patch

from prism_sdk import (
    AdapterExecutionEvidenceQueryRequest,
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    Workspace,
    adapter_execution_evidence_query_report,
)


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-devplat-adapter-execution-evidence-query/0.1",
        "workflow": "adapter_execution_evidence_query",
        "filters": {"adapter_id": "bioprism.python.vcf_text", "max_items": 1, "include_artifacts": True},
        "registry_generation": 4,
        "registry_size": 3,
        "rows": [
            {
                "row_digest": "a" * 64,
                "content_digest": "b" * 64,
                "evidence_digest": "c" * 64,
                "subject_id": "subject-1",
                "group_id": "biological_domains",
                "domains": ["genomics"],
                "adapter_id": "bioprism.python.vcf_text",
                "adapter_version": "0.1.0",
                "source_id": "vcf-1",
                "input_digest": "d" * 64,
                "output_digest": "e" * 64,
                "execution_status": "succeeded",
                "conformance_status": "verified",
                "semantic_loss_status": "lossless",
                "loss_count": 0,
                "parent_digests": ["f" * 64],
                "attestation_posture": "caller_asserted",
                "join_status": "source_bound",
                "joins": {
                    "source_plan_digests": ["f" * 64],
                    "intake_digests": [],
                    "external_payload_digests": [],
                    "workflow_reconciliation_digests": [],
                    "missing_parent_digests": [],
                    "unclassified_parent_digests": [],
                    "source_bound": True,
                    "workflow_bound": False,
                },
                "evidence_artifact": {"evidence_digest": "c" * 64},
            }
        ],
        "next_after": None,
        "has_more": False,
        "query_digest": "1" * 64,
        "execution": "not_started",
        "readiness_claimed": False,
        "guarantees": [],
        "limitations": [],
    }


def request() -> AdapterExecutionEvidenceQueryRequest:
    return AdapterExecutionEvidenceQueryRequest(
        adapter_id="bioprism.python.vcf_text",
        execution_status="succeeded",
        max_items=1,
        include_artifacts=True,
    )


def test_query_report_preserves_join_and_cursor_posture() -> None:
    report = adapter_execution_evidence_query_report(payload())
    assert report.rows[0]["join_status"] == "source_bound"
    assert report.rows[0]["joins"]["source_bound"] is True
    assert report.readiness_claimed is False
    assert request().to_mcp_arguments()["adapter_id"] == "bioprism.python.vcf_text"


def test_sync_async_workspace_and_http_helpers_route_query() -> None:
    with patch.object(ApiClient, "call_tool", return_value=payload()) as call_tool:
        report = ApiClient("http://127.0.0.1:8787").adapter_execution_evidence_query_report(request())
        assert report.query_digest == "1" * 64
        assert call_tool.call_args.args[0] == "adapter_execution_evidence_query"
    with patch.object(Workspace, "tool", return_value=payload()) as tool:
        assert Workspace(None).adapter_execution_evidence_query_report(request()).registry_size == 3
        assert tool.call_args.args[0] == "adapter_execution_evidence_query"

    async def run() -> None:
        client = AsyncApiClient(ApiClient("http://127.0.0.1:8787"))
        with patch.object(ApiClient, "call_tool", return_value=payload()):
            assert (await client.adapter_execution_evidence_query_report(request())).has_more is False
        with patch.object(AsyncWorkspace, "tool", new_callable=AsyncMock, return_value=payload()):
            assert (await AsyncWorkspace(None).adapter_execution_evidence_query_report(request())).rows[0]["source_id"] == "vcf-1"

    asyncio.run(run())
