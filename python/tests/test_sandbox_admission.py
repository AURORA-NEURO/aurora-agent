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
    SandboxAuditReport,
    SandboxCapabilityArgs,
    SandboxExecutionProfileArgs,
    SandboxManifestArgs,
    SandboxMountArgs,
    SandboxOutputArgs,
    SandboxResourceLimitsArgs,
    SandboxSystemArgs,
    Workspace,
    sandbox_admission_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> SandboxManifestArgs:
    return SandboxManifestArgs(
        system=SandboxSystemArgs("prism-sandbox", "0.1.0", "platform"),
        artifacts=[
            SandboxArtifactArgs("source", "source_code", "a" * 64, "repo/source.py", "ci", "reviewed"),
            SandboxArtifactArgs("dataset", "dataset", "b" * 64, "registry/dataset", "registry", "untrusted", ("source",)),
        ],
        profiles=[SandboxExecutionProfileArgs(
            "profile", "dataset", "oci", "c" * 64, "d" * 64, "runner", True, True, True,
            "allowlist", ("packages.example",), (SandboxMountArgs("input", "dataset", "/inputs/data", "read_only"),),
            ("network",), SandboxResourceLimitsArgs(1000, 1024, 60, 8, 1_000_000), True, True,
        )],
        capabilities=[SandboxCapabilityArgs("network", "profile", "network_egress", "packages.example", "allow", "e" * 64)],
        outputs=[SandboxOutputArgs("result", "profile", "dataset", "f" * 64, "quarantine", True, False, False, None, ("dataset",))],
    )


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "workflow": "sandbox_admission_audit",
        "schema": "bioprism-sandbox-audit/0.1",
        "manifest_digest": digest,
        "valid": True,
        "sandbox_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-sandbox-audit/0.1",
            "manifest_schema": "bioprism-sandbox/0.1",
            "digest": digest,
            "valid": True,
            "system_id": "prism-sandbox",
            "counts": {"artifacts": 2, "untrusted_artifacts": 1, "profiles": 1, "isolated_profiles": 1, "capabilities": 1, "approved_capabilities": 1, "dangerous_capabilities": 1, "outputs": 1, "quarantined_outputs": 1, "released_outputs": 0},
            "artifact_audits": [{"artifact_id": "dataset", "digest_valid": True, "lineage_valid": True, "source_valid": True, "trust": "untrusted", "hardening_required": True, "ready": True}],
            "profile_audits": [{"profile_id": "profile", "artifact_valid": True, "isolation_valid": True, "network_valid": True, "mounts_valid": True, "capabilities_valid": True, "resources_valid": True, "output_valid": True, "ready": True}],
            "capability_audits": [{"capability_id": "network", "profile_valid": True, "target_valid": True, "approved": True, "dangerous": True, "evidence_valid": True, "ready": True}],
            "boundary_audits": [{"profile_id": "profile", "default_deny": True, "network_mode": "allowlist", "allowlist_valid": True, "host_paths_rejected": True, "dangerous_capabilities": 1, "ready": True}],
            "resource_audits": [{"profile_id": "profile", "cpu_bounded": True, "memory_bounded": True, "wall_time_bounded": True, "processes_bounded": True, "output_bounded": True, "ready": True}],
            "output_audits": [{"output_id": "result", "profile_valid": True, "artifact_valid": True, "digest_valid": True, "lineage_valid": True, "quarantined": True, "review_valid": True, "release_valid": True, "ready": True}],
            "issues": [],
            "guarantees": ["layers remain separate"],
            "limitations": ["admission only"],
        },
        "guarantees": ["layers remain separate"],
        "limitations": ["admission only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class SandboxAdmissionTests(unittest.TestCase):
    def test_args_round_trip_digest_paths_and_bounds(self) -> None:
        args = request()
        self.assertEqual(SandboxManifestArgs.from_wire(args.to_wire()), args)
        self.assertEqual(args.to_wire()["profiles"][0]["mounts"][0]["mode"], "read_only")
        with self.assertRaises(ArgumentError):
            SandboxCapabilityArgs("cap", "profile", "network_egress", "*", "allow", "e" * 64)
        with self.assertRaises(ArgumentError):
            SandboxManifestArgs(args.system, artifacts=[args.artifacts[0]] * 4097)

    def test_report_preserves_isolation_capability_resource_and_output_layers(self) -> None:
        report = sandbox_admission_audit_report(payload())
        self.assertIsInstance(report, SandboxAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.sandbox_ready)
        self.assertEqual(report.counts["untrusted_artifacts"], 1)
        self.assertTrue(report.profile_audits[0].isolation_valid)
        self.assertTrue(report.capability_audits[0].evidence_valid)
        self.assertTrue(report.resource_audits[0].ready)
        self.assertTrue(report.output_audits[0].quarantined)

    def test_invalid_projection_keeps_sandbox_blockers_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["sandbox_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{"code": "rootless_required", "severity": "blocking", "subject": "profile", "detail": "missing", "remediation": "enable"}, {"code": "released_output_unreviewed", "severity": "blocking", "subject": "result", "detail": "missing", "remediation": "review"}]
        report = sandbox_admission_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertTrue(report.has_blockers)
        self.assertEqual({issue.code for issue in report.blocking_issues}, {"rootless_required", "released_output_unreviewed"})

    def test_all_facades_keep_sandbox_admission_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).sandbox_admission_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).sandbox_admission_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").sandbox_admission_audit_report(args)
        self.assertTrue(report.sandbox_ready)
        call.assert_called_once_with("sandbox_admission_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).sandbox_admission_audit_report(args)
            self.assertEqual(result.output_audits[0].output_id, "result")
            async_call.assert_called_once_with("sandbox_admission_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
