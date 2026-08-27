import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousGoalPersistenceCoordinator,
  AutonomousGoalScheduler,
  AutonomousGoalWorker,
  AutonomousGoalControlLoop,
  AutonomousGoalBanditLearner,
  AutonomousGoalControlLoopPersistenceCoordinator,
  AutonomousGoalRecoveryCoordinator,
  TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence,
  sealAutonomousGoalControlLoopSnapshot,
  validateAutonomousGoalControlLoopSnapshot,
  AutonomousGoalAgentRuntime,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  AutonomousActionAdmissionController,
  AutonomousBrainFacade,
  InMemoryAutonomousActionAdmissionLedger,
  AutonomousGoalWorkerJournal,
  AutonomousGoalWorkerJournalPersistenceCoordinator,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  InMemoryAutonomousCycleReplanStateStore,
  InMemoryAutonomousGoalLedger,
  InMemoryAutonomousRunTraceStore,
  JsonAutonomousGoalPersistence,
  JsonAutonomousGoalWorkerJournalPersistence,
  LLMRuntime,
  TransactionalJsonAutonomousGoalPersistence,
  WebStorageAutonomousGoalTextStore,
  builtinAutonomousDomainProfiles,
  AUTONOMOUS_DOMAIN_NAMES,
  claimAutonomousGoals,
  canonicalJson,
  digestJsonSync,
  goalTaskDigest,
  openaiCompatibleProvider,
  scheduleAutonomousGoals,
  validateAutonomousGoalSchedule,
  validateAutonomousGoalSnapshot,
  validateAutonomousGoalRecoveryReport,
} from "../dist/index.js";

test("goal scheduler prioritizes dependency-closed work across every domain", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: AUTONOMOUS_DOMAIN_NAMES.length, clock: () => 0 });
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) ledger.create({ goal_id: `goal-${domain}`, task_digest: goalTaskDigest(`task-${domain}`), domain, now_ns: 0 });
  const schedule = new AutonomousGoalScheduler().plan(ledger.list({ limit: AUTONOMOUS_DOMAIN_NAMES.length }), {
    now_ns: 1_000,
    max_selected: AUTONOMOUS_DOMAIN_NAMES.length,
    max_concurrent: AUTONOMOUS_DOMAIN_NAMES.length,
    required_domains: [...AUTONOMOUS_DOMAIN_NAMES],
    signals: [
      { goal_id: "goal-coding", priority: 0.2 },
      { goal_id: "goal-science", priority: 1, urgency: 1, dependencies: ["goal-coding"] },
    ],
  });
  assert.ok(schedule.selected_goal_ids.indexOf("goal-coding") < schedule.selected_goal_ids.indexOf("goal-science"));
  assert.deepEqual(new Set(schedule.selected_goal_ids), new Set(AUTONOMOUS_DOMAIN_NAMES.map((domain) => `goal-${domain}`)));
  assert.deepEqual(schedule.coverage.missing_domains, []);
  assert.deepEqual(schedule.coverage.selected_domains, [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(JSON.stringify(schedule).includes("task-coding"), false);
  assert.equal(validateAutonomousGoalSchedule(schedule).schedule_digest, schedule.schedule_digest);
  assert.equal(schedule.schedule_digest, "30451f0e55e23ad929f23415a2ffe0a9281e3c3632c51ac9420d00995c789654");
});

test("goal scheduler enforces cycles, budgets, retry policy, and stale claims", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 8, clock: () => 20 });
  ledger.create({ goal_id: "base", task_digest: goalTaskDigest("base task"), domain: "coding", now_ns: 0 });
  ledger.create({ goal_id: "dependent", task_digest: goalTaskDigest("dependent task"), domain: "science", now_ns: 0 });
  ledger.create({ goal_id: "cycle-a", task_digest: goalTaskDigest("cycle a"), domain: "data", now_ns: 0 });
  ledger.create({ goal_id: "cycle-b", task_digest: goalTaskDigest("cycle b"), domain: "operations", now_ns: 0 });
  const failed = ledger.create({ goal_id: "retry", task_digest: goalTaskDigest("retry task"), domain: "evaluation", max_attempts: 3, now_ns: 0 });
  const running = ledger.transition(failed.goal_id, "running", { expected_revision: failed.revision, now_ns: 1 });
  ledger.transition(running.goal_id, "failed", { expected_revision: running.revision, now_ns: 2 });
  const schedule = scheduleAutonomousGoals(ledger.list({ limit: 8 }), {
    now_ns: 20,
    max_selected: 2,
    max_concurrent: 2,
    max_cost: 3,
    allow_failed_retry: true,
    signals: [
      { goal_id: "dependent", priority: 1, urgency: 1, dependencies: ["base"], estimated_cost: 2 },
      { goal_id: "cycle-a", dependencies: ["cycle-b"] },
      { goal_id: "cycle-b", dependencies: ["cycle-a"] },
      { goal_id: "retry", priority: 0.1 },
    ],
  });
  const rows = new Map(schedule.rows.map((row) => [row.goal_id, row]));
  assert.equal(rows.get("cycle-a").reason, "dependency_cycle");
  assert.equal(rows.get("cycle-b").reason, "dependency_cycle");
  assert.equal(rows.get("dependent").decision, "admit");
  assert.deepEqual(rows.get("dependent").unmet_dependencies, []);
  assert.equal(schedule.used_cost, 3);
  const claim = claimAutonomousGoals(ledger, schedule, { now_ns: 30 });
  assert.deepEqual(claim.claims.map((item) => item.goal_id), ["base", "dependent"]);
  assert.equal(ledger.get("dependent").status, "running");
  assert.equal(ledger.get("dependent").attempt, 1);
  assert.throws(() => claimAutonomousGoals(ledger, schedule, { now_ns: 31 }), /stale/);
  const tampered = structuredClone(schedule);
  tampered.selected_goal_ids = [];
  assert.throws(() => validateAutonomousGoalSchedule(tampered), /schedule_digest/);
});

