import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainJobProtectedRehydrator,
  AutonomousDurableBrainJobWorker,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  LLMRuntime,
  ProviderSetup,
  ProviderRuntimeError,
  InMemoryAutonomousRunTraceStore,
  AutonomousRunTraceRegistry,
  autonomousBrainJobSpecDigest,
  protectedValueDigest,
} from "../dist/index.js";

const model = {
  provider: "remote-brain-offline",
  model: "remote-brain-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "neuroscience", "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

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
  multimodal: "align document image and audio observations",
  evaluation: "replay a benchmark and analyze evaluator failures",
};

function response(structuredContent) {
  return { ok: true, mcp: { result: { structuredContent } } };
}

function event(jobId, sequence, eventType) {
  return {
    schema: "brain-control-event",
    sequence,
    event_type: eventType,
    job_id: jobId,
    payload: {},
    previous_digest: "0".repeat(64),
    event_digest: `${String(sequence).padStart(2, "0")}${"e".repeat(62)}`,
    head_digest: `${String(sequence).padStart(2, "0")}${"d".repeat(62)}`,
    created_ns: sequence,
    retention: "metadata_only",
  };
}

function remoteBrainApi() {
  const jobs = new Map();
  const events = new Map();
  const seen = [];
  let sequence = 0;
  const append = (jobId, eventType) => {
    sequence += 1;
    const row = event(jobId, sequence, eventType);
    events.set(jobId, [...(events.get(jobId) ?? []), row]);
    return row;
  };
  const create = (args) => {
    const id = `remote-brain-${jobs.size + 1}`;
    const job = {
      schema: "brain-job",
      job_id: id,
      idempotency_key_digest: "a".repeat(64),
      spec_digest: args.spec_digest,
      domain: args.domain,
      capability: args.capability,
      risk_class: args.risk_class,
      priority: args.priority ?? 10,
      max_attempts: args.max_attempts ?? 3,
      state: "queued",
      attempts: 0,
      lease_owner: null,
      lease_expires_ns: null,
      checkpoint_digest: args.checkpoint_digest ?? null,
      side_effect_boundary: "not_started",
      recovered_after_restart: false,
      created_sequence: sequence + 1,
      updated_sequence: sequence + 1,
      record_digest: `${String(jobs.size + 1).padStart(2, "0")}${"c".repeat(62)}`,
      spec: "not_returned; caller resolver owns rehydration",
      retention: "metadata_only",
    };
    jobs.set(id, job);
    events.set(id, []);
    append(id, "job_submitted");
    return job;
  };
  const get = (jobId) => {
    const job = jobs.get(jobId);
    if (!job) throw new Error(`unknown remote job ${jobId}`);
    return { ...job };
  };
  const update = (jobId, changes) => {
    const next = { ...get(jobId), ...changes, updated_sequence: sequence + 1 };
    jobs.set(jobId, next);
    return next;
  };
  const api = {
    seen,
    jobs,
    async brainJobSubmit(args) {
      seen.push({ operation: "submit", args });
      const job = create(args);
      return response({ schema: "brain-job-submit", ok: true, created: true, idempotent: false, job, event: events.get(job.job_id).at(-1), retention: "metadata_only", durability: "durable" });
    },
    async brainJobStatus(args) {
      seen.push({ operation: "status", args });
      return response({ schema: "brain-job-status", ok: true, job: get(args.job_id), head_digest: "d".repeat(64), durability: "durable" });
    },
    async brainJobEvents(args) {
      seen.push({ operation: "events", args });
      const page = events.get(args.job_id) ?? [];
      const after = args.after ?? 0;
      return response({ schema: "brain-job-events", ok: true, events: page.filter((item) => item.sequence > after), after, next_after: page.at(-1)?.sequence ?? after, head_digest: "d".repeat(64), chain: "sha256_prev_digest", retention: "metadata_only", durability: "durable" });
    },
    async brainJobApproval(args) {
      seen.push({ operation: "approval", args });
      const current = get(args.job_id);
      const nextState = args.action === "approve" ? "queued" : args.action === "deny" ? "cancelled" : "waiting_approval";
      const eventType = args.action === "approve" ? "job_approval_granted" : args.action === "deny" ? "job_approval_denied" : "job_approval_requested";
      const job = update(args.job_id, { state: nextState, lease_owner: null, lease_expires_ns: null });
      const approvalEvent = append(args.job_id, eventType);
      return response({ schema: "brain-job-approval", ok: true, job, event: approvalEvent, authorization: { posture: "caller_proof", verified_by_server: false, execution: "not_started" }, durability: "durable" });
    },
    async brainJobClaimNext(args) {
      seen.push({ operation: "claim_next", args });
      const job = [...jobs.values()].filter((candidate) => candidate.state === "queued").sort((left, right) => right.priority - left.priority)[0];
      if (!job) return response({ schema: "brain-job-claim-next", ok: true, operation: "claim_next", claimed: false, idempotent: false, job: null, event: null, retention: "metadata_only", durability: "durable" });
      const claimed = update(job.job_id, { state: "leased", attempts: job.attempts + 1, lease_owner: args.worker_id, lease_expires_ns: 1 });
      const claimEvent = append(job.job_id, "job_claimed");
      return response({ schema: "brain-job-claim-next", ok: true, operation: "claim_next", claimed: true, idempotent: false, job: claimed, event: claimEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobClaim(args) {
      seen.push({ operation: "claim", args });
      const current = get(args.job_id);
      const claimed = update(args.job_id, { state: "leased", attempts: current.attempts + 1, lease_owner: args.worker_id, lease_expires_ns: 1 });
      const claimEvent = append(args.job_id, "job_claimed");
      return response({ schema: "brain-job-claim", ok: true, operation: "claim", idempotent: false, job: claimed, event: claimEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobRenew(args) {
      seen.push({ operation: "renew", args });
      const job = update(args.job_id, { lease_expires_ns: 2 });
      return response({ schema: "brain-job-renew", ok: true, operation: "renew", idempotent: false, job, event: null, retention: "metadata_only", durability: "durable" });
    },
    async brainJobCheckpoint(args) {
      seen.push({ operation: "checkpoint", args });
      const job = update(args.job_id, { state: args.waiting_for_approval ? "waiting_approval" : "running", lease_owner: args.waiting_for_approval ? null : get(args.job_id).lease_owner, lease_expires_ns: args.waiting_for_approval ? null : get(args.job_id).lease_expires_ns, checkpoint_digest: args.checkpoint_digest, side_effect_boundary: args.side_effect_boundary });
      const checkpointEvent = append(args.job_id, args.waiting_for_approval ? "job_waiting_approval" : "job_checkpointed");
      return response({ schema: "brain-job-checkpoint", ok: true, operation: "checkpoint", idempotent: false, job, event: checkpointEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobComplete(args) {
      seen.push({ operation: "complete", args });
      const job = update(args.job_id, { state: "succeeded", result_digest: args.result_digest, lease_owner: null, lease_expires_ns: null });
      const completeEvent = append(args.job_id, "job_completed");
      return response({ schema: "brain-job-complete", ok: true, operation: "complete", idempotent: false, job, event: completeEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobFail(args) {
      seen.push({ operation: "fail", args });
      const current = get(args.job_id);
      const external = current.side_effect_boundary === "dispatched" || current.side_effect_boundary === "unknown";
      const nextState = external ? "reconciliation_required" : args.retryable && current.attempts < current.max_attempts ? "queued" : current.attempts >= current.max_attempts ? "dead_lettered" : "failed";
      const job = update(args.job_id, { state: nextState, lease_owner: null, lease_expires_ns: null });
      const failureEvent = append(args.job_id, nextState === "queued" ? "job_retry_queued" : nextState === "reconciliation_required" ? "job_reconciliation_required" : "job_failed");
      return response({ schema: "brain-job-fail", ok: true, operation: "fail", idempotent: false, job, event: failureEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobReconcile(args) {
      seen.push({ operation: "reconcile", args });
      const nextState = args.outcome === "succeeded" ? "succeeded" : args.outcome === "failed" ? "failed" : args.outcome === "not_executed" ? "queued" : "reconciliation_required";
      const job = update(args.job_id, { state: nextState, reconciliation_outcome: args.outcome, lease_owner: null, lease_expires_ns: null });
      const reconcileEvent = append(args.job_id, `job_reconciled_${args.outcome}`);
      return response({ schema: "brain-job-reconcile", ok: true, operation: "reconcile", idempotent: false, job, event: reconcileEvent, retention: "metadata_only", durability: "durable" });
    },
    async brainJobCancel(args) {
      seen.push({ operation: "cancel", args });
      const job = update(args.job_id, { state: "cancelled", lease_owner: null, lease_expires_ns: null });
      const cancelEvent = append(args.job_id, "job_cancelled");
      return response({ schema: "brain-job-cancel", ok: true, operation: "cancel", cancelled: true, reconciliation_required: false, job, event: cancelEvent, retention: "metadata_only", durability: "durable" });
    },
  };
  return api;
}

function makeBrain() {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("remote-brain-offline", () => {
    providerCalls += 1;
    return { output_text: "bounded remote brain result" };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return { brain: new AutonomousBrainFacade({ agent }), get providerCalls() { return providerCalls; } };
}

function policy(letter = "p") {
  return letter.repeat(64);
}

test("remote brain worker submits, approval-gates, and executes every built-in single-domain profile", async () => {
  const runtime = makeBrain();
  const { brain } = runtime;
  const api = remoteBrainApi();
  const traceStore = new InMemoryAutonomousRunTraceStore({ clock: () => 500 });
  const traceRegistry = new AutonomousRunTraceRegistry({ max_runs: 64, max_events: 4_096, max_bytes: 2_000_000 });
  const policies = new Map();
  const requests = new Map();
  const worker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-brain-worker",
    traceStore,
    traceRegistry,
    resolve: ({ job }) => ({ specDigest: job.spec_digest, policyDigest: policies.get(job.job_id), request: requests.get(job.job_id), mode: "execute", execute: { run: { candidates: [model] } } }),
  });
  const submitted = [];
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    if (domain === "cross_domain") continue;
    const request = { task: tasks[domain], domain, capability: "bounded_task" };
    const selectedPolicy = policy("abcdef"[index % 6]);
    const result = await worker.submit({ idempotencyKey: `remote-${domain}-${index}`, request, mode: "execute", policyDigest: selectedPolicy });
    assert.equal(result.status, "submitted", domain);
    submitted.push(result.job);
    policies.set(result.job.job_id, selectedPolicy);
    requests.set(result.job.job_id, request);
  }
  for (const job of submitted) {
    const waiting = await worker.runOnce(job.job_id);
    assert.equal(waiting.status, "waiting_approval", job.domain);
    await worker.approval(job.job_id, "approve", { authorizationDigest: "a".repeat(64) });
    const completed = await worker.runOnce(job.job_id);
    assert.equal(completed.status, "succeeded", job.domain);
    assert.equal(completed.execution.status, "completed", job.domain);
    assert.equal(completed.trace_registry.status, "published", job.domain);
    assert.equal(completed.trace_registry.run_import_state, "imported", job.domain);
  }
  assert.equal(runtime.providerCalls, submitted.length);
  assert.ok(api.seen.some((row) => row.operation === "claim_next" || row.operation === "claim"));
  assert.ok(api.seen.some((row) => row.operation === "checkpoint" && row.args.side_effect_boundary === "unknown"));
  assert.ok(api.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "task")));
  assert.ok(api.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "prompt")));
  assert.equal(traceRegistry.verifyIntegrity().runs, submitted.length);
});

test("remote brain worker rehydrates protected receipts across every domain and preserves explicit resolver precedence", async () => {
  const runtime = makeBrain();
  const { brain } = runtime;
  const api = remoteBrainApi();
  const values = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(
    new AutonomousProtectedRehydrationContext({ tenantId: "tenant-remote-worker", actorId: "remote-worker", sessionId: "protected", authorizationDigest: "c".repeat(64) }),
    (reference) => values.get(reference.value_digest),
    { authorizer: () => true, clock: () => 400 },
  );
  const protectedRehydration = new AutonomousBrainJobProtectedRehydrator({
    adapter: new AutonomousProtectedRehydrationAdapter(boundary),
    receiptResolver: (context) => ({
      job_id: context.jobId,
      spec_digest: context.specDigest,
      domain: context.domain,
      capability: context.capability,
      attempt: context.attempt,
      approval_released: context.approvalReleased,
      value_digest: values.get(context.jobId).valueDigest,
    }),
  });
  const worker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-protected-worker",
    protectedRehydration,
  });
  const submitted = [];
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const request = domain === "cross_domain"
      ? { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true }
      : { task: tasks[domain], domain, capability: "bounded_task" };
    const selectedPolicy = policy("abcdef"[index % 6]);
    const result = await worker.submit({ idempotencyKey: `protected-remote-${domain}-${index}`, request, mode: "execute", policyDigest: selectedPolicy });
    assert.equal(result.status, "submitted", domain);
    const resolution = {
      specDigest: result.job.spec_digest,
      policyDigest: selectedPolicy,
      request,
      mode: "execute",
      execute: { run: { candidates: [model] } },
    };
    const valueDigest = protectedValueDigest(resolution);
    values.set(valueDigest, resolution);
    values.set(result.job.job_id, { valueDigest });
    submitted.push(result.job);
  }
  for (const job of submitted) {
    const waiting = await worker.runOnce(job.job_id);
    assert.equal(waiting.status, "waiting_approval", job.domain);
    await worker.approval(job.job_id, "approve", { authorizationDigest: "d".repeat(64) });
    const completed = await worker.runOnce(job.job_id);
    assert.equal(completed.status, "succeeded", job.domain);
    assert.equal(completed.execution.status, "completed", job.domain);
  }
  // Cross-domain execution has one bounded provider call per specialist plus synthesis.
  const protectedProviderCalls = runtime.providerCalls;
  assert.equal(protectedProviderCalls, submitted.length + 2);

  const tamperedWorker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-tampered-worker",
    protectedRehydration: new AutonomousBrainJobProtectedRehydrator({
      adapter: new AutonomousProtectedRehydrationAdapter(boundary),
      receiptResolver: (context) => ({
        job_id: context.jobId,
        spec_digest: "0".repeat(64),
        domain: context.domain,
        capability: context.capability,
        attempt: context.attempt,
        approval_released: context.approvalReleased,
        value_digest: values.get(submitted[0].job_id).valueDigest,
      }),
    }),
  });
  const tamperedSubmission = await tamperedWorker.submit({ idempotencyKey: "protected-remote-tampered", request: { task: tasks.coding, domain: "coding", capability: "bounded_task" }, mode: "execute", policyDigest: policy("f") });
  const tamperedRun = await tamperedWorker.runOnce(tamperedSubmission.job.job_id);
  assert.equal(tamperedRun.status, "failed");
  assert.equal(tamperedRun.failure_code, "protocol");
  assert.equal(runtime.providerCalls, protectedProviderCalls);

  const explicitSubmission = await worker.submit({ idempotencyKey: "protected-remote-explicit", request: { task: tasks.coding, domain: "coding", capability: "bounded_task" }, mode: "execute", policyDigest: policy("e") });
  let explicitCalls = 0;
  let fallbackCalls = 0;
  const explicitWorker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-explicit-worker",
    protectedRehydration: new AutonomousBrainJobProtectedRehydrator({
      adapter: new AutonomousProtectedRehydrationAdapter(boundary),
      receiptResolver: () => { fallbackCalls += 1; throw new Error("protected fallback must remain dormant"); },
    }),
    resolve: ({ job }) => {
      explicitCalls += 1;
      return { specDigest: job.spec_digest, policyDigest: policy("e"), request: { task: tasks.coding, domain: "coding", capability: "bounded_task" }, mode: "execute", execute: { run: { candidates: [model] } } };
    },
  });
  assert.equal((await explicitWorker.runOnce(explicitSubmission.job.job_id)).status, "waiting_approval");
  await explicitWorker.approval(explicitSubmission.job.job_id, "approve", { authorizationDigest: "e".repeat(64) });
  assert.equal((await explicitWorker.runOnce(explicitSubmission.job.job_id)).status, "succeeded");
  assert.equal(explicitCalls, 2);
  assert.equal(fallbackCalls, 0);
  assert.equal(runtime.providerCalls, protectedProviderCalls + 1);
  assert.ok(api.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "task")));
  assert.ok(api.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "prompt")));
});

