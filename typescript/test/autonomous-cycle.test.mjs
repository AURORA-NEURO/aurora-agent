import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  LLMRuntime,
  openaiCompatibleProvider,
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousDecisionCycle,
  runAutonomousReplanCycle,
} from "../dist/index.js";

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

const capabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];
const loopTools = [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }];

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

function toolLoopAgent(stopResponses = 0) {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      if (calls <= stopResponses) return jsonResponse({ choices: [{ message: { role: "assistant", content: "child answer" }, finish_reason: "stop" }] });
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: `cycle-tool-${calls}`, type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-tool-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
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

test("decision cycle preserves structured output and caller selection policy", async () => {
  const cycle = cycleAgent([{ text: JSON.stringify({ answer: "cycle-structured" }) }]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await runAutonomousDecisionCycle(cycle.agent, "Return a structured coding result.", {
    domain: "coding",
    approveProviderCall: true,
    maxCostPerMillionTokens: 5,
    maxLatencyMs: 100,
    minQuality: 0.9,
    requireJson: true,
    responseSchema,
  });
  assert.equal(result.status, "completed");
  assert.deepEqual(result.run.response.structured, { answer: "cycle-structured" });
  assert.deepEqual(cycle.bodies[0].response_format, { type: "json_object" });
});

test("decision cycle preserves bounded tool-loop exhaustion without evaluator settlement", async () => {
  const { agent, calls } = toolLoopAgent();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousDecisionCycle(agent, "Review this coding repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: loopTools,
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    learning: {
      controller: learning,
      episodeId: "cycle-tool-limit",
      evaluate: () => { throw new Error("tool-limit result must not be evaluated"); },
    },
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.run.status, "turn_limit_reached");
  assert.equal(result.run.tool_loop.status, "turn_limit_reached");
  assert.equal(result.learning_episode_id, null);
  assert.equal(result.evaluation, null);
  assert.equal(result.settlement, null);
  assert.equal(calls(), 4);
});

test("decision cycle fails the shared execution when post-run evaluation throws", async () => {
  const { agent, calls } = cycleAgent();
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-post-run-failure-1", domain: "coding", capability: "code_review", riskClass: "read_only", journal: new InMemoryAutonomousExecutionJournal() });
  const learning = new AutonomousLearningController(agent);
  await assert.rejects(
    runAutonomousDecisionCycle(agent, "Review this coding change", {
      domain: "coding",
      approveProviderCall: true,
      execution,
      learning: { controller: learning, episodeId: "cycle-post-run-failure-episode", evaluate: async () => { throw new Error("post-run evaluator unavailable"); } },
    }),
    /post-run evaluator unavailable/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("replan cycle feeds bounded evaluator guidance into the next attempt and settles each attempt", async () => {
  const { agent, bodies, calls } = cycleAgent([{ text: "first answer" }, { text: "verified answer" }]);
  const learning = new AutonomousLearningController(agent);
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-execution-1", domain: "coding", capability: "code_review", riskClass: "read_only", journal: new InMemoryAutonomousExecutionJournal() });
  let evaluations = 0;
  const result = await runAutonomousReplanCycle(agent, "Debug this coding repository and report the verified tests.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 1,
    execution,
    evaluate: () => {
      evaluations += 1;
      return evaluations === 1
        ? { evaluator_id: "coding-reviewer", evaluator_version: "2", reward: 0.25, passed: false, failed: true, replan_requested: true, replan_instruction: "Add explicit verification evidence before concluding.", evidence_digest: "a".repeat(64) }
        : { evaluator_id: "coding-reviewer", evaluator_version: "2", reward: 0.95, passed: true, failed: false, replan_requested: false, evidence_digest: "b".repeat(64) };
    },
    learning: { controller: learning, episodePrefix: "cycle-replan" },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.replan_count, 1);
  assert.equal(result.attempts.length, 2);
  assert.equal(result.attempts[0].evaluation.replan_requested, true);
  assert.equal(result.attempts[1].evaluation.replan_requested, false);
  assert.equal(result.learning_episode_ids.length, 2);
  assert.equal(result.settlements.length, 2);
  assert.equal(result.settlements.at(-1).next_state.generation, 2);
  assert.equal(execution.state.status, "completed");
  assert.equal(execution.state.provider_calls, 2);
  assert.equal(execution.state.replans, 1);
  assert.equal(calls(), 2);
  assert.match(JSON.stringify(bodies[1]), /autonomous-replan-2/);
  assert.match(JSON.stringify(bodies[1]), /Add explicit verification evidence/);
  assert.equal(JSON.stringify(result.attempts).includes("Add explicit verification evidence"), false);
});

test("replan cycle preserves one-shot completion and enforces the replan ceiling", async () => {
  const oneShot = cycleAgent();
  const completed = await runAutonomousReplanCycle(oneShot.agent, "Review this coding change.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.replan_count, 0);
  assert.equal(oneShot.calls(), 1);

  const limited = cycleAgent();
  const result = await runAutonomousReplanCycle(limited.agent, "Review this coding change.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "Collect an independent verification witness." }),
  });
  assert.equal(result.status, "replan_limit_reached");
  assert.equal(result.replan_count, 0);
  assert.equal(limited.calls(), 1);
  await assert.rejects(
    runAutonomousReplanCycle(limited.agent, "Review this coding change.", { domain: "coding", maxReplans: 4, evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: false }) }),
    /maxReplans/,
  );
});

test("replan cycle refuses credential-shaped evaluator instructions", async () => {
  const { agent } = cycleAgent();
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change.", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "Use the api_key from the task." }),
    }),
    /credential material/,
  );
});

