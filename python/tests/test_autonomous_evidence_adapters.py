from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousEvidenceAcquisitionError,
    AutonomousEvidenceAdapterHealthController,
    AutonomousEvidenceAdapterHealthPersistenceCoordinator,
    AutonomousEvidenceAdapterRegistry,
    AutonomousEvidenceAdapterSelector,
    AutonomousEvidenceAdapterFailoverAcquirer,
    AutonomousEvidenceFailoverPolicy,
    AutonomousEvidenceRetryPolicy,
    InMemoryAutonomousEvidenceAdapterHealthStore,
    JsonAutonomousEvidenceAdapterHealthPersistence,
    TransactionalJsonAutonomousEvidenceAdapterHealthPersistence,
    ArgumentError,
    content_digest,
    register_autonomous_evidence_adapters_for_all_domains,
    validate_autonomous_evidence_adapter_health_snapshot,
)


def _context(domain: str) -> dict[str, object]:
    return {"requirement": {"domain": domain}, "request": {"source_digest": None}}


def _registry(*, calls: list[str] | None = None) -> AutonomousEvidenceAdapterRegistry:
    target = calls if calls is not None else []
    registry = AutonomousEvidenceAdapterRegistry()

    def factory(domain: str) -> dict[str, object]:
        adapter_id = f"source_{domain}"

        def acquire(context: dict[str, object]) -> dict[str, object]:
            target.append(adapter_id)
            return {"domain": domain, "adapter": adapter_id}

        return {
            "adapter_id": adapter_id,
            "version": "1",
            "capabilities": ("evidence", "review"),
            "source_kinds": ("fixture",),
            "acquire": acquire,
        }

    register_autonomous_evidence_adapters_for_all_domains(registry, factory)
    return registry


def test_generic_registry_selection_and_execution_cover_every_domain() -> None:
    calls: list[str] = []
    registry = _registry(calls=calls)
    assert len(registry.manifests()) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert all(row.state == "complete" for row in registry.coverage())
    projection = registry.to_dict()
    assert projection["registry_digest"] == registry.registry_digest
    assert "api_key" not in json.dumps(projection).lower()

    selector = AutonomousEvidenceAdapterSelector(registry)
    plan = selector.select_for_domains(AUTONOMOUS_DOMAIN_NAMES, capability="evidence")
    assert plan.complete
    assert plan.from_dict(plan.to_dict()).plan_digest == plan.plan_digest
    acquirer = selector.create_acquirer_from_selection(plan)
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        assert acquirer.acquire(_context(domain))["domain"] == domain
    assert len(calls) == len(AUTONOMOUS_DOMAIN_NAMES)


def test_weighted_selection_requires_observed_signals_and_refuses_tampering() -> None:
    registry = _registry()
    selector = AutonomousEvidenceAdapterSelector(registry)
    adapter_id = registry.resolve("coding").adapter_id
    with pytest.raises(ArgumentError):
        selector.select_for_domains((AUTONOMOUS_DOMAIN_NAMES[0],), strategy="weighted_evidence")
    plan = selector.select_for_domains(
        (AUTONOMOUS_DOMAIN_NAMES[0],),
        capability="evidence",
        strategy="weighted_evidence",
        selection_signals={adapter_id: {"eligible": True, "health": 1, "success_rate": 1, "evaluator_reward": 1}},
    )
    assert plan.complete
    tampered = plan.to_dict()
    tampered["rows"][0]["reason"] = "tampered"
    with pytest.raises(ArgumentError):
        type(plan).from_dict(tampered)


def test_health_store_is_hash_chained_metadata_only_and_cas_fenced() -> None:
    clock_value = [100.0]
    store = InMemoryAutonomousEvidenceAdapterHealthStore(clock=lambda: clock_value[0])
    adapter_id = "source_coding"
    manifest_digest = "a" * 64
    for _ in range(3):
        store.record_acquisition(adapter_id=adapter_id, manifest_digest=manifest_digest, domain="coding", outcome="success", status="success", latency_ms=10, cost_units=2)
    store.record_evaluation(adapter_id=adapter_id, manifest_digest=manifest_digest, domain="coding", status="verdict_accepted", evaluator_reward=1, evaluator_passed=True, evaluator_id="eval", evaluator_version="1")
    health = store.health(domain="coding", min_attempts=1)
    assert health[0]["success_rate"] == 1
    assert health[0]["quality_observations"] == 1
    assert store.selection_signals(domain="coding", min_attempts=1)[adapter_id]["eligible"] is True
    snapshot = store.snapshot()
    assert validate_autonomous_evidence_adapter_health_snapshot(snapshot).snapshot_digest == snapshot.snapshot_digest
    encoded = json.dumps(snapshot.to_dict(), sort_keys=True, separators=(",", ":"))

    class TextStore:
        value: str | None = None

        def read(self) -> str | None:
            return self.value

        def write(self, value: str) -> None:
            self.value = value

    text_store = TextStore()
    persistence = JsonAutonomousEvidenceAdapterHealthPersistence(text_store)
    coordinator = AutonomousEvidenceAdapterHealthPersistenceCoordinator(store, persistence)
    coordinator.flush()
    assert text_store.value is not None
    assert json.loads(text_store.value)["snapshot_digest"] == snapshot.snapshot_digest
    restored = InMemoryAutonomousEvidenceAdapterHealthStore()
    restored_coordinator = AutonomousEvidenceAdapterHealthPersistenceCoordinator(restored, persistence)
    assert restored_coordinator.restore()["verified"] is True
    assert restored.snapshot().snapshot_digest == snapshot.snapshot_digest
    assert encoded.find("source_coding") >= 0

    class CasTextStore(TextStore):
        def write_if_unchanged(self, expected: str | None, value: str) -> bool:
            if expected is not None and self.value is not None and json.loads(self.value)["snapshot_digest"] != expected:
                return False
            if expected is None and self.value is not None:
                return False
            self.value = value
            return True

    cas_store = CasTextStore()
    cas_persistence = TransactionalJsonAutonomousEvidenceAdapterHealthPersistence(cas_store)
    cas_coordinator = AutonomousEvidenceAdapterHealthPersistenceCoordinator(InMemoryAutonomousEvidenceAdapterHealthStore(), cas_persistence)
    cas_coordinator.flush()
    assert cas_coordinator.flush().snapshot_digest


