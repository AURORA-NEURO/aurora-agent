from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    CapabilityDashboardQueryArgs,
    CapabilityDashboardReport,
    Workspace,
    capability_dashboard_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "capability_dashboard",
        "schema": "bioprism-devplat-capability-dashboard/0.1",
        "catalog_digest": "a" * 64,
        "dashboard_digest": "b" * 64,
        "capability_dashboard_ready": True,
        "duplicate_schema_names": [],
        "audit": {
            "schema": "bioprism-devplat-capability-dashboard/0.1",
            "catalog_digest": "a" * 64,
            "dashboard_digest": "b" * 64,
            "query": {"domain": "oncology", "max_groups": 128, "include_tools": True, "include_gaps": True},
            "total_group_count": 29,
            "selected_group_count": 1,
            "available_group_count": 1,
            "callable_group_count": 1,
            "partial_group_count": 0,
            "declared_only_group_count": 0,
            "selected_tool_memberships": 2,
            "selected_unique_tools": 2,
            "schema_backed_unique_tools": 2,
            "readiness_counts": {"callable": 1},
            "gap_counts": {"no_cli_entrypoints": 1, "no_python_artifact": 1},
            "groups": [{
                "id": "biological_domains",
                "domains": ["oncology"],
                "status": "available",
                "readiness": "callable",
                "surfaces": {"crates": 2, "mcp_tools": 2, "cli_entrypoints": 0, "python_artifacts": 0},
                "tool_count": 2,
                "callable_tool_count": 2,
                "schema_backed_tool_count": 2,
                "missing_transport_schemas": [],
                "invalid_transport_schemas": [],
                "tools": ["onco_boundary_check", "onco_response_assess"],
                "gaps": ["no_cli_entrypoints", "no_python_artifact"],
            }],
            "warnings": [],
            "guarantees": [],
            "limitations": [],
            "ready": True,
        },
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class CapabilityDashboardTests(unittest.TestCase):
    def test_query_bounds_and_wire_shape(self) -> None:
        args = CapabilityDashboardQueryArgs(domain="oncology", max_groups=4, include_tools=True)
        self.assertEqual(args.to_mcp_arguments()["domain"], "oncology")
        self.assertEqual(args.to_mcp_arguments()["max_groups"], 4)
        self.assertEqual(args.to_query_params()["include_tools"], "true")
        with self.assertRaises(ArgumentError):
            CapabilityDashboardQueryArgs(max_groups=0)

    def test_report_preserves_surface_gaps_and_callable_groups(self) -> None:
        report = capability_dashboard_report(payload())
        self.assertIsInstance(report, CapabilityDashboardReport)
        self.assertTrue(report.ready)
        self.assertEqual(report.callable[0].id, "biological_domains")
        self.assertEqual(report.gap_labels, ("no_cli_entrypoints", "no_python_artifact"))
        self.assertEqual(report.groups[0].tools[-1], "onco_response_assess")
        self.assertTrue(capability_dashboard_report({"ok": True, "mcp": {"result": {"structuredContent": payload()}}}).ready)

    def test_all_facades_keep_dashboard_typed(self) -> None:
        args = CapabilityDashboardQueryArgs(domain="oncology", include_tools=True)
        self.assertTrue(Workspace(_SyncTool()).capability_dashboard_report(args).ready)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).capability_dashboard_report(args)).ready)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").capability_dashboard_report(args)
        self.assertEqual(report.selected_group_count, 1)
        call.assert_called_once_with("capability_dashboard", args.to_mcp_arguments())
        with patch.object(ApiClient, "request", return_value=payload()) as request:
            rest_report = ApiClient("http://127.0.0.1:1").capability_dashboard_rest_report(args)
        self.assertTrue(rest_report.ready)
        request.assert_called_once_with(
            "GET",
            "/v1/capabilities/dashboard?max_groups=128&include_tools=true&include_gaps=true&domain=oncology",
        )

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).capability_dashboard_report(args)
            self.assertEqual(result.groups[0].readiness, "callable")
            async_call.assert_called_once_with("capability_dashboard", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
