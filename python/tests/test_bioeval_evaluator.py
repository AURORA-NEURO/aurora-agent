from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalEvaluatorAuditArgs,
    BioevalEvaluatorAuditReport,
    Workspace,
    bioeval_evaluator_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "runs": [
            {
                "evaluator": "grader-a",
                "health": {"health": "healthy"},
                "reached": "met",
                "diagnostic": {},
            },
            {
                "evaluator": "grader-b",
                "health": {"health": "healthy"},
                "reached": "not_met",
                "diagnostic": {"command": "pytest", "exit_state": "1", "diff": "expected output missing"},
            },
            {
                "evaluator": "timeout",
                "health": {"health": "timed_out", "after": "120s"},
                "reached": "met",
                "diagnostic": {},
            },
        ],
        "require_task_evidence": True,
        "max_items": 2,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-evaluator-audit/0.1",
        "workflow": "bioeval_evaluator_audit",
        "runs": {"rows": [], "returned": 0, "total": 0, "omitted": 0},
        "panel": {
            "run_count": 3,
            "healthy_count": 2,
            "unhealthy_count": 1,
            "task_evidence_count": 1,
            "posture": "task_evidence_available",
        },
        "findings": {
            "hidden_data_evaluators": {"ids": ["grader-b"], "total": 1, "omitted": 0},
        },
        "guarantees": ["harness failures remain unscored"],
        "limitations": ["the route does not execute a harness"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalEvaluatorTests(unittest.TestCase):
    def test_args_keep_health_separate_from_task_outcome_and_normalize_diagnostics(self) -> None:
        parsed = BioevalEvaluatorAuditArgs.from_wire(request())
        self.assertIsInstance(parsed.runs[0].health, object)
        self.assertEqual(parsed.runs[0].to_wire()["diagnostic"]["command"], "")
        self.assertEqual(parsed.runs[1].reached, "not_met")
        self.assertEqual(parsed.runs[2].to_wire()["health"], {"health": "timed_out", "after": "120s"})
        self.assertTrue(parsed.require_task_evidence)
        with self.assertRaises(ArgumentError):
            BioevalEvaluatorAuditArgs.from_wire({**request(), "runs": [{"evaluator": "x", "health": {"health": "timed_out"}}]})
        with self.assertRaises(ArgumentError):
            BioevalEvaluatorAuditArgs.from_wire({**request(), "max_items": 0})

    def test_report_preserves_panel_posture_and_hidden_data_findings(self) -> None:
        report = bioeval_evaluator_audit_report(payload())
        self.assertIsInstance(report, BioevalEvaluatorAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.posture, "task_evidence_available")
        self.assertEqual(report.task_evidence_count, 1)
        self.assertEqual(report.hidden_data_evaluators, ("grader-b",))

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_evaluator_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-evaluator-audit/0.1",
            "workflow": "bioeval_evaluator_audit",
            "stage": "hidden_data_policy",
            "refusal": "hidden evaluator access",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "hidden_data_policy")
        self.assertTrue(report.fail_closed)

    def test_all_facades_keep_evaluator_audit_typed(self) -> None:
        parsed = BioevalEvaluatorAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_evaluator_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_evaluator_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_evaluator_audit_report(parsed)
        self.assertEqual(report.posture, "task_evidence_available")
        call.assert_called_once_with("bioeval_evaluator_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_evaluator_audit_report(parsed)
            self.assertEqual(report.task_evidence_count, 1)
            async_call.assert_called_once_with("bioeval_evaluator_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
