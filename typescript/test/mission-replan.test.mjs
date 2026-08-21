import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousMissionExecutor,
  AutonomousMissionReplanPersistenceCoordinator,
  AutonomousMissionReplanContractError,
  InMemoryAutonomousMissionReplanStateStore,
  InMemoryAutonomousMissionCheckpointStore,
  InMemoryAutonomousMissionResultStore,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  openaiCompatibleProvider,
  runAutonomousMissionReplanCycle,
  validateAutonomousMissionReplanCheckpoint,
  validateAutonomousMissionReplanSnapshot,
} from "../dist/index.js";

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
  llm.registerProvider(openaiCompatibleProvider("mission-replan-router", "https://mission-replan-router.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({
    provider: "mission-replan-router",
    model: "mission-replan-router-model",
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

async function catalogue() {
  return ToolCatalogue.fromDefinitions([
    {
      name: "mission_probe",
      description: "bounded mission test probe",
      inputSchema: { type: "object", additionalProperties: true },
    },
  ]);
}

function mission(steps, missionId = "mission-replan-root") {
  return {
    mission_id: missionId,
    goal: "execute a bounded multi-domain verification mission",
    steps,
    policy: {
      execute: true,
      stop_on_error: true,
      allow_side_effects: false,
      max_steps: 64,
      max_step_output_bytes: 100_000,
      max_total_output_bytes: 2_000_000,
      execution_mode: "serial",
      max_parallelism: 1,
      allowed_tools: ["mission_probe"],
    },
  };
}

function domainSteps() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `step-${index}`,
    domain,
    capability: "verification",
    objective: `verify ${domain}`,
    tool: "mission_probe",
    arguments: { domain, index },
  }));
}

test("mission replan cycle covers every domain and retains only metadata at the attempt boundary", async () => {
  const calls = [];
  const checkpoints = [];
  let evaluations = 0;
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    checkpointStore: new InMemoryAutonomousMissionCheckpointStore(),
    resultStore: new InMemoryAutonomousMissionResultStore(),
    executeStep: async ({ mission_id, step }) => {
      calls.push({ mission_id, step: step.id, domain: step.domain, objective: step.objective });
      return { status: "succeeded", value: { step: step.id, private_output: "local-only" } };
    },
  });
  const result = await runAutonomousMissionReplanCycle(executor, mission(domainSteps()), {
    maxReplans: 1,
    evaluate: () => {
      evaluations += 1;
      return evaluations === 1
        ? { evaluator_id: "mission-reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "Add an independent verification pass." }
        : { evaluator_id: "mission-reviewer", evaluator_version: "1", reward: 0.95, passed: true, replan_requested: false };
    },
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(result.status, "completed");
  assert.equal(result.replan_count, 1);
  assert.equal(result.attempts.length, 2);
  assert.equal(result.evaluations.length, 2);
  assert.equal(calls.length, AUTONOMOUS_DOMAIN_NAMES.length * 2);
  assert.deepEqual(new Set(calls.map((call) => call.domain)), new Set(AUTONOMOUS_DOMAIN_NAMES));
  assert.equal(result.final_execution.mission_id, "mission-replan-root:attempt-2");
  assert.ok(result.final_execution.results.every((row) => row.step.objective.includes("independent verification pass")));
  assert.equal(checkpoints.length, 2);
  assert.equal(checkpoints[0].phase, "replan_scheduled");
  assert.equal(checkpoints[1].phase, "terminal");
  assert.equal(JSON.stringify(checkpoints).includes("independent verification pass"), false);
  assert.equal(JSON.stringify(checkpoints).includes("local-only"), false);
  assert.ok(checkpoints.every((checkpoint) => checkpoint.checkpoint_digest.length === 64));
  assert.equal((await validateAutonomousMissionReplanCheckpoint(checkpoints[0])).checkpoint_digest, checkpoints[0].checkpoint_digest);
  await assert.rejects(() => validateAutonomousMissionReplanCheckpoint({ ...checkpoints[0], phase: "terminal" }), /digest/);
});

test("mission replanning refuses protected contract drift and credential-shaped guidance", async () => {
  let calls = 0;
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async () => {
      calls += 1;
      return { status: "succeeded", value: { ok: true } };
    },
  });
  const step = { id: "step-1", domain: "coding", capability: "verification", objective: "verify", tool: "mission_probe", arguments: {} };
  await assert.rejects(() => runAutonomousMissionReplanCycle(executor, mission([step]), {
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "retry" }),
    replan: ({ mission: previous, instruction }) => {
      assert.equal(instruction, "retry");
      return { ...previous, mission_id: "mission-replan-root:attempt-2", steps: [{ ...previous.steps[0], tool: "different-tool" }] };
    },
  }), AutonomousMissionReplanContractError);
  assert.equal(calls, 1);

  await assert.rejects(() => runAutonomousMissionReplanCycle(executor, mission([step], "mission-secret-guidance"), {
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "use the api_key from the task" }),
  }), /credential-shaped/);
  assert.equal(calls, 2);
});

