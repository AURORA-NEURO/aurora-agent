import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousDomainToolRegistry,
  AutonomousDomainToolRuntime,
  AutonomousOnlineLearner,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  assembleAutonomousPrompt,
  compileAutonomousPlan,
  openaiCompatibleProvider,
  routeAutonomousTask,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const candidate = (provider, model, capabilities = ["reasoning", "code"]) => ({
  provider,
  model,
  capabilities,
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
});

test("all twelve built-in domains expose profiles, workflows, tools, and deterministic routing", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  assert.equal(profiles.length, 12);
  assert.equal(new Set(profiles.map((profile) => profile.domain)).size, 12);
  const examples = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review this patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  for (const [domain, task] of Object.entries(examples)) {
    const route = await routeAutonomousTask(task);
    assert.equal(route.abstained, false, `${domain} should route`);
    assert.equal(route.primary_domain, domain);
    assert.equal(route.route_digest.length, 64);
    const profile = profiles.find((row) => row.domain === domain);
    assert.ok(profile);
    assert.ok(profile.workflow.stages.length >= 4);
    assert.ok(profile.tool_profile.bindings.length >= 10);
    assert.equal(profile.workflow.workflow_digest.length, 64);
  }
});

test("routing abstains on weak evidence and permits explicit cross-domain review", async () => {
  const unknown = await routeAutonomousTask("please help me with something");
  assert.equal(unknown.abstained, true);
  assert.equal(unknown.reason, "no_matching_evidence");

  const cross = await routeAutonomousTask("research a biomedical neuroscience experiment with EEG patient evidence", { allowCrossDomain: true });
  assert.equal(cross.abstained, false);
  assert.equal(cross.cross_domain, true);
  assert.ok(cross.selected_domains.length >= 2);
  assert.equal(cross.reason, "cross_domain");
});

test("prompt and plan construction preserve budgets, omissions, dependencies, and digests", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((row) => row.domain === "coding");
  assert.ok(profile);
  const prompt = await assembleAutonomousPrompt(profile, "Implement and verify the requested change.", {
    maxInputTokens: 512,
    context: [
      { id: "required", content: "This small required acceptance criterion must remain.", required: true, priority: 10 },
      { id: "optional-large", content: "optional evidence ".repeat(300), priority: 1 },
    ],
  });
  assert.equal(prompt.complete, false);
  assert.deepEqual(prompt.included_context_ids, ["required"]);
  assert.deepEqual(prompt.omitted_context_ids, ["optional-large"]);
  assert.equal(prompt.prompt_digest.length, 64);

  const plan = await compileAutonomousPlan(profile, "Implement and verify the requested change.", {
    taskDigest: "a".repeat(64),
    activeToolNames: ["repository_catalog"],
    selectedToolNames: ["repository_catalog"],
  });
  assert.deepEqual(plan.ordered_step_ids, profile.workflow.stages.map((stage) => stage.id));
  assert.equal(plan.steps[1].depends_on[0], plan.steps[0].id);
  assert.equal(plan.steps[0].tool, "provider.invoke");
  assert.equal(plan.steps[1].tool, "repository_catalog");
  assert.equal(plan.plan_digest.length, 64);
});

test("live catalogue binding covers every domain and effectful tools remain approval-gated", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => {
    const binding = profile.tool_profile.bindings[0];
    return [{ name: binding.name, description: `Test ${binding.name}`, inputSchema: { type: "object", additionalProperties: true } }];
  }).map((definition) => [definition.name, definition])).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  assert.equal(plan.coverage.length, 12);
  assert.equal(plan.domains.length, 12);
  assert.equal(plan.available_curated_tools.length, definitions.length);
  assert.equal(plan.secret_material, "never_returned");

  const coding = profiles.find((profile) => profile.domain === "coding");
  const effectfulDefinition = coding.tool_profile.bindings.find((binding) => binding.name === "agent_mission");
  const effectfulCatalogue = await ToolCatalogue.fromDefinitions([{ name: effectfulDefinition.name, description: "Effectful", inputSchema: { type: "object", additionalProperties: true } }]);
  const effectfulRegistry = await AutonomousDomainToolRegistry.create(effectfulCatalogue, [coding.tool_profile]);
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(effectfulRegistry, async () => { executions += 1; return { ok: true }; });
  const refused = await runtime.authorizeAndExecute([{ id: "call-1", name: "agent_mission", arguments: {} }], { domains: ["coding"], approveEffects: false });
  assert.equal(refused[0].approved, false);
  assert.equal(executions, 0);
  const approved = await runtime.authorizeAndExecute([{ id: "call-2", name: "agent_mission", arguments: {} }], { domains: ["coding"], approveEffects: true });
  assert.equal(approved[0].approved, true);
  assert.equal(executions, 1);
});

