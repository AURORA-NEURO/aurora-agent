from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_AUTHORIZATION_OPERATIONS,
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAuthorizationContext,
    AutonomousAuthorizationLedger,
    AutonomousAuthorizationGate,
    AutonomousAuthorizationPersistenceCoordinator,
    AutonomousAuthorizationRequest,
    JsonAutonomousAuthorizationSnapshotPersistence,
    TransactionalJsonAutonomousAuthorizationSnapshotPersistence,
    validate_autonomous_authorization_snapshot,
)
from prism_sdk.errors import ArgumentError


class _TextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected: str | None, value: str) -> bool:
        current = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if current != expected:
            return False
        self.value = value
        return True


def _ledger() -> AutonomousAuthorizationLedger:
    return AutonomousAuthorizationLedger(max_grants=16, max_events=64)


def _grant(ledger: AutonomousAuthorizationLedger, *, max_uses: int | None = 2, grant_id: str = "grant-1"):
    return ledger.issue(
        grant_id=grant_id,
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=AUTONOMOUS_DOMAIN_NAMES,
        allowed_operations=("provider_invocation", "tool_execution"),
        allowed_capabilities=("analysis",),
        allowed_risk_classes=("read_only",),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=max_uses,
    )


def _request(*, request_id: str = "request-1", grant_id: str = "grant-1", domain: str = "coding", tenant_id: str = "tenant-a", capability: str | None = "analysis") -> AutonomousAuthorizationRequest:
    return AutonomousAuthorizationRequest.create(
        request_id=request_id,
        grant_id=grant_id,
        tenant_id=tenant_id,
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        domains=(domain,),
        operation="provider_invocation",
        capability=capability,
        risk_class="read_only",
        issued_at=1_100,
    )


def test_authorization_is_scoped_idempotent_and_bounded() -> None:
    ledger = _ledger()
    grant = _grant(ledger)
    first = ledger.authorize(_request(), now=1_200)
    assert first.status == "allowed"
    assert first.grant_digest == grant.grant_digest
    assert first.remaining_uses == 1
    assert ledger.authorize(_request(), now=1_201).status == "already_allowed"
    assert ledger.authorize(_request(request_id="request-2"), now=1_202).status == "allowed"
    assert ledger.authorize(_request(request_id="request-3"), now=1_203).status == "exhausted"
    assert len(ledger.events()) == 4
    assert ledger.verify_integrity()["domain_coverage"]["coding"] == 1


def test_authorization_context_mints_fresh_domain_bound_provider_requests() -> None:
    ledger = _ledger()
    grant = ledger.issue(
        grant_id="provider-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=("coding",),
        allowed_operations=("provider_invocation",),
        allowed_risk_classes=(),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=2,
    )
    context = AutonomousAuthorizationContext(
        gate=AutonomousAuthorizationGate(ledger),
        grant_id=grant.grant_id,
        tenant_id=grant.tenant_id,
        actor_id=grant.actor_id,
        session_id=grant.session_id,
        authorization_digest=grant.authorization_digest,
        domains=("coding",),
        clock=lambda: 1_200,
    )

    first = context.authorize_provider(provider="offline", model="model-a", invocation_kind="provider_call")
    second = context.authorize_provider(provider="offline", model="model-a", invocation_kind="provider_call", turn=1)

    assert first.status == "allowed"
    assert second.status == "allowed"
    assert first.request_digest != second.request_digest
    assert ledger.get(grant.grant_id).used_count == 2  # type: ignore[union-attr]
    with pytest.raises(ArgumentError, match="outside its context scope"):
        context.authorize_provider(provider="offline", model="model-a", invocation_kind="provider_call", domain="science")


