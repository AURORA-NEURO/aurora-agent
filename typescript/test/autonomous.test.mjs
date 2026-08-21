import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousCapabilityActivation,
  AutonomousCapabilityActivationPersistenceCoordinator,
  AutonomousCapabilityActivationStore,
  AutonomousCostBudgetError,
  AutonomousDomainToolRegistry,
  AutonomousDomainToolRuntime,
  AutonomousOnlineLearner,
  AUTONOMOUS_READINESS_SCHEMA,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  assembleAutonomousPrompt,
  compileAutonomousPlan,
  digestCanonicalJsonTextSync,
  digestJson,
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

const learningContextDigest = (context) => digestCanonicalJsonTextSync(JSON.stringify({
  domain: context.domain,
  capability: context.capability,
  risk_class: context.risk_class,
  task_family: context.task_family ?? null,
}));

test("synchronous control-plane SHA-256 matches the standard digest", () => {
  assert.equal(digestCanonicalJsonTextSync("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
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

test("provider planning is approval-gated, dependency-closed, and domain-neutral", async () => {
  const calls = [];
  const allCapabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
      const ids = (contract.stage_catalogue ?? contract.child_catalogue).map((row) => row.id);
      const focusField = contract.stage_catalogue ? "focus_stage_ids" : "focus_child_ids";
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.91, abstain: false }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("planner", "https://planner.test", { requiresCredential: false, structuredOutputMode: "json_schema" }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("planner", "planner-model", allCapabilities));

  const blueprint = await agent.blueprint("Debug this coding repository and report verified tests.", { domain: "coding" });
  assert.ok(blueprint.blueprint);
  const refused = await agent.planWithProvider(blueprint.blueprint);
  assert.equal(refused.status, "approval_required");
  assert.equal(calls.length, 0);
  assert.doesNotMatch(JSON.stringify(refused), /Debug this coding repository/);

  const planned = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true });
  assert.equal(planned.status, "completed");
  assert.deepEqual(planned.priority_stage_ids, blueprint.blueprint.workflow.stages.map((stage) => stage.id));
  assert.equal(planned.focus_stage_ids.length, 1);
  assert.equal(planned.planner_prompt_digest.length, 64);
  assert.equal(planned.selection_digest.length, 64);
  assert.equal(calls[0].response_format.type, "json_schema");

  const crossBlueprint = await agent.blueprint("Write Python code for this dataset pipeline.");
  assert.ok(crossBlueprint.cross_domain_blueprint);
  const cross = await agent.planCrossDomainWithProvider(crossBlueprint.cross_domain_blueprint, { approveProviderCall: true });
  assert.equal(cross.status, "completed");
  assert.deepEqual(cross.priority_child_ids, crossBlueprint.cross_domain_blueprint.child_ids);
  assert.equal(cross.planner_prompt_digest.length, 64);
  assert.doesNotMatch(JSON.stringify(cross), /Write Python code/);

  const domains = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  for (const [domain, task] of Object.entries(domains)) {
    const routed = await agent.blueprint(task, { domain });
    if (routed.cross_domain_blueprint) {
      const result = await agent.planCrossDomainWithProvider(routed.cross_domain_blueprint, { approveProviderCall: true });
      assert.equal(result.status, "completed", domain);
    } else {
      assert.ok(routed.blueprint, domain);
      const result = await agent.planWithProvider(routed.blueprint, { approveProviderCall: true });
      assert.equal(result.status, "completed", domain);
    }
  }
});

test("provider planning refuses dependency-invalid proposals without retaining provider output", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const message = body.messages.find((row) => row.content.startsWith("Context planning-contract:\n"));
      const contract = JSON.parse(message.content.slice("Context planning-contract:\n".length));
      const ids = contract.stage_catalogue.map((row) => row.id).reverse();
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: [ids[0]], review_required: false, confidence: 1, abstain: false }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("invalid-planner", "https://invalid-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("invalid-planner", "planner-model", ["reasoning", "code", "structured_output"]));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const result = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true });
  assert.equal(result.status, "provider_disagreement");
  assert.equal(result.review_required, true);
  assert.doesNotMatch(JSON.stringify(result), /provider_private_text/);
});

