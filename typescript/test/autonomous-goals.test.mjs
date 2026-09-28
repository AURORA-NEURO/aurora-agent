import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import { test } from "node:test";

import {
  AutonomousGoalPersistenceCoordinator,
  AuthenticatedTransactionalJsonAutonomousGoalPersistence,
  MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalPersistence,
  AutonomousGoalScheduler,
  AutonomousGoalWorker,
  AutonomousGoalControlLoop,
  AutonomousGoalBanditLearner,
  AutonomousGoalControlLoopPersistenceCoordinator,
  JsonAutonomousGoalControlLoopSnapshotPersistence,
  AutonomousGoalRecoveryCoordinator,
  TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence,
  sealAutonomousGoalControlLoopSnapshot,
  migrateLegacyAutonomousGoalControlLoopSnapshot,
  validateAutonomousGoalControlLoopSnapshot,
  AutonomousGoalAgentRuntime,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  AutonomousActionAdmissionController,
  AutonomousBrainFacade,
  InMemoryAutonomousActionAdmissionLedger,
  AutonomousGoalWorkerJournal,
  MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence,
  AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence,
  AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA,
  AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01,
  AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
  AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
  AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA_V01,
  AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01,
  AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
  AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
  AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
  validateAutonomousGoalDispatchResolution,
  AutonomousGoalWorkerJournalPersistenceCoordinator,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  InMemoryAutonomousCycleReplanStateStore,
  InMemoryAutonomousGoalLedger,
  AUTONOMOUS_GOAL_AUTH_SCHEMA,
  AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES,
  InMemoryAutonomousRunTraceStore,
  AutonomousRunTraceRegistry,
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
  hmacSha256HexSync,
  goalTaskDigest,
  openaiCompatibleProvider,
  scheduleAutonomousGoals,
  validateAutonomousGoalSchedule,
  validateAutonomousGoalSnapshot,
  migrateLegacyAutonomousGoalSnapshot,
  migrateLegacyAutonomousGoalWorkerJournalSnapshot,
  migrateLegacyAuthenticatedAutonomousGoalWorkerJournalEnvelope,
  validateAutonomousGoalRecoveryReport,
  InMemoryAutonomousGoalPreviewAdmissionLedger,
  TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence,
  AutonomousGoalPreviewAdmissionPersistenceCoordinator,
  createAutonomousGoalPreviewAdmissionRecord,
  reviewAutonomousGoalPreviewAdmissionRecord,
  revokeAutonomousGoalPreviewAdmissionRecord,
  verifyAutonomousGoalPreviewApproval,
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
  assert.equal(schedule.schedule_digest, "f3415809692fee49b5bca897c2c2376c65570800af1d5875e3770d0b3f6d3587");
});

test("goal schedule snapshots are immutable and remain replayable for claims", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "immutable-plan", task_digest: goalTaskDigest("immutable task"), domain: "coding", now_ns: 0 });
  const schedule = scheduleAutonomousGoals(ledger.list({ limit: 1 }), {
    now_ns: 100,
    max_selected: 1,
    max_concurrent: 1,
    signals: [{ goal_id: "immutable-plan", deadline_ns: "200", dependencies: [] }],
  });

  assert.equal(Object.isFrozen(schedule), true);
  assert.equal(Object.isFrozen(schedule.selected_goal_ids), true);
  assert.equal(Object.isFrozen(schedule.rows), true);
  assert.equal(Object.isFrozen(schedule.rows[0]), true);
  assert.equal(Object.isFrozen(schedule.rows[0].dependencies), true);
  assert.equal(Object.isFrozen(schedule.coverage), true);
  assert.equal(Object.isFrozen(schedule.coverage.required_domains), true);
  assert.throws(() => schedule.selected_goal_ids.push("foreign"), TypeError);
  assert.throws(() => schedule.rows[0].dependencies.push("foreign"), TypeError);
  assert.throws(() => { schedule.rows[0].domain = "biology"; }, TypeError);

  const replayed = validateAutonomousGoalSchedule(schedule);
  assert.notEqual(replayed, schedule);
  assert.equal(replayed.schedule_digest, schedule.schedule_digest);
  assert.equal(Object.isFrozen(replayed.rows[0]), true);
  const claim = claimAutonomousGoals(ledger, schedule, { now_ns: 100 });
  assert.equal(Object.isFrozen(claim), true);
  assert.equal(Object.isFrozen(claim.claims), true);
  assert.equal(Object.isFrozen(claim.claims[0]), true);
  assert.throws(() => { claim.claims[0].goal_id = "foreign"; }, TypeError);
  assert.deepEqual(claim.claims.map((item) => item.goal_id), ["immutable-plan"]);

  const rowWithPayload = structuredClone(schedule);
  rowWithPayload.rows[0].private_task = "must not survive schedule validation";
  const { schedule_digest: _rowDigest, ...rowBody } = rowWithPayload;
  rowWithPayload.schedule_digest = digestJsonSync(rowBody);
  assert.throws(() => validateAutonomousGoalSchedule(rowWithPayload), /schedule row .* unsupported fields/);

  const coverageWithPayload = structuredClone(schedule);
  coverageWithPayload.coverage.private_task = "must not survive schedule validation";
  const { schedule_digest: _coverageDigest, ...coverageBody } = coverageWithPayload;
  coverageWithPayload.schedule_digest = digestJsonSync(coverageBody);
  assert.throws(() => validateAutonomousGoalSchedule(coverageWithPayload), /schedule coverage contains unsupported fields/);
});

test("goal scheduler handles the maximum supported dependency chain without recursion", () => {
  const count = 4_096;
  const goals = Array.from({ length: count }, (_, index) => ({
    goal_id: `goal-${String(index).padStart(4, "0")}`,
    domain: "coding",
    status: "ready",
    revision: 0,
    attempt: 0,
    max_attempts: 1,
    updated_ns: "0",
    blockers: [],
  }));
  const schedule = scheduleAutonomousGoals(goals, {
    now_ns: "100",
    max_selected: 128,
    max_concurrent: 128,
    signals: goals.map((goal, index) => ({
      goal_id: goal.goal_id,
      dependencies: index + 1 < count ? [`goal-${String(index + 1).padStart(4, "0")}`] : [],
    })),
  });
  assert.equal(schedule.rows.length, count);
  assert.equal(schedule.selected_goal_ids.length, 128);
  assert.deepEqual(
    schedule.selected_goal_ids,
    Array.from({ length: 128 }, (_, index) => `goal-${String(count - index - 1).padStart(4, "0")}`),
  );
});

test("schema 0.2 goal replay artifacts require canonical timestamp strings", () => {
  const ledger = new InMemoryAutonomousGoalLedger();
  const goal = ledger.create({ goal_id: "wire-time", task_digest: goalTaskDigest("wire time"), domain: "coding", now_ns: "100" });
  const schedule = scheduleAutonomousGoals([goal], { now_ns: "200", signals: [{ goal_id: goal.goal_id, deadline_ns: "300" }] });
  const numericNow = structuredClone(schedule);
  numericNow.now_ns = 200;
  const { schedule_digest: _oldDigest, ...numericNowBody } = numericNow;
  numericNow.schedule_digest = digestJsonSync(numericNowBody);
  assert.throws(() => validateAutonomousGoalSchedule(numericNow), /canonical decimal-string wire format/);

  const numericDeadline = structuredClone(schedule);
  numericDeadline.rows[0].deadline_ns = 300;
  const { schedule_digest: _deadlineDigest, ...numericDeadlineBody } = numericDeadline;
  numericDeadline.schedule_digest = digestJsonSync(numericDeadlineBody);
  assert.throws(() => validateAutonomousGoalSchedule(numericDeadline), /canonical decimal-string wire format/);

  const previewLoop = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({ ledger, resolver: () => ({ task: "wire time" }), executor: () => ({ status: "completed" }) }),
  });
  const numericNestedSchedule = previewLoop.preview({ schedule_options: { now_ns: "200" } }).toJSON();
  numericNestedSchedule.schedule.now_ns = 200;
  const { schedule_digest: _nestedScheduleDigest, ...nestedScheduleBody } = numericNestedSchedule.schedule;
  numericNestedSchedule.schedule.schedule_digest = digestJsonSync(nestedScheduleBody);
  const { preview_digest: _previewDigest, ...previewBody } = numericNestedSchedule;
  numericNestedSchedule.preview_digest = digestJsonSync(previewBody);
  assert.throws(() => createAutonomousGoalPreviewAdmissionRecord(numericNestedSchedule, { admission_id: "wire-preview", issued_at_ns: "200", expires_at_ns: "300" }), /canonical decimal-string wire format/);
});

test("goal bandit separates capability and risk contexts with deterministic restore", () => {
  const goals = [
    { goal_id: "coding-low-risk", domain: "coding", capability: "implementation", risk_class: "low", status: "ready" },
    { goal_id: "coding-high-risk", domain: "coding", capability: "implementation", risk_class: "high", status: "ready" },
  ];
  const learner = new AutonomousGoalBanditLearner({ exploration: 0.35 });
  learner.update([
    { goal_id: "coding-low-risk", domain: "coding", reward: -1, passed: false },
    { goal_id: "coding-high-risk", domain: "coding", reward: 0.75, passed: true },
  ], goals);
  const snapshot = learner.snapshot();
  assert.equal(snapshot.retention, "value_only_goal_contextual_bandit_state");
  assert.equal(snapshot.arms.length, 2);
  assert.deepEqual(new Set(snapshot.arms.map((row) => row.risk_class)), new Set(["low", "high"]));
  assert.ok(snapshot.arms.every((row) => typeof row.arm_id === "string" && row.arm_id.length === 64));
  assert.equal(snapshot.arms[0].domain, "coding");
  assert.equal(learner.update([], goals).signals[0].goal_id, "coding-high-risk");
  const restored = new AutonomousGoalBanditLearner({ state: snapshot });
  assert.deepEqual(restored.snapshot(), snapshot);
  const checkpoint = sealAutonomousGoalControlLoopSnapshot({
    schema: "bioprism-autonomous-goal-control-checkpoint/0.2",
    run_id: "contextual-bandit-checkpoint",
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
    learner_state: snapshot,
    stop_reason: "cycle_budget_exhausted",
    generation: 1,
    previous_snapshot_digest: null,
    retention: "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
    secret_material: "never_returned",
  });
  assert.deepEqual(validateAutonomousGoalControlLoopSnapshot(checkpoint).learner_state, snapshot);
});

test("goal control loop preview is provider-free and explains all-domain admission", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length + 1, clock: () => 100 });
  for (const domain of domains) ledger.create({ goal_id: `preview-${domain}`, task_digest: goalTaskDigest(`private preview task ${domain}`), domain, now_ns: 0 });
  ledger.create({ goal_id: "preview-blocked", task_digest: goalTaskDigest("private preview blocked task"), domain: "evaluation", now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 101 });
  const calls = { resolve: 0, execute: 0 };
  const learner = new AutonomousGoalBanditLearner();
  const worker = new AutonomousGoalWorker({
    ledger,
    journal,
    resolver: () => { calls.resolve += 1; throw new Error("preview must not rehydrate tasks"); },
    executor: () => { calls.execute += 1; throw new Error("preview must not dispatch work"); },
  });
  const loop = new AutonomousGoalControlLoop({ worker, evaluator: () => [], learner });
  const schedule_options = {
    now_ns: 100,
    max_selected: domains.length,
    max_concurrent: domains.length,
    required_domains: domains,
    signals: [{ goal_id: "preview-blocked", dependencies: ["missing-preview-goal"], priority: 1 }],
  };
  const preview = loop.preview({ schedule_options });
  assert.equal(preview.preview_digest, loop.preview({ schedule_options }).preview_digest);
  assert.equal(preview.status, "admissible_work");
  assert.equal(preview.eligible_goal_count, domains.length + 1);
  assert.deepEqual(new Set(preview.schedule.coverage.selected_domains), new Set(domains));
  assert.deepEqual(preview.schedule.coverage.missing_domains, []);
  assert.deepEqual(preview.dependency_blocked_goal_ids, ["preview-blocked"]);
  assert.equal(preview.reason_counts.dependency_not_ready, 1);
  assert.equal(preview.learning_state_digest, learner.snapshot().state_digest);
  assert.deepEqual(calls, { resolve: 0, execute: 0 });
  assert.deepEqual(journal.events(), []);
  assert.equal(learner.snapshot().generation, 0);
  assert.equal(JSON.stringify(preview).includes("private preview task"), false);
  assert.equal(JSON.stringify(preview).includes("private preview blocked task"), false);
  const previewBrain = new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("preview must not reach a provider"); } })) });
  const facadePreview = previewBrain.previewGoalControlLoop({ runtime: { ledger }, schedule_options });
  assert.equal(facadePreview.schedule.schedule_digest, preview.schedule.schedule_digest);
  const previewRuntime = previewBrain.createGoalAgentRuntime({ ledger });
  assert.equal(previewRuntime.metadata().task_rehydration, "not_configured_preview_only");
  await assert.rejects(() => previewRuntime.run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }), /task rehydration is not configured/);
});

