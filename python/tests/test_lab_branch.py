from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LabBranchAuditArgs,
    LabBranchAuditReport,
    Workspace,
    lab_branch_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def branch_request() -> dict:
    return {
        "policy": {"ceiling": {"max_branches": 4, "max_verifier_calls": 2}, "on_undetermined": "escalate", "rules": []},
        "decisions": [
            {
                "decision": "uncertain",
                "features": {
                    "reversibility": "reversible",
                    "permission": "read_only",
                    "value_at_stake": "low",
                    "unseparated_hypotheses": 1,
                    "unmet_mandatory_obligations": 0,
                    "historical_failure_rate": None,
                    "verifier_available": True,
                },
                "escaped": "an unmeasured risk escaped",
            }
        ],
        "max_rows": 1,
    }


def branch_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/lab-branch-audit/0.1",
        "policy": branch_request()["policy"],
        "decision_count": 1,
        "yield": {
            "decisions": 1,
            "escalations": 1,
            "escalations_on_undetermined": 1,
            "spent": {"branches": 1, "verifier_calls": 1},
            "catches": 0,
            "wasted_escalations": 1,
            "escaped_after_escalation": 1,
            "escaped_without_escalation": 0,
            "branches_per_catch": None,
        },
        "verdict": {"verdict": "paid_and_caught_nothing", "spent": {"branches": 1, "verifier_calls": 1}, "escalations": 1},
        "rows": [{"index": 0, "decision": "uncertain"}],
        "rows_omitted": 0,
        "max_rows": 1,
        "guarantees": ["undetermined risk escalates"],
        "limitations": ["planning only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(branch_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(branch_payload())}]})


class LabBranchTests(unittest.TestCase):
    def test_args_validate_decisions_and_bounds(self) -> None:
        request = LabBranchAuditArgs.from_wire(branch_request())
        self.assertEqual(request.to_mcp_arguments()["max_rows"], 1)
        with self.assertRaises(ArgumentError):
            LabBranchAuditArgs.from_wire({**branch_request(), "decisions": []})
        with self.assertRaises(ArgumentError):
            LabBranchAuditArgs.from_wire({**branch_request(), "decisions": [{"decision": "x", "features": {}, "caught": {"what": ""}}]})

    def test_report_preserves_undetermined_and_paid_without_catch(self) -> None:
        report = lab_branch_audit_report(branch_payload())
        self.assertIsInstance(report, LabBranchAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.paid_and_caught_nothing)
        self.assertEqual(report.yielded["escalations_on_undetermined"], 1)  # type: ignore[index]

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/lab-branch-audit/0.1",
            "stage": "policy_validation",
            "refusal": "branch ceiling exceeded",
            "fail_closed": True,
            "guarantees": ["no partial ledger"],
        }
        report = lab_branch_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = LabBranchAuditArgs.from_wire(branch_request())
        self.assertTrue(Workspace(_SyncTool()).lab_branch_audit_report(request).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).lab_branch_audit_report(request)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=branch_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").lab_branch_audit_report(request)
        self.assertEqual(result.verdict, "paid_and_caught_nothing")
        call.assert_called_once_with("lab_branch_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=branch_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).lab_branch_audit_report(request)
            self.assertEqual(result.rows_omitted, 0)
            async_call.assert_called_once_with("lab_branch_audit", request.to_mcp_arguments())

        asyncio.run(run())
