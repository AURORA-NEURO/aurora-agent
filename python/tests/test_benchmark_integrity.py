from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    BenchmarkIntegrityAuditArgs,
    BenchmarkIntegrityAuditReport,
    Workspace,
    benchmark_integrity_audit_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def integrity_payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/benchmark-integrity-audit/0.1",
        "instance_digest": "a" * 64,
        "counts": {"instances": 3, "panel_runs": 2, "bench_instances": 3, "known_instances": 3, "safety_vetoes": 1},
        "dedup": {"examined": 3, "distinct": 2, "groups": [{"layer": "content", "fingerprint": "f", "representative": "a", "duplicates": ["b"]}], "groups_omitted": 0, "removed": ["b"], "removed_omitted": 0, "caveat": "no semantic similarity"},
        "holdout": {"private_share": 20, "rotating_panels": 0, "counts": {"private": 1, "public": 2}, "rows": [], "omitted": 3},
        "contamination": {"counts": {"clean": 1, "unassessed": 1, "leaks_through_channel": 1}, "admissible": 1, "inadmissible": 2, "rows": [], "omitted": 3},
        "calibration": {"discriminating": 1, "trivial_cue": 0, "universally_passed": 0, "universally_failed": 0, "unmeasured": 2, "safety_vetoes": 0, "instances": [], "omitted": 3},
        "effective_diversity": {"instances": 3, "parents": 2, "families": 2, "signatures": 2, "equivalence_classes": 2, "inflation_ratio": 1.5, "caveat": "independent classes"},
        "guarantees": ["unmeasured is not zero"],
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


class BenchmarkIntegrityTests(unittest.TestCase):
    def test_args_preserve_maps_and_reject_invalid_bounds(self) -> None:
        request = BenchmarkIntegrityAuditArgs.from_wire({
            "instances": [{"instance_id": "a", "content": {}, "acceptable_verdicts": [], "required_witnesses": []}],
            "exposure": {"a": {"assessed": True}},
            "probes": {"a": [{"channel": "metadata_only", "solved": False, "note": "checked"}]},
            "private_share": 40,
            "rotating_panels": 2,
            "max_items": 7,
        })
        wire = request.to_mcp_arguments()
        self.assertEqual(wire["exposure"]["a"]["assessed"], True)
        self.assertEqual(wire["probes"]["a"][0]["channel"], "metadata_only")
        with self.assertRaises(ArgumentError):
            BenchmarkIntegrityAuditArgs.from_wire({"instances": [], "private_share": 101})

    def test_report_keeps_denominators_and_effective_sample_size_separate(self) -> None:
        report = benchmark_integrity_audit_report(integrity_payload())
        self.assertIsInstance(report, BenchmarkIntegrityAuditReport)
        self.assertTrue(report.accepted)
        self.assertEqual(report.admissible_instances, 1)
        self.assertEqual(report.effective_sample_size, 2)
        self.assertEqual(report.counts["instances"], 3)

    def test_refusal_and_all_facades_preserve_fail_closed_state(self) -> None:
        refusal = {"ok": False, "stage": "input", "refusal": "duplicate instance_id", "fail_closed": True, "guarantees": ["no silent merge"]}
        report = benchmark_integrity_audit_report(refusal)
        self.assertTrue(report.refused)
        self.assertTrue(report.fail_closed)
        request = BenchmarkIntegrityAuditArgs.from_wire({"instances": [{"instance_id": "a", "content": {}, "acceptable_verdicts": [], "required_witnesses": []}]})
        self.assertEqual(Workspace(_SyncTool(integrity_payload())).benchmark_integrity_audit_report(request).effective_sample_size, 2)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(integrity_payload())).benchmark_integrity_audit_report(request)).admissible_instances, 1)
        with patch.object(ApiClient, "call_tool", return_value=integrity_payload()) as call:
            result = ApiClient("http://127.0.0.1:1").benchmark_integrity_audit_report(request)
        self.assertEqual(result.instance_digest, "a" * 64)
        call.assert_called_once_with("benchmark_integrity_audit", request.to_mcp_arguments())

