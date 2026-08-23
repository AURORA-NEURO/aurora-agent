from __future__ import annotations

import json
import hashlib

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousRunTraceSession,
    AutonomousRunTracePersistenceCoordinator,
    AutonomousTracedRunResult,
    BrainRunResult,
    InMemoryAutonomousRunTraceStore,
    InMemoryAutonomousRunTraceTextStore,
    TransactionalJsonAutonomousRunTracePersistence,
    autonomous_run_trace_status,
    validate_autonomous_run_trace_snapshot,
)
from prism_sdk.errors import ArgumentError
from prism_sdk.authoring import content_digest
from prism_sdk.autonomy import AutonomousAgent


def test_hash_chained_trace_covers_every_authoritative_domain() -> None:
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 123)
    session = AutonomousRunTraceSession(
        store,
        run_id="all-domains",
        task_digest="a" * 64,
        domains=AUTONOMOUS_DOMAIN_NAMES,
    )
    session.started()
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        session.record(phase="plan_compiled", status="running", domains=(domain,), plan_digest="b" * 64)
    session.record(
        phase="evaluation_settled",
        status="running",
        selection_digest="c" * 64,
        detail_digest="d" * 64,
    )
    session.complete(status="completed", plan_digest="b" * 64, selection_digest="c" * 64)

    summary = session.summary()
    assert summary.domains == tuple(sorted(AUTONOMOUS_DOMAIN_NAMES))
    assert summary.status == "completed"
    assert summary.event_count == len(AUTONOMOUS_DOMAIN_NAMES) + 3
    summary_body = summary.to_dict()
    summary_body.pop("trace_digest")
    assert summary.trace_digest == content_digest(summary_body)
    assert store.verify_integrity()["events"] == summary.event_count


def test_provider_observer_and_receipt_projection_are_payload_free() -> None:
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 456)
    session = AutonomousRunTraceSession(store, run_id="provider-run", task_digest="a" * 64, domains=("coding",))
    session.started()
    observer = session.provider_observer()

    class Metadata:
        provider = "local"
        model = "test-model"
        input_tokens = 12
        requested_output_tokens = 20
        tool_count = 2

    observer.before(Metadata())
    observer.after(Metadata(), None, None, 4.5)
    session.complete(status="completed")
    events = store.events({"run_id": "provider-run"})
    assert events[1].phase == "provider_invocation_started"
    assert events[2].phase == "provider_invocation_finished"
    assert events[2].input_tokens == 12
    assert events[2].output_tokens == 0
    event_keys = {key for event in events for key in event.to_dict()}
    assert event_keys.isdisjoint({"prompt", "response", "messages", "arguments", "output"})

    receipt_store = InMemoryAutonomousRunTraceStore(clock=lambda: 789)
    receipt_session = AutonomousRunTraceSession(receipt_store, run_id="receipt-run", task_digest="a" * 64, domains=("data",))
    receipt_session.started()
    receipt_session.record_provider_receipts(
        ({
            "provider": "local",
            "model": "test-model",
            "attempt": 1,
            "turn": 2,
            "input_tokens": 10,
            "output_tokens": 6,
            "tool_count": 1,
            "latency_ms": 3.0,
            "selection_digest": "c" * 64,
            "outcome_digest": "d" * 64,
        },)
    )
    receipt_session.complete(status="completed")
    assert receipt_session.summary().provider_invocations == 1


