from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    RoutingLabRunArgs,
    RoutingLabRunReport,
    Workspace,
    routing_lab_run_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def lab_request() -> dict:
    return {
        "tasks": [
            {"task_id": "reference-task", "world": {"schema_version": "fiber-world/0.1"}, "query": {"schema_version": "fiber-query/0.1"}},
            {"task_id": "discriminating-task", "world": {"schema_version": "fiber-world/0.1"}, "query": {"schema_version": "fiber-query/0.1"}},
        ],
        "settings": {
            "policy": {"approved": [{"kind": "full_context"}, {"kind": "fiber_compiled"}], "safe_default": {"kind": "full_context"}},
            "fixed_default": {"kind": "full_context"},
            "holdout": "task",
            "calibration_bins": 5,
        },
        "include_rows": True,
        "max_rows": 1,
    }


def lab_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/routing-lab-run/0.1",
        "tasks": 2,
        "holdout": "task",
        "holdout_label": "leave-one-task-out",
        "approved_architectures": ["full-context", "fiber"],
        "fixed_default": {"kind": "full_context"},
        "include_rows": True,
        "report": {
            "account": {"router": {"mean_utility": 0.4}, "oracle": {"mean_utility": 0.8}},
            "calibration": {"bins": []},
            "verdict": "router_loses_to_fixed_default",
            "abstention_rate": 0.5,
            "oracle_agreement_rate": 0.0,
            "tasks_won": 0,
            "tasks_lost": 1,
            "tasks_tied": 1,
            "caveats": ["leave-one-task-out"],
            "task_rows": [{"task_id": "reference-task", "abstained": True}],
            "task_rows_omitted": 1,
        },
        "guarantees": ["route_unseen holdout is enforced"],
        "limitations": ["offline context architecture lab"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(lab_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(lab_payload())}]})


class RoutingLabTests(unittest.TestCase):
    def test_args_bound_tasks_and_rows(self) -> None:
        request = RoutingLabRunArgs.from_wire(lab_request())
        self.assertEqual(request.to_mcp_arguments()["max_rows"], 1)
        with self.assertRaises(ArgumentError):
            RoutingLabRunArgs.from_wire({**lab_request(), "tasks": [lab_request()["tasks"][0], lab_request()["tasks"][0]]})
        with self.assertRaises(ArgumentError):
            RoutingLabRunArgs(tuple(), lab_request()["settings"])

    def test_report_preserves_holdout_regret_and_negative_verdict(self) -> None:
        report = routing_lab_run_report(lab_payload())
        self.assertIsInstance(report, RoutingLabRunReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.negative_result)
        self.assertEqual(report.holdout_label, "leave-one-task-out")
        self.assertEqual(report.task_rows_omitted, 1)
        self.assertEqual(report.report["tasks_lost"], 1)  # type: ignore[index]

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/routing-lab-run/0.1",
            "stage": "lab_execution",
            "refusal": "unjudged architecture outcome",
            "fail_closed": True,
            "guarantees": ["partial regret is not reported"],
        }
        report = routing_lab_run_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.stage, "lab_execution")
        request = RoutingLabRunArgs.from_wire(lab_request())
        self.assertEqual(Workspace(_SyncTool()).routing_lab_run_report(request).tasks, 2)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).routing_lab_run_report(request)).verdict, "router_loses_to_fixed_default")
        with patch.object(ApiClient, "call_tool", return_value=lab_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").routing_lab_run_report(request)
        self.assertTrue(result.negative_result)
        call.assert_called_once_with("routing_lab_run", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=lab_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).routing_lab_run_report(request)
            self.assertEqual(result.task_rows[0]["task_id"], "reference-task")
            async_call.assert_called_once_with("routing_lab_run", request.to_mcp_arguments())

        asyncio.run(run())
