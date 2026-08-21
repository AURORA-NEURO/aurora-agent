import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousMissionExecutor,
  CredentialStore,
  InMemoryAutonomousMissionCheckpointStore,
  InMemoryAutonomousMissionResultStore,
  LLMRuntime,
  agentMissionStepExecutor,
  digestJson,
  openaiCompatibleProvider,
  settleAutonomousMissionLearning,
  ToolCatalogue,
} from "../dist/index.js";

async function catalogue() {
  return ToolCatalogue.fromDefinitions([
    {
      name: "mission_probe",
      description: "bounded test probe",
      inputSchema: { type: "object", additionalProperties: true },
    },
  ]);
}

function mission(steps, policy = {}, goal = "exercise the durable autonomous mission executor") {
  return {
    mission_id: "mission-local-1",
    goal,
    steps,
    policy: {
      execute: true,
      stop_on_error: true,
      allow_side_effects: false,
      max_steps: 64,
      max_step_output_bytes: 100_000,
      max_total_output_bytes: 1_000_000,
      execution_mode: "serial",
      max_parallelism: 4,
      allowed_tools: ["mission_probe"],
      ...policy,
    },
  };
}

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

function semanticAgent(payload) {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(payload) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("mission-router", "https://mission-router.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({
    provider: "mission-router",
    model: "mission-router-model",
    capabilities: ["reasoning", "structured_output", "code", "data", "coordination"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 50,
    cost_per_million_tokens: 5,
    reliability: 0.99,
  });
  return { agent, calls: () => calls };
}

test("durable mission execution resumes dependency waves and rehydrates caller-owned outputs", async () => {
  const steps = [
    { id: "seed", domain: "coding", capability: "testing", objective: "produce a seed", tool: "mission_probe", arguments: { value: "private-seed" } },
    {
      id: "bound",
      domain: "data",
      capability: "data_analysis",
      objective: "consume the seed",
      tool: "mission_probe",
      arguments: { value: "placeholder" },
      depends_on: ["seed"],
      bindings: [{ from_step: "seed", source_pointer: "/value", target_pointer: "/value" }],
    },
  ];
  const store = new InMemoryAutonomousMissionCheckpointStore();
  const resultStore = new InMemoryAutonomousMissionResultStore();
  const calls = [];
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    checkpointStore: store,
    resultStore,
    executeStep: async ({ step, arguments: args }) => {
      calls.push({ id: step.id, args });
      return { status: "succeeded", value: { value: `${args.value}-resolved`, secret_echo: "private-output" } };
    },
  });

  const first = await executor.start(mission(steps), { max_waves: 1, approveProviderCall: true });
  assert.equal(first.status, "running");
  assert.equal(first.next_wave, 1);
  assert.deepEqual(calls.map((call) => call.id), ["seed"]);
  assert.equal(JSON.stringify(first.checkpoint).includes("private-seed"), false);
  assert.equal(JSON.stringify(first.checkpoint).includes("private-output"), false);
  assert.equal(first.events.at(-1).event_type, "checkpointed");

  // A new executor represents a process restart. The metadata checkpoint is restored, while the
  // caller-owned result store is deliberately the only source of the dependency payload.
  const snapshot = await store.snapshot();
  const restoredStore = new InMemoryAutonomousMissionCheckpointStore();
  await restoredStore.restore(snapshot);
  const resumed = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    checkpointStore: restoredStore,
    resultStore,
    executeStep: async ({ step, arguments: args }) => {
      calls.push({ id: step.id, args });
      return { status: "succeeded", value: { value: `${args.value}-done` } };
    },
  });
  const second = await resumed.resume(mission(steps), { approveProviderCall: true });
  assert.equal(second.status, "succeeded");
  assert.equal(second.next_wave, null);
  assert.deepEqual(calls.map((call) => call.id), ["seed", "bound"]);
  assert.equal(calls[1].args.value, "private-seed-resolved");
  assert.equal(second.completed_steps, 2);
  assert.equal((await resumed.events("mission-local-1")).at(-1).event_type, "mission.completed");
});

test("parallel mission waves merge deterministically and cap in-flight execution", async () => {
  let active = 0;
  let maximum = 0;
  const steps = ["a", "b", "c", "d", "e"].map((id, index) => ({
    id,
    domain: AUTONOMOUS_DOMAIN_NAMES[index],
    capability: "bounded",
    objective: `run ${id}`,
    tool: "mission_probe",
    arguments: { index },
  }));
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async ({ step }) => {
      active += 1;
      maximum = Math.max(maximum, active);
      await new Promise((resolve) => setTimeout(resolve, 2));
      active -= 1;
      return { status: "succeeded", value: { id: step.id } };
    },
  });
  const result = await executor.start(mission(steps, { execution_mode: "parallel_waves", max_parallelism: 2 }), { approveProviderCall: true });
  assert.equal(result.status, "succeeded");
  assert.equal(result.completed_steps, steps.length);
  assert.equal(maximum, 2);
  assert.deepEqual(result.checkpoint.completed_step_ids, steps.map((step) => step.id));
});