test("goal scheduler admits cross-domain objectives", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 100 });
  ledger.create({ goal_id: "cross", task_digest: goalTaskDigest("cross task"), domain: "cross_domain", now_ns: 0 });
  const schedule = scheduleAutonomousGoals(ledger.list({ limit: 2 }), {
    now_ns: 100,
    max_selected: 1,
    max_concurrent: 1,
    required_domains: ["cross_domain"],
  });
  assert.deepEqual(schedule.selected_goal_ids, ["cross"]);
  assert.deepEqual(schedule.coverage.selected_domains, ["cross_domain"]);
  assert.deepEqual(schedule.coverage.missing_domains, []);
});

test("goal worker rehydrates and settles every domain without persisting task values", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 100 });
  for (const domain of domains) ledger.create({ goal_id: `worker-${domain}`, task_digest: goalTaskDigest(`private task for ${domain}`), domain, now_ns: 0 });
  const observedTasks = [];
  const worker = new AutonomousGoalWorker({
    ledger,
    resolver: (goal) => ({ task: `private task for ${goal.domain}`, parameters: { private: true } }),
    executor: async (request) => {
      observedTasks.push(request.task);
      return { status: "completed", settlement_metadata: { progress_digest: goalTaskDigest(`progress ${request.goal.domain}`) } };
    },
  });
  const batch = await worker.run({ schedule_options: { now_ns: 100, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains } });
  assert.equal(observedTasks.length, domains.length);
  assert.equal(batch.runs.length, domains.length);
  assert.ok(batch.runs.every((run) => run.goal_status === "completed"));
  assert.ok(ledger.list({ limit: domains.length }).every((goal) => goal.status === "completed"));
  const publicValue = JSON.stringify(batch.toJSON());
  assert.equal(publicValue.includes("private task for"), false);
  assert.equal(publicValue.includes('"private"'), false);
  assert.equal(batch.toJSON().counts.completed, domains.length);
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("goal worker single-attempt digest matches the Python reference", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "parity", task_digest: goalTaskDigest("private"), domain: "coding", now_ns: 0 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "private" }),
    executor: async () => ({ status: "completed" }),
  }).run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  assert.equal(batch.worker_digest, "ce6809a88e6a2c0c44748f9c3ec9e57b13915d8472f29da35ed8e1c1fc8baad2");
});

test("goal worker converts executor failure into redacted retry state", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "failure", task_digest: goalTaskDigest("private failure"), domain: "operations", now_ns: 0 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "private failure" }),
    executor: async () => { throw new Error("private provider response must not cross the ledger boundary"); },
  }).run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  const run = batch.runs[0];
  assert.equal(run.execution_status, "failed");
  assert.equal(run.goal_status, "failed");
  assert.equal(run.error_class, "Error");
  assert.ok(run.error_digest);
  assert.equal(JSON.stringify(batch.toJSON()).includes("private provider response"), false);
  assert.equal(ledger.get("failure").status, "failed");
  assert.equal(ledger.get("failure").next_action_digest, goalTaskDigest("goal-retry"));
});

test("goal worker refuses task rehydration drift before claiming or dispatching", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "rehydration-drift", task_digest: goalTaskDigest("immutable task"), domain: "coding", now_ns: 0 });
  let executions = 0;
  const worker = new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "different task" }),
    executor: async () => { executions += 1; return { status: "completed" }; },
  });
  await assert.rejects(() => worker.run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }), /task digest/);
  assert.equal(executions, 0);
  assert.equal(ledger.get("rehydration-drift").status, "ready");
});

