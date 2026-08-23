import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousGoalPersistenceCoordinator,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  InMemoryAutonomousCycleReplanStateStore,
  InMemoryAutonomousGoalLedger,
  JsonAutonomousGoalPersistence,
  LLMRuntime,
  TransactionalJsonAutonomousGoalPersistence,
  WebStorageAutonomousGoalTextStore,
  builtinAutonomousDomainProfiles,
  digestJsonSync,
  goalTaskDigest,
  openaiCompatibleProvider,
  validateAutonomousGoalSnapshot,
} from "../dist/index.js";

test("goal execution wrapper advances approval, completion, terminal replay, and failure states", async () => {
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 10 });
  agent.run = async () => ({ status: "approval_required" });
  const paused = await agent.runGoalStep(ledger, "wrapper-goal", "review a release", "coding", {
    goalCriteria: [{ criterion_id: "reviewed", criterion_digest: goalTaskDigest("reviewed") }],
  });
  assert.equal(paused.goal_status, "paused");
  assert.equal(paused.result_status, "approval_required");
  assert.equal(paused.goal.attempt, 1);
  assert.equal(JSON.stringify(ledger.snapshot()).includes("review a release"), false);

  agent.run = async () => ({ status: "completed" });
  const completed = await agent.runGoalStep(ledger, "wrapper-goal", "review a release", "coding", {
    criterionUpdates: [{ criterion_id: "reviewed", status: "satisfied", evidence_digest: goalTaskDigest("local receipt") }],
    settlementMetadata: { learning_state_digest: goalTaskDigest("bandit state"), progress_digest: goalTaskDigest("evaluation progress") },
  });
  assert.equal(completed.goal_status, "completed");
  assert.equal(completed.goal.attempt, 2);
  assert.ok(completed.goal.evaluator_digest);
  assert.equal(completed.goal.learning_state_digest, goalTaskDigest("bandit state"));
  assert.equal(completed.goal.progress_digest, goalTaskDigest("evaluation progress"));
  const terminal = await agent.runGoalStep(ledger, "wrapper-goal", "review a release", "coding");
  assert.equal(terminal.result, null);
  assert.equal(terminal.result_status, "terminal");

  const failedLedger = new InMemoryAutonomousGoalLedger({ clock: () => 20 });
  agent.run = async () => { throw new Error("synthetic provider failure"); };
  await assert.rejects(() => agent.runGoalStep(failedLedger, "failed-goal", "retry a provider", "operations"), /synthetic provider failure/);
  assert.equal(failedLedger.get("failed-goal").status, "failed");
  assert.equal(failedLedger.verifyIntegrity().ok, true);
});

test("goal ledger carries value-only objective state across attempts and snapshots", async () => {
  let now = 100;
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => now });
  const task = "prepare a cross-domain release evidence review";
  ledger.create({
    goal_id: "release-review",
    task_digest: goalTaskDigest(task),
    domain: "engineering",
    capability: "release_review",
    risk_class: "high_review",
    criteria: [{ criterion_id: "evidence", criterion_digest: goalTaskDigest("verified evidence") }],
    max_attempts: 2,
  });
  now = 101;
  ledger.transition("release-review", "running", { expected_revision: 0 });
  now = 102;
  ledger.transition("release-review", "paused", {
    expected_revision: 1,
    criterion_updates: [{ criterion_id: "evidence", status: "satisfied", evidence_digest: goalTaskDigest("receipt") }],
    next_action_digest: goalTaskDigest("operator review"),
  });
  now = 103;
  ledger.transition("release-review", "running", { expected_revision: 2 });
  now = 104;
  const completed = ledger.transition("release-review", "completed", { expected_revision: 3 });
  assert.equal(completed.status, "completed");
  assert.equal(completed.attempt, 2);
  assert.equal(JSON.stringify(completed).includes(task), false);
  assert.equal(ledger.verifyIntegrity().ok, true);
  assert.equal(ledger.stats().statuses.completed, 1);

  const snapshot = ledger.snapshot();
  let persisted = null;
  await new AutonomousGoalPersistenceCoordinator(ledger, { read: () => persisted, write: (next) => { persisted = next; } }).flush();
  const restored = new InMemoryAutonomousGoalLedger({ clock: () => 200 });
  await new AutonomousGoalPersistenceCoordinator(restored, { read: () => persisted, write: () => {} }).restore();
  assert.equal(restored.get("release-review").state_digest, completed.state_digest);
  assert.equal(restored.verifyIntegrity().events, 5);
  const tampered = structuredClone(snapshot);
  tampered.goals[0].status = "failed";
  assert.throws(() => restored.restore(tampered), /snapshot digest mismatch/);
});