test("one mission contract accepts every built-in autonomous domain", async () => {
  const steps = AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `domain-${index}`,
    domain,
    capability: "coverage",
    objective: `validate ${domain}`,
    tool: "mission_probe",
    arguments: { domain },
  }));
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async ({ step }) => ({ status: "succeeded", value: { domain: step.domain } }),
  });
  const result = await executor.start(mission(steps, { execution_mode: "parallel_waves", max_parallelism: 4, max_step_output_bytes: 100, max_total_output_bytes: 10_000 }), { approveProviderCall: true });
  assert.equal(result.status, "succeeded");
  assert.equal(result.completed_steps, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.failed_steps, 0);
});

test("approval and uncertain-effect step states remain resumable", async () => {
  const step = {
    id: "effect-step",
    domain: "operations",
    capability: "approval",
    objective: "perform an approved external operation",
    tool: "mission_probe",
    arguments: { operation: "bounded" },
  };
  let calls = 0;
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async () => {
      calls += 1;
      if (calls === 1) return { status: "reconciliation_required", error_class: "TransportLostAfterDispatch" };
      return { status: "succeeded", value: { reconciled: true } };
    },
  });
  const first = await executor.start(mission([step], { allow_side_effects: true }), { approveProviderCall: true });
  assert.equal(first.status, "reconciliation_required");
  assert.equal(first.next_wave, 0);
  const second = await executor.resume(mission([step], { allow_side_effects: true }), { approveProviderCall: true });
  assert.equal(second.status, "succeeded");
  assert.equal(second.completed_steps, 1);
  assert.equal(calls, 2);
});

test("mission learning linkage is metadata-only and requires exact evaluator coverage", async () => {
  const step = {
    id: "learned-step",
    domain: "coding",
    capability: "verification",
    objective: "run a bounded learning-linked operation",
    tool: "mission_probe",
    arguments: { value: "private-input" },
  };
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async () => ({ status: "succeeded", value: { private_output: "not-in-checkpoint" }, learning_episode_id: "episode-mission-1" }),
  });
  const execution = await executor.start(mission([step]), { approveProviderCall: true });
  assert.equal(execution.status, "succeeded");
  assert.equal(execution.checkpoint.step_states["learned-step"].learning_episode_id, "episode-mission-1");
  assert.equal(JSON.stringify(execution.checkpoint).includes("private_output"), false);
  assert.equal(JSON.stringify(execution.checkpoint).includes("private-input"), false);

  const calls = [];
  const learning = {
    async prepareTrajectory(ids, options) {
      calls.push({ kind: "prepare", ids, options });
      return { trajectory_id: options.trajectoryId, steps: ids.map((episode_id, index) => ({ index, episode_id })) };
    },
    async settleTrajectory(trajectoryId, rewards, options) {
      calls.push({ kind: "settle", trajectoryId, rewards, options });
      return { trajectory: { trajectory_id: trajectoryId, status: "settled" }, settlements: [], return_to_go: {} };
    },
  };
  const settled = await settleAutonomousMissionLearning(execution, learning, {
    trajectoryId: "mission-trajectory-1",
    rewards: { "episode-mission-1": { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.8, passed: true } },
    outbox: { workerId: "mission-worker" },
  });
  assert.deepEqual(settled.episode_ids, ["episode-mission-1"]);
  assert.equal(calls[0].kind, "prepare");
  assert.equal(calls[1].kind, "settle");
  assert.deepEqual(calls[1].options.outbox, { workerId: "mission-worker" });
  await assert.rejects(() => settleAutonomousMissionLearning(execution, learning, {
    trajectoryId: "mission-trajectory-missing-reward",
    rewards: {},
  }), /cover exactly every successful learning episode/);
});

