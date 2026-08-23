import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AutonomousMissionExecutor,
  AutonomousMissionReplanRemoteWorker,
  AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator,
  InMemoryAutonomousMissionReplanRemoteJobQueue,
  JsonAutonomousMissionReplanRemoteJobQueuePersistence,
  JsonAutonomousMissionReplanRemoteJobQueueTextStore,
  ToolCatalogue,
  ProviderRuntimeError,
  digestJson,
  runAutonomousMissionReplanCycle,
  validateAutonomousMissionReplanRemoteJobQueueSnapshot,
} from "../dist/index.js";

async function catalogue() {
  return ToolCatalogue.fromDefinitions([{ name: "mission_probe", description: "bounded mission test probe", inputSchema: { type: "object", additionalProperties: true } }]);
}

function mission() {
  return {
    mission_id: "remote-mission-root",
    goal: "execute a remote mission with a reviewed ordering",
    steps: [
      { id: "first", domain: "coding", capability: "verification", objective: "verify first", tool: "mission_probe", arguments: {} },
      { id: "second", domain: "coding", capability: "verification", objective: "verify second", tool: "mission_probe", arguments: {} },
    ],
    policy: { execute: true, stop_on_error: true, allow_side_effects: false, max_steps: 64, max_step_output_bytes: 100_000, max_total_output_bytes: 2_000_000, execution_mode: "serial", max_parallelism: 1, allowed_tools: ["mission_probe"] },
  };
}

async function planFor(root) {
  return {
    schema: "bioprism-typescript-autonomous-ordered-step-plan-refinement/0.1",
    status: "completed",
    task_digest: await digestJson({ task: root.goal }),
    base_plan_digest: await digestJson({ steps: root.steps.map((step) => ({ id: step.id, domain: step.domain, capability: step.capability, objective: step.objective, depends_on: [], required: true })) }),
    protected_contract_digest: null,
    priority_step_ids: ["second", "first"],
    focus_step_ids: ["second"],
    review_required: false,
    confidence: 0.9,
    selected_model: null,
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    cost_budget: null,
    retention: "step_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_arguments_or_effects_authorized",
  };
}

test("remote mission replan worker rehydrates accepted plans without provider replay and persists only digests", async () => {
  const root = mission();
  const baselineExecutor = new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async () => ({ status: "succeeded", value: { local: true } }) });
  const baseline = await runAutonomousMissionReplanCycle(baselineExecutor, root, { evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) });
  const plan = await planFor(root);
  plan.protected_contract_digest = baseline.protected_contract_digest;
  const planDigest = await digestJson(plan);
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "remote-mission-job", rootMissionId: root.mission_id, protectedContractDigest: baseline.protected_contract_digest, planningStatus: "accepted", planRefinementDigest: planDigest });
  const executed = [];
  const worker = new AutonomousMissionReplanRemoteWorker({
    queue,
    workerId: "remote-worker-1",
    resolve: async ({ job }) => ({
      executor: new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async ({ step }) => { executed.push(step.id); return { status: "succeeded", value: { private_output: "transient" } }; } }),
      mission: root,
      options: {
        acceptedPlanRefinement: plan,
        acceptPlan: true,
        evaluatePlanning: () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.9, passed: true }),
        plannerLearning: {
          settlePlanningQuality: async (candidate) => ({
            schema: "bioprism-typescript-autonomous-planning-quality-settlement/0.1",
            status: "settled",
            plan_refinement: candidate,
            evaluation: { evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.9, passed: true },
            next_state: null,
            model_quality: null,
            reason: null,
            remote: true,
            retention: "value_only;secret_material_excluded",
            secret_material: "never_returned",
          }),
        },
        plannerLearningRemote: true,
        evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
      },
    }),
  });
  const run = await worker.run();
  assert.equal(run.completed, 1);
  assert.deepEqual(executed, ["second", "first"]);
  const job = await queue.load("remote-mission-job");
  assert.equal(job.status, "completed");
  assert.equal(job.plan_refinement_digest, planDigest);
  assert.match(job.planner_learning_settlement_digest, /^[0-9a-f]{64}$/);
  assert.equal(run.rows[0].planner_learning_settlement_digest, job.planner_learning_settlement_digest);
  assert.doesNotMatch(JSON.stringify(await queue.snapshot()), /verify first|private_output|remote mission/);
  assert.equal((await validateAutonomousMissionReplanRemoteJobQueueSnapshot(await queue.snapshot())).snapshot_digest.length, 64);
});

