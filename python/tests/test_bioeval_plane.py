from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalPlaneAuditArgs,
    BioevalPlaneAuditReport,
    Workspace,
    bioeval_plane_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "plane": {
            "system": "fixed-model",
            "tier": "fixed_input_model",
            "dimensions": [
                {"id": "accuracy", "required": "fixed_input_model", "weight": 2.0},
                {"id": "assay-selection", "required": "tool_using_agent", "weight": 1.0},
                {"id": "calibration", "required": "fixed_input_model", "weight": 1.0},
            ],
            "cells": {
                "accuracy": {"state": "scored", "score": 0.8},
                "assay-selection": {"state": "inapplicable", "required": "tool_using_agent", "declared": "fixed_input_model"},
                "calibration": {"state": "unscored", "reason": "no_reference_standard", "note": "reference panel pending"},
            },
        },
        "max_items": 2,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-plane-audit/0.1",
        "workflow": "bioeval_plane_audit",
        "plane": {"system": "pipeline", "tier": "workflow_pipeline", "dimension_count": 2, "scored_count": 2, "unscored_count": 0, "inapplicable_count": 0},
        "dimensions": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "findings": {"unscored_dimensions": {"ids": [], "total": 0, "omitted": 0}, "fold_blocked": False},
        "fold": {"folded": True, "value": 0.8, "included": ["accuracy", "workflow"], "excluded": [], "refusal": None},
        "guarantees": ["missing measurements remain distinct"],
        "limitations": ["no system ranking"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalPlaneTests(unittest.TestCase):
    def test_plane_args_preserve_cell_states_and_validate_tier_basis(self) -> None:
        parsed = BioevalPlaneAuditArgs.from_wire(request())
        self.assertEqual(parsed.plane.system, "fixed-model")  # type: ignore[union-attr]
        self.assertEqual(parsed.plane.cells["accuracy"].state, "scored")  # type: ignore[union-attr]
        self.assertEqual(parsed.plane.cells["calibration"].reason, "no_reference_standard")  # type: ignore[union-attr]
        self.assertEqual(parsed.plane.to_wire()["cells"]["assay-selection"]["declared"], "fixed_input_model")  # type: ignore[union-attr]
        with self.assertRaises(ArgumentError):
            BioevalPlaneAuditArgs.from_wire({**request(), "plane": {**request()["plane"], "cells": {**request()["plane"]["cells"], "accuracy": {"state": "scored", "score": 1.5}}}})
        with self.assertRaises(ArgumentError):
            BioevalPlaneAuditArgs.from_wire({**request(), "plane": {**request()["plane"], "tier": "workflow_pipeline"}})

    def test_report_exposes_fold_value_and_unscored_dimensions(self) -> None:
        report = bioeval_plane_audit_report(payload())
        self.assertIsInstance(report, BioevalPlaneAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.folded)
        self.assertAlmostEqual(report.fold_value or 0.0, 0.8)
        self.assertEqual(report.unscored_dimensions, ())

    def test_fail_closed_fold_refusal_is_typed(self) -> None:
        report = bioeval_plane_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-plane-audit/0.1",
            "workflow": "bioeval_plane_audit",
            "stage": "fold_policy",
            "refusal": "unscored dimensions remain",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "fold_policy")

    def test_all_facades_keep_plane_audit_typed(self) -> None:
        parsed = BioevalPlaneAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_plane_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_plane_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_plane_audit_report(parsed)
        self.assertTrue(report.folded)
        call.assert_called_once_with("bioeval_plane_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_plane_audit_report(parsed)
            self.assertAlmostEqual(report.fold_value or 0.0, 0.8)
            async_call.assert_called_once_with("bioeval_plane_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
