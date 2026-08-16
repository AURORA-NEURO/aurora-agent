from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    AdrSpecArgs,
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    EngineeringAuditReport,
    EngineeringManifestArgs,
    EngineeringPoliciesArgs,
    OwnershipSpecArgs,
    PackageSpecArgs,
    ProjectIdentityArgs,
    TechnologyBaselineArgs,
    TicketSpecArgs,
    Workspace,
    engineering_manifest_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> EngineeringManifestArgs:
    return EngineeringManifestArgs(
        project=ProjectIdentityArgs("aurora-agent", "0.1.0", "github.com/AURORA-NEURO/aurora-agent"),
        baseline=TechnologyBaselineArgs(
            "Rust 2021",
            "cargo",
            "MCP JSON-RPC",
            "in-memory",
            "structured stderr audit",
            "local process",
            {"runtime": "deterministic local execution"},
        ),
        packages=[
            PackageSpecArgs("core", "crates/core", "rust", "library", "platform"),
            PackageSpecArgs("api", "crates/api", "rust", "service", "platform", ("core",), True, "cargo test -p api"),
        ],
        tickets=[
            TicketSpecArgs("T-001", "ship core", "core", "core-contract", "done", acceptance=("core tests pass",)),
            TicketSpecArgs("T-002", "ship api", "api", "api-contract", "planned", ("T-001",), ("protocol tests pass",)),
        ],
        adrs=[AdrSpecArgs("ADR-001", "use rust", "accepted", "Rust owns canonical semantics", ("core", "api"))],
        ownership=[OwnershipSpecArgs("api", "platform-lead", ("api-team",), independent_reviewer="review-board")],
        policies=EngineeringPoliciesArgs(),
    )


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "engineering_manifest_audit",
        "schema": "bioprism-engineering-audit/0.1",
        "manifest_digest": "a" * 64,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-engineering-audit/0.1",
            "manifest_schema": "bioprism-engineering-manifest/0.1",
            "digest": "a" * 64,
            "valid": True,
            "counts": {"packages": 2, "public_packages": 1, "tickets": 2, "completed_tickets": 1, "actionable_tickets": 1, "adrs": 1, "accepted_adrs": 1, "ownership_rows": 1},
            "package_order": ["core", "api"],
            "cyclic_packages": [],
            "ticket_readiness": [
                {"ticket_id": "T-001", "status": "done", "state": "complete", "blocking_dependencies": [], "dependency_ready": True},
                {"ticket_id": "T-002", "status": "planned", "state": "actionable", "blocking_dependencies": [], "dependency_ready": True},
            ],
            "adr_supersession": [],
            "ownership_surfaces": ["api"],
            "issues": [],
            "guarantees": ["package edges are checked"],
            "limitations": ["does not run CI"],
        },
        "guarantees": ["package edges are checked"],
        "limitations": ["does not run CI"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class EngineeringManifestTests(unittest.TestCase):
    def test_args_round_trip_and_safety_bounds(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["manifest"]["packages"][1]["depends_on"], ["core"])
        self.assertEqual(EngineeringManifestArgs.from_wire(wire["manifest"]), args)
        with self.assertRaises(ArgumentError):
            EngineeringManifestArgs.from_wire({"project": {}, "baseline": {}})
        with self.assertRaises(ArgumentError):
            TicketSpecArgs("T", "ticket", "core", "contract", "unknown", acceptance=("test",))  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            EngineeringManifestArgs(request().project, request().baseline, packages=[request().packages[0]] * 4_097)

    def test_report_preserves_digest_topology_and_ticket_readiness(self) -> None:
        report = engineering_manifest_audit_report(payload())
        self.assertIsInstance(report, EngineeringAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.manifest_digest, "a" * 64)
        self.assertEqual(report.package_order, ("core", "api"))
        self.assertEqual(report.actionable_tickets, ("T-002",))
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(engineering_manifest_audit_report(envelope).counts["packages"], 2)

    def test_invalid_projection_keeps_blocking_issue_typed(self) -> None:
        invalid = payload()
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{
            "code": "package_cycle",
            "severity": "blocking",
            "subject": "api -> core",
            "detail": "cycle",
            "remediation": "break it",
        }]
        report = engineering_manifest_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertTrue(report.has_blockers)
        self.assertEqual(report.blocking_issues[0].code, "package_cycle")

    def test_all_facades_keep_engineering_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).engineering_manifest_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).engineering_manifest_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").engineering_manifest_audit_report(args)
        self.assertEqual(report.actionable_tickets, ("T-002",))
        call.assert_called_once_with("engineering_manifest_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).engineering_manifest_audit_report(args)
            self.assertEqual(result.package_order, ("core", "api"))
            async_call.assert_called_once_with("engineering_manifest_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
