import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  AutonomousProtectedRehydrationError,
  AutonomousProtectedRehydrationPersistenceCoordinator,
  AutonomousMemoryConsolidationScheduler,
  AutonomousMemoryConsolidationSchedulerError,
  AutonomousMemoryConsolidator,
  JsonAutonomousProtectedRehydrationPersistence,
  TransactionalJsonAutonomousProtectedRehydrationPersistence,
  protectedValueDigest,
  validateAutonomousProtectedRehydrationSnapshot,
} from "../dist/index.js";

const authorizationDigest = "a".repeat(64);

class CasStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expected, value) {
    const observed = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (observed !== expected) return false;
    this.value = value;
    return true;
  }
}

const context = (tenantId = "tenant-a") => new AutonomousProtectedRehydrationContext({ tenantId, actorId: "actor-a", sessionId: "session-a", authorizationDigest });

test("all built-in domains share a bounded transient protected-value boundary", () => {
  const values = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(context(), (reference) => values.get(reference.reference_id), { authorizer: () => true, clock: () => 100 });
  AUTONOMOUS_DOMAIN_NAMES.forEach((domain, index) => {
    const referenceId = `reference-${index}`;
    const value = `user-provider-secret-${index}`;
    values.set(referenceId, value);
    boundary.issueForValue(referenceId, value, { domain, purpose: "provider_credential", valueKind: "credential", issuedAt: 100, expiresAt: 200 });
  });
  const snapshot = boundary.snapshot();
  assert.deepEqual(snapshot.coverage.map((row) => row.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.deepEqual(snapshot.coverage.map((row) => row.reference_count), AUTONOMOUS_DOMAIN_NAMES.map(() => 1));
  assert.equal(JSON.stringify(snapshot).includes("user-provider-secret"), false);
  const result = boundary.resolve("reference-0", { now: 101 });
  assert.equal(result.value, values.get("reference-0"));
  assert.equal(result.toJSON().value_retained, false);
  assert.equal(boundary.get("reference-0").status, "consumed");
  assert.throws(() => boundary.resolve("reference-0", { now: 101 }), AutonomousProtectedRehydrationError);
});

test("authorization, expiry, digest mismatch, and tenant restore are fenced", () => {
  const boundary = new AutonomousProtectedRehydrationBoundary(context(), () => "wrong-value", { authorizer: () => false, clock: () => 100, maxAttempts: 2 });
  boundary.issue("auth", { domain: "enterprise", purpose: "delegated_session", valueDigest: protectedValueDigest("expected"), issuedAt: 100, expiresAt: 102 });
  assert.throws(() => boundary.resolve("auth", { now: 100.5 }), AutonomousProtectedRehydrationError);
  assert.equal(boundary.get("auth").last_error_class, "authorization_denied");

  const expired = new AutonomousProtectedRehydrationBoundary(context(), () => "expected", { clock: () => 100 });
  expired.issue("expired", { domain: "operations", purpose: "session", valueDigest: protectedValueDigest("expected"), issuedAt: 100, expiresAt: 101 });
  assert.throws(() => expired.resolve("expired", { now: 101 }), AutonomousProtectedRehydrationError);
  assert.equal(expired.get("expired").status, "expired");

  const mismatch = new AutonomousProtectedRehydrationBoundary(context(), () => "wrong", { clock: () => 100 });
  mismatch.issue("mismatch", { domain: "coding", purpose: "credential", valueDigest: protectedValueDigest("expected"), issuedAt: 100, expiresAt: 200 });
  assert.throws(() => mismatch.resolve("mismatch", { now: 100.1 }), AutonomousProtectedRehydrationError);
  assert.equal(mismatch.get("mismatch").last_error_class, "value_digest_mismatch");
});

test("receipt adapter rehydrates all domains without retaining payloads", () => {
  const values = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(context(), (reference) => values.get(reference.value_digest), { authorizer: () => true, clock: () => 100 });
  const adapter = new AutonomousProtectedRehydrationAdapter(boundary);
  AUTONOMOUS_DOMAIN_NAMES.forEach((domain, index) => {
    const value = `transient-domain-secret-${index}`;
    const receipt = {
      receipt_digest: "a".repeat(63) + String(index % 10),
      request_digest: "b".repeat(63) + String(index % 10),
      value_digest: protectedValueDigest(value),
      domain,
    };
    values.set(protectedValueDigest(value), value);
    assert.equal(adapter.resolveReceipt(receipt, { purpose: "domain_value", valueKind: "credential", now: 100 }), value);
  });
  const snapshot = boundary.snapshot();
  assert.deepEqual(snapshot.coverage.map((row) => row.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.deepEqual(snapshot.coverage.map((row) => row.reference_count), AUTONOMOUS_DOMAIN_NAMES.map(() => 1));
  assert.equal(JSON.stringify(snapshot).includes("transient-domain-secret"), false);
  assert.equal(JSON.stringify(snapshot).toLowerCase().includes('"value":'), false);
});

test("snapshot restore is tenant-bound and CAS-safe", () => {
  const source = new AutonomousProtectedRehydrationBoundary(context(), () => "token", { clock: () => 100, maxTtlSeconds: 60 });
  source.issueForValue("persist", "token", { domain: "science", purpose: "provider_credential", issuedAt: 100, expiresAt: 150 });
  const store = new CasStore();
  const coordinator = new AutonomousProtectedRehydrationPersistenceCoordinator(source, new TransactionalJsonAutonomousProtectedRehydrationPersistence(store));
  const snapshot = coordinator.flush();
  assert.equal(validateAutonomousProtectedRehydrationSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);

  const restored = new AutonomousProtectedRehydrationBoundary(context(), () => "token", { clock: () => 100, maxTtlSeconds: 60 });
  const restoredCoordinator = new AutonomousProtectedRehydrationPersistenceCoordinator(restored, new JsonAutonomousProtectedRehydrationPersistence(store));
  assert.equal(restoredCoordinator.restore().snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restored.get("persist").value_digest, protectedValueDigest("token"));

  const otherTenant = new AutonomousProtectedRehydrationBoundary(context("tenant-b"), () => "token", { clock: () => 100, maxTtlSeconds: 60 });
  assert.throws(() => otherTenant.restore(snapshot), AutonomousProtectedRehydrationError);
  source.issueForValue("second", "token-2", { domain: "science", purpose: "provider_credential", issuedAt: 100, expiresAt: 150 });
  const competing = new AutonomousProtectedRehydrationBoundary(context(), () => "token", { clock: () => 100, maxTtlSeconds: 60 });
  const competingCoordinator = new AutonomousProtectedRehydrationPersistenceCoordinator(competing, new JsonAutonomousProtectedRehydrationPersistence(store));
  competingCoordinator.restore();
  competing.issueForValue("competing", "token-3", { domain: "science", purpose: "provider_credential", issuedAt: 100, expiresAt: 150 });
  competingCoordinator.flush();
  assert.throws(() => coordinator.flush(), AutonomousProtectedRehydrationError);
});

test("memory scheduler snapshots are bound to the same execution context", () => {
  const source = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { executionContext: context() });
  const snapshot = source.snapshot();
  assert.equal(snapshot.policy.rehydration_context_digest, context().contextDigest);
  const restored = new AutonomousMemoryConsolidationScheduler(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0 }), { executionContext: context("tenant-b") });
  assert.throws(() => restored.restore(snapshot), AutonomousMemoryConsolidationSchedulerError);
});