test("mission replanning settles exact delayed-credit rewards for every attempt", async () => {
  const prepared = [];
  const settled = [];
  const learning = {
    async prepareTrajectory(ids, options) {
      prepared.push({ ids, options });
      return { trajectory_id: options.trajectoryId, steps: ids.map((episode_id, index) => ({ index, episode_id })) };
    },
    async settleTrajectory(trajectoryId, rewards, options) {
      settled.push({ trajectoryId, rewards, options });
      return { trajectory: { trajectory_id: trajectoryId, status: "settled" }, settlements: [], return_to_go: {} };
    },
  };
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async ({ mission_id }) => ({ status: "succeeded", value: { local: true }, learning_episode_id: `${mission_id}:episode` }),
  });
  let evaluations = 0;
  const result = await runAutonomousMissionReplanCycle(executor, mission([{ id: "step-1", domain: "coding", capability: "verification", objective: "verify", tool: "mission_probe", arguments: {} }]), {
    maxReplans: 1,
    learning: { adapter: learning, trajectoryIdPrefix: "mission-learning" },
    evaluate: (execution) => {
      evaluations += 1;
      const episodeId = execution.checkpoint.step_states["step-1"].learning_episode_id;
      return {
        evaluator_id: "reviewer",
        evaluator_version: "1",
        reward: evaluations === 1 ? 0.3 : 0.9,
        passed: evaluations === 2,
        replan_requested: evaluations === 1,
        replan_instruction: evaluations === 1 ? "add verification" : null,
        rewards: { [episodeId]: { evaluator_id: "reviewer", evaluator_version: "1", reward: evaluations === 1 ? 0.3 : 0.9, passed: evaluations === 2 } },
      };
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(prepared.length, 2);
  assert.equal(settled.length, 2);
  assert.notEqual(settled[0].trajectoryId, settled[1].trajectoryId);
  assert.equal(result.learning_settlements.length, 2);
  assert.deepEqual(result.learning_settlements[0].episode_ids, ["mission-replan-root:episode"]);
  assert.deepEqual(result.learning_settlements[1].episode_ids, ["mission-replan-root:attempt-2:episode"]);
});

test("mission replanning returns approval and recovery states without evaluating or retrying them", async () => {
  let calls = 0;
  let evaluations = 0;
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async () => {
      calls += 1;
      return { status: "approval_required", error_class: "CallerApprovalRequired" };
    },
  });
  const result = await runAutonomousMissionReplanCycle(executor, mission([{ id: "step-1", domain: "operations", capability: "approval", objective: "wait for approval", tool: "mission_probe", arguments: {} }]), {
    evaluate: () => {
      evaluations += 1;
      return { evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "should not run" };
    },
  });
  assert.equal(result.status, "approval_required");
  assert.equal(result.evaluations.length, 0);
  assert.equal(calls, 1);
  assert.equal(evaluations, 0);
});

test("default mission replanning refuses side-effect-enabled missions", async () => {
  const executor = new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    executeStep: async () => ({ status: "succeeded", value: { ok: true } }),
  });
  const effectMission = mission([{ id: "step-1", domain: "operations", capability: "change", objective: "change", tool: "mission_probe", arguments: {} }], "mission-effect-replan");
  effectMission.policy.allow_side_effects = true;
  await assert.rejects(() => runAutonomousMissionReplanCycle(executor, effectMission, {
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "retry safely" }),
  }), /side-effect-enabled/);
});

