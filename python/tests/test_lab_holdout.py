from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LabHoldoutAuditArgs,
    LabHoldoutAuditReport,
    Workspace,
    lab_holdout_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def holdout_request() -> dict:
    candidate = lambda identifier: {
        "id": identifier,
        "components": [
            {"id": "select", "kind": "context_selector"},
            {"id": "run", "kind": "executor"},
            {"id": "stop", "kind": "terminator"},
        ],
        "cost_units": 0,
    }
    return {
        "cost_ceiling": 100,
        "candidates": [candidate("v1"), {**candidate("v2"), "derived_from": "v1"}],
        "holdouts": [{"id": "private-a", "partition": "rotating_private_certification", "query_budget": 4}],
        "current": "v1",
        "operations": [
            {"kind": "checkpoint", "label": "before-v2"},
            {"kind": "promote", "configuration": "v2", "selected_using": "private-a", "rationale": "won panel"},
            {"kind": "rollback", "checkpoint": "before-v2"},
            {"kind": "measure", "holdout": "private-a", "configuration": "v2", "metric": "rate", "value": 0.9},
        ],
        "max_rows": 2,
    }


def holdout_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/lab-holdout-audit/0.1",
        "current": "v1",
        "space": {"candidate_count": 2, "registered_ids": ["v1", "v2"]},
        "holdouts": [{"id": "private-a", "partition": "rotating_private_certification", "retired": False}],
        "remaining_certification_budget": [["private-a", 3]],
        "checkpoints": [{"label": "before-v2"}],
        "checkpoint_count": 1,
        "history": [{"event": "rolled_back"}],
        "operations": [
            {"index": 0, "kind": "checkpoint", "result": "accepted"},
            {"index": 1, "kind": "promote", "result": "accepted"},
        ],
        "operations_omitted": 2,
        "operation_count": 4,
        "measurement_count": 0,
        "measurement_refusal_count": 1,
        "rollback_count": 1,
        "permanently_burned": [{"holdout": "private-a", "configuration": "v2"}],
        "max_rows": 2,
        "guarantees": ["rollback never rewinds exposure"],
        "limitations": ["offline audit"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(holdout_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(holdout_payload())}]})


class LabHoldoutTests(unittest.TestCase):
    def test_args_validate_operation_program_and_bounds(self) -> None:
        request = LabHoldoutAuditArgs.from_wire(holdout_request())
        self.assertEqual(request.to_mcp_arguments()["max_rows"], 2)
        with self.assertRaises(ArgumentError):
            LabHoldoutAuditArgs.from_wire({**holdout_request(), "operations": []})
        with self.assertRaises(ArgumentError):
            LabHoldoutAuditArgs.from_wire({**holdout_request(), "operations": [{"kind": "invent"}]})

    def test_report_preserves_contamination_and_rollback_accounting(self) -> None:
        report = lab_holdout_audit_report(holdout_payload())
        self.assertIsInstance(report, LabHoldoutAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.has_contamination_refusals)
        self.assertEqual(report.operations_omitted, 2)
        self.assertEqual(report.current, "v1")

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/lab-holdout-audit/0.1",
            "stage": "architecture_validation",
            "refusal": "candidate graph is invalid",
            "fail_closed": True,
            "guarantees": ["no deployment state was created"],
        }
        report = lab_holdout_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = LabHoldoutAuditArgs.from_wire(holdout_request())
        self.assertTrue(Workspace(_SyncTool()).lab_holdout_audit_report(request).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).lab_holdout_audit_report(request)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=holdout_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").lab_holdout_audit_report(request)
        self.assertEqual(result.measurement_refusal_count, 1)
        call.assert_called_once_with("lab_holdout_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=holdout_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).lab_holdout_audit_report(request)
            self.assertEqual(result.rollback_count, 1)
            async_call.assert_called_once_with("lab_holdout_audit", request.to_mcp_arguments())

        asyncio.run(run())
