import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainJobWorker,
  InMemoryAutonomousBrainJobScheduler,
  InMemoryAutonomousBrainJobSchedulerPersistence,
  InMemoryAutonomousModelHealthStore,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
  AutonomousBrainJobSchedulerPersistenceCoordinator,
  ProviderRuntimeError,
  autonomousBrainJobSpecDigest,
} from "../dist/index.js";

const tasks = {
  coding: "debug and verify a bounded repository change",
  browser: "compare fresh web sources and report citation gaps",
  data: "profile a dataset schema, lineage, and missingness",
  science: "design a reproducible experiment and uncertainty report",
  biomedical: "review biomedical evidence with safety boundaries",
  neuroscience: "analyze signal preprocessing and study limitations",
  operations: "prepare a reversible incident rollback runbook",
  enterprise: "map governance ownership, policy, and approvals",
  multi_agent: "delegate specialists and reconcile their evidence",
  multimodal: "align document, image, and audio observations",
  cross_domain: "synthesize evidence across several disciplines",
  evaluation: "replay a benchmark and analyze evaluator failures",
};

const model = {
  provider: "worker-offline",
  model: "worker-model",
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

function makeBrain(onRequest = () => {}, modelHealthStore = undefined) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("worker-offline", (request) => {
    onRequest(request);
    return { output_text: "worker bounded result" };
  });
  const agent = new AutonomousAgent(runtime, modelHealthStore ? { modelHealthStore } : undefined);
  agent.registerModel(model);
  return { runtime, agent, brain: new AutonomousBrainFacade({ agent }) };
}

function policyDigest(letter = "p") {
  return letter.repeat(64);
}

function requestFor(domain) {
  return { task: tasks[domain], domain, capability: "bounded_task" };
}

function jobFor(index, request, mode = "execute", policy = policyDigest()) {
  return {
    jobId: `worker-job-${index}`,
    idempotencyKey: `worker-idempotency-${index}-private-task-never-retained`,
    specDigest: autonomousBrainJobSpecDigest({ request, mode, policyDigest: policy }),
    domain: request.domain,
    capability: request.capability,
    riskClass: "review",
    maxAttempts: 3,
  };
}

