from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EpistemicSelectionAuditArgs,
    EpistemicSelectionAuditReport,
    Workspace,
    epistemic_selection_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "problem": {"actions": ["treat", "defer"], "models": ["responsive", "resistant"], "loss": [0.0, 10.0, 10.0, 0.0]},
        "belief": {"mass": [0.4, 0.6]},
        "evidence_pool": {"items": [
            {"id": "scan", "cost": 2.0, "likelihood": [0.9, 0.1]},
            {"id": "marker", "cost": 1.0, "likelihood": [0.8, 0.2]},
            {"id": "uninformative", "cost": 1.0, "likelihood": [1.0, 1.0]},
        ]},
        "constraint": {"cardinality": 2},
        "protected": [],
        "check_submodularity": True,
        "include_lazy": True,
        "compare_optimum": True,
        "tolerance": 1e-9,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/epistemic-selection-audit/0.1",
        "objective": "regret_reduction",
        "problem": {"actions": ["treat", "defer"], "models": ["responsive", "resistant"], "action_count": 2, "model_count": 2},
        "evidence_pool": {"count": 3, "items": [], "total_cost": 4.0},
        "constraint": {"cardinality": 2, "budget": None, "costs": [2.0, 1.0, 1.0]},
        "baseline": {"full_context_regret": 9.5, "empty_context_value": 0.0},
        "submodularity": {"status": "evaluated", "report": {"monotone_submodular": True}},
        "greedy": {"chosen": [{"index": 0, "id": "scan"}], "guarantee": {"applicability": "applies"}, "evaluations": 5},
        "lazy": {"chosen": [{"index": 0, "id": "scan"}], "evaluations": 3},
        "comparisons": {"greedy_lazy_agree": True, "exact_optimum": {"status": "evaluated", "ratio": 1.0}},
        "guarantees": ["exact only within cap"],
        "limitations": ["scalarized cost"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class EpistemicSelectionTests(unittest.TestCase):
    def test_args_validate_constraints_and_protected_closure(self) -> None:
        parsed = EpistemicSelectionAuditArgs.from_wire(request())
        self.assertEqual(parsed.to_mcp_arguments()["constraint"]["cardinality"], 2)
        with self.assertRaises(ArgumentError):
            EpistemicSelectionAuditArgs.from_wire({**request(), "protected": [0, 0]})
        with self.assertRaises(ArgumentError):
            EpistemicSelectionAuditArgs.from_wire({**request(), "constraint": {"budget": 2.0, "costs": [0.0, 1.0, 1.0]}})

    def test_report_preserves_guarantee_and_exactness_posture(self) -> None:
        report = epistemic_selection_audit_report(payload())
        self.assertIsInstance(report, EpistemicSelectionAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.guarantee_applies)
        self.assertEqual(report.exact_status, "evaluated")

    def test_all_facades_keep_selection_route_typed(self) -> None:
        parsed = EpistemicSelectionAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).epistemic_selection_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).epistemic_selection_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").epistemic_selection_audit_report(parsed)
        self.assertTrue(report.guarantee_applies)
        call.assert_called_once_with("epistemic_selection_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).epistemic_selection_audit_report(parsed)
            self.assertEqual(report.exact_status, "evaluated")
            async_call.assert_called_once_with("epistemic_selection_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