test("goal worker journals the dispatch boundary and reconciles restart uncertainty", async () => {
  const workerLedger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  workerLedger.create({ goal_id: "journal-worker", task_digest: goalTaskDigest("private journal task"), domain: "coding", now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 123 });
  const worker = new AutonomousGoalWorker({
    ledger: workerLedger,
    journal,
    resolver: () => ({ task: "private journal task", parameters: { secret: true } }),
    executor: async () => ({ status: "completed" }),
  });
  await worker.run({ batch_id: "batch-success", schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  const journalEvents = journal.events({ goal_id: "journal-worker" });
  assert.deepEqual(journalEvents.map((event) => event.phase), ["prepared", "claimed", "dispatch_started", "settled"]);
  assert.equal(journalEvents[0].task_digest, goalTaskDigest("private journal task"));
  assert.ok(journalEvents[0].execution_binding_digest);
  assert.equal(JSON.stringify(journal.snapshot()).includes("private journal task"), false);

  const recoveryLedger = new InMemoryAutonomousGoalLedger({ clock: () => 200 });
  recoveryLedger.create({ goal_id: "pre-dispatch", task_digest: goalTaskDigest("pre task"), domain: "coding", now_ns: 0 });
  recoveryLedger.create({ goal_id: "post-dispatch", task_digest: goalTaskDigest("post task"), domain: "operations", now_ns: 0 });
  recoveryLedger.transition("pre-dispatch", "running", { expected_revision: 0, now_ns: 1 });
  recoveryLedger.transition("post-dispatch", "running", { expected_revision: 0, now_ns: 1 });
  const recoveredJournal = new AutonomousGoalWorkerJournal({ clock: () => 201 });
  const scheduleDigest = goalTaskDigest("schedule");
  const claimDigest = goalTaskDigest("claim");
  recoveredJournal.record({ batch_id: "batch-restart", goal_id: "pre-dispatch", phase: "claimed", attempt: 1, revision: 1, schedule_digest: scheduleDigest, claim_digest: claimDigest, task_digest: goalTaskDigest("pre task"), execution_binding_digest: "a".repeat(64) });
  recoveredJournal.record({ batch_id: "batch-restart", goal_id: "post-dispatch", phase: "claimed", attempt: 1, revision: 1, schedule_digest: scheduleDigest, claim_digest: claimDigest, task_digest: goalTaskDigest("post task"), execution_binding_digest: "b".repeat(64) });
  recoveredJournal.record({ batch_id: "batch-restart", goal_id: "post-dispatch", phase: "dispatch_started", attempt: 1, revision: 1, schedule_digest: scheduleDigest, claim_digest: claimDigest, task_digest: goalTaskDigest("post task"), execution_binding_digest: "b".repeat(64) });
  assert.equal(recoveredJournal.activeFor("post-dispatch").execution_binding_digest, "b".repeat(64));
  assert.throws(() => recoveredJournal.assertNoActive("post-dispatch"), /unreconciled/);
  const recovery = recoveredJournal.recover(recoveryLedger, { now_ns: 202 });
  assert.deepEqual(recovery.recovered.map((item) => item.goal_status), ["paused", "blocked"]);
  assert.equal(recoveryLedger.get("pre-dispatch").next_action_digest, goalTaskDigest("goal-retry"));
  assert.equal(recoveryLedger.get("post-dispatch").next_action_digest, goalTaskDigest("goal-reconciliation-review"));
  assert.deepEqual(recoveredJournal.active(), []);

  const snapshot = recoveredJournal.snapshot();
  const restored = new AutonomousGoalWorkerJournal();
  restored.restore(snapshot);
  assert.equal(restored.head_digest, snapshot.head_digest);
  const tampered = structuredClone(snapshot);
  tampered.events[0].goal_id = "tampered";
  assert.throws(() => restored.restore(tampered), /digest does not match/);

  let encoded = null;
  const persistence = new JsonAutonomousGoalWorkerJournalPersistence({ read: () => encoded, write: (value) => { encoded = value; } });
  const coordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(restored, persistence);
  await coordinator.flush();
  const roundTripped = new AutonomousGoalWorkerJournal();
  await new AutonomousGoalWorkerJournalPersistenceCoordinator(roundTripped, persistence).restore();
  assert.equal(roundTripped.head_digest, restored.head_digest);
  assert.equal(JSON.stringify(encoded).includes("private journal task"), false);
});

test("goal recovery reconciles every domain before exposing a resumable loop", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 100 });
  for (const domain of domains) {
    ledger.create({ goal_id: `recovery-${domain}`, task_digest: goalTaskDigest(`private recovery task ${domain}`), domain, now_ns: 0 });
    ledger.transition(`recovery-${domain}`, "running", { expected_revision: 0, now_ns: 1 });
  }
  const sourceJournal = new AutonomousGoalWorkerJournal({ clock: () => 2 });
  const scheduleDigest = goalTaskDigest("recovery-schedule");
  for (const domain of domains) {
    const goalId = `recovery-${domain}`;
    sourceJournal.record({
      batch_id: "recovery-batch",
      goal_id: goalId,
      phase: domain === "coding" ? "dispatch_started" : "claimed",
      attempt: 1,
      revision: 1,
      schedule_digest: scheduleDigest,
      claim_digest: goalTaskDigest("recovery-claim"),
      task_digest: goalTaskDigest(`private recovery task ${domain}`),
      execution_binding_digest: goalTaskDigest(`private binding ${domain}`),
    });
  }
  const order = [];
  const journalStore = {
    value: canonicalJson(sourceJournal.snapshot()),
    read: () => { order.push("journal-read"); return journalStore.value; },
    write: (value) => { order.push("journal-write"); journalStore.value = value; },
    writeIfUnchanged: (expected, value) => {
      const actual = journalStore.value === null ? null : JSON.parse(journalStore.value).snapshot_digest;
      if (actual !== expected) return false;
      order.push("journal-write");
      journalStore.value = value;
      return true;
    },
  };
  const controlStore = {
    value: null,
    read: () => { order.push("control-read"); return controlStore.value; },
    write: (value) => { order.push("control-write"); controlStore.value = value; },
  };
  const journalCoordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(
    new AutonomousGoalWorkerJournal({ clock: () => 3 }),
    new JsonAutonomousGoalWorkerJournalPersistence(journalStore),
  );
  const controlCoordinator = new AutonomousGoalControlLoopPersistenceCoordinator(
    new (class {
      read() { return controlStore.read() === null ? null : JSON.parse(controlStore.read()); }
      write(value) { controlStore.write(canonicalJson(value)); }
    })(),
  );
  const recovery = new AutonomousGoalRecoveryCoordinator(ledger, journalCoordinator, controlCoordinator);
  const report = await recovery.restore({ now_ns: 4 });
  assert.deepEqual(order, ["journal-read", "journal-write", "control-read"]);
  assert.equal(report.status, "recovered");
  assert.equal(report.active_count_before_recovery, domains.length);
  assert.equal(report.recovered.length, domains.length);
  assert.equal(report.requires_external_reconciliation, true);
  assert.equal(report.ready_to_resume, true);
  assert.equal(report.resume_snapshot, null);
  assert.equal(validateAutonomousGoalRecoveryReport(report).report_digest, report.report_digest);
  const tamperedReport = structuredClone(report);
  tamperedReport.report_digest = "0".repeat(64);
  assert.throws(() => validateAutonomousGoalRecoveryReport(tamperedReport), /report digest/);
  await assert.rejects(() => recovery.resume(new AutonomousGoalControlLoop({ worker: new AutonomousGoalWorker({ ledger, resolver: () => ({ task: "private recovery task coding" }), executor: async () => ({ status: "completed" }) }) }), { resume_snapshot: report.resume_snapshot }), /resume_snapshot is owned/);
  assert.equal(JSON.stringify(report).includes("private recovery task"), false);
  assert.equal(JSON.stringify(report).includes("private binding"), false);
  assert.equal(journalCoordinator.journal.active().length, 0);
  assert.equal(ledger.get("recovery-coding").status, "blocked");
  assert.ok(journalStore.value.includes("reconciled"));

  const executed = [];
  const loop = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      journal: journalCoordinator.journal,
      resolver: (goal) => ({ task: `private recovery task ${goal.domain}` }),
      executor: async (request) => { executed.push(request.goal.goal_id); return { status: "completed" }; },
    }),
  });
  const result = await recovery.resume(loop, {
    schedule_options: { now_ns: 5, max_selected: domains.length, max_concurrent: domains.length, include_paused: true },
    max_cycles: 2,
    checkpoint: (snapshot) => recovery.checkpoint(snapshot),
  });
  assert.equal(result.stop_reason, "no_admissible_work");
  assert.equal(executed.length, domains.length - 1);
  assert.equal(ledger.get("recovery-coding").status, "blocked");
  assert.ok(domains.filter((domain) => domain !== "coding").every((domain) => ledger.get(`recovery-${domain}`).status === "completed"));
});

