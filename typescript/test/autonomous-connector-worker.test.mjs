import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousConnectorDispatchRequest,
  AutonomousConnectorObservation,
  AutonomousConnectorOperationRegistry,
  AutonomousConnectorRegistration,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousConnectorWorker,
  InMemoryAutonomousConnectorFeedbackLedger,
  InMemoryAutonomousConnectorReceiptJournal,
  InMemoryAutonomousConnectorWorkQueue,
} from "../dist/index.js";

function connectorManifest(capabilities) {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "worker-test-connector",
    version: "1.0.0",
    provider: "worker-test-provider",
    connector_kind: "provider_api",
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
    capabilities,
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: ["opaque-session-ref"],
      does_not_claim: ["credential validity", "provider availability"],
    },
  };
}

function fixture() {
  const operationRegistry = new AutonomousConnectorOperationRegistry();
  const connectorCapabilities = [...new Set(operationRegistry.operations().flatMap((operation) => operation.capabilities))];
  let calls = 0;
  const receiptJournal = new InMemoryAutonomousConnectorReceiptJournal();
  const connectorRegistry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(connectorManifest(connectorCapabilities), async (_manifest, request) => {
      calls += 1;
      return new AutonomousConnectorObservation({
        operation_id: request.operation_id,
        subject_digest: request.subject_digest,
        observed: true,
      }, "observed");
    }),
  ]);
  const runtime = new AutonomousConnectorRuntime(connectorRegistry, { receiptStore: receiptJournal });
  return { operationRegistry, connectorRegistry, runtime, receiptJournal, calls: () => calls };
}

function request(plan, overrides = {}) {
  return new AutonomousConnectorDispatchRequest({
    dispatch_id: "worker-dispatch-1",
    execution_id: "worker-execution-1",
    call_id: "worker-call-1",
    connector_id: "worker-test-connector",
    domains: ["coding"],
    capability: "review",
    request: {
      operation_id: "coding.repository_change_analysis",
      subject_digest: "a".repeat(64),
    },
    parent_digests: ["b".repeat(64)],
    selection_plan_digest: plan.plan_digest,
    approved: true,
    ...overrides,
  });
}

test("the operation registry covers every domain and every built-in stage vocabulary", () => {
  const registry = new AutonomousConnectorOperationRegistry();
  assert.deepEqual(registry.operations().map((operation) => operation.domain).sort(), [...AUTONOMOUS_DOMAIN_NAMES].sort());
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const operations = registry.forDomain(domain);
    assert.equal(operations.length, 1, domain);
    assert.ok(operations[0].capabilities.length >= 1, domain);
    assert.equal(operations[0].operation_digest.length, 64, domain);
  }
  assert.equal(JSON.stringify(registry.toJSON()).includes("subject_digest"), false);
  assert.throws(() => new AutonomousConnectorOperationRegistry(registry.operations().slice(0, -1)), /cover every autonomous domain/);

  const compositeFixture = fixture();
  const compositePlan = compositeFixture.connectorRegistry.selectForDomains(["coding"], { capability: "review+debugging" });
  assert.equal(compositePlan.rows[0].connector_id, "worker-test-connector");
  assert.doesNotThrow(() => request(compositePlan, { capability: "review+debugging" }));
});

test("the work queue is metadata-only, fenced, recoverable, retry-bounded, and tamper-evident", () => {
  const fixtureData = fixture();
  const plan = fixtureData.connectorRegistry.selectForDomains(["coding"], { capability: "review" });
  const originalRequest = request(plan);
  const queue = new InMemoryAutonomousConnectorWorkQueue(fixtureData.operationRegistry);
  const item = queue.enqueue({ work_id: "work-1", operation_id: "coding.repository_change_analysis", request: originalRequest, now: 1_000, max_attempts: 3 });
  assert.equal(JSON.stringify(item).includes("subject_digest"), false);
  assert.equal(item.request_digest, originalRequest.request_digest);
  assert.deepEqual(queue.pending(8, 1_000).map((row) => row.work_id), ["work-1"]);
  const leased = queue.claim("work-1", "worker-a", 100, 1_000);
  assert.equal(leased.status, "leased");
  assert.throws(() => queue.fail("work-1", "worker-b", "unknown", true, 1_001), /fenced/);
  assert.equal(queue.claim("work-1", "worker-b", 100, 1_001), null);
  const recovered = queue.claim("work-1", "worker-b", 100, 1_101);
  assert.equal(recovered.lease_owner, "worker-b");
  const retried = queue.fail("work-1", "worker-b", "transport_error", true, 1_101);
  assert.equal(retried.status, "queued");
  assert.equal(retried.available_at, 3_101);
  const finalLease = queue.claim("work-1", "worker-c", 100, 3_101);
  assert.equal(finalLease.attempts, 3);
  const failed = queue.fail("work-1", "worker-c", "transport_error", true, 3_101);
  assert.equal(failed.status, "failed");
  assert.deepEqual(queue.verifyIntegrity().verified, true);

  const snapshot = queue.snapshot();
  assert.equal(JSON.stringify(snapshot).includes("subject_digest"), false);
  const restored = new InMemoryAutonomousConnectorWorkQueue(fixtureData.operationRegistry);
  restored.restore(snapshot);
  assert.deepEqual(restored.verifyIntegrity(), queue.verifyIntegrity());
  const tampered = structuredClone(snapshot);
  tampered.items[0].status = "completed";
  assert.throws(() => restored.restore(tampered), /snapshot digest/);
  assert.throws(() => queue.enqueue({ work_id: "work-1", operation_id: "coding.repository_change_analysis", request: request(plan, { dispatch_id: "other-dispatch" }), now: 3_000 }), /identity conflicts/);
});