test("goal control loop preview reports terminal and retry-policy-blocked states", () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 200 });
  ledger.create({ goal_id: "preview-completed", task_digest: goalTaskDigest("private completed preview"), domain: "coding", now_ns: 0 });
  ledger.transition("preview-completed", "running", { expected_revision: 0, now_ns: 1 });
  ledger.transition("preview-completed", "completed", { expected_revision: 1, now_ns: 2 });
  const terminal = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({ ledger, resolver: () => ({ task: "private completed preview" }), executor: () => ({ status: "completed" }) }),
  }).preview({ schedule_options: { now_ns: 200 } });
  assert.equal(terminal.status, "all_terminal");
  assert.deepEqual(terminal.schedule.selected_goal_ids, []);

  const blockedLedger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 201 });
  blockedLedger.create({ goal_id: "preview-failed", task_digest: goalTaskDigest("private failed preview"), domain: "operations", max_attempts: 1, now_ns: 0 });
  blockedLedger.transition("preview-failed", "running", { expected_revision: 0, now_ns: 1 });
  blockedLedger.transition("preview-failed", "failed", { expected_revision: 1, now_ns: 2 });
  const blocked = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({ ledger: blockedLedger, resolver: () => ({ task: "private failed preview" }), executor: () => ({ status: "completed" }) }),
  }).preview({ schedule_options: { now_ns: 201, allow_failed_retry: true } });
  assert.equal(blocked.status, "no_admissible_work");
  assert.equal(blocked.reason_counts.retry_budget_exhausted, 1);
});

test("goal control loop requires an unchanged preview before dispatch", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 250 });
  ledger.create({ goal_id: "preview-bound-a", task_digest: goalTaskDigest("private preview bound a"), domain: "coding", now_ns: 0 });
  const calls = { resolve: 0, execute: 0 };
  const loop = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({
      ledger,
      resolver: (goal) => {
        calls.resolve += 1;
        return { task: `private preview bound ${goal.goal_id.at(-1)}` };
      },
      executor: () => { calls.execute += 1; return { status: "completed" }; },
    }),
  });
  const schedule_options = { now_ns: 250, max_selected: 1, max_concurrent: 1 };
  const preview = loop.preview({ schedule_options });
  ledger.create({ goal_id: "preview-bound-b", task_digest: goalTaskDigest("private preview bound b"), domain: "science", now_ns: 0 });
  await assert.rejects(() => loop.run({ schedule_options, max_total_runs: 1, expected_preview_digest: preview.preview_digest }), /expected_preview_digest/);
  assert.deepEqual(calls, { resolve: 0, execute: 0 });

  const result = await loop.run({
    schedule_options,
    max_total_runs: 1,
    expected_preview_digest: loop.preview({ schedule_options }).preview_digest,
  });
  assert.equal(result.stop_reason, "run_budget_exhausted");
  assert.deepEqual(calls, { resolve: 1, execute: 1 });
});

test("goal preview admission is operator-reviewed, expiring, persisted, and all-domain bound", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 100 });
  for (const domain of domains) ledger.create({ goal_id: `approval-${domain}`, task_digest: goalTaskDigest(`approval task ${domain}`), domain, now_ns: 0 });
  const calls = { resolve: 0, execute: 0 };
  const resolve = (goal) => { calls.resolve += 1; return { task: `approval task ${goal.domain}` }; };
  const execute = () => { calls.execute += 1; return { status: "completed" }; };
  const schedule_options = { now_ns: 100, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains };
  const loop = new AutonomousGoalControlLoop({ worker: new AutonomousGoalWorker({ ledger, resolver: resolve, executor: execute }) });
  const preview = loop.preview({ schedule_options });
  const admissions = new InMemoryAutonomousGoalPreviewAdmissionLedger({ max_records: 4 });
  const approvalNow = (BigInt(Date.now()) * 1_000_000n).toString();
  const submitted = admissions.submit(preview, {
    admission_id: "all-domain-preview",
    issued_at_ns: approvalNow,
    expires_at_ns: (BigInt(approvalNow) + 1_000_000_000n).toString(),
    requested_by_digest: digestJsonSync("operator-requester"),
    reason: "operator review of the exact all-domain admission",
  });
  const approved = admissions.review("all-domain-preview", {
    approved: true,
    reviewer_digest: digestJsonSync("operator-reviewer"),
    reason: "approved after reviewing the bounded schedule",
    expected_record_digest: submitted.record_digest,
  });
  assert.equal(verifyAutonomousGoalPreviewApproval(approved, { current_preview_digest: preview.preview_digest, now_ns: approvalNow }).status, "approved");
  assert.throws(() => verifyAutonomousGoalPreviewApproval(approved, { current_preview_digest: preview.preview_digest, now_ns: (BigInt(approvalNow) - 1n).toString() }), /not yet valid/);
  assert.throws(() => verifyAutonomousGoalPreviewApproval(approved, { current_preview_digest: preview.preview_digest, now_ns: approved.expires_at_ns }), /expired/);
  const nanosecondReceipt = structuredClone(approved);
  nanosecondReceipt.issued_at_ns = "1700000000000000000";
  nanosecondReceipt.expires_at_ns = "1700000001000000000";
  const { record_digest: _oldApprovalDigest, ...nanosecondBody } = nanosecondReceipt;
  nanosecondReceipt.record_digest = digestJsonSync(nanosecondBody);
  assert.equal(verifyAutonomousGoalPreviewApproval(nanosecondReceipt, { current_preview_digest: preview.preview_digest, now_ns: "1700000000500000000" }).status, "approved");
  const legacyApproval = structuredClone(approved);
  legacyApproval.schema = "bioprism-autonomous-goal-preview-admission-record/0.1";
  assert.throws(() => verifyAutonomousGoalPreviewApproval(legacyApproval, { current_preview_digest: preview.preview_digest, now_ns: approvalNow }), /must be re-reviewed/);

  const rejectedSubmitted = admissions.submit(preview, { admission_id: "rejected-preview", issued_at_ns: 100, expires_at_ns: 100 + 1_000_000_000 });
  const rejected = admissions.review("rejected-preview", { approved: false, reviewer_digest: digestJsonSync("operator-reviewer"), expected_record_digest: rejectedSubmitted.record_digest });
  assert.throws(() => verifyAutonomousGoalPreviewApproval(rejected, { current_preview_digest: preview.preview_digest, now_ns: 100 }), /not approved/);

  const revoked = admissions.revoke("all-domain-preview", {
    reviewer_digest: digestJsonSync("operator-reviewer"),
    reason: "operator policy changed before dispatch",
    expected_record_digest: approved.record_digest,
  });
  assert.equal(revoked.status, "revoked");
  assert.equal(revokeAutonomousGoalPreviewAdmissionRecord(approved, {
    reviewer_digest: digestJsonSync("operator-reviewer"),
    reason: "operator policy changed before dispatch",
    expected_record_digest: approved.record_digest,
  }).record_digest, revoked.record_digest);
  assert.throws(() => verifyAutonomousGoalPreviewApproval(revoked, { current_preview_digest: preview.preview_digest, now_ns: 100 }), /not approved/);
  assert.throws(() => admissions.revoke("all-domain-preview", { reviewer_digest: digestJsonSync("operator-reviewer"), reason: "duplicate revoke" }), /only an approved/);

  const tampered = structuredClone(approved);
  tampered.preview_digest = "0".repeat(64);
  assert.throws(() => admissions.put(tampered), /digest/);

  let encoded = null;
  const persistence = new TransactionalJsonAutonomousGoalPreviewAdmissionSnapshotPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const actual = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actual !== expected) return false;
      encoded = value;
      return true;
    },
  });
  const coordinator = new AutonomousGoalPreviewAdmissionPersistenceCoordinator(admissions, persistence);
  const firstSnapshot = await coordinator.flush();
  const restored = new InMemoryAutonomousGoalPreviewAdmissionLedger({ max_records: 4 });
  const restoredCoordinator = new AutonomousGoalPreviewAdmissionPersistenceCoordinator(restored, persistence);
  assert.equal((await restoredCoordinator.restore()).snapshot_digest, firstSnapshot.snapshot_digest);
  const staleCoordinator = new AutonomousGoalPreviewAdmissionPersistenceCoordinator(new InMemoryAutonomousGoalPreviewAdmissionLedger({ max_records: 4 }), persistence);
  await staleCoordinator.restore();
  admissions.submit(preview, { admission_id: "third-preview", issued_at_ns: 100, expires_at_ns: 100 + 1_000_000_000 });
  await coordinator.flush();
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap/);

  const gatedLedger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 100 });
  for (const domain of domains) gatedLedger.create({ goal_id: `approval-${domain}`, task_digest: goalTaskDigest(`approval task ${domain}`), domain, now_ns: 0 });
  const gatedLoop = new AutonomousGoalControlLoop({
    worker: new AutonomousGoalWorker({ ledger: gatedLedger, resolver: resolve, executor: execute }),
    preview_admission_ledger: admissions,
  });
  await assert.rejects(() => gatedLoop.run({ schedule_options, max_cycles: 1, max_total_runs: domains.length, preview_approval: approved }), /stale relative to the live admission ledger/);
  assert.deepEqual(calls, { resolve: 0, execute: 0 });

  ledger.transition("approval-coding", "running", { expected_revision: 0, now_ns: 101 });
  await assert.rejects(() => loop.run({ schedule_options, max_cycles: 1, max_total_runs: domains.length, preview_approval: approved }), /expected_preview_digest|current preview/);
  assert.deepEqual(calls, { resolve: 0, execute: 0 });
  await assert.rejects(() => loop.run({ schedule_options, max_cycles: 2, max_total_runs: domains.length, preview_approval: approved }), /scoped to one scheduler cycle/);

  const freshLedger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 100 });
  for (const domain of domains) freshLedger.create({ goal_id: `approval-${domain}`, task_digest: goalTaskDigest(`approval task ${domain}`), domain, now_ns: 0 });
  const result = await new AutonomousGoalControlLoop({ worker: new AutonomousGoalWorker({ ledger: freshLedger, resolver: resolve, executor: execute }) }).run({ schedule_options, max_cycles: 1, max_total_runs: domains.length, preview_approval: approved });
  assert.equal(result.stop_reason, "all_terminal");
  assert.deepEqual(calls, { resolve: domains.length, execute: domains.length });
});

test("goal control loop checks approval expiry against live time when the schedule clock is pinned", async () => {
  const originalNow = Date.now;
  let wallClock = 1_000;
  Date.now = () => wallClock;
  try {
    const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 100 });
    ledger.create({ goal_id: "approval-expiry", task_digest: goalTaskDigest("approval expiry task"), domain: "coding", now_ns: 0 });
    let executions = 0;
    const loop = new AutonomousGoalControlLoop({
      worker: new AutonomousGoalWorker({
        ledger,
        resolver: () => ({ task: "approval expiry task" }),
        executor: () => { executions += 1; return { status: "completed" }; },
      }),
    });
    const schedule_options = { now_ns: 100, max_selected: 1, max_concurrent: 1 };
    const preview = loop.preview({ schedule_options });
    const admissions = new InMemoryAutonomousGoalPreviewAdmissionLedger();
    const submitted = admissions.submit(preview, { admission_id: "approval-expiry", issued_at_ns: 1_000, expires_at_ns: 1_100 });
    const approved = admissions.review("approval-expiry", { approved: true, reviewer_digest: digestJsonSync("reviewer"), expected_record_digest: submitted.record_digest });

    wallClock = 1_200;
    await assert.rejects(
      () => loop.run({ schedule_options, max_cycles: 1, max_total_runs: 1, preview_approval: approved }),
      /expired/,
    );
    assert.equal(executions, 0);
    assert.equal(ledger.get("approval-expiry").status, "ready");
  } finally {
    Date.now = originalNow;
  }
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

