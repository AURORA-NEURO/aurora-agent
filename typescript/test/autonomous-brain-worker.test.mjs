import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainJobWorker,
  InMemoryAutonomousBrainJobScheduler,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
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

function makeBrain(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("worker-offline", (request) => {
    onRequest(request);
    return { output_text: "worker bounded result" };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return { runtime, brain: new AutonomousBrainFacade({ agent }) };
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
  const { runtime, brain } = makeBrain(() => { providerCalls += 1; });
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
