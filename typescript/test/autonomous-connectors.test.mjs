import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousConnectorDispatchRequest,
  AutonomousConnectorObservation,
  AutonomousConnectorRegistration,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousConnectorReceiptJournalPersistenceCoordinator,
  CredentialStore,
  InMemoryAutonomousConnectorReceiptJournal,
  LLMRuntime,
  AutonomousAgent,
} from "../dist/index.js";

function manifest(connectorId, domains = AUTONOMOUS_DOMAIN_NAMES) {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: connectorId,
    version: "1.0.0",
    provider: "local-test-provider",
    connector_kind: "provider_api",
    domains: [...domains],
    capabilities: ["evidence_read"],
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: ["opaque-session-ref"],
      does_not_claim: ["credential validity", "provider availability"],
    },
  };
}

function request(overrides = {}) {
  return new AutonomousConnectorDispatchRequest({
    dispatch_id: "dispatch-1",
    execution_id: "execution-1",
    call_id: "call-1",
    connector_id: "connector-a",
    domains: ["coding"],
    capability: "evidence_read",
    request: { subject_digest: "a".repeat(64), operation: "read" },
    approved: true,
    ...overrides,
  });
}

test("connector selection covers all twelve autonomous domains and binds a live registry digest", () => {
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a"), async () => ({ source: "a", observed: true })),
  ]);
  const plan = registry.selectForDomains(AUTONOMOUS_DOMAIN_NAMES, { capability: "evidence_read" });
  assert.equal(plan.complete, true);
  assert.equal(plan.rows.length, 12);
  assert.ok(plan.rows.every((row) => row.connector_id === "connector-a"));
  assert.equal(plan.registry_digest, registry.digest);
  assert.equal(plan.plan_digest.length, 64);
  plan.verify(registry);
  const coverage = registry.planForDomains(["coding", "biomedical"], { capability: "evidence_read" });
  assert.equal(coverage.coverage.coding.status, "selected");
  assert.equal(coverage.coverage.biomedical.status, "selected");
});

test("weighted connector selection consumes explicit health/evaluator evidence with deterministic ties", () => {
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a", ["science"]), async () => null),
    new AutonomousConnectorRegistration(manifest("connector-b", ["science"]), async () => null),
  ]);
  const plan = registry.selectAdaptiveForDomains(["science"], "evidence_read", {
    "connector-a": { eligible: true, health: 0.4, evaluator_reward: 0.1 },
    "connector-b": { eligible: true, health: 0.95, evaluator_reward: 0.9 },
  });
  assert.equal(plan.rows[0].connector_id, "connector-b");
  assert.equal(plan.strategy, "weighted_evidence");
  assert.equal(plan.signal_digest.length, 64);
  assert.throws(() => registry.selectForDomains(["science"], { capability: "evidence_read", strategy: "weighted_evidence" }), /requires selectionSignals/);
});

test("connector runtime enforces approval and scope, returns transient values, and journals metadata only", async () => {
  let calls = 0;
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a", ["coding"]), async (_manifest, input) => {
      calls += 1;
      return new AutonomousConnectorObservation({ subject_digest: input.subject_digest, records: 2 }, "observed");
    }),
  ]);
  const runtime = new AutonomousConnectorRuntime(registry, { receiptStore: journal });
  const plan = registry.selectForDomains(["coding"], { capability: "evidence_read" });
  const first = await runtime.dispatchFromPlan(plan, request({ selection_plan_digest: plan.plan_digest }));
  assert.equal(first.receipt.status, "observed");
  assert.equal(first.replay, "fresh");
  assert.deepEqual(first.value, { subject_digest: "a".repeat(64), records: 2 });
  assert.equal(Object.prototype.hasOwnProperty.call(first.receipt, "request"), false);
  const replay = await runtime.dispatchFromPlan(plan, request({ selection_plan_digest: plan.plan_digest }));
  assert.equal(replay.replay, "replayed");
  assert.equal(replay.value, null);
  assert.equal(calls, 1);

  const refused = await runtime.dispatch(request({ dispatch_id: "dispatch-approval", call_id: "call-approval", approved: false }));
  assert.equal(refused.receipt.status, "refused");
  assert.equal(refused.receipt.failure_class, "approval_required");
  const outOfScope = await runtime.dispatch(request({ dispatch_id: "dispatch-scope", call_id: "call-scope", domains: ["biomedical"] }));
  assert.equal(outOfScope.receipt.status, "refused");
  assert.equal(outOfScope.receipt.failure_class, "domain_scope");
  assert.equal(calls, 1);

  const snapshot = journal.snapshot();
  const restored = new InMemoryAutonomousConnectorReceiptJournal();
  restored.restore(snapshot);
  assert.deepEqual(restored.verifyIntegrity(), journal.verifyIntegrity());
  const tampered = structuredClone(snapshot);
  tampered.entries[0].receipt.status = "error";
  assert.throws(() => restored.restore(tampered), /snapshot digest/);
});

