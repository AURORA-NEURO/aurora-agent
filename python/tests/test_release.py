from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    BundleVerifyArgs,
    BundleVerifyReport,
    ReleaseAuditArgs,
    ReleaseAuditCheckReport,
    ReleaseAuditCheckRequest,
    ReleaseAuditReport,
    release_audit_report,
)


def release_payload(*, blocked: bool = False) -> dict:
    if blocked:
        return {
            "ok": True,
            "release_ready": False,
            "required_check_count": 1,
            "check_count": 1,
            "invocation_failures": 1,
            "blocking_count": 1,
            "blockers": [{"index": 0, "kind": "conformance_run", "reason": "fixture suite refused", "fail_closed": True}],
            "checks": [{
                "index": 0,
                "kind": "conformance_run",
                "required": True,
                "advisory": False,
                "evaluated": False,
                "gate": None,
                "passed": False,
                "refusal": "fixture suite refused",
                "fail_closed": True,
            }],
            "guarantees": ["required gates are conjunctive"],
            "limitations": ["local evidence only"],
        }
    return {
        "ok": True,
        "release_ready": True,
        "required_check_count": 1,
        "check_count": 2,
        "invocation_failures": 0,
        "blocking_count": 0,
        "blockers": [],
        "checks": [
            {
                "index": 0,
                "kind": "bundle_verify",
                "required": True,
                "advisory": False,
                "evaluated": True,
                "gate": True,
                "passed": True,
                "result_digest": "d" * 64,
                "result_ok": True,
                "fail_closed": True,
                "result": {"ok": True, "verified": True},
            },
            {
                "index": 1,
                "kind": "repository_impact",
                "required": False,
                "advisory": True,
                "evaluated": True,
                "gate": None,
                "passed": False,
                "result_digest": "i" * 64,
                "result_ok": True,
                "fail_closed": False,
            },
        ],
        "guarantees": ["advisory evidence cannot offset a blocker"],
        "limitations": ["does not publish or deploy"],
    }


class ReleaseAuditTests(unittest.TestCase):
    def test_request_preserves_defaults_and_advisory_only_policy(self) -> None:
        request = ReleaseAuditArgs([
            ReleaseAuditCheckRequest("bundle_verify", {"bundle": {"id": "b"}}),
            {"kind": "repository_impact", "arguments": {"changed": "docs/README"}},
        ], include_details=True)
        self.assertEqual(request.to_mcp_arguments()["checks"][0]["kind"], "bundle_verify")
        self.assertNotIn("required", request.to_mcp_arguments()["checks"][1])
        self.assertTrue(request.to_mcp_arguments()["include_details"])
        with self.assertRaises(ArgumentError):
            ReleaseAuditCheckRequest("repository_impact", required=True)

    def test_report_reconciles_strict_required_conjunction_and_advisory_null_gate(self) -> None:
        report = ReleaseAuditReport.from_wire(release_payload())
        self.assertTrue(report.release_ready)
        self.assertEqual(report.required_checks[0].kind, "bundle_verify")
        self.assertEqual(report.advisory_checks[0].gate, None)
        self.assertEqual(report.failed_checks[0].kind, "repository_impact")
        self.assertEqual(report.refused_checks, ())
        self.assertTrue(report.details_included)

    def test_report_preserves_refusal_as_invocation_failure_and_blocker(self) -> None:
        report = release_audit_report({"ok": True, "mcp": {"result": {"structuredContent": release_payload(blocked=True)}}})
        self.assertFalse(report.release_ready)
        self.assertEqual(report.invocation_failures, 1)
        self.assertEqual(report.blockers[0].reason, "fixture suite refused")
        self.assertEqual(report.refused_checks[0].fail_closed, True)

    def test_report_rejects_compensating_advisory_or_count_drift(self) -> None:
        payload = release_payload()
        payload["checks"][1]["gate"] = True
        with self.assertRaises(ArgumentError):
            ReleaseAuditCheckReport.from_wire(payload["checks"][1])
        payload = release_payload()
        payload["release_ready"] = False
        with self.assertRaises(ArgumentError):
            ReleaseAuditReport.from_wire(payload)

    def test_bundle_verify_request_requires_exactly_one_source_and_key_for_public_attestation(self) -> None:
        with self.assertRaises(ArgumentError):
            BundleVerifyArgs()
        with self.assertRaises(ArgumentError):
            BundleVerifyArgs(publicly_attested_bundle={"bundle": {}})
        request = BundleVerifyArgs(
            publicly_attested_bundle={"bundle": {}},
            verification_key={
                "key_identity": "publisher-ed25519",
                "public_key": "ed25519:" + "a" * 64,
                "validity": {"not_before": 10, "not_after": 20},
            },
        )
        self.assertIn("publicly_attested_bundle", request.to_mcp_arguments())
        self.assertEqual(request.to_mcp_arguments()["verification_key"]["key_identity"], "publisher-ed25519")

    def test_bundle_verify_request_supports_explicit_registry_policy_mode(self) -> None:
        with self.assertRaises(ArgumentError):
            BundleVerifyArgs(
                publicly_attested_bundle={"bundle": {}},
                trust_registry={"schema_version": "bioprism-key-registry/0.1"},
            )
        request = BundleVerifyArgs(
            document="signed-bundle.json",
            trust_registry={
                "schema_version": "bioprism-key-registry/0.1",
                "keys": {},
                "delegations": {},
                "rotations": {},
                "revocations": {},
            },
            trust_policy={
                "purpose": "publisher_manifest",
                "require_producer_binding": True,
                "max_delegation_depth": 32,
            },
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["trust_policy"]["purpose"], "publisher_manifest")
        self.assertIn("trust_registry", arguments)

    def test_bundle_verify_report_keeps_public_verification_and_fail_closed_refusal_typed(self) -> None:
        report = BundleVerifyReport.from_wire({
            "ok": True,
            "verification_mode": "ed25519_public_key",
            "manifest_digest": "d" * 64,
            "authentication": {"scheme": "ed25519_public_key"},
            "trust_report": {"verdict": "trusted"},
            "guarantees": ["signature checked"],
            "limitations": ["key registry external"],
        })
        self.assertTrue(report.is_public_key_verified)
        self.assertEqual(report.manifest_digest, "d" * 64)
        policy_report = BundleVerifyReport.from_wire({
            "ok": True,
            "verification_mode": "ed25519_registry_policy",
            "manifest_digest": "e" * 64,
            "trust_report": {"verdict": "trusted"},
        })
        self.assertTrue(policy_report.is_public_key_verified)
        self.assertTrue(policy_report.is_trust_policy_verified)
        refused = BundleVerifyReport.from_wire({
            "ok": False,
            "refusal": "signature mismatch",
            "fail_closed": True,
        })
        self.assertFalse(refused.ok)
        self.assertEqual(refused.refusal, "signature mismatch")


if __name__ == "__main__":
    unittest.main()
