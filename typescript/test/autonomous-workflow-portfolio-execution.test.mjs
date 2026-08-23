import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowPortfolioExecutionTracePersistenceCoordinator,
  InMemoryAutonomousWorkflowPortfolioExecutionTraceStore,
  JsonAutonomousWorkflowPortfolioExecutionTracePersistence,
  TransactionalJsonAutonomousWorkflowPortfolioExecutionTracePersistence,
  WebStorageAutonomousWorkflowPortfolioExecutionTraceTextStore,
  LLMRuntime,
  CredentialStore,
  InMemoryAutonomousModelHealthStore,
  openaiCompatibleProvider,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
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

function runtime(onRequest = () => {}) {
  const value = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  value.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline result for ${request.model}` };
  });
  return value;
}

function agentFor(onRequest = () => {}) {
  const value = new AutonomousAgent(runtime(onRequest));
  value.registerModel(model);
  return value;
}

function providerPlanningAgent(onRequest = () => {}) {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      let body = {};
      try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
      const prompt = JSON.stringify(body.messages ?? []);
      onRequest({ kind: prompt.includes("priority_order") ? "planning" : "execution", prompt });
      if (prompt.includes("priority_order") && prompt.includes("review_required")) {
        const contractMessage = (body.messages ?? []).find((message) => String(message.content ?? "").startsWith("Context planning-contract:\n"));
        let contract = {};
        try { contract = JSON.parse(String(contractMessage?.content ?? "").slice("Context planning-contract:\n".length)); } catch { /* bounded fixture fallback */ }
        const ids = (contract.stage_catalogue ?? []).map((stage) => stage.id);
        return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: ids.slice(0, 1), review_required: false, confidence: 0.97, abstain: false }) }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
      }
      return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "portfolio execution result" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("portfolio-planner", "https://portfolio-planner.test", { requiresCredential: false }));
  const health = new InMemoryAutonomousModelHealthStore();
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  agent.registerModel({ ...model, provider: "portfolio-planner", model: "portfolio-planner-model" });
  return { agent, health };
}

function allDomainRequests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `portfolio-${domain}`,
    task: `private task payload for ${domain} must remain transient`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`portfolio-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
    hints: [`private hint for ${domain}`],
  }));
}

test("portfolio execution runs every domain in dependency waves and hands off bounded transient output", async () => {
  const providerRequests = [];
  const agent = agentFor((request) => providerRequests.push(request));
  const requests = allDomainRequests();
  const result = await agent.executeWorkflowPortfolio(requests, {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    maxParallelism: 3,
  });

  assert.equal(result.status, "completed");
  assert.equal(result.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(result.items.every((item) => item.status === "succeeded" && item.run?.status === "completed"));
  assert.equal(providerRequests.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.match(JSON.stringify(providerRequests[1]), /offline result for offline-model/);
  assert.equal(result.toJSON().completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.toJSON().failed_count, 0);
  assert.doesNotMatch(JSON.stringify(result), /private task payload|private hint|offline result/);
  assert.equal(result.items[0].outputBytes > 0, true);
  assert.equal(result.items[0].outputDigest.length, 64);
});

test("portfolio execution emits a hash-chained decision trace without transient values", async () => {
  const events = [];
  const agent = agentFor();
  const result = await agent.executeWorkflowPortfolio(allDomainRequests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    traceId: "portfolio-trace-1",
    traceSink: (event) => { events.push(event); },
  });

  assert.equal(result.status, "completed");
  assert.equal(result.traceDigest, events.at(-1).event_digest);
  assert.equal(result.toJSON().trace_digest, result.traceDigest);
  assert.equal(events[0].phase, "started");
  assert.equal(events[1].phase, "plan_verified");
  assert.equal(events.filter((event) => event.phase === "item_started").length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(events.filter((event) => event.phase === "item_decided").length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(events.at(-1).phase, "completed");
  assert.equal(events.every((event, index) => event.sequence === index + 1 && (index === 0 ? event.previous_digest === "" : event.previous_digest === events[index - 1].event_digest)), true);
  assert.doesNotMatch(JSON.stringify(events), /private task payload|private hint|offline result/);
});

test("portfolio decision traces restore through bounded JSON and CAS persistence", async () => {
  const agent = agentFor();
  const requests = allDomainRequests();
  const plan = await agent.planWorkflowPortfolio(requests, { requireAllDomains: true });
  const sourceStore = new InMemoryAutonomousWorkflowPortfolioExecutionTraceStore({ traceId: "durable-portfolio-trace", planDigest: plan.portfolio_digest });
  const execution = await agent.executeWorkflowPortfolio(requests, {
    plan,
    approveProviderCall: true,
    traceId: "durable-portfolio-trace",
    traceSink: (event) => sourceStore.append(event),
  });
  const snapshot = sourceStore.snapshot();
  assert.equal(snapshot.head_digest, execution.traceDigest);
  assert.equal(sourceStore.verifyIntegrity().verified, true);

  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousWorkflowPortfolioExecutionTracePersistence(textStore);
  const coordinator = new AutonomousWorkflowPortfolioExecutionTracePersistenceCoordinator(sourceStore, persistence);
  await coordinator.flush();
  const restoredStore = new InMemoryAutonomousWorkflowPortfolioExecutionTraceStore({ traceId: "durable-portfolio-trace", planDigest: plan.portfolio_digest });
  const restoredCoordinator = new AutonomousWorkflowPortfolioExecutionTracePersistenceCoordinator(restoredStore, persistence);
  const restored = await restoredCoordinator.restore();
  assert.equal(restored.snapshot_digest, snapshot.snapshot_digest);
  assert.deepEqual(restoredStore.snapshot(), snapshot);

  const stale = new AutonomousWorkflowPortfolioExecutionTracePersistenceCoordinator(new InMemoryAutonomousWorkflowPortfolioExecutionTraceStore({ traceId: "durable-portfolio-trace", planDigest: plan.portfolio_digest }), persistence);
  await stale.restore();
  const competingStore = new InMemoryAutonomousWorkflowPortfolioExecutionTraceStore({ traceId: "competing-portfolio-trace", planDigest: plan.portfolio_digest });
  await new JsonAutonomousWorkflowPortfolioExecutionTracePersistence(textStore).write(competingStore.snapshot());
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const tampered = structuredClone(snapshot);
  tampered.events[0].status = "failed";
  assert.throws(() => restoredStore.restore(tampered), /digest|hash chain|invalid/);
  assert.deepEqual(restoredStore.snapshot(), snapshot);

  const values = new Map();
  const browserStore = new WebStorageAutonomousWorkflowPortfolioExecutionTraceTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  }, "portfolio-trace-key");
  const browserPersistence = new JsonAutonomousWorkflowPortfolioExecutionTracePersistence(browserStore);
  await browserPersistence.write(snapshot);
  assert.equal((await browserPersistence.read()).snapshot_digest, snapshot.snapshot_digest);
});

test("portfolio execution fails closed at the provider approval boundary", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const result = await agent.executeWorkflowPortfolio(allDomainRequests(), { planOptions: { requireAllDomains: true } });

  assert.equal(result.status, "approval_required");
  assert.equal(providerCalls, 0);
  const first = result.items.find((item) => item.itemId === "portfolio-coding");
  assert.equal(first?.status, "approval_required");
  assert.equal(first?.run?.status, "approval_required");
  assert.equal(result.items.filter((item) => item.status === "blocked").length, AUTONOMOUS_DOMAIN_NAMES.length - 1);
  assert.equal(result.toJSON().approval_required_count, 1);
  assert.equal(result.toJSON().blocked_count, AUTONOMOUS_DOMAIN_NAMES.length - 1);
});