test("AutonomousAgent performs a real selected-provider tool loop with domain policy", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      if (calls === 1) return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "tool-1", type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "repository inspected" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://autonomous.test", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const definition = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings.find((binding) => binding.name === "repository_catalog");
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: definition.name, description: "Inspect repository", inputSchema: { type: "object", additionalProperties: true } }]);
  const agent = new AutonomousAgent(llm, {
    toolCatalogue: catalogue,
    toolExecutor: async (tool) => ({ tool: tool.name, files: ["README.md"] }),
  });
  agent.registerModel(candidate("local", "local-model"));
  const result = await agent.run("Debug this Rust repository and report the tests", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.tool_loop.toolCalls, 1);
  assert.equal(result.response.text, "repository inspected");
  assert.equal(bodies[1].messages.at(-1).role, "tool");
  assert.equal(bodies[1].messages.at(-1).content, JSON.stringify({ tool: "repository_catalog", files: ["README.md"] }));
});

test("AutonomousAgent preserves authorization pauses instead of reporting tool success", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "approval-tool-1", type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("approval-loop", "https://approval-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("approval-loop", "approval-model"));
  const result = await agent.run("Review this repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }],
    authorizeAndExecute: async () => [{ callId: "approval-tool-1", approved: false, isError: true, content: { status: "authorization_required", secret_material: "never_returned" } }],
  });
  assert.equal(result.status, "approval_required");
  assert.equal(result.tool_loop.status, "authorization_required");
});

test("AutonomousAgent reports bounded tool-loop exhaustion instead of completed", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: `limit-tool-${calls}`, type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("limit-loop", "https://limit-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("limit-loop", "limit-model"));
  const result = await agent.run("Review this repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }],
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.tool_loop.status, "turn_limit_reached");
  assert.equal(result.tool_loop.turns, 4);
  assert.equal(calls, 4);
});

test("cross-domain execution fans out to specialists, gates approval, and synthesizes bounded local outputs", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      const text = calls === 1 ? "biomedical evidence finding" : calls === 2 ? "neuroscience signal finding" : "integrated biomedical-neuroscience conclusion";
      return jsonResponse({ choices: [{ message: { role: "assistant", content: text }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cross", "https://cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const capabilities = ["reasoning", "coordination", "code", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multimodal", "evaluation"];
  agent.registerModel(candidate("cross", "cross-model", capabilities));
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
  const preview = await agent.blueprint(task);
  assert.equal(preview.route.cross_domain, true);
  assert.ok(preview.cross_domain_blueprint);
  assert.equal(preview.cross_domain_blueprint.child_blueprints.length, preview.route.selected_domains.length);
  assert.equal(preview.cross_domain_blueprint.execution, "not_started");
  const gated = await agent.run(task, { candidates: agent.models() });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.cross_domain?.status, "approval_required");
  assert.equal(calls, 0);

  const result = await agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review the biomedical evidence and safety boundary." },
      { id: "neuro", domain: "neuroscience", task: "Analyze the EEG neuroscience design and signal limits." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(result.route.cross_domain, true);
  assert.deepEqual(result.blueprint.child_ids, ["bio", "neuro"]);
  assert.equal(result.child_runs.length, 2);
  assert.equal(result.completed_children, 2);
  assert.equal(result.synthesis.response.text, "integrated biomedical-neuroscience conclusion");
  assert.equal(calls, 3);
  const synthesisBody = bodies[2];
  assert.ok(synthesisBody.messages.some((message) => String(message.content).includes("biomedical evidence finding")));
  assert.ok(synthesisBody.messages.some((message) => String(message.content).includes("neuroscience signal finding")));
});

test("cross-domain fan-out uses bounded concurrency and preserves deterministic child order", async () => {
  let active = 0;
  let maximumActive = 0;
  let started = 0;
  let release;
  let resolveStarted;
  const releaseGate = new Promise((resolve) => { release = resolve; });
  const startedGate = new Promise((resolve) => { resolveStarted = resolve; });
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      active += 1;
      maximumActive = Math.max(maximumActive, active);
      started += 1;
      if (started === 2) resolveStarted();
      await releaseGate;
      active -= 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded specialist result" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("parallel", "https://parallel.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("parallel", "parallel-model", ["reasoning", "science", "coordination", "biomedical", "neuroscience"]));
  const runPromise = agent.runCrossDomain("Research a biomedical neuroscience study", {
    candidates: agent.models(),
    approveProviderCall: true,
    synthesize: false,
    maxParallelChildren: 2,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience signals." },
    ],
  });
  const observedParallelism = await Promise.race([
    startedGate.then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), 250)),
  ]);
  release();
  const result = await runPromise;
  assert.equal(observedParallelism, true);
  assert.equal(maximumActive, 2);
  assert.equal(result.status, "children_completed");
  assert.deepEqual(result.child_runs.map((child) => child.id), ["bio", "neuro"]);
  assert.equal(result.completed_children, 2);
  await assert.rejects(
    agent.runCrossDomain("Research a biomedical neuroscience study", {
      candidates: agent.models(),
      approveProviderCall: true,
      synthesize: false,
      maxParallelChildren: 5,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review neuroscience signals." },
      ],
    }),
    (error) => error?.name === "ArgumentError",
  );
});

