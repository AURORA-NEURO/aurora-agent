from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    TraceOtelIngestArgs,
    TraceOtelIngestReport,
    Workspace,
    trace_otel_ingest,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> TraceOtelIngestArgs:
    return TraceOtelIngestArgs("otel-run", otlp_json='{"resourceSpans":[]}', include_events=True, max_items=5)


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/trace-otel-ingest/0.1",
        "trace_id": "otel-run",
        "event_count": 2,
        "succeeded": False,
        "trace_sha256": "a" * 64,
        "valid": True,
        "validation_error": None,
        "mapping": {"format": "otlp_json", "resource_count": 1, "scope_count": 1, "source_span_count": 2, "accepted_span_count": 2, "span_event_count": 1},
        "loss": {
            "dropped_spans": [],
            "dropped_span_events": [],
            "unmapped_fields": [],
            "duplicate_attributes": [],
            "inferred_kinds": [],
            "missing_start_times": [],
            "unresolved_parents": [],
            "multiple_trace_ids": [],
        },
        "lossless": True,
        "dropped_events": 0,
        "compilable": True,
        "events_included": True,
        "events": [
            {"step": 0, "kind": "goal", "payload": {"name": "agent.goal", "events": []}, "visible": ["service.name"]},
            {"step": 1, "kind": "action", "payload": {"name": "agent.tool.call", "events": [{"name": "tool.input"}]}, "caused_by": 0, "visible": ["service.name", "prism.event.kind"]},
        ],
        "omitted_events": 0,
        "guarantees": [
            "source spans are retained inside normalized Event payloads, while unsupported fields remain explicit in the loss report",
            "the caller supplies trajectory success; the adapter never infers benchmark outcome from span status",
        ],
        "limitations": [
            "this is a deterministic OTLP JSON importer, not an OTLP exporter, collector client, network publisher, or clock reader",
            "vendor-specific conventions are preserved as source data but are not interpreted unless they use prism.event.kind or aurora.event.kind",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class TraceOtelProjectionTests(unittest.TestCase):
    def test_request_preserves_exclusive_source_and_bounds(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["trace_id"], "otel-run")
        self.assertTrue(args.to_mcp_arguments()["include_events"])
        with self.assertRaises(ArgumentError):
            TraceOtelIngestArgs("trace", otlp_json="{}", document="fixtures/trace.json")
        with self.assertRaises(ArgumentError):
            TraceOtelIngestArgs("trace", otlp_json="{}", max_spans=0)

    def test_report_preserves_mapping_loss_event_ir_and_readiness(self) -> None:
        report = trace_otel_ingest(payload())
        self.assertIsInstance(report, TraceOtelIngestReport)
        self.assertTrue(report.valid)
        self.assertTrue(report.compilable)
        self.assertEqual(report.mapping.accepted_span_count, 2)
        self.assertTrue(report.loss.lossless)
        self.assertEqual(report.events[1].caused_by, 0)
        self.assertEqual(report.events[1].kind, "action")
        self.assertTrue(report.semantic_loss_is_explicit)
        self.assertTrue(report.network_export_is_not_claimed)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(trace_otel_ingest(envelope).trace_id, "otel-run")

    def test_all_python_facades_return_typed_otlp_reports(self) -> None:
        args = request()
        self.assertTrue(Workspace(_SyncTool()).trace_otel_ingest_report(args).ready_for_compilation)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).trace_otel_ingest_report(args)).ready_for_compilation)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").trace_otel_ingest_report(args)
        self.assertEqual(report.mapping.format, "otlp_json")
        call.assert_called_once_with("trace_otel_ingest", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).trace_otel_ingest_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/trace-otel-ingest/0.1")
            async_call.assert_called_once_with("trace_otel_ingest", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
