from __future__ import annotations

import asyncio
import json
import unittest

from prism_sdk import (
    AsyncWorkspace,
    FiberDecisionQuotientSummary,
    FiberRateDistortionSummary,
    Workspace,
    fiber_decision_quotient_summary,
    fiber_rate_distortion_summary,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def summary() -> dict:
    return {
        "schema": "bioprism-mcp/epistemic-decision-quotient/0.1",
        "basis": "permitted_loss_difference_profile",
        "permitted_actions": ["accept", "defer", "reject"],
        "original_model_count": 3,
        "quotient_model_count": 2,
        "merged_model_count": 1,
        "compressed": True,
        "compression_fraction": 2 / 3,
        "certificate_binding": {
            "query_sha256": "a" * 64,
            "certificate_sha256": "b" * 64,
        },
        "limitations": ["decision-relative only", "not rate-distortion"],
    }


def compile_payload() -> dict:
    return {"layer": "l0", "decision_quotient": summary(), "rate_distortion": rate_summary()}


def rate_summary() -> dict:
    return {
        "schema": "bioprism-mcp/epistemic-context-audit/0.2",
        "criterion": "bayes_regret",
        "tolerance": 0.25,
        "compatibility_floor": 0.05,
        "evidence_count": 2,
        "full_rate": 3.0,
        "identification": {"status": "point_identified", "action": 0},
        "sufficiency": {"outcome": "sufficient", "retained": [0], "rate": 2.0},
        "frontier": {"criterion": "bayes_regret", "evaluated": 4, "points": []},
        "certificate_binding": {
            "query_sha256": "a" * 64,
            "certificate_sha256": "b" * 64,
        },
        "guarantees": ["exhaustive frontier"],
        "limitations": ["caller-declared evidence"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(compile_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(compile_payload())}]})


class FiberContractTests(unittest.TestCase):
    def test_projection_validates_counts_digests_and_limitations(self) -> None:
        report = fiber_decision_quotient_summary(compile_payload())
        self.assertIsInstance(report, FiberDecisionQuotientSummary)
        self.assertEqual(report.permitted_actions, ("accept", "defer", "reject"))
        self.assertEqual(report.quotient_model_count, 2)
        self.assertTrue(report.compressed)
        self.assertEqual(report.query_sha256, "a" * 64)
        self.assertIn("not rate-distortion", report.limitations)

    def test_http_envelope_and_workspace_facades_preserve_the_same_projection(self) -> None:
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": compile_payload()}}}
        self.assertEqual(fiber_decision_quotient_summary(envelope).merged_model_count, 1)
        self.assertEqual(
            Workspace(_SyncTool()).fiber_compile_decision_quotient("world.json", "query.json").certificate_sha256,
            "b" * 64,
        )
        self.assertEqual(
            asyncio.run(
                AsyncWorkspace(_AsyncTool()).fiber_compile_decision_quotient("world.json", "query.json")
            ).compression_fraction,
            2 / 3,
        )

    def test_rate_distortion_projection_validates_frontier_and_binding(self) -> None:
        report = fiber_rate_distortion_summary(compile_payload())
        self.assertIsInstance(report, FiberRateDistortionSummary)
        self.assertEqual(report.criterion, "bayes_regret")
        self.assertEqual(report.evidence_count, 2)
        self.assertEqual(report.frontier["evaluated"], 4)
        self.assertEqual(report.certificate_sha256, "b" * 64)

    def test_rate_distortion_workspace_facades_preserve_the_same_projection(self) -> None:
        self.assertEqual(
            Workspace(_SyncTool()).fiber_compile_rate_distortion("world.json", "query.json").full_rate,
            3.0,
        )
        self.assertEqual(
            asyncio.run(
                AsyncWorkspace(_AsyncTool()).fiber_compile_rate_distortion("world.json", "query.json")
            ).tolerance,
            0.25,
        )

    def test_legacy_or_malformed_projection_fails_closed(self) -> None:
        with self.assertRaises(ArgumentError):
            fiber_decision_quotient_summary({"layer": "l0"})
        broken = compile_payload()
        broken["decision_quotient"]["merged_model_count"] = 0
        with self.assertRaises(ArgumentError):
            fiber_decision_quotient_summary(broken)
        broken_rate = compile_payload()
        broken_rate["rate_distortion"]["frontier"]["evaluated"] = 0
        with self.assertRaises(ArgumentError):
            fiber_rate_distortion_summary(broken_rate)