test("remote mission worker accepts a structural queue adapter for external persistence", async () => {
  const root = mission();
  const baseline = await runAutonomousMissionReplanCycle(new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async () => ({ status: "succeeded", value: {} }) }), root, { evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) });
  const backing = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await backing.enqueue({ jobId: "adapter-job", rootMissionId: root.mission_id, protectedContractDigest: baseline.protected_contract_digest });
  const queue = {
    enqueue: backing.enqueue.bind(backing),
    load: backing.load.bind(backing),
    claimNext: backing.claimNext.bind(backing),
    renew: backing.renew.bind(backing),
    beginExecution: backing.beginExecution.bind(backing),
    complete: backing.complete.bind(backing),
    fail: backing.fail.bind(backing),
    reconcile: backing.reconcile.bind(backing),
    cancel: backing.cancel.bind(backing),
    requeue: backing.requeue.bind(backing),
    snapshot: backing.snapshot.bind(backing),
  };
  const worker = new AutonomousMissionReplanRemoteWorker({
    queue,
    workerId: "adapter-worker",
    resolve: async () => ({
      executor: new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async () => ({ status: "succeeded", value: { transient: true } }) }),
      mission: root,
      options: { evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) },
    }),
  });
  const run = await worker.run();
  assert.equal(run.completed, 1);
  assert.equal((await backing.load("adapter-job")).status, "completed");
});

test("remote mission replan queue snapshots round-trip through canonical text persistence and reject tampering", async () => {
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "persistence-job", rootMissionId: "persistence-root", protectedContractDigest: "a".repeat(64) });
  const memory = { value: null, async read() { return this.value; }, async write(value) { this.value = value; } };
  const persistence = new JsonAutonomousMissionReplanRemoteJobQueuePersistence(memory);
  const snapshot = await persistence.flush(queue);
  const restored = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  assert.equal(await persistence.restore(restored), true);
  assert.equal((await restored.load("persistence-job")).job_digest, (await queue.load("persistence-job")).job_digest);
  assert.throws(() => validateAutonomousMissionReplanRemoteJobQueueSnapshot({ ...snapshot, snapshot_digest: "0".repeat(64) }), /digest/);
  const textStore = new JsonAutonomousMissionReplanRemoteJobQueueTextStore({ value: null, async read() { return this.value; }, async write(value) { this.value = value; } });
  await textStore.write(snapshot);
  assert.equal((await textStore.read()).snapshot_digest, snapshot.snapshot_digest);
  const requeue = await restored.claimNext("review-worker");
  await restored.fail(requeue.job_id, "review-worker", "execution_error", "review_failed", false);
  const reopened = await restored.requeue(requeue.job_id, { planningStatus: "accepted", planRefinementDigest: "b".repeat(64) });
  assert.equal(reopened.status, "queued");
  assert.equal(reopened.planning_status, "accepted");
  assert.equal(reopened.planner_learning_settlement_digest, null);
});

test("remote mission worker renews long private resolution and retries typed provider failures", async () => {
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "heartbeat-job", rootMissionId: "heartbeat-root", protectedContractDigest: "c".repeat(64), maxAttempts: 2, availableAt: 0 });
  let resolverCalls = 0;
  const worker = new AutonomousMissionReplanRemoteWorker({
    queue,
    workerId: "heartbeat-worker",
    leaseMs: 40,
    resolve: async ({ renew }) => {
      resolverCalls += 1;
      await new Promise((resolve) => setTimeout(resolve, 25));
      await renew(40);
      throw new ProviderRuntimeError("typed provider simulation", { code: "transport", retryable: true });
    },
  });
  const run = await worker.run({ limit: 1, heartbeatMs: 10 });
  assert.equal(resolverCalls, 1);
  assert.equal(run.retried, 1);
  assert.equal(run.failed, 0);
  assert.equal((await queue.load("heartbeat-job")).status, "queued");
});