test("goal agent runtime enforces recovery before invoking a rehydrated task", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 900 });
  ledger.create({ goal_id: "runtime-recovery", task_digest: goalTaskDigest("private runtime recovery task"), domain: "coding", now_ns: 0 });
  ledger.transition("runtime-recovery", "running", { expected_revision: 0, now_ns: 1 });
  const sourceJournal = new AutonomousGoalWorkerJournal({ clock: () => 2 });
  sourceJournal.record({ batch_id: "runtime-recovery-batch", goal_id: "runtime-recovery", phase: "claimed", attempt: 1, revision: 1, schedule_digest: goalTaskDigest("runtime-recovery-schedule"), claim_digest: goalTaskDigest("runtime-recovery-claim"), task_digest: goalTaskDigest("private runtime recovery task"), execution_binding_digest: goalTaskDigest("private runtime binding") });
  const journalStore = {
    value: canonicalJson(sourceJournal.snapshot()),
    read: () => journalStore.value,
    write: (value) => { journalStore.value = value; },
    writeIfUnchanged: (expected, value) => {
      const actual = journalStore.value === null ? null : JSON.parse(journalStore.value).snapshot_digest;
      if (actual !== expected) return false;
      journalStore.value = value;
      return true;
    },
  };
  const controlStore = {
    value: null,
    read: () => controlStore.value,
    write: (value) => { controlStore.value = canonicalJson(value); },
  };
  const journalCoordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(
    new AutonomousGoalWorkerJournal({ clock: () => 3 }),
    new JsonAutonomousGoalWorkerJournalPersistence(journalStore),
  );
  const controlCoordinator = new AutonomousGoalControlLoopPersistenceCoordinator(controlStore);
  const recovery = new AutonomousGoalRecoveryCoordinator(ledger, journalCoordinator, controlCoordinator);
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  agent.run = async () => ({ status: "completed" });
  const runtime = new AutonomousGoalAgentRuntime({ agent, ledger, journal: journalCoordinator.journal, recovery, task_resolver: () => "private runtime recovery task" });
  await assert.rejects(() => runtime.run({ schedule_options: { now_ns: 900, max_selected: 1, max_concurrent: 1, include_paused: true } }), /restore/);
  const report = await runtime.restore({ now_ns: 4 });
  assert.equal(report.status, "recovered");
  const result = await runtime.run({ schedule_options: { now_ns: 901, max_selected: 1, max_concurrent: 1, include_paused: true }, max_cycles: 1 });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(ledger.get("runtime-recovery").status, "completed");
  assert.ok(controlStore.value);
  assert.equal(runtime.metadata().recovery_execution, "ordered_journal_then_control_checkpoint");
  assert.equal(JSON.stringify(recovery.report).includes("private runtime recovery task"), false);
});