def test_authorization_context_shares_request_sequence_across_operation_and_domain_children() -> None:
    ledger = _ledger()
    grant = ledger.issue(
        grant_id="operation-grant",
        tenant_id="tenant-a",
        actor_id="actor-a",
        session_id="session-a",
        authorization_digest="a" * 64,
        allowed_domains=AUTONOMOUS_DOMAIN_NAMES,
        allowed_operations=("provider_invocation", "connector_dispatch", "tool_execution", "effect_dispatch"),
        allowed_risk_classes=(),
        issued_at=1_000,
        expires_at=2_000,
        max_uses=4,
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

    first = context.authorize_provider(provider="offline", model="model-a", invocation_kind="provider_call", domain="coding")
    child = context.for_domain("coding")
    second = child.authorize_operation(
        operation="connector_dispatch",
        domain="coding",
        capability="evidence_read",
        resource_digest="b" * 64,
    )
    third = context.authorize_operation(
        operation="tool_execution",
        domain="coding",
        capability="evidence_read",
        resource_digest="c" * 64,
    )

    assert len({first.request_digest, second.request_digest, third.request_digest}) == 3
    assert context._counter == 3
    assert child._counter == 2


def test_authorization_rejects_identity_scope_and_time_drift() -> None:
    ledger = _ledger()
    _grant(ledger, max_uses=None)
    assert ledger.authorize(_request(tenant_id="tenant-b"), now=1_200).status == "tenant_mismatch"
    assert ledger.authorize(_request(domain="science"), now=1_200).status == "allowed"
    assert ledger.authorize(_request(request_id="expired"), now=2_000).status == "expired"
    ledger.revoke("grant-1", revoked_at=1_500, reason="operator-revoked")
    assert ledger.authorize(_request(request_id="revoked"), now=1_501).status == "revoked"


def test_authorization_snapshot_survives_restart_and_detects_tampering() -> None:
    ledger = _ledger()
    _grant(ledger, max_uses=None)
    ledger.authorize(_request(), now=1_200)
    snapshot = ledger.snapshot()
    assert validate_autonomous_authorization_snapshot(snapshot)["snapshot_digest"] == snapshot["snapshot_digest"]

    restarted = _ledger()
    restarted.restore(snapshot)
    assert restarted.authorize(_request(), now=1_201).status == "already_allowed"
    tampered = json.loads(json.dumps(snapshot))
    tampered["grants"][0]["tenant_id"] = "tenant-b"
    with pytest.raises(ArgumentError):
        validate_autonomous_authorization_snapshot(tampered)


def test_authorization_persistence_is_cas_fenced_and_all_domains_are_present() -> None:
    store = _TextStore()
    ledger = _ledger()
    coordinator = AutonomousAuthorizationPersistenceCoordinator(
        ledger,
        TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store),
    )
    assert coordinator.restore() is None
    _grant(ledger, max_uses=None)
    persisted = coordinator.flush()
    assert store.value is not None
    assert set(persisted["grants"][0]["allowed_domains"]) == set(AUTONOMOUS_DOMAIN_NAMES)
    assert set(ledger.get("grant-1").allowed_operations) == {"provider_invocation", "tool_execution"}
    assert len(AUTONOMOUS_AUTHORIZATION_OPERATIONS) == 12

    stale = AutonomousAuthorizationPersistenceCoordinator(
        _ledger(), TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store)
    )
    stale.restore()
    _grant(ledger, max_uses=None, grant_id="grant-live")
    coordinator.flush()
    _grant(stale.ledger, max_uses=None, grant_id="grant-2")
    with pytest.raises(ArgumentError):
        stale.flush()


def test_secret_shaped_request_metadata_is_not_accepted() -> None:
    ledger = _ledger()
    _grant(ledger)
    request = _request().to_dict()
    request["prompt"] = "must never cross the authorization boundary"
    with pytest.raises(ArgumentError):
        AutonomousAuthorizationRequest.from_dict(request)


def test_fail_closed_gate_never_calls_refused_operation() -> None:
    ledger = _ledger()
    _grant(ledger, max_uses=None)
    called = False

    def operation() -> str:
        nonlocal called
        called = True
        return "should-not-run"

    denied = _request(capability="different-capability")
    with pytest.raises(ArgumentError):
        AutonomousAuthorizationGate(ledger).execute(denied, now=1_200, operation=operation)
    assert called is False