test("online learner adapts only from explicit evaluator rewards", async () => {
  const learner = new AutonomousOnlineLearner();
  const request = {
    task: "choose a reasoning model",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    candidates: [candidate("a", "one"), candidate("b", "two")],
    provider_health: {
      a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
      b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
    },
    model_health: {},
  };
  const first = learner.select(request);
  assert.equal(first.selected_model.provider, "a");
  learner.update({ arm_id: "b/two", reward: 1 });
  learner.update({ arm_id: "a/one", reward: 0.1 });
  const second = learner.select(request);
  assert.equal(second.selected_model.provider, "b");
  assert.equal(learner.snapshot().generation, 2);
});

test("online learner does not double-credit a replayed evaluator outcome", async () => {
  const learner = new AutonomousOnlineLearner();
  const outcomeDigest = "a".repeat(64);
  const first = learner.update({ arm_id: "a/one", reward: 0.8, outcome_digest: outcomeDigest });
  const replay = learner.update({ arm_id: "a/one", reward: 0.8, outcome_digest: outcomeDigest });
  assert.equal(first.generation, 1);
  assert.deepEqual(replay, first);
  assert.equal(replay.arms[0].pulls, 1);
  assert.deepEqual(replay.credited_outcomes, [{ outcome_digest: outcomeDigest, arm_id: "a/one", reward: 0.8, failed: false, contract_digest: null }]);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.1, outcome_digest: outcomeDigest }), /contradictory evaluator evidence/);
});

test("contextual selector bridge sends only model and health metadata to the control plane", async () => {
  let received;
  const apiClient = {
    async brainModelSelectContextual(args) {
      received = args;
      return { ok: true, mcp: { result: { structuredContent: { selection: { selected_model_id: "remote/remote-model", selection_status: "selected" } } } } };
    },
  };
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "remote answer" }, finish_reason: "stop" }] }) });
  llm.registerProvider(openaiCompatibleProvider("remote", "https://remote.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { apiClient });
  agent.registerModel(candidate("remote", "remote-model"));
  const result = await agent.run("Implement this code change", { domain: "coding", approveProviderCall: true });
  assert.equal(result.response.text, "remote answer");
  assert.equal(received.context.domain, "coding");
  assert.equal(received.base.models[0].model_id, "remote/remote-model");
  assert.equal(received.base.models[0].provider, "remote");
  assert.equal(received.base.models[0].authorization, undefined);
});

test("contextual selector abstains on an ambiguous model-only id without dispatch", async () => {
  let calls = 0;
  const apiClient = {
    async brainModelSelectContextual() {
      return { ok: true, mcp: { result: { structuredContent: { selection: { selected_model_id: "shared-model", selection_status: "selected" } } } } };
    },
  };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("provider-a", "https://provider-a.test", { requiresCredential: false }));
  llm.registerProvider(openaiCompatibleProvider("provider-b", "https://provider-b.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { apiClient });
  agent.registerModel(candidate("provider-a", "shared-model"));
  agent.registerModel(candidate("provider-b", "shared-model"));
  await assert.rejects(agent.run("Implement this code change", { domain: "coding", approveProviderCall: true }), /ambiguous model id/);
  assert.equal(calls, 0);
});