test("remote brain worker executes cross-domain fan-out and synthesis through the same queue contract", async () => {
  const { brain } = makeBrain();
  const api = remoteBrainApi();
  const request = { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true };
  const selectedPolicy = policy("e");
  const worker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-cross-domain-worker",
    resolve: ({ job }) => ({ specDigest: job.spec_digest, policyDigest: selectedPolicy, request, mode: "execute", execute: { run: { candidates: [model], maxParallelChildren: 2 } } }),
  });
  const submitted = await worker.submit({ idempotencyKey: "remote-cross-domain", request, mode: "execute", policyDigest: selectedPolicy });
  assert.equal(submitted.status, "submitted");
  assert.ok(submitted.plan.cross_domain_plan);
  assert.equal(submitted.job.domain, "cross_domain");
  const waiting = await worker.runOnce(submitted.job.job_id);
  assert.equal(waiting.status, "waiting_approval");
  await worker.approval(submitted.job.job_id, "approve", { authorizationDigest: "b".repeat(64) });
  const completed = await worker.runOnce(submitted.job.job_id);
  assert.equal(completed.status, "succeeded");
  assert.equal(completed.execution.status, "completed");
  assert.ok(completed.execution.run.child_runs.length >= 2);
});

test("durable worker opens provisioned credentials only after approval across direct, cycle, adaptive, and cross-domain modes", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("groq", {
    transport: {
      invoke: () => {
        providerCalls += 1;
        return { output_text: "credentialed durable worker result" };
      },
    },
  });
  await setup.provisioner.registerResolver("groq", "test-worker-secret-reference", async () => "test-worker-secret");
  const agent = new AutonomousAgent(runtime);
  const credentialedModel = { ...model, provider: "groq", model: "durable-groq-model" };
  agent.registerModel(credentialedModel);
  const brain = new AutonomousBrainFacade({ agent });
  const scope = setup.createCredentialScope(agent, { credentialProviders: ["groq"] });
  const api = remoteBrainApi();
  const requests = new Map();
  const policies = new Map();
  const resolutions = new Map();
  const worker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-provisioned-worker",
    credentialScope: scope,
    resolve: ({ job }) => ({ ...resolutions.get(job.job_id), specDigest: job.spec_digest, policyDigest: policies.get(job.job_id), request: requests.get(job.job_id) }),
  });

  const jobs = [
    {
      request: { task: tasks.coding, domain: "coding" },
      mode: "execute",
      resolution: { mode: "execute", execute: { run: { candidates: [credentialedModel] } } },
    },
    {
      request: { task: tasks.science, domain: "science" },
      mode: "cycle",
      resolution: { mode: "cycle", cycle: { cycle: { candidates: [credentialedModel] } } },
    },
    {
      request: { task: tasks.evaluation, domain: "evaluation" },
      mode: "adaptive",
      resolution: {
        mode: "adaptive",
        adaptive: {
          adaptive: {
            candidates: [credentialedModel],
            maxReplans: 0,
            evaluate: () => ({ evaluator_id: "durable-worker-evaluator", evaluator_version: "1", reward: 0.82, passed: true, replan_requested: false }),
          },
        },
      },
    },
    {
      request: { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
      mode: "execute",
      resolution: { mode: "execute", execute: { run: { candidates: [credentialedModel], maxParallelChildren: 2 } } },
    },
  ];

  for (const [index, item] of jobs.entries()) {
    const selectedPolicy = policy(String.fromCharCode(97 + index));
    const submitted = await worker.submit({ idempotencyKey: `provisioned-durable-${index}`, request: item.request, mode: item.mode, policyDigest: selectedPolicy });
    assert.equal(submitted.status, "submitted");
    requests.set(submitted.job.job_id, item.request);
    policies.set(submitted.job.job_id, selectedPolicy);
    resolutions.set(submitted.job.job_id, item.resolution);
    const waiting = await worker.runOnce(submitted.job.job_id);
    assert.equal(waiting.status, "waiting_approval", item.mode);
    assert.equal(runtime.credentials.status("groq").active_handles, 0, `${item.mode} opened credentials before approval`);
    await worker.approval(submitted.job.job_id, "approve", { authorizationDigest: "c".repeat(64) });
    const completed = await worker.runOnce(submitted.job.job_id);
    assert.equal(completed.status, "succeeded", item.mode);
    assert.equal(runtime.credentials.status("groq").active_handles, 0, `${item.mode} did not close credentials`);
    assert.equal(JSON.stringify(completed).includes("test-worker-secret"), false);
  }
  assert.ok(providerCalls >= jobs.length);
  assert.equal(JSON.stringify(api.seen).includes("test-worker-secret"), false);
  assert.equal(JSON.stringify(api.seen).includes("credentialFor"), false);
});

