from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor
import json
import sqlite3

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationLedger,
    AutonomousDomainTool,
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    AutonomousEffectBoundary,
    AutonomousEffectPolicyError,
    AutonomousEffectReconciliationRequiredError,
    AutonomousProtectedProviderEffectResolver,
    AutonomousProtectedRehydrationAdapter,
    AutonomousProtectedRehydrationBoundary,
    AutonomousProtectedRehydrationContext,
    AUTONOMOUS_PROTECTED_PROVIDER_EFFECT_REHYDRATION_SCHEMA,
    AUTONOMOUS_PROVIDER_EFFECT_RECONCILIATION_SCHEMA,
    AutonomousProviderEffectResolver,
    AutonomousProviderEffectReconciliationWorker,
    AutonomousProviderEffectReconciliationCoordinator,
    AutonomousExecutionController,
    AutonomousExecutionJournal,
    AutonomousExecutionPolicy,
    InMemoryAutonomousEffectJournal,
    SQLiteAutonomousEffectJournal,
    InMemoryAutonomousEffectSnapshotTextStore,
    ProviderToolCall,
    TransactionalJsonAutonomousEffectSnapshotPersistence,
    AutonomousEffectPersistenceCoordinator,
    protected_value_digest,
)
from prism_sdk.errors import ArgumentError


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


def test_effect_boundary_requires_context_before_external_dispatch_for_all_domains() -> None:
    ledger = AutonomousAuthorizationLedger(max_grants=4, max_events=64)
    grant = ledger.issue(
        grant_id="effect-runtime-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=AUTONOMOUS_DOMAIN_NAMES,
        allowed_operations=("effect_dispatch",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=len(AUTONOMOUS_DOMAIN_NAMES),
    )
    context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=grant.authorization_digest,
        domains=AUTONOMOUS_DOMAIN_NAMES,
        clock=lambda: 1_200,
    )
    journal = InMemoryAutonomousEffectJournal()
    boundary = AutonomousEffectBoundary(journal=journal)
    executed: list[str] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        result = boundary.execute(
            {"execution_id": f"effect-{domain}", "tool": "external_write", "call_id": f"call-{domain}", "risk_class": "external_effect", "arguments": {}},
            lambda _effect_context, domain=domain: executed.append(domain) or {"domain": domain},
            authorization_context=context,
            authorization_domains=(domain,),
        )
        assert result == {"domain": domain}
    assert executed == list(AUTONOMOUS_DOMAIN_NAMES)
    assert ledger.get(grant.grant_id).used_count == len(AUTONOMOUS_DOMAIN_NAMES)  # type: ignore[union-attr]

    blocked_ledger = AutonomousAuthorizationLedger(max_grants=2, max_events=8)
    blocked = blocked_ledger.issue(
        grant_id="blocked-effect-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=("coding",),
        allowed_operations=("tool_execution",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=1,
    )
    blocked_context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(blocked_ledger),
        grant_id=blocked.grant_id,
        tenant_id=blocked.tenant_id,
        actor_id=blocked.actor_id,
        session_id=blocked.session_id,
        authorization_digest=blocked.authorization_digest,
        domains=("coding",),
        clock=lambda: 1_200,
    )
    blocked_calls = 0
    with pytest.raises(ArgumentError, match="authorization was refused"):
        boundary.execute(
            {"execution_id": "effect-blocked", "tool": "external_write", "call_id": "call-blocked", "risk_class": "external_effect", "arguments": {}},
            lambda _context: (_ for _ in ()).throw(AssertionError("effect executor must not run")),
            authorization_context=blocked_context,
            authorization_domains=("coding",),
        )
    assert blocked_calls == 0


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


