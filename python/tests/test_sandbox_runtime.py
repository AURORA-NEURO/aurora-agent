from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    SandboxArtifactArgs,
    SandboxCapabilityArgs,
    SandboxExecutionProfileArgs,
    SandboxManifestArgs,
    SandboxMountArgs,
    SandboxResourceLimitsArgs,
    SandboxRuntimeAuditReport,
    SandboxRuntimeManifestArgs,
    SandboxRuntimePoliciesArgs,
    SandboxRuntimeRequestArgs,
    SandboxSystemArgs,
    Workspace,
    sandbox_runtime_simulate_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def admission() -> SandboxManifestArgs:
    return SandboxManifestArgs(
        system=SandboxSystemArgs("runtime", "0.1.0", "platform"),
        artifacts=[
            SandboxArtifactArgs("source", "source_code", "a" * 64, "repo/source.py", "ci", "reviewed"),
            SandboxArtifactArgs("dataset", "dataset", "b" * 64, "registry/dataset", "registry", "untrusted", ("source",)),
        ],
        profiles=[SandboxExecutionProfileArgs(
            "profile", "dataset", "oci", "c" * 64, "d" * 64, "runner", True, True, True,
            "allowlist", ("packages.example",), (SandboxMountArgs("input", "dataset", "/inputs/data", "read_only"),),
            ("read", "network"), SandboxResourceLimitsArgs(1000, 1024, 60, 8, 1_000_000), True, True,
        )],
        capabilities=[
            SandboxCapabilityArgs("read", "profile", "filesystem_read", "/inputs/data", "allow", None),
            SandboxCapabilityArgs("network", "profile", "network_egress", "packages.example", "allow", "e" * 64),
        ],
    )


def request(id: str, kind: str, target: str, cpu: int = 100) -> SandboxRuntimeRequestArgs:
    return SandboxRuntimeRequestArgs(id, kind, target, cpu, 128, 5, 1, 1000)


def args() -> SandboxRuntimeManifestArgs:
    return SandboxRuntimeManifestArgs(
        admission=admission(),
        profile="profile",
        requests=(request("read-input", "filesystem_read", "/inputs/data"), request("fetch", "network_egress", "packages.example")),
        policies=SandboxRuntimePoliciesArgs(),
    )


def payload() -> dict:
    return {
        "ok": True,
        "workflow": "sandbox_runtime_simulate",
        "schema": "bioprism-sandbox-runtime-audit/0.1",
        "manifest_digest": "a" * 64,
        "admission_digest": "b" * 64,
        "trace_digest": "c" * 64,
        "valid": True,
        "sandbox_runtime_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-sandbox-runtime-audit/0.1",
            "manifest_schema": "bioprism-sandbox-runtime/0.1",
            "admission_digest": "b" * 64,
            "trace_digest": "c" * 64,
            "valid": True,
            "profile_id": "profile",
            "admission_valid": True,
            "simulation_started": True,
            "completed": True,
            "stopped_on_refusal": False,
            "request_count": 2,
            "simulated_count": 2,
            "refused_count": 0,
            "not_run_count": 0,
            "usage": {"cpu_millis": 200, "memory_mb_peak": 128, "wall_time_seconds": 10, "processes_peak": 1, "output_bytes": 2000},
            "steps": [
                {"request_id": "read-input", "kind": "filesystem_read", "target": "/inputs/data", "capability_id": "read", "capability_valid": True, "target_valid": True, "resource_valid": True, "decision": "simulated", "charged": True, "usage_after": {"cpu_millis": 100, "memory_mb_peak": 128, "wall_time_seconds": 5, "processes_peak": 1, "output_bytes": 1000}, "refusal": None},
                {"request_id": "fetch", "kind": "network_egress", "target": "packages.example", "capability_id": "network", "capability_valid": True, "target_valid": True, "resource_valid": True, "decision": "simulated", "charged": True, "usage_after": {"cpu_millis": 200, "memory_mb_peak": 128, "wall_time_seconds": 10, "processes_peak": 1, "output_bytes": 2000}, "refusal": None},
            ],
            "admission_issues": [],
            "issues": [],
            "guarantees": ["decisions remain traceable"],
            "limitations": ["simulation only"],
        },
        "guarantees": ["decisions remain traceable"],
        "limitations": ["simulation only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class SandboxRuntimeTests(unittest.TestCase):
    def test_args_round_trip_and_positive_bounded_charges(self) -> None:
        value = args()
        self.assertEqual(SandboxRuntimeManifestArgs.from_wire(value.to_wire()), value)
        self.assertEqual(value.to_wire()["requests"][0]["kind"], "filesystem_read")
        with self.assertRaises(ArgumentError):
            request("bad", "filesystem_read", "/proc/self")
        with self.assertRaises(ArgumentError):
            SandboxRuntimeManifestArgs(admission(), "profile", tuple(request(str(i), "clock", "clock") for i in range(4097)))

    def test_report_preserves_trace_usage_and_decisions(self) -> None:
        report = sandbox_runtime_simulate_report(payload())
        self.assertIsInstance(report, SandboxRuntimeAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.simulated_count, 2)
        self.assertEqual(report.usage.cpu_millis, 200)
        self.assertEqual(report.steps[0].capability_id, "read")
        self.assertTrue(report.steps[1].charged)

    def test_invalid_projection_keeps_runtime_refusal_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["sandbox_runtime_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["refused_count"] = 1
        invalid["audit"]["not_run_count"] = 1
        invalid["audit"]["stopped_on_refusal"] = True
        invalid["audit"]["issues"] = [{"code": "resource_budget_exceeded", "severity": "blocking", "subject": "fetch", "detail": "over budget", "remediation": "reduce charge"}]
        report = sandbox_runtime_simulate_report(invalid)
        self.assertFalse(report.accepted)
        self.assertTrue(report.has_blockers)
        self.assertEqual(report.blocking_issues[0].code, "resource_budget_exceeded")

    def test_all_facades_keep_runtime_simulation_typed(self) -> None:
        value = args()
        self.assertTrue(Workspace(_SyncTool()).sandbox_runtime_simulate_report(value).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).sandbox_runtime_simulate_report(value)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").sandbox_runtime_simulate_report(value)
        self.assertEqual(report.trace_digest, "c" * 64)
        call.assert_called_once_with("sandbox_runtime_simulate", value.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).sandbox_runtime_simulate_report(value)
            self.assertEqual(result.steps[1].decision, "simulated")
            async_call.assert_called_once_with("sandbox_runtime_simulate", value.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
