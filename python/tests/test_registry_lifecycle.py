from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    RegistryLifecycleReport,
    RegistryLifecycleSimulateArgs,
    Workspace,
    registry_lifecycle_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> RegistryLifecycleSimulateArgs:
    return RegistryLifecycleSimulateArgs(
        packs=[{"not": "an attested benchmark pack"}],
        actions=[
            {"op": "publish", "pack_index": 0, "tier": "exploratory"},
            {"op": "resolve", "name": "missing@0.1.0"},
            {"op": "verify_all"},
        ],
    )


def payload() -> dict:
    return {
        "ok": True,
        "schema": "bioprism-mcp/registry-lifecycle/0.1",
        "policy": {"minimum_tier": "unranked", "require_independent_rebuild": False},
        "packs": [{"index": 0, "valid": False, "refusal": "document does not deserialise as a benchmark pack", "fail_closed": True}],
        "initial_integrity": {"artifact_count": 0, "log_count": 0, "broken_count": 0, "broken": [], "operations_allowed": True},
        "actions": [
            {"index": 0, "op": "publish", "ok": False, "refusal": "pack 0 is unavailable: invalid", "fail_closed": True},
            {"index": 1, "op": "resolve", "ok": True, "result": {"name": "missing@0.1.0", "found": False, "digest": None, "core_digest": None}},
            {"index": 2, "op": "verify_all", "ok": True, "result": {"clean": True, "broken_count": 0, "broken": []}},
        ],
        "final": {"artifact_count": 0, "log_count": 0, "broken_count": 0, "integrity_clean": True, "verification": [], "log": []},
        "registry": {"artifacts": {}, "core_digests": {}, "tiers": {}, "statuses": {}, "names": {}, "latest_artifact": {}, "log": []},
        "guarantees": [
            "attested input packs are re-verified before they can be published or supersede an artifact",
            "artifact bytes remain content-addressed and lifecycle events remain append-only",
            "failed actions are typed refusals and do not abort independent later actions",
            "serialized registry integrity is checked before any lookup or mutation",
            "the returned registry can be supplied as index in a later call to continue the simulation",
        ],
        "limitations": [
            "this is a local deterministic registry projection; it does not provide network transport, signatures, federation, moderation, quarantine, or authentication",
            "a valid attestation proves internal digest consistency, not scientific validity or publisher identity",
            "withdrawal preserves historical bytes and records a reason; it does not delete or hide an artifact",
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


class RegistryLifecycleProjectionTests(unittest.TestCase):
    def test_args_keep_invalid_pack_and_action_rows_for_authority(self) -> None:
        args = request()
        self.assertEqual(args.packs[0]["not"], "an attested benchmark pack")
        self.assertTrue(args.include_index)
        with self.assertRaises(ArgumentError):
            RegistryLifecycleSimulateArgs(actions=[{} for _ in range(257)])

    def test_report_preserves_preflight_refusal_integrity_and_continuation(self) -> None:
        report = registry_lifecycle_report(payload())
        self.assertIsInstance(report, RegistryLifecycleReport)
        self.assertEqual(report.valid_pack_count, 0)
        self.assertEqual(report.invalid_pack_count, 1)
        self.assertEqual(report.failed_action_count, 1)
        self.assertEqual(report.fail_closed_action_count, 1)
        self.assertTrue(report.actions[1].found is False)
        self.assertTrue(report.actions[2].clean)
        self.assertTrue(report.initial_integrity.clean)
        self.assertTrue(report.final.integrity_clean)
        self.assertTrue(report.continuation_available)
        self.assertTrue(report.append_only_events_are_claimed)
        self.assertTrue(report.independent_actions_are_claimed)
        self.assertTrue(report.integrity_checked_before_mutation)
        self.assertTrue(report.withdrawal_is_non_destructive)
        self.assertTrue(report.local_and_side_effect_free)

    def test_mcp_http_envelopes_and_all_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "registry_lifecycle_simulate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertTrue(registry_lifecycle_report(envelope).continuation_available)
        args = request()
        self.assertEqual(Workspace(_SyncTool(payload())).registry_lifecycle_simulate_report(args).actions[2].clean, True)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(payload())).registry_lifecycle_simulate_report(args)).invalid_pack_count, 1)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").registry_lifecycle_simulate_report(args)
        self.assertTrue(report.local_and_side_effect_free)
        call.assert_called_once_with("registry_lifecycle_simulate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).registry_lifecycle_simulate_report(args)
            self.assertEqual(report.final.artifact_count, 0)
            async_call.assert_called_once_with("registry_lifecycle_simulate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