test("goal control loop continues all domains and re-admits paused work with fresh signals", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length + 1, clock: () => 100 });
  for (const domain of domains) ledger.create({ goal_id: `loop-${domain}`, task_digest: goalTaskDigest(`private loop task ${domain}`), domain, now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 101 });
  const seenCycles = [];
  const loop = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      journal,
      resolver: (goal) => ({ task: `private loop task ${goal.domain}` }),
      executor: async () => ({ status: "completed" }),
    }),
    batch_id_prefix: "all-domain-loop",
  });
  const result = await loop.run({
    schedule_options: { now_ns: 100, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    options_factory: (context) => {
      seenCycles.push(context.cycle);
      return { signals: [{ goal_id: "loop-coding", priority: 1, urgency: 1 }] };
    },
    max_cycles: 4,
  });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(result.cycles.length, 1);
  assert.equal(result.total_runs, domains.length);
  assert.deepEqual(result.domain_counts, Object.fromEntries(domains.map((domain) => [domain, 1])));
  assert.deepEqual(seenCycles, [1]);
  assert.deepEqual(journal.active(), []);
  assert.equal(JSON.stringify(result.toJSON()).includes("private loop task"), false);

  const retryLedger = new InMemoryAutonomousGoalLedger({ clock: () => 200 });
  retryLedger.create({ goal_id: "paused-loop", task_digest: goalTaskDigest("private paused loop"), domain: "evaluation", now_ns: 0 });
  let calls = 0;
  const resumed = await new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger: retryLedger,
      resolver: () => ({ task: "private paused loop" }),
      executor: async () => ({ status: ++calls === 1 ? "paused" : "completed" }),
    }),
  }).run({ schedule_options: { now_ns: 200, max_selected: 1, max_concurrent: 1, include_paused: true }, max_cycles: 3 });
  assert.equal(resumed.stop_reason, "all_terminal");
  assert.equal(resumed.cycles.length, 2);
  assert.equal(calls, 2);
  assert.equal(retryLedger.get("paused-loop").status, "completed");

  const failureLedger = new InMemoryAutonomousGoalLedger({ clock: () => 300 });
  failureLedger.create({ goal_id: "failed-loop", task_digest: goalTaskDigest("private failed loop"), domain: "operations", max_attempts: 2, now_ns: 0 });
  const failed = await new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger: failureLedger,
      resolver: () => ({ task: "private failed loop" }),
      executor: async () => { throw new Error("private failure"); },
    }),
  }).run({ schedule_options: { now_ns: 300, max_selected: 1, max_concurrent: 1 }, max_cycles: 2 });
  assert.equal(failed.stop_reason, "no_admissible_work");
  assert.equal(failureLedger.get("failed-loop").status, "failed");
});

test("goal control loop settles explicit evaluator credit and adapts every domain", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 400 });
  for (const domain of domains) ledger.create({ goal_id: `eval-${domain}`, task_digest: goalTaskDigest(`private evaluator task ${domain}`), domain, now_ns: 0 });
  const learner = new AutonomousGoalBanditLearner({ exploration: 0.4 });
  const evaluatorCycles = [];
  const result = await new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      resolver: (goal) => ({ task: `private evaluator task ${goal.domain}` }),
      executor: async () => ({ status: "completed" }),
    }),
    evaluator: (cycle) => {
      evaluatorCycles.push(cycle.cycle);
      return cycle.batch.runs.map((run) => ({
        goal_id: run.goal_id,
        evaluator_id: "domain-quality-evaluator",
        evaluator_version: "2026.08",
        reward: run.domain === "coding" ? 1 : 0.25,
        passed: true,
        evidence_digest: goalTaskDigest(`private evidence ${run.goal_id}`),
      }));
    },
    learner,
    batch_id_prefix: "explicit-evaluator-loop",
  }).run({
    schedule_options: { now_ns: 400, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    max_cycles: 2,
  });
  assert.equal(result.stop_reason, "all_terminal");
  assert.deepEqual(evaluatorCycles, [1]);
  assert.equal(result.evaluation_count, domains.length);
  assert.ok(result.evaluation_digest);
  assert.ok(result.learning_state_digest);
  assert.equal(learner.snapshot().generation, 1);
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.evaluator_digest !== null)), new Set([true]));
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.learning_state_digest)), new Set([result.learning_state_digest]));
  assert.equal(result.cycles[0].evaluations.length, domains.length);
  const publicResult = JSON.stringify(result.toJSON());
  assert.equal(publicResult.includes("private evaluator task"), false);
  assert.equal(publicResult.includes("private evidence"), false);
  assert.equal(publicResult.includes("domain-quality-evaluator"), false);
  assert.equal(ledger.verifyIntegrity().ok, true);

  const invalidLedger = new InMemoryAutonomousGoalLedger({ clock: () => 500 });
  invalidLedger.create({ goal_id: "invalid-eval", task_digest: goalTaskDigest("private invalid evaluator task"), domain: "coding", now_ns: 0 });
  await assert.rejects(() => new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({ ledger: invalidLedger, resolver: () => ({ task: "private invalid evaluator task" }), executor: async () => ({ status: "completed" }) }),
    evaluator: () => [{ goal_id: "invalid-eval", evaluator_id: "bad", evaluator_version: "1", reward: 2, passed: true }],
  }).run({ schedule_options: { now_ns: 500, max_selected: 1, max_concurrent: 1 } }), /reward/);
});

