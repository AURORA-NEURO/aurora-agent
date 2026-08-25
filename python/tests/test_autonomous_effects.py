from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    AutonomousEffectBoundary,
    AutonomousEffectReconciliationRequiredError,
    AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA,
    AutonomousProviderEffectResolver,
    AutonomousProviderEffectReconciliationWorker,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    InMemoryAutonomousEffectJournal,
    InMemoryAutonomousEffectSnapshotTextStore,
    ProviderToolCall,
    TransactionalJsonAutonomousEffectSnapshotPersistence,
    AutonomousEffectPersistenceCoordinator,
)


def _request(execution_id: str | None = None) -> dict[str, object]:
    return {
        "execution_id": execution_id,
        "tool": "external_write",
        "call_id": "effect-call-1",
        "risk_class": "external_effect",
        "arguments": {"note": "private-value"},
    }


def test_effect_boundary_is_idempotent_and_metadata_only() -> None:
    journal = InMemoryAutonomousEffectJournal(clock=lambda: 10)
    boundary = AutonomousEffectBoundary(journal=journal)
    calls = 0

    def execute(context: object) -> dict[str, object]:
        nonlocal calls
        calls += 1
        assert context.idempotency_key.startswith("aurora-effect-")  # type: ignore[attr-defined]
        return {"accepted": True, "private_output": "not retained"}

    first = boundary.execute(_request(), execute)
    second = boundary.execute(_request(), lambda _context: {"accepted": False})

    assert second == first
    assert calls == 1
    assert [row.event.status for row in journal.events()] == ["prepared", "dispatching", "dispatched", "completed"]
    snapshot = journal.snapshot().to_dict()
    encoded = json.dumps(snapshot, sort_keys=True)
    assert "private-value" not in encoded
    assert "private_output" not in encoded
    restored = InMemoryAutonomousEffectJournal()
    restored.restore(snapshot)
    assert restored.get(boundary.effect_id(_request())).status == "completed"  # type: ignore[union-attr]
    assert restored.verify_integrity()["verified"] is True


def test_uncertain_effect_requires_resolution_before_retry(tmp_path) -> None:
    effect_journal = InMemoryAutonomousEffectJournal(clock=lambda: 20)
    execution_journal = AutonomousExecutionJournal(tmp_path / "execution.jsonl")
    policy = AutonomousExecutionPolicy(allow_side_effects=True, max_effectful_calls=2, max_tool_calls=4, max_steps=16)
    first_execution = AutonomousExecutionController(
        execution_id="effect-execution-1",
        domain="operations",
        capability="incident_response",
        risk_class="external_effect",
        policy=policy,
        journal=execution_journal,
    )
    first_execution.admit_tool_call(tool="external_write", call_id="effect-call-1", read_only=False, approval_required=True)
    request = _request(first_execution.state.execution_id)
    boundary = AutonomousEffectBoundary(journal=effect_journal, execution=first_execution)

    with pytest.raises(AutonomousEffectReconciliationRequiredError):
        boundary.execute(request, lambda _context: (_ for _ in ()).throw(RuntimeError("transport lost")))
    assert first_execution.state.status == "reconciliation_required"

    class Resolver:
        def resolve(self, record: object) -> dict[str, object]:
            assert record.effect_id == boundary.effect_id(request)  # type: ignore[attr-defined]
            return {"status": "completed", "result": {"confirmed": True}}

    resumed = AutonomousExecutionController(
        execution_id="effect-execution-1",
        domain="operations",
        capability="incident_response",
        risk_class="external_effect",
        policy=policy,
        journal=execution_journal,
        resume=True,
    )
    recovered = AutonomousEffectBoundary(journal=effect_journal, resolver=Resolver(), execution=resumed)
    result = recovered.execute(request, lambda _context: (_ for _ in ()).throw(RuntimeError("duplicate dispatch")))

    assert result == {"confirmed": True}
    assert resumed.state.status == "running"
    assert effect_journal.get(boundary.effect_id(request)).status == "reconciled"  # type: ignore[union-attr]


def test_provider_effect_resolver_uses_explicit_key_without_persisting_it_or_fabricating_a_response() -> None:
    journal = InMemoryAutonomousEffectJournal()
    boundary = AutonomousEffectBoundary(journal=journal)
    request = {
        "tool": "provider.offline.invoke",
        "call_id": "provider-call-1",
        "risk_class": "provider_invocation",
        "arguments": {"request_digest": "a" * 64},
    }
    with pytest.raises(AutonomousEffectReconciliationRequiredError):
        boundary.execute(request, lambda _context: (_ for _ in ()).throw(RuntimeError("lost")), cache_result=False)
    observed: dict[str, object] = {}

    def lookup(provider: str, operation: str, key: str, record: object) -> dict[str, object]:
        observed.update(provider=provider, operation=operation, key=key, has_arguments=hasattr(record, "arguments"))
        return {"status": "completed", "result": {"status_code": 200, "event_count": 2}}

    resolver = AutonomousProviderEffectResolver(lookup)
    effect_id = boundary.effect_id(request)
    record = boundary.reconcile(effect_id, resolver, idempotency_key="caller-owned-status-key")
    assert record.status == "reconciled"
    assert observed == {"provider": "offline", "operation": "invoke", "key": "caller-owned-status-key", "has_arguments": False}
    encoded = json.dumps(journal.snapshot().to_dict(), sort_keys=True)
    assert "caller-owned-status-key" not in encoded
    with pytest.raises(AutonomousEffectReconciliationRequiredError):
        boundary.execute(request, lambda _context: {"duplicate": True}, cache_result=False)