def test_protected_provider_effect_receipts_bind_every_effect_identity_across_all_domains_and_keep_keys_transient() -> None:
    assert AUTONOMOUS_PROTECTED_PROVIDER_EFFECT_REHYDRATION_SCHEMA == "bioprism-python-autonomous-protected-provider-effect-rehydration/0.1"
    values: dict[str, object] = {}
    protected_boundary = AutonomousProtectedRehydrationBoundary(
        AutonomousProtectedRehydrationContext(tenant_id="tenant-effects", actor_id="effect-worker", session_id="protected", authorization_digest="e" * 64),
        lambda reference, _context: values[reference.value_digest],
        authorizer=lambda _reference, _context: True,
        clock=lambda: 500,
    )
    adapter = AutonomousProtectedRehydrationAdapter(protected_boundary)
    observed_keys: list[str] = []
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        journal = InMemoryAutonomousEffectJournal()
        boundary = AutonomousEffectBoundary(journal=journal)
        request = {"execution_id": domain, "tool": "provider.offline.invoke", "call_id": f"protected-provider-{domain}", "risk_class": "provider_invocation", "arguments": {"request_digest": "c" * 64}}
        with pytest.raises(AutonomousEffectReconciliationRequiredError):
            boundary.execute(request, lambda _context: (_ for _ in ()).throw(RuntimeError("lost_after_dispatch")), cache_result=False)
        effect_id = boundary.effect_id(request)
        record = journal.get(effect_id)
        value = {"status": "completed", "result": {"status_code": 200, "domain": domain}}
        value_digest = protected_value_digest(value)
        values[value_digest] = value

        def receipt_resolver(context: object, *, _domain: str = domain, _value_digest: str = value_digest) -> dict[str, object]:
            observed_keys.append(context.idempotency_key)  # type: ignore[attr-defined]
            return {
                "effect_id": context.effect_id, "execution_id": context.execution_id, "tool": context.tool,
                "call_id": context.call_id, "risk_class": context.risk_class, "arguments_digest": context.arguments_digest,
                "idempotency_key_digest": context.idempotency_key_digest, "dispatch_attempt": context.dispatch_attempt,
                "provider": context.provider, "operation": context.operation, "domain": _domain, "value_digest": _value_digest,
            }

        resolver = AutonomousProtectedProviderEffectResolver(adapter, receipt_resolver, domain=domain)
        updated = boundary.reconcile(effect_id, resolver, idempotency_key=f"protected-status-{domain}")
        assert updated.status == "reconciled", domain
        assert boundary.execute(request, lambda _context: {"duplicate": True}, cache_result=True) == value["result"]
        assert f"protected-status-{domain}" not in json.dumps(journal.snapshot().to_dict(), sort_keys=True)
        assert record is not None
    assert observed_keys == [f"protected-status-{domain}" for domain in AUTONOMOUS_DOMAIN_NAMES]

    tamper_journal = InMemoryAutonomousEffectJournal()
    tamper_boundary = AutonomousEffectBoundary(journal=tamper_journal)
    tamper_request = {"execution_id": "coding", "tool": "provider.offline.invoke", "call_id": "protected-tamper", "risk_class": "provider_invocation", "arguments": {"request_digest": "d" * 64}}
    with pytest.raises(AutonomousEffectReconciliationRequiredError):
        tamper_boundary.execute(tamper_request, lambda _context: (_ for _ in ()).throw(RuntimeError("lost_after_dispatch")), cache_result=False)
    tamper_id = tamper_boundary.effect_id(tamper_request)
    tamper_value = {"status": "completed", "result": {"confirmed": True}}
    tamper_digest = protected_value_digest(tamper_value)
    values[tamper_digest] = tamper_value
    tampered_resolver = AutonomousProtectedProviderEffectResolver(
        adapter,
        lambda context: {
            "effect_id": "0" * 64, "execution_id": context.execution_id, "tool": context.tool,
            "call_id": context.call_id, "risk_class": context.risk_class, "arguments_digest": context.arguments_digest,
            "idempotency_key_digest": context.idempotency_key_digest, "dispatch_attempt": context.dispatch_attempt,
            "provider": context.provider, "operation": context.operation, "domain": "coding", "value_digest": tamper_digest,
        },
        domain="coding",
    )
    with pytest.raises(AutonomousEffectPolicyError, match="effect_id"):
        tamper_boundary.reconcile(tamper_id, tampered_resolver)
    assert tamper_journal.get(tamper_id).status == "uncertain"  # type: ignore[union-attr]


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


