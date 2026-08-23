from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    BrainEpisodicMemory,
    BrainLearningLedger,
    BrainMemoryPersistenceCoordinator,
    BrainLearningPersistenceCoordinator,
    BrainJobPersistenceCoordinator,
    BrainJobStore,
    BrainModelHealthPersistenceCoordinator,
    BrainModelHealthStore,
    BrainModelObservation,
    ProviderHealthLedger,
    ProviderHealthPersistenceCoordinator,
    PROVIDER_OBSERVATION_SCHEMA,
    AutonomousGoalLedger,
    AutonomousGoalPersistenceCoordinator,
    AutonomousDecisionCycle,
    AutonomousDecisionCyclePersistenceCoordinator,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPersistenceCoordinator,
    AutonomousExecutionPolicy,
    AutonomousHttpSnapshotTextStore,
    InMemoryAutonomousDecisionCycleStateStore,
    TransactionalJsonAutonomousExecutionSnapshotPersistence,
    TransactionalJsonBrainLearningSnapshotPersistence,
    TransactionalJsonBrainMemorySnapshotPersistence,
    TransactionalJsonAutonomousGoalSnapshotPersistence,
    TransactionalJsonProviderHealthSnapshotPersistence,
    TransactionalJsonBrainJobSnapshotPersistence,
    TransactionalJsonBrainModelHealthSnapshotPersistence,
    TransactionalJsonAutonomousDecisionCycleSnapshotPersistence,
)
from prism_sdk.authoring import content_digest
from prism_sdk.errors import ArgumentError, TransportError


class _Response:
    def __init__(self, payload: bytes = b"", *, status: int = 200) -> None:
        self.status = status
        self.headers = {"Content-Type": "application/json"}
        self._payload = payload
        self._offset = 0

    def __enter__(self) -> "_Response":
        return self

    def __exit__(self, *_args: object) -> None:
        return None

    def getcode(self) -> int:
        return self.status

    def read(self, size: int = -1) -> bytes:
        if size < 0:
            size = len(self._payload) - self._offset
        chunk = self._payload[self._offset : self._offset + size]
        self._offset += len(chunk)
        return chunk


def _snapshot(domain: str, version: str) -> str:
    body = {"schema": "test-snapshot/0.1", "domain": domain, "version": version, "metadata_only": True}
    return json.dumps({**body, "snapshot_digest": content_digest(body)}, sort_keys=True, separators=(",", ":"))


def _header(request, name: str) -> str | None:
    for key, value in request.headers.items():
        if key.lower() == name.lower():
            return value
    return None


def test_http_snapshot_store_supports_all_domains_transient_headers_and_cas() -> None:
    values: dict[str, str] = {}
    contexts: list[dict[str, object]] = []
    requests: list[tuple[str, str | None, str | None]] = []

    def opener(request, _timeout):
        resource = _header(request, "X-Aurora-Snapshot-Resource")
        assert resource is not None
        requests.append((request.get_method(), _header(request, "If-Match"), _header(request, "If-None-Match")))
        if request.get_method() == "GET":
            return _Response(values[resource].encode("utf-8")) if resource in values else _Response(status=404)
        current = values.get(resource)
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        expected = _header(request, "If-Match")
        if expected is not None:
            expected = expected.strip('"')
            if current is None or json.loads(current)["snapshot_digest"] != expected:
                return _Response(status=412)
        values[resource] = request.data.decode("utf-8")
        return _Response(status=204)

    for domain in AUTONOMOUS_DOMAINS:
        store = AutonomousHttpSnapshotTextStore(
            "https://state.test/snapshots",
            f"{domain}/state",
            allowed_hosts=("state.test",),
            header_resolver=lambda context: (contexts.append(dict(context)) or {"Authorization": "transient-test-credential"}),
            opener=opener,
        )
        assert store.read() is None
        assert store.write_if_unchanged(None, _snapshot(domain, "one"))
        assert not store.write_if_unchanged(None, _snapshot(domain, "two"))
        current = json.loads(store.read())
        assert current["domain"] == domain
        assert store.write_if_unchanged(current["snapshot_digest"], _snapshot(domain, "three"))
        assert store.describe()["credentials"] == "transient_header_resolver;never_returned"

    assert len(contexts) == len(AUTONOMOUS_DOMAINS) * 5
    assert all(context["expected_snapshot_digest"] is None or len(context["expected_snapshot_digest"]) == 64 for context in contexts)
    assert len(requests) == len(AUTONOMOUS_DOMAINS) * 5
    assert all("transient-test-credential" not in json.dumps(values) for _ in [0])