test("goal worker persists its dispatch intent before invoking the executor", async () => {
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const actualDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actualDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  };
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 10 });
  ledger.create({ goal_id: "persist-before-execute", task_digest: goalTaskDigest("durable task"), domain: "coding", now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 11 });
  const journalPersistence = new JsonAutonomousGoalWorkerJournalPersistence(textStore);
  const journalCoordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(journal, journalPersistence);
  const order = [];
  const worker = new AutonomousGoalWorker({
    ledger,
    journal,
    resolver: () => ({ task: "durable task" }),
    persist_dispatch_intent: async (event) => {
      assert.equal(event.phase, "dispatch_started");
      assert.equal(Object.isFrozen(event), true);
      await journalCoordinator.flush();
      const durable = JSON.parse(encoded);
      assert.equal(durable.events.at(-1).event_digest, event.event_digest);
      order.push("persisted");
    },
    executor: () => {
      const durable = JSON.parse(encoded);
      assert.equal(durable.events.at(-1).phase, "dispatch_started");
      order.push("executor");
      return { status: "completed" };
    },
  });
  await worker.run({ batch_id: "persist-intent-batch", schedule_options: { now_ns: 10, max_selected: 1, max_concurrent: 1 } });
  assert.deepEqual(order, ["persisted", "executor"]);

  let parallelEncoded = null;
  const parallelStore = {
    read: () => parallelEncoded,
    write: (value) => { parallelEncoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const actualDigest = parallelEncoded === null ? null : JSON.parse(parallelEncoded).snapshot_digest;
      if (actualDigest !== expectedDigest) return false;
      parallelEncoded = value;
      return true;
    },
  };
  const parallelLedger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 30 });
  for (const goal_id of ["parallel-persist-a", "parallel-persist-b"]) {
    parallelLedger.create({ goal_id, task_digest: goalTaskDigest(goal_id), domain: "coding", now_ns: 0 });
  }
  const parallelJournal = new AutonomousGoalWorkerJournal({ clock: () => 31 });
  const parallelCoordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(parallelJournal, new JsonAutonomousGoalWorkerJournalPersistence(parallelStore));
  let activePersisters = 0;
  let maxActivePersisters = 0;
  const persistedGoals = new Set();
  await new AutonomousGoalWorker({
    ledger: parallelLedger,
    journal: parallelJournal,
    resolver: (goal) => ({ task: goal.goal_id }),
    persist_dispatch_intent: async (event) => {
      activePersisters += 1;
      maxActivePersisters = Math.max(maxActivePersisters, activePersisters);
      await new Promise((resolve) => setTimeout(resolve, 5));
      await parallelCoordinator.flush();
      persistedGoals.add(event.goal_id);
      activePersisters -= 1;
    },
    executor: (request) => {
      assert.equal(persistedGoals.has(request.goal.goal_id), true);
      return { status: "completed" };
    },
  }).run({ batch_id: "parallel-persist-batch", schedule_options: { now_ns: 30, max_selected: 2, max_concurrent: 2 } });
  assert.equal(maxActivePersisters, 1);

  const refusedLedger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 20 });
  refusedLedger.create({ goal_id: "persist-refused", task_digest: goalTaskDigest("durable refusal"), domain: "coding", now_ns: 0 });
  const refusedJournal = new AutonomousGoalWorkerJournal({ clock: () => 21 });
  let refusedDispatches = 0;
  const refusedWorker = new AutonomousGoalWorker({
    ledger: refusedLedger,
    journal: refusedJournal,
    resolver: () => ({ task: "durable refusal" }),
    persist_dispatch_intent: () => { throw new Error("durable intent write refused"); },
    executor: () => { refusedDispatches += 1; return { status: "completed" }; },
  });
  await assert.rejects(() => refusedWorker.run({ batch_id: "refused-intent-batch", schedule_options: { now_ns: 20, max_selected: 1, max_concurrent: 1 } }), /durable intent write refused/);
  assert.equal(refusedDispatches, 0);
  assert.equal(refusedJournal.activeFor("persist-refused").phase, "dispatch_started");
  refusedJournal.recover(refusedLedger, { now_ns: 22 });
  assert.equal(refusedLedger.get("persist-refused").status, "blocked");
});

test("goal agent runtime persists dispatch intent before invoking the agent facade", async () => {
  let encoded = null;
  const store = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const actualDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actualDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  };
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 40 });
  ledger.create({ goal_id: "runtime-persist-intent", task_digest: goalTaskDigest("runtime persist intent task"), domain: "coding", now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 41 });
  const coordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(
    journal,
    new JsonAutonomousGoalWorkerJournalPersistence(store),
  );
  const order = [];
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  agent.run = async (task, options) => {
    assert.equal(task, "runtime persist intent task");
    assert.equal(options.domain, "coding");
    assert.deepEqual(order, ["persisted"]);
    assert.equal(journal.activeFor("runtime-persist-intent").phase, "dispatch_started");
    const durable = JSON.parse(encoded);
    assert.equal(durable.events.at(-1).phase, "dispatch_started");
    order.push("executor");
    return { status: "completed" };
  };
  const runtime = new AutonomousGoalAgentRuntime({
    agent,
    ledger,
    journal,
    task_resolver: () => "runtime persist intent task",
    persist_dispatch_intent: async (event) => {
      assert.equal(event.phase, "dispatch_started");
      await Promise.resolve();
      await coordinator.flush();
      assert.equal(JSON.parse(encoded).events.at(-1).event_digest, event.event_digest);
      order.push("persisted");
    },
  });

  const result = await runtime.run({
    schedule_options: { now_ns: 40, max_selected: 1, max_concurrent: 1 },
  });

  assert.equal(result.stop_reason, "all_terminal");
  assert.deepEqual(order, ["persisted", "executor"]);
  assert.equal(ledger.get("runtime-persist-intent").status, "completed");
});

test("goal worker uses the Python Unicode whitespace contract for tasks and executor status", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 100 });
  ledger.create({ goal_id: "unicode-worker-status", task_digest: goalTaskDigest("task"), domain: "coding", now_ns: 0 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "task" }),
    executor: () => ({ status: "\u0085completed\u0085" }),
  }).run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  assert.equal(ledger.get("unicode-worker-status").status, "completed");
  assert.equal(batch.runs[0].execution_status, "completed");

  const blankLedger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 100 });
  blankLedger.create({ goal_id: "unicode-blank-task", task_digest: goalTaskDigest("placeholder"), domain: "coding", now_ns: 0 });
  const blankWorker = new AutonomousGoalWorker({
    ledger: blankLedger,
    resolver: () => ({ task: "\u0085" }),
    executor: () => ({ status: "completed" }),
  });
  await assert.rejects(
    () => blankWorker.run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }),
    /resolved task/,
  );
  assert.equal(blankLedger.get("unicode-blank-task").status, "ready");

  const journalWorker = new AutonomousGoalWorker({
    ledger: blankLedger,
    resolver: () => ({ task: "placeholder" }),
    executor: () => ({ status: "completed" }),
    journal: new AutonomousGoalWorkerJournal(),
  });
  await assert.rejects(
    () => journalWorker.run({ batch_id: "\u0085", schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }),
    /batch_id/,
  );
  assert.equal(blankLedger.get("unicode-blank-task").status, "ready");
});

test("goal worker executes independent goals concurrently, then waits for prerequisites", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 3, clock: () => 100 });
  const tasks = { prerequisite: "prerequisite task", independent: "independent task", dependent: "dependent task" };
  for (const [goal_id, task] of Object.entries(tasks)) ledger.create({ goal_id, task_digest: goalTaskDigest(task), domain: "coding", now_ns: 0 });
  let active = 0;
  let maximumActive = 0;
  let firstWaveCount = 0;
  let releaseFirstWave;
  const firstWave = new Promise((resolve) => { releaseFirstWave = resolve; });
  const timer = setTimeout(releaseFirstWave, 100);
  let dependencyObserved = false;
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 101 });
  const worker = new AutonomousGoalWorker({
    ledger,
    journal,
    resolver: (goal) => ({ task: tasks[goal.goal_id] }),
    executor: async (request) => {
      if (["prerequisite", "independent"].includes(request.goal.goal_id)) {
        active += 1;
        maximumActive = Math.max(maximumActive, active);
        firstWaveCount += 1;
        if (firstWaveCount === 2) releaseFirstWave();
        await firstWave;
        active -= 1;
      } else {
        dependencyObserved = ledger.get("prerequisite").status === "completed";
      }
      return { status: "completed" };
    },
  });
  const batch = await worker.run({
    batch_id: "bounded-parallel-wave",
    schedule_options: {
      now_ns: 100,
      max_selected: 3,
      max_concurrent: 3,
      signals: [{ goal_id: "dependent", dependencies: ["prerequisite"] }],
    },
  });
  clearTimeout(timer);
  assert.equal(maximumActive, 2);
  assert.equal(dependencyObserved, true);
  assert.deepEqual(batch.runs.map((run) => run.goal_id), batch.schedule.selected_goal_ids);
  assert.ok(batch.runs.every((run) => run.goal_status === "completed"));
  assert.equal(Object.isFrozen(batch.schedule), true);
  assert.equal(Object.isFrozen(batch.claim), true);
  const jsonProjection = batch.toJSON();
  const selectedGoalIds = [...batch.schedule.selected_goal_ids];
  const claimedGoalId = batch.claim.claims[0].goal_id;
  jsonProjection.schedule.selected_goal_ids.splice(0);
  jsonProjection.claim.claims[0].goal_id = "foreign";
  assert.deepEqual(batch.toJSON().schedule.selected_goal_ids, selectedGoalIds);
  assert.equal(batch.claim.claims[0].goal_id, claimedGoalId);
  for (const goalId of ["prerequisite", "independent", "dependent"]) {
    assert.deepEqual(journal.events({ goal_id: goalId }).map((event) => event.phase), ["prepared", "claimed", "dispatch_started", "settled"]);
  }
  const journalSnapshot = journal.snapshot();
  const restoredJournal = new AutonomousGoalWorkerJournal();
  restoredJournal.restore(journalSnapshot);
  assert.equal(restoredJournal.head_digest, journalSnapshot.head_digest);
  assert.equal(JSON.stringify(journalSnapshot).includes("prerequisite task"), false);
});

test("goal worker drains parallel siblings before releasing its run fence on an unexpected error", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 100 });
  const tasks = { broken: "broken task", sibling: "sibling task" };
  for (const [goal_id, task] of Object.entries(tasks)) ledger.create({ goal_id, task_digest: goalTaskDigest(task), domain: "coding", now_ns: 0 });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 101 });
  const record = journal.record.bind(journal);
  let failSettlementWrite = true;
  journal.record = (input) => {
    if (failSettlementWrite && input.goal_id === "broken" && input.phase === "settled") {
      failSettlementWrite = false;
      throw new Error("injected journal settlement failure");
    }
    return record(input);
  };
  let signalSiblingStarted;
  const siblingStarted = new Promise((resolve) => { signalSiblingStarted = resolve; });
  let releaseSibling;
  const siblingGate = new Promise((resolve) => { releaseSibling = resolve; });
  const worker = new AutonomousGoalWorker({
    ledger,
    journal,
    resolver: (goal) => ({ task: tasks[goal.goal_id] }),
    executor: async (request) => {
      if (request.goal.goal_id === "sibling") {
        signalSiblingStarted();
        await siblingGate;
      }
      return { status: "completed" };
    },
  });
  const firstRun = worker.run({
    batch_id: "drain-before-release",
    schedule_options: { now_ns: 100, max_selected: 2, max_concurrent: 2 },
  }).then(() => "resolved", (error) => error);
  try {
    await siblingStarted;
    const stateWhileSiblingIsRunning = await Promise.race([
      firstRun.then(() => "settled"),
      new Promise((resolve) => setTimeout(() => resolve("pending"), 20)),
    ]);
    assert.equal(stateWhileSiblingIsRunning, "pending");
    await assert.rejects(
      () => worker.run({ batch_id: "overlapping-run", schedule_options: { now_ns: 100, max_selected: 2, max_concurrent: 2 } }),
      /another worker run is already active/,
    );
  } finally {
    releaseSibling();
  }
  const failure = await firstRun;
  assert.ok(failure instanceof Error);
  assert.match(failure.message, /injected journal settlement failure/);
  assert.equal(ledger.get("sibling").status, "completed");
  assert.equal(ledger.get("broken").status, "completed");
  assert.equal(journal.activeFor("broken").phase, "dispatch_started");
  journal.recover(ledger);
  assert.equal(ledger.get("broken").status, "completed");
  assert.equal(journal.activeFor("broken"), undefined);
});

test("goal worker pauses dependent work when its prerequisite settles unsuccessfully", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 100 });
  const tasks = { prerequisite: "prerequisite task", dependent: "dependent task" };
  for (const [goal_id, task] of Object.entries(tasks)) ledger.create({ goal_id, task_digest: goalTaskDigest(task), domain: "coding", now_ns: 0 });
  const dispatched = [];
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: (goal) => ({ task: tasks[goal.goal_id] }),
    executor: async (request) => {
      dispatched.push(request.goal.goal_id);
      return { status: request.goal.goal_id === "prerequisite" ? "blocked" : "completed" };
    },
  }).run({
    schedule_options: {
      now_ns: 100,
      max_selected: 2,
      max_concurrent: 2,
      signals: [{ goal_id: "dependent", dependencies: ["prerequisite"] }],
    },
  });

  assert.deepEqual(dispatched, ["prerequisite"]);
  assert.equal(ledger.get("dependent").status, "paused");
  const dependentRun = batch.runs.find((run) => run.goal_id === "dependent");
  assert.equal(dependentRun.dispatched, false);
  assert.equal(dependentRun.execution_status, "paused");
});