def test_provider_reconciliation_admission_is_cached_blocks_uncertainty_and_reopens_explicitly() -> None:
    journal = InMemoryAutonomousEffectJournal()
    boundary = AutonomousEffectBoundary(journal=journal)
    with pytest.raises(AutonomousEffectReconciliationRequiredError):
        boundary.execute(
            {"execution_id": "admission", "tool": "provider.offline.invoke", "call_id": "uncertain", "risk_class": "provider_invocation", "arguments": {}},
            lambda _context: (_ for _ in ()).throw(RuntimeError("lost")),
            cache_result=False,
        )
    lookups = 0

    def lookup(_provider: str, _operation: str, _key: str, _record: object) -> dict[str, object]:
        nonlocal lookups
        lookups += 1
        return {"status": "unknown"}

    coordinator = AutonomousProviderEffectReconciliationCoordinator(
        AutonomousProviderEffectReconciliationWorker(boundary, AutonomousProviderEffectResolver(lookup))
    )
    blocked = coordinator.admit()
    assert blocked["status"] == "blocked"
    assert blocked["reason"] == "uncertain_effect_state"
    assert lookups == 1
    assert coordinator.admit() == blocked
    assert lookups == 1
    coordinator.reset()
    reopened = coordinator.admit()
    assert reopened["status"] == "blocked"
    assert lookups == 2
    encoded = json.dumps(reopened, sort_keys=True)
    assert "never_returned" in encoded
    assert "lost" not in encoded


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


def test_sqlite_effect_journal_preserves_chain_across_restart_and_concurrent_appenders(tmp_path) -> None:
    path = tmp_path / "effects.sqlite3"
    writers = [SQLiteAutonomousEffectJournal(path, clock=lambda: 42) for _ in range(4)]
    events = [
        {
            "schema": "bioprism-python-autonomous-effect-event/0.1",
            "effect_id": f"sqlite-effect-{index}",
            "execution_id": "sqlite-execution",
            "tool": "external_write",
            "call_id": f"sqlite-call-{index}",
            "risk_class": "external_effect",
            "arguments_digest": "a" * 64,
            "idempotency_key_digest": "b" * 64,
            "status": "prepared",
            "dispatch_attempt": 1,
            "reason": None,
            "retention": "metadata_only_no_arguments_outputs_credentials_or_provider_material",
        }
        for index in range(32)
    ]

    def append(index: int):
        return writers[index % len(writers)].append(events[index])

    try:
        with ThreadPoolExecutor(max_workers=len(writers)) as pool:
            receipts = list(pool.map(append, range(len(events))))
        assert sorted(receipt.sequence for receipt in receipts) == list(range(1, len(events) + 1))
        assert len({receipt.event_digest for receipt in receipts}) == len(events)
        snapshot = writers[0].snapshot()
        assert snapshot.head_digest == receipts[max(range(len(receipts)), key=lambda index: receipts[index].sequence)].head_digest
        assert writers[0].verify_integrity()["verified"] is True
    finally:
        for writer in writers:
            writer.close()

    with SQLiteAutonomousEffectJournal(path) as reopened:
        assert reopened.get("sqlite-effect-17").status == "prepared"  # type: ignore[union-attr]
        assert len(reopened.events(limit=64)) == len(events)
        assert reopened.snapshot().snapshot_digest == snapshot.snapshot_digest


def test_sqlite_effect_journal_rejects_tampered_event_storage(tmp_path) -> None:
    path = tmp_path / "tampered-effects.sqlite3"
    with SQLiteAutonomousEffectJournal(path, clock=lambda: 7) as journal:
        journal.append(
            {
                "schema": "bioprism-python-autonomous-effect-event/0.1",
                "effect_id": "tampered-effect",
                "execution_id": None,
                "tool": "external_write",
                "call_id": "tampered-call",
                "risk_class": "external_effect",
                "arguments_digest": "c" * 64,
                "idempotency_key_digest": "d" * 64,
                "status": "dispatched",
                "dispatch_attempt": 1,
                "retention": "metadata_only_no_arguments_outputs_credentials_or_provider_material",
            }
        )
    with sqlite3.connect(path) as connection:
        connection.execute(
            "UPDATE autonomous_effect_journal_events SET event_digest = ? WHERE sequence = 1",
            ("e" * 64,),
        )
    with SQLiteAutonomousEffectJournal(path) as reopened:
        with pytest.raises(Exception, match="digest"):
            reopened.verify_integrity()


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