test("portfolio execution blocks descendants and can omit later waves after a hard failure", async () => {
  const agent = agentFor();
  const originalRun = agent.run.bind(agent);
  agent.run = async (task, options) => {
    if (task === "force portfolio failure") throw new Error("synthetic provider boundary failure");
    return originalRun(task, options);
  };
  const result = await agent.executeWorkflowPortfolio([
    { id: "a", task: "force portfolio failure", domain: "coding" },
    { id: "b", task: "dependent work must not run", domain: "data", dependsOn: ["a"] },
    { id: "c", task: "independent work may finish in the current wave", domain: "science" },
  ], { approveProviderCall: true, stopOnError: true, maxParallelism: 1 });

  const byId = new Map(result.items.map((item) => [item.itemId, item]));
  assert.equal(result.status, "partial");
  assert.equal(byId.get("a").status, "failed");
  assert.equal(byId.get("c").status, "succeeded");
  assert.equal(byId.get("b").status, "omitted");
  assert.equal(byId.get("b").run, null);
});

test("portfolio execution rejects a drifted reviewed plan before provider dispatch", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const requests = [{ id: "reviewed", task: "original task", domain: "coding" }];
  const plan = await agent.planWorkflowPortfolio(requests);

  await assert.rejects(
    () => agent.executeWorkflowPortfolio([{ ...requests[0], task: "drifted task" }], { plan, approveProviderCall: true }),
    /plan verification failed/,
  );
  assert.equal(providerCalls, 0);
});

test("portfolio provider planning is review-gated, accepted per item, and learns planner quality separately across every domain", async () => {
  const calls = [];
  const { agent, health } = providerPlanningAgent((event) => calls.push(event));
  const requests = allDomainRequests().map((request) => ({ ...request, task: `${request.task} provider planning review` }));
  const plan = await agent.planWorkflowPortfolio(requests, { requireAllDomains: true });
  const learning = new AutonomousLearningController(agent);
  const review = await agent.executeWorkflowPortfolio(requests, {
    plan,
    providerPlanning: { candidates: agent.models(), approveProviderCall: true },
    approveProviderCall: true,
  });

  assert.equal(review.status, "plan_review_required");
  assert.equal(review.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(review.items.every((item) => item.planningStatus === "plan_review_required" && item.planRefinement?.status === "completed"));
  assert.equal(calls.filter((event) => event.kind === "planning").length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(calls.filter((event) => event.kind === "execution").length, 0);
  assert.doesNotMatch(JSON.stringify(review.toJSON()), /private task payload|provider planning review|portfolio execution result/);

  const executed = await agent.executeWorkflowPortfolio(requests, {
    plan,
    providerPlanning: { candidates: agent.models(), approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    learning,
    evaluateItem: () => ({ evaluator_id: "portfolio-execution-reviewer", evaluator_version: "1", reward: 0.81, passed: true }),
    evaluatePlanningItem: () => ({ evaluator_id: "portfolio-planner-reviewer", evaluator_version: "1", reward: 0.93, passed: true }),
  });

  assert.equal(executed.status, "completed");
  assert.ok(executed.items.every((item) => item.status === "succeeded" && item.planningStatus === "accepted" && item.plannerLearningStatus === "settled" && item.learningStatus === "settled"));
  assert.equal(executed.toJSON().planner_settled_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(executed.toJSON().learning_settled_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(calls.filter((event) => event.kind === "execution").length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(health.health({ model: "portfolio-planner-model", capability: "planning" })[0]?.quality_observations, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(executed.toJSON()), /portfolio execution result/);
});