test("remote brain worker fails closed on spec drift and retains retryable preflight failures as queued work", async () => {
  const runtime = makeBrain();
  const { brain } = runtime;
  const api = remoteBrainApi();
  const request = { task: tasks.coding, domain: "coding", capability: "bounded_task" };
  const selectedPolicy = policy("d");
  const driftWorker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-drift-worker",
    resolve: ({ job }) => ({ specDigest: job.spec_digest, policyDigest: selectedPolicy, request: { ...request, task: "tampered private request" }, mode: "execute" }),
  });
  const drift = await driftWorker.submit({ idempotencyKey: "remote-drift", request, mode: "execute", policyDigest: selectedPolicy });
  const failed = await driftWorker.runOnce(drift.job.job_id);
  assert.equal(failed.status, "failed");
  assert.equal(failed.error_class, "ArgumentError");
  assert.equal(api.jobs.get(drift.job.job_id).state, "failed");
  assert.equal(runtime.providerCalls, 0);
  assert.equal(JSON.stringify(api.seen).includes("tampered private request"), false);

  const retryWorker = new AutonomousDurableBrainJobWorker({
    brain,
    apiClient: api,
    workerId: "remote-retry-worker",
    resolve: () => { throw new ProviderRuntimeError("temporary resolver outage", { code: "transport", retryable: true }); },
  });
  const retry = await retryWorker.submit({ idempotencyKey: "remote-retry", request, mode: "execute", policyDigest: selectedPolicy });
  const scheduled = await retryWorker.runOnce(retry.job.job_id);
  assert.equal(scheduled.status, "retry_scheduled");
  assert.equal(scheduled.error_retryable, true);
  assert.equal(api.jobs.get(retry.job.job_id).state, "queued");
});

