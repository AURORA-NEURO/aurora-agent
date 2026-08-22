import assert from "node:assert/strict";
import test from "node:test";

import {
  AutonomousAgent,
  AutonomousWorkflowPortfolioExecutionController,
  AutonomousWorkflowPortfolioItemExecutionResult,
  InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore,
  LLMRuntime,
  digestJson,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: ["reasoning", "structured_output", "code", "data", "science"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function agentFor(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline result for ${request.model}` };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return agent;
}

async function rehydratedSuccess(itemId, domain, run, dependsOn = []) {
  const output = run.response?.text ?? "";
  return new AutonomousWorkflowPortfolioItemExecutionResult(
    itemId,
    domain,
    dependsOn,
    "succeeded",
    run,
    output ? await digestJson({ item_id: itemId, output }) : null,
    new TextEncoder().encode(output).byteLength,
    null,
    null,
    true,
    output,
  );
}

test("resumable portfolio execution rehydrates settled work and dispatches only the pending wave", async () => {
  const firstRuns = new Map();
  let firstProviderCalls = 0;
  const firstAgent = agentFor(() => { firstProviderCalls += 1; });
  const originalRun = firstAgent.run.bind(firstAgent);
  firstAgent.run = async (task, options) => {
    const run = await originalRun(task, options);
    firstRuns.set(task, run);
    return run;
  };
  const requests = [
    { id: "first", task: "private first task", domain: "coding" },
    { id: "second", task: "private dependent task", domain: "data", dependsOn: ["first"] },
  ];
  const plan = await firstAgent.planWorkflowPortfolio(requests);
  let checkpoint = null;
  let writes = 0;
  await assert.rejects(
    () => firstAgent.executeWorkflowPortfolioResumable(requests, {
      jobId: "portfolio-restart",
      plan,
      approveProviderCall: true,
      checkpointSink: async (value) => {
        checkpoint = value;
        writes += 1;
        if (value.settled_item_ids.length > 0) throw new Error("synthetic process interruption after first wave");
      },
    }),
    /synthetic process interruption/,
  );
  assert.equal(firstProviderCalls, 1);
  assert.equal(writes, 2);
  assert.ok(checkpoint);
  assert.deepEqual(checkpoint.settled_item_ids, ["first"]);
  assert.doesNotMatch(JSON.stringify(checkpoint), /private first task|private dependent task|offline result/);

  let resumedProviderCalls = 0;
  const resumedAgent = agentFor(() => { resumedProviderCalls += 1; });
  let finalCheckpoint = null;
  const result = await resumedAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-restart",
    plan,
    checkpoint,
    approveProviderCall: true,
    checkpointSink: async (value) => { finalCheckpoint = value; },
    rehydrateItem: async (context) => {
      assert.equal(context.item_id, "first");
      return rehydratedSuccess("first", "coding", firstRuns.get("private first task"));
    },
  });

  assert.equal(result.status, "completed");
  assert.equal(resumedProviderCalls, 1, "the first provider call was not replayed");
  assert.ok(result.items.every((item) => item.status === "succeeded"));
  assert.deepEqual(finalCheckpoint.settled_item_ids, ["first", "second"]);
  assert.equal(finalCheckpoint.status, "completed");
});

test("portfolio checkpoint storage and controller restore a completed run without provider replay", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const requests = [{ id: "one", task: "private controller task", domain: "science" }];
  const plan = await agent.planWorkflowPortfolio(requests);
  const store = new InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore();
  const controller = new AutonomousWorkflowPortfolioExecutionController(agent, "portfolio-controller", store);
  const firstRun = await controller.run(requests, {
    plan,
    approveProviderCall: true,
  });
  const first = firstRun.execution;
  assert.equal(first.status, "completed");
  assert.equal(providerCalls, 1);

  const restartedAgent = agentFor(() => { providerCalls += 1; });
  const restartedController = new AutonomousWorkflowPortfolioExecutionController(restartedAgent, "portfolio-controller", store);
  const restoredProjection = await restartedController.restore();
  assert.equal(restoredProjection.status, "restored");
  const restoredRun = await restartedController.run(requests, {
    plan,
    approveProviderCall: true,
    rehydrateItem: async () => first.items[0],
  });
  const restored = restoredRun.execution;
  assert.equal(restored.status, "completed");
  assert.equal(providerCalls, 1, "a completed checkpoint must not replay its provider call");

  const restoredCheckpoint = await store.read();
  const tampered = structuredClone(restoredCheckpoint);
  tampered.settled_result_digests[0] = "0".repeat(64);
  await assert.rejects(() => restartedAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-controller",
    plan,
    checkpoint: tampered,
    approveProviderCall: true,
    rehydrateItem: async () => first.items[0],
  }), /checkpoint digest is invalid/);
});

test("portfolio rehydration rejects output or plan drift before provider dispatch", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const requests = [{ id: "one", task: "private drift task", domain: "coding" }];
  const plan = await agent.planWorkflowPortfolio(requests);
  const first = await agent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-drift",
    plan,
    approveProviderCall: true,
  });
  const store = new InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore();
  await agent.executeWorkflowPortfolioResumable(requests, { jobId: "portfolio-drift", plan, approveProviderCall: true, checkpointSink: (value) => store.write(value) });
  const checkpoint = await store.read();
  assert.ok(checkpoint);
  await assert.rejects(() => agent.executeWorkflowPortfolioResumable([{ ...requests[0], task: "changed private drift task" }], {
    jobId: "portfolio-drift",
    plan,
    checkpoint,
    approveProviderCall: true,
    rehydrateItem: async () => new AutonomousWorkflowPortfolioItemExecutionResult("one", "coding", [], "succeeded", first.items[0].run, "0".repeat(64), 1, null, null, true, "wrong output"),
  }), /plan verification failed|output digest/);
  await assert.rejects(() => agent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-drift",
    plan,
    checkpoint,
    approveProviderCall: true,
    rehydrateItem: async () => new AutonomousWorkflowPortfolioItemExecutionResult("one", "coding", [], "succeeded", first.items[0].run, "0".repeat(64), 1, null, null, true, "wrong output"),
  }), /output digest/);
  assert.equal(providerCalls, 2, "only the two explicitly requested initial executions dispatched");
});
