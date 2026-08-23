from __future__ import annotations

import json
import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorObservation,
    AutonomousConnectorOperationRegistry,
    AutonomousConnectorReceiptJournal,
    AutonomousConnectorRegistration,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorWorker,
    AutonomousConnectorFeedbackPersistenceCoordinator,
    AutonomousConnectorWorkQueuePersistenceCoordinator,
    DomainEvidenceProviderConnectorManifest,
    InMemoryAutonomousConnectorFeedbackLedger,
    InMemoryAutonomousConnectorWorkQueue,
    TransactionalJsonAutonomousConnectorFeedbackSnapshotPersistence,
    content_digest,
)
from prism_sdk.errors import ArgumentError


class _SnapshotStore:
    def __init__(self):
        self.snapshot = None

    def read(self):
        return self.snapshot

    def write(self, snapshot):
        self.snapshot = snapshot


class _CasTextStore:
    def __init__(self):
        self.value = None

    def read(self):
        return self.value

    def write(self, value):
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest, value):
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


def _fixture(tmp_path):
    operations = AutonomousConnectorOperationRegistry()
    capabilities = ("review", "review+debugging")
    calls: list[str] = []

    def execute(_manifest, request):
        calls.append(request["operation_id"])
        return AutonomousConnectorObservation({"operation_id": request["operation_id"], "subject_digest": request["subject_digest"]})

    manifest = DomainEvidenceProviderConnectorManifest(
        connector_id="worker-test-connector",
        version="v1",
        provider="caller-managed",
        connector_kind="provider_api",
        domains=tuple(AUTONOMOUS_DOMAINS),
        capabilities=capabilities,
    )
    connector_registry = AutonomousConnectorRegistry([AutonomousConnectorRegistration(manifest, execute)])
    journal = AutonomousConnectorReceiptJournal(tmp_path / "receipts.jsonl")
    runtime = AutonomousConnectorRuntime(connector_registry, receipt_store=journal)
    return operations, connector_registry, runtime, journal, calls


def _request(plan, *, dispatch_id="worker-dispatch-1", operation_id="coding.repository_change_analysis", capability="review"):
    return AutonomousConnectorDispatchRequest(
        dispatch_id=dispatch_id,
        execution_id="worker-execution-1",
        call_id=f"call-{dispatch_id}",
        connector_id="worker-test-connector",
        domains=("coding",),
        capability=capability,
        request={"operation_id": operation_id, "subject_digest": "a" * 64},
        parent_digests=("b" * 64,),
        selection_plan_digest=plan.plan_digest,
        approved=True,
    )


def test_operation_registry_covers_all_domains_and_composite_capabilities(tmp_path) -> None:
    operations, connector_registry, _runtime, _journal, _calls = _fixture(tmp_path)
    assert {operation.operation_id for operation in operations.operations()} == {
        "coding.repository_change_analysis",
        "browser.web_evidence_retrieval",
        "data.dataset_quality_profile",
        "science.reproducible_evidence_acquisition",
        "biomedical.clinical_data_review",
        "neuroscience.signal_study_analysis",
        "operations.incident_runbook_observation",
        "enterprise.workflow_record_governance",
        "multi_agent.delegated_consensus_handoff",
        "multimodal.asset_alignment",
        "cross_domain.evidence_fanout_synthesis",
        "evaluation.benchmark_replay_analysis",
    }
    assert tuple(operation.domain for operation in operations.operations()) == tuple(sorted(AUTONOMOUS_DOMAINS))
    assert {operation.domain for operation in operations.operations()} == set(AUTONOMOUS_DOMAINS)
    assert all(operations.for_domain(domain) for domain in AUTONOMOUS_DOMAINS)
    assert "review+debugging" in operations.resolve("coding.repository_change_analysis").capabilities
    composite = connector_registry.select_for_domains(("coding",), capability="review+debugging")
    assert composite.complete is True
    assert _request(composite, capability="review+debugging").capability == "review+debugging"
    assert "subject_digest" not in json.dumps(operations.to_dict())
    with pytest.raises(ArgumentError, match="cover every autonomous domain"):
        AutonomousConnectorOperationRegistry(operations.operations()[:-1])


def test_work_queue_is_metadata_only_fenced_retry_bounded_and_tamper_evident(tmp_path) -> None:
    operations, connector_registry, _runtime, _journal, _calls = _fixture(tmp_path)
    plan = connector_registry.select_for_domains(("coding",), capability="review")
    request = _request(plan)
    queue = InMemoryAutonomousConnectorWorkQueue(operations)
    item = queue.enqueue(work_id="work-1", operation_id="coding.repository_change_analysis", request=request, now=1_000, max_attempts=3)
    assert "subject_digest" not in json.dumps(item.to_dict())
    assert queue.pending(now=1_000)[0].work_id == "work-1"
    assert queue.claim("work-1", "worker-a", lease_ms=100, now=1_000).lease_owner == "worker-a"
    with pytest.raises(ArgumentError, match="fenced"):
        queue.fail("work-1", "worker-b", "unknown", retryable=True, now=1_001)
    assert queue.claim("work-1", "worker-b", lease_ms=100, now=1_001) is None
    assert queue.claim("work-1", "worker-b", lease_ms=100, now=1_101).lease_owner == "worker-b"
    retried = queue.fail("work-1", "worker-b", "transport_error", retryable=True, now=1_101)
    assert retried.status == "queued"
    assert retried.available_at == 3_101
    assert queue.claim("work-1", "worker-c", lease_ms=100, now=3_101).attempts == 3
    assert queue.fail("work-1", "worker-c", "transport_error", retryable=True, now=3_101).status == "failed"
    assert queue.verify_integrity()["verified"] is True

    snapshot = queue.snapshot()
    assert "subject_digest" not in json.dumps(snapshot)
    restored = InMemoryAutonomousConnectorWorkQueue(operations)
    restored.restore(snapshot)
    assert restored.verify_integrity() == queue.verify_integrity()
    persistence = _SnapshotStore()
    coordinator = AutonomousConnectorWorkQueuePersistenceCoordinator(queue, persistence)
    flushed = coordinator.flush()
    assert flushed["snapshot_digest"] == persistence.snapshot["snapshot_digest"]
    restarted = InMemoryAutonomousConnectorWorkQueue(operations)
    assert AutonomousConnectorWorkQueuePersistenceCoordinator(restarted, persistence).restore()["items"] == 1
    assert restarted.verify_integrity() == queue.verify_integrity()
    tampered = json.loads(json.dumps(snapshot))
    tampered["items"][0]["status"] = "completed"
    with pytest.raises(ArgumentError, match="digest"):
        restored.restore(tampered)