test("connector runtime deduplicates concurrent dispatches and rejects credential-shaped payloads", async () => {
  let calls = 0;
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a", ["coding"]), async () => {
      calls += 1;
      await new Promise((resolve) => setTimeout(resolve, 5));
      return { ok: true };
    }),
  ]);
  const runtime = new AutonomousConnectorRuntime(registry);
  const base = request({ dispatch_id: "dispatch-concurrent", call_id: "call-concurrent" });
  const [left, right] = await Promise.all([runtime.dispatch(base), runtime.dispatch(base)]);
  assert.deepEqual([left.replay, right.replay].sort(), ["fresh", "replayed"]);
  assert.equal(calls, 1);
  assert.throws(() => request({ request: { api_key: "must-not-enter" } }), /credential-shaped/);
  assert.throws(() => new AutonomousConnectorObservation({ access_token: "must-not-enter" }), /credential-shaped/);
});

test("connector runtime streams lifecycle metadata for fresh, replayed, and in-flight dispatches", async () => {
  const events = [];
  let release;
  const entered = new Promise((resolve) => { release = resolve; });
  let continueExecution;
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a", ["coding"]), async () => {
      continueExecution = () => release();
      await entered;
      return { ok: true };
    }),
  ]);
  const runtime = new AutonomousConnectorRuntime(registry);
  const base = request({ dispatch_id: "dispatch-trace", execution_id: "execution-trace", call_id: "call-trace" });
  const callback = (event) => { events.push(event); };
  const first = runtime.dispatch(base, { traceEventCallback: callback });
  await new Promise((resolve) => setTimeout(resolve, 0));
  const second = runtime.dispatch(base, { traceEventCallback: callback });
  await new Promise((resolve) => setTimeout(resolve, 0));
  continueExecution();
  const [left, right] = await Promise.all([first, second]);
  assert.deepEqual([left.replay, right.replay].sort(), ["fresh", "replayed"]);
  assert.equal(events.filter((event) => event.phase === "connector_started").length, 2);
  assert.equal(events.filter((event) => event.phase === "connector_finished").length, 2);
  assert.ok(events.filter((event) => event.phase === "connector_finished").every((event) => event.status === "completed"));
  assert.equal(events.some((event) => Object.prototype.hasOwnProperty.call(event, "request")), false);
});

test("AutonomousAgent exposes connector coverage and selection without invoking a provider", async () => {
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a"), async () => ({ ok: true })),
  ]);
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), { connectorRegistry: registry });
  const coverage = agent.connectorCoverage(["coding"], { capability: "evidence_read" });
  assert.equal(coverage.coverage.coding.status, "selected");
  const plan = agent.selectConnectors(["coding"], { capability: "evidence_read" });
  assert.equal(plan.complete, true);
});

test("connector receipt snapshots restore through a caller-owned persistence coordinator", async () => {
  const registry = new AutonomousConnectorRegistry([
    new AutonomousConnectorRegistration(manifest("connector-a", ["coding"]), async () => ({ ok: true })),
  ]);
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const runtime = new AutonomousConnectorRuntime(registry, { receiptStore: journal });
  const plan = registry.selectForDomains(["coding"], { capability: "evidence_read" });
  await runtime.dispatchFromPlan(plan, request({ selection_plan_digest: plan.plan_digest }));
  let persisted = null;
  const coordinator = new AutonomousConnectorReceiptJournalPersistenceCoordinator(journal, {
    read: () => persisted,
    write: (snapshot) => { persisted = structuredClone(snapshot); },
  });
  const snapshot = await coordinator.flush();
  assert.equal(snapshot.entries.length, 1);
  const restoredJournal = new InMemoryAutonomousConnectorReceiptJournal();
  const restored = new AutonomousConnectorReceiptJournalPersistenceCoordinator(restoredJournal, { read: () => persisted, write: () => {} });
  assert.deepEqual(await restored.restore(), { status: "restored", snapshot_digest: snapshot.snapshot_digest, entries: 1 });
  assert.deepEqual(restoredJournal.verifyIntegrity(), journal.verifyIntegrity());
});
