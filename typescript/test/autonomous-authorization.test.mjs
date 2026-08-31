import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_AUTHORIZATION_OPERATIONS,
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAuthorizationContext,
  AutonomousAuthorizationError,
  AutonomousAuthorizationGate,
  AutonomousAuthorizationLedger,
  AutonomousAuthorizationPersistenceCoordinator,
  AutonomousAuthorizationRequest,
  JsonAutonomousAuthorizationSnapshotPersistence,
  TransactionalJsonAutonomousAuthorizationSnapshotPersistence,
  validateAutonomousAuthorizationSnapshot,
} from "../dist/index.js";

const digest = (letter) => letter.repeat(64);

class TextStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expected, value) {
    const current = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (current !== expected) return false;
    this.value = value;
    return true;
  }
}

function grant(ledger, grantId = "grant-1", maxUses = 2) {
  return ledger.issue({
    grant_id: grantId,
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: digest("a"),
    allowed_domains: [...AUTONOMOUS_DOMAIN_NAMES],
    allowed_operations: ["provider_invocation", "tool_execution"],
    allowed_capabilities: ["analysis"],
    allowed_risk_classes: ["read_only"],
    issued_at: 1000,
    expires_at: 2000,
    max_uses: maxUses,
  });
}

function request(requestId = "request-1", grantId = "grant-1", tenantId = "tenant-a", domain = "coding", capability = "analysis") {
  return AutonomousAuthorizationRequest.create({
    request_id: requestId,
    grant_id: grantId,
    tenant_id: tenantId,
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: digest("a"),
    domains: [domain],
    operation: "provider_invocation",
    capability,
    risk_class: "read_only",
    issued_at: 1100,
  });
}

test("authorization scopes all domains and makes allowed requests idempotent", () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  const issued = grant(ledger);
  const first = ledger.authorize(request(), 1200);
  assert.equal(first.status, "allowed");
  assert.equal(first.grant_digest, issued.grant_digest);
  assert.equal(first.remaining_uses, 1);
  assert.equal(ledger.authorize(request(), 1201).status, "already_allowed");
  assert.equal(ledger.authorize(request("request-2"), 1202).status, "allowed");
  assert.equal(ledger.authorize(request("request-3"), 1203).status, "exhausted");
  assert.equal(ledger.verifyIntegrity().domain_coverage.coding, 1);
  assert.equal(ledger.grants()[0].allowed_domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(AUTONOMOUS_AUTHORIZATION_OPERATIONS.length, 12);
});

test("authorization refuses tenant drift, expiry, and revocation", () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  grant(ledger, "grant-1", null);
  assert.equal(ledger.authorize(request("tenant", "grant-1", "tenant-b"), 1200).status, "tenant_mismatch");
  assert.equal(ledger.authorize(request("expired"), 2000).status, "expired");
  ledger.revoke("grant-1", 1500, "operator-revoked");
  assert.equal(ledger.authorize(request("revoked"), 1501).status, "revoked");
});

test("authorization snapshot restores and rejects tampering", () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  grant(ledger, "grant-1", null);
  ledger.authorize(request(), 1200);
  const snapshot = ledger.snapshot();
  assert.equal(validateAutonomousAuthorizationSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);
  const restarted = new AutonomousAuthorizationLedger(16, 64);
  restarted.restore(snapshot);
  assert.equal(restarted.authorize(request(), 1201).status, "already_allowed");
  const tampered = structuredClone(snapshot);
  tampered.grants[0].tenant_id = "tenant-b";
  assert.throws(() => validateAutonomousAuthorizationSnapshot(tampered), AutonomousAuthorizationError);
});

test("authorization persistence uses restore-before-read and CAS fencing", () => {
  const store = new TextStore();
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  const coordinator = new AutonomousAuthorizationPersistenceCoordinator(ledger, new TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store));
  assert.equal(coordinator.restore(), null);
  grant(ledger, "grant-1", null);
  const first = coordinator.flush();
  assert.equal(new Set(first.grants[0].allowed_domains).size, AUTONOMOUS_DOMAIN_NAMES.length);

  const stale = new AutonomousAuthorizationPersistenceCoordinator(new AutonomousAuthorizationLedger(16, 64), new TransactionalJsonAutonomousAuthorizationSnapshotPersistence(store));
  stale.restore();
  grant(ledger, "grant-live", null);
  coordinator.flush();
  grant(stale.ledger, "grant-stale", null);
  assert.throws(() => stale.flush(), AutonomousAuthorizationError);
});

test("authorization request rejects transient prompt-shaped metadata", () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  grant(ledger);
  const raw = request().toJSON();
  raw.prompt = "not part of authorization";
  assert.throws(() => AutonomousAuthorizationRequest.fromJSON(raw), AutonomousAuthorizationError);
  assert.ok(new JsonAutonomousAuthorizationSnapshotPersistence(new TextStore()));
});

test("fail-closed authorization gate never invokes a refused operation", async () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  grant(ledger, "grant-1", null);
  let called = false;
  await assert.rejects(
    () => new AutonomousAuthorizationGate(ledger).execute(request("denied", "grant-1", "tenant-a", "coding", "different-capability"), 1200, async () => {
      called = true;
      return "should-not-run";
    }),
    AutonomousAuthorizationError,
  );
  assert.equal(called, false);
});

test("authorization context mints fresh, domain-bound provider requests", () => {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  const issued = ledger.issue({
    grant_id: "provider-grant",
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: digest("a"),
    allowed_domains: ["coding"],
    allowed_operations: ["provider_invocation"],
    allowed_capabilities: [],
    allowed_risk_classes: [],
    issued_at: 1000,
    expires_at: 2000,
    max_uses: 2,
  });
  const context = new AutonomousAuthorizationContext(
    new AutonomousAuthorizationGate(ledger),
    issued.grant_id,
    issued.tenant_id,
    issued.actor_id,
    issued.session_id,
    issued.authorization_digest,
    ["coding"],
    null,
    "provider_invocation",
    "provider",
    () => 1200,
  );

  const first = context.authorizeProvider({ provider: "offline", model: "model-a", invocationKind: "provider_call" });
  const second = context.authorizeProvider({ provider: "offline", model: "model-a", invocationKind: "provider_call", turn: 1 });

  assert.equal(first.status, "allowed");
  assert.equal(second.status, "allowed");
  assert.notEqual(first.request_digest, second.request_digest);
  assert.equal(ledger.grants()[0].used_count, 2);
  assert.throws(
    () => context.authorizeProvider({ provider: "offline", model: "model-a", invocationKind: "provider_call", domain: "science" }),
    AutonomousAuthorizationError,
  );
});
