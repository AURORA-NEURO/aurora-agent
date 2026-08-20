import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  LLMRuntime,
  openaiCompatibleProvider,
  runAutonomousDecisionCycle,
} from "../dist/index.js";

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

const capabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation"];

function candidate() {
  return {
    provider: "cycle-provider",
    model: "cycle-model",
    capabilities,
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function cycleAgent(payloads = [{ text: "cycle answer" }]) {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      const payload = payloads[Math.min(calls, payloads.length - 1)];
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: payload.route ? JSON.stringify(payload.route) : payload.text }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, bodies, calls: () => calls };
}

test("decision cycle connects approval, invocation, evaluator settlement, and bandit adaptation", async () => {
  const { agent, calls } = cycleAgent();
  const learning = new AutonomousLearningController(agent);
  const task = "Debug this coding repository and report the verified tests.";
  const result = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    approveProviderCall: true,
    learning: {
      controller: learning,
      episodeId: "cycle-coding-1",
      evaluate: (run) => ({ evaluator_id: "coding-reviewer", evaluator_version: "1", reward: run.response?.text === "cycle answer" ? 0.9 : 0, passed: true }),
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.run.response.text, "cycle answer");
  assert.equal(result.learning_episode_id, "cycle-coding-1");
  assert.equal(result.evaluation.reward, 0.9);
  assert.equal(result.settlement.episode.status, "settled");
  assert.equal(result.settlement.next_state.generation, 1);
  assert.equal(calls(), 1);
  assert.equal(JSON.stringify(result.settlement).includes(task), false);
});

test("decision cycle keeps semantic routing, provider approval, and disagreement as separate gates", async () => {
  const semantic = cycleAgent([
    { route: { selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.94, abstain: false, abstain_reason: null } },
    { text: "semantic cycle answer" },
  ]);
  const result = await runAutonomousDecisionCycle(semantic.agent, "Help with an unfamiliar technical migration.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.semantic_route.status, "completed");
  assert.equal(result.route.source, "provider_semantic_hybrid");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.run.response.text, "semantic cycle answer");
  assert.equal(semantic.calls(), 2);

  const disagreement = cycleAgent([
    { route: { selected_domains: [{ domain: "biomedical", score: 0.95, rationale: "clinical wording" }], confidence: 0.95, abstain: false, abstain_reason: null } },
  ]);
  const refused = await runAutonomousDecisionCycle(disagreement.agent, "Debug this Rust repository and report the tests.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true },
  });
  assert.equal(refused.status, "provider_disagreement");
  assert.equal(refused.run, null);
  assert.equal(disagreement.calls(), 1);
});

test("decision cycle requires both semantic and execution approvals before any provider call", async () => {
  const gatedSemantic = cycleAgent([{ route: { selected_domains: [{ domain: "coding", score: 0.9, rationale: "code" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  const semanticGate = await runAutonomousDecisionCycle(gatedSemantic.agent, "an unfamiliar task", { semanticRouting: { enabled: true, approveProviderCall: false }, approveProviderCall: true });
  assert.equal(semanticGate.status, "approval_required");
  assert.equal(gatedSemantic.calls(), 0);

  const providerGate = cycleAgent();
  const providerResult = await runAutonomousDecisionCycle(providerGate.agent, "Debug this coding repository.", { domain: "coding", approveProviderCall: false });
  assert.equal(providerResult.status, "approval_required");
  assert.equal(providerGate.calls(), 0);
});

test("route handoff refuses a route from a different task", async () => {
  const { agent, calls } = cycleAgent();
  const route = await agent.route("Debug this coding repository.", { domain: "coding" });
  await assert.rejects(
    agent.run("Review this biomedical evidence.", { routeOverride: route, approveProviderCall: true }),
    /does not match the task digest/,
  );
  assert.equal(calls(), 0);
});

test("decision cycle executes every built-in domain through the same reviewed path", async () => {
  const { agent, calls } = cycleAgent();
  const examples = {
    coding: "debug this code repository",
    browser: "research and compare browser sources",
    data: "analyze this dataset lineage",
    science: "design a reproducible experiment",
    biomedical: "review biomedical evidence with safety boundaries",
    neuroscience: "analyze an EEG signal study",
    operations: "plan an incident rollback",
    enterprise: "review enterprise governance compliance",
    multi_agent: "delegate a bounded subtask to another agent",
    multimodal: "inspect an image and document together",
    cross_domain: "synthesize domain evidence",
    evaluation: "run a benchmark replay and failure analysis",
  };
  for (const [domain, task] of Object.entries(examples)) {
    const result = await runAutonomousDecisionCycle(agent, task, { domain, approveProviderCall: true });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.route.primary_domain, domain);
    assert.equal(result.run.blueprint.domain_profile.domain, domain);
  }
  assert.equal(calls(), 12);
});
