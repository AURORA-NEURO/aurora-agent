from __future__ import annotations

import unittest

from prism_sdk import ArgumentError, ProviderCapabilityGateArgs, ProviderCapabilityGateReport, provider_capability_gate_report


def provider_payload(*, cleared: bool = False) -> dict:
    state = {"state": "passed", "run": {"run_id": "run-1", "reproducible_environment": "image@sha256:x"}} if cleared else {"state": "untested"}
    return {
        "ok": True,
        "provider": "runtime-a",
        "required": ["host_escape"],
        "required_states": {"HostEscape": state},
        "gate": {"outcome": "cleared"} if cleared else {"outcome": "blocked", "unproven": ["HostEscape=untested"]},
        "claims": ["host_escape"] if cleared else [],
        "measurement_count": 1,
        "differential": {"HostEscape": {"drift": "indeterminate", "untested": ["runtime-a", "runtime-b"]}},
        "card": None,
        "guarantees": ["untested blocks"],
    }


class ProviderReportTests(unittest.TestCase):
    def test_untested_required_capability_blocks_and_differential_is_indeterminate(self) -> None:
        report = provider_capability_gate_report(provider_payload())
        self.assertIsInstance(report, ProviderCapabilityGateReport)
        self.assertTrue(report.blocked)
        self.assertTrue(report.has_untested_required)
        self.assertEqual(report.differential["HostEscape"]["drift"], "indeterminate")

    def test_passed_required_capability_clears_without_promoting_measurements(self) -> None:
        report = provider_capability_gate_report(provider_payload(cleared=True))
        self.assertTrue(report.cleared)
        self.assertEqual(report.claims, ("host_escape",))
        self.assertEqual(report.measurement_count, 1)

    def test_provider_request_rejects_performance_as_pass_fail_requirement(self) -> None:
        with self.assertRaises(ArgumentError):
            ProviderCapabilityGateArgs({"provider": "runtime-a", "states": {}, "measurements": []}, ("cold_startup",))


if __name__ == "__main__":
    unittest.main()
