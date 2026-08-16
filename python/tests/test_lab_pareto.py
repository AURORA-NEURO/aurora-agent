from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LabParetoAuditArgs,
    LabParetoAuditReport,
    Workspace,
    lab_pareto_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def pareto_request() -> dict:
    measured = lambda rate, cost: {
        "candidate": f"candidate-{rate}-{cost}",
        "values": {
            "admissible_rate": {"state": "measured", "value": rate},
            "cost_units": {"state": "measured", "value": cost},
        },
    }
    return {
        "objectives": [
            {"axis": "admissible_rate", "direction": "higher_is_better"},
            {"axis": "cost_units", "direction": "lower_is_better"},
        ],
        "profiles": [measured(0.8, 10.0), measured(0.95, 40.0)],
        "relations": [{"left": "candidate-0.8-10.0", "right": "candidate-0.95-40.0"}],
        "max_rows": 1,
    }


def pareto_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/lab-pareto-audit/0.1",
        "objective_count": 2,
        "profile_count": 2,
        "objectives": pareto_request()["objectives"],
        "admissions": [{"input_index": 0, "candidate": "candidate-0.8-10.0", "admission": {"admission": "admitted", "displaced": []}}],
        "admissions_omitted": 1,
        "front": {
            "count": 2,
            "members": pareto_request()["profiles"],
            "unresolved_count": 0,
            "unresolved": [],
            "selection": {"selection": "ambiguous", "front": ["candidate-0.8-10.0", "candidate-0.95-40.0"], "unresolved": []},
        },
        "archived_count": 0,
        "archived": [],
        "archived_omitted": 0,
        "relations": [{"left": "candidate-0.8-10.0", "right": "candidate-0.95-40.0", "relation": {"relation": "incomparable"}}],
        "relations_omitted": 0,
        "max_rows": 1,
        "guarantees": ["trade-offs remain incomparable"],
        "limitations": ["point measurements"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(pareto_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(pareto_payload())}]})


class LabParetoTests(unittest.TestCase):
    def test_args_validate_axes_profiles_and_bounds(self) -> None:
        request = LabParetoAuditArgs.from_wire(pareto_request())
        self.assertEqual(request.to_mcp_arguments()["max_rows"], 1)
        with self.assertRaises(ArgumentError):
            LabParetoAuditArgs.from_wire({**pareto_request(), "objectives": [{"axis": "cost", "direction": "unknown"}]})
        with self.assertRaises(ArgumentError):
            LabParetoAuditArgs.from_wire({**pareto_request(), "profiles": []})

    def test_report_preserves_front_archive_and_ambiguous_selection(self) -> None:
        report = lab_pareto_audit_report(pareto_payload())
        self.assertIsInstance(report, LabParetoAuditReport)
        self.assertTrue(report.accepted)
        self.assertTrue(report.ambiguous)
        self.assertEqual(report.profile_count, 2)
        self.assertEqual(report.admissions_omitted, 1)
        self.assertEqual(report.front_members[0]["candidate"], "candidate-0.8-10.0")

    def test_fail_closed_refusal_and_all_facades(self) -> None:
        refusal = {
            "ok": False,
            "schema": "bioprism-mcp/lab-pareto-audit/0.1",
            "stage": "profile_insertion",
            "refusal": "profile says nothing about objective",
            "fail_closed": True,
            "guarantees": ["partial front is not reported"],
        }
        report = lab_pareto_audit_report({"mcp": {"result": {"structuredContent": refusal}}})
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = LabParetoAuditArgs.from_wire(pareto_request())
        self.assertTrue(Workspace(_SyncTool()).lab_pareto_audit_report(request).ambiguous)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).lab_pareto_audit_report(request)).ambiguous)
        with patch.object(ApiClient, "call_tool", return_value=pareto_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").lab_pareto_audit_report(request)
        self.assertEqual(result.front_selection, "ambiguous")
        call.assert_called_once_with("lab_pareto_audit", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=pareto_payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).lab_pareto_audit_report(request)
            self.assertEqual(result.archived_count, 0)
            async_call.assert_called_once_with("lab_pareto_audit", request.to_mcp_arguments())

        asyncio.run(run())