test("durable brain worker preserves approval gates and completes every domain through one traced provider boundary", async () => {
  let providerCalls = 0;
  const healthStore = new InMemoryAutonomousModelHealthStore({ clock: () => 1_000 });
  const { runtime, brain } = makeBrain(() => { providerCalls += 1; }, healthStore);
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 32, clock: () => 1_000 });
  const traces = new InMemoryAutonomousRunTraceStore();
  const policies = new Map();
  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const domain = AUTONOMOUS_DOMAIN_NAMES[index];
    const request = requestFor(domain);
    const policy = policyDigest("abcdef"[index % 6]);
    policies.set(`worker-job-${index}`, policy);
    scheduler.submit(jobFor(index, request, "execute", policy), 1_000);
  }
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-a",
    traceStore: traces,
    resolve: ({ job }) => {
      const domain = job.domain;
      const request = requestFor(domain);
      const policy = policies.get(job.job_id);
      return {
        specDigest: job.spec_digest,
        policyDigest: policy,
        request,
        mode: "execute",
        execute: { approveProviderCall: true, run: { candidates: [model] } },
      };
    },
    leaseMs: 10_000,
    heartbeatMs: 1_000,
  });

  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const jobId = `worker-job-${index}`;
    const domain = AUTONOMOUS_DOMAIN_NAMES[index];
    const callsBeforeApproval = providerCalls;
    const waiting = await worker.runOnce(jobId, 1_000);
    assert.equal(waiting.status, "waiting_approval", jobId);
    assert.equal(scheduler.get(jobId).state, "waiting_approval", jobId);
    assert.equal(providerCalls, callsBeforeApproval, `${jobId} dispatched before approval`);
    scheduler.resumeApproval(jobId, "operator-1", "reviewed scope approved", 1_001 + index);
    const completed = await worker.runOnce(jobId, 2_000 + index);
    assert.equal(completed.status, "succeeded", jobId);
    assert.equal(completed.execution.run.status, "completed", jobId);
    assert.equal(completed.trace.status, "completed", jobId);
    assert.ok(completed.trace.provider_invocations >= 1, jobId);
    assert.ok(completed.trace.plan_digest, jobId);
    assert.equal(JSON.stringify(completed.trace).includes(tasks[domain]), false, jobId);
  }
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(runtime.providerStatus("worker-offline").attempts, AUTONOMOUS_DOMAIN_NAMES.length);
  const health = await healthStore.health({ limit: 32 });
  assert.equal(health.length, 1);
  assert.equal(health[0].attempts, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(health[0].successes, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(health[0].failures, 0);
  assert.equal((await healthStore.verifyIntegrity()).events, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(traces.verifyIntegrity().verified, true);
  assert.equal(JSON.stringify(scheduler.snapshot()).includes("private-task-never-retained"), false);
  assert.equal(scheduler.inventory({ limit: 32 }).every((job) => job.state === "succeeded"), true);
});

test("worker traces closed-loop cycle and evaluator-guided cross-domain learning without replaying the provider", async () => {
  const { runtime, brain } = makeBrain();
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 8, clock: () => 5_000 });
  const traces = new InMemoryAutonomousRunTraceStore();
  const cycleRequest = requestFor("science");
  const cyclePolicy = policyDigest("c");
  scheduler.submit(jobFor(20, cycleRequest, "cycle", cyclePolicy), 5_000);
  const adaptiveRequest = { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true };
  const adaptivePolicy = policyDigest("d");
  scheduler.submit({
    jobId: "worker-job-adaptive",
    idempotencyKey: "worker-adaptive-private-task",
    specDigest: autonomousBrainJobSpecDigest({ request: adaptiveRequest, mode: "adaptive", policyDigest: adaptivePolicy }),
    domain: "biomedical",
    capability: "bounded_task",
    riskClass: "review",
    maxAttempts: 3,
  }, 5_000);
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-cycle",
    traceStore: traces,
    resolve: ({ job }) => {
      if (job.job_id === "worker-job-20") return {
        specDigest: job.spec_digest,
        policyDigest: cyclePolicy,
        request: cycleRequest,
        mode: "cycle",
        cycle: { approveProviderCall: true, cycle: { candidates: [model] } },
      };
      return {
        specDigest: job.spec_digest,
        policyDigest: adaptivePolicy,
        request: adaptiveRequest,
        mode: "adaptive",
        adaptive: {
          approveProviderCall: true,
          adaptive: {
            maxReplans: 0,
            synthesize: false,
            maxParallelChildren: 2,
            subtasks: [
              { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
              { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
            ],
            evaluate: () => ({ evaluator_id: "worker-reviewer", evaluator_version: "1", reward: 0.86, passed: true, replan_requested: false, rewards: {} }),
          },
        },
      };
    },
  });

  const first = await worker.runOnce("worker-job-20", 5_000);
  assert.equal(first.status, "waiting_approval");
  scheduler.resumeApproval("worker-job-20", "operator-cycle", "cycle approved", 5_001);
  const cycle = await worker.runOnce("worker-job-20", 5_002);
  assert.equal(cycle.status, "succeeded");
  assert.equal(cycle.cycle.status, "completed");
  assert.equal(cycle.trace.status, "completed");
  assert.ok(cycle.trace.provider_invocations >= 1);

  const adaptiveFirst = await worker.runOnce("worker-job-adaptive", 5_003);
  assert.equal(adaptiveFirst.status, "waiting_approval");
  scheduler.resumeApproval("worker-job-adaptive", "operator-adaptive", "adaptive scope approved", 5_004);
  const adaptive = await worker.runOnce("worker-job-adaptive", 5_005);
  assert.equal(adaptive.status, "succeeded");
  assert.equal(adaptive.adaptive.status, "completed");
  assert.equal(adaptive.adaptive.adaptive.final.run.child_runs.length, 2);
  assert.equal(adaptive.trace.status, "completed");
  assert.ok(adaptive.trace.provider_invocations >= 2);
  assert.ok(adaptive.trace.event_count >= 7);
  assert.equal(runtime.providerStatus("worker-offline").attempts >= 3, true);
});

