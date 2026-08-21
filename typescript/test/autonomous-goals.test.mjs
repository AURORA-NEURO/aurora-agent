import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousGoalPersistenceCoordinator,
  AutonomousAgent,
  InMemoryAutonomousGoalLedger,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  goalTaskDigest,
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
  });
  assert.equal(completed.goal_status, "completed");
  assert.equal(completed.goal.attempt, 2);
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
  assert.equal(record.state_digest, "3d90744da6795394cde9323d93c03b22fccef0de32810a4fdc8fd39f81b8496b");
});
