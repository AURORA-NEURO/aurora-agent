from __future__ import annotations

import asyncio
import json
import unittest
from unittest.mock import patch

from prism_sdk import (
    ApiClient,
    AsyncApiClient,
    AsyncWorkspace,
    FactoryLifecycleReport,
    FactoryLifecycleSimulateArgs,
    Workspace,
    factory_lifecycle_report,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.models import ToolResult


def request() -> FactoryLifecycleSimulateArgs:
    return FactoryLifecycleSimulateArgs(
        jobs=[
            {
                "id": "job-1",
                "resource_class": "compile",
                "idempotency": "idempotent",
                "priority": 5,
                "max_attempts": 3,
                "spec": {"kind": "pure-build"},
                "state": "queued",
                "attempts": 0,
            }
        ],
        workers=[
            {
                "worker_id": "worker-1",
                "classes": ["compile"],
                "lease_duration_nanos": 30_000_000_000,
            }
        ],
        actions=[
            {"kind": "lease", "worker_id": "worker-1", "now_nanos": 0},
            {"kind": "stage", "job_id": "job-1", "worker_id": "worker-1", "now_nanos": 1, "output": {"digest": "out-1"}},
            {"kind": "commit", "job_id": "job-1", "worker_id": "worker-1", "now_nanos": 2},
        ],
    )


def payload() -> dict:
    return {
        "ok": False,
        "action_count": 5,
        "action_failures": 1,
        "trace": [
            {
                "index": 0,
                "kind": "lease",
                "ok": True,
                "result": {
                    "job_id": "job-1",
                    "worker_id": "worker-1",
                    "attempt": 1,
                    "granted_at": {"nanos": 0},
                    "expires_at": {"nanos": 30_000_000_000},
                    "last_heartbeat": {"nanos": 0},
                },
            },
            {
                "index": 1,
                "kind": "stage",
                "ok": True,
                "result": {"job_id": "job-1", "visible_before_commit": False},
            },
            {"index": 2, "kind": "commit", "ok": True, "result": {"job_id": "job-1", "committed": True}},
            {
                "index": 3,
                "kind": "recover_expired",
                "ok": True,
                "result": [{"outcome": "quarantined", "job_id": "job-2", "reason": "ambiguous effect"}],
            },
            {"index": 4, "kind": "commit", "ok": False, "refusal": "job \"job-2\" has no active lease", "fail_closed": True},
        ],
        "jobs": [
            {
                "id": "job-1",
                "job": {
                    "id": "job-1",
                    "resource_class": "compile",
                    "idempotency": "idempotent",
                    "priority": 5,
                    "max_attempts": 3,
                    "spec": {"kind": "pure-build"},
                    "state": "succeeded",
                    "attempts": 1,
                },
                "committed_result": {"digest": "out-1"},
            },
            {
                "id": "job-2",
                "job": {
                    "id": "job-2",
                    "resource_class": "compile",
                    "idempotency": "non_idempotent",
                    "priority": 5,
                    "max_attempts": 3,
                    "spec": {"kind": "external-effect"},
                    "state": "quarantined",
                    "attempts": 1,
                    "reason": "ambiguous effect",
                },
                "committed_result": None,
            },
        ],
        "quarantined": [
            {
                "id": "job-2",
                "resource_class": "compile",
                "idempotency": "non_idempotent",
                "priority": 5,
                "max_attempts": 3,
                "spec": {"kind": "external-effect"},
                "state": "quarantined",
                "attempts": 1,
                "reason": "ambiguous effect",
            }
        ],
        "dead_lettered": [],
        "counts_by_class": {"compile": 2},
        "guarantees": [
            "the simulation delegates every lifecycle transition to the typed in-memory JobStore",
            "lease expiry branches on idempotency and never treats non-idempotent ambiguity as a safe retry",
            "staged outputs are reported invisible until atomic commit",
            "compensation, quarantine release, cancellation, and every refusal remain explicit in the replay trace",
            "no worker process, queue, clock, filesystem, network, or external side effect is created",
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


class FactoryLifecycleProjectionTests(unittest.TestCase):
    def test_args_bound_workers_and_unknown_actions_are_preserved_for_authority(self) -> None:
        args = request()
        self.assertEqual(args.to_mcp_arguments()["actions"][0]["kind"], "lease")
        unknown = FactoryLifecycleSimulateArgs(args.jobs, args.workers, [{"kind": "future_action"}])
        self.assertEqual(unknown.actions[0]["kind"], "future_action")
        with self.assertRaises(ArgumentError):
            FactoryLifecycleSimulateArgs(args.jobs, [{**args.workers[0], "worker_id": "worker-1"}, {**args.workers[0], "worker_id": "worker-1"}], [])

    def test_report_preserves_trace_refusals_recovery_and_visibility(self) -> None:
        report = factory_lifecycle_report(payload())
        self.assertIsInstance(report, FactoryLifecycleReport)
        self.assertFalse(report.complete)
        self.assertEqual(report.action_failures, 1)
        self.assertEqual(report.fail_closed_refusal_count, 1)
        self.assertEqual(report.committed_job_ids, ("job-1",))
        self.assertEqual(report.quarantined_job_ids, ("job-2",))
        self.assertEqual(report.recovery_outcomes, ("quarantined",))
        self.assertTrue(report.staged_visibility_is_explicit)
        self.assertTrue(report.no_external_effects_claimed)
        self.assertEqual(report.state_counts, {"succeeded": 1, "quarantined": 1})
        self.assertEqual(report.trace[0].lease.worker_id, "worker-1")

    def test_mcp_http_envelopes_and_sync_async_facades_delegate(self) -> None:
        envelope = {"ok": True, "tool": "factory_lifecycle_simulate", "mcp": {"result": {"structuredContent": payload()}}}
        self.assertEqual(factory_lifecycle_report(envelope).trace[-1].refusal, payload()["trace"][-1]["refusal"])
        args = request()
        self.assertEqual(Workspace(_SyncTool(payload())).factory_lifecycle_simulate_report(args).action_count, 5)
        self.assertEqual(asyncio.run(AsyncWorkspace(_AsyncTool(payload())).factory_lifecycle_simulate_report(args)).successful_action_count, 4)
        with patch.object(ApiClient, "call_tool", return_value=payload()) as call:
            report = ApiClient("http://127.0.0.1:1").factory_lifecycle_simulate_report(args)
        self.assertTrue(report.lifecycle_invariants_are_claimed)
        call.assert_called_once_with("factory_lifecycle_simulate", args.to_mcp_arguments())

        async def run() -> None:
            with patch.object(ApiClient, "call_tool", return_value=payload()) as async_call:
                report = await AsyncApiClient(ApiClient("http://127.0.0.1:1")).factory_lifecycle_simulate_report(args)
            self.assertEqual(report.dead_lettered_job_ids, ())
            async_call.assert_called_once_with("factory_lifecycle_simulate", args.to_mcp_arguments())

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
