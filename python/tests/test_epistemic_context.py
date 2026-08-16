from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EpistemicContextAuditArgs,
    EpistemicContextAuditReport,
    Workspace,
    epistemic_context_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "problem": {"actions": ["treat", "abstain"], "models": ["responsive", "resistant"], "loss": [0.0, 10.0, 10.0, 0.0]},
        "belief": {"mass": [0.5, 0.5]},
        "evidence_pool": {"items": [
            {"id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1]},
            {"id": "marker", "cost": 1.0, "likelihood": [0.1, 0.9]},
        ]},
        "criterion": "bayes_regret",
        "tolerance": 1.0,
        "compatibility_floor": 0.0,
        "subsets": [[0], [0, 1]],
        "include_frontier": True,
        "max_rows": 1,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/epistemic-context-audit/0.1",
        "criterion": "bayes_regret",
        "tolerance": 1.0,
        "compatibility_floor": 0.0,
        "problem": {"actions": ["treat", "abstain"], "models": ["responsive", "resistant"]},
        "evidence_pool": {"item_count": 2, "full_rate": 3.0},
        "identification": {"status": "non_identified", "minimax_regret": 10.0},
        "sufficiency": {"outcome": "sufficient", "retained": [0, 1], "rate": 3.0, "distortion": 0.0},
        "frontier": {"criterion": "bayes_regret", "evaluated": 4, "points": []},
        "include_frontier": True,
        "subset_rows": [{"index": 0, "result": "evaluated"}],
        "subset_count": 2,
        "subset_refusal_count": 0,
        "subset_rows_omitted": 1,
        "max_rows": 1,
        "guarantees": ["decision regret is not embedding similarity"],
        "limitations": ["caller-declared prior"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class EpistemicContextTests(unittest.TestCase):
    def test_args_validate_ordered_pool_and_subset_bounds(self) -> None:
        parsed = EpistemicContextAuditArgs.from_wire(request())
        self.assertEqual(parsed.to_mcp_arguments()["evidence_pool"]["items"][1]["id"], "marker")
        with self.assertRaises(ArgumentError):
            EpistemicContextAuditArgs.from_wire({**request(), "criterion": "unknown"})
        with self.assertRaises(ArgumentError):
            EpistemicContextAuditArgs.from_wire({**request(), "subsets": [[0, 0]]})

    def test_report_preserves_frontier_and_omitted_subset_rows(self) -> None:
        report = epistemic_context_audit_report(payload())
        self.assertIsInstance(report, EpistemicContextAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.frontier_evaluated, 4)
        self.assertFalse(report.abstained)
        self.assertEqual(report.subset_rows_omitted, 1)

    def test_refusal_and_all_facades_remain_typed(self) -> None:
        refusal = {"ok": False, "schema": "bioprism-mcp/epistemic-context-audit/0.1", "stage": "enumeration_bound", "refusal": "frontier cap", "fail_closed": True, "guarantees": ["no sampled minimum"]}
        self.assertTrue(epistemic_context_audit_report({"mcp": {"result": {"structuredContent": refusal}}}).refused)
        parsed = EpistemicContextAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).epistemic_context_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).epistemic_context_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").epistemic_context_audit_report(parsed)
        self.assertEqual(report.subset_count, 2)
        call.assert_called_once_with("epistemic_context_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).epistemic_context_audit_report(parsed)
            self.assertEqual(report.criterion, "bayes_regret")
            async_call.assert_called_once_with("epistemic_context_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
