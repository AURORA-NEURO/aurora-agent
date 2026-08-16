from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalMeshAuditArgs,
    BioevalMeshAuditReport,
    Workspace,
    bioeval_mesh_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "system_artifacts": ["system-weights"],
        "evaluators": [
            {"id": "reader-a", "kind": "expert_review", "inputs": ["report-77"]},
            {"id": "reader-b", "kind": "expert_review", "inputs": ["report-77"]},
            {"id": "molecular", "kind": "executable_analysis", "inputs": ["panel-9"]},
        ],
        "verdicts": [
            {"evaluator": "reader-a", "position": "progression"},
            {"evaluator": "reader-b", "position": "progression"},
            {"evaluator": "molecular", "position": "pseudoprogression"},
        ],
        "expected": "progression",
        "require_independence": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-mesh-audit/0.1",
        "workflow": "bioeval_mesh_audit",
        "mesh": {"evaluator_count": 3, "independent_class_count": 2, "non_model_class_count": 2, "independence_verified": True, "kinds_present": ["expert_review", "executable_analysis"], "inputs_undeclared": []},
        "evaluators": {"rows": [], "returned": 0, "total": 3, "omitted": 3},
        "classes": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "verdicts": {"rows": [], "returned": 0, "total": 3, "omitted": 3},
        "disagreements": {"rows": [], "returned": 0, "total": 1, "omitted": 1, "within_class_count": 0, "across_class_count": 1},
        "independent_ratings": {"status": "accepted", "rows": [], "refusal": None},
        "contributions": {"status": "accepted", "expected": "progression", "rows": [], "refusal": None},
        "findings": {"inputs_undeclared": {"ids": [], "total": 0, "omitted": 0}, "unreported_evaluators": {"ids": [], "total": 0, "omitted": 0}, "abstaining_evaluators": {"ids": [], "total": 0, "omitted": 0}, "within_class_disagreement_count": 0, "across_class_disagreement_count": 1, "rating_projection_refused": False},
        "guarantees": ["shared inputs collapse into classes"],
        "limitations": ["no adjudication"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalMeshTests(unittest.TestCase):
    def test_args_preserve_shared_inputs_and_verdict_scope(self) -> None:
        parsed = BioevalMeshAuditArgs.from_wire(request())
        self.assertEqual(parsed.evaluators[0].inputs, ("report-77",))
        self.assertEqual(parsed.verdicts[2].position, "pseudoprogression")
        self.assertEqual(parsed.to_mcp_arguments()["expected"], "progression")
        with self.assertRaises(ArgumentError):
            BioevalMeshAuditArgs.from_wire({**request(), "verdicts": [{"evaluator": "unknown", "position": "x"}]})
        with self.assertRaises(ArgumentError):
            BioevalMeshAuditArgs.from_wire({**request(), "verdicts": [{"evaluator": "reader-a", "position": ""}]})

    def test_report_preserves_independence_and_disagreement_counts(self) -> None:
        report = bioeval_mesh_audit_report(payload())
        self.assertIsInstance(report, BioevalMeshAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.independence_verified)
        self.assertEqual(report.independent_class_count, 2)
        self.assertEqual(report.within_class_count, 0)
        self.assertEqual(report.across_class_count, 1)

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = bioeval_mesh_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-mesh-audit/0.1",
            "workflow": "bioeval_mesh_audit",
            "stage": "independence_policy",
            "refusal": "inputs undeclared",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "independence_policy")

    def test_all_facades_keep_mesh_audit_typed(self) -> None:
        parsed = BioevalMeshAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_mesh_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_mesh_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_mesh_audit_report(parsed)
        self.assertEqual(report.across_class_count, 1)
        call.assert_called_once_with("bioeval_mesh_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_mesh_audit_report(parsed)
            self.assertTrue(report.independence_verified)
            async_call.assert_called_once_with("bioeval_mesh_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