def test_http_snapshot_store_enforces_endpoint_snapshot_and_response_bounds() -> None:
    with pytest.raises(ArgumentError, match="HTTPS"):
        AutonomousHttpSnapshotTextStore("http://state.test/snapshots", "state", allowed_hosts=("state.test",))
    with pytest.raises(ArgumentError, match="allowlist"):
        AutonomousHttpSnapshotTextStore("https://state.test/snapshots", "state", allowed_hosts=("other.test",))
    with pytest.raises(ArgumentError, match="resource"):
        AutonomousHttpSnapshotTextStore("https://state.test/snapshots", "unsafe resource", allowed_hosts=("state.test",))
    oversized = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        max_response_bytes=8,
        opener=lambda _request, _timeout: _Response(b"x" * 9),
    )
    with pytest.raises(TransportError, match="exceeded"):
        oversized.read()
    malformed = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: _Response(b"[]"),
    )
    with pytest.raises(ArgumentError, match="JSON object"):
        malformed.read()


def test_http_snapshot_store_separates_cas_conflicts_from_transport_failures_and_rejects_headers() -> None:
    conflict = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: _Response(status=409),
    )
    assert not conflict.write_if_unchanged("a" * 64, _snapshot("coding", "one"))
    with pytest.raises(ArgumentError, match="digest"):
        conflict.write_if_unchanged("bad", _snapshot("coding", "one"))
    timeout = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        opener=lambda _request, _timeout: (_ for _ in ()).throw(TimeoutError()),
    )
    with pytest.raises(TransportError, match="transport"):
        timeout.read()
    unsafe = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "state",
        allowed_hosts=("state.test",),
        header_resolver=lambda _context: {"X-Test": "line\nbreak"},
        opener=lambda _request, _timeout: _Response(status=404),
    )
    with pytest.raises(ArgumentError, match="header value"):
        unsafe.read()


def test_http_snapshot_store_plugs_into_decision_cycle_restart_and_cas() -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/decision-cycles",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonAutonomousDecisionCycleSnapshotPersistence(text_store)
    source = InMemoryAutonomousDecisionCycleStateStore()
    cycle = AutonomousDecisionCycle(source, cycle_id="http-cycle", task="HTTP restart cycle", mode="single_domain")
    cycle.advance(phase="route_pending", route_digest="a" * 64)
    coordinator = AutonomousDecisionCyclePersistenceCoordinator(source, persistence)
    flushed = coordinator.flush()

    restored_store = InMemoryAutonomousDecisionCycleStateStore()
    restored = AutonomousDecisionCyclePersistenceCoordinator(restored_store, persistence)
    assert restored.restore().snapshot_digest == flushed.snapshot_digest
    assert restored_store.load("http-cycle").state_digest == source.load("http-cycle").state_digest


