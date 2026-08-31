import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAuthorizationContext,
  AutonomousAuthorizationGate,
  AutonomousAuthorizationLedger,
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

test("connector runtime enforces the authorization context before every domain executor", async () => {
  const calls = [];
  const registry = new AutonomousConnectorRegistry(AUTONOMOUS_DOMAIN_NAMES.map((domain) =>
    new AutonomousConnectorRegistration(manifest(`connector-${domain}`, [domain]), async () => {
      calls.push(domain);
      return { domain };
    }, false),
  ));
  const runtime = new AutonomousConnectorRuntime(registry);
  const ledger = new AutonomousAuthorizationLedger(4, 64);
  const grant = ledger.issue({
    grant_id: "connector-runtime-grant",
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: "a".repeat(64),
    allowed_domains: [...AUTONOMOUS_DOMAIN_NAMES],
    allowed_operations: ["connector_dispatch"],
    allowed_capabilities: [],
    allowed_risk_classes: [],
    issued_at: 1000,
    expires_at: 2000,
    max_uses: AUTONOMOUS_DOMAIN_NAMES.length,
  });
  const context = new AutonomousAuthorizationContext(
    new AutonomousAuthorizationGate(ledger), grant.grant_id, grant.tenant_id, grant.actor_id,
    grant.session_id, grant.authorization_digest, [...AUTONOMOUS_DOMAIN_NAMES], null,
    "provider_invocation", "connector", () => 1200,
  );
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = registry.selectForDomains([domain], { capability: "evidence_read" });
    const result = await runtime.dispatchFromPlan(plan, request({
      dispatch_id: `dispatch-${domain}`,
      execution_id: `execution-${domain}`,
      call_id: `call-${domain}`,
      connector_id: `connector-${domain}`,
      domains: [domain],
      selection_plan_digest: plan.plan_digest,
    }), { authorizationContext: context });
    assert.equal(result.receipt.status, "observed", domain);
  }
  assert.deepEqual(calls, [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(ledger.get(grant.grant_id).used_count, AUTONOMOUS_DOMAIN_NAMES.length);

  const blockedLedger = new AutonomousAuthorizationLedger(2, 8);
  const blocked = blockedLedger.issue({
    grant_id: "blocked-connector-grant", tenant_id: "tenant-a", actor_id: "actor-a", session_id: "session-a",
    authorization_digest: "a".repeat(64), allowed_domains: ["coding"], allowed_operations: ["tool_execution"],
    allowed_capabilities: [], allowed_risk_classes: [], issued_at: 1000, expires_at: 2000, max_uses: 1,
  });
  const blockedContext = new AutonomousAuthorizationContext(
    new AutonomousAuthorizationGate(blockedLedger), blocked.grant_id, blocked.tenant_id, blocked.actor_id,
    blocked.session_id, blocked.authorization_digest, ["coding"], null, "provider_invocation", "blocked", () => 1200,
  );
  await assert.rejects(() => runtime.dispatch(request({ dispatch_id: "dispatch-blocked", execution_id: "execution-blocked", call_id: "call-blocked", connector_id: "connector-coding", domains: ["coding"] }), { authorizationContext: blockedContext }), /authorization was refused/);
  assert.deepEqual(calls, [...AUTONOMOUS_DOMAIN_NAMES]);
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