test("provider planning converts malformed structured output into a digest-only refusal", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const message = body.messages.find((row) => row.content.startsWith("Context planning-contract:\n"));
      const contract = JSON.parse(message.content.slice("Context planning-contract:\n".length));
      const ids = contract.stage_catalogue.map((row) => row.id);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: [ids[0]], review_required: false, confidence: 1, abstain: false, provider_private_text: "must not be projected" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("malformed-planner", "https://malformed-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("malformed-planner", "planner-model", ["reasoning", "code", "structured_output"]));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const result = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true });
  assert.equal(result.status, "provider_invalid");
  assert.equal(result.review_required, true);
  assert.equal(result.planner_plan_digest, null);
  assert.equal(result.outcome_digest.length, 64);
  assert.doesNotMatch(JSON.stringify(result), /provider_private_text/);
});

test("provider planning rejects a broken blueprint dependency closure before dispatch", async () => {
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const malformed = structuredClone(blueprint.blueprint);
  malformed.workflow.stages[0].depends_on = ["missing-stage"];
  await assert.rejects(() => agent.planWithProvider(malformed), /dependencies are not closed/);
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
  await assert.rejects(agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    maxTotalCostUnits: 0,
    synthesize: false,
    subtasks: [
      { id: "bio-budget", domain: "biomedical", task: "Review the biomedical evidence." },
      { id: "neuro-budget", domain: "neuroscience", task: "Review the neuroscience evidence." },
    ],
  }), (error) => error instanceof AutonomousCostBudgetError);
  assert.equal(calls, 3, "aggregate budget refusal must happen before another provider dispatch");
});

test("cross-domain structured output propagates through specialists and synthesis", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: `structured-${calls}` }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("structured-cross", "https://structured-cross.test", { requiresCredential: false, structuredOutputMode: "json_object" }));
  const agent = new AutonomousAgent(llm);
  const model = candidate("structured-cross", "structured-cross-model", ["reasoning", "coordination", "biomedical", "science", "structured_output"]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await agent.runCrossDomain("Research a biomedical neuroscience experiment with EEG patient evidence", {
    candidates: [model],
    approveProviderCall: true,
    requireJson: true,
    responseSchema,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review the biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Analyze the neuroscience signal limits." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(calls, 3);
  assert.deepEqual(result.child_runs.map((child) => child.result.response.structured), [{ answer: "structured-1" }, { answer: "structured-2" }]);
  assert.deepEqual(result.synthesis.response.structured, { answer: "structured-3" });
  assert.deepEqual(bodies.map((body) => body.response_format), [{ type: "json_object" }, { type: "json_object" }, { type: "json_object" }]);
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

test("accepted cross-domain plan refinement reorders bounded fan-out and carries digest metadata", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: `accepted-cross-child-${calls}` }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("accepted-cross", "https://accepted-cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("accepted-cross", "accepted-cross-model", ["reasoning", "coordination", "biomedical", "neuroscience", "science"]));
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
  const preview = await agent.blueprint(task);
  assert.ok(preview.cross_domain_blueprint);
  const blueprint = preview.cross_domain_blueprint;
  const acceptedPlan = {
    schema: "bioprism-python-autonomous-cross-domain-plan-refinement/0.1",
    status: "completed",
    task_digest: blueprint.task_digest,
    base_plan_digest: blueprint.plan_digest,
    priority_child_ids: [...blueprint.child_ids].reverse(),
    focus_child_ids: [blueprint.child_ids.at(-1)],
    review_required: false,
    confidence: 0.94,
    selected_model: { provider: "accepted-cross", model: "accepted-cross-model" },
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "child_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_or_effects_authorized",
  };
  const acceptedPlanDigest = await digestJson(acceptedPlan);
  const result = await agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    synthesize: false,
    maxParallelChildren: 1,
    acceptedCrossDomainPlanRefinement: acceptedPlan,
  });
  assert.equal(result.status, "children_completed");
  assert.deepEqual(result.child_runs.map((child) => child.id), [...blueprint.child_ids].reverse());
  assert.equal(result.plan_refinement_digest, acceptedPlanDigest);
  assert.equal(calls, blueprint.child_ids.length);
  assert.match(bodies[0].messages.find((message) => message.content.startsWith("Context accepted-cross-domain-plan:\n"))?.content ?? "", /priority_rank/);

  const invalidPlan = { ...acceptedPlan, base_plan_digest: "0".repeat(64) };
  await assert.rejects(
    () => agent.runCrossDomain(task, { candidates: agent.models(), approveProviderCall: true, synthesize: false, acceptedCrossDomainPlanRefinement: invalidPlan }),
    /base does not match/,
  );
  assert.equal(calls, blueprint.child_ids.length, "invalid accepted plans must fail before child dispatch");
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
  const constrained = learner.select({ ...request, max_latency_ms: 50 });
  assert.equal(constrained.selected_model, null);
  assert.match(constrained.abstention_reason, /no eligible candidate/);
  assert.equal(constrained.ranking.length, 2);
  assert.equal(constrained.ranking.every((row) => row.eligible === false), true);
  assert.match(constrained.ranking[0].reasons.join(";"), /latency exceeds the caller bound/);
  assert.throws(() => learner.select({ ...request, min_quality: 2 }), /min_quality is outside its bounds/);
});

