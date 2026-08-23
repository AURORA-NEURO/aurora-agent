import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AutonomousMissionExecutor,
  AutonomousMissionReplanRemoteWorker,
  InMemoryAutonomousMissionReplanRemoteJobQueue,
  JsonAutonomousMissionReplanRemoteJobQueuePersistence,
  JsonAutonomousMissionReplanRemoteJobQueueTextStore,
  ToolCatalogue,
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
      options: { acceptedPlanRefinement: plan, acceptPlan: true, evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }) },
    }),
  });
  const run = await worker.run();
  assert.equal(run.completed, 1);
  assert.deepEqual(executed, ["second", "first"]);
  const job = await queue.load("remote-mission-job");
  assert.equal(job.status, "completed");
  assert.equal(job.plan_refinement_digest, planDigest);
  assert.doesNotMatch(JSON.stringify(await queue.snapshot()), /verify first|private_output|remote mission/);
  assert.equal((await validateAutonomousMissionReplanRemoteJobQueueSnapshot(await queue.snapshot())).snapshot_digest.length, 64);
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
});
