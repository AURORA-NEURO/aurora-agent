from __future__ import annotations

import asyncio
import hashlib

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AsyncAutonomousBrainControlPlaneMonitor,
    AutonomousBrainControlPlaneMonitor,
    BrainControlClient,
    BrainControlError,
    DurableBrainControlPlaneAdapter,
)
from prism_sdk.jobs import BrainJobStore


def _digest(letter: str) -> str:
    return letter * 64


def _job(domain: str, index: int, state: str = "queued") -> dict[str, object]:
    return {
        "schema": "bioprism-brain-job/0.1",
        "job_id": f"control-job-{index}",
        "idempotency_key_digest": _digest("a"),
        "spec_digest": _digest("b"),
        "domain": domain,
        "capability": "bounded_task",
        "risk_class": "review",
        "priority": 0,
        "max_attempts": 3,
        "state": state,
        "attempts": 0,
        "lease_owner": None,
        "lease_expires_ns": None,
        "checkpoint_digest": None,
        "side_effect_boundary": "not_started",
        "recovered_after_restart": False,
        "reason_digest": None,
        "result_digest": None,
        "reconciliation_digest": None,
        "created_sequence": index + 1,
        "updated_sequence": index + 1,
        "record_digest": _digest("c"),
        "spec": "not_returned; caller resolver owns rehydration",
        "retention": "metadata_only_hash_chained",
    }


def _event(job_id: str, state: str = "succeeded") -> dict[str, object]:
    return {
        "schema": "bioprism-brain-job-event/0.1",
        "sequence": 1,
        "event_type": "job_completed",
        "job_id": job_id,
        "payload": {"state": state},
        "previous_digest": "",
        "event_digest": _digest("e"),
        "head_digest": _digest("e"),
        "created_ns": 1,
        "retention": "metadata_only_hash_chained",
    }


class _Client:
    def __init__(self, jobs: dict[str, dict[str, object]]) -> None:
        self.jobs = jobs
        self.approval_arguments: list[dict[str, object]] = []

    def job_status(self, job_id: str) -> dict[str, object]:
        return {"schema": "bioprism-brain-control-plane/0.1", "ok": True, "job": self.jobs[job_id], "head_digest": _digest("d")}

    def job_events(self, request: object) -> dict[str, object]:
        arguments = request.to_arguments()
        job_id = arguments.get("job_id")
        state = self.jobs[job_id]["state"] if isinstance(job_id, str) else "queued"
        events = [] if state != "succeeded" else [_event(job_id)]
        return {
            "schema": "bioprism-brain-control-plane/0.1",
            "ok": True,
            "events": events,
            "after": arguments["after"],
            "next_after": events[-1]["sequence"] if events else arguments["after"],
            "head_digest": _digest("d"),
            "chain": "sha256_prev_digest",
        }

    def approval(self, request: object) -> dict[str, object]:
        arguments = request.to_arguments()
        self.approval_arguments.append(arguments)
        job_id = arguments["job_id"]
        return {
            "schema": "bioprism-brain-control-plane/0.1",
            "ok": True,
            "operation": arguments["action"],
            "job": self.jobs[job_id],
            "event": None,
            "authorization": {"posture": "caller_authenticated_out_of_band"},
        }


def test_sync_monitor_fans_out_status_across_all_twelve_domains() -> None:
    jobs = {
        f"control-job-{index}": _job(domain, index)
        for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)
    }
    result = AutonomousBrainControlPlaneMonitor(_Client(jobs)).status_all(
        tuple(jobs), max_parallel=3
    )
    assert result["status"] == "completed"
    assert len(result["jobs"]) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert set(result["domains"]) == set(AUTONOMOUS_DOMAIN_NAMES)
    assert result["max_parallel"] == 3