test("goal control checkpoints restart bandit state and fence tampering across every domain", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 550 });
  for (const domain of domains) ledger.create({ goal_id: `checkpoint-${domain}`, task_digest: goalTaskDigest(`private checkpoint task ${domain}`), domain, now_ns: 0 });
  let paused = true;
  const snapshots = [];
  const evaluate = (cycle) => cycle.batch.runs.map((run) => ({
    goal_id: run.goal_id,
    evaluator_id: "checkpoint-evaluator",
    evaluator_version: "1",
    reward: 0.75,
    passed: !paused,
  }));

  const first = await new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      resolver: (goal) => ({ task: `private checkpoint task ${goal.domain}` }),
      executor: async () => ({ status: paused ? "paused" : "completed" }),
    }),
    evaluator: evaluate,
    batch_id_prefix: "checkpoint-all-domains",
  }).run({
    run_id: "checkpoint-all-domains",
    schedule_options: { now_ns: 550, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    max_cycles: 1,
    checkpoint: (snapshot) => { snapshots.push(snapshot); },
  });
  assert.equal(first.stop_reason, "cycle_budget_exhausted");
  assert.equal(first.cycles[0].cycle, 1);
  assert.equal(snapshots[0].completed_cycles, 1);
  assert.equal(snapshots[0].learner_state.generation, 1);
  assert.equal(JSON.stringify(snapshots[0]).includes("private checkpoint task"), false);
  assert.equal(JSON.stringify(snapshots[0]).includes("checkpoint-evaluator"), false);

  paused = false;
  const resumed = await new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      resolver: (goal) => ({ task: `private checkpoint task ${goal.domain}` }),
      executor: async () => ({ status: "completed" }),
    }),
    evaluator: evaluate,
    batch_id_prefix: "checkpoint-all-domains",
  }).run({
    run_id: "checkpoint-all-domains",
    resume_snapshot: snapshots.at(-1),
    schedule_options: { now_ns: 551, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    max_cycles: 3,
    checkpoint: (snapshot) => { snapshots.push(snapshot); },
  });
  assert.equal(resumed.stop_reason, "all_terminal");
  assert.equal(resumed.restored_cycle_count, 1);
  assert.equal(resumed.cycles[0].cycle, 2);
  assert.equal(resumed.evaluation_count, domains.length * 2);
  assert.equal(snapshots.at(-1).generation, 2);
  assert.equal(snapshots.at(-1).previous_snapshot_digest, snapshots[0].snapshot_digest);
  assert.equal(snapshots.at(-1).learner_state.generation, 2);
  assert.ok(ledger.list({ limit: domains.length }).every((goal) => goal.status === "completed"));

  let encoded = null;
  const persistence = new TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
    write_if_unchanged: (expected, value) => {
      const actual = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actual !== expected) return false;
      encoded = value;
      return true;
    },
  });
  const coordinator = new AutonomousGoalControlLoopPersistenceCoordinator(persistence);
  await coordinator.flush(snapshots[0]);
  const restored = await coordinator.restore();
  assert.equal(restored.snapshot_digest, snapshots[0].snapshot_digest);
  const tampered = structuredClone(restored);
  tampered.total_runs += 1;
  assert.throws(() => validateAutonomousGoalControlLoopSnapshot(tampered), /digest mismatch|aggregate counts/);

  const stale = new AutonomousGoalControlLoopPersistenceCoordinator(persistence);
  assert.equal((await stale.restore()).snapshot_digest, restored.snapshot_digest);
  const nextDescriptor = structuredClone(restored);
  delete nextDescriptor.snapshot_digest;
  nextDescriptor.generation = 2;
  nextDescriptor.previous_snapshot_digest = restored.snapshot_digest;
  nextDescriptor.stop_reason = "cycle_budget_exhausted";
  const nextSnapshot = sealAutonomousGoalControlLoopSnapshot(nextDescriptor);
  await coordinator.flush(nextSnapshot);
  await assert.rejects(() => stale.flush(nextSnapshot), /compare-and-swap/);
});

test("goal control checkpoint digest matches the Python reference", () => {
  const snapshot = sealAutonomousGoalControlLoopSnapshot({
    schema: "bioprism-autonomous-goal-control-checkpoint/0.1",
    run_id: "parity-fixture",
    next_cycle: 1,
    cycle_summaries: [],
    previous_cycle: null,
    completed_cycles: 0,
    total_selected: 0,
    total_claimed: 0,
    total_runs: 0,
    status_counts: {},
    domain_counts: {},
    evaluation_count: 0,
    evaluation_digests: [],
    learning_state_digest: null,
    learned_signals: [],
    learner_state: null,
    stop_reason: "cycle_budget_exhausted",
    generation: 1,
    previous_snapshot_digest: null,
    retention: "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
    secret_material: "never_returned",
  });
  assert.equal(snapshot.snapshot_digest, "6781485690bc2aa87d5c4992de3017de959f236084f37f0d285cb1cd897ec5fb");
});

