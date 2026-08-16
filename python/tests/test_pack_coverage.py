from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PackCoverageAuditArgs,
    PackCoverageAuditReport,
    Workspace,
    pack_coverage_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def coverage_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/pack-coverage-audit/0.1",
        "section": "15",
        "selected_pack_count": 25,
        "selected_pack_ids": [f"pack-{index}" for index in range(25)],
        "summary": {
            "families": 8,
            "covered": 6,
            "uncovered": 2,
            "singly_covered": 3,
            "weakly_covered": 4,
            "coverage_fraction": 0.75,
            "gap_summary": "2 capability families are uncovered; 3 are singly covered",
        },
        "rows": [
            {"family": "choose evidence", "packs": ["prism.context-acquisition"], "execution_grounded": True},
            {"family": "audit claims", "packs": [], "execution_grounded": False},
        ],
        "rows_omitted": 6,
        "uncovered": ["audit claims", "manage abstention"],
        "uncovered_omitted": 0,
        "singly_covered": ["choose evidence", "rank options", "select assay"],
        "singly_covered_omitted": 0,
        "weakly_covered": ["choose evidence", "audit claims", "rank options", "select assay"],
        "weakly_covered_omitted": 0,
        "matrix": [{"family": "choose evidence", "pack_id": "prism.context-acquisition", "covered": True}],
        "matrix_omitted": 24,
        "guarantees": ["coverage is computed by the packs kernel over the selected portfolio"],
        "limitations": ["declaration-level portfolio coverage; no instances were executed"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(coverage_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(coverage_payload())}]})


class PackCoverageTests(unittest.TestCase):
    def test_args_bound_sections_ids_and_projection_size(self) -> None:
        request = PackCoverageAuditArgs.from_wire({"section": "29", "pack_ids": ["pack-a", "pack-b"], "max_items": 3})
        self.assertEqual(request.to_mcp_arguments(), {"section": "29", "pack_ids": ["pack-a", "pack-b"], "max_items": 3})
        with self.assertRaises(ArgumentError):
            PackCoverageAuditArgs("bad")
        with self.assertRaises(ArgumentError):
            PackCoverageAuditArgs("all", ("pack-a", "pack-a"))
        with self.assertRaises(ArgumentError):
            PackCoverageAuditArgs("all", (), 0)

    def test_report_preserves_gaps_matrix_and_declaration_limitations(self) -> None:
        report = pack_coverage_audit_report(coverage_payload())
        self.assertIsInstance(report, PackCoverageAuditReport)
        self.assertTrue(report.accepted)
        self.assertFalse(report.refused)
        self.assertEqual(report.selected_pack_count, 25)
        self.assertEqual(report.coverage_fraction, 0.75)
        self.assertEqual(report.uncovered, ("audit claims", "manage abstention"))
        self.assertEqual(report.singly_covered_omitted, 0)
        self.assertEqual(report.weakly_covered_omitted, 0)
        self.assertEqual(report.matrix_omitted, 24)
        self.assertEqual(report.matrix[0]["pack_id"], "prism.context-acquisition")
        self.assertIn("no instances", report.limitations[0])

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/pack-coverage-audit/0.1",
            "stage": "pack_selection",
            "unknown_pack_ids": ["missing-pack"],
            "refusal": "coverage cannot be computed for unknown pack identifiers",
            "fail_closed": True,
            "guarantees": ["an unknown pack is not silently dropped"],
        }
        report = pack_coverage_audit_report({"tool": "pack_coverage_audit", "mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.stage, "pack_selection")
        request = PackCoverageAuditArgs("15", ("pack-a",), 1)
        self.assertEqual(Workspace(_SyncTool()).pack_coverage_audit_report(request).section, "15")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).pack_coverage_audit_report(request)).coverage_fraction, 0.75)
        with patch.object(ApiClient, "call_tool", return_value=coverage_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").pack_coverage_audit_report(request)
        self.assertEqual(result.selected_pack_count, 25)
        call.assert_called_once_with("pack_coverage_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=coverage_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).pack_coverage_audit_report(request)
            self.assertEqual(result.summary["uncovered"], 2)  # type: ignore[index]
            async_call.assert_called_once_with("pack_coverage_audit", request.to_mcp_arguments())

        asyncio.run(run())