test("the worker rehydrates plans and requests, invokes once, handles replay, and reconciles missing state", async () => {
  const fixtureData = fixture();
  const plan = fixtureData.connectorRegistry.selectForDomains(["coding"], { capability: "review" });
  const firstRequest = request(plan);
  const missingRequest = request(plan, { dispatch_id: "worker-dispatch-missing", call_id: "worker-call-missing" });
  const queue = new InMemoryAutonomousConnectorWorkQueue(fixtureData.operationRegistry);
  queue.enqueue({ work_id: "work-fresh", operation_id: "coding.repository_change_analysis", request: firstRequest, now: 1_000 });
  queue.enqueue({ work_id: "work-replay", operation_id: "coding.repository_change_analysis", request: firstRequest, now: 1_000 });
  queue.enqueue({ work_id: "work-missing", operation_id: "coding.repository_change_analysis", request: missingRequest, now: 1_000 });
  const worker = new AutonomousConnectorWorker(fixtureData.runtime, queue, (item) => {
    if (item.work_id === "work-missing") return null;
    return { plan, request: firstRequest };
  });
  const firstRun = await worker.run({ workerId: "worker-a", now: 1_000, leaseMs: 10_000 });
  assert.equal(firstRun.completed, 2);
  assert.equal(firstRun.reconciled, 1);
  assert.equal(fixtureData.calls(), 1, "the second identical request must be a receipt replay");
  assert.ok(firstRun.rows.every((row) => row.value_retained === false));
  assert.equal(JSON.stringify(firstRun).includes("subject_digest"), false);
  assert.equal(queue.get("work-fresh").status, "completed");
  assert.equal(queue.get("work-replay").status, "completed");
  assert.equal(queue.get("work-missing").status, "reconciliation_required");
  assert.equal(fixtureData.receiptJournal.verifyIntegrity().entries, 1);
});

test("feedback requires an explicit evaluator and produces adaptive signals without inferring reward", async () => {
  const fixtureData = fixture();
  const plan = fixtureData.connectorRegistry.selectForDomains(["coding"], { capability: "review" });
  const result = await fixtureData.runtime.dispatchFromPlan(plan, request(plan));
  const ledger = new InMemoryAutonomousConnectorFeedbackLedger();
  assert.throws(() => ledger.record({ receipt: result.receipt, feedback: { feedback_id: "implicit", evaluator_id: "eval", evaluator_version: "1", reward: 1, passed: true } }), /caller_evaluator/);
  const entry = ledger.record({
    receipt: result.receipt,
    feedback: {
      feedback_id: "feedback-1",
      evaluator_id: "offline-rubric",
      evaluator_version: "2026.08",
      reward: 0.8,
      passed: true,
      source: "caller_evaluator",
      evidence_digest: "c".repeat(64),
      created_at: 1_000,
    },
  });
  assert.equal(entry.reward, 0.8);
  const signals = ledger.signals({ domain: "coding", capability: "review" });
  assert.equal(signals["worker-test-connector"].evaluator_reward, 0.8);
  assert.equal(signals["worker-test-connector"].success_rate, 1);
  assert.equal(signals["worker-test-connector"].latency_ms, null);
  assert.equal(JSON.stringify(ledger.snapshot()).includes("subject_digest"), false);
  const snapshot = ledger.snapshot();
  const restored = new InMemoryAutonomousConnectorFeedbackLedger();
  restored.restore(snapshot);
  assert.deepEqual(restored.verifyIntegrity(), ledger.verifyIntegrity());
  const tampered = structuredClone(snapshot);
  tampered.entries[0].reward = -1;
  assert.throws(() => restored.restore(tampered), /snapshot digest/);
});
