import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousRunTraceSession,
  AutonomousRunTracePersistenceCoordinator,
  JsonAutonomousRunTracePersistence,
  AutonomousRunTraceRegistry,
  AutonomousRunTraceRegistryPersistenceCoordinator,
  JsonAutonomousRunTraceRegistryPersistence,
  publishAutonomousRunTraceRegistrySnapshot,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
  autonomousRunTraceStatus,
  digestJson,
  TransactionalJsonAutonomousRunTracePersistence,
  TransactionalJsonAutonomousRunTraceRegistryPersistence,
  WebStorageAutonomousRunTraceTextStore,
  validateAutonomousRunTraceSnapshot,
} from "../dist/index.js";

const taskFor = (domain) => `produce a bounded, reviewable result for ${domain}`;
const digest = (letter) => letter.repeat(64);

const model = {
  provider: "trace-offline",
  model: "trace-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function localAgent() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("trace-offline", () => ({ output_text: "bounded offline result" }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return agent;
}

test("run trace journal covers every autonomous domain and preserves only bounded metadata", async () => {
  let now = 1_000;
  const store = new InMemoryAutonomousRunTraceStore({ clock: () => now++ });
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const session = new AutonomousRunTraceSession(store, { run_id: `trace-${domain}`, task_digest: digest(index.toString(16)), domains: [domain] });
    await session.started();
    const observer = session.providerObserver();
    await observer.before({ provider: "offline", model: "local", kind: "provider_call", inputTokens: 17, requestedOutputTokens: 64, toolCount: 0 });
    await observer.after({ provider: "offline", model: "local", kind: "provider_call", inputTokens: 17, requestedOutputTokens: 64, toolCount: 0 }, { success: true, status: "completed", latencyMs: 3, inputTokens: 17, outputTokens: 9 });
    await session.complete({ status: "completed", route_digest: digest("a"), plan_digest: digest("b"), selection_digest: digest("c") });
    const summary = await session.summary();
    assert.equal(summary.status, "completed");
    assert.deepEqual(summary.domains, [domain]);
    assert.equal(summary.provider_invocations, 1);
    assert.equal(summary.input_tokens, 17);
    assert.equal(summary.output_tokens, 9);
    assert.equal(summary.tool_calls, 0);
  }

  assert.equal(store.verifyIntegrity().events, AUTONOMOUS_DOMAIN_NAMES.length * 4);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const events = store.events({ run_id: `trace-${domain}`, domain });
    assert.equal(events.length, 4, domain);
    assert.ok(events.every((event) => event.secret_material === "never_returned"));
    assert.ok(events.every((event) => event.retention.includes("no_prompts_responses")));
    assert.doesNotMatch(JSON.stringify(events), /credential|authorization|api[_ -]?key|bounded offline result/i);
  }
});

