from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AdaptivePanelReport,
    AdaptivePanelRunArgs,
    AsyncApiClient,
    AsyncWorkspace,
    Workspace,
    adaptive_panel_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> AdaptivePanelRunArgs:
    return AdaptivePanelRunArgs(
        {"config": {}, "ledger": {}},
        [{"instance": "inst-1", "capability": "capability-a", "parent": "parent-1", "cost": 1.0}],
        capability="capability-a",
    )


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/adaptive-panel/0.1",
        "audit": {"trials": 0, "scored_trials": 0, "abstentions": 0, "total_cost": 0.0, "capabilities": [], "caveat": "clustered evidence caveat"},
        "audit_summary": {"trials": 0, "scored_trials": 0, "abstentions": 0, "total_cost": 0.0, "capabilities": 0, "reported": 0, "withheld": 0, "effective_trials": 0.0, "headline": "empty"},
        "audit_digest": None,
        "selection": {
            "ok": True,
            "value": {
                "mode": "next",
                "record": {
                    "chosen": {"instance": "inst-1", "capability": "capability-a", "parent": "parent-1", "score": 0.5, "expected_variance_reduction": 0.1, "independence_weight": 1.0, "cost": 1.0, "parent_trials_before": 0},
                    "eligible": 1,
                    "already_run": 0,
                    "coverage_gated_out": 0,
                    "gated_by": None,
                    "runners_up": [],
                    "icc_used": 0.5,
                    "icc_source": "assumed",
                    "caveat": "greedy",
                },
            },
        },
        "capability": {
            "capability": "capability-a",
            "coverage": {"capability": "capability-a", "trials": 0, "parents": 0, "qualifying_parents": 0, "abstentions": 0, "shortfalls": [{"kind": "trials", "have": 0, "need": 30}]},
            "stopping": None,
            "stopping_refusal": "no recorded trials",
            "estimate": None,
            "estimate_refusal": "no recorded trials",
            "fail_closed": True,
        },
        "comparison": None,
        "finished": False,
        "finished_refusal": None,
        "guarantees": ["abstentions are retained and costed but never counted as failures"],
        "limitations": ["selection never executes candidates"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class AdaptivePanelTests(unittest.TestCase):
    def test_request_preserves_selection_controls_and_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["capability"], "capability-a")
        self.assertEqual(args.to_mcp_arguments()["candidates"][0]["parent"], "parent-1")
        with self.assertRaises(ArgumentError):
            AdaptivePanelRunArgs({}, batch_size=2)
        with self.assertRaises(ArgumentError):
            AdaptivePanelRunArgs({}, left="a")

    def test_report_keeps_withheld_estimate_and_deterministic_selection_visible(self) -> None:
        report = adaptive_panel_report(payload())
        self.assertIsInstance(report, AdaptivePanelReport)
        self.assertEqual(report.audit.withheld, 0)
        self.assertEqual(report.selection.record.chosen.instance, "inst-1")
        self.assertEqual(report.capability.coverage.shortfalls[0].kind, "trials")
        self.assertIsNone(report.capability.estimate)
        self.assertTrue(report.capability.fail_closed)
        self.assertFalse(report.finished)
        self.assertTrue(report.reportable_estimates_are_clustered)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(adaptive_panel_report(envelope).schema, "bioprism-mcp/adaptive-panel/0.1")

    def test_all_python_facades_return_typed_adaptive_reports(self) -> None:
        args = request()
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).adaptive_panel_report(args)).selection.mode, "next")
        self.assertTrue(Workspace(_SyncTool()).adaptive_panel_report(args).capability.fail_closed)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").adaptive_panel_report(args)
        self.assertEqual(report.audit.trials, 0)
        call.assert_called_once_with("adaptive_panel", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).adaptive_panel_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/adaptive-panel/0.1")
            async_call.assert_called_once_with("adaptive_panel", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
