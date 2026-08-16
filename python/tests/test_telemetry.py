from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    TelemetryProjectRequest,
    TelemetryProjectionReport,
    Workspace,
    telemetry_project,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> TelemetryProjectRequest:
    return TelemetryProjectRequest(
        {"id": "evt-1", "kind": "job.completed", "fields": {}, "epoch": 7},
        {"version": "telemetry-v1", "treatments": {}},
        "trace-1",
    )


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/telemetry-projection/0.1",
        "event_id": "evt-1",
        "event_kind": "job.completed",
        "trace": "trace-1",
        "policy_version": "telemetry-v1",
        "record": {
            "event_id": "evt-1",
            "kind": "job.completed",
            "trace": "trace-1",
            "attributes": {"status": "ok", "specimen": "cohort"},
            "epoch": 7,
            "policy": "telemetry-v1",
        },
        "loss": {"dropped": ["subject"], "coarsened": ["specimen"]},
        "lossless": False,
        "metric": {
            "ok": True,
            "value": {"metric": "trace_coverage", "unit": "ratio", "value": 0.98, "supported_by": ["spans_emitted", "operations_total"]},
            "audit_statement": "trace_coverage = 0.98 ratio supported by spans_emitted, operations_total",
        },
        "guarantees": [
            "telemetry is a one-way projection of the canonical DomainEvent",
            "semantic loss is returned beside every projected record",
            "the call performs no OTLP export, backend write, sampling, span creation, clock read, storage, or network operation",
        ],
    }


class _SyncTool:
    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class _AsyncTool:
    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(payload())}]})


class TelemetryProjectionTests(unittest.TestCase):
    def test_request_requires_metric_and_observations_as_a_pair(self) -> None:
        with self.assertRaises(ArgumentError):
            TelemetryProjectRequest(request().event, request().policy, "trace-1", observations={"samples": {}})
        with self.assertRaises(ArgumentError):
            TelemetryProjectRequest(request().event, request().policy, "trace-1", metric={"name": "coverage"})

    def test_report_preserves_record_loss_and_observed_metric_support(self) -> None:
        report = telemetry_project(payload())
        self.assertIsInstance(report, TelemetryProjectionReport)
        self.assertTrue(report.ok)
        self.assertEqual(report.record.event_id, "evt-1")
        self.assertEqual(report.loss.dropped, ("subject",))
        self.assertEqual(report.loss.coarsened, ("specimen",))
        self.assertFalse(report.lossless)
        self.assertTrue(report.semantic_loss_is_explicit)
        self.assertTrue(report.metric_supported)
        self.assertEqual(report.metric.value.supported_by, ("spans_emitted", "operations_total"))
        self.assertTrue(report.projection_is_one_way)
        self.assertTrue(report.network_export_is_not_claimed)
        envelope = {"ok": True, "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(telemetry_project(envelope).trace, "trace-1")

    def test_metric_refusal_keeps_asserted_inputs_visible(self) -> None:
        refused_metric = {
            "ok": False,
            "refusal": "metric trace_coverage is unsupported because operations_total is not observed",
            "asserted_signals": ["operations_total"],
            "observed_sample_count": 2,
        }
        refused = dict(payload())
        refused["metric"] = refused_metric
        report = telemetry_project(refused)
        self.assertTrue(report.ok)
        self.assertTrue(report.metric_refused)
        self.assertEqual(report.asserted_signals, ("operations_total",))
        self.assertEqual(report.metric.observed_sample_count, 2)

    def test_fail_closed_projection_refusal_and_all_python_facades(self) -> None:
        refused = {
            "ok": False,
            "schema": "bioprism-mcp/telemetry-projection/0.1",
            "stage": "telemetry_projection",
            "refusal": "field count has no declared treatment",
            "fail_closed": True,
            "record": None,
            "loss": None,
            "guarantees": ["unclassified fields cannot be emitted"],
        }
        report = telemetry_project(refused)
        self.assertFalse(report.ok)
        self.assertTrue(report.fail_closed)
        self.assertIsNone(report.record)
        args = request()
        self.assertTrue(Workspace(_SyncTool()).telemetry_project_report(args).metric is not None)
        self.assertTrue(asyncio.run(AsyncWorkspace(_AsyncTool()).telemetry_project_report(args)).semantic_loss_is_explicit)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            self.assertTrue(ApiClient("http://127.0.0.1:1").telemetry_project_report(args).metric_supported)
        call.assert_called_once_with("telemetry_project", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                result = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).telemetry_project_report(args)
            self.assertEqual(result.schema, "bioprism-mcp/telemetry-projection/0.1")
            async_call.assert_called_once_with("telemetry_project", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
