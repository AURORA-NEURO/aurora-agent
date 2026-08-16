from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    CacheInvalidationReport,
    CacheInvalidationSimulateArgs,
    Workspace,
    cache_invalidation_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


DIGEST_INVALID = "d-invalid"
DIGEST_UNPROVEN = "d-unproven"


def request() -> CacheInvalidationSimulateArgs:
    return CacheInvalidationSimulateArgs(
        schema={"name": "decision-cache", "components": ["input", "code"], "reuse": "same_build_only"},
        entries=[
            {"components": {"input": "world@1", "code": "build-a"}, "value": {"answer": "derived"}, "produced_by": "build-a", "written_at": 1, "dependencies": {"kind": "declared", "resources": ["derived"]}},
            {"components": {"input": "world@2", "code": "build-a"}, "value": {"answer": "legacy"}, "produced_by": "build-a", "written_at": 1, "dependencies": "undeclared"},
        ],
        graph={"declared": [{"resource": "derived", "depends_on": ["input"]}], "opaque": ["input"]},
        changed="input",
        apply=True,
        apply_at=2,
        lookups=[{"components": {"input": "world@2", "code": "build-a"}, "requested_by": "build-a"}],
    )


def row(index: int, **extra: object) -> dict:
    return {"index": index, **extra}


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/cache-invalidation/0.1",
        "max_items": 100,
        "key_schema": {"name": "decision-cache", "components": ["code", "input"], "reuse": "SameBuildOnly"},
        "entries": {"accepted": 2, "submitted": 2, "rows": [row(0, ok=True, digest=DIGEST_INVALID, dependencies={"Declared": ["derived"]}), row(1, ok=True, digest=DIGEST_UNPROVEN, dependencies="Undeclared")], "omitted_rows": 0},
        "graph": {"known_resources": ["derived", "input"], "known_resource_count": 2, "opaque_resources": ["input"], "cycle": None, "cycle_is_a_scheduler_defect_not_an_invalidation_hang": False},
        "invalidation": {
            "changed": "input",
            "plan": {
                "changed": "input",
                "affected_resources": ["derived", "input"],
                "invalid_entries": [DIGEST_INVALID],
                "proved_unaffected": [],
                "completeness": {"Partial": {"opaque_resources": [], "unknown_resources": [], "entries_without_declared_dependencies": [DIGEST_UNPROVEN], "entries_depending_on_opaque_resources": []}},
                "population": 2,
            },
            "apply_requested": True,
            "apply_report": {"removed": [DIGEST_INVALID], "marked_unproven": [DIGEST_UNPROVEN], "left_proven": [], "invalidation_was_complete": False},
        },
        "lookups": {
            "pre_apply": [row(0, ok=True, hit=True, value={"answer": "legacy"}, proof={"digest": DIGEST_UNPROVEN, "schema_digest": "schema", "matched": [["input", "world@2"]], "produced_by": "build-a", "requested_by": "build-a", "reuse": "SameBuildOnly", "written_at": 1})],
            "post_apply": [row(0, ok=True, hit=False, miss_reason={"UnprovenAfterPartialInvalidation": {"since": 2, "cause": "invalidation of input was partial"}})],
            "omitted_post_apply": 0,
        },
        "reprove": [],
        "cache": {
            "entry_count": 1,
            "unproven": [DIGEST_UNPROVEN],
            "hits": 1,
            "misses_by_reason": [{"reason": "unproven", "count": 1}],
            "hit_rate": 0.5,
            "entries": [{"digest": DIGEST_UNPROVEN, "key": {"schema_name": "decision-cache", "schema_digest": "schema", "components": {"code": "build-a", "input": "world@2"}}, "value": {"answer": "legacy"}, "produced_by": "build-a", "written_at": 1, "dependencies": "Undeclared", "status": {"Unproven": {"since": 2, "cause": "invalidation of input was partial"}}}],
            "omitted_entries": 0,
        },
        "guarantees": [
            "cache keys are rebuilt from every declared component and never from a bare digest",
            "cross-build reuse, key collisions, unproven entries, and cold misses remain distinct outcomes",
            "partial invalidation marks unknown entries unproven rather than serving them optimistically",
            "apply is explicit and requires a caller-supplied logical epoch; no wall clock is read",
            "re-proving names the digest and build that re-established currentness",
        ],
        "limitations": [
            "the cache and dependency graph are in-memory projections; no durable index, tenant isolation, eviction worker, or external invalidation feed is created",
            "a caller-supplied dependency declaration is not independently discovered or scientifically validated",
            "values are returned as supplied and are not treated as canonical truth outside the cache proof",
        ],
    }


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


class CacheInvalidationProjectionTests(unittest.TestCase):
    def test_args_keep_explicit_apply_and_reproof_bounds(self) -> None:
        args = request()
        self.assertTrue(args.apply)
        self.assertEqual(args.apply_at, 2)
        with self.assertRaises(ArgumentError):
            CacheInvalidationSimulateArgs({"name": "x", "components": [], "reuse": "same_build_only"})

    def test_report_preserves_partial_unknowns_misses_and_reconciliation(self) -> None:
        report = cache_invalidation_report(payload())
        self.assertIsInstance(report, CacheInvalidationReport)
        self.assertFalse(report.explicit_dry_run)
        self.assertTrue(report.partial_invalidation)
        self.assertEqual(report.plan.invalid_entries, (DIGEST_INVALID,))
        self.assertEqual(report.apply.removed, (DIGEST_INVALID,))
        self.assertEqual(report.apply.marked_unproven, (DIGEST_UNPROVEN,))
        self.assertEqual(report.unproven_count, 1)
        self.assertEqual(report.post_apply[0].miss_name, "UnprovenAfterPartialInvalidation")
        self.assertTrue(report.key_reconstruction_is_claimed)
        self.assertTrue(report.partial_unknowns_are_not_served)
        self.assertTrue(report.reproof_is_attributed)
        self.assertTrue(report.side_effect_free)

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "cache_invalidation_simulate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertTrue(cache_invalidation_report(envelope).partial_invalidation)
        args = request()
        self.assertEqual(Workspace(_SyncTool(payload())).cache_invalidation_simulate_report(args).cache.unproven, (DIGEST_UNPROVEN,))
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(payload())).cache_invalidation_simulate_report(args)).apply.removed, (DIGEST_INVALID,))
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").cache_invalidation_simulate_report(args)
        self.assertEqual(report.lookup_hit_count, 1)
        call.assert_called_once_with("cache_invalidation_simulate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).cache_invalidation_simulate_report(args)
            self.assertEqual(report.cache.hit_rate, 0.5)
            async_call.assert_called_once_with("cache_invalidation_simulate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