test("remote mission CAS coordinator serializes competing lease writers and restores the latest snapshot", async () => {
  const backing = {
    value: null,
    async read() { return this.value; },
    async write(value) { this.value = value; },
    async writeIfUnchanged(expectedDigest, value) {
      const currentDigest = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
      if (currentDigest !== expectedDigest) return false;
      this.value = value;
      return true;
    },
  };
  const persistence = new JsonAutonomousMissionReplanRemoteJobQueueTextStore(backing);
  const first = new AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousMissionReplanRemoteJobQueue(), persistence);
  const second = new AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousMissionReplanRemoteJobQueue(), persistence);
  assert.equal(await first.restore(), false);
  assert.equal(await second.restore(), false);
  await Promise.all([
    first.enqueue({ jobId: "cas-a", rootMissionId: "cas-root-a", protectedContractDigest: "a".repeat(64), availableAt: 0 }),
    second.enqueue({ jobId: "cas-b", rootMissionId: "cas-root-b", protectedContractDigest: "b".repeat(64), availableAt: 0 }),
  ]);
  const claim = await first.claimNext("cas-worker", 10_000, Date.now());
  assert.ok(claim);
  assert.equal(claim.status, "leased");
  const latest = await second.snapshot();
  assert.equal(latest.jobs.length, 2);
  const restored = new AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousMissionReplanRemoteJobQueue(), persistence);
  assert.equal(await restored.restore(), true);
  assert.equal((await restored.load(claim.job_id)).status, "leased");
  assert.equal((await restored.snapshot()).snapshot_digest, JSON.parse(backing.value).snapshot_digest);

  const nonCasBacking = { value: null, async read() { return this.value; }, async write(value) { this.value = value; } };
  const nonCas = new AutonomousMissionReplanRemoteJobQueuePersistenceCoordinator(
    new InMemoryAutonomousMissionReplanRemoteJobQueue(),
    new JsonAutonomousMissionReplanRemoteJobQueueTextStore(nonCasBacking),
  );
  await nonCas.restore();
  await nonCas.enqueue({ jobId: "non-cas", rootMissionId: "non-cas-root", protectedContractDigest: "d".repeat(64), availableAt: 0 });
  assert.equal((await nonCas.load("non-cas")).status, "queued");
});

test("remote mission worker rejects accepted-plan drift before dispatch", async () => {
  const root = mission();
  const baselineExecutor = new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async () => ({ status: "succeeded", value: {} }) });
  const baseline = await runAutonomousMissionReplanCycle(baselineExecutor, root, { evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) });
  const acceptedPlan = await planFor(root);
  acceptedPlan.protected_contract_digest = baseline.protected_contract_digest;
  const acceptedPlanDigest = await digestJson(acceptedPlan);
  const driftedPlan = { ...acceptedPlan, focus_step_ids: ["first"] };
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "drift-job", rootMissionId: root.mission_id, protectedContractDigest: baseline.protected_contract_digest, planningStatus: "accepted", planRefinementDigest: acceptedPlanDigest });
  let dispatched = 0;
  const worker = new AutonomousMissionReplanRemoteWorker({
    queue,
    workerId: "drift-worker",
    resolve: async () => ({
      executor: new AutonomousMissionExecutor({ catalogue: await catalogue(), executeStep: async () => { dispatched += 1; return { status: "succeeded", value: {} }; } }),
      mission: root,
      options: { acceptedPlanRefinement: driftedPlan, acceptPlan: true, evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) },
    }),
  });
  const run = await worker.run();
  assert.equal(run.failed, 1);
  assert.equal(dispatched, 0);
  const job = await queue.load("drift-job");
  assert.equal(job.status, "failed");
  assert.equal(job.failure_class, "contract_mismatch");
});