test("run trace snapshot rehydrates atomically and rejects tampering", async () => {
  const source = new InMemoryAutonomousRunTraceStore({ clock: () => 10 });
  const session = new AutonomousRunTraceSession(source, { run_id: "snapshot-run", task_digest: digest("d"), domains: ["science", "evaluation"] });
  await session.started();
  await session.complete({ status: "paused", route_digest: digest("f") });
  const snapshot = source.snapshot();
  assert.equal(snapshot.snapshot_generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  assert.deepEqual(source.snapshot(), snapshot);
  const restored = new InMemoryAutonomousRunTraceStore({ clock: () => 20 });
  restored.restore(snapshot);
  assert.deepEqual(restored.snapshot(), snapshot);
  assert.equal(restored.verifyIntegrity().head_digest, source.verifyIntegrity().head_digest);

  let persisted = null;
  const coordinator = new AutonomousRunTracePersistenceCoordinator(restored, {
    read: () => persisted,
    write: (value) => { persisted = structuredClone(value); },
  });
  await coordinator.flush();
  const restarted = new InMemoryAutonomousRunTraceStore({ clock: () => 30 });
  const restartedCoordinator = new AutonomousRunTracePersistenceCoordinator(restarted, {
    read: () => persisted,
    write: () => {},
  });
  await restartedCoordinator.restore();
  assert.deepEqual(restarted.snapshot(), snapshot);

  const tampered = structuredClone(snapshot);
  tampered.events[0].status = "failed";
  assert.throws(() => restored.restore(tampered), /digest|hash chain|invalid/);
  assert.deepEqual(restored.snapshot(), snapshot, "failed restore must leave live state unchanged");

  const forged = structuredClone(snapshot);
  forged.snapshot_generation = 2;
  forged.previous_snapshot_digest = null;
  const { snapshot_digest: _forgedDigest, ...forgedBody } = forged;
  forged.snapshot_digest = await digestJson(forgedBody);
  assert.throws(() => restored.restore(forged), /generation and previous_snapshot_digest/);

  const legacy = structuredClone(snapshot);
  delete legacy.snapshot_generation;
  delete legacy.previous_snapshot_digest;
  legacy.schema = "bioprism-typescript-autonomous-run-trace-snapshot/0.1";
  const { snapshot_digest: _legacyDigest, ...legacyBody } = legacy;
  legacy.snapshot_digest = await digestJson(legacyBody);
  const legacyStore = new InMemoryAutonomousRunTraceStore({ clock: () => 40 });
  legacyStore.restore(legacy);
  const upgraded = legacyStore.snapshot();
  assert.equal(upgraded.snapshot_generation, 1);
  assert.equal(upgraded.previous_snapshot_digest, null);
  assert.notEqual(upgraded.snapshot_digest, legacy.snapshot_digest);
});

test("run trace JSON, browser, and CAS persistence survive all-domain restart without stale overwrite", async () => {
  const source = new InMemoryAutonomousRunTraceStore({ clock: () => 100 });
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const session = new AutonomousRunTraceSession(source, { run_id: `persist-${domain}`, task_digest: digest((index + 1).toString(16)), domains: [domain] });
    await session.started();
    await session.complete({ status: "completed", detail_digest: digest("a") });
  }
  const snapshot = source.snapshot();
  let remoteText = null;
  const remoteStore = {
    read: () => remoteText,
    write: (value) => { remoteText = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = remoteText === null ? null : JSON.parse(remoteText).snapshot_digest;
      if (observed !== expected) return false;
      remoteText = value;
      return true;
    },
  };
  const transactional = new TransactionalJsonAutonomousRunTracePersistence(remoteStore);
  const coordinator = new AutonomousRunTracePersistenceCoordinator(source, transactional);
  await coordinator.flush();
  assert.equal(JSON.parse(remoteText).events.length, AUTONOMOUS_DOMAIN_NAMES.length * 2);

  const restartedStore = new InMemoryAutonomousRunTraceStore({ clock: () => 200 });
  const restartedCoordinator = new AutonomousRunTracePersistenceCoordinator(restartedStore, transactional);
  const restored = await restartedCoordinator.restore();
  assert.equal(restored.snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restartedStore.verifyIntegrity().events, AUTONOMOUS_DOMAIN_NAMES.length * 2);
  await restartedCoordinator.flush();
  const advanced = restartedStore.snapshot();
  assert.equal(advanced.snapshot_generation, 1);
  assert.equal(advanced.previous_snapshot_digest, null);

  const staleStore = new InMemoryAutonomousRunTraceStore({ clock: () => 300 });
  const staleCoordinator = new AutonomousRunTracePersistenceCoordinator(staleStore, transactional);
  await staleCoordinator.restore();
  const freshSession = new AutonomousRunTraceSession(restartedStore, { run_id: "persist-fresh", task_digest: digest("f"), domains: ["evaluation"] });
  await freshSession.started();
  await freshSession.complete({ status: "completed" });
  const second = await restartedCoordinator.flush();
  assert.equal(second.snapshot_generation, 2);
  assert.equal(second.previous_snapshot_digest, snapshot.snapshot_digest);
  const staleSession = new AutonomousRunTraceSession(staleStore, { run_id: "persist-stale", task_digest: digest("e"), domains: ["evaluation"] });
  await staleSession.started();
  await staleSession.complete({ status: "completed" });
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);

  const browser = new Map();
  const browserTextStore = new WebStorageAutonomousRunTraceTextStore({ getItem: (key) => browser.get(key) ?? null, setItem: (key, value) => browser.set(key, value) }, "run-trace");
  const browserPersistence = new JsonAutonomousRunTracePersistence(browserTextStore);
  await browserPersistence.write(snapshot);
  assert.deepEqual(await browserPersistence.read(), snapshot);
  browser.set("run-trace", ` ${browser.get("run-trace")}`);
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  await browserPersistence.write(snapshot);
  const tampered = structuredClone(snapshot);
  tampered.events[0].status = "failed";
  assert.throws(() => validateAutonomousRunTraceSnapshot(tampered), /digest|hash chain|invalid/);
  assert.doesNotMatch(remoteText, /credential|authorization|api[_ -]?key/i);
});