test("goal worker callback snapshots cannot be mutated by executor callbacks", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 100 });
  const tasks = { prerequisite: "prerequisite task", dependent: "dependent task" };
  for (const [goal_id, task] of Object.entries(tasks)) ledger.create({ goal_id, task_digest: goalTaskDigest(task), domain: "coding", now_ns: 0 });
  const rows = new Map();
  const goals = new Map();
  const dispatched = [];
  let mutationRejected = false;
  let goalMutationRejected = false;
  let requestMutationRejected = false;

  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: (goal, row) => {
      rows.set(goal.goal_id, row);
      goals.set(goal.goal_id, goal);
      return { task: tasks[goal.goal_id] };
    },
    executor: (request) => {
      dispatched.push(request.goal.goal_id);
      if (request.goal.goal_id === "prerequisite") {
        try {
          request.task = "changed after the binding digest was sealed";
        } catch (error) {
          if (!(error instanceof TypeError)) throw error;
          requestMutationRejected = true;
        }
        try {
          rows.get("dependent").dependencies.splice(0);
        } catch (error) {
          if (!(error instanceof TypeError)) throw error;
          mutationRejected = true;
        }
        try {
          goals.get("dependent").domain = "biology";
        } catch (error) {
          if (!(error instanceof TypeError)) throw error;
          goalMutationRejected = true;
        }
        return { status: "blocked" };
      }
      return { status: "completed" };
    },
  }).run({
    schedule_options: {
      now_ns: 100,
      max_selected: 2,
      max_concurrent: 2,
      signals: [{ goal_id: "dependent", dependencies: ["prerequisite"] }],
    },
  });

  assert.equal(mutationRejected, true);
  assert.equal(goalMutationRejected, true);
  assert.equal(requestMutationRejected, true);
  assert.equal(goals.get("dependent").domain, "coding");
  assert.deepEqual(dispatched, ["prerequisite"]);
  assert.equal(ledger.get("dependent").status, "paused");
  assert.equal(batch.runs.find((run) => run.goal_id === "dependent").dispatched, false);
});

test("goal worker single-attempt digest matches the Python reference", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "parity", task_digest: goalTaskDigest("private"), domain: "coding", now_ns: 0 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "private" }),
    executor: async () => ({ status: "completed" }),
  }).run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  assert.equal(batch.worker_digest, "9506fc32a9c6101fd6187d9b3df12c1ffe679115e5b189512903ee83c4b3e2ab");
});

test("goal worker detaches nested parameters before binding digest and dispatch", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 2, clock: () => 100 });
  const tasks = { "a-target": "target task", "z-mutator": "mutator task" };
  ledger.create({ goal_id: "a-target", task_digest: goalTaskDigest(tasks["a-target"]), domain: "coding", now_ns: 0 });
  ledger.create({ goal_id: "z-mutator", task_digest: goalTaskDigest(tasks["z-mutator"]), domain: "operations", now_ns: 0 });
  const resolverParameters = JSON.parse('{"nested":{"values":["admitted"],"__proto__":{"flag":"safe"}}}');
  const observed = [];
  const executorMutationRejected = { object: false, array: false };
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 101 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    journal,
    resolver: (goal) => {
      if (goal.goal_id === "z-mutator") resolverParameters.nested.values[0] = "changed after admission";
      return {
        task: tasks[goal.goal_id],
        parameters: goal.goal_id === "a-target" ? resolverParameters : {},
      };
    },
    executor: async (request) => {
      if (request.goal.goal_id === "a-target") {
        try {
          request.parameters.nested.values[0] = "changed by executor";
        } catch (error) {
          if (!(error instanceof TypeError)) throw error;
          executorMutationRejected.object = true;
        }
        try {
          request.parameters.nested.values.push("changed by executor");
        } catch (error) {
          if (!(error instanceof TypeError)) throw error;
          executorMutationRejected.array = true;
        }
        observed.push([
          request.parameters.nested.values[0],
          Object.prototype.hasOwnProperty.call(request.parameters.nested, "__proto__"),
          request.parameters.nested["__proto__"].flag,
          digestJsonSync({ parameters: request.parameters }),
        ]);
      }
      return { status: "completed" };
    },
  }).run({
    batch_id: "detached-parameters",
    schedule_options: {
      now_ns: 100,
      max_selected: 2,
      max_concurrent: 2,
      required_domains: ["coding", "operations"],
      signals: [
        { goal_id: "a-target", priority: 1 },
        { goal_id: "z-mutator", priority: 0 },
      ],
    },
  });

  assert.deepEqual(batch.schedule.selected_goal_ids, ["a-target", "z-mutator"]);
  assert.deepEqual(observed, [["admitted", true, "safe", journal.events({ goal_id: "a-target" })[0].execution_binding_digest]]);
  assert.deepEqual(resolverParameters.nested.values, ["changed after admission"]);
  assert.deepEqual(executorMutationRejected, { object: true, array: true });
  assert.ok(batch.runs.every((run) => run.goal_status === "completed"));
});

test("goal worker blocks unknown executor outcome until reconciliation", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  ledger.create({ goal_id: "failure", task_digest: goalTaskDigest("private failure"), domain: "operations", now_ns: 0 });
  const batch = await new AutonomousGoalWorker({
    ledger,
    resolver: () => ({ task: "private failure" }),
    executor: async () => { throw new Error("private provider response must not cross the ledger boundary"); },
  }).run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } });
  const run = batch.runs[0];
  assert.equal(run.execution_status, "blocked");
  assert.equal(run.goal_status, "blocked");
  assert.equal(run.error_class, "Error");
  assert.ok(run.error_digest);
  assert.equal(JSON.stringify(batch.toJSON()).includes("private provider response"), false);
  assert.equal(ledger.get("failure").status, "blocked");
  assert.deepEqual(ledger.get("failure").blockers, ["worker_dispatch_outcome_requires_reconciliation"]);
  assert.equal(ledger.get("failure").next_action_digest, goalTaskDigest("goal-reconciliation-review"));
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

test("goal worker rejects malformed Unicode task and batch identities before claim", async () => {
  const taskLedger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  taskLedger.create({ goal_id: "unicode-task", task_digest: goalTaskDigest("valid task"), domain: "coding", now_ns: 0 });
  let taskExecutions = 0;
  const taskWorker = new AutonomousGoalWorker({
    ledger: taskLedger,
    resolver: () => ({ task: "invalid task \uD800" }),
    executor: async () => { taskExecutions += 1; return { status: "completed" }; },
  });
  await assert.rejects(() => taskWorker.run({ schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }), /resolved task/);
  assert.equal(taskExecutions, 0);
  assert.equal(taskLedger.get("unicode-task").status, "ready");

  const batchLedger = new InMemoryAutonomousGoalLedger({ clock: () => 100 });
  batchLedger.create({ goal_id: "unicode-batch", task_digest: goalTaskDigest("valid batch task"), domain: "coding", now_ns: 0 });
  let resolutions = 0;
  const batchWorker = new AutonomousGoalWorker({
    ledger: batchLedger,
    journal: new AutonomousGoalWorkerJournal({ clock: () => 101 }),
    resolver: () => { resolutions += 1; return { task: "valid batch task" }; },
    executor: async () => ({ status: "completed" }),
  });
  await assert.rejects(() => batchWorker.run({ batch_id: "invalid batch \uD800", schedule_options: { now_ns: 100, max_selected: 1, max_concurrent: 1 } }), /batch_id/);
  assert.equal(resolutions, 0);
  assert.equal(batchLedger.get("unicode-batch").status, "ready");
});

test("worker journal timestamps use exact ns and legacy snapshots migrate only after chain verification", () => {
  const legacyEventBody = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_EVENT_SCHEMA_V01,
    sequence: 1,
    batch_id: "legacy-batch",
    goal_id: "legacy-goal",
    phase: "prepared",
    attempt: 0,
    revision: 0,
    schedule_digest: "a".repeat(64),
    claim_digest: null,
    outcome_digest: null,
    error_digest: null,
    created_ns: 123,
    previous_digest: "",
    retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
    secret_material: "never_returned",
  };
  const legacyEvent = { ...legacyEventBody, event_digest: digestJsonSync(legacyEventBody) };
  const legacySnapshotBody = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    sequence: 1,
    head_digest: legacyEvent.event_digest,
    events: [legacyEvent],
    retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
    secret_material: "never_returned",
  };
  const legacySnapshot = { ...legacySnapshotBody, snapshot_digest: digestJsonSync(legacySnapshotBody) };

  const migrated = migrateLegacyAutonomousGoalWorkerJournalSnapshot(legacySnapshot, "milliseconds");
  assert.equal(migrated.schema.endsWith("/0.2"), true);
  assert.equal(migrated.events[0].created_ns, "123000000");
  assert.deepEqual(migrated.migration, {
    source_schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    source_timestamp_unit: "milliseconds",
    source_snapshot_digest: legacySnapshot.snapshot_digest,
    source_head_digest: legacyEvent.event_digest,
  });
  assert.equal(migrated.events[0].event_digest, "2603e5e32a5bb01498a652f143d2ed1bec2403298450f6a601dd8a68f1fc2253");
  assert.equal(migrated.snapshot_digest, "1ed7feef8b629d3bec289d036b8808b6adb673a4bc63ac0e978deec6a01c26dd");
  const restored = new AutonomousGoalWorkerJournal();
  restored.restore(migrated);
  assert.deepEqual(restored.snapshot().migration, migrated.migration);
  const exposedSnapshot = restored.snapshot();
  exposedSnapshot.migration.source_timestamp_unit = "nanoseconds";
  assert.equal(restored.snapshot().migration.source_timestamp_unit, "milliseconds");
  const tampered = structuredClone(legacySnapshot);
  tampered.events[0].created_ns += 1;
  assert.throws(() => migrateLegacyAutonomousGoalWorkerJournalSnapshot(tampered, "milliseconds"), /snapshot digest/);

  const oldKey = Uint8Array.from({ length: 32 }, (_, index) => index);
  const currentKey = Uint8Array.from({ length: 32 }, (_, index) => 31 - index);
  const oldAuthBody = { schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01, key_id: "legacy-key", snapshot: legacySnapshot };
  const legacyEnvelope = {
    ...legacySnapshot,
    authentication: {
      schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA_V01,
      key_id: "legacy-key",
      tag: hmacSha256HexSync(new TextEncoder().encode(canonicalJson(oldAuthBody)), oldKey),
    },
  };
  const migratedEnvelope = migrateLegacyAuthenticatedAutonomousGoalWorkerJournalEnvelope(legacyEnvelope, {
    source_timestamp_unit: "milliseconds",
    keys: new Map([["legacy-key", oldKey], ["current-key", currentKey]]),
    active_key_id: "current-key",
  });
  assert.equal(migratedEnvelope.authentication.schema, AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA);
  assert.equal(migratedEnvelope.authentication.key_id, "current-key");
  assert.equal(migratedEnvelope.authentication.tag, "872839d10c95802edfd7a1f0b88e2eec7fed0de36a1dee857c28ee4f682bac86");
  assert.equal(migratedEnvelope.events[0].created_ns, "123000000");
  const tamperedEnvelope = structuredClone(legacyEnvelope);
  tamperedEnvelope.events[0].goal_id = "tampered";
  assert.throws(() => migrateLegacyAuthenticatedAutonomousGoalWorkerJournalEnvelope(tamperedEnvelope, {
    source_timestamp_unit: "milliseconds",
    keys: new Map([["legacy-key", oldKey], ["current-key", currentKey]]),
    active_key_id: "current-key",
  }), /tag does not match/);

  assert.throws(() => validateAutonomousGoalDispatchResolution({
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA_V01,
    goal_id: "legacy-goal",
    attempt: 1,
    dispatch_event_digest: "a".repeat(64),
    execution_binding_digest: "b".repeat(64),
    status: "completed",
    evidence_digest: "c".repeat(64),
    verifier_id: "deployment.status-adapter.v1",
    observed_ns: 123,
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  }), /re-verification and re-issuance/);

  const exact = new AutonomousGoalWorkerJournal({ clock: () => 123 });
  assert.equal(exact.record({ batch_id: "clock", goal_id: "clock", phase: "prepared", attempt: 0, revision: 0, schedule_digest: "d".repeat(64) }).created_ns, "123");
});

test("goal worker journal revisions stay within the cross-language safe integer boundary", () => {
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 0 });
  const accepted = journal.record({
    batch_id: "safe-revision-boundary",
    goal_id: "maximum-safe-revision",
    phase: "prepared",
    attempt: 0,
    revision: Number.MAX_SAFE_INTEGER,
    schedule_digest: "a".repeat(64),
    created_ns: 0,
  });
  assert.equal(accepted.revision, Number.MAX_SAFE_INTEGER);
  const prior = journal.snapshot();
  assert.equal(accepted.event_digest, "3e7a90d87134f4d9ffafac06697e468b9f257acf22b1f56e39884a6391630dee");
  assert.equal(prior.snapshot_digest, "083bb651279e9b5b8d0f090bf02fbfe708f10b2c894c6a25df89dce58bcd9c9d");
  assert.throws(() => journal.record({
    batch_id: "safe-revision-boundary",
    goal_id: "unsafe-revision",
    phase: "prepared",
    attempt: 0,
    revision: Number.MAX_SAFE_INTEGER + 1,
    schedule_digest: "b".repeat(64),
    created_ns: 0,
  }), /revision is outside its integer bounds/);
  assert.deepEqual(journal.snapshot(), prior);
});