test("remote mission queue quarantines expired in-flight execution until explicit reconciliation", async () => {
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "in-flight-job", rootMissionId: "in-flight-root", protectedContractDigest: "e".repeat(64), availableAt: 0 });
  const claimed = await queue.claimNext("worker-a", 10, 100);
  await queue.beginExecution(claimed.job_id, "worker-a", 100);
  assert.equal(await queue.claimNext("worker-b", 10, 200), null);
  const quarantined = await queue.load("in-flight-job");
  assert.equal(quarantined.status, "reconciliation_required");
  assert.equal(quarantined.execution_phase, "running");
  assert.equal(quarantined.failure_class, "lease_expired");
  await assert.rejects(() => queue.requeue("in-flight-job", {}, 200), /requires a reconciliation receipt/);
  const unknown = await queue.reconcile("in-flight-job", { outcome: "unknown", evidenceDigest: "f".repeat(64), evidenceKind: "provider_status", operator: "operator-1" }, 201);
  assert.equal(unknown.status, "reconciliation_required");
  assert.match(unknown.reconciliation_digest, /^[0-9a-f]{64}$/);
  assert.equal(JSON.stringify(await queue.snapshot()).includes("provider_status"), true);
  await assert.rejects(() => queue.requeue("in-flight-job", { reconciliationDigest: unknown.reconciliation_digest }, 202), /does not authorize requeue/);
  const notExecuted = await queue.reconcile("in-flight-job", { outcome: "not_executed", evidenceDigest: "a".repeat(64), evidenceKind: "idempotency_probe", operator: "operator-1", effectAbsent: true }, 203);
  assert.equal(notExecuted.status, "reconciliation_required");
  assert.notEqual(notExecuted.reconciliation_digest, unknown.reconciliation_digest);
  await assert.rejects(() => queue.requeue("in-flight-job", { reconciliationDigest: "b".repeat(64) }, 204), /matching reconciliation digest/);
  const reopened = await queue.requeue("in-flight-job", { reconciliationDigest: notExecuted.reconciliation_digest }, 204);
  assert.equal(reopened.status, "queued");
  assert.equal(reopened.execution_phase, "not_started");
  assert.equal(reopened.reconciliation_digest, null);
  assert.equal(reopened.reconciliation_outcome, null);
  assert.deepEqual(reopened.reconciliation_history, [notExecuted.reconciliation_digest]);
  const restored = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await restored.restore(await queue.snapshot());
  assert.equal((await restored.load("in-flight-job")).reconciliation_digest, reopened.reconciliation_digest);
  assert.deepEqual((await restored.load("in-flight-job")).reconciliation_history, [notExecuted.reconciliation_digest]);
});

test("remote reconciliation can settle a completed external effect without replay", async () => {
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  await queue.enqueue({ jobId: "settled-job", rootMissionId: "settled-root", protectedContractDigest: "c".repeat(64), availableAt: 0 });
  const claimed = await queue.claimNext("worker-a", 10_000, 10);
  await queue.beginExecution(claimed.job_id, "worker-a", 11);
  await queue.fail(claimed.job_id, "worker-a", "transport_error", "transport", true, 12);
  const settled = await queue.reconcile(claimed.job_id, { outcome: "succeeded", evidenceDigest: "d".repeat(64), evidenceKind: "provider_receipt", operator: "operator-2", effectAbsent: false }, 13);
  assert.equal(settled.status, "completed");
  assert.equal(settled.execution_phase, "settled");
  assert.equal(settled.failure_class, null);
  assert.equal(settled.result_digest, settled.reconciliation_digest);
  const repeated = await queue.reconcile(claimed.job_id, { outcome: "succeeded", evidenceDigest: "d".repeat(64), evidenceKind: "provider_receipt", operator: "operator-2", effectAbsent: false }, 14);
  assert.equal(repeated.job_digest, settled.job_digest);
  await assert.rejects(() => queue.requeue(claimed.job_id, {}, 14), /not requeueable/);
});

test("remote mission queue refuses cancellation and completion across an unrecorded execution boundary", async () => {
  const queue = new InMemoryAutonomousMissionReplanRemoteJobQueue();
  const contractDigest = "1".repeat(64);
  await queue.enqueue({ jobId: "boundary-job", rootMissionId: "boundary-root", protectedContractDigest: contractDigest, availableAt: 0 });
  const claimed = await queue.claimNext("worker-a", 10_000, 10);
  const result = { status: "completed", root_mission_id: "boundary-root", protected_contract_digest: contractDigest, planning_status: "disabled", replan_count: 0 };
  await assert.rejects(() => queue.complete(claimed.job_id, "worker-a", result, 11), /execution phase to be running/);
  await assert.rejects(() => queue.cancel(claimed.job_id, 11), /active or uncertain execution/);
  await queue.beginExecution(claimed.job_id, "worker-a", 12);
  await assert.rejects(() => queue.cancel(claimed.job_id, 13), /active or uncertain execution/);
  await queue.fail(claimed.job_id, "worker-a", "transport_error", "transport", true, 14);
  await assert.rejects(() => queue.cancel(claimed.job_id, 15), /active or uncertain execution/);
  const reconciled = await queue.reconcile(claimed.job_id, { outcome: "not_executed", evidenceDigest: "2".repeat(64), effectAbsent: true }, 16);
  const reopened = await queue.requeue(claimed.job_id, { reconciliationDigest: reconciled.reconciliation_digest }, 17);
  const cancelled = await queue.cancel(reopened.job_id, 18);
  assert.equal(cancelled.status, "cancelled");
});
