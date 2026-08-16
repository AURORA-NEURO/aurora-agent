from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalDesignAuditArgs,
    BioevalDesignAuditReport,
    Workspace,
    bioeval_design_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "cell_id": "cell-7",
        "factors": ["planner", "verifier"],
        "baseline": "base",
        "arms": [
            {"id": "base", "levels": {"planner": "react", "verifier": "off"}, "conclusion": "fail", "tier": "execution"},
            {"id": "p1", "levels": {"planner": "tree", "verifier": "off"}, "conclusion": "pass", "tier": "execution"},
            {"id": "v1", "levels": {"planner": "react", "verifier": "on"}, "conclusion": "pass", "tier": "execution"},
            {"id": "both", "levels": {"planner": "tree", "verifier": "on"}, "conclusion": "pass", "tier": "execution"},
        ],
        "controlled": True,
        "require_complete_interactions": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-design-audit/0.1",
        "workflow": "bioeval_design_audit",
        "design": {"cell_id": "cell-7", "factors": ["planner", "verifier"], "baseline": "base", "arm_count": 4, "contrast_count": 4, "unattributable_arm_count": 1, "controlled": True, "valid": True},
        "arms": {"rows": [], "returned": 0, "total": 4, "omitted": 4},
        "contrasts": {"rows": [], "returned": 0, "total": 4, "omitted": 4},
        "interactions": {"rows": [], "returned": 0, "total": 1, "omitted": 1, "estimable_count": 1, "missing_count": 0},
        "attributions": {"rows": [], "returned": 0, "total": 4, "omitted": 4, "refused_count": 0, "causal_count": 4},
        "findings": {"unattributable_arms": {"ids": ["both"], "total": 1, "omitted": 0}, "missing_interactions": {"ids": [], "total": 0, "omitted": 0}, "no_single_factor_contrasts": False, "attribution_refusal_count": 0},
        "guarantees": ["single-factor contrasts remain distinct"],
        "limitations": ["no arm execution"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalDesignTests(unittest.TestCase):
    def test_args_require_complete_factor_assignments(self) -> None:
        parsed = BioevalDesignAuditArgs.from_wire(request())
        self.assertEqual(parsed.arms[0].levels["planner"], "react")
        self.assertEqual(parsed.to_mcp_arguments()["baseline"], "base")
        with self.assertRaises(ArgumentError):
            BioevalDesignAuditArgs.from_wire({**request(), "arms": [{**request()["arms"][0], "levels": {"planner": "react"}}]})
        with self.assertRaises(ArgumentError):
            BioevalDesignAuditArgs.from_wire({**request(), "baseline": "missing"})

    def test_report_preserves_contrast_and_interaction_findings(self) -> None:
        report = bioeval_design_audit_report(payload())
        self.assertIsInstance(report, BioevalDesignAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.contrast_count, 4)
        self.assertEqual(report.causal_count, 4)
        self.assertEqual(report.unattributable_arms, ("both",))
        self.assertEqual(report.missing_interactions, ())

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_design_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-design-audit/0.1",
            "workflow": "bioeval_design_audit",
            "stage": "interaction_coverage",
            "refusal": "missing cell",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "interaction_coverage")

    def test_all_facades_keep_design_audit_typed(self) -> None:
        parsed = BioevalDesignAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_design_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_design_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_design_audit_report(parsed)
        self.assertEqual(report.contrast_count, 4)
        call.assert_called_once_with("bioeval_design_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_design_audit_report(parsed)
            self.assertEqual(report.causal_count, 4)
            async_call.assert_called_once_with("bioeval_design_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