test("goal ledger fails closed on conflicts, incomplete criteria, and exhausted retries", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 1 });
  ledger.create({ goal_id: "bounded", task_digest: goalTaskDigest("bounded task"), domain: "operations", criteria: [{ criterion_id: "safe", criterion_digest: goalTaskDigest("safe change") }], max_attempts: 1 });
  assert.throws(() => ledger.transition("bounded", "running", { expected_revision: 9 }), /revision conflict/);
  ledger.transition("bounded", "running", { expected_revision: 0 });
  assert.throws(() => ledger.transition("bounded", "completed", { expected_revision: 1 }), /required criterion/);
  ledger.transition("bounded", "failed", { expected_revision: 1 });
  assert.throws(() => ledger.transition("bounded", "ready", { expected_revision: 2 }), /attempt budget/);
});

test("goal creation is idempotent across clock ticks but rejects identity drift", () => {
  let now = 1;
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => now++ });
  const first = ledger.create({ goal_id: "same", task_digest: goalTaskDigest("same task"), domain: "coding" });
  const second = ledger.create({ goal_id: "same", task_digest: goalTaskDigest("same task"), domain: "coding" });
  assert.equal(second.state_digest, first.state_digest);
  assert.throws(() => ledger.create({ goal_id: "same", task_digest: goalTaskDigest("different task"), domain: "coding" }), /different identity/);
});

test("goal ledger accepts every built-in domain without domain-specific semantics", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: profiles.length });
  for (const profile of profiles) ledger.create({ goal_id: `goal-${profile.domain}`, task_digest: goalTaskDigest(`task for ${profile.domain}`), domain: profile.domain });
  assert.equal(ledger.list({ limit: profiles.length }).length, profiles.length);
  assert.equal(ledger.list({ domain: profiles[0].domain }).length, 1);
  assert.equal(ledger.verifyIntegrity().goals, profiles.length);
});

