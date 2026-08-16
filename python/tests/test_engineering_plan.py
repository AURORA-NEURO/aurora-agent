from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EngineeringManifestArgs,
    EngineeringPlanPoliciesArgs,
    EngineeringPlanReport,
    EngineeringPlanRequestArgs,
    PackageSpecArgs,
    ProjectIdentityArgs,
    TechnologyBaselineArgs,
    TicketSpecArgs,
    Workspace,
    engineering_execution_plan_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def manifest() -> EngineeringManifestArgs:
    return EngineeringManifestArgs(
        ProjectIdentityArgs("aurora-agent", "0.1.0", "github.com/AURORA-NEURO/aurora-agent"),
        TechnologyBaselineArgs("Rust 2021", "cargo", "MCP JSON-RPC", "in-memory", "structured", "local"),
        packages=[
            PackageSpecArgs("core", "crates/core", "rust", "library", "platform"),
            PackageSpecArgs("api", "crates/api", "rust", "service", "platform", ("core",), True),
        ],
        tickets=[
            TicketSpecArgs("T-001", "ship core", "core", "core-contract", "done", acceptance=("tests pass",)),
            TicketSpecArgs("T-002", "ship api", "api", "api-contract", "planned", ("T-001",), ("protocol passes",)),
            TicketSpecArgs("T-003", "ship docs", "api", "docs-contract", "planned", ("T-002",), ("docs pass",)),
        ],
        ownership=[{"surface": "api", "accountable": "platform", "responsible": ["api-team"]}],
    )


def request() -> EngineeringPlanRequestArgs:
    return EngineeringPlanRequestArgs(manifest(), EngineeringPlanPoliciesArgs(max_tickets=3, max_parallelism=2))


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "engineering_execution_plan",
        "schema": "bioprism-engineering-plan-audit/0.1",
        "request_digest": "a" * 64,
        "manifest_digest": "b" * 64,
        "plan_digest": "c" * 64,
        "valid": True,
        "engineering_plan_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-engineering-plan-audit/0.1",
            "valid": True,
            "planning_started": True,
            "truncated": False,
            "ticket_count": 3,
            "planned_ticket_count": 2,
            "omitted_ticket_count": 0,
            "package_order": ["core", "api"],
            "ticket_plans": [
                {"ticket_id": "T-002", "package": "api", "contract": "api-contract", "status": "planned", "state": "ready", "dependency_ids": ["T-001"], "blocking_dependencies": [], "dependency_ready": True, "scheduled": True, "wave": 0, "critical_path_length": 2},
                {"ticket_id": "T-003", "package": "api", "contract": "docs-contract", "status": "planned", "state": "ready", "dependency_ids": ["T-002"], "blocking_dependencies": [], "dependency_ready": True, "scheduled": True, "wave": 1, "critical_path_length": 1},
            ],
            "waves": [
                {"index": 0, "ticket_ids": ["T-002"], "package_ids": ["api"], "depends_on_waves": [], "parallelism": 1},
                {"index": 1, "ticket_ids": ["T-003"], "package_ids": ["api"], "depends_on_waves": [0], "parallelism": 1},
            ],
            "critical_path": ["T-001", "T-002", "T-003"],
            "gates": [{"name": "manifest_admission", "passed": True, "required": True, "detail": "valid"}],
            "manifest_issues": [],
            "issues": [],
            "guarantees": ["deterministic ordering"],
            "limitations": ["does not mutate a tracker"],
        },
        "guarantees": ["deterministic ordering"],
        "limitations": ["does not mutate a tracker"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class EngineeringPlanTests(unittest.TestCase):
    def test_request_round_trip_and_bounds(self) -> None:
        args = request()
        self.assertEqual(EngineeringPlanRequestArgs.from_wire(args.to_wire()), args)
        self.assertEqual(args.to_mcp_arguments()["request"]["policies"]["max_parallelism"], 2)
        with self.assertRaises(ArgumentError):
            EngineeringPlanPoliciesArgs(max_tickets=101)
        with self.assertRaises(ArgumentError):
            EngineeringPlanRequestArgs.from_wire({"schema": "wrong", "manifest": manifest().to_wire()})

    def test_report_preserves_waves_path_and_acceptance(self) -> None:
        report = engineering_execution_plan_report(payload())
        self.assertIsInstance(report, EngineeringPlanReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.scheduled_ticket_ids, ("T-002", "T-003"))
        self.assertEqual(report.waves[1].depends_on_waves, (0,))
        self.assertEqual(report.critical_path, ("T-001", "T-002", "T-003"))
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertTrue(engineering_execution_plan_report(envelope).engineering_plan_ready)

    def test_refusal_is_fail_closed_and_typed(self) -> None:
        refused = {"ok": False, "schema": "bioprism-engineering-plan-audit/0.1", "refusal": "invalid request", "fail_closed": True}
        report = engineering_execution_plan_report(refused)
        self.assertFalse(report.accepted)
        self.assertTrue(report.fail_closed)
        with self.assertRaises(ArgumentError):
            engineering_execution_plan_report({"ok": False, "refusal": "unsafe"})

    def test_all_facades_keep_plan_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).engineering_execution_plan_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).engineering_execution_plan_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            result = ApiClient("http://127.0.0.1:1").engineering_execution_plan_report(args)
        self.assertEqual(result.critical_path[-1], "T-003")
        call.assert_called_once_with("engineering_execution_plan", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).engineering_execution_plan_report(args)
            self.assertEqual(result.waves[0].ticket_ids, ("T-002",))
            async_call.assert_called_once_with("engineering_execution_plan", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
