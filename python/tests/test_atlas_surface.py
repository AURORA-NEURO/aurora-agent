from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    AtlasSurfaceAuditArgs,
    AtlasSurfaceAuditReport,
    Workspace,
    atlas_surface_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> AtlasSurfaceAuditArgs:
    return AtlasSurfaceAuditArgs(
        {
            "label": "surface-system",
            "conditions": {},
            "cells": {
                "identity.lineage": {
                    "state": "measured",
                    "estimate": {"uncertainty": "point", "estimate": {"value": 0.8, "no_interval": "estimator_not_available"}},
                    "effective_size": 4,
                }
            },
        },
        later_grid={"label": "surface-system", "conditions": {}, "cells": {}},
        failures=[{"failure_id": "f-1"}],
        failure_subject="surface-system",
        facet="mechanism",
        visibility=[{"failure_id": "f-1", "state": "under-review"}],
        rate_capabilities=["identity.lineage"],
        require_sound_surfaces=True,
        max_items=10,
    )


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/atlas-surface-audit/0.1",
        "workflow": "atlas_surface_audit",
        "coverage": {
            "subject": "surface-system",
            "total_capabilities": 3,
            "measured": 1,
            "unmeasured": 2,
            "blocking": 2,
            "closed_by_declaration": 0,
            "vacuous": False,
            "holes": [{"capability": "causal.interpretation", "reason": "not_attempted", "blocks_claim": True}],
            "omitted_holes": 1,
            "profile_coverage": {"outcome": "answered", "cell": {"kind": "share", "value": {"numerator": 1, "denominator": 3}}},
        },
        "debt_discharge": {
            "subject": "surface-system",
            "measured": {"rows": ["cohort.statistics"], "total": 1, "omitted": 0},
            "declared_away": {"rows": ["causal.interpretation"], "total": 1, "omitted": 0},
            "persisting": {"rows": [], "total": 0, "omitted": 0},
            "newly_unmeasured": {"rows": [], "total": 0, "omitted": 0},
            "any_evidence": True,
        },
        "failure_browse": {
            "subject": "surface-system",
            "facet": "mechanism",
            "taxonomy_version": "atlasx-test/1",
            "records_browsed": 2,
            "visible": 1,
            "withheld": 1,
            "contested": 0,
            "undiagnosed": 0,
            "evaluator_induced": 0,
            "distinct_families": 1,
            "shares_sum_to_one": True,
            "buckets": [{"label": "mechanism:stale_evidence_trusted", "member_count": 1}],
            "omitted_buckets": 0,
        },
        "rate_checks": {"rows": [{"capability": "identity.lineage", "answered": True}], "total": 1},
        "surface_audits": {"sound": True, "debt": {}, "browse": {}},
        "policies": {"require_sound_surfaces": True},
        "guarantees": ["holes are not zeroes"],
        "limitations": ["caller-supplied records"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class AtlasSurfaceTests(unittest.TestCase):
    def test_args_preserve_denominators_visibility_and_policies(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["facet"], "mechanism")
        self.assertEqual(args.to_mcp_arguments()["visibility"][0]["state"], "under-review")
        self.assertTrue(args.to_mcp_arguments()["require_sound_surfaces"])
        with self.assertRaises(ArgumentError):
            AtlasSurfaceAuditArgs({}, facet="invented")
        with self.assertRaises(ArgumentError):
            AtlasSurfaceAuditArgs({}, max_items=0)

    def test_report_keeps_discharge_withheld_records_and_surface_soundness(self) -> None:
        report = atlas_surface_audit_report(payload())
        self.assertIsInstance(report, AtlasSurfaceAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.measured, 1)
        self.assertEqual(report.unmeasured, 2)
        self.assertEqual(report.withheld, 1)
        self.assertTrue(report.has_evidence_discharge)
        self.assertTrue(report.surface_sound)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(atlas_surface_audit_report(envelope).failure_browse.records_browsed, 2)

    def test_fail_closed_refusal_is_typed(self) -> None:
        report = atlas_surface_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/atlas-surface-audit/0.1",
            "workflow": "atlas_surface_audit",
            "stage": "coverage_policy",
            "refusal": "holes remain",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "coverage_policy")

    def test_all_facades_keep_surface_audit_typed(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).atlas_surface_audit_report(args).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).atlas_surface_audit_report(args)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").atlas_surface_audit_report(args)
        self.assertEqual(report.withheld, 1)
        call.assert_called_once_with("atlas_surface_audit", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).atlas_surface_audit_report(args)
            self.assertTrue(result.surface_sound)
            async_call.assert_called_once_with("atlas_surface_audit", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
