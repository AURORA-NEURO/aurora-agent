import test from "node:test";
import assert from "node:assert/strict";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator,
  AutonomousWorkflowPortfolioRemoteWorker,
  InMemoryAutonomousWorkflowPortfolioRemoteJobQueue,
  JsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence,
  LLMRuntime,
  TransactionalJsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence,
  WebStorageAutonomousWorkflowPortfolioRemoteJobQueueTextStore,
  admitAutonomousWorkflowPortfolioRemoteJob,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function requests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `remote-${domain}`,
    task: `private remote task payload for ${domain}`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`remote-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
    hints: [`private remote hint for ${domain}`],
  }));
}

function agentFor(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", async (request) => {
    await onRequest(request);
    return { output_text: `offline result for ${request.model}` };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return agent;
}

test("remote portfolio worker executes every domain from private resolver state and persists only digests", async () => {
  const requestsForJob = requests();
  const providerCalls = [];
  const traceEvents = [];
  const agent = agentFor((request) => providerCalls.push(request));
  const plan = await agent.planWorkflowPortfolio(requestsForJob, { requireAllDomains: true });
  const admission = await agent.admitWorkflowPortfolio(requestsForJob, { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(queue, {
    jobId: "remote-portfolio-job",
    plan,
    admission,
    traceId: "remote-portfolio-trace",
    now: 1_000,
  });
  assert.equal(job.status, "queued");
  assert.equal(job.admission_digest, admission.admission_digest);
  assert.doesNotMatch(JSON.stringify(job), /private remote task|private remote hint/);

  const worker = new AutonomousWorkflowPortfolioRemoteWorker(agent, queue, () => ({
    requests: requestsForJob,
    plan,
    admission,
    executionOptions: {
      approveProviderCall: true,
      traceId: "remote-portfolio-trace",
      traceSink: (event) => { traceEvents.push(event); },
    },
  }), "remote-portfolio-worker");
  const run = await worker.run({ now: 1_001 });

  assert.equal(run.completed, 1);
  assert.equal(run.failed, 0);
  assert.equal(providerCalls.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(traceEvents.at(-1).phase, "completed");
  assert.equal(queue.get(job.job_id).status, "completed");
  assert.equal(queue.get(job.job_id).checkpoint_digest.length, 64);
  assert.equal(queue.get(job.job_id).result_digest.length, 64);
  assert.equal(queue.get(job.job_id).trace_digest, traceEvents.at(-1).event_digest);
  assert.doesNotMatch(JSON.stringify(run), /private remote task|private remote hint|offline result/);
});

test("remote portfolio worker preserves approval-required pauses and resumes only after explicit requeue", async () => {
  let providerCalls = 0;
  const requestsForJob = requests();
  const agent = agentFor(() => { providerCalls += 1; });
  const plan = await agent.planWorkflowPortfolio(requestsForJob, { requireAllDomains: true });
  const admission = await agent.admitWorkflowPortfolio(requestsForJob, { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "approval-pause-job", plan, admission, now: 1_500 });
  let approved = false;
  const worker = new AutonomousWorkflowPortfolioRemoteWorker(agent, queue, () => ({
    requests: requestsForJob,
    plan,
    admission,
    executionOptions: { approveProviderCall: approved },
  }), "approval-pause-worker");

  const paused = await worker.run({ now: 1_501 });
  assert.equal(paused.approval_required, 1);
  assert.equal(paused.completed, 0);
  assert.equal(providerCalls, 0);
  assert.equal(queue.get(job.job_id).status, "approval_required");
  assert.equal(queue.get(job.job_id).failure_class, "approval_required");
  assert.ok(queue.get(job.job_id).result_digest);

  approved = true;
  const requeued = queue.requeue(job.job_id, 1_502);
  assert.equal(requeued.status, "queued");
  const resumed = await worker.run({ now: 1_503 });
  assert.equal(resumed.completed, 1);
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(queue.get(job.job_id).status, "completed");
});

test("remote portfolio worker renews long-running leases and rejects an unsafe heartbeat", async () => {
  let providerCalls = 0;
  const agent = agentFor(async () => {
    providerCalls += 1;
    await new Promise((resolve) => setTimeout(resolve, 35));
  });
  const privateRequests = [{ id: "heartbeat-coding", task: "private heartbeat task", domain: "coding" }];
  const plan = await agent.planWorkflowPortfolio(privateRequests);
  const admission = await agent.admitWorkflowPortfolio(privateRequests, { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "heartbeat-job", plan, admission });
  const worker = new AutonomousWorkflowPortfolioRemoteWorker(agent, queue, () => ({
    requests: privateRequests,
    plan,
    admission,
    executionOptions: { approveProviderCall: true },
  }), "heartbeat-worker");

  await assert.rejects(() => worker.run({ leaseMs: 100, heartbeatMs: 100 }), /heartbeatMs must be less than leaseMs/);
  const run = await worker.run({ leaseMs: 100, heartbeatMs: 20 });
  assert.equal(run.completed, 1);
  assert.equal(providerCalls, 1);
  assert.equal(queue.get(job.job_id).status, "completed");
});

test("remote portfolio worker refuses plan/admission drift before provider dispatch and fences leases", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const original = requests();
  const plan = await agent.planWorkflowPortfolio(original, { requireAllDomains: true });
  const admission = await agent.admitWorkflowPortfolio(original, { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "drift-job", plan, admission, now: 2_000 });
  const claimed = queue.claim(job.job_id, "foreign-worker", 100, 2_000);
  assert.ok(claimed);
  assert.equal(queue.claim(job.job_id, "other-worker", 100, 2_050), null);
  assert.throws(() => queue.renew(job.job_id, "other-worker", 100, 2_060), /cannot be renewed/);
  queue.reclaimExpired(2_101);
  assert.equal(queue.get(job.job_id).status, "queued");
  queue.cancel(job.job_id, 2_102);

  const retryableJob = await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "drift-job-2", plan, admission, now: 2_000 });
  const worker = new AutonomousWorkflowPortfolioRemoteWorker(agent, queue, () => ({
    requests: original.map((request, index) => index === 1 ? { ...request, task: "tampered private task" } : request),
    plan,
    admission,
    executionOptions: { approveProviderCall: true },
  }), "drift-worker");
  const result = await worker.run({ now: 2_001 });
  assert.equal(result.failed, 1);
  assert.equal(queue.get(retryableJob.job_id).status, "failed");
  assert.equal(providerCalls, 0);
  assert.doesNotMatch(JSON.stringify(queue.snapshot()), /tampered private task|private remote task/);
});

test("remote portfolio job snapshots are bounded, transactional, browser-portable, and CAS-fenced", async () => {
  const agent = agentFor();
  const plan = await agent.planWorkflowPortfolio([{ id: "remote-coding", task: "private coding task", domain: "coding" }]);
  const admission = await agent.admitWorkflowPortfolio([{ id: "remote-coding", task: "private coding task", domain: "coding" }], { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "persisted-job", plan, admission, now: 3_000 });
  const snapshot = queue.snapshot();
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence(textStore);
  const coordinator = new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(queue, persistence);
  await coordinator.flush();
  const restoredQueue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const restored = new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(restoredQueue, persistence);
  assert.equal((await restored.restore()).snapshot_digest, snapshot.snapshot_digest);
  assert.deepEqual(restoredQueue.snapshot(), snapshot);

  const stale = new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue(), persistence);
  await stale.restore();
  const competing = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const competingPlan = await agent.planWorkflowPortfolio([{ id: "other", task: "other private task", domain: "data" }]);
  const competingAdmission = await agent.admitWorkflowPortfolio([{ id: "other", task: "other private task", domain: "data" }], { plan: competingPlan });
  await admitAutonomousWorkflowPortfolioRemoteJob(competing, { jobId: "other-job", plan: competingPlan, admission: competingAdmission, now: 3_100 });
  await new JsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence(textStore).write(competing.snapshot());
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const values = new Map();
  const browserStore = new WebStorageAutonomousWorkflowPortfolioRemoteJobQueueTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  }, "remote-job-key");
  const browserPersistence = new JsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence(browserStore);
  await browserPersistence.write(snapshot);
  assert.equal((await browserPersistence.read()).snapshot_digest, snapshot.snapshot_digest);
});

test("CAS remote portfolio coordinators prevent duplicate claims after two workers restore", async () => {
  const agent = agentFor();
  const plan = await agent.planWorkflowPortfolio([{ id: "cas-coding", task: "private CAS task", domain: "coding" }]);
  const admission = await agent.admitWorkflowPortfolio([{ id: "cas-coding", task: "private CAS task", domain: "coding" }], { plan });
  const seed = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(seed, { jobId: "cas-job", plan, admission, now: 4_000 });
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousWorkflowPortfolioRemoteJobQueuePersistence(textStore);
  await new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(seed, persistence).flush();
  const coordinatorA = new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue(), persistence);
  const coordinatorB = new AutonomousWorkflowPortfolioRemoteJobQueuePersistenceCoordinator(new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue(), persistence);
  await Promise.all([coordinatorA.restore(), coordinatorB.restore()]);
  const claims = await Promise.all([
    coordinatorA.claim(job.job_id, "cas-worker-a", 30_000, 4_001),
    coordinatorB.claim(job.job_id, "cas-worker-b", 30_000, 4_001),
  ]);
  assert.equal(claims.filter(Boolean).length, 1);
  const persisted = await persistence.read();
  assert.equal(persisted.jobs[0].status, "leased");
  assert.ok(["cas-worker-a", "cas-worker-b"].includes(persisted.jobs[0].lease_owner));
});

test("remote portfolio execution quarantines in-flight expiry and requires evidence-bound requeue", async () => {
  const agent = agentFor();
  const privateRequests = [{ id: "reconcile-coding", task: "private reconciliation task", domain: "coding" }];
  const plan = await agent.planWorkflowPortfolio(privateRequests);
  const admission = await agent.admitWorkflowPortfolio(privateRequests, { plan });
  const queue = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  const job = await admitAutonomousWorkflowPortfolioRemoteJob(queue, { jobId: "reconcile-portfolio-job", plan, admission, now: 0 });
  const claimed = queue.claim(job.job_id, "reconcile-worker", 10, 1);
  queue.beginExecution(claimed.job_id, "reconcile-worker", 2);
  const expired = queue.reclaimExpired(12);
  assert.equal(expired[0].status, "reconciliation_required");
  assert.equal(expired[0].execution_phase, "running");
  assert.throws(() => queue.requeue(job.job_id, 13), /matching no-effect reconciliation receipt/);
  assert.throws(() => queue.cancel(job.job_id, 13), /active or uncertain execution boundary/);
  const unknown = queue.settleReconciliation(job.job_id, { outcome: "unknown", evidenceDigest: "a".repeat(64), evidenceKind: "provider_status", operator: "operator-1" }, 14);
  assert.equal(unknown.status, "reconciliation_required");
  assert.throws(() => queue.requeue(job.job_id, 15, { reconciliationDigest: unknown.reconciliation_digest }), /matching no-effect/);
  const notExecuted = queue.settleReconciliation(job.job_id, { outcome: "not_executed", evidenceDigest: "b".repeat(64), evidenceKind: "idempotency_probe", operator: "operator-1", effectAbsent: true }, 16);
  const reopened = queue.requeue(job.job_id, 17, { reconciliationDigest: notExecuted.reconciliation_digest });
  assert.equal(reopened.status, "queued");
  assert.equal(reopened.reconciliation_digest, null);
  assert.equal(reopened.reconciliation_outcome, null);
  assert.deepEqual(reopened.reconciliation_history, [notExecuted.reconciliation_digest]);
  const restored = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  restored.restore(queue.snapshot());
  assert.equal(restored.get(job.job_id).reconciliation_digest, reopened.reconciliation_digest);
  assert.deepEqual(restored.get(job.job_id).reconciliation_history, [notExecuted.reconciliation_digest]);
  queue.claim(job.job_id, "reconcile-worker-2", 10, 18);
  queue.beginExecution(job.job_id, "reconcile-worker-2", 19);
  queue.fail(job.job_id, "reconcile-worker-2", "transport_error", true, "transport", 20);
  const succeeded = queue.settleReconciliation(job.job_id, { outcome: "succeeded", evidenceDigest: "c".repeat(64), evidenceKind: "provider_receipt", operator: "operator-2", effectAbsent: false }, 21);
  assert.equal(succeeded.status, "completed");
  assert.equal(succeeded.result_digest, succeeded.reconciliation_digest);
  assert.equal(queue.settleReconciliation(job.job_id, { outcome: "succeeded", evidenceDigest: "c".repeat(64), evidenceKind: "provider_receipt", operator: "operator-2", effectAbsent: false }, 22).job_digest, succeeded.job_digest);
});

test("remote portfolio worker accepts an external structural queue adapter", async () => {
  const agent = agentFor();
  const privateRequests = [{ id: "adapter-coding", task: "private adapter task", domain: "coding" }];
  const plan = await agent.planWorkflowPortfolio(privateRequests);
  const admission = await agent.admitWorkflowPortfolio(privateRequests, { plan });
  const backing = new InMemoryAutonomousWorkflowPortfolioRemoteJobQueue();
  await admitAutonomousWorkflowPortfolioRemoteJob(backing, { jobId: "adapter-portfolio-job", plan, admission });
  const queue = {
    maxJobs: backing.maxJobs,
    get: backing.get.bind(backing),
    pending: backing.pending.bind(backing),
    claim: backing.claim.bind(backing),
    renew: backing.renew.bind(backing),
    checkpoint: backing.checkpoint.bind(backing),
    beginExecution: backing.beginExecution.bind(backing),
    complete: backing.complete.bind(backing),
    fail: backing.fail.bind(backing),
    reconcile: backing.reconcile.bind(backing),
    settleReconciliation: backing.settleReconciliation.bind(backing),
    reclaimExpired: backing.reclaimExpired.bind(backing),
    requeue: backing.requeue.bind(backing),
    cancel: backing.cancel.bind(backing),
    snapshot: backing.snapshot.bind(backing),
  };
  const worker = new AutonomousWorkflowPortfolioRemoteWorker(agent, queue, () => ({ requests: privateRequests, plan, admission, executionOptions: { approveProviderCall: true } }), "adapter-portfolio-worker");
  const run = await worker.run();
  assert.equal(run.completed, 1);
  assert.equal(backing.get("adapter-portfolio-job").status, "completed");
});
