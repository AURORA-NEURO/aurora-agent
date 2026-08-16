from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PipelineArtifactArgs,
    PipelineAttestationArgs,
    PipelineEnvironmentArgs,
    PipelinePromotionArgs,
    PipelineProjectArgs,
    PipelineSourceArgs,
    PipelineStageArgs,
    ReleasePipelineAuditReport,
    ReleasePipelineManifestArgs,
    Workspace,
    release_pipeline_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> ReleasePipelineManifestArgs:
    digest = "a" * 64
    return ReleasePipelineManifestArgs(
        project=PipelineProjectArgs("aurora-agent", "0.1.0", "github.com/AURORA-NEURO/aurora-agent"),
        source=PipelineSourceArgs("main", digest, "release.yml"),
        environments=[
            PipelineEnvironmentArgs("staging", "staging", True, 0, True, True),
            PipelineEnvironmentArgs("production", "production", True, 1, True, True),
        ],
        stages=[
            PipelineStageArgs("build", "build", "staging", produces=("binary",)),
            PipelineStageArgs("test", "test", "staging", ("build",), "cargo test --locked"),
        ],
        artifacts=[PipelineArtifactArgs("binary", "binary", digest, "build", attestations=("prov", "sig"), immutable=True)],
        attestations=[
            PipelineAttestationArgs("prov", "provenance", "binary", digest, "ci", "built from pinned source"),
            PipelineAttestationArgs("sig", "signature", "binary", digest, "release-key", "signed artifact"),
            PipelineAttestationArgs("approval", "approval", "binary", digest, "release-board", "approved"),
        ],
        promotions=[
            PipelinePromotionArgs("to-production", "advance", "staging", "production", ("binary",), ("prov", "sig"), ("approval",), "rollback"),
            PipelinePromotionArgs("rollback", "rollback", "production", "staging", ("binary",), ("prov",)),
        ],
    )


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "workflow": "release_pipeline_audit",
        "schema": "bioprism-release-pipeline-audit/0.1",
        "manifest_digest": digest,
        "valid": True,
        "release_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-release-pipeline-audit/0.1",
            "manifest_schema": "bioprism-release-pipeline/0.1",
            "digest": digest,
            "valid": True,
            "counts": {"environments": 2, "protected_environments": 2, "stages": 2, "required_stages": 2, "artifacts": 1, "attestations": 3, "promotions": 2, "production_promotions": 1},
            "stage_order": ["build", "test"],
            "cyclic_stages": [],
            "stage_readiness": [{"stage_id": "build", "state": "ready_to_schedule", "dependency_ready": True, "blocking_dependencies": []}],
            "artifact_audits": [{"artifact_id": "binary", "digest_valid": True, "producer_valid": True, "inputs_valid": True, "attestations_valid": True, "provenance_present": True, "signature_present": True}],
            "promotion_audits": [{"promotion_id": "to-production", "from": "staging", "to": "production", "valid": True, "production": True, "missing_attestations": [], "missing_approvals": [], "rollback_present": True}],
            "issues": [],
            "guarantees": ["stage and provenance layers remain separate"],
            "limitations": ["does not execute CI"],
        },
        "guarantees": ["stage and provenance layers remain separate"],
        "limitations": ["does not execute CI"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class ReleasePipelineTests(unittest.TestCase):
    def test_args_round_trip_digest_and_bounds(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["manifest"]["promotions"][0]["rollback_target"], "rollback")
        self.assertEqual(ReleasePipelineManifestArgs.from_wire(wire["manifest"]), args)
        with self.assertRaises(ArgumentError):
            PipelineSourceArgs("main", "not-a-digest", "release.yml")
        with self.assertRaises(ArgumentError):
            ReleasePipelineManifestArgs(args.project, args.source, environments=[args.environments[0]] * 257)

    def test_report_preserves_release_ready_provenance_and_rollback(self) -> None:
        report = release_pipeline_audit_report(payload())
        self.assertIsInstance(report, ReleasePipelineAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.release_ready)
        self.assertEqual(report.stage_order, ("build", "test"))
        self.assertEqual(report.production_promotions[0].rollback_present, True)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(release_pipeline_audit_report(envelope).counts["artifacts"], 1)

    def test_invalid_projection_keeps_provenance_blocker_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["release_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{"code": "production_signature_missing", "severity": "blocking", "subject": "to-production", "detail": "signature missing", "remediation": "sign the artifact"}]
        report = release_pipeline_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertFalse(report.release_ready)
        self.assertTrue(report.has_blockers)
        self.assertEqual(report.blocking_issues[0].code, "production_signature_missing")

    def test_all_facades_keep_release_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).release_pipeline_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).release_pipeline_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").release_pipeline_audit_report(args)
        self.assertTrue(report.release_ready)
        call.assert_called_once_with("release_pipeline_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).release_pipeline_audit_report(args)
            self.assertEqual(result.production_promotions[0].promotion_id, "to-production")
            async_call.assert_called_once_with("release_pipeline_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