def test_snapshot_json_cas_restore_and_tamper_detection() -> None:
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 1)
    session = AutonomousRunTraceSession(store, run_id="persisted", task_digest="a" * 64, domains=("science",))
    session.started()
    session.complete(status="paused", failure_code="approval_required")
    text_store = InMemoryAutonomousRunTraceTextStore()
    persistence = TransactionalJsonAutonomousRunTracePersistence(text_store)
    coordinator = AutonomousRunTracePersistenceCoordinator(store, persistence)
    snapshot = coordinator.flush()
    assert snapshot.snapshot_generation == 1
    assert snapshot.previous_snapshot_digest is None
    assert store.snapshot().to_dict() == snapshot.to_dict()
    assert coordinator.restore() is not None
    assert validate_autonomous_run_trace_snapshot(snapshot.to_dict()).snapshot_digest == snapshot.snapshot_digest

    legacy = snapshot.to_dict()
    legacy.pop("snapshot_generation")
    legacy.pop("previous_snapshot_digest")
    legacy["schema"] = "bioprism-python-autonomous-run-trace-snapshot/0.1"
    legacy_body = dict(legacy)
    legacy_body.pop("snapshot_digest")
    legacy["snapshot_digest"] = hashlib.sha256(
        json.dumps(legacy_body, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
    ).hexdigest()
    assert validate_autonomous_run_trace_snapshot(legacy).schema == "bioprism-python-autonomous-run-trace-snapshot/0.1"
    legacy_store = InMemoryAutonomousRunTraceStore(clock=lambda: 4)
    legacy_store.restore(legacy)
    upgraded = legacy_store.snapshot()
    assert upgraded.snapshot_generation == 1
    assert upgraded.previous_snapshot_digest is None
    assert upgraded.snapshot_digest != legacy["snapshot_digest"]

    tampered = snapshot.to_dict()
    tampered["events"][0]["status"] = "failed"
    with pytest.raises(ArgumentError):
        validate_autonomous_run_trace_snapshot(tampered)

    stale = InMemoryAutonomousRunTraceStore(clock=lambda: 2)
    restored = AutonomousRunTraceSession(stale, run_id="persisted", task_digest="a" * 64, domains=("science",))
    stale.restore(snapshot)
    assert restored.summary().status == "paused"
    stale_session = AutonomousRunTraceSession(stale, run_id="new-run", task_digest="a" * 64, domains=("science",))
    stale_session.started()
    stale_session.complete(status="completed")
    competing = InMemoryAutonomousRunTraceStore(clock=lambda: 3)
    competing_coordinator = AutonomousRunTracePersistenceCoordinator(
        competing,
        persistence,
    )
    competing_coordinator.restore()
    competing.append(
        {
            "run_id": "competing",
            "task_digest": "a" * 64,
            "domains": ("science",),
            "phase": "started",
            "status": "running",
        }
    )
    advanced = competing_coordinator.flush()
    assert advanced.snapshot_generation == 2
    assert advanced.previous_snapshot_digest == snapshot.snapshot_digest
    with pytest.raises(ArgumentError):
        coordinator.flush()


def test_agent_trace_facade_returns_live_result_and_metadata_summary() -> None:
    receipt = {
        "provider": "local",
        "model": "test-model",
        "attempt": 0,
        "turn": 0,
        "input_tokens": 8,
        "output_tokens": 4,
        "tool_count": 0,
        "latency_ms": 1.0,
        "selection_digest": "c" * 64,
        "outcome_digest": "d" * 64,
    }
    live = BrainRunResult(
        run_id="run",
        status="completed_provider_call",
        selection={"decision_digest": "c" * 64},
        prompt={"prompt_digest": "e" * 64},
        plan={"plan_digest": "b" * 64},
        response=None,
        outcome_digest="d" * 64,
        provider_invocations=(receipt,),
    )
    agent = AutonomousAgent.__new__(AutonomousAgent)
    agent.run = lambda **_: live  # type: ignore[method-assign]
    store = InMemoryAutonomousRunTraceStore(clock=lambda: 1)
    traced = agent.run_with_trace(
        task="trace this",
        domain="coding",
        credentials={},
        trace_store=store,
        run_id="facade-run",
    )
    assert isinstance(traced, AutonomousTracedRunResult)
    assert traced.result is live
    assert traced.trace.status == "completed"
    assert traced.trace.provider_invocations == 1
    wire = traced.to_dict()
    assert wire["result"] == "caller_owned_live_result_not_serialized"
    assert "trace this" not in json.dumps(wire)


@pytest.mark.parametrize(
    ("provider_status", "trace_status"),
    (
        ("completed", "completed"),
        ("completed_provider_call", "completed"),
        ("completed_without_replan", "partial"),
        ("approval_required", "paused"),
        ("provider_invalid", "refused"),
        ("execution_failed", "failed"),
    ),
)
def test_trace_status_mapping_is_conservative(provider_status: str, trace_status: str) -> None:
    assert autonomous_run_trace_status(provider_status) == trace_status