test("traced autonomous execution spans all domains and cross-domain fan-out without a provider key", async () => {
  const agent = localAgent();
  const store = new InMemoryAutonomousRunTraceStore();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const execution = await agent.runWithTrace(taskFor(domain), {
      traceStore: store,
      runId: `run-${domain}`,
      run: { domain, candidates: [model], approveProviderCall: true },
    });
    assert.equal(execution.result.status, "completed", domain);
    assert.equal(execution.trace.status, "completed", domain);
    assert.equal(execution.trace.domains.includes(domain), true, domain);
    assert.equal(execution.trace.provider_invocations, 1, domain);
    assert.equal(execution.result.provider_invocations.length, 1, domain);
    assert.equal(execution.result.provider_invocations[0].provider, "trace-offline", domain);
    assert.equal(execution.result.provider_failover, null, domain);
    assert.equal(execution.result.provider_invocations[0].secret_material, "never_returned", domain);
    assert.doesNotMatch(JSON.stringify(execution.result.provider_invocations), /bounded offline result|api[_ -]?key/i, domain);
    assert.ok(execution.trace.route_digest);
    assert.ok(execution.trace.plan_digest);
    const phases = store.events({ run_id: `run-${domain}` }).map((event) => event.phase);
    assert.ok(phases.includes("model_selection_started"), domain);
    assert.ok(phases.includes("model_selection_finished"), domain);
    assert.equal(JSON.stringify(execution.trace).includes(taskFor(domain)), false);
  }

  const cross = await agent.runCrossDomainWithTrace("coordinate a biomedical neuroscience evidence review", {
    traceStore: store,
    runId: "run-cross-domain",
    run: { candidates: [model], approveProviderCall: true, allowPartial: true, synthesize: false, maxParallelChildren: 2 },
  });
  assert.ok(["completed", "partial", "paused"].includes(cross.trace.status));
  assert.ok(cross.trace.domains.includes("cross_domain"));
  assert.ok(cross.trace.provider_invocations >= 2);
  assert.equal(cross.trace.provider_failures, 0);
  assert.ok(store.events({ run_id: "run-cross-domain" }).some((event) => event.phase === "model_selection_finished"));
  assert.equal(JSON.stringify(cross.trace).includes("coordinate a biomedical neuroscience evidence review"), false);
  assert.equal(store.verifyIntegrity().verified, true);
});

test("trace status mapping and terminal boundaries remain explicit", async () => {
  assert.equal(autonomousRunTraceStatus("completed"), "completed");
  assert.equal(autonomousRunTraceStatus("approval_required"), "paused");
  assert.equal(autonomousRunTraceStatus("abstained"), "refused");
  assert.equal(autonomousRunTraceStatus("child_failed"), "failed");
  assert.equal(autonomousRunTraceStatus("future_status"), "unknown");

  const session = new AutonomousRunTraceSession(new InMemoryAutonomousRunTraceStore(), { run_id: "terminal-run", task_digest: digest("e"), domains: ["coding"] });
  await session.started();
  await session.complete({ status: "refused", failure_code: "route_review" });
  const reused = new AutonomousRunTraceSession(session.store, { run_id: "terminal-run", task_digest: digest("e"), domains: ["coding"] });
  await assert.rejects(() => reused.started(), /already has events/);
  await assert.rejects(() => session.record({ phase: "started", status: "running" }), /already terminal/);
  await assert.rejects(() => session.complete({ status: "completed" }), /already terminal/);
});

