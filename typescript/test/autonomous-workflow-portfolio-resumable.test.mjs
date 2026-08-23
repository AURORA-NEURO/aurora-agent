import assert from "node:assert/strict";
import test from "node:test";

import {
  AutonomousAgent,
  AutonomousWorkflowPortfolioExecutionController,
  AutonomousWorkflowPortfolioItemExecutionResult,
  CredentialStore,
  InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore,
  LLMRuntime,
  openaiCompatibleProvider,
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

function planningAgent(onRequest = () => {}) {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      let body = {};
      try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
      const prompt = JSON.stringify(body.messages ?? []);
      if (prompt.includes("priority_order") && prompt.includes("review_required")) {
        onRequest("planning");
        const contractMessage = (body.messages ?? []).find((message) => String(message.content ?? "").startsWith("Context planning-contract:\n"));
        let contract = {};
        try { contract = JSON.parse(String(contractMessage?.content ?? "").slice("Context planning-contract:\n".length)); } catch { /* bounded fixture fallback */ }
        const ids = (contract.stage_catalogue ?? []).map((stage) => stage.id);
        return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: ids.slice(0, 1), review_required: false, confidence: 0.96, abstain: false }) }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
      }
      onRequest("execution");
      return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "restart execution result" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("portfolio-restart-planner", "https://portfolio-restart-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel({ ...model, provider: "portfolio-restart-planner", model: "portfolio-restart-model" });
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

test("resumable portfolio execution rehydrates accepted provider plans and never re-invokes the planner", async () => {
  const firstCalls = [];
  const firstAgent = planningAgent((kind) => firstCalls.push(kind));
  const requests = [
    { id: "planner-first", task: "private planner restart first task", domain: "coding" },
    { id: "planner-second", task: "private planner restart second task", domain: "data", dependsOn: ["planner-first"] },
  ];
  const plan = await firstAgent.planWorkflowPortfolio(requests);
  const reviewed = await firstAgent.executeWorkflowPortfolio(requests, {
    plan,
    providerPlanning: { candidates: firstAgent.models(), approveProviderCall: true },
  });
  const accepted = Object.fromEntries(reviewed.items.map((item) => [item.itemId, item.planRefinement]));
  const firstRuns = new Map();
  const originalRun = firstAgent.run.bind(firstAgent);
  firstAgent.run = async (task, options) => {
    const run = await originalRun(task, options);
    firstRuns.set(task, run);
    return run;
  };
  const store = new InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore();
  let checkpoint = null;
  await assert.rejects(() => firstAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-planner-restart",
    plan,
    acceptedPlanRefinements: accepted,
    approveProviderCall: true,
    checkpointSink: async (value) => {
      checkpoint = value;
      await store.write(value);
      if (value.settled_item_ids.length > 0) throw new Error("planner restart interruption");
    },
  }), /planner restart interruption/);
  assert.equal(firstCalls.filter((kind) => kind === "planning").length, 2, "the review phase should plan each item once");
  assert.ok(checkpoint?.plan_refinement_digests?.every((digest) => typeof digest === "string"));
  assert.doesNotMatch(JSON.stringify(checkpoint), /private planner restart|restart execution result/);

  const resumedCalls = [];
  const resumedAgent = planningAgent((kind) => resumedCalls.push(kind));
  const restored = await resumedAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-planner-restart",
    plan,
    checkpoint: await store.read(),
    approveProviderCall: true,
    rehydratePlanRefinement: (context) => {
      assert.equal(context.jobId, "portfolio-planner-restart");
      return accepted[context.itemId];
    },
    rehydrateItem: async (context) => {
      const item = context.item_id === "planner-first" ? reviewed.items[0] : null;
      assert.ok(item);
      const run = firstRuns.get(requests[0].task);
      const output = run.response?.text ?? "";
      return new AutonomousWorkflowPortfolioItemExecutionResult(
        item.itemId,
        item.domain,
        item.dependsOn,
        "succeeded",
        run,
        await digestJson({ item_id: item.itemId, output }),
        new TextEncoder().encode(output).byteLength,
        null,
        null,
        true,
        output,
        item.learningStatus,
        item.learningEpisodeId,
        item.evaluationDigest,
        item.settlementDigest,
        item.learningErrorClass,
        item.planRefinement,
        "accepted",
        "not_eligible",
        item.plannerEvaluationDigest,
        item.plannerSettlementDigest,
        item.plannerErrorClass,
      );
    },
  });
  assert.equal(restored.status, "completed");
  assert.equal(resumedCalls.filter((kind) => kind === "planning").length, 0);
  assert.equal(resumedCalls.filter((kind) => kind === "execution").length, 1);
  assert.ok(restored.items.every((item) => item.planningStatus === "accepted" && item.planRefinement));
});

test("resumable portfolio checkpoints preserve planner review without promoting it to execution authority", async () => {
  const firstCalls = [];
  const firstAgent = planningAgent((kind) => firstCalls.push(kind));
  const requests = [{ id: "review-only", task: "private review-only planning task", domain: "coding" }];
  const plan = await firstAgent.planWorkflowPortfolio(requests);
  const store = new InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore();
  const reviewed = await firstAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-review-restart",
    plan,
    providerPlanning: { candidates: firstAgent.models(), approveProviderCall: true },
    approveProviderCall: true,
    checkpointSink: (value) => store.write(value),
  });
  assert.equal(reviewed.status, "plan_review_required");
  const checkpoint = await store.read();
  assert.equal(checkpoint.planning_statuses[0], "plan_review_required");
  assert.equal(typeof checkpoint.plan_refinement_digests[0], "string");

  const resumedCalls = [];
  const resumedAgent = planningAgent((kind) => resumedCalls.push(kind));
  const resumed = await resumedAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "portfolio-review-restart",
    plan,
    checkpoint,
    providerPlanning: { candidates: resumedAgent.models(), approveProviderCall: true },
    approveProviderCall: true,
    rehydratePlanRefinement: () => reviewed.items[0].planRefinement,
  });
  assert.equal(resumed.status, "plan_review_required");
  assert.equal(resumed.items[0].planningStatus, "plan_review_required");
  assert.equal(resumed.items[0].run, null);
  assert.equal(resumedCalls.filter((kind) => kind === "planning").length, 0);
  assert.equal(resumedCalls.filter((kind) => kind === "execution").length, 0);
  assert.equal(firstCalls.filter((kind) => kind === "planning").length, 1);
});