def test_health_and_registration_reject_secret_shaped_metadata_and_open_circuits() -> None:
    registry = AutonomousEvidenceAdapterRegistry()
    registry.register(adapter_id="safe", version="1", domains=("coding",), capabilities=("evidence",), source_kinds=("fixture",), acquire=lambda _context: {"api_key": "caller-owned"})
    store = InMemoryAutonomousEvidenceAdapterHealthStore()
    for _ in range(3):
        store.record_acquisition(adapter_id="source_coding", manifest_digest="b" * 64, domain="coding", outcome="failure", status="failure", latency_ms=1, failure_class="transport_error")
    assert store.health(domain="coding")[0]["circuit"] == "open"
    with pytest.raises(ArgumentError):
        store.record({"schema": "bioprism-python-autonomous-evidence-adapter-health-observation/0.1", "adapter_id": "source_coding", "manifest_digest": "b" * 64, "domain": "coding", "observation_kind": "acquisition", "outcome": "success", "status": "success", "latency_ms": 1, "api_key": "never", "retention": "metadata_only;raw_source_values_credentials_prompts_and_errors_never_persisted", "secret_material": "never_returned"})


def test_health_controller_adapts_all_domains_and_failover_is_reviewed() -> None:
    calls: list[str] = []
    registry = AutonomousEvidenceAdapterRegistry()

    def flaky(context: dict[str, object]) -> object:
        calls.append("flaky")
        raise AutonomousEvidenceAcquisitionError("transport_error", True)

    def healthy(context: dict[str, object]) -> object:
        calls.append("healthy")
        return {"ok": True}

    registry.register(adapter_id="a_flaky", version="1", domains=AUTONOMOUS_DOMAIN_NAMES, capabilities=("evidence",), source_kinds=("fixture",), acquire=flaky)
    registry.register(adapter_id="b_healthy", version="1", domains=AUTONOMOUS_DOMAIN_NAMES, capabilities=("evidence",), source_kinds=("fixture",), acquire=healthy)
    health = InMemoryAutonomousEvidenceAdapterHealthStore()
    for _ in range(3):
        health.record_acquisition(adapter_id="a_flaky", manifest_digest=registry.resolve("coding", "a_flaky").manifest_digest, domain="coding", outcome="failure", status="failure", latency_ms=1, failure_class="transport_error")
        health.record_acquisition(adapter_id="b_healthy", manifest_digest=registry.resolve("coding", "b_healthy").manifest_digest, domain="coding", outcome="success", status="success", latency_ms=1)
    controller = AutonomousEvidenceAdapterHealthController(health, registry)
    plan = controller.select_adaptive_for_domains(AUTONOMOUS_DOMAIN_NAMES, capability="evidence", min_attempts=1)
    assert plan.rows[0].adapter_id == "b_healthy"
    assert plan.rows[-1].domain == AUTONOMOUS_DOMAIN_NAMES[-1]

    selector = AutonomousEvidenceAdapterSelector(registry)
    static_plan = selector.select_for_domains(("coding",), capability="evidence")
    events: list[dict[str, object]] = []
    failover = AutonomousEvidenceAdapterFailoverAcquirer(
        registry,
        static_plan,
        policy=AutonomousEvidenceFailoverPolicy(max_failovers=1, retry_policy=AutonomousEvidenceRetryPolicy(max_attempts=1, base_delay_ms=0, max_delay_ms=0)),
        observe_failover=lambda event: events.append(event.to_dict()),
        sleep=lambda _delay: None,
    )
    assert failover.acquire(_context("coding")) == {"ok": True}
    assert calls == ["flaky", "healthy"]
    assert [event["status"] for event in events] == ["candidate_failed", "fallback_started", "candidate_succeeded"]
    assert all("raw" not in json.dumps(event).lower() for event in events)

    class Evaluator:
        evaluator_id = "fixture-evaluator"
        evaluator_version = "1"

        def evaluate(self, input_value: dict[str, object]) -> dict[str, object]:
            return {"verdict": "accepted", "score": 1.0}

    observed_evaluator = controller.create_observed_evaluator(Evaluator(), {domain: "b_healthy" for domain in AUTONOMOUS_DOMAIN_NAMES})
    assert observed_evaluator.evaluate(_context("coding"))["verdict"] == "accepted"
    healthy_health = next(row for row in health.health(domain="coding", min_attempts=1) if row["adapter_id"] == "b_healthy")
    assert healthy_health["quality_observations"] == 1
