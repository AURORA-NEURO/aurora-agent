from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PackReleaseAuditArgs,
    PackReleaseAuditReport,
    Workspace,
    pack_release_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def release_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/pack-release-audit/0.1",
        "section": "15",
        "selected_pack_count": 25,
        "selected_pack_ids": [f"pack-{index}" for index in range(25)],
        "sequenced_count": 13,
        "unsequenced_count": 12,
        "release_coverage_fraction": 0.52,
        "wave_counts": {"1": 2, "2": 2, "3": 2, "4": 2, "5": 2, "6": 1, "7": 1, "8": 1},
        "axis_counts": {"mechanism": 8, "domain": 11, "platform": 6},
        "release_order": [
            {"selected_position": 1, "portfolio_position": 1, "id": "prism.context-acquisition", "release_wave": {"wave": 1}, "axis": "mechanism"},
            {"selected_position": 2, "portfolio_position": 2, "id": "prism.tool-selection", "release_wave": {"wave": 1}, "axis": "mechanism"},
            {"selected_position": 3, "portfolio_position": 3, "id": "prism.memory-lifecycle", "release_wave": {"wave": 2}, "axis": "mechanism"},
        ],
        "release_order_omitted": 10,
        "unsequenced": [
            {"id": "prism.benchmark-meta-evaluation", "release_wave": "unsequenced", "axis": "platform"},
            {"id": "prism.transfer-and-routing", "release_wave": "unsequenced", "axis": "mechanism"},
            {"id": "prism.terminal-and-devops", "release_wave": "unsequenced", "axis": "domain"},
        ],
        "unsequenced_omitted": 9,
        "guarantees": ["unsequenced packs remain explicit"],
        "limitations": ["not an approval or deployment action"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(release_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(release_payload())}]})


class PackReleaseTests(unittest.TestCase):
    def test_args_bound_sections_ids_and_item_limit(self) -> None:
        request = PackReleaseAuditArgs.from_wire({"section": "29", "pack_ids": ["pack-a"], "max_items": 3})
        self.assertEqual(request.to_mcp_arguments(), {"section": "29", "pack_ids": ["pack-a"], "max_items": 3})
        with self.assertRaises(ArgumentError):
            PackReleaseAuditArgs("bad")
        with self.assertRaises(ArgumentError):
            PackReleaseAuditArgs("all", ("pack-a", "pack-a"))
        with self.assertRaises(ArgumentError):
            PackReleaseAuditArgs("all", (), 0)

    def test_report_reconciles_sequence_and_preserves_positions(self) -> None:
        report = pack_release_audit_report(release_payload())
        self.assertIsInstance(report, PackReleaseAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.has_unsequenced)
        self.assertEqual(report.selected_pack_count, 25)
        self.assertEqual(report.sequenced_count, 13)
        self.assertEqual(report.unsequenced_count, 12)
        self.assertEqual(report.release_order[0]["portfolio_position"], 1)
        self.assertEqual(report.release_order_omitted, 10)
        self.assertEqual(report.unsequenced_omitted, 9)
        self.assertEqual(report.axis_counts["platform"], 6)

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/pack-release-audit/0.1",
            "stage": "pack_selection",
            "out_of_section_pack_ids": ["bio.statistical-estimands"],
            "refusal": "release order cannot be computed for an unknown or section-incompatible pack selection",
            "fail_closed": True,
            "guarantees": ["section-incompatible identifiers are reported"],
        }
        report = pack_release_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.stage, "pack_selection")
        request = PackReleaseAuditArgs("15", ("pack-a",), 1)
        self.assertEqual(Workspace(_SyncTool()).pack_release_audit_report(request).section, "15")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).pack_release_audit_report(request)).sequenced_count, 13)
        with patch.object(ApiClient, "call_tool", return_value=release_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").pack_release_audit_report(request)
        self.assertEqual(result.unsequenced_count, 12)
        call.assert_called_once_with("pack_release_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=release_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).pack_release_audit_report(request)
            self.assertTrue(result.has_unsequenced)
            async_call.assert_called_once_with("pack_release_audit", request.to_mcp_arguments())

        asyncio.run(run())
