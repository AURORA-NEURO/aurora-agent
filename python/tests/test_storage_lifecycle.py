from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    StorageLifecycleReport,
    StorageLifecycleSimulateArgs,
    Workspace,
    storage_lifecycle_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> StorageLifecycleSimulateArgs:
    return StorageLifecycleSimulateArgs(
        now=20,
        tiering_policy={"demote_to_warm_after": 5, "demote_to_cold_after": 12, "promote_after_accesses": 3, "promote_within": 2},
        records=[
            {"object": "stale-hot", "tier": "hot", "last_access": 0, "bytes": 100},
            {"object": "pinned-hot", "tier": "hot", "last_access": 0, "bytes": 200, "pinned": True},
            {"object": "recent-cold", "tier": "cold", "last_access": 19, "recent_accesses": 3, "bytes": 50},
        ],
        apply_tiering=True,
        quota={"limit": 1000, "reserve": 100},
        charges=[{"class": "objects", "purpose": "ingest", "bytes": 850}],
        releases=[{"class": "objects", "bytes": 50}],
        delegations=[{"bytes": 50, "charges": [{"class": "cache", "purpose": "cleanup", "bytes": 30}]}],
        absorb_delegated=[0],
    )


def row(index: int, **extra: object) -> dict:
    return {"index": index, "ok": True, **extra}


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/storage-lifecycle/0.1",
        "max_items": 100,
        "now": 20,
        "tiering": {
            "policy": {"demote_to_warm_after": 5, "demote_to_cold_after": 12, "promote_after_accesses": 3, "promote_within": 2},
            "plan": {
                "now": 20,
                "transitions": [
                    {"object": "stale-hot", "from": "Hot", "to": "Cold", "reason": {"Idle": {"epochs": 20, "threshold": 12}}, "skipped_a_tier": True},
                    {"object": "pinned-hot", "from": "Hot", "to": "Warm", "reason": {"HeldByPin": {"epochs": 20}}, "skipped_a_tier": False},
                    {"object": "recent-cold", "from": "Cold", "to": "Hot", "reason": {"Recent": {"accesses": 3, "idle_epochs": 1}}, "skipped_a_tier": True},
                ],
            },
            "transition_count": 3,
            "bytes_by_target": [{"tier": "Cold", "name": "cold", "bytes": 100}, {"tier": "Warm", "name": "warm", "bytes": 200}, {"tier": "Hot", "name": "hot", "bytes": 50}],
            "apply_requested": True,
            "apply_report": {"applied": 3, "absent": 0},
            "records": [
                {"object": "stale-hot", "tier": "Cold", "last_access": 0, "recent_accesses": 0, "bytes": 100, "pinned": False},
                {"object": "pinned-hot", "tier": "Warm", "last_access": 0, "recent_accesses": 0, "bytes": 200, "pinned": True},
                {"object": "recent-cold", "tier": "Hot", "last_access": 19, "recent_accesses": 3, "bytes": 50, "pinned": False},
            ],
            "omitted_records": 0,
            "input_rows": [row(0, object="stale-hot"), row(1, object="pinned-hot"), row(2, object="recent-cold")],
            "omitted_input_rows": 0,
        },
        "quota": {
            "limit": 1000,
            "reserve": 100,
            "used": 930,
            "remaining": 70,
            "remaining_for_ingest": 0,
            "remaining_for_evidence_finalization": 70,
            "remaining_for_cleanup": 70,
            "classes": [
                {"class": "Objects", "name": "objects", "reconstructible": False, "charged": 800},
                {"class": "Events", "name": "events", "reconstructible": False, "charged": 100},
                {"class": "Indexes", "name": "indexes", "reconstructible": True, "charged": 0},
                {"class": "Results", "name": "results", "reconstructible": False, "charged": 0},
                {"class": "Cache", "name": "cache", "reconstructible": True, "charged": 30},
            ],
            "charges": [row(0, class_name="objects", bytes=850), {"index": 1, "ok": False, "refusal": "ordinary ingest would consume the protected reserve", "fail_closed": True}],
            "omitted_charges": 0,
            "releases": [row(0, class_name="objects", bytes=50)],
            "omitted_releases": 0,
            "delegations": [row(0, child_index=0, bytes=50, charges=[row(0, class_name="cache", bytes=30)])],
            "omitted_delegations": 0,
            "absorptions": [row(0, child_index=0)],
            "omitted_absorptions": 0,
            "remaining_children": [],
            "omitted_children": 0,
        },
        "guarantees": [
            "tiering is planned against a caller-supplied logical epoch, so the same records and policy replay to the same transitions",
            "pinned objects cannot be planned below warm, and skipped hot-to-cold moves remain explicit",
            "dry-run is the default; applying a plan reports absent objects instead of silently under-applying it",
            "ordinary ingest protects the quota reserve while evidence finalization and cleanup may use it",
            "delegation subtracts allowance from the parent and absorption consumes the child, so allowance is not copied",
            "storage classes retain raw attribution and identify reconstructible indexes and cache data",
        ],
        "limitations": [
            "this is a deterministic in-memory lifecycle projection; it does not move bytes, run a scheduler, or persist an audit event",
            "quota authorization is a typed calculation and does not enforce writes in an external backend or provide tenant isolation",
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


class StorageLifecycleProjectionTests(unittest.TestCase):
    def test_args_preserve_bounded_planning_envelope(self) -> None:
        args = request()
        self.assertTrue(args.apply_tiering)
        self.assertEqual(args.to_mcp_arguments()["max_items"], 100)
        with self.assertRaises(ArgumentError):
            StorageLifecycleSimulateArgs(20, {"demote_to_warm_after": 5, "demote_to_cold_after": 12, "promote_after_accesses": 3, "promote_within": 2}, [], {"limit": 10, "reserve": 10})

    def test_report_keeps_plan_application_quota_reserve_and_allowance_distinct(self) -> None:
        report = storage_lifecycle_report(payload())
        self.assertIsInstance(report, StorageLifecycleReport)
        self.assertTrue(report.deterministic_plan)
        self.assertTrue(report.side_effect_free)
        self.assertEqual(report.tiering.transition_count, 3)
        self.assertEqual(report.tiering.skipped_transition_count, 2)
        self.assertEqual(report.tiering.pin_held_transition_count, 1)
        self.assertTrue(report.tiering.apply_reconciles)
        self.assertEqual(report.quota.remaining, 70)
        self.assertEqual(report.quota.charge_refusal_count, 1)
        self.assertEqual(report.reserve_protected_refusal_count, 1)
        self.assertTrue(report.quota.reserve_is_explicit)
        self.assertTrue(report.allowance_is_non_copyable)
        self.assertTrue(report.raw_class_attribution_is_preserved)
        self.assertEqual(report.fail_closed_row_count, 1)

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "storage_lifecycle_simulate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(storage_lifecycle_report(envelope).quota.used, 930)
        args = request()
        self.assertEqual(Workspace(_SyncTool(payload())).storage_lifecycle_simulate_report(args).tiering.transition_count, 3)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(payload())).storage_lifecycle_simulate_report(args)).quota.remaining, 70)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").storage_lifecycle_simulate_report(args)
        self.assertTrue(report.side_effect_free)
        call.assert_called_once_with("storage_lifecycle_simulate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).storage_lifecycle_simulate_report(args)
            self.assertEqual(report.tiering.records[1].tier, "Warm")
            async_call.assert_called_once_with("storage_lifecycle_simulate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