test("goal journal rejects non-string genesis chain links after digest restamping", () => {
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 0 });
  journal.record({ batch_id: "bad-chain", goal_id: "bad-chain", phase: "prepared", attempt: 0, revision: 0, schedule_digest: "a".repeat(64) });
  const restamp = (snapshot) => {
    const { event_digest: _eventDigest, ...eventBody } = snapshot.events[0];
    snapshot.events[0].event_digest = digestJsonSync(eventBody);
    snapshot.head_digest = snapshot.events[0].event_digest;
    const { snapshot_digest: _snapshotDigest, ...snapshotBody } = snapshot;
    snapshot.snapshot_digest = digestJsonSync(snapshotBody);
    return snapshot;
  };

  const invalidGenesis = structuredClone(journal.snapshot());
  invalidGenesis.events[0].previous_digest = 0;
  assert.throws(() => AutonomousGoalWorkerJournal.validateSnapshot(restamp(invalidGenesis)), /event.previous_digest must be a string/);

  const missingRequired = structuredClone(journal.snapshot());
  delete missingRequired.events[0].claim_digest;
  assert.throws(() => AutonomousGoalWorkerJournal.validateSnapshot(restamp(missingRequired)), /event is missing required fields/);

  const nullOptional = structuredClone(journal.snapshot());
  nullOptional.events[0].task_digest = null;
  assert.throws(() => AutonomousGoalWorkerJournal.validateSnapshot(restamp(nullOptional)), /event.task_digest/);
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
  await worker.run({ batch_id: "batch-success", schedule_options: { now_ns: "100", max_selected: 1, max_concurrent: 1 } });
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
  assert.deepEqual(recoveredJournal.active().map((event) => [event.goal_id, event.phase]), [["post-dispatch", "reconciled"]]);

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

test("authenticated shared goal journal rotates keys, rejects tampering, and fences stale writers", async () => {
  assert.equal(hmacSha256HexSync(new TextEncoder().encode("Hi There"), new Uint8Array(20).fill(0x0b)), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");
  assert.equal(createHmac("sha256", Buffer.alloc(20, 0x0b)).update("Hi There").digest("hex"), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7");

  const makeStore = () => ({
    value: null,
    read() { return this.value; },
    write(value) { this.value = value; },
    writeIfUnchanged(expected, value) {
      const actual = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
      if (actual !== expected) return false;
      this.value = value;
      return true;
    },
  });
  const makeJournal = (goalId) => {
    const journal = new AutonomousGoalWorkerJournal({ clock: () => 123 });
    journal.record({ batch_id: "shared-batch", goal_id: goalId, phase: "prepared", attempt: 0, revision: 0, schedule_digest: "a".repeat(64), created_ns: 123 });
    return journal;
  };

  assert.throws(() => new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({
    store: { read: () => null, write: () => {} }, keys: new Map([["journal-v1", Uint8Array.from({ length: 32 }, (_, index) => index)]]), active_key_id: "journal-v1",
  }), /requires writeIfUnchanged/);
  const store = makeStore();
  const oldKey = Uint8Array.from({ length: 32 }, (_, index) => index);
  const newKey = Uint8Array.from({ length: 32 }, (_, index) => 31 - index);
  const oldPersistence = new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({ store, keys: new Map([["journal-v1", oldKey]]), active_key_id: "journal-v1" });
  const first = new AutonomousGoalWorkerJournalPersistenceCoordinator(makeJournal("shared-one"), oldPersistence);
  assert.equal(await first.restore(), null);
  const oldSnapshot = await first.flush();
  const oldEnvelope = JSON.parse(store.value);
  assert.equal(oldEnvelope.authentication.schema, AUTONOMOUS_GOAL_WORKER_JOURNAL_AUTH_SCHEMA);
  assert.equal(oldEnvelope.authentication.key_id, "journal-v1");
  assert.equal(oldEnvelope.authentication.tag, "063a74ad880fb8ed0ebe30989038abd6483a71bb3f568197a5de15113d1f7ffa");
  assert.equal(oldEnvelope.snapshot_digest, oldSnapshot.snapshot_digest);
  assert.equal(store.value.includes(Buffer.from(oldKey).toString("hex")), false);

  const rotatingPersistence = new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({
    store, keys: new Map([["journal-v1", oldKey], ["journal-v2", newKey]]), active_key_id: "journal-v2",
  });
  const rotating = new AutonomousGoalWorkerJournalPersistenceCoordinator(new AutonomousGoalWorkerJournal(), rotatingPersistence);
  assert.equal((await rotating.restore()).snapshot_digest, oldSnapshot.snapshot_digest);
  assert.equal((await rotating.flush()).snapshot_digest, oldSnapshot.snapshot_digest);
  assert.equal(JSON.parse(store.value).authentication.key_id, "journal-v1");
  rotating.journal.record({ batch_id: "shared-batch", goal_id: "shared-one", phase: "claimed", attempt: 1, revision: 1, schedule_digest: "a".repeat(64), created_ns: 124 });
  const rotatedSnapshot = await rotating.flush();
  assert.notEqual(rotatedSnapshot.snapshot_digest, oldSnapshot.snapshot_digest);
  assert.equal(JSON.parse(store.value).authentication.key_id, "journal-v2");

  const oldReader = new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({ store, keys: new Map([["journal-v1", oldKey]]), active_key_id: "journal-v1" });
  await assert.rejects(() => oldReader.read(), /key id is not trusted/);
  const tampered = JSON.parse(store.value);
  tampered.authentication.tag = "0".repeat(64);
  store.value = canonicalJson(tampered);
  await assert.rejects(() => rotatingPersistence.read(), /tag does not match/);

  const shared = makeStore();
  const persistenceA = new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({ store: shared, keys: new Map([["journal-v1", oldKey]]), active_key_id: "journal-v1" });
  const persistenceB = new AuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({ store: shared, keys: new Map([["journal-v1", oldKey]]), active_key_id: "journal-v1" });
  const coordinatorA = new AutonomousGoalWorkerJournalPersistenceCoordinator(makeJournal("shared-a"), persistenceA);
  const coordinatorB = new AutonomousGoalWorkerJournalPersistenceCoordinator(makeJournal("shared-b"), persistenceB);
  await coordinatorA.restore();
  await coordinatorB.restore();
  await coordinatorA.flush();
  await assert.rejects(() => coordinatorB.flush(), /compare-and-swap conflict/);
});

test("monotonic journal anchor rejects replay of an older valid signed snapshot", async () => {
  const store = {
    value: null,
    read() { return this.value; },
    write(value) { this.value = value; },
    writeIfUnchanged(expected, value) {
      const actual = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
      if (actual !== expected) return false;
      this.value = value;
      return true;
    },
  };
  const anchor = {
    value: null,
    read() { return this.value === null ? null : structuredClone(this.value); },
    writeIfUnchanged(expected, value) {
      const actual = this.value?.anchor_digest ?? null;
      if (actual !== expected) return false;
      this.value = structuredClone(value);
      return true;
    },
  };
  const persistence = new MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalWorkerJournalPersistence({
    store,
    anchor,
    keys: new Map([["journal-v1", new Uint8Array(32).fill(0x61)]]),
    active_key_id: "journal-v1",
  });
  const journal = new AutonomousGoalWorkerJournal({ clock: () => 123 });
  journal.record({ batch_id: "anchored-batch", goal_id: "anchored-goal", phase: "prepared", attempt: 0, revision: 0, schedule_digest: "a".repeat(64), created_ns: 123 });
  const coordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(journal, persistence);
  assert.equal(await coordinator.restore(), null);
  const oldSnapshot = await coordinator.flush();
  const oldEnvelope = store.value;
  assert.equal(anchor.value.snapshot_digest, oldSnapshot.snapshot_digest);

  journal.record({ batch_id: "anchored-batch", goal_id: "anchored-goal", phase: "claimed", attempt: 1, revision: 1, schedule_digest: "a".repeat(64), created_ns: 124 });
  const newSnapshot = await coordinator.flush();
  assert.ok(newSnapshot.sequence > oldSnapshot.sequence);
  assert.equal(anchor.value.snapshot_digest, newSnapshot.snapshot_digest);

  store.value = oldEnvelope;
  await assert.rejects(() => persistence.read(), /rollback or incomplete commit/);
});

test("verified dispatch outcomes settle exact recovered attempts and keep uncertain status blocked", () => {
  const statusVerifier = (verifier_id = "deployment.status-adapter.v1", accepted = true) => ({ verifier_id, verify: () => accepted });
  const makeRecovered = (goalId, criteria = []) => {
    const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 500 });
    ledger.create({ goal_id: goalId, task_digest: goalTaskDigest(`${goalId} task`), domain: "coding", criteria, now_ns: 0 });
    ledger.transition(goalId, "running", { expected_revision: 0, now_ns: 1 });
    const journal = new AutonomousGoalWorkerJournal({ clock: () => 2 });
    const common = { batch_id: `${goalId}-batch`, goal_id: goalId, attempt: 1, revision: 1, schedule_digest: goalTaskDigest(`${goalId} schedule`), claim_digest: goalTaskDigest(`${goalId} claim`), task_digest: goalTaskDigest(`${goalId} task`), execution_binding_digest: goalTaskDigest(`${goalId} binding`) };
    journal.record({ ...common, phase: "prepared" });
    journal.record({ ...common, phase: "claimed" });
    const dispatch = journal.record({ ...common, phase: "dispatch_started" });
    journal.recover(ledger, { now_ns: 100 });
    return { ledger, journal, dispatch };
  };
  const makeResolution = ({ goalId, dispatch, status, observed_ns }) => ({
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
    goal_id: goalId,
    attempt: 1,
    dispatch_event_digest: dispatch.event_digest,
    execution_binding_digest: dispatch.execution_binding_digest,
    status,
    evidence_digest: goalTaskDigest(`${goalId} status evidence ${status}`),
    verifier_id: "deployment.status-adapter.v1",
    observed_ns,
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  });

  const parityReceipt = validateAutonomousGoalDispatchResolution({
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
    goal_id: "parity-goal",
    attempt: 1,
    dispatch_event_digest: "a".repeat(64),
    execution_binding_digest: "b".repeat(64),
    status: "unknown",
    evidence_digest: "c".repeat(64),
    verifier_id: "status-v1",
    observed_ns: "1720000000000000",
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  });
  assert.equal(digestJsonSync(parityReceipt), "ec699263c7c934dd1b405d2d9ac0667ee7c68f56f0f7a9f341f7c4226d57668d");
  assert.throws(() => validateAutonomousGoalDispatchResolution({ ...parityReceipt, observed_ns: Number.MAX_SAFE_INTEGER + 1 }), /canonical decimal-string wire format/);

  const uncertain = makeRecovered("status-uncertain");
  const wrongBinding = makeResolution({ goalId: "status-uncertain", dispatch: uncertain.dispatch, status: "completed", observed_ns: "200" });
  wrongBinding.execution_binding_digest = "f".repeat(64);
  assert.throws(() => uncertain.journal.reconcileExternalOutcome(uncertain.ledger, wrongBinding), /requires a deployment-owned verifier/);
  assert.throws(() => uncertain.journal.reconcileExternalOutcome(uncertain.ledger, wrongBinding, statusVerifier("other.status-adapter.v1")), /configured status authority/);
  assert.throws(() => uncertain.journal.reconcileExternalOutcome(uncertain.ledger, wrongBinding, statusVerifier("deployment.status-adapter.v1", false)), /verifier rejected/);
  assert.throws(() => uncertain.journal.reconcileExternalOutcome(uncertain.ledger, wrongBinding, statusVerifier()), /execution binding/);
  assert.equal(uncertain.ledger.get("status-uncertain").status, "blocked");
  const pending = makeResolution({ goalId: "status-uncertain", dispatch: uncertain.dispatch, status: "unknown", observed_ns: "200" });
  const held = uncertain.journal.reconcileExternalOutcome(uncertain.ledger, pending, statusVerifier());
  assert.equal(held.goal_status, "blocked");
  assert.equal(uncertain.journal.activeFor("status-uncertain").phase, "reconciled");
  const resumedRecovery = uncertain.journal.recover(uncertain.ledger, { now_ns: 300 });
  assert.equal(resumedRecovery.recovered[0].goal_status, "blocked");
  assert.equal(uncertain.journal.activeFor("status-uncertain").phase, "reconciled");
  const completed = makeResolution({ goalId: "status-uncertain", dispatch: uncertain.dispatch, status: "completed", observed_ns: "400" });
  assert.equal(uncertain.journal.reconcileExternalOutcome(uncertain.ledger, completed, statusVerifier()).goal_status, "completed");
  assert.equal(uncertain.ledger.get("status-uncertain").status, "completed");
  assert.equal(uncertain.journal.activeFor("status-uncertain"), undefined);
  assert.equal(uncertain.journal.reconcileExternalOutcome(uncertain.ledger, completed, statusVerifier()).idempotent, true);
  assert.equal(JSON.stringify(uncertain.journal.snapshot()).includes("status evidence completed"), false);

  const safeRetry = makeRecovered("status-not-applied");
  const notApplied = makeResolution({ goalId: "status-not-applied", dispatch: safeRetry.dispatch, status: "not_applied", observed_ns: "200" });
  assert.equal(safeRetry.journal.reconcileExternalOutcome(safeRetry.ledger, notApplied, statusVerifier()).goal_status, "ready");
  assert.equal(safeRetry.ledger.get("status-not-applied").next_action_digest, goalTaskDigest("goal-retry"));
  assert.equal(safeRetry.journal.activeFor("status-not-applied"), undefined);

  const failed = makeRecovered("status-failed");
  const terminalFailure = makeResolution({ goalId: "status-failed", dispatch: failed.dispatch, status: "failed", observed_ns: "200" });
  assert.equal(failed.journal.reconcileExternalOutcome(failed.ledger, terminalFailure, statusVerifier()).goal_status, "failed");
  assert.equal(failed.journal.activeFor("status-failed"), undefined);

  const criteriaOpen = makeRecovered("status-completed-criteria-open", [{ criterion_id: "evidence-reviewed", criterion_digest: goalTaskDigest("evidence-reviewed") }]);
  const externalCompletion = makeResolution({ goalId: "status-completed-criteria-open", dispatch: criteriaOpen.dispatch, status: "completed", observed_ns: "200" });
  assert.equal(criteriaOpen.journal.reconcileExternalOutcome(criteriaOpen.ledger, externalCompletion, statusVerifier()).goal_status, "paused");
  assert.deepEqual(criteriaOpen.ledger.get("status-completed-criteria-open").blockers, ["required_goal_criteria_review"]);
  assert.equal(criteriaOpen.ledger.get("status-completed-criteria-open").next_action_digest, goalTaskDigest("goal-reconciliation-review"));
  assert.equal(criteriaOpen.journal.activeFor("status-completed-criteria-open"), undefined);
  const criteriaSchedule = scheduleAutonomousGoals([criteriaOpen.ledger.get("status-completed-criteria-open")], { now_ns: 300, max_selected: 1, max_concurrent: 1, include_paused: true });
  assert.deepEqual(criteriaSchedule.selected_goal_ids, []);
});

