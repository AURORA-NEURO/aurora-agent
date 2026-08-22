import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousRunTraceSession,
  AutonomousRunTracePersistenceCoordinator,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
  autonomousRunTraceStatus,
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
    assert.ok(execution.trace.route_digest);
    assert.ok(execution.trace.plan_digest);
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