test("goal execution wrapper uses the same approval lifecycle across every built-in domain", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  agent.run = async () => ({ status: "approval_required" });
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: profiles.length });
  for (const profile of profiles) {
    const step = await agent.runGoalStep(ledger, `wrapper-${profile.domain}`, `bounded work for ${profile.domain}`, profile.domain);
    assert.equal(step.goal_status, "paused");
    assert.equal(step.result_status, "approval_required");
  }
  assert.equal(ledger.stats().statuses.paused, profiles.length);
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("cross-domain goal execution wrapper persists fan-out progress without payloads", async () => {
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  const ledger = new InMemoryAutonomousGoalLedger();
  const subtasks = [{ domain: "coding", task: "inspect" }, { domain: "science", task: "compare" }];
  agent.runCrossDomain = async () => ({ status: "approval_required", child_runs: [], completed_children: 0, total_children: 2 });
  const paused = await agent.runCrossDomainGoalStep(ledger, "cross-domain-goal", "coordinate a bounded cross-domain review", {
    runOptions: { subtasks },
    goalCriteria: [{ criterion_id: "synthesis", criterion_digest: goalTaskDigest("synthesis") }],
  });
  assert.equal(paused.goal_status, "paused");
  assert.equal(paused.goal.domain, "cross_domain");
  assert.ok(paused.progress_digest);
  assert.equal(JSON.stringify(ledger.snapshot()).includes("inspect"), false);
  assert.equal(JSON.stringify(ledger.snapshot()).includes("compare"), false);

  agent.runCrossDomain = async () => ({ status: "completed", child_runs: [{ result: { status: "completed" } }], completed_children: 2, total_children: 2 });
  const completed = await agent.runCrossDomainGoalStep(ledger, "cross-domain-goal", "coordinate a bounded cross-domain review", {
    runOptions: { subtasks },
    criterionUpdates: [{ criterion_id: "synthesis", status: "satisfied", evidence_digest: goalTaskDigest("synthesis receipt") }],
  });
  assert.equal(completed.goal_status, "completed");
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("goal learning wrapper settles evaluator and bandit projections without an API key", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "value-only answer" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("goal-learning-provider", "https://goal-learning.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel({ provider: "goal-learning-provider", model: "goal-learning-model", capabilities: ["reasoning", "code"], context_window_tokens: 16_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 50, cost_per_million_tokens: 1, reliability: 0.95 });
  const learning = new AutonomousLearningController(agent);
  const ledger = new InMemoryAutonomousGoalLedger();
  const result = await agent.runGoalLearningStep(ledger, "goal-learning", "adapt a coding review strategy", "coding", {
    cycleId: "goal-cycle-1",
    learning: { controller: learning, episodePrefix: "goal-learning" },
    runOptions: { approveProviderCall: true, stateStore: new InMemoryAutonomousCycleReplanStateStore() },
    evaluate: () => ({ evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 0.9, passed: true, replan_requested: false }),
    goalCriteria: [{ criterion_id: "quality", criterion_digest: goalTaskDigest("quality") }],
    criterionUpdates: [{ criterion_id: "quality", status: "satisfied", evidence_digest: goalTaskDigest("quality receipt") }],
  });
  assert.equal(result.goal_status, "completed");
  assert.equal(result.learning_mode, "single_domain_replan");
  assert.ok(result.evaluator_digest);
  assert.ok(result.learning_state_digest);
  assert.ok(result.progress_digest);
  assert.equal(result.cycle.learning_episode_ids.length, 1);
  const serialized = JSON.stringify(ledger.snapshot());
  assert.equal(serialized.includes("adapt a coding review strategy"), false);
  assert.equal(serialized.includes("value-only answer"), false);
  assert.equal(serialized.includes("goal-cycle-1"), false);
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("cross-domain goal learning wrapper settles specialist trajectory projections", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "cross-domain value-only answer" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("cross-goal-learning-provider", "https://cross-goal-learning.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel({ provider: "cross-goal-learning-provider", model: "cross-goal-learning-model", capabilities: ["reasoning", "science", "biomedical", "neuroscience", "code", "web", "data", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"], context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 50, cost_per_million_tokens: 1, reliability: 0.95 });
  const learning = new AutonomousLearningController(agent);
  const ledger = new InMemoryAutonomousGoalLedger();
  const subtasks = [{ id: "bio", domain: "biomedical", task: "Review biomedical evidence." }, { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." }];
  const result = await agent.runCrossDomainGoalLearningStep(ledger, "cross-goal-learning", "coordinate biomedical neuroscience evidence review", {
    cycleId: "cross-goal-cycle-1",
    learning: { controller: learning, episodePrefix: "cross-goal-learning", trajectoryIdPrefix: "cross-goal-trajectory" },
    runOptions: { approveProviderCall: true, stateStore: new InMemoryAutonomousCycleReplanStateStore(), subtasks },
    evaluate: (run) => ({
      evaluator_id: "cross-domain-reviewer",
      evaluator_version: "1",
      reward: 0.8,
      passed: true,
      replan_requested: false,
      rewards: Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "cross-domain-reviewer", evaluator_version: "1", reward: 0.8, passed: true }])),
    }),
    goalCriteria: [{ criterion_id: "synthesis", criterion_digest: goalTaskDigest("synthesis") }],
    criterionUpdates: [{ criterion_id: "synthesis", status: "satisfied", evidence_digest: goalTaskDigest("synthesis receipt") }],
  });
  assert.equal(result.goal_status, "completed");
  assert.equal(result.learning_mode, "cross_domain_replan");
  assert.equal(result.cycle.learning_episode_ids.length, 3);
  assert.ok(result.evaluator_digest);
  assert.ok(result.learning_state_digest);
  assert.ok(result.progress_digest);
  const serialized = JSON.stringify(ledger.snapshot());
  assert.equal(serialized.includes("coordinate biomedical"), false);
  assert.equal(serialized.includes("cross-domain value-only answer"), false);
  assert.equal(serialized.includes("cross-goal-cycle-1"), false);
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("goal digest and state identity match the Python reference contract", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  const record = ledger.create({
    goal_id: "parity-goal",
    task_digest: goalTaskDigest("parity task"),
    domain: "coding",
    capability: "review",
    risk_class: "research",
    criteria: [{ criterion_id: "done", criterion_digest: goalTaskDigest("done") }],
    max_attempts: 2,
  });
  assert.equal(goalTaskDigest("parity task"), "75c9dd12cec986f5aa50dcab2416229220e8c2b3e28283c550fb7fad9c8d9841");
  assert.equal(record.state_digest, "553312b08e201b99e81f39761bec11ed2127a9b7873f8e07859d867cdd1912cc");
});

