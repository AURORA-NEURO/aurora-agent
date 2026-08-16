from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    SecurityProgramAuditReport,
    SecurityProgramCampaignArgs,
    SecurityProgramControlsArgs,
    SecurityProgramDisclosureArgs,
    SecurityProgramFindingArgs,
    SecurityProgramIncidentArgs,
    SecurityProgramManifestArgs,
    SecurityProgramRemediationArgs,
    SecurityProgramScopeArgs,
    SecurityProgramSystemArgs,
    SecurityProgramTimelineEventArgs,
    Workspace,
    security_program_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> SecurityProgramManifestArgs:
    digest = "a" * 64
    return SecurityProgramManifestArgs(
        system=SecurityProgramSystemArgs("aurora-security", "0.1.0", "security-owner", "bounded adversarial assurance"),
        scopes=[SecurityProgramScopeArgs("api-staging", "staging API", "api", "api-staging.internal", "service-owner", digest, ("authenticated-read",), ("production-write",), ("isolated-staging",), "synthetic fixtures only")],
        campaigns=[SecurityProgramCampaignArgs("campaign-1", "api-staging", "red-team", "bounded mutation", "boundary crossing", "completed", "independent-reviewer", "2026-01-01", "2026-01-02", digest, ("stop on production boundary",), ("finding-1",))],
        findings=[SecurityProgramFindingArgs("finding-1", "campaign-1", "boundary mismatch", "high", "closed", "2026-01-02", digest, digest, digest, ("api-staging",), ("remediation-1",), "incident-1", True)],
        remediations=[SecurityProgramRemediationArgs("remediation-1", "finding-1", "service-owner", "validate boundary", "complete", "2026-01-10", digest)],
        incidents=[SecurityProgramIncidentArgs("incident-1", "finding-1", "high", "incident-owner", "closed", "2026-01-02", "2026-01-02", "2026-01-03", digest, digest, True, (SecurityProgramTimelineEventArgs(1, "incident-owner", "opened", digest),))],
        disclosures=[SecurityProgramDisclosureArgs("advisory-1", "finding-1", "advisory", "affected operators", "2026-01-04", "independent-reviewer", digest, digest, "2026-01-04")],
        controls=SecurityProgramControlsArgs(True, True, True, True, True, True, True, True),
    )


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "workflow": "security_program_audit",
        "schema": "bioprism-security-program-audit/0.1",
        "manifest_digest": digest,
        "valid": True,
        "security_program_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-security-program-audit/0.1",
            "manifest_schema": "bioprism-security-program/0.1",
            "digest": digest,
            "valid": True,
            "system_id": "aurora-security",
            "counts": {"scopes": 1, "authorized_scopes": 1, "campaigns": 1, "completed_campaigns": 1, "findings": 1, "high_or_worse_findings": 1, "actionable_findings": 0, "remediations": 1, "completed_remediations": 1, "incidents": 1, "open_incidents": 0, "closed_incidents": 1, "disclosures": 1, "advisory_disclosures": 1, "public_disclosures": 0, "enabled_controls": 8},
            "scope_audits": [{"scope_id": "api-staging", "authorization_valid": True, "methods_valid": True, "guardrails_valid": True, "environments_valid": True, "ready": True}],
            "campaign_audits": [{"campaign_id": "campaign-1", "scope_valid": True, "operator_present": True, "independent_review_valid": True, "methodology_valid": True, "evidence_valid": True, "complete": True, "ready": True}],
            "finding_audits": [{"finding_id": "finding-1", "campaign_valid": True, "evidence_valid": True, "reproduction_valid": True, "severity_requires_action": True, "remediation_valid": True, "incident_required": True, "incident_valid": True, "regression_present": True, "ready": True}],
            "remediation_audits": [{"remediation_id": "remediation-1", "finding_valid": True, "owner_valid": True, "completion_valid": True, "verification_valid": True, "ready": True}],
            "incident_audits": [{"incident_id": "incident-1", "finding_valid": True, "timeline_valid": True, "containment_valid": True, "closure_valid": True, "notification_valid": True, "ready": True}],
            "disclosure_audits": [{"disclosure_id": "advisory-1", "finding_valid": True, "stage_order_valid": True, "approval_valid": True, "advisory_valid": True, "publication_valid": True, "ready": True}],
            "control_audits": [{"control": "independent_review", "enabled": True, "required": True, "ready": True}],
            "issues": [],
            "guarantees": ["program layers remain separate"],
            "limitations": ["declaration only"],
        },
        "guarantees": ["program layers remain separate"],
        "limitations": ["declaration only"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class SecurityProgramTests(unittest.TestCase):
    def test_args_round_trip_and_digest_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_wire()["scopes"][0]["kind"], "api")
        self.assertEqual(SecurityProgramManifestArgs.from_wire(args.to_wire()), args)
        with self.assertRaises(ArgumentError):
            SecurityProgramScopeArgs("scope", "scope", "api", "*", "owner", "bad", ("read",), ("write",), ("staging",))
        with self.assertRaises(ArgumentError):
            SecurityProgramManifestArgs(args.system, scopes=[args.scopes[0]] * 4097)

    def test_report_preserves_program_layers(self) -> None:
        report = security_program_audit_report(payload())
        self.assertIsInstance(report, SecurityProgramAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.security_program_ready)
        self.assertTrue(report.finding_audits[0].incident_valid)
        self.assertTrue(report.remediation_audits[0].verification_valid)
        self.assertTrue(report.disclosure_audits[0].approval_valid)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(security_program_audit_report(envelope).counts["enabled_controls"], 8)

    def test_invalid_projection_keeps_program_blockers_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["security_program_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{"code": "scope_authorization_missing", "severity": "blocking", "subject": "api-staging", "detail": "missing", "remediation": "approve"}, {"code": "incident_closure_missing", "severity": "blocking", "subject": "incident-1", "detail": "missing", "remediation": "close"}]
        report = security_program_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertTrue(report.has_blockers)
        self.assertEqual({issue.code for issue in report.blocking_issues}, {"scope_authorization_missing", "incident_closure_missing"})

    def test_all_facades_keep_security_program_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).security_program_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).security_program_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").security_program_audit_report(args)
        self.assertTrue(report.security_program_ready)
        call.assert_called_once_with("security_program_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).security_program_audit_report(args)
            self.assertEqual(result.scope_audits[0].scope_id, "api-staging")
            async_call.assert_called_once_with("security_program_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