def test_worker_rehydrates_once_replays_without_invocation_and_quarantines_missing_state(tmp_path) -> None:
    operations, connector_registry, runtime, journal, calls = _fixture(tmp_path)
    plan = connector_registry.select_for_domains(("coding",), capability="review")
    request = _request(plan)
    missing = _request(plan, dispatch_id="missing-dispatch")
    queue = InMemoryAutonomousConnectorWorkQueue(operations)
    queue.enqueue(work_id="fresh-work", operation_id="coding.repository_change_analysis", request=request, now=1_000)
    queue.enqueue(work_id="replay-work", operation_id="coding.repository_change_analysis", request=request, now=1_000)
    queue.enqueue(work_id="missing-work", operation_id="coding.repository_change_analysis", request=missing, now=1_000)

    def rehydrate(item):
        if item.work_id == "missing-work":
            return None
        return {"plan": plan, "request": request}

    result = AutonomousConnectorWorker(runtime, queue, rehydrate).run(worker_id="worker-a", now=1_000, lease_ms=10_000)
    assert result["completed"] == 2
    assert result["reconciled"] == 1
    assert calls == ["coding.repository_change_analysis"]
    assert all(row["value_retained"] is False for row in result["rows"])
    assert "subject_digest" not in json.dumps(result)
    assert queue.get("fresh-work").status == "completed"
    assert queue.get("replay-work").status == "completed"
    assert queue.get("missing-work").status == "reconciliation_required"
    assert journal.verify_integrity()["entries"] == 1


def test_feedback_requires_explicit_evaluator_and_projects_adaptive_signals(tmp_path) -> None:
    _operations, connector_registry, runtime, _journal, _calls = _fixture(tmp_path)
    plan = connector_registry.select_for_domains(("coding",), capability="review")
    request = _request(plan)
    result = runtime.dispatch_from_plan(plan, request)
    ledger = InMemoryAutonomousConnectorFeedbackLedger()
    with pytest.raises(ArgumentError, match="caller_evaluator"):
        ledger.record(feedback={"feedback_id": "implicit", "evaluator_id": "eval", "evaluator_version": "1", "reward": 1, "passed": True}, receipt=result.receipt)
    entry = ledger.record(
        feedback={"feedback_id": "feedback-1", "evaluator_id": "offline-rubric", "evaluator_version": "2026.08", "reward": 0.8, "passed": True, "source": "caller_evaluator", "evidence_digest": "c" * 64, "created_at": 1_000},
        receipt=result.receipt,
    )
    assert entry["reward"] == 0.8
    signals = ledger.signals(domain="coding", capability="review")
    assert signals["worker-test-connector"]["evaluator_reward"] == 0.8
    assert signals["worker-test-connector"]["success_rate"] == 1.0
    assert signals["worker-test-connector"]["latency_ms"] is None
    snapshot = ledger.snapshot()
    assert "subject_digest" not in json.dumps(snapshot)
    restored = InMemoryAutonomousConnectorFeedbackLedger()
    restored.restore(snapshot)
    assert restored.verify_integrity() == ledger.verify_integrity()
    tampered = json.loads(json.dumps(snapshot))
    tampered["entries"][0]["reward"] = -1
    with pytest.raises(ArgumentError, match="digest"):
        restored.restore(tampered)

    backend = _CasTextStore()
    persistence = TransactionalJsonAutonomousConnectorFeedbackSnapshotPersistence(backend)
    source_coordinator = AutonomousConnectorFeedbackPersistenceCoordinator(ledger, persistence)
    flushed = source_coordinator.flush()
    assert flushed["snapshot_digest"] == json.loads(backend.value)["snapshot_digest"]
    restarted = InMemoryAutonomousConnectorFeedbackLedger()
    restarted_coordinator = AutonomousConnectorFeedbackPersistenceCoordinator(restarted, persistence)
    assert restarted_coordinator.restore()["snapshot_digest"] == flushed["snapshot_digest"]
    assert restarted.signals(domain="coding", capability="review")["worker-test-connector"]["evaluator_reward"] == 0.8
    backend.value = json.dumps(json.loads(backend.value), indent=2)
    with pytest.raises(ArgumentError, match="not canonical"):
        persistence.read()
    persistence.write({"schema": "bioprism-python-autonomous-connector-feedback-ledger/0.1", "entries": [], "retention": "metadata_only_explicit_evaluator_signal_no_request_or_payload", "secret_material": "never_returned", "snapshot_digest": content_digest({"schema": "bioprism-python-autonomous-connector-feedback-ledger/0.1", "entries": [], "retention": "metadata_only_explicit_evaluator_signal_no_request_or_payload", "secret_material": "never_returned"})})
    with pytest.raises(ArgumentError, match="compare-and-swap conflict"):
        restarted_coordinator.flush()
