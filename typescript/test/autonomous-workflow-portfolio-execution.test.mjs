import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  LLMRuntime,
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
