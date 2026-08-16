from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    AtlasReport,
    AtlasReportArgs,
    Workspace,
    atlas_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> AtlasReportArgs:
    return AtlasReportArgs({"ontology": {}, "cells": {}, "failures": []}, {"intended_use": "release", "weights": {"measured": 1.0}}, 10)


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/atlas-report/0.1",
        "ontology_version": "atlas-test/1",
        "summary": {"measured": 1, "holes": 2, "families": 3, "inconsistencies": 0, "coverage_debt_ratio": 0.667, "has_holes": True, "coverage_supports_aggregation": False},
        "debt": {"total_capabilities": 3, "measured": 1, "unmeasured": 2, "closed_by_declaration": 0, "dark_families": ["tool_use"], "unclassified_failures": 0, "undiagnosed_failures": 0},
        "measured": [{"capability": "measured", "family": "verification", "score": 1.0, "depth": "single", "evaluable": 1, "excluded": 0, "effective_size": 1, "generated_instances": 0, "permitted_claim": "unit_conformance"}],
        "omitted_measured": 0,
        "holes": [
            {"capability": "unmeasured", "family": "tool_use", "reason": "not_attempted", "influence": "unknown", "aggregate": False, "blocks_claims_for": ["agent"]},
            {"capability": "agent", "family": "domain_reasoning", "reason": "not_attempted", "influence": "unknown", "aggregate": True, "blocks_claims_for": []},
        ],
        "omitted_holes": 0,
        "family_coverage": [
            {"family": "domain_reasoning", "total": 1, "measured": 0, "holes": 1},
            {"family": "tool_use", "total": 1, "measured": 0, "holes": 1},
            {"family": "verification", "total": 1, "measured": 1, "holes": 0},
        ],
        "omitted_families": 0,
        "depth_histogram": [{"depth": "single", "count": 1}],
        "stage_histogram": [],
        "inconsistencies": [],
        "omitted_inconsistencies": 0,
        "composite": {"ok": False, "refusal": "unmeasured capability", "fail_closed": True},
        "guarantees": ["unmeasured capabilities remain holes and are never rendered as zero"],
        "limitations": ["the atlas indexes caller-supplied evidence; it does not run trials"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class AtlasReportTests(unittest.TestCase):
    def test_request_preserves_atlas_weighting_and_bound(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["max_items"], 10)
        self.assertEqual(args.to_mcp_arguments()["weighting"]["intended_use"], "release")
        with self.assertRaises(ArgumentError):
            AtlasReportArgs({}, max_items=0)

    def test_report_keeps_holes_debt_and_composite_refusal_distinct(self) -> None:
        report = atlas_report(payload())
        self.assertIsInstance(report, AtlasReport)
        self.assertTrue(report.has_holes)
        self.assertFalse(report.coverage_supports_aggregation)
        self.assertEqual(report.debt.dark_families, ("tool_use",))
        self.assertEqual(report.measured[0].effective_size, 1)
        self.assertEqual(report.holes[0].blocks_claims_for, ("agent",))
        self.assertEqual(report.composite.state, "refused")
        self.assertFalse(report.composite_is_eligible)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(atlas_report(envelope).ontology_version, "atlas-test/1")

    def test_all_python_facades_return_typed_atlas_reports(self) -> None:
        args = request()
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).atlas_report_typed(args)).has_holes)
        self.assertTrue(Workspace(_SyncTool()).atlas_report_typed(args).all_holes_visible)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").atlas_report_typed(args)
        self.assertEqual(report.summary.measured, 1)
        call.assert_called_once_with("atlas_report", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).atlas_report_typed(args)
            self.assertEqual(result.schema, "bioprism-mcp/atlas-report/0.1")
            async_call.assert_called_once_with("atlas_report", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