test("remote brain worker reports already-terminal claims before requiring a lease", async () => {
  const runtime = makeBrain();
  const api = remoteBrainApi();
  const worker = new AutonomousDurableBrainJobWorker({
    brain: runtime.brain,
    apiClient: api,
    workerId: "terminal-observer",
    resolve: () => { throw new Error("terminal jobs must not rehydrate private work"); },
  });
  const request = { task: tasks.coding, domain: "coding", capability: "bounded_task" };
  const submission = await worker.submit({ idempotencyKey: "remote-terminal", request, mode: "execute", policyDigest: policy("d") });
  const terminal = { ...api.jobs.get(submission.job.job_id), state: "succeeded", lease_owner: null, lease_expires_ns: null, result_digest: "c".repeat(64) };
  api.brainJobClaim = async (args) => response({ schema: "brain-job-claim", ok: true, operation: "claim", idempotent: false, job: terminal, event: null, retention: "metadata_only", durability: "durable" });
  const observed = await worker.runOnce(submission.job.job_id);
  assert.equal(observed.status, "already_terminal");
  assert.equal(runtime.providerCalls, 0);
});

test("remote brain worker quarantines a settlement response that changes the job specification", async () => {
  const runtime = makeBrain();
  const api = remoteBrainApi();
  const worker = new AutonomousDurableBrainJobWorker({
    brain: runtime.brain,
    apiClient: api,
    workerId: "settlement-fence",
    resolve: ({ job }) => ({ specDigest: job.spec_digest, policyDigest: policy("e"), request: { task: tasks.coding, domain: "coding", capability: "bounded_task" }, mode: "execute" }),
  });
  const request = { task: tasks.coding, domain: "coding", capability: "bounded_task" };
  const submission = await worker.submit({ idempotencyKey: "remote-settlement-fence", request, mode: "execute", policyDigest: policy("e") });
  await worker.runOnce(submission.job.job_id);
  await worker.approval(submission.job.job_id, "approve", { authorizationDigest: "a".repeat(64) });
  const originalComplete = api.brainJobComplete;
  api.brainJobComplete = async (args) => {
    const projected = await originalComplete(args);
    projected.mcp.result.structuredContent.job = { ...projected.mcp.result.structuredContent.job, spec_digest: "b".repeat(64) };
    return projected;
  };
  const quarantined = await worker.runOnce(submission.job.job_id);
  assert.equal(quarantined.status, "reconciliation_required");
  assert.equal(runtime.providerCalls, 1);
});
