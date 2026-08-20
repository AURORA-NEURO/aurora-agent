import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  LLMRuntime,
  openaiCompatibleProvider,
  runAutonomousCrossDomainDecisionCycle,
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

test("cross-domain decision cycle settles specialist and synthesis credit as one trajectory", async () => {
  const { agent, calls } = cycleAgent();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence and safety boundaries." },
      { id: "neuro", domain: "neuroscience", task: "Analyze EEG signal design and interpretation limits." },
    ],
    learning: {
      controller: learning,
      trajectoryId: "cross-cycle-1",
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "cross-reviewer", evaluator_version: "1", reward: 0.8, passed: true }])),
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.run.child_runs.length, 2);
  assert.equal(result.run.completed_children, 2);
  assert.equal(result.run.synthesis.response.text, "cycle answer");
  assert.equal(result.learning_episode_ids.length, 3);
  assert.equal(Object.keys(result.evaluation).length, 3);
  assert.equal(result.settlement.trajectory.trajectory.status, "settled");
  assert.equal(result.settlement.trajectory.settlements.length, 3);
  assert.equal(result.settlement.trajectory.settlements.at(-1).next_state.generation, 3);
  assert.equal(calls(), 3);
});

test("cross-domain decision cycle applies semantic routing before fan-out and preserves both gates", async () => {
  const semantic = cycleAgent([
    { route: { selected_domains: [{ domain: "biomedical", score: 0.93, rationale: "biomedical evidence" }, { domain: "neuroscience", score: 0.91, rationale: "EEG study" }], confidence: 0.92, abstain: false, abstain_reason: null } },
    { text: "biomedical specialist" },
    { text: "neuroscience specialist" },
    { text: "integrated synthesis" },
  ]);
  const result = await runAutonomousCrossDomainDecisionCycle(semantic.agent, "Help with an unfamiliar biomedical neuroscience study.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true },
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(result.semantic_route.status, "completed");
  assert.equal(result.route.source, "provider_semantic_hybrid");
  assert.deepEqual(result.run.child_runs.map((child) => child.domain), ["biomedical", "neuroscience"]);
  assert.equal(result.run.synthesis.response.text, "integrated synthesis");
  assert.equal(semantic.calls(), 4);

  const gatedSemantic = cycleAgent([{ route: { selected_domains: [{ domain: "biomedical", score: 0.9, rationale: "bio" }, { domain: "neuroscience", score: 0.9, rationale: "neuro" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  const semanticGate = await runAutonomousCrossDomainDecisionCycle(gatedSemantic.agent, "an unfamiliar biomedical neuroscience study", { approveProviderCall: true, semanticRouting: { enabled: true, approveProviderCall: false } });
  assert.equal(semanticGate.status, "approval_required");
  assert.equal(gatedSemantic.calls(), 0);

  const gatedExecution = cycleAgent();
  const executionGate = await runAutonomousCrossDomainDecisionCycle(gatedExecution.agent, "biomedical neuroscience", { approveProviderCall: false, synthesize: false, subtasks: [{ id: "bio", domain: "biomedical", task: "bio" }, { id: "neuro", domain: "neuroscience", task: "neuro" }] });
  assert.equal(executionGate.status, "approval_required");
  assert.equal(gatedExecution.calls(), 0);
});

test("cross-domain decision cycle settles partial specialist trajectories without inventing synthesis", async () => {
  const { agent, calls } = cycleAgent();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "biomedical neuroscience", {
    approveProviderCall: true,
    synthesize: false,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "bio" },
      { id: "neuro", domain: "neuroscience", task: "neuro" },
    ],
    learning: {
      controller: learning,
      trajectoryId: "cross-cycle-specialists-only",
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "specialist-reviewer", evaluator_version: "1", reward: 0.7, passed: true }])),
    },
  });
  assert.equal(result.status, "children_completed");
  assert.equal(result.run.synthesis, null);
  assert.equal(result.learning_episode_ids.length, 2);
  assert.equal(result.settlement.trajectory.settlements.length, 2);
  assert.equal(calls(), 2);
});

test("cross-domain fan-out accepts a representative pair for every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const { agent, calls } = cycleAgent();
  for (let index = 0; index < domains.length; index += 1) {
    const left = domains[index];
    const right = domains[(index + 1) % domains.length];
    const task = `${left} ${right}`;
    const route = await agent.route(task, { maxDomains: 2, minMargin: 0.2, allowCrossDomain: true });
    assert.equal(route.cross_domain, true, `${left}/${right} should fan out`);
    const result = await runAutonomousCrossDomainDecisionCycle(agent, task, {
      routeOverride: route,
      approveProviderCall: true,
      synthesize: false,
      subtasks: [
        { id: "left", domain: left, task: `${left} specialist review` },
        { id: "right", domain: right, task: `${right} specialist review` },
      ],
    });
    assert.equal(result.status, "children_completed", `${left}/${right} should complete`);
    assert.deepEqual(result.run.child_runs.map((child) => child.domain), [left, right]);
    assert.equal(result.run.synthesis, null);
  }
  assert.equal(calls(), domains.length * 2);
});
