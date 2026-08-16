from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BioevalMetamorphicAuditArgs,
    BioevalMetamorphicAuditReport,
    Workspace,
    bioeval_metamorphic_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> dict:
    return {
        "families": [
            {
                "id": "formatting",
                "relation": "invariant",
                "trials": [
                    {"id": "same", "relation": "invariant", "response": {"response": "unchanged"}},
                    {"id": "shortcut", "relation": "invariant", "response": {"response": "moved", "direction": "increase"}},
                    {"id": "unknown", "relation": "invariant", "response": {"response": "incomparable"}},
                ],
            },
            {
                "id": "biology-change",
                "relation": {"directional_change": {"expected": "increase"}},
                "trials": [
                    {"id": "expected", "relation": {"directional_change": {"expected": "increase"}}, "response": {"response": "moved", "direction": "increase"}},
                    {"id": "blind-spot", "relation": {"directional_change": {"expected": "increase"}}, "response": {"response": "unchanged"}},
                ],
            },
        ],
        "max_items": 2,
        "require_both_relations": True,
    }


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/bioeval-metamorphic-audit/0.1",
        "workflow": "bioeval_metamorphic_audit",
        "suite": {"family_count": 2, "trial_count": 5, "relation_coverage": {"invariant": True, "directional_change": True, "complete": True}, "undetermined_trial_count": 1, "has_suite_wide_consistency": False},
        "families": {"rows": [], "returned": 0, "total": 2, "omitted": 2},
        "findings": {
            "false_sensitivity_trials": {"ids": ["shortcut"], "total": 1, "omitted": 0},
            "false_invariance_trials": {"ids": ["blind-spot"], "total": 1, "omitted": 0},
            "wrong_direction_trials": {"ids": [], "total": 0, "omitted": 0},
        },
        "guarantees": ["failure directions remain separate"],
        "limitations": ["no mutation execution"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class BioevalMetamorphicTests(unittest.TestCase):
    def test_args_keep_internal_response_tags_and_relation_matching(self) -> None:
        parsed = BioevalMetamorphicAuditArgs.from_wire(request())
        self.assertEqual(parsed.families[0].trials[1].response.response, "moved")
        self.assertEqual(parsed.families[1].relation.expected, "increase")  # type: ignore[union-attr]
        self.assertEqual(parsed.to_mcp_arguments()["families"][0]["trials"][0]["response"], {"response": "unchanged"})
        with self.assertRaises(ArgumentError):
            BioevalMetamorphicAuditArgs.from_wire({**request(), "families": [{**request()["families"][0], "trials": [{"id": "mismatch", "relation": {"directional_change": {"expected": "increase"}}, "response": {"response": "unchanged"}}]}]})

    def test_report_keeps_failure_directions_and_undetermined_count(self) -> None:
        report = bioeval_metamorphic_audit_report(payload())
        self.assertIsInstance(report, BioevalMetamorphicAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.relation_coverage_complete)
        self.assertEqual(report.false_sensitivity_trials, ("shortcut",))
        self.assertEqual(report.false_invariance_trials, ("blind-spot",))
        self.assertEqual(report.undetermined_trial_count, 1)

    def test_fail_closed_oracle_refusal_is_typed(self) -> None:
        report = bioeval_metamorphic_audit_report({
            "ok": False,
            "schema": "bioprism-mcp/bioeval-metamorphic-audit/0.1",
            "workflow": "bioeval_metamorphic_audit",
            "stage": "oracle_quality",
            "refusal": "incomparable response",
            "fail_closed": True,
            "guarantees": [],
            "limitations": [],
        })
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "oracle_quality")

    def test_all_facades_keep_metamorphic_audit_typed(self) -> None:
        parsed = BioevalMetamorphicAuditArgs.from_wire(request())
        self.assertTrue(Workspace(_SyncTool()).bioeval_metamorphic_audit_report(parsed).accepted)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).bioeval_metamorphic_audit_report(parsed)).accepted)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").bioeval_metamorphic_audit_report(parsed)
        self.assertTrue(report.relation_coverage_complete)
        call.assert_called_once_with("bioeval_metamorphic_audit", parsed.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).bioeval_metamorphic_audit_report(parsed)
            self.assertEqual(report.wrong_direction_trials, ())
            async_call.assert_called_once_with("bioeval_metamorphic_audit", parsed.to_mcp_arguments())

        asyncio.run(run())