test("worker rejects spec drift before dispatch and quarantines uncertain provider failures", async () => {
  const { runtime, brain } = makeBrain();
  runtime.registerInMemoryProvider("worker-failing", () => { throw new Error("simulated provider failure"); });
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 8, clock: () => 9_000 });
  const request = requestFor("coding");
  const policy = policyDigest("e");
  scheduler.submit(jobFor(30, request, "execute", policy), 9_000);
  scheduler.submit({
    jobId: "worker-job-failure",
    idempotencyKey: "worker-failure-private-task",
    specDigest: autonomousBrainJobSpecDigest({ request, mode: "execute", policyDigest: policyDigest("f") }),
    domain: "coding",
    capability: "bounded_task",
    riskClass: "review",
  }, 9_000);
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-failure",
    resolve: ({ job }) => {
      if (job.job_id === "worker-job-30") return { specDigest: job.spec_digest, policyDigest: policyDigest("z"), request, mode: "execute", execute: { run: { candidates: [model] } } };
      return { specDigest: job.spec_digest, policyDigest: policyDigest("f"), request, mode: "execute", execute: { approveProviderCall: true, run: { candidates: [{ ...model, provider: "worker-failing", model: "failing-model" }] } } };
    },
  });
  const drift = await worker.runOnce("worker-job-30", 9_000);
  assert.equal(drift.status, "failed");
  assert.equal(scheduler.get("worker-job-30").state, "failed");
  assert.equal(runtime.providerStatus("worker-offline").attempts, 0);

  const failure = await worker.runOnce("worker-job-failure", 9_001);
  assert.equal(failure.status, "waiting_approval");
  scheduler.resumeApproval("worker-job-failure", "operator-failure", "approved for failure-path test", 9_002);
  const uncertain = await worker.runOnce("worker-job-failure", 9_003);
  assert.equal(uncertain.status, "reconciliation_required");
  assert.equal(scheduler.get("worker-job-failure").state, "reconciliation_required");
});

test("worker retries only typed preflight failures and dead-letters after bounded exhaustion", async () => {
  const { brain } = makeBrain();
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 8, clock: () => 12_000 });
  const recoverRequest = requestFor("coding");
  const recoverPolicy = policyDigest("a");
  const exhaustedRequest = requestFor("evaluation");
  const exhaustedPolicy = policyDigest("b");
  scheduler.submit(jobFor(40, recoverRequest, "execute", recoverPolicy), 12_000);
  scheduler.submit({
    ...jobFor(41, exhaustedRequest, "execute", exhaustedPolicy),
    maxAttempts: 2,
  }, 12_000);
  const attempts = new Map();
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-retry",
    resolve: ({ job }) => {
      const attempt = (attempts.get(job.job_id) ?? 0) + 1;
      attempts.set(job.job_id, attempt);
      if (job.job_id === "worker-job-40" && attempt === 1) throw new ProviderRuntimeError("temporary local preflight refusal", { code: "timeout", retryable: true });
      if (job.job_id === "worker-job-41") throw new ProviderRuntimeError("persistent local preflight refusal", { code: "transport", retryable: true });
      const request = recoverRequest;
      return {
        specDigest: job.spec_digest,
        policyDigest: recoverPolicy,
        request,
        mode: "execute",
        execute: { approveProviderCall: true, run: { candidates: [model] } },
      };
    },
  });

  const scheduled = await worker.runOnce("worker-job-40", 12_000);
  assert.equal(scheduled.status, "retry_scheduled");
  assert.equal(scheduled.error_retryable, true);
  assert.equal(scheduler.get("worker-job-40").state, "queued");
  assert.equal(scheduler.get("worker-job-40").attempts, 1);
  const waiting = await worker.runOnce("worker-job-40", 12_002);
  assert.equal(waiting.status, "waiting_approval");
  scheduler.resumeApproval("worker-job-40", "operator-retry", "approved after plan review", 12_003);
  const recovered = await worker.runOnce("worker-job-40", 12_004);
  assert.equal(recovered.status, "succeeded");
  assert.equal(scheduler.get("worker-job-40").state, "succeeded");

  const exhaustedFirst = await worker.runOnce("worker-job-41", 12_005);
  assert.equal(exhaustedFirst.status, "retry_scheduled");
  assert.equal(scheduler.get("worker-job-41").state, "queued");
  const exhaustedSecond = await worker.runOnce("worker-job-41", 12_006);
  assert.equal(exhaustedSecond.status, "failed");
  assert.equal(exhaustedSecond.error_retryable, true);
  assert.equal(scheduler.get("worker-job-41").state, "dead_lettered");
  assert.equal((await worker.run({ limit: 4 })).status, "empty");

  const immediateRequest = requestFor("operations");
  const immediatePolicy = policyDigest("d");
  scheduler.submit(jobFor(42, immediateRequest, "execute", immediatePolicy), 12_007);
  const immediateWorker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-no-retry",
    retryPreflightFailures: false,
    resolve: () => { throw new ProviderRuntimeError("retryable but explicitly non-retried", { code: "timeout", retryable: true }); },
  });
  const immediate = await immediateWorker.runOnce("worker-job-42", 12_008);
  assert.equal(immediate.status, "failed");
  assert.equal(immediate.error_retryable, true);
  assert.equal(scheduler.get("worker-job-42").state, "failed");
});