test("goal agent runtime bridges the real facade across every domain without retaining runtime values", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 600 });
  for (const domain of domains) ledger.create({ goal_id: `agent-${domain}`, task_digest: goalTaskDigest(`private agent task ${domain}`), domain, now_ns: 0 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in bridge test"); } }));
  const calls = [];
  agent.run = async (task, options) => {
    calls.push({ kind: "single", task, options });
    return { status: "completed" };
  };
  agent.runCrossDomain = async (task, options) => {
    calls.push({ kind: "cross", task, options });
    return { status: "completed" };
  };
  const runtime = new AutonomousGoalAgentRuntime({
    agent,
    ledger,
    task_resolver: (goal) => `private agent task ${goal.domain}`,
    run_options_factory: (goal) => ({
      private_runtime_handle: { token: `private-${goal.goal_id}` },
      ...(goal.domain === "cross_domain" ? { subtasks: [{ domain: "coding", task: "private child task" }] } : {}),
    }),
    evaluator: (cycle) => cycle.batch.runs.map((run) => ({ goal_id: run.goal_id, evaluator_id: "agent-runtime-evaluator", evaluator_version: "1", reward: 0.75, passed: true })),
  });
  const result = await runtime.run({ schedule_options: { now_ns: 600, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains } });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(result.evaluation_count, domains.length);
  assert.equal(calls.length, domains.length);
  assert.deepEqual(new Set(calls.map((call) => call.kind)), new Set(["single", "cross"]));
  const crossCall = calls.find((call) => call.kind === "cross");
  assert.equal(crossCall.options.subtasks[0].task, "private child task");
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.status)), new Set(["completed"]));
  const serialized = JSON.stringify(result.toJSON());
  assert.equal(serialized.includes("private agent task"), false);
  assert.equal(serialized.includes("private child task"), false);
  assert.equal(serialized.includes("private_runtime_handle"), false);
  assert.equal(runtime.metadata().domain_count, domains.length);
  assert.equal(runtime.metadata().execution_surface, "autonomous_agent_facade");
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("goal agent runtime uses protected task rehydration across every domain", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const values = new Map();
  const protectedContext = new AutonomousProtectedRehydrationContext({ tenantId: "tenant-a", actorId: "actor-a", sessionId: "session-a", authorizationDigest: "a".repeat(64) });
  const boundary = new AutonomousProtectedRehydrationBoundary(protectedContext, (reference) => values.get(reference.value_digest), { authorizer: () => true, clock: () => 600 });
  const protectedRehydration = new AutonomousProtectedRehydrationAdapter(boundary);
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 600 });
  for (const domain of domains) {
    const task = `protected agent task ${domain}`;
    values.set(goalTaskDigest(task), task);
    ledger.create({ goal_id: `protected-agent-${domain}`, task_digest: goalTaskDigest(task), domain, now_ns: 0 });
  }
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in protected task test"); } }));
  const calls = [];
  agent.run = async (task, options) => { calls.push({ kind: "single", task, options }); return { status: "completed" }; };
  agent.runCrossDomain = async (task, options) => { calls.push({ kind: "cross", task, options }); return { status: "completed" }; };
  const runtime = new AutonomousGoalAgentRuntime({
    agent,
    ledger,
    protected_rehydration: protectedRehydration,
    run_options_factory: (goal) => ({
      private_runtime_handle: { token: `private-${goal.goal_id}` },
      ...(goal.domain === "cross_domain" ? { subtasks: [{ domain: "coding", task: "protected child task" }] } : {}),
    }),
    evaluator: (cycle) => cycle.batch.runs.map((run) => ({ goal_id: run.goal_id, evaluator_id: "protected-agent-evaluator", evaluator_version: "1", reward: 0.75, passed: true })),
  });
  const result = await runtime.run({ schedule_options: { now_ns: 600, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains } });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(result.evaluation_count, domains.length);
  assert.equal(calls.length, domains.length);
  assert.deepEqual(new Set(calls.map((call) => call.kind)), new Set(["single", "cross"]));
  assert.equal(runtime.metadata().task_rehydration, "protected_receipt_adapter_fallback");
  const serialized = JSON.stringify(result.toJSON());
  assert.equal(serialized.includes("protected agent task"), false);
  assert.equal(serialized.includes("protected child task"), false);
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.status)), new Set(["completed"]));
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("goal agent runtime traces the complete adaptive loop across every domain without payload retention", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 650 });
  for (const domain of domains) ledger.create({ goal_id: `trace-agent-${domain}`, task_digest: goalTaskDigest(`private trace task ${domain}`), domain, now_ns: 0 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in trace bridge test"); } }));
  const calls = [];
  let callerObserverBefore = 0;
  let callerObserverAfter = 0;
  let callerSelectionEvents = 0;
  const callerObserver = { before: () => { callerObserverBefore += 1; }, after: () => { callerObserverAfter += 1; } };
  const callerSelectionEventCallback = () => { callerSelectionEvents += 1; };
  const emitLifecycle = async (options) => {
    await options.observer?.before?.({ provider: "local", model: "trace-fixture", kind: "chat", inputTokens: 3, requestedOutputTokens: 2, toolCount: 0 });
    await options.selectionEventCallback?.({ phase: "model_selection_started", status: "running", attempt: 1, failover: false, candidate_count: 1, eligible_candidate_count: 1, strategy: "deterministic_health_utility", selected_provider: null, selected_model: null, selection_digest: null, detail_digest: null, failure_code: null });
    await options.selectionEventCallback?.({ phase: "model_selection_finished", status: "selected", attempt: 1, failover: false, candidate_count: 1, eligible_candidate_count: 1, strategy: "deterministic_health_utility", selected_provider: "local", selected_model: "trace-fixture", selection_digest: "a".repeat(64), detail_digest: null, failure_code: null });
    await options.observer?.after?.({ provider: "local", model: "trace-fixture", kind: "chat", inputTokens: 3, requestedOutputTokens: 2, toolCount: 0 }, { success: true, status: "completed", latencyMs: 1, inputTokens: 3, outputTokens: 2, statusCode: 200 });
  };
  agent.run = async (task, options) => { calls.push({ kind: "single", task, options }); await emitLifecycle(options); return { status: "completed", output: "private provider output" }; };
  agent.runCrossDomain = async (task, options) => { calls.push({ kind: "cross", task, options }); await emitLifecycle(options); return { status: "completed", output: "private cross-domain output" }; };
  const runtime = new AutonomousGoalAgentRuntime({
    agent,
    ledger,
    task_resolver: (goal) => `private trace task ${goal.domain}`,
    run_options_factory: (goal) => ({
      observer: callerObserver,
      selectionEventCallback: callerSelectionEventCallback,
      ...(goal.domain === "cross_domain" ? { subtasks: [{ domain: "coding", task: "private child trace task" }] } : {}),
    }),
    evaluator: (cycle) => cycle.batch.runs.map((run) => ({ goal_id: run.goal_id, evaluator_id: "trace-evaluator", evaluator_version: "1", reward: 1, passed: true })),
  });
  const traceStore = new InMemoryAutonomousRunTraceStore({ clock: () => 650 });
  const traced = await runtime.runWithTrace({
    traceStore,
    runId: "goal-trace-every-domain",
    schedule_options: { now_ns: 650, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    max_cycles: 2,
    max_total_runs: domains.length,
  });
  assert.equal(traced.result.stop_reason, "all_terminal");
  assert.equal(traced.trace.status, "completed");
  assert.equal(traced.trace.provider_invocations, domains.length);
  assert.deepEqual(new Set(traced.trace.domains), new Set(domains));
  const events = traceStore.events({ run_id: "goal-trace-every-domain" });
  assert.ok(events.filter((event) => event.phase === "plan_compiled").length >= domains.length + 1);
  assert.ok(events.some((event) => event.phase === "model_selection_finished" && event.selection_digest === "a".repeat(64)));
  assert.ok(events.some((event) => event.phase === "evaluation_settled"));
  assert.ok(events.some((event) => event.phase === "learning_prepared"));
  const serialized = JSON.stringify(traced);
  assert.equal(serialized.includes("private trace task"), false);
  assert.equal(serialized.includes("private child trace task"), false);
  assert.equal(serialized.includes("private provider output"), false);
  assert.equal(JSON.stringify(traceStore.snapshot()).includes("private provider output"), false);
  assert.equal(calls.length, domains.length);
  assert.equal(callerObserverBefore, domains.length);
  assert.equal(callerObserverAfter, domains.length);
  assert.equal(callerSelectionEvents, domains.length * 2);
  assert.equal(traceStore.verifyIntegrity().verified, true);
});

test("goal agent runtime replays caller-owned action handoffs before the run boundary across every domain", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 800 });
  for (const domain of domains) ledger.create({ goal_id: `handoff-goal-${domain}`, task_digest: goalTaskDigest(`private handoff task ${domain}`), domain, now_ns: 0 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in goal handoff test"); } }));
  const brain = new AutonomousBrainFacade({ agent });
  const calls = [];
  agent.runAuto = async (task, options) => {
    calls.push({ task, options });
    return { status: "completed", execution_status: "completed" };
  };
  const controller = new AutonomousActionAdmissionController(new InMemoryAutonomousActionAdmissionLedger({ maxRecords: domains.length + 1 }));
  const runtime = new AutonomousGoalAgentRuntime({
    agent,
    brain,
    ledger,
    task_resolver: (goal) => `private handoff task ${goal.domain}`,
    action_handoff_resolver: async (goal, _row, task) => {
      const input = goal.domain === "cross_domain"
        ? { task, hints: ["coding", "biomedical"], allow_cross_domain: true }
        : { task, domain: goal.domain, allow_cross_domain: false };
      const plan = await brain.actionPlan(input);
      const actionId = `goal-handoff-${goal.domain}`;
      controller.submit(actionId, plan, {
        approvals: Object.fromEntries(plan.required_approvals.map((gate) => [gate, true])),
        reviewed: true,
        authorizationDigest: "c".repeat(64),
      });
      const handoff = controller.dispatchHandoff(actionId);
      return goal.domain === "cross_domain" ? { handoff, request: { hints: input.hints, allow_cross_domain: true } } : handoff;
    },
    run_options_factory: (goal) => goal.domain === "cross_domain" ? { subtasks: [{ domain: "coding", task: "private child task" }] } : {},
    evaluator: (cycle) => cycle.batch.runs.map((run) => ({ goal_id: run.goal_id, evaluator_id: "handoff-evaluator", evaluator_version: "1", reward: 1, passed: true })),
  });
  const result = await runtime.run({ schedule_options: { now_ns: 800, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains } });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(calls.length, domains.length);
  assert.equal(calls.every((call) => call.options.approveProviderCall === true), true);
  assert.equal(runtime.metadata().execution_surface, "autonomous_goal_action_handoff_facade");
  assert.equal(runtime.metadata().action_handoff_execution, "verified_handoff_replay_before_run_boundary");
  assert.equal(JSON.stringify(result.toJSON()).includes("private handoff task"), false);
  assert.equal(JSON.stringify(result.toJSON()).includes("private child task"), false);
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.status)), new Set(["completed"]));
  assert.equal(ledger.verifyIntegrity().ok, true);
});

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
