from __future__ import annotations

import asyncio
import json
import unittest

from prism_sdk import (
    AsyncWorkspace,
    FiberDecisionQuotientSummary,
    Workspace,
    fiber_decision_quotient_summary,
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
    return {"layer": "l0", "decision_quotient": summary()}


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

    def test_legacy_or_malformed_projection_fails_closed(self) -> None:
        with self.assertRaises(ArgumentError):
            fiber_decision_quotient_summary({"layer": "l0"})
        broken = compile_payload()
        broken["decision_quotient"]["merged_model_count"] = 0
        with self.assertRaises(ArgumentError):
            fiber_decision_quotient_summary(broken)