test("selection confidence abstains on ambiguous ranking across every built-in domain", () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  for (const domain of domains) {
    const learner = new AutonomousOnlineLearner();
    const decision = learner.select({
      task: `choose a model for ${domain}`,
      domain,
      capability: "reasoning",
      risk_class: "review_required",
      required_capabilities: ["reasoning"],
      estimated_input_tokens: 10,
      requested_output_tokens: 50,
      min_selection_confidence: 0.1,
      candidates: [candidate("a", "same-prior"), candidate("b", "same-prior")],
      provider_health: {
        a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
        b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
      },
      model_health: {},
    });
    assert.equal(decision.selected_model, null, domain);
    assert.equal(decision.selection_confidence, 0, domain);
    assert.equal(decision.min_selection_confidence, 0.1, domain);
    assert.match(decision.abstention_reason, /selection confidence/, domain);
  }
  assert.throws(() => learnerSelectConfidenceFailure(), /min_selection_confidence is outside its bounds/);
});

function learnerSelectConfidenceFailure() {
  return new AutonomousOnlineLearner().select({
    task: "invalid confidence",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    min_selection_confidence: 2,
    candidates: [candidate("a", "one")],
    provider_health: { a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true } },
    model_health: {},
  });
}

test("autonomous invocation preserves learner exploration and ranking evidence", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "selected through the learner" }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("a", "https://learner-a.test", { requiresCredential: false }));
  llm.registerProvider(openaiCompatibleProvider("b", "https://learner-b.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, {
    learner: new AutonomousOnlineLearner({ policy: { strategy: "epsilon_greedy", epsilon: 1, seed: 7 } }),
  });
  agent.registerModel(candidate("a", "one"));
  agent.registerModel(candidate("b", "two"));
  agent.learner.update({ arm_id: "a/one", reward: 0.2 });
  agent.learner.update({ arm_id: "b/two", reward: 0.8 });
  const result = await agent.run("Choose a model for this bounded coding task.", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(result.selection.exploration_taken, true);
  assert.equal(typeof result.selection.exploration_draw, "number");
  assert.ok(result.selection.ranking.some((row) => row.reasons.some((reason) => reason.startsWith("history="))));
});

test("online learner honors seeded epsilon exploration, failure penalties, and signed rewards", () => {
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
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "epsilon_greedy", epsilon: 1, seed: 7, failure_penalty: 1 } });
  learner.update({ arm_id: "a/one", reward: -0.5, failed: true, outcome_digest: "5".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 0.8, outcome_digest: "6".repeat(64) });
  const decision = learner.select(request);
  assert.equal(decision.exploration_taken, true);
  assert.match(String(decision.exploration_draw), /^0\./);
  assert.equal(learner.snapshot().policy.strategy, "epsilon_greedy");
  assert.ok(decision.ranking.find((row) => row.provider === "a").reasons.some((reason) => reason.startsWith("failure_rate=")));
  const disabled = new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, policy: { strategy: "ucb1" }, arms: [{ arm_id: "a/one", disabled: true }] } });
  const disabledDecision = disabled.select(request);
  assert.deepEqual(disabledDecision.selected_model, { provider: "b", model: "two" });
  assert.match(disabledDecision.ranking.find((row) => row.provider === "a").reasons.join(";"), /bandit arm is disabled/);
});