test("recovery coordinator persists verified dispatch status and refreshes its external-reconciliation marker", async () => {
  const goalId = "coordinated-status-resolution";
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 500 });
  ledger.create({ goal_id: goalId, task_digest: goalTaskDigest(`${goalId} task`), domain: "coding", now_ns: 0 });
  ledger.transition(goalId, "running", { expected_revision: 0, now_ns: 1 });
  const sourceJournal = new AutonomousGoalWorkerJournal({ clock: () => 2 });
  const common = { batch_id: `${goalId}-batch`, goal_id: goalId, attempt: 1, revision: 1, schedule_digest: goalTaskDigest(`${goalId} schedule`), claim_digest: goalTaskDigest(`${goalId} claim`), task_digest: goalTaskDigest(`${goalId} task`), execution_binding_digest: goalTaskDigest(`${goalId} binding`) };
  sourceJournal.record({ ...common, phase: "claimed" });
  const dispatch = sourceJournal.record({ ...common, phase: "dispatch_started" });
  const journalStore = { value: canonicalJson(sourceJournal.snapshot()), read() { return this.value; }, write(value) { this.value = value; } };
  const controlStore = { value: null, read() { return this.value === null ? null : JSON.parse(this.value); }, write(value) { this.value = canonicalJson(value); } };
  const journal = new AutonomousGoalWorkerJournalPersistenceCoordinator(new AutonomousGoalWorkerJournal(), new JsonAutonomousGoalWorkerJournalPersistence(journalStore));
  const control = new AutonomousGoalControlLoopPersistenceCoordinator(controlStore);
  const recovery = new AutonomousGoalRecoveryCoordinator(ledger, journal, control);
  const statusVerifier = { verifier_id: "deployment.status-adapter.v1", verify: () => true };
  const initial = await recovery.restore({ now_ns: 100 });
  assert.equal(initial.requires_external_reconciliation, true);
  const pending = await recovery.reconcileExternalOutcome({
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
    goal_id: goalId,
    attempt: 1,
    dispatch_event_digest: dispatch.event_digest,
    execution_binding_digest: dispatch.execution_binding_digest,
    status: "unknown",
    evidence_digest: goalTaskDigest(`${goalId} unknown external status`),
    verifier_id: "deployment.status-adapter.v1",
    observed_ns: "200",
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  }, statusVerifier);
  assert.equal(pending.resolution.goal_status, "blocked");
  assert.equal(pending.recovery.requires_external_reconciliation, true);
  const repeatedRestore = await recovery.restore({ now_ns: 300 });
  assert.equal(repeatedRestore.requires_external_reconciliation, true);
  assert.equal(repeatedRestore.ready_to_resume, true);
  assert.equal(journal.journal.activeFor(goalId).phase, "reconciled");
  const preResolutionSnapshot = ledger.snapshot();

  const completedReceipt = {
    schema: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_SCHEMA,
    goal_id: goalId,
    attempt: 1,
    dispatch_event_digest: dispatch.event_digest,
    execution_binding_digest: dispatch.execution_binding_digest,
    status: "completed",
    evidence_digest: goalTaskDigest(`${goalId} external status`),
    verifier_id: "deployment.status-adapter.v1",
    observed_ns: "400",
    retention: AUTONOMOUS_GOAL_DISPATCH_RESOLUTION_RETENTION,
    secret_material: "never_returned",
  };
  const originalTransition = ledger.transition.bind(ledger);
  let failBeforeLedgerCommit = true;
  ledger.transition = (...args) => {
    if (failBeforeLedgerCommit) {
      failBeforeLedgerCommit = false;
      throw new Error("injected crash after durable journal stage");
    }
    return originalTransition(...args);
  };
  await assert.rejects(() => recovery.reconcileExternalOutcome(completedReceipt, statusVerifier), /durable journal stage/);
  assert.equal(JSON.parse(journalStore.value).events.at(-1).resolution_goal_status, "completed");
  assert.equal(ledger.get(goalId).status, "blocked");
  ledger.transition = originalTransition;

  const replayed = await recovery.restore({ now_ns: 500 });
  assert.equal(replayed.status, "recovered");
  assert.equal(replayed.requires_external_reconciliation, false);
  assert.equal(replayed.ready_to_resume, true);
  assert.equal(ledger.get(goalId).status, "completed");
  assert.equal(journal.journal.activeFor(goalId), undefined);
  assert.equal(JSON.parse(journalStore.value).events.at(-1).phase, "settled");

  const staleLedger = new InMemoryAutonomousGoalLedger({ clock: () => 900 });
  staleLedger.restore(preResolutionSnapshot);
  const staleJournal = new AutonomousGoalWorkerJournalPersistenceCoordinator(new AutonomousGoalWorkerJournal(), new JsonAutonomousGoalWorkerJournalPersistence(journalStore));
  const staleRecovery = new AutonomousGoalRecoveryCoordinator(staleLedger, staleJournal, new AutonomousGoalControlLoopPersistenceCoordinator(controlStore));
  const staleReport = await staleRecovery.restore({ now_ns: 600 });
  assert.equal(staleReport.requires_external_reconciliation, false);
  assert.equal(staleLedger.get(goalId).status, "completed");
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
  assert.deepEqual(journalCoordinator.journal.active().map((event) => [event.goal_id, event.phase]), [["recovery-coding", "reconciled"]]);
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

test("goal control loop retries paused work but stops on uncertain dispatch", async () => {
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
  assert.equal(failureLedger.get("failed-loop").status, "blocked");
  assert.deepEqual(failureLedger.get("failed-loop").blockers, ["worker_dispatch_outcome_requires_reconciliation"]);
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
    schema: "bioprism-autonomous-goal-control-checkpoint/0.2",
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
  assert.equal(snapshot.snapshot_digest, "043717b3e941042676eaf7cf1c2933fd1a50d09abf78a7bbad00f2608db97f2a");
});

test("legacy goal control checkpoints verify before deadline conversion and keep provenance", () => {
  const legacyBody = {
    schema: AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
    run_id: "legacy-control",
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
    learned_signals: [{ goal_id: "deadline-goal", priority: 0.75, urgency: 0.25, deadline_ns: 123, estimated_cost: 2, dependencies: [] }],
    learner_state: null,
    stop_reason: "cycle_budget_exhausted",
    generation: 1,
    previous_snapshot_digest: null,
    retention: "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
    secret_material: "never_returned",
  };
  const legacy = { ...legacyBody, snapshot_digest: digestJsonSync(legacyBody) };
  const migrated = migrateLegacyAutonomousGoalControlLoopSnapshot(legacy, "milliseconds");
  assert.equal(migrated.learned_signals[0].deadline_ns, "123000000");
  assert.deepEqual(migrated.migration, {
    source_schema: AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
    source_timestamp_unit: "milliseconds",
    source_snapshot_digest: legacy.snapshot_digest,
  });
  assert.equal(migrated.snapshot_digest, "3455302ab636f4f1b1c7293f5c5270ac512b6966396ae23d8c98061424359ca7");
  assert.equal(validateAutonomousGoalControlLoopSnapshot(migrated).snapshot_digest, migrated.snapshot_digest);
  const tampered = structuredClone(legacy);
  tampered.learned_signals[0].deadline_ns += 1;
  assert.throws(() => migrateLegacyAutonomousGoalControlLoopSnapshot(tampered, "milliseconds"), /snapshot digest mismatch/);
  assert.throws(() => migrateLegacyAutonomousGoalControlLoopSnapshot(legacy, "guessed"), /requires milliseconds or nanoseconds/);

  const { snapshot_digest: _sourceDigest, ...nextDescriptor } = migrated;
  nextDescriptor.generation += 1;
  nextDescriptor.previous_snapshot_digest = migrated.snapshot_digest;
  const next = sealAutonomousGoalControlLoopSnapshot(nextDescriptor);
  assert.deepEqual(next.migration, migrated.migration);
});

test("goal agent runtime bridges the real facade across every domain without retaining runtime values", async () => {
  const domains = [...AUTONOMOUS_DOMAIN_NAMES];
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: domains.length, clock: () => 600 });
  for (const domain of domains) ledger.create({
    goal_id: `agent-${domain}`,
    task_digest: goalTaskDigest(`private agent task ${domain}`),
    domain,
    capability: `goal-${domain}-capability`,
    risk_class: `goal-${domain}-risk`,
    now_ns: 0,
  });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in bridge test"); } }));
  const brain = new AutonomousBrainFacade({ agent });
  const calls = [];
  agent.run = async (task, options) => {
    calls.push({ kind: "single", task, options });
    return { status: "completed" };
  };
  agent.runCrossDomain = async (task, options) => {
    calls.push({ kind: "cross", task, options });
    return { status: "completed" };
  };
  const result = await brain.runGoalControlLoop({
    runtime: {
      ledger,
      task_resolver: (goal) => `private agent task ${goal.domain}`,
      run_options_factory: (goal) => ({
        private_runtime_handle: { token: `private-${goal.goal_id}` },
        ...(goal.domain === "cross_domain" ? { subtasks: [{ domain: "coding", task: "private child task" }] } : {}),
      }),
      evaluator: (cycle) => cycle.batch.runs.map((run) => ({ goal_id: run.goal_id, evaluator_id: "agent-runtime-evaluator", evaluator_version: "1", reward: 0.75, passed: true })),
    },
    run: { schedule_options: { now_ns: 600, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains } },
  });
  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(result.evaluation_count, domains.length);
  assert.equal(calls.length, domains.length);
  assert.deepEqual(new Set(calls.map((call) => call.kind)), new Set(["single", "cross"]));
  const crossCall = calls.find((call) => call.kind === "cross");
  assert.equal(crossCall.options.subtasks[0].task, "private child task");
  const singleCalls = calls.filter((call) => call.kind === "single");
  assert.equal(singleCalls.length, domains.length - 1);
  assert.ok(singleCalls.every((call) => call.options.capability === `goal-${call.options.domain}-capability`));
  assert.ok(singleCalls.every((call) => call.options.risk_class === `goal-${call.options.domain}-risk`));
  assert.equal(crossCall.options.capability, "goal-cross_domain-capability");
  assert.equal(crossCall.options.risk_class, "goal-cross_domain-risk");
  assert.deepEqual(new Set(ledger.list({ limit: domains.length }).map((goal) => goal.status)), new Set(["completed"]));
  const serialized = JSON.stringify(result.toJSON());
  assert.equal(serialized.includes("private agent task"), false);
  assert.equal(serialized.includes("private child task"), false);
  assert.equal(serialized.includes("private_runtime_handle"), false);
  assert.equal(brain.agent, agent);
  assert.equal(ledger.verifyIntegrity().ok, true);
});

test("brain goal-control convenience preserves checkpoint run identity across resume", async () => {
  const task = "checkpointed facade convenience task";
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 800 });
  ledger.create({ goal_id: "checkpointed-facade-convenience", task_digest: goalTaskDigest(task), domain: "coding", now_ns: 0 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached in checkpoint facade test"); } }));
  const brain = new AutonomousBrainFacade({ agent });
  const calls = [];
  agent.run = async (receivedTask, options) => {
    calls.push({ task: receivedTask, options });
    return { status: "completed" };
  };
  const snapshots = [];

  const result = await brain.runGoalControlLoop({
    runtime: { ledger, task_resolver: () => task },
    run: {
      run_id: "checkpointed-facade-run",
      checkpoint: (snapshot) => snapshots.push(snapshot),
      schedule_options: { now_ns: 800, max_selected: 1, max_concurrent: 1 },
    },
  });

  assert.equal(result.stop_reason, "all_terminal");
  assert.equal(snapshots.length, 1);
  assert.equal(snapshots[0].run_id, "checkpointed-facade-run");
  assert.equal(JSON.stringify(snapshots[0]).includes(task), false);
  await assert.rejects(() => brain.runGoalControlLoop({
    runtime: { ledger, task_resolver: () => task },
    run: {
      run_id: "different-facade-run",
      resume_snapshot: snapshots[0],
      schedule_options: { now_ns: 801, max_selected: 1, max_concurrent: 1 },
    },
  }), /run_id does not match the resume snapshot/);
  assert.equal(calls.length, 1);
  assert.equal(ledger.get("checkpointed-facade-convenience").status, "completed");
});