test("worker batch reports retry backpressure without hot-looping a queued job", async () => {
  const { brain } = makeBrain();
  const scheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 4, clock: () => 13_000 });
  const request = requestFor("operations");
  const policy = policyDigest("c");
  scheduler.submit(jobFor(50, request, "execute", policy), 13_000);
  let calls = 0;
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler,
    workerId: "worker-batch-retry",
    resolve: () => {
      calls += 1;
      throw new ProviderRuntimeError("retryable local planning transport", { code: "transport", retryable: true });
    },
  });
  const batch = await worker.run({ limit: 4 });
  assert.equal(batch.status, "partial");
  assert.equal(batch.claimed_count, 1);
  assert.equal(batch.retry_scheduled_count, 1);
  assert.equal(batch.failed_count, 0);
  assert.equal(calls, 1);
  assert.equal(scheduler.get("worker-job-50").state, "queued");
});

test("worker restores a metadata-only scheduler and persists approval recovery across every domain", async () => {
  const { brain } = makeBrain();
  const persistence = new InMemoryAutonomousBrainJobSchedulerPersistence();
  const initialScheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 32, clock: () => 14_000 });
  const initialController = new AutonomousBrainJobSchedulerPersistenceCoordinator(initialScheduler, persistence);
  const policies = new Map();
  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const domain = AUTONOMOUS_DOMAIN_NAMES[index];
    const request = requestFor(domain);
    const policy = policyDigest("abcdef"[index % 6]);
    policies.set(`worker-job-restart-${index}`, policy);
    initialScheduler.submit({
      jobId: `worker-job-restart-${index}`,
      idempotencyKey: `restart-idempotency-${index}-private-task-never-retained`,
      specDigest: autonomousBrainJobSpecDigest({ request, mode: "execute", policyDigest: policy }),
      domain,
      capability: request.capability,
      riskClass: "review",
      maxAttempts: 3,
    }, 14_000);
  }
  await initialController.flush();

  const restartedScheduler = new InMemoryAutonomousBrainJobScheduler({ maxJobs: 32, clock: () => 14_000 });
  const restartedController = new AutonomousBrainJobSchedulerPersistenceCoordinator(restartedScheduler, persistence);
  const worker = new AutonomousBrainJobWorker({
    brain,
    scheduler: restartedScheduler,
    persistence: restartedController,
    workerId: "worker-restarted",
    resolve: ({ job }) => {
      const request = requestFor(job.domain);
      return {
        specDigest: job.spec_digest,
        policyDigest: policies.get(job.job_id),
        request,
        mode: "execute",
        execute: { approveProviderCall: true, run: { candidates: [model] } },
      };
    },
  });
  await assert.rejects(worker.runOnce("worker-job-restart-0", 14_001), /restore before execution/);
  const restored = await worker.restore();
  assert.equal(restored.jobs.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(JSON.stringify(restored).includes("private-task-never-retained"), false);

  for (let index = 0; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const jobId = `worker-job-restart-${index}`;
    const waiting = await worker.runOnce(jobId, 14_100 + index);
    assert.equal(waiting.status, "waiting_approval", jobId);
    assert.equal(persistence.read().jobs.find((job) => job.job_id === jobId).state, "waiting_approval", jobId);
    await worker.resumeApproval(jobId, "operator-restart", "approved after process restart", 14_200 + index);
    assert.equal(persistence.read().jobs.find((job) => job.job_id === jobId).state, "queued", jobId);
    const completed = await worker.runOnce(jobId, 14_300 + index);
    assert.equal(completed.status, "succeeded", jobId);
    assert.equal(persistence.read().jobs.find((job) => job.job_id === jobId).state, "succeeded", jobId);
  }
  assert.equal(restartedScheduler.inventory({ limit: 32 }).every((job) => job.state === "succeeded"), true);
  assert.equal(restartedScheduler.verifyIntegrity().verified, true);
  assert.equal(JSON.stringify(persistence.read()).includes("review a bounded"), false);
});