test("online learner supports deterministic Thompson posteriors with auditable evidence for every domain", () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  for (const [domainIndex, domain] of domains.entries()) {
    const request = {
      task: `choose a reasoning model for ${domain}`,
      domain,
      capability: "reasoning",
      risk_class: "bounded_review",
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
    const learner = new AutonomousOnlineLearner({ policy: { strategy: "thompson_sampling", seed: 19 } });
    learner.update({ arm_id: "a/one", reward: 0.9, outcome_digest: "7".repeat(63) + domainIndex.toString(16) });
    learner.update({ arm_id: "b/two", reward: -0.5, failed: true, outcome_digest: "8".repeat(63) + domainIndex.toString(16) });
    const first = learner.select(request);
    const replay = learner.select(request);
    assert.deepEqual(first, replay, domain);
    assert.equal(first.exploration_taken, true, domain);
    assert.equal(first.exploration_draw, null, domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_alpha="))), domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_beta="))), domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_sample="))), domain);
    assert.equal(learner.snapshot().policy.strategy, "thompson_sampling", domain);
  }
});

test("online learner isolates evaluator rewards by domain learning context", async () => {
  const learner = new AutonomousOnlineLearner();
  const request = {
    task: "choose a reasoning model",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    task_family: "coding_delivery",
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
  const codingContext = { domain: "coding", capability: "implementation", risk_class: "engineering_change", task_family: "coding_delivery" };
  const biomedicalContext = { domain: "biomedical", capability: "biomedical_review", risk_class: "biomedical_safety", task_family: "biomedical_review" };
  const codingDigest = learningContextDigest(codingContext);
  const biomedicalDigest = learningContextDigest(biomedicalContext);
  learner.update({ arm_id: "a/one", reward: 1, context_digest: codingDigest, context: codingContext, outcome_digest: "1".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 0, context_digest: codingDigest, context: codingContext, outcome_digest: "2".repeat(64) });
  learner.update({ arm_id: "a/one", reward: 0, context_digest: biomedicalDigest, context: biomedicalContext, outcome_digest: "3".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 1, context_digest: biomedicalDigest, context: biomedicalContext, outcome_digest: "4".repeat(64) });
  const state = learner.snapshot();
  assert.equal(state.arms.length, 0, "contextual rewards must not pollute the legacy global arm ledger");
  assert.deepEqual(state.contextual_states.map((row) => row.context_digest), [codingDigest, biomedicalDigest]);
  assert.equal(learner.select({ ...request, context_digest: codingDigest }).selected_model.provider, "a");
  assert.equal(learner.select({ ...request, domain: "biomedical", capability: "biomedical_review", risk_class: "biomedical_safety", task_family: "biomedical_review", context_digest: biomedicalDigest }).selected_model.provider, "b");
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.2, context_digest: codingDigest, context: codingContext, outcome_digest: "1".repeat(64) }), /contradictory evaluator evidence/);
});

test("online learner rejects malformed contextual snapshots with typed errors", () => {
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [], contextual_states: [null] } }), /bandit contextual state must contain context and arms/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [{ arm_id: "a/one", pulls: 1, reward_sum: 2 }] } }), /online learner arm is malformed/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: -1, arms: [] } }), /generation must be a non-negative safe integer/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [{ arm_id: "a/one" }, { arm_id: "a/one" }] } }), /arm a\/one is duplicated/);
  const learner = new AutonomousOnlineLearner();
  assert.throws(() => learner.restore({ schema: "test", generation: 0, policy: { epsilon: 0.9 }, arms: [] }), /remote policy epsilon conflicts/);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.5, context: { domain: "coding", capability: "implementation", risk_class: "engineering_change" } }), /context requires a context_digest/);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.5, context_digest: "0".repeat(64), context: { domain: "coding", capability: "implementation", risk_class: "engineering_change" } }), /does not match its context identity/);
});

test("every built-in domain blueprint binds a distinct bounded learning context", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("blueprint must not invoke a provider"); } });
  const agent = new AutonomousAgent(runtime);
  const profiles = await builtinAutonomousDomainProfiles();
  const blueprints = await Promise.all(profiles.map((profile) => agent.blueprint(`Review the ${profile.domain} workflow.`, { domain: profile.domain })));
  const digests = blueprints.map((row) => row.blueprint.learning_context_digest);
  assert.equal(new Set(digests).size, profiles.length);
  assert.ok(digests.every((digest) => /^[0-9a-f]{64}$/.test(digest)));
  assert.deepEqual(blueprints.map((row) => row.blueprint.selection_context.domain).sort(), profiles.map((profile) => profile.domain).sort());
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

