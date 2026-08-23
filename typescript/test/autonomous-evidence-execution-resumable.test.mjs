import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceExecutionResumableController,
  AutonomousEvidenceReadinessPolicy,
  CredentialStore,
  digestJsonSync,
  InMemoryAutonomousEvidenceExecutionCheckpointStore,
  InMemoryAutonomousEvidenceRuntimeJournal,
  JsonAutonomousEvidenceExecutionCheckpointStore,
  LLMRuntime,
  TransactionalJsonAutonomousEvidenceExecutionCheckpointStore,
  WebStorageAutonomousEvidenceExecutionCheckpointTextStore,
  validateAutonomousEvidenceExecutionCheckpoint,
} from "../dist/index.js";

function planAgent() {
  return new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
}

function registerAllDomains(registry, calls) {
  registry.register({
    adapterId: "resumable.all-domains",
    version: "1.0.0",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: async () => {
      calls.count += 1;
      const value = { transient_source_value: calls.count };
      calls.values.set(digestJsonSync(value), value);
      return value;
    },
  });
}

function executionOptions() {
  return {
    sleep: async () => {},
    projector: {
      project: (_value, context) => [{
        label: context.requirement.label,
        kind: "fact",
        status: "observed",
        value_digest: null,
        source_digest: context.request.source_digest ?? null,
      }],
    },
    evaluator: {
      evaluator_id: "resumable-evaluator",
      evaluator_version: "1.0.0",
      evaluate: () => ({
        evaluator_id: "resumable-evaluator",
        evaluator_version: "1.0.0",
        verdict: "accepted",
        score: 1,
        evidence_digest: "d".repeat(64),
      }),
    },
  };
}

test("restart-safe evidence execution fences approval, readiness, settlement, and replay across all domains", async () => {
  const calls = { count: 0, values: new Map() };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(registry, calls);
  const agent = planAgent();
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `resumable-source-${index}`,
    source_digest: "c".repeat(64),
    request_id: `resumable-request-${index}`,
    metadata: { operation: "observe" },
  }));
  const controller = new AutonomousEvidenceExecutionController(registry);
  const executionPlan = await controller.prepare(evidencePlan, {
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
  });
  assert.equal(executionPlan.status, "ready_for_review");

  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const firstWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains");
  const gated = await firstWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.checkpoint.completed_request_count, 0);
  assert.equal(calls.count, 0);

  const restartedWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains");
  assert.equal((await restartedWorker.restore()).status, "restored");
  const completed = await restartedWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true });
  assert.equal(completed.status, "completed");
  assert.equal(completed.checkpoint.completed_request_count, requests.length);
  assert.equal(completed.checkpoint.accepted_request_count, requests.length);
  assert.equal(completed.checkpoint.runtime_status, "completed");
  assert.equal(completed.replayed, false);
  assert.equal(calls.count, requests.length);
  assert.doesNotMatch(JSON.stringify(completed.toJSON()), /transient_source_value|operation/);

  const replayWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains");
  const replayed = await replayWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, rehydrateValue: (receipt) => calls.values.get(receipt.value_digest) ?? null, approveSourceDispatch: true });
  assert.equal(replayed.status, "completed");
  assert.equal(replayed.replayed, true);
  assert.equal(replayed.result?.runtime.json.receipts.every((receipt) => receipt.replay === "replayed"), true);
  assert.equal(calls.count, requests.length);
  assert.equal(replayed.toJSON().checkpoint_digest, replayed.checkpoint.checkpoint_digest);
});

test("evidence execution checkpoint persistence is JSON-portable, browser-portable, tamper-evident, and CAS-fenced", async () => {
  let encoded = null;
  const storage = {
    getItem: () => encoded,
    setItem: (_key, value) => { encoded = value; },
  };
  const textStore = new WebStorageAutonomousEvidenceExecutionCheckpointTextStore(storage, "evidence-checkpoint");
  const jsonStore = new JsonAutonomousEvidenceExecutionCheckpointStore(textStore);
  const transactionalTextStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).checkpoint_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const transactionalStore = new TransactionalJsonAutonomousEvidenceExecutionCheckpointStore(transactionalTextStore);
  const checkpoint = {
    schema: "bioprism-typescript-autonomous-evidence-execution-checkpoint/0.1",
    job_id: "checkpoint-job",
    evidence_plan_digest: "a".repeat(64),
    execution_plan_digest: "b".repeat(64),
    request_digest: "c".repeat(64),
    readiness_report_digest: "d".repeat(64),
    status: "approval_required",
    runtime_status: null,
    runtime_result_digest: null,
    completed_request_count: 0,
    pending_request_count: 0,
    accepted_request_count: 0,
    checkpoint_digest: "",
    retention: "metadata_only;requests_readiness_and_source_values_caller_owned",
    secret_material: "never_returned",
  };
  const { checkpoint_digest: _checkpointDigest, retention: _retention, secret_material: _secretMaterial, ...payload } = checkpoint;
  checkpoint.checkpoint_digest = digestJsonSync(payload);
  await jsonStore.write(checkpoint);
  const restored = await jsonStore.read();
  assert.equal(restored?.checkpoint_digest, checkpoint.checkpoint_digest);
  assert.equal((await transactionalStore.writeIfUnchanged(null, checkpoint)), false);
  assert.equal((await transactionalStore.writeIfUnchanged(checkpoint.checkpoint_digest, checkpoint)), true);
  assert.equal(validateAutonomousEvidenceExecutionCheckpoint(restored).job_id, "checkpoint-job");
  assert.throws(() => validateAutonomousEvidenceExecutionCheckpoint({ ...restored, checkpoint_digest: "e".repeat(64) }), /digest is invalid/);

  const stale = { ...checkpoint };
  assert.equal(await transactionalStore.writeIfUnchanged("0".repeat(64), stale), false);
});

test("AutonomousAgent exposes the restart-safe evidence lifecycle at the high-level facade", async () => {
  const calls = { count: 0, values: new Map() };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(registry, calls);
  const agent = planAgent();
  const plan = await agent.evidencePlan(["coding"]);
  const requests = plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `facade-source-${index}`,
    source_digest: "e".repeat(64),
  }));
  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const first = await agent.executeReviewedEvidenceResumable(registry, ["coding"], requests, {
    jobId: "facade-resumable-job",
    checkpointStore,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    execute: executionOptions(),
  });
  assert.equal(first.status, "approval_required");
  assert.equal(calls.count, 0);

  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const second = await agent.executeReviewedEvidenceResumable(registry, ["coding"], requests, {
    jobId: "facade-resumable-job",
    checkpointStore,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    execute: { ...executionOptions(), journal, approveSourceDispatch: true },
  });
  assert.equal(second.status, "completed");
  assert.equal(second.checkpoint.completed_request_count, requests.length);
  assert.equal(calls.count, requests.length);
});
