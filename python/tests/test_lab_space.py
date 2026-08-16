from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LabSpaceAuditArgs,
    LabSpaceAuditReport,
    Workspace,
    lab_space_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def space_request() -> dict:
    return {
        "cost_ceiling": 10,
        "candidates": [
            {"id": "v1", "components": []},
            {"id": "v2", "derived_from": "v1", "components": []},
        ],
        "inspect": ["v2"],
        "comparisons": [{"before": "v1", "after": "v2"}],
        "include_components": True,
        "max_rows": 1,
    }


def space_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/lab-space-audit/0.1",
        "cost_ceiling": 10,
        "candidate_count": 2,
        "registered_count": 2,
        "space_committed": True,
        "space": {"registered_ids": ["v1", "v2"], "root_ids": ["v1"], "lineage_depth_max": 2},
        "candidate_rows": [{"index": 0, "validation": "valid", "registration": "registered"}],
        "candidate_rows_omitted": 1,
        "inspection_count": 1,
        "inspection_rows": [{"configuration": "v2", "lineage": ["v2", "v1"]}],
        "inspection_rows_omitted": 0,
        "comparison_count": 1,
        "comparison_rows": [{"before": "v1", "after": "v2", "changes": ["cost_units 0 -> 2"]}],
        "comparison_rows_omitted": 0,
        "max_rows": 1,
        "guarantees": ["immutable registry"],
        "limitations": ["no component execution"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(space_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(space_payload())}]})


class LabSpaceTests(unittest.TestCase):
    def test_args_bound_inspection_comparisons_and_optional_detail(self) -> None:
        request = LabSpaceAuditArgs.from_wire(space_request())
        self.assertTrue(request.to_mcp_arguments()["include_components"])
        self.assertEqual(request.to_mcp_arguments()["comparisons"][0]["after"], "v2")
        with self.assertRaises(ArgumentError):
            LabSpaceAuditArgs.from_wire({**space_request(), "max_rows": 0})
        with self.assertRaises(ArgumentError):
            LabSpaceAuditArgs.from_wire({**space_request(), "inspect": [""]})

    def test_report_reconciles_three_bounded_projections(self) -> None:
        report = lab_space_audit_report(space_payload())
        self.assertIsInstance(report, LabSpaceAuditReport)
        self.assertTrue(report.accepted)
        self.assertFalse(report.complete)
        self.assertEqual(report.candidate_rows_omitted, 1)
        self.assertEqual(report.inspection_rows[0]["lineage"], ["v2", "v1"])
        self.assertEqual(report.comparison_count, 1)

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/lab-space-audit/0.1",
            "stage": "candidate_validation",
            "refusal": "protected surface",
            "fail_closed": True,
            "candidate_count": 1,
            "registered_count": 0,
            "space_committed": False,
            "candidate_rows": [{"validation": "refused"}],
            "candidate_rows_omitted": 0,
            "max_rows": 1,
            "guarantees": ["no partial space"],
        }
        report = lab_space_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = LabSpaceAuditArgs.from_wire(space_request())
        self.assertTrue(Workspace(_SyncTool()).lab_space_audit_report(request).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).lab_space_audit_report(request)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=space_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").lab_space_audit_report(request)
        self.assertEqual(result.registered_count, 2)
        call.assert_called_once_with("lab_space_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=space_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).lab_space_audit_report(request)
            self.assertEqual(result.comparison_rows[0]["after"], "v2")
            async_call.assert_called_once_with("lab_space_audit", request.to_mcp_arguments())

        asyncio.run(run())
