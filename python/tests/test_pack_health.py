from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PackBuilder,
    PackHealthAssessArgs,
    PackHealthAssessmentReport,
    Workspace,
    pack_health_assessment_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def pack() -> dict:
    return (
        PackBuilder(
            pack_id="demo.pack",
            version=(1, 0, 0),
            schema_range=(1, 2),
            title="Decision evidence",
            measures="sufficiency of evidence selection",
            blueprint_module="15.01",
            axis="mechanism",
            capabilities=[{"agent": "evidence_acquisition"}],
            domains=["science"],
            owners=["aurora"],
            license="Apache-2.0",
        )
        .parent("world:demo", 3)
        .decision_family("smallest-sufficient-context")
        .mutation_relation("preserves_verdict")
        .oracle("deterministic")
        .authored_instances(8)
        .trial_counts(12, 2)
        .effective_sample(8)
        .build()
        .document
    )


def observations() -> dict:
    return {
        "calibration": {
            "observations": [
                {"system": "system-a", "trials": 100, "passes": 10},
                {"system": "system-b", "trials": 100, "passes": 50},
                {"system": "system-c", "trials": 100, "passes": 80},
            ]
        },
        "trivial_baselines": [],
        "contamination": [],
    }


def healthy_payload() -> dict:
    return {
        "ok": True,
        "pack": "demo.pack",
        "pack_digest": "a" * 64,
        "verdict": "healthy",
        "finding_count": 0,
        "blocking_findings": 0,
        "advisory_findings": 0,
        "health": {"pack": "demo.pack", "pack_digest": "a" * 64, "findings": []},
        "calibration": {
            "observations": [
                {"system": "system-a", "trials": 100, "passes": 10},
                {"system": "system-b", "trials": 100, "passes": 50},
                {"system": "system-c", "trials": 100, "passes": 80},
            ]
        },
        "score_gate": {
            "reportable": True,
            "score": {
                "pack": "demo.pack",
                "pack_digest": "a" * 64,
                "pooled_pass_rate": 0.4666666667,
                "discrimination": {"verdict": "discriminating", "lowest": 0.1, "highest": 0.8, "separated": False},
                "advisories": [],
            },
        },
        "guarantees": [
            "health is bound to the immutable pack digest that was assessed",
            "blocking findings prevent any numeric score from being returned",
            "advisories remain attached to a reportable score rather than being dropped",
            "declarations, observed outcomes, oracle posture, and reportability remain separate",
        ],
    }


def unreportable_payload() -> dict:
    payload = healthy_payload()
    payload.update(
        {
            "verdict": "unreportable",
            "finding_count": 1,
            "blocking_findings": 1,
            "health": {
                "pack": "demo.pack",
                "pack_digest": "a" * 64,
                "findings": [{"finding": "saturated", "pooled_pass_rate": 0.99, "systems": 3}],
            },
            "score_gate": {"reportable": False, "refusal": "pack is saturated", "fail_closed": True, "score": None},
        }
    )
    payload["advisory_findings"] = 0
    return payload


def refusal_payload() -> dict:
    return {
        "ok": False,
        "stage": "pack_validation",
        "refusal": "pack has no oracle",
        "fail_closed": True,
        "score": None,
        "guarantees": ["invalid packs cannot reach observed health"],
    }


class _SyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class _AsyncTool:
    def __init__(self, payload: dict) -> None:
        self.payload = payload

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.payload)}]})


class PackHealthProjectionTests(unittest.TestCase):
    def test_args_bound_input_and_wire_shape(self) -> None:
        request = PackHealthAssessArgs(pack(), observations(), {"degenerate_absolute": 0.5})
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["pack"]["manifest"]["id"], "demo.pack")
        self.assertEqual(wire["policy"]["degenerate_absolute"], 0.5)
        with self.assertRaises(ArgumentError):
            PackHealthAssessArgs(pack(), {"bad": object()})

    def test_reportable_score_keeps_digest_calibration_and_discrimination(self) -> None:
        report = pack_health_assessment_report(healthy_payload())
        self.assertIsInstance(report, PackHealthAssessmentReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.reportable)
        self.assertFalse(report.score_withheld)
        self.assertTrue(report.digest_bound)
        self.assertEqual(report.calibration.pooled_pass_rate, 140 / 300)
        self.assertFalse(report.score.discrimination.separated)
        self.assertTrue(report.declarations_and_observations_separate)

    def test_blocking_health_finding_withholds_score_but_remains_inspectable(self) -> None:
        report = PackHealthAssessmentReport.from_wire(unreportable_payload())
        self.assertFalse(report.reportable)
        self.assertTrue(report.score_withheld)
        self.assertEqual(report.verdict, "unreportable")
        self.assertEqual(report.blocking_findings[0].finding, "saturated")
        self.assertEqual(report.score_gate.refusal, "pack is saturated")

    def test_validation_refusal_is_structured_and_fail_closed(self) -> None:
        report = PackHealthAssessmentReport.from_wire(refusal_payload())
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        self.assertEqual(report.stage, "pack_validation")
        self.assertIsNone(report.score)

    def test_contamination_and_materialization_variants_are_typed(self) -> None:
        payload = unreportable_payload()
        payload["health"]["findings"] = [
            {"finding": "contaminated", "signal": {"signal": "memorization_gap", "public": {"system": "s", "trials": 100, "passes": 90}, "held_out": {"system": "s", "trials": 100, "passes": 50}}},
            {"finding": "counts_not_materialized", "declared": 1000, "validated": 2, "materialized_fraction": 0.002},
        ]
        payload["finding_count"] = 2
        payload["blocking_findings"] = 1
        payload["advisory_findings"] = 1
        report = PackHealthAssessmentReport.from_wire(payload)
        self.assertEqual(report.health.findings[0].signal.pass_rate_gap, 0.4)
        self.assertEqual(report.health.findings[1].materialized_fraction, 0.002)
        self.assertEqual(report.advisory_findings[0].finding, "counts_not_materialized")

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "pack_health_assess", "mcp": {"result": {"structuredContent": healthy_payload()}}}
        self.assertTrue(pack_health_assessment_report(envelope).reportable)
        request = PackHealthAssessArgs(pack(), observations())
        self.assertTrue(Workspace(_SyncTool(healthy_payload())).pack_health_assess_report(pack(), observations()).reportable)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool(healthy_payload())).pack_health_assess_report(pack(), observations())).reportable)
        with patch.object(ApiClient, "call_tool", return_value=healthy_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").pack_health_assess_report(request)
        self.assertTrue(report.digest_bound)
        call.assert_called_once_with("pack_health_assess", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=healthy_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).pack_health_assess_report(request)
            self.assertTrue(report.reportable)
            async_call.assert_called_once_with("pack_health_assess", request.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
