from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalBurdenAuditArgs,
    BioevalBurdenAuditReport,
    Workspace,
    bioeval_burden_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "root": "root",
        "resources": [
            {"id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL"},
            {"id": "compute", "class": "compute_and_money", "initial": 10, "unit": "hour"},
        ],
        "branches": [{"id": "candidate-a"}, {"id": "candidate-b"}],
        "draws": [
            {"branch": "root", "action": "extract", "resource": "biopsy", "amount": 30, "unit": "uL", "outcome": "wasted", "destructive": True},
            {"branch": "candidate-a", "action": "sequence", "resource": "biopsy", "amount": 60, "unit": "uL", "outcome": "productive", "destructive": True},
        ],
        "inspect_branches": ["root", "candidate-a"],
        "joint_branches": ["candidate-a"],
        "require_joint_feasible": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-burden-audit/0.1",
        "workflow": "bioeval_burden_audit",
        "burden": {"root": "root", "resource_count": 2, "branch_count": 3, "draw_count": 2, "nonrenewable_resource_count": 1},
        "resources": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "branches": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "draws": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "joint_feasibility": {"status": "accepted", "branches": ["candidate-a"], "refusal": None},
        "wasted_nonrenewable": {"rows": [], "returned": 0, "total": 1, "omitted": 1},
        "findings": {"wasted_nonrenewable_actions": {"ids": ["extract"], "total": 1, "omitted": 0}},
        "guarantees": ["failed actions retain consumption"],
        "limitations": ["no pricing"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalBurdenTests(unittest.TestCase):
    def test_args_preserve_resources_branches_and_inherited_draws(self) -> None:
        parsed = BioevalBurdenAuditArgs.from_wire(request())
        self.assertEqual(parsed.resources[0].resource_class, "tissue_aliquot")
        self.assertEqual(parsed.branches[0].parent, None)
        self.assertEqual(parsed.to_mcp_arguments()["draws"][0]["outcome"], "wasted")
        with self.assertRaises(ArgumentError):
            BioevalBurdenAuditArgs.from_wire({**request(), "branches": [{"id": "child", "parent": "missing"}]})
        with self.assertRaises(ArgumentError):
            BioevalBurdenAuditArgs.from_wire({**request(), "resources": [{**request()["resources"][0], "class": "invented"}]})

    def test_report_preserves_fork_and_waste_posture(self) -> None:
        report = bioeval_burden_audit_report(payload())
        self.assertIsInstance(report, BioevalBurdenAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.branch_count, 3)
        self.assertFalse(report.joint_refused)
        self.assertEqual(report.wasted_nonrenewable_count, 1)

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_burden_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-burden-audit/0.1",
            "workflow": "bioeval_burden_audit",
            "stage": "joint_feasibility_policy",
            "refusal": "fork double spend",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "joint_feasibility_policy")

    def test_all_facades_keep_burden_audit_typed(self) -> None:
        parsed = BioevalBurdenAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_burden_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_burden_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_burden_audit_report(parsed)
        self.assertEqual(report.wasted_nonrenewable_count, 1)
        call.assert_called_once_with("bioeval_burden_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_burden_audit_report(parsed)
            self.assertEqual(report.branch_count, 3)
            async_call.assert_called_once_with("bioeval_burden_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