def test_sync_monitor_validates_cursor_approval_and_waits_without_raw_values() -> None:
    job = _job("science", 90, "waiting_approval")
    client = _Client({"control-job-90": job})
    monitor = AutonomousBrainControlPlaneMonitor(client)
    approval = monitor.approval(
        "control-job-90",
        "approve",
        reason="reviewed scope",
        authorization_digest=_digest("f"),
    )
    assert approval["approval"]["job"]["job_id"] == "control-job-90"
    assert client.approval_arguments[0]["authorization_digest"] == _digest("f")

    job["state"] = "succeeded"
    events = monitor.events("control-job-90", after=0, limit=4)
    assert events["events"]["events"][0]["sequence"] == 1

    now = 0.0
    transitioning = True

    def clock() -> float:
        return now

    def sleep(milliseconds: int) -> None:
        nonlocal now, transitioning
        now += milliseconds
        if transitioning:
            client.jobs["control-job-90"]["state"] = "succeeded"

    client.jobs["control-job-90"]["state"] = "queued"
    reached = AutonomousBrainControlPlaneMonitor(client, clock=clock, sleep=sleep).wait(
        "control-job-90", timeout_ms=10, poll_ms=1, max_polls=4
    )
    assert reached["status"] == "reached"
    assert reached["terminal_state"] == "succeeded"
    assert len(reached["events"]) == 1

    transitioning = False
    client.jobs["control-job-90"]["state"] = "waiting_approval"
    now = 0.0
    timed_out = AutonomousBrainControlPlaneMonitor(client, clock=clock, sleep=sleep).wait(
        "control-job-90", timeout_ms=2, poll_ms=1, max_polls=2
    )
    assert timed_out["status"] == "timed_out"
    assert timed_out["terminal_state"] == "waiting_approval"


def test_monitor_rejects_secret_shaped_projection_and_broken_cursor() -> None:
    job = _job("coding", 91)
    unsafe = _Client({"control-job-91": job})
    job["prompt"] = "must not cross boundary"
    with pytest.raises(BrainControlError):
        AutonomousBrainControlPlaneMonitor(unsafe).status("control-job-91")

    job.pop("prompt")

    class BrokenCursor(_Client):
        def job_events(self, request: object) -> dict[str, object]:
            result = super().job_events(request)
            result["after"] = 1
            return result

    with pytest.raises(BrainControlError):
        AutonomousBrainControlPlaneMonitor(BrokenCursor({"control-job-91": job})).events(
            "control-job-91", after=0, limit=4
        )


def test_monitor_uses_the_existing_durable_brain_transport(tmp_path) -> None:
    def digest(value: str) -> str:
        return hashlib.sha256(value.encode("utf-8")).hexdigest()

    with BrainJobStore(tmp_path / "monitor.sqlite3") as store:
        adapter = DurableBrainControlPlaneAdapter(
            store,
            authorizer=lambda _operation, _metadata: True,
            principal="monitor-test",
        )
        client = BrainControlClient(adapter.call_tool)
        client.submit_job(
            {
                "job_id": "durable-monitor-job",
                "idempotency_key": "durable-monitor-key",
                "spec_digest": digest("spec"),
                "domain": "coding",
                "capability": "bounded_task",
                "risk_class": "review",
                "max_attempts": 3,
            }
        )
        monitor = AutonomousBrainControlPlaneMonitor(client)
        status = monitor.status("durable-monitor-job")
        page = monitor.events("durable-monitor-job", after=0, limit=4)

    assert status["status"]["job"]["domain"] == "coding"
    assert page["events"]["events"][0]["job_id"] == "durable-monitor-job"


def test_async_monitor_fans_out_all_domains_and_preserves_bounds() -> None:
    jobs = {
        f"control-job-{index}": _job(domain, index)
        for index, domain in enumerate(AUTONOMOUS_DOMAIN_NAMES)
    }

    class AsyncClient(_Client):
        async def job_status(self, job_id: str) -> dict[str, object]:
            await asyncio.sleep(0)
            return super().job_status(job_id)

        async def job_events(self, request: object) -> dict[str, object]:
            await asyncio.sleep(0)
            return super().job_events(request)

        async def approval(self, request: object) -> dict[str, object]:
            await asyncio.sleep(0)
            return super().approval(request)

    async def run() -> None:
        result = await AsyncAutonomousBrainControlPlaneMonitor(AsyncClient(jobs)).status_all(
            tuple(jobs), max_parallel=4
        )
        assert result["status"] == "completed"
        assert set(result["domains"]) == set(AUTONOMOUS_DOMAIN_NAMES)

    asyncio.run(run())
