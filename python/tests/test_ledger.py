from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    LedgerIngestArgs,
    LedgerIngestReport,
    LedgerTemporalCut,
    Workspace,
    ledger_ingest,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> LedgerIngestArgs:
    return LedgerIngestArgs(
        [{"class": "material", "kind": "specimen.collected", "payload": {"value": 1}}],
        LedgerTemporalCut(as_of_record="2024-06-01T00:00:00Z"),
        True,
        10,
    )


def payload() -> dict:
    events = [
        {"event_index": 0, "receipt": {"admission": {"admission": "quarantined", "key": "child-key", "missing": ["evt-parent"]}, "released": []}},
        {"event_index": 1, "receipt": {"admission": {"admission": "recorded", "id": "evt-parent", "seq": 0}, "released": ["evt-child"]}},
        {"event_index": 2, "receipt": {"admission": {"admission": "duplicate", "id": "evt-parent"}, "released": []}},
    ]
    return {
        "ok": True,
        "schema": "bioprism-mcp/ledger-ingest/0.1",
        "entries": 2,
        "next_seq": 2,
        "head": "entry-digest",
        "admissions": {"recorded": 2, "duplicates": 1, "quarantined": 1, "released": 1, "receipts": events},
        "chain": {"status": "intact"},
        "clock_anomalies": [{"seq": 1, "previous_record": "2025-01-01T00:00:00Z", "record": "2024-01-01T00:00:00Z"}],
        "quarantine": {"count": 0, "items": [], "omitted": 0},
        "class_counts": {"material": 2},
        "latest_by_subject": {"count": 1, "items": [{"subject": "patient-7/specimen-1", "event": "evt-parent", "seq": 1, "valid": "2025-01-01T00:00:00Z", "payload_digest": "payload-digest"}], "omitted": 0},
        "cut": {"requested": {"as_of_record": "2024-06-01T00:00:00Z"}, "count": 1, "entries": [{"seq": 0, "id": "evt-parent", "class": "material", "kind": "specimen.collected", "subject": "patient-7/specimen-1", "valid": "2025-01-01T00:00:00Z", "record": "2025-01-01T00:00:00Z", "release": "2025-01-01T00:00:00Z"}], "omitted": 0},
        "guarantees": [
            "valid, record, and release times remain separate and caller-supplied",
            "unknown causal parents quarantine instead of creating dangling history",
            "duplicate idempotent events converge while conflicting keys refuse",
            "hash-chain, clock-anomaly, quarantine, and projection states remain independently visible",
            "payload bodies are not returned by default; projections carry digests rather than copied payloads",
            "no durable storage, clock reading, network, or external side effect occurs",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class LedgerIngestTests(unittest.TestCase):
    def test_args_bound_events_cut_receipts_and_item_budget(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["cut"]["as_of_record"], "2024-06-01T00:00:00Z")
        self.assertTrue(args.to_mcp_arguments()["include_receipts"])
        with self.assertRaises(ArgumentError):
            LedgerIngestArgs([], max_items=1)
        with self.assertRaises(ArgumentError):
            LedgerIngestArgs([{}], max_items=0)

    def test_report_preserves_admission_variants_temporal_cut_and_digest_projection(self) -> None:
        report = ledger_ingest(payload())
        self.assertIsInstance(report, LedgerIngestReport)
        self.assertTrue(report.ok)
        self.assertTrue(report.chain_intact)
        self.assertEqual(report.entries, 2)
        self.assertEqual(report.admissions.duplicates, 1)
        self.assertEqual(report.admissions.receipts[0].admission.kind, "quarantined")
        self.assertEqual(report.admissions.receipts[1].released, ("evt-child",))
        self.assertEqual(report.clock_anomalies[0].seq, 1)
        self.assertEqual(report.cut.entries[0].event_id, "evt-parent")
        self.assertEqual(report.latest_by_subject.items[0].payload_digest, "payload-digest")
        self.assertTrue(report.projections_are_digest_only)
        self.assertTrue(report.durable_storage_is_not_claimed)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(ledger_ingest(envelope).head, "entry-digest")

    def test_append_refusal_keeps_pre_refusal_chain_state_and_all_python_facades(self) -> None:
        refused = {
            "ok": False,
            "schema": "bioprism-mcp/ledger-ingest/0.1",
            "stage": "append",
            "event_index": 1,
            "refusal": "conflicting idempotency key",
            "fail_closed": True,
            "ledger_before_refusal": {"recorded_entries": 1, "quarantined": 0, "next_seq": 1, "chain": {"status": "intact"}},
            "guarantee": "events after the first append refusal are not processed or silently discarded",
        }
        report = ledger_ingest(refused)
        self.assertFalse(report.ok)
        self.assertEqual(report.event_index, 1)
        self.assertTrue(report.ledger_before_refusal.chain.intact)
        args = request()
        self.assertTrue(Workspace(_SyncTool()).ledger_ingest_report(args).receipts_included)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).ledger_ingest_report(args)).causal_releases_are_visible)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            self.assertEqual(ApiClient("http://127.0.0.1:1").ledger_ingest_report(args).entries, 2)
        call.assert_called_once_with("ledger_ingest", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).ledger_ingest_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/ledger-ingest/0.1")
            async_call.assert_called_once_with("ledger_ingest", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