test("goal JSON persistence round-trips through browser storage and rejects unsafe snapshots", async () => {
  const values = new Map();
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
  const browserPersistence = new JsonAutonomousGoalPersistence(new WebStorageAutonomousGoalTextStore(storage, "aurora-goals"));
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 7 });
  ledger.create({ goal_id: "browser-goal", task_digest: goalTaskDigest("browser persistence"), domain: "operations" });
  ledger.transition("browser-goal", "running", { expected_revision: 0, now_ns: 8 });
  const snapshot = ledger.snapshot();
  await browserPersistence.write(snapshot);
  assert.deepEqual(await browserPersistence.read(), snapshot);
  const canonical = values.get("aurora-goals");
  values.set("aurora-goals", JSON.stringify(JSON.parse(canonical), null, 2));
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  values.set("aurora-goals", canonical);

  const inconsistent = structuredClone(snapshot);
  inconsistent.goals[0] = structuredClone(inconsistent.events[0].payload);
  const { snapshot_digest: _snapshotDigest, ...snapshotBody } = inconsistent;
  inconsistent.snapshot_digest = digestJsonSync(snapshotBody);
  assert.throws(() => validateAutonomousGoalSnapshot(inconsistent), /current state is not bound to its latest event/);

  const unsafe = structuredClone(snapshot);
  unsafe.api_key = "must never be persisted";
  assert.throws(() => validateAutonomousGoalSnapshot(unsafe), /unsupported or unsafe metadata/);
  const malformed = structuredClone(snapshot);
  malformed.events[0].payload.secret_material = "accidentally-retained";
  await assert.rejects(() => browserPersistence.write(malformed), /goal snapshot digest mismatch/);
});

test("transactional goal persistence fences stale writers after restart", async () => {
  let encoded = null;
  const store = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const observedDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observedDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousGoalPersistence(store);
  const primary = new InMemoryAutonomousGoalLedger({ clock: () => 10 });
  primary.create({ goal_id: "cas-goal", task_digest: goalTaskDigest("compare and swap"), domain: "coding" });
  const primaryCoordinator = new AutonomousGoalPersistenceCoordinator(primary, persistence);
  await primaryCoordinator.flush();

  const stale = new InMemoryAutonomousGoalLedger({ clock: () => 11 });
  const staleCoordinator = new AutonomousGoalPersistenceCoordinator(stale, persistence);
  await staleCoordinator.restore();
  primary.transition("cas-goal", "running", { expected_revision: 0, now_ns: 12 });
  await primaryCoordinator.flush();
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);

  const recovered = new InMemoryAutonomousGoalLedger({ clock: () => 13 });
  const recoveredCoordinator = new AutonomousGoalPersistenceCoordinator(recovered, persistence);
  await recoveredCoordinator.restore();
  assert.equal(recovered.get("cas-goal").status, "running");
  assert.equal(recovered.verifyIntegrity().ok, true);
});
