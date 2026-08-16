from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalGroundingAuditArgs,
    BioevalGroundingAuditReport,
    Workspace,
    bioeval_grounding_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "claims": [{"id": "supported"}, {"id": "contested"}],
        "evidence": [
            {
                "id": "source",
                "last_modified": "2026-01-01T00:00:00Z",
                "lineage": ["specimen-1"],
                "locator_status": {"locator": "resolved", "digest": "sha256:source"},
            },
            {"id": "asserted", "last_modified": "2026-01-01T00:00:00Z"},
        ],
        "edges": [
            {"claim": "supported", "evidence": "source", "kind": "supports"},
            {"claim": "contested", "evidence": "source", "kind": "supports"},
            {"claim": "contested", "evidence": "asserted", "kind": "contradicts"},
        ],
        "stale_against": "2026-03-01T00:00:00Z",
        "max_items": 50,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-grounding-audit/0.1",
        "workflow": "bioeval_grounding_audit",
        "claims": {"rows": [], "returned": 0, "total": 2, "omitted": 0},
        "evidence": {"rows": [], "returned": 0, "total": 2, "omitted": 0},
        "edges": {"rows": [], "returned": 0, "total": 3, "omitted": 0},
        "census": {
            "claims": 2,
            "supported": 1,
            "contested": 1,
            "contradicted": 0,
            "unsupported": 0,
            "support_unverified": 0,
            "adjacent_citations": 0,
            "fully_grounded": False,
        },
        "graph": {"claim_count": 2, "evidence_count": 2, "edge_count": 3},
        "locator_census": {"resolved": 1, "not_checked": 1, "unresolvable": 0},
        "staleness": {"requested": True, "stale_count": 0},
        "findings": {"contested_claims": {"ids": ["contested"], "total": 1, "omitted": 0}},
        "guarantees": ["states remain distinct"],
        "limitations": ["no dereference"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalGroundingTests(unittest.TestCase):
    def test_args_preserve_typed_locator_and_graph_order(self) -> None:
        parsed = BioevalGroundingAuditArgs.from_wire(request())
        self.assertEqual(parsed.edges[1].kind, "supports")
        self.assertEqual(parsed.evidence[0].locator_status["locator"], "resolved")  # type: ignore[index]
        self.assertEqual(parsed.to_mcp_arguments()["claims"][1]["id"], "contested")
        with self.assertRaises(ArgumentError):
            BioevalGroundingAuditArgs.from_wire({**request(), "edges": [{"claim": "bad", "evidence": "source", "kind": "unknown"}]})
        with self.assertRaises(ArgumentError):
            BioevalGroundingAuditArgs.from_wire({**request(), "evidence": [{"id": "source", "last_modified": "bad"}, request()["evidence"][1]]})

    def test_report_preserves_partition_and_contested_findings(self) -> None:
        report = bioeval_grounding_audit_report(payload())
        self.assertIsInstance(report, BioevalGroundingAuditReport)
        self.assertTrue(report.accepted)
        self.assertFalse(report.fully_grounded)
        self.assertEqual(report.contested_claims, ("contested",))

    def test_all_facades_keep_grounding_audit_typed(self) -> None:
        parsed = BioevalGroundingAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_grounding_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_grounding_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_grounding_audit_report(parsed)
        self.assertEqual(report.census["contested"], 1)  # type: ignore[index]
        call.assert_called_once_with("bioeval_grounding_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_grounding_audit_report(parsed)
            self.assertEqual(report.locator_census["not_checked"], 1)  # type: ignore[index]
            async_call.assert_called_once_with("bioeval_grounding_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
