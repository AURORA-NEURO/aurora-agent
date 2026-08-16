from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BenchmarkDecisionAuditArgs,
    BenchmarkDecisionAuditReport,
    Workspace,
    benchmark_decision_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def trace() -> dict:
    return {
        "trace_id": "failed-run",
        "succeeded": False,
        "events": [
            {"step": 0, "kind": "goal", "payload": {"summary": "solve"}},
            {"step": 1, "kind": "choice", "payload": {"action": "unsafe", "alternatives": ["safe"]}, "visible": ["task"]},
            {"step": 2, "kind": "termination", "payload": {"summary": "failed"}, "caused_by": 1, "visible": ["task"]},
        ],
    }


def audit_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-decision-audit/0.1",
        "trace_id": "failed-run",
        "trace_digest": "a" * 64,
        "analysis": {"trace_id": "failed-run", "ancestry": [1], "candidates": [], "verdict": {"verdict": "first_causal", "step": 1, "score": 0.8}},
        "analysis_omitted": {"ancestry": 0, "candidates": 0},
        "decision": {
            "selected_step": 1,
            "causal_step": 1,
            "causal_alignment": "aligned",
            "event_kind": "choice",
            "coverage": {"total": 3, "visible_at_decision_time": 2, "validation_only": 1, "feasible": 2, "strong": 1, "plausible_wrong_alternatives": 1, "adequate": True},
            "action_counts": {"all": 3, "visible_to_agent": 2, "validation_only": 1, "acceptable": 2},
            "actions": [],
            "visible_to_agent": [],
            "validation_only": [],
            "acceptable": [],
            "omitted": {"all": 3, "visible_to_agent": 2, "validation_only": 1, "acceptable": 2},
        },
        "failure_card": {"trace_id": "failed-run", "terminal_step": 2, "blame": {"blame": "agent", "at_step": 1}, "recommended_cell_steps": [1], "findings": [], "hypotheses": [], "violated_constraints": [], "alternative_explanations": [], "missing_evidence": [], "evidence_ratio": 1.0},
        "failure_card_omitted": {},
        "guarantees": ["future options are validation-only"],
    }


class _SyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class _AsyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class BenchmarkDecisionAuditTests(unittest.TestCase):
    def test_args_preserve_evidence_and_bound_rows(self) -> None:
        request = BenchmarkDecisionAuditArgs.from_wire({
            "trace": trace(),
            "decision_step": 1,
            "actions": [{"label": "safe", "provenance": {"source": "from_future", "from_step": 3}}],
            "constraints": [],
            "claims": [],
            "max_items": 7,
        })
        self.assertEqual(request.to_mcp_arguments()["trace"]["events"][1]["visible"], ["task"])
        self.assertEqual(request.to_mcp_arguments()["max_items"], 7)
        with self.assertRaises(ArgumentError):
            BenchmarkDecisionAuditArgs.from_wire({"trace": trace(), "max_items": 0})

    def test_report_keeps_firewall_counts_and_card_typed(self) -> None:
        report = benchmark_decision_audit_report(audit_payload())
        self.assertIsInstance(report, BenchmarkDecisionAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.localized)
        self.assertEqual(report.selected_step, 1)
        self.assertEqual(report.visible_action_count, 2)
        self.assertTrue(report.coverage.adequate)  # type: ignore[union-attr]
        self.assertEqual(report.failure_card.blame["blame"], "agent")  # type: ignore[union-attr]

    def test_refusal_and_all_facades_preserve_fail_closed_state(self) -> None:
        refusal = {"ok": False, "stage": "hindsight_firewall", "refusal": "future provenance", "fail_closed": True, "guarantees": ["no leak"]}
        report = benchmark_decision_audit_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = BenchmarkDecisionAuditArgs.from_wire({"trace": trace()})
        sync_report = Workspace(_SyncTool(audit_payload())).benchmark_decision_audit_report(request)
        self.assertEqual(sync_report.action_counts["all"], 3)
        async_report = asyncio.run(AsyncWorkspace(_AsyncTool(audit_payload())).benchmark_decision_audit_report(request))
        self.assertEqual(async_report.failure_card.evidence_ratio, 1.0)  # type: ignore[union-attr]
        with patch.object(ApiClient, "call_tool", return_value=audit_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").benchmark_decision_audit_report(request)
        self.assertEqual(report.trace_id, "failed-run")
        call.assert_called_once_with("benchmark_decision_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=audit_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).benchmark_decision_audit_report(request)
            self.assertEqual(report.causal_alignment, "aligned")
            async_call.assert_called_once_with("benchmark_decision_audit", request.to_mcp_arguments())

        asyncio.run(run())

