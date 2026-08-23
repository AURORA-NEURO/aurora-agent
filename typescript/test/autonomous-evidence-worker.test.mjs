import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  AutonomousEvidenceRuntime,
  AutonomousEvidenceWorker,
  InMemoryAutonomousEvidenceRuntimeJournal,
  InMemoryAutonomousEvidenceWorkQueue,
  AutonomousEvidenceWorkQueuePersistenceCoordinator,
  buildAutonomousEvidencePlan,
  builtinAutonomousDomainProfiles,
  digestJsonSync,
  TransactionalJsonAutonomousEvidenceWorkQueueSnapshotPersistence,
} from "../dist/index.js";

function requestFor(requirement, index = 0) {
  return {
    requirement_id: requirement.requirement_id,
    source_id: `worker-source-${index}`,
    request_id: `worker-request-${index}`,
    metadata: { fixture: true, domain: requirement.domain },
  };
}

function adapters(calls, evaluator = true) {
  return {
    acquirer: {
      acquire(context) {
        calls.push(context.requirement.requirement_id);
        return { fixture: "transient-evidence", requirement: context.requirement.requirement_id };
      },
    },
    projector: {
      project(_value, context) {
        return [{ label: context.requirement.label, status: "observed", kind: "fact", confidence: 1 }];
      },
    },
    ...(evaluator ? {
      evaluator: {
        evaluator_id: "worker-evaluator",
        evaluator_version: "1",
        evaluate: ({ requirement }) => ({ evaluator_id: "worker-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
      },
    } : {}),
  };
}

async function singleDomainPlan(domain) {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === domain);
  const baseline = await buildAutonomousEvidencePlan([profile.workflow]);
  return buildAutonomousEvidencePlan([profile.workflow], { availableEvidence: baseline.requirements.slice(1).map((item) => item.requirement_id) });
}

test("evidence worker executes one accepted request for every autonomous domain", async () => {
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const contexts = new Map();
  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const domain = AUTONOMOUS_DOMAIN_NAMES[index];
    const plan = await singleDomainPlan(domain);
    const request = requestFor(plan.requirements[0], index);
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    const calls = [];
    const work = queue.enqueue({ workId: `evidence-work-${domain}`, plan, request, now: 1_000 });
    contexts.set(work.work_id, { plan, request, runtime: new AutonomousEvidenceRuntime({ plan, journal }), execute: adapters(calls) });
  }
  const worker = new AutonomousEvidenceWorker(queue, (item) => contexts.get(item.work_id));
  const run = await worker.run({ workerId: "worker-a", limit: AUTONOMOUS_DOMAIN_NAMES.length, now: 1_000 });
  assert.equal(run.completed, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(run.failed, 0);
  assert.equal(run.reconciled, 0);
  assert.equal(queue.rows().every((item) => item.status === "completed"), true);
  assert.equal(run.rows.every((row) => row.value_retained === false), true);
  assert.equal(run.rows.every((row) => row.result_digest && row.receipt_digest), true);
  assert.equal(run.rows.every((row) => row.acceptance_digest && row.acceptance_digest.length === 64), true);
  assert.equal(queue.rows().every((item) => item.acceptance_digest && item.acceptance_digest.length === 64), true);
});

test("worker handoff is explicit for evaluator-pending work and resumes after restart without reacquisition", async () => {
  const plan = await singleDomainPlan("science");
  const request = requestFor(plan.requirements[0]);
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const item = queue.enqueue({ workId: "pending-evidence", plan, request, now: 2_000 });
  const calls = [];
  const first = new AutonomousEvidenceWorker(queue, () => ({ plan, request, runtime: new AutonomousEvidenceRuntime({ plan, journal }), execute: adapters(calls, false) }));
  const pending = await first.run({ workerId: "worker-a", now: 2_000 });
  assert.equal(pending.awaiting_evaluation, 1);
  assert.equal(queue.get(item.work_id).status, "awaiting_evaluation");
  assert.equal(calls.length, 1);

  const snapshot = queue.snapshot();
  const restartedQueue = new InMemoryAutonomousEvidenceWorkQueue();
  const persistence = new AutonomousEvidenceWorkQueuePersistenceCoordinator(restartedQueue, { read: () => snapshot, write: () => {} });
  await persistence.restore();
  restartedQueue.requeue(item.work_id, 3_000);
  const restartedRuntime = new AutonomousEvidenceRuntime({ plan, journal });
  await restartedRuntime.rehydrate();
  const restarted = new AutonomousEvidenceWorker(restartedQueue, () => ({
    plan,
    request,
    runtime: restartedRuntime,
    execute: {
      ...adapters(calls),
      rehydrateValue: () => ({ fixture: "transient-evidence", requirement: plan.requirements[0].requirement_id }),
      reevaluatePending: true,
    },
  }));
  const accepted = await restarted.run({ workerId: "worker-b", now: 3_000 });
  assert.equal(accepted.completed, 1);
  assert.equal(restartedQueue.get(item.work_id).status, "completed");
  assert.equal(calls.length, 1, "restart reconciliation must not reacquire the evidence value");
});

test("worker leases are fenced and queue snapshots refuse tampering", async () => {
  const plan = await singleDomainPlan("coding");
  const request = requestFor(plan.requirements[0]);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const item = queue.enqueue({ workId: "fenced-evidence", plan, request, now: 4_000 });
  assert.ok(queue.claim(item.work_id, "worker-a", 100, 4_000));
  assert.equal(queue.claim(item.work_id, "worker-b", 100, 4_050), null);
  assert.throws(() => queue.renew(item.work_id, "worker-b", 100, 4_060), /cannot be renewed/);
  const snapshot = queue.snapshot();
  const tampered = { ...snapshot, items: snapshot.items.map((row) => ({ ...row, source_id: "tampered" })) };
  assert.throws(() => new InMemoryAutonomousEvidenceWorkQueue().restore(tampered), /snapshot digest is invalid/);
  assert.throws(() => queue.requeue(item.work_id, 4_070), /not waiting/);
});

test("evidence work queue JSON persistence fences stale workers", async () => {
  const plan = await singleDomainPlan("evaluation");
  const request = requestFor(plan.requirements[0]);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  queue.enqueue({ workId: "persisted-evidence", plan, request, now: 4_500 });
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observed !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousEvidenceWorkQueueSnapshotPersistence(textStore);
  const coordinator = new AutonomousEvidenceWorkQueuePersistenceCoordinator(queue, persistence);
  const first = await coordinator.flush();
  assert.equal(JSON.parse(encoded).snapshot_digest, first.snapshot_digest);
  const stale = new AutonomousEvidenceWorkQueuePersistenceCoordinator(new InMemoryAutonomousEvidenceWorkQueue(), persistence);
  await assert.rejects(() => stale.flush(), /compare-and-swap/);
  const restored = new AutonomousEvidenceWorkQueuePersistenceCoordinator(new InMemoryAutonomousEvidenceWorkQueue(), persistence);
  const receipt = await restored.restore();
  assert.equal(receipt.items, 1);
  const canonical = encoded;
  encoded = JSON.stringify(JSON.parse(canonical), null, 2);
  await assert.rejects(() => persistence.read(), /canonical/);
  encoded = canonical;
  encoded = "{invalid";
  await assert.rejects(() => persistence.read(), /invalid/);
});

test("worker identity rehydration failures quarantine work instead of reacquiring", async () => {
  const plan = await singleDomainPlan("biomedical");
  const request = requestFor(plan.requirements[0]);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const item = queue.enqueue({ workId: "identity-evidence", plan, request, now: 5_000 });
  const worker = new AutonomousEvidenceWorker(queue, () => ({
    plan,
    request: { ...request, source_id: "different-source" },
    runtime: new AutonomousEvidenceRuntime({ plan }),
    execute: adapters([]),
  }));
  const result = await worker.run({ workerId: "worker-a", now: 5_000 });
  assert.equal(result.reconciled, 1);
  assert.equal(result.rows[0].error_class, "identity_conflict");
  assert.equal(queue.get(item.work_id).status, "reconciliation_required");
});

test("evidence reconciliation receipts are idempotent and fresh across safe requeue", async () => {
  const plan = await singleDomainPlan("coding");
  const request = requestFor(plan.requirements[0], 99);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const successful = queue.enqueue({ workId: "evidence-reconciliation-success", plan, request: requestFor(plan.requirements[0], 98), now: 5_400 });
  queue.claim(successful.work_id, "worker-a", 100, 5_400);
  queue.beginExecution(successful.work_id, "worker-a", 5_401);
  queue.reconcile(successful.work_id, "worker-a", "result_reconciliation_required", 5_499);
  const successReceipt = queue.settleReconciliation(successful.work_id, { outcome: "succeeded", evidenceDigest: "a".repeat(64) }, 5_500);
  assert.equal(successReceipt.status, "completed");
  assert.equal(successReceipt.result_digest, successReceipt.reconciliation_digest);
  const item = queue.enqueue({ workId: "evidence-reconciliation-history", plan, request, now: 5_500 });
  queue.claim(item.work_id, "worker-a", 100, 5_500);
  queue.beginExecution(item.work_id, "worker-a", 5_501);
  queue.reconcile(item.work_id, "worker-a", "result_reconciliation_required", 5_599);
  const observed = queue.settleReconciliation(item.work_id, { outcome: "not_executed", evidenceDigest: "c".repeat(64) }, 5_600);
  assert.equal(observed.reconciliation_effect_absent, true);
  assert.throws(() => queue.requeue(item.work_id, { reconciliationDigest: "d".repeat(64) }, 5_601), /matching reconciliation digest/);
  const queued = queue.requeue(item.work_id, { reconciliationDigest: observed.reconciliation_digest }, 5_602);
  assert.equal(queued.reconciliation_digest, null);
  assert.deepEqual(queued.reconciliation_history, [observed.reconciliation_digest]);
  queue.claim(item.work_id, "worker-a", 100, 5_603);
  queue.beginExecution(item.work_id, "worker-a", 5_604);
  queue.reconcile(item.work_id, "worker-a", "result_reconciliation_required", 5_699);
  const second = queue.settleReconciliation(item.work_id, { outcome: "not_executed", evidenceDigest: "e".repeat(64) }, 5_700);
  assert.notEqual(second.reconciliation_digest, observed.reconciliation_digest);
  assert.deepEqual(second.reconciliation_history, [observed.reconciliation_digest]);
  assert.deepEqual(queue.settleReconciliation(item.work_id, { outcome: "not_executed", evidenceDigest: "e".repeat(64) }, 5_701), second);
  const restored = new InMemoryAutonomousEvidenceWorkQueue();
  restored.restore(queue.snapshot());
  assert.equal(restored.get(item.work_id).reconciliation_digest, second.reconciliation_digest);
  assert.deepEqual(restored.get(item.work_id).reconciliation_history, [observed.reconciliation_digest]);
});

test("evidence expiry reclaims only pre-dispatch work and quarantines in-flight work", async () => {
  const plan = await singleDomainPlan("coding");
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const preDispatch = queue.enqueue({ workId: "evidence-pre-dispatch-expiry", plan, request: requestFor(plan.requirements[0], 100), now: 5_800 });
  queue.claim(preDispatch.work_id, "worker-a", 100, 5_800);
  const reclaimed = queue.reclaimExpired(128, 5_900);
  assert.equal(reclaimed.find((item) => item.work_id === preDispatch.work_id).status, "queued");
  const inFlight = queue.enqueue({ workId: "evidence-in-flight-expiry", plan, request: requestFor(plan.requirements[0], 101), now: 5_800 });
  queue.claim(inFlight.work_id, "worker-a", 100, 5_800);
  queue.beginExecution(inFlight.work_id, "worker-a", 5_801);
  const quarantined = queue.reclaimExpired(128, 5_901).find((item) => item.work_id === inFlight.work_id);
  assert.equal(quarantined.status, "reconciliation_required");
  assert.equal(quarantined.execution_phase, "running");
  assert.throws(() => queue.requeue(inFlight.work_id, 5_902), /no-effect reconciliation/);
});

test("work queue rejects credential-shaped metadata before persistence", async () => {
  const plan = await singleDomainPlan("operations");
  const requirement = plan.requirements[0];
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  assert.throws(
    () => queue.enqueue({
      workId: "secret-metadata-evidence",
      plan,
      request: {
        ...requestFor(requirement),
        metadata: { api_key: "caller-secret-must-never-enter-the-queue" },
      },
      now: 6_000,
    }),
    /credential-shaped metadata/,
  );
});

test("completion requires the exact queued requirement to have a digest-valid accepted assessment", async () => {
  const plan = await singleDomainPlan("science");
  const request = requestFor(plan.requirements[0]);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const item = queue.enqueue({ workId: "unaccepted-evidence", plan, request, now: 7_000 });
  const runtime = new AutonomousEvidenceRuntime({ plan });
  const result = await runtime.execute([request], { acquirer: adapters([]).acquirer, projector: adapters([]).projector });
  assert.equal(result.json.status, "awaiting_evaluation");
  assert.ok(queue.claim(item.work_id, "worker-a", 30_000, 7_000));
  queue.beginExecution(item.work_id, "worker-a", 7_000);
  assert.throws(() => queue.complete(item.work_id, "worker-a", result, 7_001), /accepted queued requirement/);
  assert.equal(queue.get(item.work_id).status, "leased");
});

test("legacy queue snapshots migrate conservatively and completed legacy work is quarantined", async () => {
  const plan = await singleDomainPlan("coding");
  const request = requestFor(plan.requirements[0]);
  const queue = new InMemoryAutonomousEvidenceWorkQueue();
  const item = queue.enqueue({ workId: "legacy-evidence", plan, request, now: 8_000 });
  const current = queue.snapshot();
  const legacyItems = current.items.map((row) => {
    const { acceptance_digest: _acceptanceDigest, ...withoutAcceptance } = row;
    const legacy = { ...withoutAcceptance, schema: "bioprism-typescript-autonomous-evidence-work-item/0.1", item_digest: "" };
    const { item_digest: _itemDigest, ...payload } = legacy;
    return { ...legacy, item_digest: digestJsonSync(payload) };
  });
  const legacyDescriptor = {
    schema: "bioprism-typescript-autonomous-evidence-work-queue/0.1",
    items: legacyItems,
    retention: "metadata_only_request_and_values_caller_owned",
    secret_material: "never_returned",
  };
  const legacySnapshot = { ...legacyDescriptor, snapshot_digest: digestJsonSync(legacyDescriptor) };
  const restored = new InMemoryAutonomousEvidenceWorkQueue();
  restored.restore(legacySnapshot);
  assert.equal(restored.get(item.work_id).schema, "bioprism-typescript-autonomous-evidence-work-item/0.3");
  assert.equal(restored.get(item.work_id).status, "queued");

  const completedLegacy = { ...legacyItems[0], status: "completed" };
  const { item_digest: _completedDigest, ...completedPayload } = completedLegacy;
  completedLegacy.item_digest = digestJsonSync(completedPayload);
  const completedDescriptor = { ...legacyDescriptor, items: [completedLegacy] };
  const completedSnapshot = { ...completedDescriptor, snapshot_digest: digestJsonSync(completedDescriptor) };
  const quarantined = new InMemoryAutonomousEvidenceWorkQueue();
  quarantined.restore(completedSnapshot);
  assert.equal(quarantined.get(item.work_id).status, "reconciliation_required");
});