def test_provider_reconciliation_worker_recovers_pending_effects_across_every_domain() -> None:
    journal = InMemoryAutonomousEffectJournal()
    boundary = AutonomousEffectBoundary(journal=journal)
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        request = {
            "execution_id": domain,
            "tool": "provider.offline.invoke",
            "call_id": f"provider-{domain}",
            "risk_class": "provider_invocation",
            "arguments": {"request_digest": "b" * 64},
        }
        with pytest.raises(AutonomousEffectReconciliationRequiredError):
            boundary.execute(request, lambda _context: (_ for _ in ()).throw(RuntimeError("lost")), cache_result=False)

    seen: list[tuple[str, str, str]] = []

    def lookup(provider: str, operation: str, key: str, record: object) -> dict[str, object]:
        seen.append((provider, operation, key))
        return {"status": "not_found", "retry_safe": True}

    worker = AutonomousProviderEffectReconciliationWorker(
        boundary,
        AutonomousProviderEffectResolver(lookup),
        key_resolver=lambda record: f"status-key-{record.effect_id}",
        maximum_records=32,
    )
    report = worker.run_once()
    assert report["schema"] == AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA
    assert report["inspected"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["retry_ready"] == len(AUTONOMOUS_DOMAIN_NAMES)
    assert report["uncertain"] == 0
    assert all(operation == "invoke" and provider == "offline" for provider, operation, _key in seen)
    assert len({key for _provider, _operation, key in seen}) == len(AUTONOMOUS_DOMAIN_NAMES)
    encoded = json.dumps(journal.snapshot().to_dict(), sort_keys=True)
    assert "status-key-" not in encoded
    second_report = worker.run_once()
    assert second_report["inspected"] == 0


def test_effect_snapshot_persistence_is_canonical_and_cas_fenced() -> None:
    source = InMemoryAutonomousEffectJournal(clock=lambda: 1)
    boundary = AutonomousEffectBoundary(journal=source)
    boundary.execute({"tool": "external_write", "call_id": "persist-1", "risk_class": "external_effect", "arguments": {}}, lambda _context: {"ok": True})
    text = InMemoryAutonomousEffectSnapshotTextStore()
    persistence = TransactionalJsonAutonomousEffectSnapshotPersistence(text)
    coordinator = AutonomousEffectPersistenceCoordinator(source, persistence)
    first = coordinator.flush()
    assert json.loads(text.read() or "{}")["snapshot_digest"] == first.snapshot_digest
    assert coordinator.restore().snapshot_digest == first.snapshot_digest  # type: ignore[union-attr]
    assert text.read() is not None

    stale = AutonomousEffectPersistenceCoordinator(InMemoryAutonomousEffectJournal(), persistence)
    with pytest.raises(Exception, match="compare-and-set"):
        stale.flush()
    text.value = "{not-json"
    with pytest.raises(Exception, match="invalid"):
        persistence.read()


def test_effect_boundary_is_shared_by_every_builtin_domain() -> None:
    executions: list[str] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        tool = AutonomousDomainTool(
            name=f"{domain}_external_write",
            domains=(domain,),
            capability="external_change",
            description=f"Apply a reviewed {domain} change.",
            parameters={"type": "object", "additionalProperties": False},
            risk_class="external_effect",
            read_only=False,
            approval_required=True,
        )
        registry = AutonomousDomainToolRegistry([tool])
        boundary = AutonomousEffectBoundary(journal=InMemoryAutonomousEffectJournal())
        runtime = AutonomousDomainToolRuntime(
            registry,
            executor=lambda _tool, _arguments: {"legacy": True},
            approve=lambda _tool, _call: True,
            effect_boundary=boundary,
            effect_executor=lambda resolved, _arguments, context: executions.append(resolved.name) or {"domain": domain, "committed": True},
        )
        result = runtime((ProviderToolCall(f"call-{domain}", tool.name, {}),))
        assert result[0].approved is True, domain
        assert result[0].is_error is False, domain
        assert runtime.receipts[-1].status == "executed", domain
        assert boundary.journal.events()[-1].event.status == "completed", domain
        assert "committed" not in json.dumps(boundary.journal.snapshot().to_dict())
    assert len(executions) == len(AUTONOMOUS_DOMAIN_NAMES)
