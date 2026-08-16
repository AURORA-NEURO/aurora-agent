from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalWaiverAuditArgs,
    BioevalWaiverAuditReport,
    Workspace,
    bioeval_waiver_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "version": "release-2026.08",
        "at": "2026-08-16T12:00:00Z",
        "gates": [
            {"id": "health", "kind": "benchmark_health", "verdict": {"verdict": "violated", "detail": "calibration below floor"}},
            {"id": "unknown-rate", "kind": "maximum_unknown_rate", "verdict": {"verdict": "unevaluable", "missing": "reference panel"}},
            {"id": "safety", "kind": "safety_veto", "verdict": {"verdict": "violated", "detail": "forbidden action"}},
        ],
        "waivers": [{
            "gate": "health",
            "authoriser": "release-board",
            "rationale": "ship the documented calibration exception",
            "expiry": "2026-09-01T00:00:00Z",
            "affected_versions": ["release-2026.08"],
            "follow_up": "recalibrate before next release",
        }],
        "require_releasable": False,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-waiver-audit/0.1",
        "workflow": "bioeval_waiver_audit",
        "release": {"version": "release-2026.08", "blocking_before": 3, "blocking_after": 2, "waived_count": 1, "unevaluable_count": 1, "releasable": False},
        "gates": {"rows": [], "returned": 0, "total": 3, "omitted": 3},
        "waivers": {"rows": [], "returned": 0, "total": 1, "omitted": 1},
        "findings": {
            "still_blocking": {"ids": ["safety", "unknown-rate"], "total": 2, "omitted": 0},
            "waived_gates": {"ids": ["health"], "total": 1, "omitted": 0},
            "unevaluable_gates": {"ids": ["unknown-rate"], "total": 1, "omitted": 0},
            "safety_vetoes": {"ids": ["safety"], "total": 1, "omitted": 0},
        },
        "guarantees": ["underlying verdict remains visible"],
        "limitations": ["no identity provider"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalWaiverTests(unittest.TestCase):
    def test_args_preserve_tagged_verdicts_and_required_waiver_fields(self) -> None:
        parsed = BioevalWaiverAuditArgs.from_wire(request())
        self.assertEqual(parsed.gates[0].verdict.verdict, "violated")  # type: ignore[union-attr]
        self.assertEqual(parsed.gates[1].verdict.missing, "reference panel")  # type: ignore[union-attr]
        self.assertEqual(parsed.to_mcp_arguments()["waivers"][0]["affected_versions"], ["release-2026.08"])
        with self.assertRaises(ArgumentError):
            BioevalWaiverAuditArgs.from_wire({**request(), "waivers": [{**request()["waivers"][0], "follow_up": ""}]})
        with self.assertRaises(ArgumentError):
            BioevalWaiverAuditArgs.from_wire({**request(), "gates": request()["gates"] + [request()["gates"][0]]})

    def test_report_preserves_blockers_waivers_and_unevaluable_findings(self) -> None:
        report = bioeval_waiver_audit_report(payload())
        self.assertIsInstance(report, BioevalWaiverAuditReport)
        self.assertTrue(report.accepted)
        self.assertFalse(report.releasable)
        self.assertEqual(report.still_blocking, ("safety", "unknown-rate"))
        self.assertEqual(report.waived_gates, ("health",))
        self.assertEqual(report.unevaluable_gates, ("unknown-rate",))

    def test_fail_closed_policy_refusal_is_typed(self) -> None:
        report = bioeval_waiver_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-waiver-audit/0.1",
            "workflow": "bioeval_waiver_audit",
            "stage": "release_gate_policy",
            "refusal": "blocking gate remains",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "release_gate_policy")

    def test_all_facades_keep_waiver_audit_typed(self) -> None:
        parsed = BioevalWaiverAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_waiver_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_waiver_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_waiver_audit_report(parsed)
        self.assertEqual(report.waived_gates, ("health",))
        call.assert_called_once_with("bioeval_waiver_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_waiver_audit_report(parsed)
            self.assertEqual(report.safety_vetoes, ("safety",))
            async_call.assert_called_once_with("bioeval_waiver_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
