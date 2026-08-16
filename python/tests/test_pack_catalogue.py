from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    PackCatalogueArgs,
    PackCatalogueReport,
    Workspace,
    pack_catalogue_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def catalogue_payload() -> dict:
    entry = {
        "id": "prism.context-acquisition",
        "title": "Context Acquisition and Evidence Value",
        "blueprint_module": "15.01",
        "axis": "mechanism",
        "measures": "smallest evidence needed",
        "capabilities": ["A00", "A03"],
        "domains": ["coding", "scientific reasoning"],
        "decision_families": ["choose evidence"],
        "oracles": ["deterministic", "executable", "policy_veto", "expert_review"],
        "strongest_oracle": "deterministic",
        "has_execution_grounded_oracle": True,
        "release_wave": {"wave": 1},
        "capability_signature": "Mechanism|A00,A03|coding,scientific reasoning",
    }
    return {
        "ok": True,
        "section": "15",
        "portfolio_count": 46,
        "section_counts": {"15": 25, "29": 21},
        "returned": [entry],
        "omitted": 24,
        "duplicate_signature_groups": [{"signature": "Domain|B5|biomedical research", "pack_ids": ["bio.statistical-estimands", "bio.causal-inference"]}],
        "guarantees": ["catalogue rows are typed portfolio declarations, not measured system scores"],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(catalogue_payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(catalogue_payload())}]})


class PackCatalogueProjectionTests(unittest.TestCase):
    def test_args_enforce_sections_and_item_bounds(self) -> None:
        request = PackCatalogueArgs.from_wire({"section": "29", "max_items": 3})
        self.assertEqual(request.to_mcp_arguments(), {"section": "29", "max_items": 3})
        with self.assertRaises(ArgumentError):
            PackCatalogueArgs("bad", 3)
        with self.assertRaises(ArgumentError):
            PackCatalogueArgs("all", 0)

    def test_report_preserves_declaration_axes_oracle_ceiling_and_duplicates(self) -> None:
        report = pack_catalogue_report(catalogue_payload())
        self.assertIsInstance(report, PackCatalogueReport)
        self.assertFalse(report.complete_for_request)
        self.assertTrue(report.declaration_only)
        self.assertEqual(report.returned[0].release_wave, 1)
        self.assertTrue(report.returned[0].is_sequenced)
        self.assertTrue(report.returned[0].has_execution_grounded_oracle)
        self.assertEqual(report.duplicate_review_count, 1)
        self.assertEqual(report.section_29_count, 21)

    def test_unsequenced_release_and_duplicate_groups_are_validated(self) -> None:
        payload = catalogue_payload()
        payload["returned"][0]["release_wave"] = "unsequenced"
        payload["returned"][0]["strongest_oracle"] = None
        report = PackCatalogueReport.from_wire(payload)
        self.assertIsNone(report.returned[0].release_wave)
        self.assertFalse(report.returned[0].is_sequenced)
        payload["duplicate_signature_groups"][0]["pack_ids"] = ["only-one"]
        with self.assertRaises(ArgumentError):
            PackCatalogueReport.from_wire(payload)

    def test_mcp_and_http_envelopes_parse(self) -> None:
        envelope = {"ok": True, "tool": "pack_catalogue", "mcp": {"result": {"structuredContent": catalogue_payload()}}}
        self.assertEqual(pack_catalogue_report(envelope).section, "15")

    def test_all_facades_delegate_the_typed_catalogue_request(self) -> None:
        request = PackCatalogueArgs("15", 1)
        self.assertEqual(Workspace(_SyncTool()).pack_catalogue_report(request).returned[0].id, "prism.context-acquisition")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool()).pack_catalogue_report(request)).section, "15")
        with patch.object(ApiClient, "call_tool", return_value=catalogue_payload()) as call:
            report = ApiClient("http://127.0.0.1:1").pack_catalogue_report(request)
        self.assertEqual(report.returned[0].blueprint_module, "15.01")
        call.assert_called_once_with("pack_catalogue", request.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=catalogue_payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).pack_catalogue_report(request)
            self.assertTrue(report.declaration_only)
            async_call.assert_called_once_with("pack_catalogue", request.to_mcp_arguments())

        asyncio.run(run())

    def test_count_reconciliation_rejects_overstated_returned_rows(self) -> None:
        payload = catalogue_payload()
        payload["omitted"] = 46
        with self.assertRaises(ArgumentError):
            PackCatalogueReport.from_wire(payload)