test("execution policy stops a replanned provider call before dispatch", async () => {
  const { agent, calls } = cycleAgent([{ text: "first answer" }, { text: "must not dispatch" }]);
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-execution-budget-1", domain: "coding", capability: "code_review", riskClass: "read_only", policy: { max_provider_calls: 1 } });
  let evaluations = 0;
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change.", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 1,
      execution,
      evaluate: () => {
        evaluations += 1;
        return { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: evaluations === 1, replan_instruction: evaluations === 1 ? "Collect another independent witness." : null };
      },
    }),
    /max_provider_calls/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
});

test("replan policy failures after evaluation fail the shared execution", async () => {
  const { agent, calls } = cycleAgent([{ text: "first answer" }]);
  const execution = await AutonomousExecutionController.create({ executionId: "replan-policy-failure-1", domain: "coding", capability: "code_review", riskClass: "read_only", policy: { max_replans: 0 }, journal: new InMemoryAutonomousExecutionJournal() });
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 1,
      execution,
      evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "Collect another independent witness." }),
    }),
    /max_replans/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("replan cycle runs the same reviewed path for every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const { agent, calls } = cycleAgent();
  for (const domain of domains) {
    const execution = await AutonomousExecutionController.create({ executionId: `domain-execution-${domain}`, domain, capability: "domain_review", riskClass: "read_only" });
    const result = await runAutonomousReplanCycle(agent, `${domain} review`, {
      domain,
      approveProviderCall: true,
      maxReplans: 0,
      execution,
      evaluate: () => ({ evaluator_id: `${domain}-reviewer`, evaluator_version: "1", reward: 0.75, passed: true, replan_requested: false }),
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.final.run.blueprint.domain_profile.domain, domain);
    assert.equal(execution.state.status, "completed", domain);
    assert.equal(execution.state.provider_calls, 1, domain);
  }
  assert.equal(calls(), domains.length);
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
  const execution = await AutonomousExecutionController.create({ executionId: "cross-execution-1", domain: "cross_domain", capability: "cross_domain_synthesis", riskClass: "review_required", policy: { max_provider_calls: 4 }, journal: new InMemoryAutonomousExecutionJournal() });
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    execution,
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
  assert.equal(execution.state.status, "completed");
  assert.equal(execution.state.provider_calls, 3);
  assert.equal(calls(), 3);
});

test("cross-domain decision cycle propagates structured output through fan-out and synthesis", async () => {
  const cycle = cycleAgent([{ text: JSON.stringify({ answer: "specialist-1" }) }, { text: JSON.stringify({ answer: "specialist-2" }) }, { text: JSON.stringify({ answer: "synthesis" }) }]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await runAutonomousCrossDomainDecisionCycle(cycle.agent, "Return a structured biomedical neuroscience synthesis.", {
    approveProviderCall: true,
    requireJson: true,
    responseSchema,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.deepEqual(result.run.child_runs.map((child) => child.result.response.structured), [{ answer: "specialist-1" }, { answer: "specialist-2" }]);
  assert.deepEqual(result.run.synthesis.response.structured, { answer: "synthesis" });
  assert.deepEqual(cycle.bodies.map((body) => body.response_format), [{ type: "json_object" }, { type: "json_object" }, { type: "json_object" }]);
});

test("cross-domain decision cycle preserves synthesis tool-loop exhaustion", async () => {
  const { agent, calls } = toolLoopAgent(2);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    tools: loopTools,
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.run.status, "turn_limit_reached");
  assert.equal(result.run.synthesis.status, "turn_limit_reached");
  assert.equal(result.run.completed_children, 2);
  assert.equal(calls(), 6);
});

test("cross-domain decision cycle fails the shared execution when settlement throws", async () => {
  const { agent, calls } = cycleAgent();
  const execution = await AutonomousExecutionController.create({ executionId: "cross-post-run-failure-1", domain: "cross_domain", capability: "cross_domain_synthesis", riskClass: "review_required", policy: { max_provider_calls: 4 }, journal: new InMemoryAutonomousExecutionJournal() });
  const learning = new AutonomousLearningController(agent);
  await assert.rejects(
    runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
      approveProviderCall: true,
      execution,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
      ],
      learning: {
        controller: learning,
        trajectoryId: "cross-post-run-failure",
        evaluate: async () => { throw new Error("cross-domain evaluator unavailable"); },
      },
    }),
    /cross-domain evaluator unavailable/,
  );
  assert.equal(calls(), 3);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
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
