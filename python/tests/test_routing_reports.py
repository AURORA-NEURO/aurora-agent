from __future__ import annotations

import unittest

from prism_sdk import ArgumentError, RoutingDecisionReport, routing_decision_report


def routing_payload(*, abstained: bool = True) -> dict:
    reason = (
        {"reason": "insufficient_coverage", "eligible_architectures": 0, "neighbouring_observations": 0}
        if abstained
        else {"reason": "routed", "margin": 0.25, "supporting_tasks": 3, "runner_up": {"kind": "fiber_compiled"}}
    )
    return {
        "ok": True,
        "decision": {
            "architecture": {"kind": "full_context"},
            "confidence": 0.0 if abstained else 0.8,
            "abstained": abstained,
            "reason": reason,
            "considered": [
                {"architecture": {"kind": "full_context"}, "observations": 0, "distinct_tasks": 0, "mean_utility": 0.0, "admissible_rate": 0.0}
            ],
        },
        "task_id": None,
        "holdout_check": "caller_must_supply_unseen_identity",
        "evidence": {"observations": 0, "distinct_tasks": 0, "neighbourhood_observations": 0, "neighbourhood_radius": 3},
        "guarantees": ["safe default remains explicit"],
    }


class RoutingReportTests(unittest.TestCase):
    def test_abstention_preserves_reason_and_safe_default_posture(self) -> None:
        report = routing_decision_report(routing_payload())
        self.assertIsInstance(report, RoutingDecisionReport)
        self.assertTrue(report.abstained)
        self.assertTrue(report.safe_default)
        self.assertEqual(report.reason["reason"], "insufficient_coverage")
        self.assertEqual(report.holdout_check, "caller_must_supply_unseen_identity")

    def test_routed_decision_keeps_margin_and_approved_architecture(self) -> None:
        report = routing_decision_report(routing_payload(abstained=False))
        self.assertTrue(report.routed)
        self.assertEqual(report.architecture["kind"], "full_context")
        self.assertEqual(report.reason["margin"], 0.25)

    def test_inconsistent_abstention_and_reason_is_rejected(self) -> None:
        payload = routing_payload()
        payload["decision"]["abstained"] = False
        with self.assertRaises(ArgumentError):
            routing_decision_report(payload)


if __name__ == "__main__":
    unittest.main()