def test_http_snapshot_store_plugs_into_execution_journal_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/execution-journal",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonAutonomousExecutionSnapshotPersistence(text_store)
    journal = AutonomousExecutionJournal(tmp_path / "source.jsonl")
    policy = AutonomousExecutionPolicy(max_steps=4)
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        execution = AutonomousExecutionController(
            execution_id=f"http-execution-{index}",
            domain=domain,
            capability="observability",
            risk_class="read_only",
            policy=policy,
            journal=journal,
        )
        execution.checkpoint(status="paused", reason="http_restart_round_trip")
    source_coordinator = AutonomousExecutionPersistenceCoordinator(journal, persistence)
    snapshot = source_coordinator.flush()

    restored_journal = AutonomousExecutionJournal(tmp_path / "restored.jsonl")
    restored_snapshot = AutonomousExecutionPersistenceCoordinator(restored_journal, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == snapshot["snapshot_digest"]
    assert {row["event"]["domain"] for row in restored_journal.events()} == set(AUTONOMOUS_DOMAINS)
    assert restored_journal.verify_integrity()["verified"] is True


def test_http_snapshot_store_plugs_into_durable_job_worker_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/brain-jobs",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonBrainJobSnapshotPersistence(text_store)
    with BrainJobStore(tmp_path / "source-jobs.sqlite3") as source:
        for index, domain in enumerate(AUTONOMOUS_DOMAINS):
            source.submit({
                "job_id": f"http-job-{index}",
                "idempotency_key": f"http-idempotency-{index}",
                "spec_digest": content_digest({"domain": domain, "index": index}),
                "domain": domain,
                "capability": "observability",
                "risk_class": "read_only",
                "priority": index,
                "max_attempts": 3,
                "checkpoint": {"phase": "submitted"},
            })
        flushed = BrainJobPersistenceCoordinator(source, persistence).flush()
    with BrainJobStore(tmp_path / "restored-jobs.sqlite3") as restored:
        restored_snapshot = BrainJobPersistenceCoordinator(restored, persistence).restore()
        assert restored_snapshot is not None
        assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
        assert {record.domain for record in restored.inventory(limit=256)} == set(AUTONOMOUS_DOMAINS)
        assert restored.verify_integrity()["ok"] is True


def test_http_snapshot_store_plugs_into_model_health_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/model-health",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonBrainModelHealthSnapshotPersistence(text_store)
    with BrainModelHealthStore(tmp_path / "source-model-health.sqlite3") as source:
        for index, domain in enumerate(AUTONOMOUS_DOMAINS):
            source.record(BrainModelObservation(
                provider="offline",
                model="offline-model",
                domain=domain,
                capability="domain_review",
                risk_class="read_only",
                status="completed",
                outcome="success",
                latency_ms=1 + index,
                quality_reward=0.75,
                quality_passed=True,
            ))
        flushed = BrainModelHealthPersistenceCoordinator(source, persistence).flush()
    with BrainModelHealthStore(tmp_path / "restored-model-health.sqlite3") as restored:
        restored_snapshot = BrainModelHealthPersistenceCoordinator(restored, persistence).restore()
        assert restored_snapshot is not None
        assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
        assert restored.health()[0].attempts == len(AUTONOMOUS_DOMAINS)
        assert restored.verify_integrity()["verified"] is True


def test_http_snapshot_store_plugs_into_learning_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/learning-ledger",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonBrainLearningSnapshotPersistence(text_store)
    source = BrainLearningLedger(tmp_path / "source-learning.jsonl")
    for index, domain in enumerate(AUTONOMOUS_DOMAINS, start=1):
        source.append(
            {
                "learning_evidence": {"evidence_digest": f"{index:064x}", "domain": domain},
                "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": index, "arms": []},
            },
            context_digest=f"{index:064x}",
            replay={"run_id": f"http-{domain}", "domain": domain},
        )
    flushed = BrainLearningPersistenceCoordinator(source, persistence).flush()

    restored = BrainLearningLedger(tmp_path / "restored-learning.jsonl")
    restored_snapshot = BrainLearningPersistenceCoordinator(restored, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {row["domain"] for row in restored.replays(limit=128)} == set(AUTONOMOUS_DOMAINS)


def test_http_snapshot_store_plugs_into_episodic_memory_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/episodic-memory",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonBrainMemorySnapshotPersistence(text_store)
    source = BrainEpisodicMemory(tmp_path / "source-memory.sqlite3")
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        source.record_episode({
            "episode_id": f"http-memory-{index}",
            "run_id": f"http-run-{index}",
            "result_kind": "provider",
            "status": "completed",
            "task_digest": content_digest({"domain": domain, "index": index}),
            "context": {"domain": domain, "capability": "review", "risk_class": "read_only"},
            "selected_model": {"provider": "offline", "model": "offline-model"},
            "digests": {"outcome_digest": f"{index + 1:064x}"},
            "tags": ["http", "restart"],
        })
    flushed = BrainMemoryPersistenceCoordinator(source, persistence).flush()
    restored = BrainEpisodicMemory(tmp_path / "restored-memory.sqlite3")
    restored_snapshot = BrainMemoryPersistenceCoordinator(restored, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {row["context"]["domain"] for row in restored.retrieve(limit=128)} == set(AUTONOMOUS_DOMAINS)
    assert restored.verify_integrity()["ok"] is True
    source.close()
    restored.close()


def test_http_snapshot_store_plugs_into_goal_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/goals",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonAutonomousGoalSnapshotPersistence(text_store)
    source = AutonomousGoalLedger(str(tmp_path / "source-goals.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS))
    for index, domain in enumerate(AUTONOMOUS_DOMAINS):
        source.create(
            goal_id=f"http-goal-{index}",
            task_digest=content_digest({"domain": domain, "index": index}),
            domain=domain,
            capability="review",
            risk_class="read_only",
            now_ns=index + 1,
        )
    flushed = AutonomousGoalPersistenceCoordinator(source, persistence).flush()
    restored = AutonomousGoalLedger(str(tmp_path / "restored-goals.sqlite3"), max_goals=len(AUTONOMOUS_DOMAINS))
    restored_snapshot = AutonomousGoalPersistenceCoordinator(restored, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {record.domain for record in restored.list(limit=128)} == set(AUTONOMOUS_DOMAINS)
    assert restored.verify_integrity()["ok"] is True
    source.close()
    restored.close()


def test_http_snapshot_store_plugs_into_provider_health_restart_for_every_domain(tmp_path) -> None:
    remote: str | None = None

    def opener(request, _timeout):
        nonlocal remote
        if request.get_method() == "GET":
            return _Response(remote.encode("utf-8")) if remote is not None else _Response(status=404)
        current = None if remote is None else json.loads(remote)["snapshot_digest"]
        expected = _header(request, "If-Match")
        if _header(request, "If-None-Match") == "*" and current is not None:
            return _Response(status=412)
        if expected is not None and current != expected.strip('"'):
            return _Response(status=412)
        remote = request.data.decode("utf-8")
        return _Response(status=204)

    text_store = AutonomousHttpSnapshotTextStore(
        "https://state.test/snapshots",
        "all-domains/provider-health",
        allowed_hosts=("state.test",),
        opener=opener,
    )
    persistence = TransactionalJsonProviderHealthSnapshotPersistence(text_store)
    domains = AUTONOMOUS_DOMAINS
    source = ProviderHealthLedger(tmp_path / "source-provider-health.jsonl", max_records=32)
    for index, domain in enumerate(domains):
        source.record({
            "schema": PROVIDER_OBSERVATION_SCHEMA,
            "provider": "offline",
            "model": f"model-{domain}",
            "status": "completed",
            "outcome": "success",
            "latency_ms": index + 1,
            "observed_at": index + 1,
        })
    flushed = ProviderHealthPersistenceCoordinator(source, persistence).flush()
    restored = ProviderHealthLedger(tmp_path / "restored-provider-health.jsonl", max_records=32)
    restored_snapshot = ProviderHealthPersistenceCoordinator(restored, persistence).restore()
    assert restored_snapshot is not None
    assert restored_snapshot["snapshot_digest"] == flushed["snapshot_digest"]
    assert {row["model"] for row in restored.records()} == {f"model-{domain}" for domain in domains}