test("brain goal runtime factory owns agent and brain bindings before any goal is claimed", () => {
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  const brain = new AutonomousBrainFacade({ agent });
  const otherAgent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("other provider must not be reached"); } }));
  const otherBrain = new AutonomousBrainFacade({ agent: otherAgent });
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 600 });

  // JavaScript callers can still provide forged properties; the factory's spread order must
  // preserve the same binding guarantee as the TypeScript omission type.
  const runtime = brain.createGoalAgentRuntime({
    agent: otherAgent,
    brain: otherBrain,
    ledger,
    task_resolver: () => "private factory task",
  });
  assert.equal(runtime.agent, agent);
  assert.equal(runtime.brain, brain);
  assert.throws(() => brain.createGoalAgentRuntime(null), /options must be an object/);
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
  const brain = new AutonomousBrainFacade({ agent });
  const calls = [];
  agent.run = async (task, options) => { calls.push({ kind: "single", task, options }); return { status: "completed" }; };
  agent.runCrossDomain = async (task, options) => { calls.push({ kind: "cross", task, options }); return { status: "completed" }; };
  const runtime = brain.createGoalAgentRuntime({
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
  const brain = new AutonomousBrainFacade({ agent });
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
  const runtime = brain.createGoalAgentRuntime({
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
  const traceRegistry = new AutonomousRunTraceRegistry({ max_runs: 4_096, max_events: 20_000, max_bytes: 2_000_000 });
  const traced = await runtime.runWithTrace({
    traceStore,
    traceRegistry,
    runId: "goal-trace-every-domain",
    schedule_options: { now_ns: 650, max_selected: domains.length, max_concurrent: domains.length, required_domains: domains },
    max_cycles: 2,
    max_total_runs: domains.length,
  });
  assert.equal(traced.result.stop_reason, "all_terminal");
  assert.equal(traced.trace.status, "completed");
  assert.equal(traced.traceRegistry.status, "published");
  assert.equal(traced.traceRegistry.run_import_state, "imported");
  assert.equal(traceRegistry.query({ run_id: "goal-trace-every-domain" }).total_matches, 1);
  assert.equal(traceRegistry.query({ domain: "neuroscience" }).total_matches, 1);
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
  assert.equal(traceRegistry.verifyIntegrity().verified, true);
});

test("goal agent trace closes startup failures and reserves the runtime during asynchronous trace startup", async () => {
  const ledger = new InMemoryAutonomousGoalLedger({ maxGoals: 1, clock: () => 700 });
  ledger.create({ goal_id: "trace-startup-failure", task_digest: goalTaskDigest("trace startup task"), domain: "coding", now_ns: 0 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }));
  agent.run = async () => ({ status: "completed" });
  const brain = new AutonomousBrainFacade({ agent });
  const runtime = brain.createGoalAgentRuntime({ ledger, task_resolver: () => "trace startup task" });
  const traceStore = new InMemoryAutonomousRunTraceStore({ clock: () => 700 });
  const originalEvents = traceStore.events.bind(traceStore);
  const originalAppend = traceStore.append.bind(traceStore);
  let releaseStartup;
  const startupGate = new Promise((resolve) => { releaseStartup = resolve; });
  let holdFirstRead = true;
  let failPlanWriteOnce = true;
  traceStore.events = async (query) => {
    if (holdFirstRead) {
      holdFirstRead = false;
      await startupGate;
    }
    return originalEvents(query);
  };
  traceStore.append = (event) => {
    if (event.phase === "plan_compiled" && failPlanWriteOnce) {
      failPlanWriteOnce = false;
      throw new Error("injected transient trace-store write failure");
    }
    return originalAppend(event);
  };

  const firstRun = runtime.runWithTrace({ traceStore, runId: "trace-startup-failure", max_cycles: 1 });
  const secondRun = runtime.runWithTrace({ traceStore, runId: "trace-startup-reentry", max_cycles: 1 });
  const untracedRun = (() => {
    try {
      return Promise.resolve(runtime.run({ max_cycles: 1 }));
    } catch (error) {
      return Promise.reject(error);
    }
  })();
  releaseStartup();
  const [firstResult, secondResult, untracedResult] = await Promise.allSettled([firstRun, secondRun, untracedRun]);

  assert.equal(firstResult.status, "rejected");
  assert.match(firstResult.reason.message, /injected transient trace-store write failure/);
  assert.equal(secondResult.status, "rejected");
  assert.match(secondResult.reason.message, /cannot be re-entered/);
  assert.equal(untracedResult.status, "rejected");
  assert.match(untracedResult.reason.message, /another goal runtime operation is already active/);
  const events = await traceStore.events({ run_id: "trace-startup-failure" });
  assert.deepEqual(events.map((event) => event.phase), ["started", "failed"]);
  assert.equal(events.at(-1).failure_code, "goal_control_loop_error");
  assert.equal(events.at(-1).failure_class, "Error");
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
  const runtime = brain.createGoalAgentRuntime({
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
  assert.throws(() => ledger.transition("bounded", "running", { expected_revision: Number.MAX_SAFE_INTEGER + 1 }), /expected_revision must be a non-negative safe integer/);
  assert.equal(ledger.get("bounded").revision, 0);
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
  assert.equal(record.state_digest, "c433d1780c63522fbf3a8fee6476b2617a6421df1879b6331b1c44ea5d515fc3");
  assert.equal(record.created_ns, "100");
});

test("goal snapshot v0.2 preserves exact cross-SDK nanoseconds", () => {
  const ledger = new InMemoryAutonomousGoalLedger();
  const record = ledger.create({ goal_id: "exact-time", task_digest: goalTaskDigest("exact-time task"), domain: "coding", now_ns: "1780000000123456789" });
  const snapshot = ledger.snapshot();
  assert.equal(record.schema, "bioprism-autonomous-goal/0.2");
  assert.equal(record.created_ns, "1780000000123456789");
  assert.equal(snapshot.schema, "bioprism-autonomous-goal-snapshot/0.2");
  assert.equal(snapshot.events[0].created_ns, "1780000000123456789");
  assert.equal(record.state_digest, "56e996525cf194c079e3599c933a6811b7de27ee5e9b69949dfd6929c2e032db");
  assert.equal(snapshot.snapshot_digest, "83740f2f1f02a369914601bbbdca389bf50bdce4a2eaf5ff8d92f8f2254f73b9");
});

test("goal snapshot v0.1 migration verifies original hashes and preserves provenance", () => {
  const ledger = new InMemoryAutonomousGoalLedger();
  ledger.create({ goal_id: "legacy-ms", task_digest: goalTaskDigest("legacy goal"), domain: "coding", now_ns: "123000000" });
  const current = ledger.snapshot();
  const downgradeRecord = (record) => {
    const { state_digest: _stateDigest, retention, secret_material, ...state } = record;
    state.schema = "bioprism-autonomous-goal/0.1";
    state.created_ns = 123;
    state.updated_ns = 123;
    const state_digest = digestJsonSync(state);
    return { ...state, state_digest, retention, secret_material };
  };
  const legacyRecord = downgradeRecord(current.goals[0]);
  const oldEventBody = {
    ...current.events[0],
    schema: "bioprism-autonomous-goal-event/0.1",
    payload: legacyRecord,
    created_ns: 123,
    previous_digest: "",
  };
  delete oldEventBody.event_digest;
  const legacyEvent = { ...oldEventBody, event_digest: digestJsonSync(oldEventBody) };
  const legacyBody = {
    schema: "bioprism-autonomous-goal-snapshot/0.1",
    sequence: 1,
    head_digest: legacyEvent.event_digest,
    goals: [legacyRecord],
    events: [legacyEvent],
    retention: current.retention,
    secret_material: current.secret_material,
  };
  const legacy = { ...legacyBody, snapshot_digest: digestJsonSync(legacyBody) };
  const migrated = migrateLegacyAutonomousGoalSnapshot(legacy, "milliseconds");
  assert.equal(migrated.snapshot_digest, "63e476b1b6afc29bf262a901d3e86c25593b7cc6f0f05c1c747412ad91e07163");
  assert.equal(migrated.goals[0].created_ns, "123000000");
  assert.equal(migrated.events[0].created_ns, "123000000");
  assert.deepEqual(migrated.migration, {
    source_schema: "bioprism-autonomous-goal-snapshot/0.1",
    source_timestamp_unit: "milliseconds",
    source_snapshot_digest: legacy.snapshot_digest,
    source_head_digest: legacy.head_digest,
  });
  const restored = new InMemoryAutonomousGoalLedger();
  restored.restore(migrated);
  assert.deepEqual(restored.snapshot(), migrated);
  assert.equal(restored.verifyIntegrity().ok, true);

  const tampered = structuredClone(legacy);
  tampered.events[0].created_ns += 1;
  assert.throws(() => migrateLegacyAutonomousGoalSnapshot(tampered, "milliseconds"), /snapshot digest mismatch/);
  assert.throws(() => migrateLegacyAutonomousGoalSnapshot(legacy, "guessed"), /requires milliseconds or nanoseconds/);

  const legacyNanoseconds = structuredClone(legacy);
  const { state_digest: _legacyStateDigest, retention: legacyRetention, secret_material: legacySecretMaterial, ...legacyState } = legacyNanoseconds.goals[0];
  legacyState.created_ns = 123_000_000;
  legacyState.updated_ns = 123_000_000;
  legacyNanoseconds.goals[0] = { ...legacyState, state_digest: digestJsonSync(legacyState), retention: legacyRetention, secret_material: legacySecretMaterial };
  legacyNanoseconds.events[0].payload = structuredClone(legacyNanoseconds.goals[0]);
  legacyNanoseconds.events[0].created_ns = 123_000_000;
  const { event_digest: _legacyEventDigest, ...legacyEventBody } = legacyNanoseconds.events[0];
  legacyNanoseconds.events[0].event_digest = digestJsonSync(legacyEventBody);
  legacyNanoseconds.head_digest = legacyNanoseconds.events[0].event_digest;
  const { snapshot_digest: _legacySnapshotDigest, ...legacySnapshotBody } = legacyNanoseconds;
  legacyNanoseconds.snapshot_digest = digestJsonSync(legacySnapshotBody);
  const migratedNanoseconds = migrateLegacyAutonomousGoalSnapshot(legacyNanoseconds, "nanoseconds");
  assert.equal(migratedNanoseconds.goals[0].created_ns, "123000000");
  assert.equal(migratedNanoseconds.migration.source_timestamp_unit, "nanoseconds");
});

test("goal scheduler scores exact nanosecond ages and deadlines", () => {
  const created_ns = "1780000000123456789";
  const now_ns = "1780043200123456789";
  const deadline_ns = "1780086400123456789";
  const ledger = new InMemoryAutonomousGoalLedger();
  const goal = ledger.create({ goal_id: "exact-schedule", task_digest: goalTaskDigest("exact schedule"), domain: "coding", now_ns: created_ns });
  const schedule = scheduleAutonomousGoals([goal], { now_ns, aging_window_ns: "86400000000000", signals: [{ goal_id: goal.goal_id, deadline_ns }] });
  assert.equal(schedule.rows[0].age_score, 0.5);
  assert.equal(schedule.rows[0].deadline_score, 0.6667);
  assert.equal(schedule.now_ns, now_ns);
  assert.equal(schedule.schedule_digest, "4b43220e5a07d16d0fda20c9b7fb4d9e99d884598ba7a20d4bb6e43df1aab0c9");
});

test("goal digest and canonical JSON reject lone surrogates before UTF-8 encoding", () => {
  assert.throws(() => goalTaskDigest("invalid task \uD800"), /bounded contract/);
  assert.throws(() => canonicalJson({ task: "invalid task \uD800" }), /Unicode scalar/);
  assert.throws(() => canonicalJson({ ["invalid key \uD800"]: "value" }), /object keys.*Unicode scalar/);
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
  values.set("aurora-goals", "\uD800");
  await assert.rejects(() => browserPersistence.read(), /byte bound/);
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

test("authenticated goal persistence shares a key-rotatable HMAC contract with Python and requires CAS", async () => {
  let encoded = null;
  const store = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const actualDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actualDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  };
  const oldKey = Uint8Array.from({ length: 32 }, (_, index) => index);
  const newKey = Uint8Array.from({ length: 32 }, (_, index) => 255 - index);
  assert.throws(() => new AuthenticatedTransactionalJsonAutonomousGoalPersistence({
    textStore: { read: () => null, write: () => {} }, keys: new Map([["goal-v1", oldKey]]), active_key_id: "goal-v1",
  }), /lacks compare-and-swap/);

  const oldPersistence = new AuthenticatedTransactionalJsonAutonomousGoalPersistence({
    textStore: store, keys: new Map([["goal-v1", oldKey]]), active_key_id: "goal-v1",
  });
  const source = new InMemoryAutonomousGoalLedger({ clock: () => 7 });
  source.create({ goal_id: "auth-vector", task_digest: goalTaskDigest("authenticated goal snapshot"), domain: "coding", now_ns: 7 });
  const sourceCoordinator = new AutonomousGoalPersistenceCoordinator(source, oldPersistence);
  await sourceCoordinator.flush();
  const firstSnapshot = source.snapshot();
  const firstEnvelope = JSON.parse(encoded);
  const hmacBody = { schema: AUTONOMOUS_GOAL_AUTH_SCHEMA, key_id: "goal-v1", snapshot: firstSnapshot };
  assert.equal(firstEnvelope.authentication.schema, AUTONOMOUS_GOAL_AUTH_SCHEMA);
  assert.equal(firstEnvelope.authentication.key_id, "goal-v1");
  assert.equal(firstSnapshot.snapshot_digest, "56ee95b1d328789bdf36017c47e03db3737f6345ffada26979b5e4e74c197ad9");
  assert.equal(firstEnvelope.authentication.tag, "56eebea964e1be47083d9e57bd035c2566042eba1bdc998067b8be258488a047");
  assert.equal(firstEnvelope.authentication.tag, createHmac("sha256", oldKey).update(canonicalJson(hmacBody)).digest("hex"));
  assert.equal(Buffer.byteLength(encoded, "utf8") <= AUTONOMOUS_GOAL_MAX_AUTHENTICATED_SNAPSHOT_BYTES, true);
  assert.equal(encoded.includes(Buffer.from(oldKey).toString("hex")), false);

  const rotatingPersistence = new AuthenticatedTransactionalJsonAutonomousGoalPersistence({
    textStore: store, keys: new Map([["goal-v1", oldKey], ["goal-v2", newKey]]), active_key_id: "goal-v2",
  });
  const rotatingLedger = new InMemoryAutonomousGoalLedger({ clock: () => 8 });
  const rotatingCoordinator = new AutonomousGoalPersistenceCoordinator(rotatingLedger, rotatingPersistence);
  assert.equal((await rotatingCoordinator.restore()).snapshot_digest, firstSnapshot.snapshot_digest);
  await rotatingCoordinator.flush();
  assert.equal(JSON.parse(encoded).authentication.key_id, "goal-v1", "same-digest flushes must not rewrite the old key");
  rotatingLedger.transition("auth-vector", "running", { expected_revision: 0, now_ns: 8 });
  await rotatingCoordinator.flush();
  assert.equal(JSON.parse(encoded).authentication.key_id, "goal-v2");

  const staleLedger = new InMemoryAutonomousGoalLedger({ clock: () => 9 });
  const staleCoordinator = new AutonomousGoalPersistenceCoordinator(staleLedger, rotatingPersistence);
  await staleCoordinator.restore();
  rotatingLedger.transition("auth-vector", "blocked", { expected_revision: 1, now_ns: 10 });
  await rotatingCoordinator.flush();
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);

  const oldReader = new AuthenticatedTransactionalJsonAutonomousGoalPersistence({
    textStore: store, keys: new Map([["goal-v1", oldKey]]), active_key_id: "goal-v1",
  });
  await assert.rejects(() => oldReader.read(), /key id is not trusted/);
  const tampered = JSON.parse(encoded);
  tampered.authentication.tag = "0".repeat(64);
  encoded = canonicalJson(tampered);
  await assert.rejects(() => rotatingPersistence.read(), /tag does not match/);
});

test("monotonic goal anchor rejects replay of an older valid signed snapshot", async () => {
  const store = {
    value: null,
    read() { return this.value; },
    write(value) { this.value = value; },
    writeIfUnchanged(expected, value) {
      const actual = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
      if (actual !== expected) return false;
      this.value = value;
      return true;
    },
  };
  const anchor = {
    value: null,
    read() { return this.value === null ? null : structuredClone(this.value); },
    writeIfUnchanged(expected, value) {
      const actual = this.value?.anchor_digest ?? null;
      if (actual !== expected) return false;
      this.value = structuredClone(value);
      return true;
    },
  };
  const persistence = new MonotonicAnchoredAuthenticatedTransactionalJsonAutonomousGoalPersistence({
    textStore: store,
    anchor,
    keys: new Map([["goal-v1", new Uint8Array(32).fill(0x67)]]),
    active_key_id: "goal-v1",
  });
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => 123 });
  ledger.create({ goal_id: "anchored-goal", task_digest: goalTaskDigest("anchored goal"), domain: "coding", now_ns: 123 });
  const coordinator = new AutonomousGoalPersistenceCoordinator(ledger, persistence);
  assert.equal(await coordinator.restore(), null);
  const oldSnapshot = await coordinator.flush();
  const oldEnvelope = store.value;

  ledger.transition("anchored-goal", "running", { expected_revision: 0, now_ns: 124 });
  const newSnapshot = await coordinator.flush();
  assert.ok(newSnapshot.sequence > oldSnapshot.sequence);
  assert.equal(anchor.value.snapshot_digest, newSnapshot.snapshot_digest);

  store.value = oldEnvelope;
  await assert.rejects(() => persistence.read(), /rollback or incomplete commit/);
});

test("migrated goal, journal, and checkpoint snapshots survive persistence and a live restart cycle", async () => {
  const goalId = "legacy-ms";
  const task = "legacy goal";
  const sourceLedger = new InMemoryAutonomousGoalLedger();
  sourceLedger.create({ goal_id: goalId, task_digest: goalTaskDigest(task), domain: "coding", now_ns: "123000000" });
  const source = sourceLedger.snapshot();
  const { state_digest: _stateDigest, retention, secret_material, ...legacyState } = source.goals[0];
  legacyState.schema = "bioprism-autonomous-goal/0.1";
  legacyState.created_ns = 123;
  legacyState.updated_ns = 123;
  const legacyRecord = { ...legacyState, state_digest: digestJsonSync(legacyState), retention, secret_material };
  const { event_digest: _eventDigest, ...legacyEventBody } = source.events[0];
  legacyEventBody.schema = "bioprism-autonomous-goal-event/0.1";
  legacyEventBody.payload = legacyRecord;
  legacyEventBody.created_ns = 123;
  const legacyEvent = { ...legacyEventBody, event_digest: digestJsonSync(legacyEventBody) };
  const legacyGoalBody = {
    schema: "bioprism-autonomous-goal-snapshot/0.1",
    sequence: 1,
    head_digest: legacyEvent.event_digest,
    goals: [legacyRecord],
    events: [legacyEvent],
    retention: source.retention,
    secret_material: source.secret_material,
  };
  const migratedGoal = migrateLegacyAutonomousGoalSnapshot({ ...legacyGoalBody, snapshot_digest: digestJsonSync(legacyGoalBody) }, "milliseconds");

  const legacyJournalBody = {
    schema: AUTONOMOUS_GOAL_WORKER_JOURNAL_SNAPSHOT_SCHEMA_V01,
    sequence: 0,
    head_digest: "",
    events: [],
    retention: AUTONOMOUS_GOAL_WORKER_JOURNAL_RETENTION,
    secret_material: "never_returned",
  };
  const migratedJournal = migrateLegacyAutonomousGoalWorkerJournalSnapshot({ ...legacyJournalBody, snapshot_digest: digestJsonSync(legacyJournalBody) }, "milliseconds");
  const legacyCheckpointBody = {
    schema: AUTONOMOUS_GOAL_CONTROL_CHECKPOINT_SCHEMA_V01,
    run_id: "migrated-live-cycle",
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
    learned_signals: [{ goal_id: goalId, priority: 0.75, urgency: 0.25, deadline_ns: 200, estimated_cost: 1, dependencies: [] }],
    learner_state: null,
    stop_reason: "cycle_budget_exhausted",
    generation: 1,
    previous_snapshot_digest: null,
    retention: "metadata_only_goal_control_checkpoint;tasks_prompts_parameters_credentials_and_results_not_retained",
    secret_material: "never_returned",
  };
  const migratedCheckpoint = migrateLegacyAutonomousGoalControlLoopSnapshot({ ...legacyCheckpointBody, snapshot_digest: digestJsonSync(legacyCheckpointBody) }, "milliseconds");

  const textStore = (value) => ({ value, read() { return this.value; }, write(next) { this.value = next; } });
  const goalStore = textStore(canonicalJson(migratedGoal));
  const journalStore = textStore(canonicalJson(migratedJournal));
  const checkpointStore = textStore(canonicalJson(migratedCheckpoint));
  const ledger = new InMemoryAutonomousGoalLedger({ clock: () => "600000000" });
  const ledgerPersistence = new AutonomousGoalPersistenceCoordinator(ledger, new JsonAutonomousGoalPersistence(goalStore));
  await ledgerPersistence.restore();
  const journalCoordinator = new AutonomousGoalWorkerJournalPersistenceCoordinator(
    new AutonomousGoalWorkerJournal({ clock: () => "700000000" }),
    new JsonAutonomousGoalWorkerJournalPersistence(journalStore),
  );
  const controlCoordinator = new AutonomousGoalControlLoopPersistenceCoordinator(new JsonAutonomousGoalControlLoopSnapshotPersistence(checkpointStore));
  const recovery = new AutonomousGoalRecoveryCoordinator(ledger, journalCoordinator, controlCoordinator);
  const report = await recovery.restore({ now_ns: "800000000" });
  assert.equal(report.status, "restored");
  assert.equal(report.resume_snapshot.migration.source_snapshot_digest, migratedCheckpoint.migration.source_snapshot_digest);

  let dispatches = 0;
  const loop = new AutonomousGoalControlLoop({
    batch_id_prefix: "migrated-live-cycle",
    worker: new AutonomousGoalWorker({
      ledger,
      journal: journalCoordinator.journal,
      resolver: () => ({ task }),
      executor: async () => { dispatches += 1; return { status: "completed" }; },
    }),
  });
  await recovery.resume(loop, {
    run_id: "migrated-live-cycle",
    schedule_options: { now_ns: "500000000", max_selected: 1, max_concurrent: 1 },
    max_cycles: 1,
    checkpoint: (snapshot) => recovery.checkpoint(snapshot),
  });
  await ledgerPersistence.flush();
  assert.equal(ledger.snapshot().snapshot_digest, "45f6015def5e0df33906c3beae15ae1ac90b2da4b3433b845fdc66f280f204c4");
  assert.equal(journalCoordinator.journal.snapshot().snapshot_digest, "2bac65b5053f0fbe7a4caee81d7fcc59b3d913713f6f63c7532ba60bb7a44376");
  assert.equal(JSON.parse(checkpointStore.value).snapshot_digest, "348616cce9e5ec46b6600c27d393668189c2bcea16e4166f843182c07952da7f");
  assert.equal(dispatches, 1);
  assert.equal(ledger.get(goalId).status, "completed");
  assert.deepEqual(ledger.snapshot().migration, migratedGoal.migration);
  assert.deepEqual(journalCoordinator.journal.snapshot().migration, migratedJournal.migration);
  assert.deepEqual(JSON.parse(checkpointStore.value).migration, migratedCheckpoint.migration);
  assert.equal(JSON.stringify([goalStore.value, journalStore.value, checkpointStore.value]).includes(task), false);

  const restartedLedger = new InMemoryAutonomousGoalLedger({ clock: () => "900000000" });
  const restartedLedgerPersistence = new AutonomousGoalPersistenceCoordinator(restartedLedger, new JsonAutonomousGoalPersistence(goalStore));
  await restartedLedgerPersistence.restore();
  const restartedJournal = new AutonomousGoalWorkerJournalPersistenceCoordinator(
    new AutonomousGoalWorkerJournal({ clock: () => "900000000" }),
    new JsonAutonomousGoalWorkerJournalPersistence(journalStore),
  );
  const restartedControl = new AutonomousGoalControlLoopPersistenceCoordinator(new JsonAutonomousGoalControlLoopSnapshotPersistence(checkpointStore));
  const restartedRecovery = new AutonomousGoalRecoveryCoordinator(restartedLedger, restartedJournal, restartedControl);
  assert.equal((await restartedRecovery.restore({ now_ns: "900000000" })).status, "restored");
  const terminalLoop = new AutonomousGoalControlLoop({
    batch_id_prefix: "migrated-live-cycle",
    worker: new AutonomousGoalWorker({
      ledger: restartedLedger,
      journal: restartedJournal.journal,
      resolver: () => { throw new Error("completed migrated work must not be rehydrated"); },
      executor: async () => { dispatches += 1; return { status: "completed" }; },
    }),
  });
  const terminal = await restartedRecovery.resume(terminalLoop, { run_id: "migrated-live-cycle", max_cycles: 2 });
  assert.equal(terminal.stop_reason, "all_terminal");
  assert.equal(dispatches, 1);
});