test("agent mission adapter prepares learning only after the exact tool call completes", async () => {
  const prepared = [];
  const step = { id: "step-1", domain: "coding", capability: "testing", objective: "invoke the probe", tool: "mission_probe", arguments: { value: "private-input" } };
  const agent = {
    async executeToolCalls(calls) {
      return calls.map((call) => ({ callId: call.id, approved: true, isError: false, content: { accepted: true } }));
    },
    async run(_task, options) {
      await options.authorizeAndExecute([{ id: "call-1", name: "mission_probe", arguments: { value: "private-input" } }]);
      return {
        status: "completed",
        selection: { selected_model: { provider: "mission-provider", model: "mission-model" } },
        route: { route_digest: "a".repeat(64) },
        blueprint: { plan: { plan_digest: "b".repeat(64) }, prompt: { prompt_digest: "c".repeat(64) } },
        response: { structured: { answer: "private-response" } },
      };
    },
  };
  const adapter = agentMissionStepExecutor(agent, {
    learning: {
      adapter: {
        async prepareRun(_run, options) {
          prepared.push(options);
          return { episode_id: options.episodeId };
        },
      },
    },
  });
  const result = await adapter({
    mission_id: "mission-adapter-1",
    goal: "exercise exact adapter contract",
    wave: 0,
    step,
    arguments: { value: "private-input" },
    dependency_outputs: {},
    execution_attempt: 1,
    resumed: false,
  });
  assert.equal(result.status, "succeeded");
  assert.equal(result.learning_episode_id, "mission:mission-adapter-1:step-1");
  assert.equal(result.value.accepted, true);
  assert.equal(result.decision.provider, "mission-provider");
  assert.equal(result.decision.model, "mission-model");
  assert.equal(result.decision.route_digest, "a".repeat(64));
  assert.equal(result.decision.plan_digest, "b".repeat(64));
  assert.equal(result.decision.prompt_digest, "c".repeat(64));
  assert.equal(prepared.length, 1);
  assert.equal(prepared[0].stageId, "step-1");
  assert.equal(prepared[0].parentJobId, "mission-adapter-1");

  const durable = new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: adapter });
  const execution = await durable.start(mission([step]), { approveProviderCall: true });
  assert.equal(execution.status, "succeeded");
  assert.equal(execution.checkpoint.step_states["step-1"].decision.provider, "mission-provider");
  assert.equal(execution.checkpoint.step_states["step-1"].decision.plan_digest, "b".repeat(64));
});

test("durable missions bind semantic routing to explicit step domains and recover by route override", async () => {
  const { agent, calls } = semanticAgent({
    selected_domains: [
      { domain: "coding", score: 0.94, rationale: "implementation work" },
      { domain: "data", score: 0.91, rationale: "data validation work" },
    ],
    confidence: 0.93,
    abstain: false,
    abstain_reason: null,
  });
  const steps = [
    { id: "code", domain: "coding", capability: "testing", objective: "run the code check", tool: "mission_probe", arguments: { value: "code" } },
    { id: "data", domain: "data", capability: "data_analysis", objective: "run the data check", tool: "mission_probe", arguments: { value: "data" }, depends_on: ["code"] },
  ];
  const store = new InMemoryAutonomousMissionCheckpointStore();
  const executor = new AutonomousMissionExecutor({
    agent,
    catalogue: await catalogue(),
    checkpointStore: store,
    executeStep: async ({ step }) => ({ status: "succeeded", value: { step: step.id } }),
  });
  const first = await executor.start(mission(steps, {}, "Help with an unfamiliar task."), {
    max_waves: 1,
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, maxDomains: 2, allowCrossDomain: true },
  });
  assert.equal(first.status, "running");
  assert.equal(first.semantic_route_status, "completed");
  assert.deepEqual([...first.route.selected_domains].sort(), ["coding", "data"]);
  assert.equal(first.checkpoint.route_digest, first.route.route_digest);
  assert.equal(calls(), 1, "semantic classifier runs once before mission dispatch");

  const tampered = { ...first.route, confidence: first.route.confidence - 0.01 };
  const { route_digest: _routeDigest, ...tamperedDescriptor } = tampered;
  tampered.route_digest = await digestJson(tamperedDescriptor);
  await assert.rejects(
    () => executor.resume(mission(steps, {}, "Help with an unfamiliar task."), { routeOverride: tampered, approveProviderCall: true }),
    /persisted route digest/,
  );

  const resumed = await executor.resume(mission(steps, {}, "Help with an unfamiliar task."), { routeOverride: first.route, approveProviderCall: true });
  assert.equal(resumed.status, "succeeded");
  assert.equal(resumed.semantic_route_status, null, "restart uses caller-owned route identity without replaying the classifier");
  assert.equal(calls(), 1);
});

test("durable mission semantic routing stays review-only until classifier approval", async () => {
  const { agent, calls } = semanticAgent({ selected_domains: [], confidence: 0, abstain: true, abstain_reason: "needs review" });
  const step = { id: "review", domain: "coding", capability: "testing", objective: "review the task", tool: "mission_probe", arguments: {} };
  const executor = new AutonomousMissionExecutor({
    agent,
    catalogue: await catalogue(),
    executeStep: async () => ({ status: "succeeded", value: { should_not_run: true } }),
  });
  const result = await executor.start(mission([step], {}, "Help with an unfamiliar task."), {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: false },
  });
  assert.equal(result.status, "route_review_required");
  assert.equal(result.semantic_route_status, "approval_required");
  assert.equal(result.checkpoint, null);
  assert.equal(calls(), 0);
});

test("mission step adapters receive one shared caller-owned cost budget", async () => {
  const budgets = [];
  const step = { id: "budgeted", domain: "coding", capability: "testing", objective: "exercise budget propagation", tool: "mission_probe", arguments: {} };
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async ({ cost_budget }) => {
      budgets.push(cost_budget);
      return { status: "succeeded", value: { ok: true } };
    },
  });
  const result = await executor.start(mission([step]), { approveProviderCall: true, maxTotalCostUnits: 100 });
  assert.equal(result.status, "succeeded");
  assert.equal(budgets.length, 1);
  assert.equal(budgets[0].maxCostUnits, 100);
});
