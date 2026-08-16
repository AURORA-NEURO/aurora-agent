from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    SecurityPrivacyAssetArgs,
    SecurityPrivacyAuditReport,
    SecurityPrivacyControlsArgs,
    SecurityPrivacyFlowArgs,
    SecurityPrivacyIdentityArgs,
    SecurityPrivacyManifestArgs,
    SecurityPrivacyReviewArgs,
    SecurityPrivacySystemArgs,
    SecurityPrivacyThreatArgs,
    Workspace,
    security_privacy_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> SecurityPrivacyManifestArgs:
    digest = "a" * 64
    return SecurityPrivacyManifestArgs(
        system=SecurityPrivacySystemArgs("aurora-api", "0.1.0", "platform"),
        assets=[SecurityPrivacyAssetArgs("patient-records", "records", "regulated", "privacy", "care research", 365, "us", "erase workflow")],
        flows=[SecurityPrivacyFlowArgs("api-to-vendor", "patient-records", "api", "approved-vendor", "care research", "allow", "consent", digest)],
        identities=[SecurityPrivacyIdentityArgs("researcher", "team", "research", "oidc", True, True, ("patient-records",))],
        threats=[SecurityPrivacyThreatArgs("exfiltration", "data-exfiltration", "high", "mitigated", "dlp", digest)],
        reviews=[SecurityPrivacyReviewArgs("pia-1", "privacy_impact", "patient-records", "independent-reviewer", "complete", digest, "2027-01-01", ("none",))],
        controls=SecurityPrivacyControlsArgs(True, True, True, True, True, True, True, True, True, True),
    )


def payload() -> dict:
    digest = "a" * 64
    return {
        "ok": True,
        "workflow": "security_privacy_audit",
        "schema": "bioprism-security-privacy-audit/0.1",
        "manifest_digest": digest,
        "valid": True,
        "security_privacy_ready": True,
        "blocking_issue_count": 0,
        "warning_count": 0,
        "audit": {
            "schema": "bioprism-security-privacy-audit/0.1",
            "manifest_schema": "bioprism-security-privacy/0.1",
            "digest": digest,
            "valid": True,
            "system_id": "aurora-api",
            "counts": {"assets": 1, "sensitive_assets": 1, "flows": 1, "allowed_flows": 1, "identities": 1, "hardened_identities": 1, "threats": 1, "high_or_worse_threats": 1, "treated_threats": 1, "reviews": 1, "current_reviews": 1, "controls": 10, "enabled_controls": 10},
            "asset_audits": [{"asset_id": "patient-records", "purpose_valid": True, "retention_valid": True, "residency_valid": True, "deletion_valid": True, "sensitive": True, "ready": True}],
            "flow_audits": [{"flow_id": "api-to-vendor", "asset_valid": True, "purpose_valid": True, "legal_basis_present": True, "authorization_present": True, "allowed": True, "ready": True}],
            "identity_audits": [{"identity_id": "researcher", "assets_valid": True, "authentication_valid": True, "mfa": True, "least_privilege": True, "sensitive_access": True, "ready": True}],
            "threat_audits": [{"threat_id": "exfiltration", "high_or_worse": True, "treated": True, "control_present": True, "evidence_valid": True, "rationale_present": False, "ready": True}],
            "review_audits": [{"review_id": "pia-1", "reviewer_independent": True, "evidence_valid": True, "current": True, "complete": True, "ready": True}],
            "control_audits": [{"control": "encryption_at_rest", "enabled": True, "required": True, "ready": True}],
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


class SecurityPrivacyTests(unittest.TestCase):
    def test_args_round_trip_digest_and_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_wire()["assets"][0]["classification"], "regulated")
        self.assertEqual(SecurityPrivacyManifestArgs.from_wire(args.to_wire()), args)
        with self.assertRaises(ArgumentError):
            SecurityPrivacyFlowArgs("flow", "asset", "api", "vendor", "research", "allow", "consent", "bad")
        with self.assertRaises(ArgumentError):
            SecurityPrivacyManifestArgs(args.system, assets=[args.assets[0]] * 4097)

    def test_report_preserves_governance_layers(self) -> None:
        report = security_privacy_audit_report(payload())
        self.assertIsInstance(report, SecurityPrivacyAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.security_privacy_ready)
        self.assertEqual(report.counts["sensitive_assets"], 1)
        self.assertTrue(report.flow_audits[0].authorization_present)
        self.assertTrue(report.threat_audits[0].treated)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(security_privacy_audit_report(envelope).counts["enabled_controls"], 10)

    def test_invalid_projection_keeps_sensitive_governance_blockers_typed(self) -> None:
        invalid = payload()
        invalid["valid"] = False
        invalid["security_privacy_ready"] = False
        invalid["audit"]["valid"] = False
        invalid["audit"]["issues"] = [{"code": "flow_authorization_missing", "severity": "blocking", "subject": "api-to-vendor", "detail": "missing", "remediation": "approve"}, {"code": "sensitive_mfa_missing", "severity": "blocking", "subject": "researcher", "detail": "missing", "remediation": "enable"}]
        report = security_privacy_audit_report(invalid)
        self.assertFalse(report.accepted)
        self.assertTrue(report.has_blockers)
        self.assertEqual({issue.code for issue in report.blocking_issues}, {"flow_authorization_missing", "sensitive_mfa_missing"})

    def test_all_facades_keep_security_privacy_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).security_privacy_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).security_privacy_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").security_privacy_audit_report(args)
        self.assertTrue(report.security_privacy_ready)
        call.assert_called_once_with("security_privacy_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).security_privacy_audit_report(args)
            self.assertEqual(result.asset_audits[0].asset_id, "patient-records")
            async_call.assert_called_once_with("security_privacy_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
