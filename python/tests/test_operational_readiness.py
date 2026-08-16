from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    OperationalControlsArgs,
    OperationalContractArgs,
    OperationalDependencyArgs,
    OperationalIncidentArgs,
    OperationalIndicatorArgs,
    OperationalReadinessAuditReport,
    OperationalReadinessManifestArgs,
    OperationalRunbookArgs,
    OperationalServiceArgs,
    Workspace,
    operational_readiness_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> OperationalReadinessManifestArgs:
    digest = "a" * 64
    return OperationalReadinessManifestArgs(
        service=OperationalServiceArgs("aurora-api", "0.1.0", "platform", "critical"),
        contracts=[OperationalContractArgs("availability", "availability", "serve requests", "99.9%", True)],
        indicators=[OperationalIndicatorArgs("availability-indicator", "availability", "availability_ratio", "metrics", "observed", "0.999", digest)],
        dependencies=[OperationalDependencyArgs("registry", "registry", "platform", "critical", "unavailable", "cached")],
        runbooks=[OperationalRunbookArgs("api-degraded", "availability alert", "oncall", ("triage", "fail over"), "reviewed", ("availability",))],
        incidents=[OperationalIncidentArgs("INC-1", "sev2", "closed", "api-degraded", "oncall", ("detected", "resolved"), "learned")],
        controls=OperationalControlsArgs(True, True, True, True, True, True, True),
    )


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "workflow": "operational_readiness_audit",
        "schema": "bioprism-operational-readiness-audit/0.1",
        "manifest_digest": digest,
        "valid": True,
        "operationally_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-operational-readiness-audit/0.1",
            "manifest_schema": "bioprism-operational-readiness/0.1",
            "digest": digest,
            "valid": True,
            "service_id": "aurora-api",
            "counts": {"contracts": 1, "required_contracts": 1, "indicators": 1, "observed_indicators": 1, "dependencies": 1, "critical_dependencies": 1, "runbooks": 1, "incidents": 1, "open_incidents": 0, "controls": 7, "enabled_controls": 7},
            "indicator_audits": [{"indicator_id": "availability-indicator", "contract_valid": True, "source_valid": True, "observed": True, "evidence_valid": True, "ready": True}],
            "dependency_audits": [{"dependency_id": "registry", "owner_valid": True, "failure_mode_valid": True, "fallback_present": True, "critical": True, "ready": True}],
            "runbook_audits": [{"runbook_id": "api-degraded", "valid": True, "review_current": True, "step_count": 2, "referenced_incidents": 1}],
            "incident_audits": [{"incident_id": "INC-1", "valid": True, "runbook_valid": True, "timeline_present": True, "postmortem_present": True, "closed": True}],
            "control_audits": [{"control": "on_call", "enabled": True, "required": True, "ready": True}],
            "issues": [],
            "guarantees": ["layers remain separate"],
            "limitations": ["artifact only"],
        },
        "guarantees": ["layers remain separate"],
        "limitations": ["artifact only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class OperationalReadinessTests(unittest.TestCase):
    def test_args_round_trip_digest_and_bounds(self) -> None:
        args = request()
        wire = args.to_mcp_arguments()
        self.assertEqual(wire["manifest"]["service"]["criticality"], "critical")
        self.assertEqual(OperationalReadinessManifestArgs.from_wire(wire["manifest"]), args)
        with self.assertRaises(ArgumentError):
            OperationalIndicatorArgs("indicator", "availability", "ratio", "metrics", "observed", evidence_digest="not-a-digest")
        with self.assertRaises(ArgumentError):
            OperationalReadinessManifestArgs(args.service, contracts=[args.contracts[0]] * 4097)

    def test_report_preserves_each_operational_layer(self) -> None:
        report = operational_readiness_audit_report(payload())
        self.assertIsInstance(report, OperationalReadinessAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.operationally_ready)
        self.assertEqual(report.counts["observed_indicators"], 1)
        self.assertTrue(report.dependency_audits[0].fallback_present)
        self.assertTrue(report.incident_audits[0].postmortem_present)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(operational_readiness_audit_report(envelope).counts["controls"], 7)

    def test_invalid_projection_keeps_observation_and_control_blockers_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["operationally_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{"code": "indicator_not_observed", "severity": "blocking", "subject": "availability-indicator", "detail": "not observed", "remediation": "collect it"}, {"code": "required_control_disabled", "severity": "blocking", "subject": "restore_test", "detail": "disabled", "remediation": "test restore"}]
        report = operational_readiness_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertFalse(report.operationally_ready)
        self.assertTrue(report.has_blockers)
        self.assertEqual({issue.code for issue in report.blocking_issues}, {"indicator_not_observed", "required_control_disabled"})

    def test_all_facades_keep_operational_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).operational_readiness_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).operational_readiness_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").operational_readiness_audit_report(args)
        self.assertTrue(report.operationally_ready)
        call.assert_called_once_with("operational_readiness_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).operational_readiness_audit_report(args)
            self.assertEqual(result.indicator_audits[0].indicator_id, "availability-indicator")
            async_call.assert_called_once_with("operational_readiness_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