test("remote evaluator reward adopts the projected state instead of replaying local credit", async () => {
  const apiClient = {
    async brainModelSelectContextual() {
      throw new Error("remote state test must not select a model");
    },
    async brainBanditUpdate(state, update) {
      assert.equal(state.generation, 0);
      assert.equal(update.arm_id, "remote/model");
      return {
        ok: true,
        mcp: {
          result: {
            structuredContent: {
              schema: "bioprism-brain-bandit/0.1",
              generation: 12,
              arms: [{ arm_id: "remote/model", pulls: 4, reward_sum: -0.2, failures: 2 }],
              credited_outcomes: [],
            },
          },
        },
      };
    },
  };
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("remote state test must not invoke a provider"); } }), {
    apiClient,
    learner: new AutonomousOnlineLearner(),
  });
  const projected = await agent.recordEvaluatorReward("remote/model", 0.9, { remote: true });
  assert.equal(projected.generation, 12);
  assert.deepEqual(agent.learner.snapshot().arms, [{ arm_id: "remote/model", pulls: 4, reward_sum: -0.2, failures: 2 }]);
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

test("keyless readiness audits every built-in domain without contacting providers", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const report = await agent.readiness();

  assert.equal(report.schema, AUTONOMOUS_READINESS_SCHEMA);
  assert.equal(report.domains.length, 12);
  assert.equal(new Set(report.domains.map((row) => row.domain)).size, 12);
  assert.ok(report.domains.every((row) => row.state === "ready_for_caller_approval"));
  assert.equal(report.readiness_state, "ready_for_caller_approval");
  assert.deepEqual(report.models[0].compatible_domains, profiles.map((profile) => profile.domain));
  assert.deepEqual(report.models[0].eligible_domains, profiles.map((profile) => profile.domain));
  assert.equal(report.learning.configured, false);
  assert.equal(report.tooling.configured, false);
  assert.equal(report.execution, "not_started; no_provider_or_tool_calls");
  assert.equal(report.secret_material, "never_returned");
  assert.match(report.readiness_digest, /^[0-9a-f]{64}$/);
  assert.match(JSON.stringify(report), /attach AutonomousOnlineLearner/);
  assert.doesNotMatch(JSON.stringify(report), /api_key|Bearer|sk-|test-secret/i);
  assert.equal(fetchCalls, 0);
});

test("readiness exposes model, provider, and credential gates as actionable states", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];

  const empty = await new AutonomousAgent(llm).readiness({ candidates: [] });
  assert.equal(empty.readiness_state, "model_catalogue_required");
  assert.ok(empty.domains.every((row) => row.state === "model_catalogue_required"));
  assert.equal(empty.models.length, 0);

  const unregistered = new AutonomousAgent(llm);
  unregistered.registerModel(candidate("unregistered", "model", capabilities));
  const registrationReport = await unregistered.readiness();
  assert.equal(registrationReport.readiness_state, "provider_registration_required");
  assert.equal(registrationReport.models[0].provider_registered, false);
  assert.equal(registrationReport.models[0].eligible_domains.length, 0);

  llm.registerProvider(openaiCompatibleProvider("credentialed", "https://credentialed.invalid", { requiresCredential: true }));
  const credentialed = new AutonomousAgent(llm);
  credentialed.registerModel(candidate("credentialed", "model", capabilities));
  const credentialReport = await credentialed.readiness();
  assert.equal(credentialReport.readiness_state, "credential_required");
  assert.equal(credentialReport.providers[0].credential_ready, false);
  assert.match(JSON.stringify(credentialReport), /collect_user_credential/);
  assert.equal(fetchCalls, 0);
});

test("readiness reports exact live tool metadata while keeping registration non-authorizing", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const binding = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings[0];
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: "metadata-only test tool", inputSchema: { type: "object", additionalProperties: true } }]);
  const agent = new AutonomousAgent(llm, { toolCatalogue: catalogue });
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const report = await agent.readiness();
  const coding = report.domains.find((row) => row.domain === "coding");

  assert.equal(report.tooling.configured, true);
  assert.equal(report.tooling.available_tool_count, 1);
  assert.equal(coding.available_tool_count, 1);
  assert.equal(coding.missing_tools.includes(binding.name), false);
  assert.ok(coding.missing_tools.length > 0);
  assert.equal(report.execution, "not_started; no_provider_or_tool_calls");
  assert.equal(fetchCalls, 0);
});