test("mission replan state resumes evaluator handoffs without replaying settled learning", async () => {
  const checkpointStore = new InMemoryAutonomousMissionCheckpointStore();
  const resultStore = new InMemoryAutonomousMissionResultStore();
  const stateStore = new InMemoryAutonomousMissionReplanStateStore();
  const prepared = [];
  const settled = [];
  const learning = {
    async prepareTrajectory(ids, options) {
      prepared.push({ ids, options });
      return { trajectory_id: options.trajectoryId, steps: ids.map((episode_id, index) => ({ index, episode_id })) };
    },
    async settleTrajectory(trajectoryId, rewards, options) {
      settled.push({ trajectoryId, rewards, options });
      return { trajectory: { trajectory_id: trajectoryId, status: "settled" }, settlements: [], return_to_go: {} };
    },
  };
  let executions = 0;
  const makeExecutor = async () => new AutonomousMissionExecutor({
    catalogue: await catalogue(),
    checkpointStore,
    resultStore,
    executeStep: async ({ mission_id }) => {
      executions += 1;
      return { status: "succeeded", value: { local: true }, learning_episode_id: `${mission_id}:episode` };
    },
  });
  const root = mission([{ id: "step-1", domain: "coding", capability: "verification", objective: "verify", tool: "mission_probe", arguments: {} }], "mission-restart-root");
  let evaluations = 0;
  const evaluate = (execution) => {
    evaluations += 1;
    const episodeId = execution.checkpoint.step_states["step-1"].learning_episode_id;
    if (evaluations === 1) return { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "add an independent verification pass", rewards: { [episodeId]: { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false } } };
    if (evaluations === 2) throw new Error("simulated process interruption after attempt two execution");
    return { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.95, passed: true, replan_requested: false, rewards: { [episodeId]: { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.95, passed: true } } };
  };
  let proposal;
  await assert.rejects(() => makeExecutor().then((executor) => runAutonomousMissionReplanCycle(executor, root, {
    stateStore,
    learning: { adapter: learning },
    evaluate,
    replan: () => { throw new Error("simulated process interruption at replan handoff"); },
  })), /replan handoff/);
  const handoff = await stateStore.load(root.mission_id);
  assert.equal(handoff.phase, "replan_handoff");
  assert.equal(handoff.attempts.length, 1);
  assert.equal(handoff.evaluations.length, 1);
  assert.equal(settled.length, 1);

  await assert.rejects(() => makeExecutor().then((executor) => runAutonomousMissionReplanCycle(executor, root, {
    stateStore,
    learning: { adapter: learning },
    evaluate,
    rehydrateReplanInstruction: ({ instruction_digest }) => {
      assert.equal(instruction_digest, handoff.replan_instruction_digest);
      return "add an independent verification pass";
    },
    replan: ({ mission: previous }) => {
      proposal = { ...previous, mission_id: "mission-restart-root:attempt-2", steps: previous.steps.map((step) => ({ ...step, objective: `${step.objective} plus review` })) };
      return proposal;
    },
  })), /simulated process interruption after attempt two execution/);
  const pending = await stateStore.load(root.mission_id);
  assert.equal(pending.phase, "evaluation_pending");
  assert.equal(pending.current_mission_id, "mission-restart-root:attempt-2");
  assert.equal(settled.length, 1);

  const resumed = await runAutonomousMissionReplanCycle(await makeExecutor(), root, {
    stateStore,
    learning: { adapter: learning },
    evaluate,
    rehydrateMission: ({ mission_id }) => {
      assert.equal(mission_id, proposal.mission_id);
      return structuredClone(proposal);
    },
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.attempts.length, 2);
  assert.equal(resumed.evaluations.length, 2);
  assert.equal(settled.length, 2);
  assert.equal(prepared.length, 2);
  assert.deepEqual(settled.map((row) => row.trajectoryId), ["mission-replan:mission-restart-root:attempt-1", "mission-replan:mission-restart-root:attempt-2"]);
  assert.equal(executions, 2);
  const snapshot = await stateStore.snapshot();
  assert.equal((await validateAutonomousMissionReplanSnapshot(snapshot)).snapshot_digest, snapshot.snapshot_digest);
  await assert.rejects(() => validateAutonomousMissionReplanSnapshot({ ...snapshot, snapshot_digest: "0".repeat(64) }), /digest/);
  const persistence = { value: null, async read() { return this.value; }, async write(value) { this.value = structuredClone(value); } };
  const coordinator = new AutonomousMissionReplanPersistenceCoordinator(stateStore, persistence);
  const flushed = await coordinator.flush();
  assert.equal(flushed.snapshot_digest, snapshot.snapshot_digest);
  const restoredStore = new InMemoryAutonomousMissionReplanStateStore();
  const restored = await new AutonomousMissionReplanPersistenceCoordinator(restoredStore, persistence).restore();
  assert.equal(restored.states, 1);
  assert.equal((await restoredStore.load(root.mission_id)).state_digest, (await stateStore.load(root.mission_id)).state_digest);
});

test("mission replanning reuses one approved semantic route and one aggregate budget across attempts", async () => {
  const { agent, calls } = semanticAgent({
    selected_domains: [
      { domain: "coding", score: 0.94, rationale: "implementation" },
      { domain: "data", score: 0.91, rationale: "data validation" },
    ],
    confidence: 0.93,
    abstain: false,
    abstain_reason: null,
  });
  const root = mission([
    { id: "code", domain: "coding", capability: "verification", objective: "verify code", tool: "mission_probe", arguments: {} },
    { id: "data", domain: "data", capability: "verification", objective: "verify data", tool: "mission_probe", arguments: {} },
  ], "mission-route-replan-root");
  root.goal = "Help with an unfamiliar task.";
  const stateStore = new InMemoryAutonomousMissionReplanStateStore();
  const executor = new AutonomousMissionExecutor({
    agent,
    catalogue: await catalogue(),
    checkpointStore: new InMemoryAutonomousMissionCheckpointStore(),
    resultStore: new InMemoryAutonomousMissionResultStore(),
    executeStep: async ({ step }) => ({ status: "succeeded", value: { step: step.id } }),
  });
  let evaluations = 0;
  const result = await runAutonomousMissionReplanCycle(executor, root, {
    maxReplans: 1,
    stateStore,
    execute: {
      maxTotalCostUnits: 100,
      semanticRouting: { enabled: true, approveProviderCall: true, maxDomains: 2, allowCrossDomain: true },
    },
    evaluate: () => {
      evaluations += 1;
      return evaluations === 1
        ? { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "add review" }
        : { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.95, passed: true, replan_requested: false };
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(calls(), 1, "semantic routing is classified once and route-bound on the replan attempt");
  assert.equal(result.route_digest, result.final_execution.route.route_digest);
  assert.equal(result.cost_budget.max_cost_units, 100);
  assert.ok(result.cost_budget.consumed_cost_units > 0);
  assert.equal(result.attempts[0].route_digest, result.attempts[1].route_digest);
  const persisted = await stateStore.load(root.mission_id);
  assert.equal(persisted.route_digest, result.route_digest);
  assert.deepEqual(persisted.cost_budget, result.cost_budget);

  const restarted = new AutonomousMissionExecutor({
    agent,
    catalogue: await catalogue(),
    checkpointStore: executor.store,
    resultStore: executor.resultStore,
    executeStep: async () => ({ status: "succeeded", value: { should_not_dispatch: true } }),
  });
  await assert.rejects(
    () => runAutonomousMissionReplanCycle(restarted, root, { stateStore, execute: { semanticRouting: { enabled: true, approveProviderCall: true } }, evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) }),
    /routeOverride/,
  );
  const resumed = await runAutonomousMissionReplanCycle(restarted, root, {
    stateStore,
    execute: { routeOverride: result.final_execution.route, maxTotalCostUnits: 100, semanticRouting: { enabled: true, approveProviderCall: true } },
    rehydrateMission: () => ({
      ...root,
      mission_id: "mission-route-replan-root:attempt-2",
      steps: root.steps.map((step) => ({ ...step, objective: result.final_execution.results.find((row) => row.step.id === step.id).step.objective })),
    }),
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(calls(), 1, "route and budget snapshots recover without provider replay");
  assert.deepEqual(resumed.cost_budget, result.cost_budget);
});
