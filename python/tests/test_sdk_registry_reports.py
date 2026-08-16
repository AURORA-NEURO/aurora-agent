from __future__ import annotations

import unittest

from prism_sdk import ArgumentError, SdkRegistryCheckArgs, SdkRegistryCheckReport, sdk_registry_check_report


class SdkRegistryReportTests(unittest.TestCase):
    def test_manifest_validation_refusal_has_no_partial_registry(self) -> None:
        report = sdk_registry_check_report({
            "ok": False,
            "stage": "manifest_validation",
            "manifests": [{"index": 0, "valid": False, "refusal": "invalid plugin manifest"}],
            "registry": None,
            "fail_closed": True,
            "guarantees": ["no partial registry"],
        })
        self.assertIsInstance(report, SdkRegistryCheckReport)
        self.assertFalse(report.ok)
        self.assertTrue(report.fail_closed)
        self.assertTrue(report.partial_registry_absent)
        self.assertEqual(report.manifests[0]["valid"], False)

    def test_success_preserves_digests_trust_and_admission_shape(self) -> None:
        report = sdk_registry_check_report({
            "ok": True,
            "manifest_count": 1,
            "manifests": [{
                "index": 0,
                "id": "plugin@1.0.0",
                "valid": True,
                "validation_error": None,
                "digest": "sha256:whole",
                "core_digest": "sha256:core",
                "capability_kinds": ["adapter"],
                "trust": {"tier": "reviewed"},
            }],
            "registry": {"registration_count": 0, "resolution": {}, "registrations": [], "policy": {}},
            "conformance_note": "registration is not execution",
            "guarantees": [],
        })
        self.assertTrue(report.admitted)
        self.assertEqual(report.manifests[0]["core_digest"], "sha256:core")
        self.assertEqual(report.registry["registration_count"], 0)

    def test_registry_request_bounds_manifest_count(self) -> None:
        request = SdkRegistryCheckArgs(({"id": "plugin"},))
        self.assertEqual(request.to_mcp_arguments()["manifests"][0]["id"], "plugin")
        with self.assertRaises(ArgumentError):
            SdkRegistryCheckArgs(tuple({"id": str(index)} for index in range(257)))


if __name__ == "__main__":
    unittest.main()