test("activation is a redacted digest-bound lifecycle across all twelve domains", async () => {
  let now = 100;
  const activation = new AutonomousCapabilityActivation({ activationId: "activation-test", clock: () => now });
  assert.equal(activation.state.status, "created");
  activation.recordProviderStatuses([{
    provider: "local",
    provider_registered: true,
    requires_credential: false,
    credential_ready: true,
    credential: { ready: true, active_handles: 0 },
    next_action: "ready",
    secret_material: "never_returned",
  }]);

  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.map((profile) => {
    const binding = profile.tool_profile.bindings[0];
    return [binding.name, { name: binding.name, description: `Activation ${binding.name}`, inputSchema: { type: "object", additionalProperties: true } }];
  })).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  assert.equal(plan.domains.length, 12);
  assert.equal(plan.coverage.length, 12);
  assert.equal(plan.plan_digest.length, 64);

  now += 10;
  const reviewed = activation.recordBindingPlan(plan);
  assert.equal(reviewed.domain_statuses.length, 12);
  assert.equal(reviewed.plan_digest, plan.plan_digest);
  const proposed = plan.proposed_bindings.map((binding) => binding.name);
  assert.ok(proposed.length > 0);
  const approved = activation.approveBindings(plan, [proposed[0]], definitions.length);
  assert.deepEqual(approved.approved_tools, [proposed[0]]);
  assert.equal(approved.authorization, "status_only; does_not_grant_provider_or_tool_authority");
  assert.equal(approved.secret_material, "never_returned");
  assert.doesNotMatch(JSON.stringify(approved), /api_key|Bearer|sk-[A-Za-z0-9]/i);
  assert.throws(() => activation.recordProviderStatuses([{ provider: "local", api_key: "must-not-enter-state" }]), /unsupported fields/);

  const store = new AutonomousCapabilityActivationStore();
  await store.save(approved);
  const snapshot = await store.snapshot();
  let persisted = null;
  const persistence = {
    read: () => persisted,
    write: (value) => { persisted = structuredClone(value); },
  };
  const coordinator = new AutonomousCapabilityActivationPersistenceCoordinator(store, persistence);
  const receipt = await coordinator.flush();
  assert.equal(receipt.state_digest, approved.state_digest);
  assert.equal(receipt.retention, "metadata_only");

  const restoredStore = new AutonomousCapabilityActivationStore();
  const restored = new AutonomousCapabilityActivation({ activationId: "activation-test", clock: () => now });
  const restoreCoordinator = new AutonomousCapabilityActivationPersistenceCoordinator(restoredStore, persistence);
  const restoreReceipt = await restoreCoordinator.restore();
  assert.equal(restoreReceipt.restored, true);
  assert.deepEqual((await restoredStore.load()).state_digest, approved.state_digest);
  restored.restore(await restoredStore.load());
  assert.deepEqual(restored.state.approved_tools, approved.approved_tools);
  await assert.rejects(() => restoredStore.restore({ ...snapshot, snapshot_digest: "0".repeat(64) }), /digest/);

  now += 10;
  activation.revoke("caller_revoked_for_test");
  assert.equal(activation.state.status, "revoked");
  assert.throws(() => activation.approveBindings(plan, [proposed[0]]), /revoked/);
});

test("agent activation refreshes keylessly and blocks unapproved custom tool calls", async () => {
  let fetchCalls = 0;
  let executions = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("activation readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://activation.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const binding = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings.find((row) => row.name === "repository_catalog");
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: "Read repository metadata", inputSchema: { type: "object", additionalProperties: true } }]);
  const activation = new AutonomousCapabilityActivation({ activationId: "agent-activation", clock: () => 200 });
  const agent = new AutonomousAgent(llm, {
    activation,
    toolCatalogue: catalogue,
    toolExecutor: async (tool) => { executions += 1; return { tool: tool.name, ok: true }; },
  });
  agent.registerModel(candidate("local", "local-model", capabilities));

  const state = await agent.refreshActivation();
  assert.equal(state.domain_statuses.length, 12);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  agent.approveActivationBindings(plan, [binding.name], 1);
  const report = await agent.readiness();
  assert.equal(report.activation.approved_tools[0], binding.name);
  assert.equal(report.activation.plan_digest, plan.plan_digest);

  const results = await agent.executeToolCalls([
    { id: "approved", name: binding.name, arguments: {} },
    { id: "blocked", name: "repository_impact_analysis", arguments: {} },
  ], { domains: ["coding"], approveEffects: true });
  assert.equal(results.find((row) => row.callId === "approved").approved, true);
  assert.equal(results.find((row) => row.callId === "blocked").content.status, "activation_required");
  assert.equal(executions, 1);
  assert.equal(fetchCalls, 0);
});
