import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
} from "../dist/index.js";

const taskFor = (domain) => `review a bounded ${domain} task and report uncertainty`;

const model = {
  provider: "brain-trace-offline",
  model: "brain-trace-model",
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

function brain() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("brain-trace-offline", () => ({ output_text: "bounded brain result" }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return new AutonomousBrainFacade({ agent });
}

test("brain facade trace spans plan compilation and provider execution for every domain", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = taskFor(domain);
    const traced = await facade.executeWithTrace({ task, domain }, {
      traceStore: store,
      runId: `facade-${domain}`,
      approveProviderCall: true,
      run: { candidates: [model] },
    });
    assert.equal(traced.execution.status, "completed", domain);
    assert.equal(traced.trace.status, "completed", domain);
    assert.equal(traced.trace.provider_invocations, 1, domain);
    assert.ok(traced.trace.route_digest);
    assert.equal(traced.trace.plan_digest, traced.execution.plan.plan_digest);
    const events = store.events({ run_id: `facade-${domain}` });
    assert.deepEqual(events.map((event) => event.phase), [
      "started", "plan_compiled", "provider_invocation_started", "provider_invocation_finished", "completed",
    ], domain);
    assert.equal(JSON.stringify(traced.trace).includes(task), false);
  }
  assert.equal(store.verifyIntegrity().verified, true);
});

test("brain facade trace preserves approval pauses and cross-domain fan-out without inventing success", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  const paused = await facade.executeWithTrace({ task: taskFor("coding"), domain: "coding" }, {
    traceStore: store,
    runId: "facade-paused",
    run: { candidates: [model] },
  });
  assert.equal(paused.execution.status, "approval_required");
  assert.equal(paused.trace.status, "paused");
  assert.equal(paused.trace.provider_invocations, 0);
  assert.equal(store.events({ run_id: "facade-paused", phase: "plan_compiled" }).length, 1);

  const cross = await facade.executeWithTrace({ task: "coordinate biomedical neuroscience evidence review" }, {
    traceStore: store,
    runId: "facade-cross",
    approveProviderCall: true,
    run: { candidates: [model] },
  });
  assert.equal(cross.execution.status, "completed");
  assert.equal(cross.trace.status, "completed");
  assert.ok(cross.trace.domains.includes("cross_domain"));
  assert.ok(cross.trace.provider_invocations >= 2);
  assert.ok(store.events({ run_id: "facade-cross", phase: "plan_compiled" }).length === 1);
  assert.equal(JSON.stringify(cross.trace).includes("coordinate biomedical neuroscience evidence review"), false);
});

test("brain facade traced plan replay revalidates identity before provider dispatch", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  const task = taskFor("evaluation");
  const plan = await facade.plan({ task, domain: "evaluation" });
  const traced = await facade.executePlannedWithTrace(plan, { task, domain: "evaluation" }, {
    traceStore: store,
    runId: "facade-replay",
    approveProviderCall: true,
    run: { candidates: [model] },
  });
  assert.equal(traced.execution.status, "completed");
  assert.equal(traced.execution.plan.plan_digest, plan.plan_digest);
  assert.equal(traced.trace.plan_digest, plan.plan_digest);
  await assert.rejects(
    () => facade.executePlannedWithTrace(plan, { task: `${task} changed`, domain: "evaluation" }, {
      traceStore: store,
      runId: "facade-replay-tampered",
      approveProviderCall: true,
      run: { candidates: [model] },
    }),
    /does not match/,
  );
  assert.equal(store.events({ run_id: "facade-replay-tampered" }).length, 0);
});