test("trace registry indexes every domain, paginates deterministically, and enforces metadata-only retention", async () => {
  const source = new InMemoryAutonomousRunTraceStore({ clock: (() => { let now = 900; return () => now++; })() });
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const session = new AutonomousRunTraceSession(source, { run_id: `registry-${domain}`, task_digest: digest(index % 2 === 0 ? "c" : "d"), domains: [domain] });
    await session.started();
    await session.record({ phase: "plan_compiled", status: "running", plan_digest: digest("a") });
    await session.record({ phase: "provider_invocation_finished", status: "running", provider: "registry-provider", model: "registry-model", input_tokens: 4, output_tokens: 3, tool_count: 1 });
    await session.complete({ status: "completed", route_digest: digest("b"), plan_digest: digest("a") });
  }

  const registry = new AutonomousRunTraceRegistry({ max_runs: 32, max_events: 512, max_bytes: 2_000_000 });
  const imported = registry.importSnapshot(source.snapshot());
  assert.equal(imported.imported_run_ids.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(registry.size, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(registry.query({ domain: "biomedical" }).records[0].run_id, "registry-biomedical");
  assert.equal(registry.query({ provider: "registry-provider" }).total_matches, AUTONOMOUS_DOMAIN_NAMES.length);
  const firstPage = registry.query({ limit: 5 });
  assert.equal(firstPage.records.length, 5);
  assert.equal(firstPage.total_matches, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(firstPage.next_after_run_id);
  const secondPage = registry.query({ after_run_id: firstPage.next_after_run_id, limit: 20 });
  assert.equal(secondPage.records.length, AUTONOMOUS_DOMAIN_NAMES.length - 5);
  assert.equal(new Set([...firstPage.records, ...secondPage.records].map((record) => record.run_id)).size, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(registry.events({ phase: "provider_invocation_finished" }).length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(registry.verifyIntegrity().verified, true);

  let persistedText = null;
  const textStore = {
    read: () => persistedText,
    write: (value) => { persistedText = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = persistedText === null ? null : JSON.parse(persistedText).snapshot_digest;
      if (observed !== expected) return false;
      persistedText = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousRunTraceRegistryPersistence(textStore, { maxBytes: 2_000_000 });
  const coordinator = new AutonomousRunTraceRegistryPersistenceCoordinator(registry, persistence);
  const persisted = await coordinator.flush();
  const restored = new AutonomousRunTraceRegistry({ max_runs: 32, max_events: 512, max_bytes: 2_000_000 });
  const restoredCoordinator = new AutonomousRunTraceRegistryPersistenceCoordinator(restored, persistence);
  await restoredCoordinator.restore();
  assert.deepEqual(restored.snapshot(), persisted);
  assert.doesNotMatch(await textStore.read(), /private provider output|bounded offline result|sk-[A-Za-z0-9]/i);

  const staleRegistry = new AutonomousRunTraceRegistry({ max_runs: 32, max_events: 512, max_bytes: 2_000_000 });
  const staleCoordinator = new AutonomousRunTraceRegistryPersistenceCoordinator(staleRegistry, persistence);
  await staleCoordinator.restore();
  const freshSession = new AutonomousRunTraceSession(source, { run_id: "registry-fresh", task_digest: digest("e"), domains: ["evaluation"] });
  await freshSession.started();
  await freshSession.complete({ status: "completed" });
  registry.importSnapshot(source.snapshot());
  await coordinator.flush();
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);

  const summaryOnly = new AutonomousRunTraceRegistry({ max_runs: 32, max_events: 512, max_bytes: 2_000_000, retain_events: false });
  summaryOnly.importSnapshot(source.snapshot());
  assert.equal(summaryOnly.query({ model: "registry-model" }).total_matches, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(summaryOnly.events().length, 0);
  assert.equal(summaryOnly.get("registry-coding").retained_event_count, 0);

  const retained = new AutonomousRunTraceRegistry({ max_runs: 2, max_events: 512, max_bytes: 2_000_000 });
  const retainedReport = retained.importSnapshot(source.snapshot());
  assert.equal(retained.size, 2);
  assert.equal(retainedReport.evicted_run_ids.length, AUTONOMOUS_DOMAIN_NAMES.length + 1 - 2);
  assert.equal(retained.verifyIntegrity().runs, 2);

  const activeSource = new InMemoryAutonomousRunTraceStore({ clock: () => 1_200 });
  const activeA = new AutonomousRunTraceSession(activeSource, { run_id: "active-a", task_digest: digest("a"), domains: ["coding"] });
  await activeA.started();
  const activeB = new AutonomousRunTraceSession(activeSource, { run_id: "active-b", task_digest: digest("b"), domains: ["data"] });
  await activeB.started();
  const activeRegistry = new AutonomousRunTraceRegistry({ max_runs: 1, max_events: 32, max_bytes: 100_000 });
  assert.throws(() => activeRegistry.importSnapshot(activeSource.snapshot()), /cannot evict an eligible terminal run/);
  assert.equal(activeRegistry.size, 0, "failed retention import must be atomic");
});

test("trace registry publication is bounded, idempotent, and isolated from source failures", async () => {
  const source = new InMemoryAutonomousRunTraceStore({ clock: () => 2_000 });
  const session = new AutonomousRunTraceSession(source, { run_id: "publication-run", task_digest: digest("f"), domains: [...AUTONOMOUS_DOMAIN_NAMES] });
  await session.started();
  await session.complete({ status: "completed" });
  const registry = new AutonomousRunTraceRegistry({ max_runs: 8, max_events: 64, max_bytes: 100_000 });
  const first = await publishAutonomousRunTraceRegistrySnapshot(registry, source, "publication-run");
  assert.equal(first.status, "published");
  assert.equal(first.run_import_state, "imported");
  assert.equal(first.evicted_run_count, 0);
  assert.equal(registry.query({ run_id: "publication-run" }).total_matches, 1);
  const second = await publishAutonomousRunTraceRegistrySnapshot(registry, source, "publication-run");
  assert.equal(second.status, "published");
  assert.equal(second.run_import_state, "unchanged");
  const failed = await publishAutonomousRunTraceRegistrySnapshot(registry, { snapshot: () => { throw new Error("source unavailable"); } }, "publication-run");
  assert.equal(failed.status, "failed");
  assert.equal(failed.failure_code, "trace_registry_publication_failed");
  assert.equal(registry.query({ run_id: "publication-run" }).total_matches, 1);
});
