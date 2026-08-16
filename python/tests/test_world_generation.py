from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    WorldGenerateArgs,
    WorldGenerateReport,
    Workspace,
    world_generate_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> WorldGenerateArgs:
    return WorldGenerateArgs({"world_id": "generated-discriminating-v1", "subjects": 4, "distractors": 2, "relay_depth": 3, "seed": 20260808}, include_world=True, include_query=True)


def payload() -> dict:
    return {
        "ok": True,
        "world_id": "generated-discriminating-v1",
        "query_id": "generated-discriminating-v1-query",
        "world_digest": "a" * 64,
        "query_digest": "b" * 64,
        "counts": {"facts": 14, "factors": 9, "events": 2, "subjects": 4, "distractors": 2, "relay_depth": 3, "generated_query_targets": 1},
        "validation": {"errors": 0, "warnings": 1, "diagnostics": [{"severity": "warning", "code": "backdated_event", "subject": "event-1", "message": "availability precedes event"}]},
        "world": {"schema_version": "fiber-world/0.1", "world_id": "generated-discriminating-v1", "facts": [], "factors": [], "events": []},
        "query": {"schema_version": "fiber-query/0.2", "query_id": "generated-discriminating-v1-query", "targets": ["decision"], "budgets": {"max_facts": 10}},
        "guarantees": [
            "generation is a pure deterministic function of the serialized WorldSpec and seed",
            "both generated documents are parsed by their typed runtime models before success",
            "world and query digests bind the exact generated JSON documents returned or withheld",
            "generation performs no file, network, model, clinical, or publication side effect",
        ],
    }


def refusal() -> dict:
    return {"ok": False, "stage": "generated_world_parse", "refusal": "generated world invalid", "fail_closed": True, "world": None, "query": None}


class _SyncTool:
    def __init__(self, value: dict) -> None:
        self.value = value

    def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.value)}]})


class _AsyncTool:
    def __init__(self, value: dict) -> None:
        self.value = value

    async def call_tool(self, name: str, arguments: dict) -> ToolResult:
        return ToolResult(name, {"isError": False, "content": [{"type": "text", "text": json.dumps(self.value)}]})


class WorldGenerationProjectionTests(unittest.TestCase):
    def test_args_preserve_spec_and_inclusion_flags_under_byte_bound(self) -> None:
        args = request()
        self.assertTrue(args.to_mcp_arguments()["include_world"])
        self.assertTrue(args.to_mcp_arguments()["include_query"])
        with self.assertRaises(ArgumentError):
            WorldGenerateArgs({"bad": object()})

    def test_success_report_keeps_digest_identity_counts_validation_and_documents(self) -> None:
        report = world_generate_report(payload())
        self.assertIsInstance(report, WorldGenerateReport)
        self.assertFalse(report.refused)
        self.assertTrue(report.documents_included)
        self.assertTrue(report.generation_is_deterministic)
        self.assertTrue(report.digests_bind_exact_documents)
        self.assertTrue(report.side_effect_free)
        self.assertEqual(report.counts.relay_depth, 3)
        self.assertFalse(report.validation.clean)
        self.assertEqual(report.validation.diagnostics[0].code, "backdated_event")

    def test_generation_refusal_retains_stage_and_fail_closed_digests_if_present(self) -> None:
        report = WorldGenerateReport.from_wire(refusal())
        self.assertTrue(report.refused)
        self.assertEqual(report.stage, "generated_world_parse")
        self.assertTrue(report.fail_closed)
        self.assertIsNone(report.world)

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "world_generate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(world_generate_report(envelope).world_digest, "a" * 64)
        args = request()
        self.assertEqual(Workspace(_SyncTool(payload())).world_generate_report(args).query_id, "generated-discriminating-v1-query")
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(payload())).world_generate_report(args)).counts.facts, 14)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").world_generate_report(args)
        self.assertTrue(report.documents_included)
        call.assert_called_once_with("world_generate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).world_generate_report(args)
            self.assertTrue(report.side_effect_free)
            async_call.assert_called_once_with("world_generate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
