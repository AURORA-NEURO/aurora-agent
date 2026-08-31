import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError, digestCanonicalJsonTextSync } from "../dist/index.js";

const model = {
  provider: "openai",
  model: "test-model",
  capabilities: ["reasoning"],
  context_window_tokens: 16000,
  max_output_tokens: 2000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
};

const testContext = { domain: "engineering", capability: "platform_status", risk_class: "low", task_family: null };
const testContextDigest = digestCanonicalJsonTextSync(JSON.stringify(testContext));

test("client exposes the autonomous brain value-only kernel", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      seen.push({ path, body: JSON.parse(init.body) });
      if (path.endsWith("brain_model_select")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_model_select", mcp: { result: { structuredContent: { selected_model_id: "openai/test-model" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_model_select_contextual")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_model_select_contextual", mcp: { result: { structuredContent: { context_digest: testContextDigest, selection: { selected_model_id: "openai/test-model" }, selection_status: "contextual_selection_exact_history" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_prompt_assemble")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_prompt_assemble", mcp: { result: { structuredContent: { prompt_digest: "p" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_plan")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_plan", mcp: { result: { structuredContent: { ok: true } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_bandit_select")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_bandit_select", mcp: { result: { structuredContent: { selected_arm_id: "openai/test-model" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_outcome_record")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_outcome_record", mcp: { result: { structuredContent: { ok: true, status: "recorded_evaluator_reward" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_submit")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_submit", mcp: { result: { structuredContent: { ok: true, created: true, idempotent: false } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_status")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_status", mcp: { result: { structuredContent: { ok: true, job: { job_id: "job-1", state: "queued" } } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_events")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_events", mcp: { result: { structuredContent: { ok: true, events: [], next_after: 0 } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_approval")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_approval", mcp: { result: { structuredContent: { ok: true, job: { job_id: "job-1", state: "waiting_approval" } } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_claim_next")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_claim_next", mcp: { result: { structuredContent: { ok: true, operation: "claim_next", claimed: true, idempotent: false, job: { job_id: "job-1", state: "leased" }, event: null } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_cancel")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_job_cancel", mcp: { result: { structuredContent: { ok: true, operation: "cancel", cancelled: true, reconciliation_required: false, idempotent: false, job: { job_id: "job-1", state: "cancelled" }, event: null } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_job_claim") || path.endsWith("brain_job_renew") || path.endsWith("brain_job_checkpoint") || path.endsWith("brain_job_complete") || path.endsWith("brain_job_fail") || path.endsWith("brain_job_reconcile")) {
        return new Response(JSON.stringify({ ok: true, tool: path.split("/").at(-1), mcp: { result: { structuredContent: { ok: true, operation: path.split("/").at(-1).replace("brain_job_", ""), idempotent: false, job: { job_id: "job-1", state: path.endsWith("reconcile") ? "queued" : "running" }, event: null } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_model_health")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_model_health", mcp: { result: { structuredContent: { ok: true, operation: "snapshot", models: [] } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_replay_evaluate")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_replay_evaluate", mcp: { result: { structuredContent: { ok: true, passed: true, reward: 1 } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      return new Response(JSON.stringify({ ok: true, tool: "brain_bandit_update", mcp: { result: { structuredContent: { generation: 1 } } } }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const selected = await client.brainModelSelect({
    task: "reason",
    input_tokens: 100,
    requested_output_tokens: 100,
    models: [model],
    provider_health: {
      openai: { registered: true, circuit: "closed", credential_ready: true, eligible: true },
    },
    model_health: {
      "openai/test-model": { attempts: 12, successes: 11, failures: 1, success_rate: 11 / 12, last_latency_ms: 42 },
    },
  });
  const contextual = await client.brainModelSelectContextual({
    context: testContext,
    base: { task: "reason", input_tokens: 100, requested_output_tokens: 100, models: [model] },
    observations: [{ context_digest: testContextDigest, arm_id: "openai/test-model", pulls: 2, reward_sum: 1.5 }],
  });
  const prompt = await client.brainPromptAssemble({ task: "reason", max_input_tokens: 100 });
  const plan = await client.brainPlan({
    objective: "reason",
    steps: [{ id: "invoke", objective: "invoke", tool: "provider.invoke" }],
    allowed_tools: ["provider.invoke"],
    max_cost: 10,
    max_parallelism: 2,
  });
  const state = { schema: "bioprism-brain-bandit/0.1", arms: [{ arm_id: "openai/test-model" }] };
  const bandit = await client.brainBanditSelect(state);
  const contextualBandit = await client.brainBanditSelectContextual(state, testContextDigest, testContext);
  const updated = await client.brainBanditUpdate(state, { arm_id: "openai/test-model", reward: 0.8 });
  const outcome = await client.brainOutcomeRecord({
    run: { run_id: "run-1", selection_digest: "a".repeat(64), prompt_digest: "b".repeat(64), plan_digest: "c".repeat(64), provider: "openai", model: "test-model", outcome_digest: "d".repeat(64) },
    assessment: { evaluator_id: "json-contract", evaluator_version: "1", reward: 0.8, passed: true },
    bandit_state: state,
    arm_id: "openai/test-model",
  });
  const submitted = await client.brainJobSubmit({
    idempotency_key: "request-1",
    spec_digest: "a".repeat(64),
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
  });
  const status = await client.brainJobStatus({ job_id: "job-1" });
  const events = await client.brainJobEvents({ after: 0, limit: 10 });
  const approval = await client.brainJobApproval({ job_id: "job-1", action: "request", reason: "review" });
  const claimed = await client.brainJobClaim({ job_id: "job-1", worker_id: "worker-a", lease_ms: 1000 });
  const claimedNext = await client.brainJobClaimNext({ worker_id: "worker-a", lease_ms: 1000 });
  const renewed = await client.brainJobRenew({ job_id: "job-1", worker_id: "worker-a", lease_ms: 1000 });
  const checkpointed = await client.brainJobCheckpoint({ job_id: "job-1", worker_id: "worker-a", phase: "preflight", checkpoint_digest: "c".repeat(64), side_effect_boundary: "preflight" });
  const completed = await client.brainJobComplete({ job_id: "job-1", worker_id: "worker-a", result_digest: "d".repeat(64) });
  const failed = await client.brainJobFail({ job_id: "job-1", worker_id: "worker-a", reason: "timeout", retryable: true });
  const reconciled = await client.brainJobReconcile({ job_id: "job-1", outcome: "not_executed", evidence_digest: "e".repeat(64), effect_absent: true });
  const cancelled = await client.brainJobCancel({ job_id: "job-1", reason: "operator stop" });
  const health = await client.brainModelHealth({ operation: "snapshot", provider: "openai" });
  const replay = await client.brainReplayEvaluate({
    case_id: "case-1",
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    evidence_digest: "b".repeat(64),
    signals: { schema_valid: true, tests_passed: 1, evidence_complete: 1 },
  });

  assert.equal(selected.mcp.result.structuredContent.selected_model_id, "openai/test-model");
  assert.equal(contextual.mcp.result.structuredContent.context_digest, testContextDigest);
  assert.equal(prompt.mcp.result.structuredContent.prompt_digest, "p");
  assert.equal(plan.mcp.result.structuredContent.ok, true);
  assert.equal(contextualBandit.mcp.result.structuredContent.selected_arm_id, "openai/test-model");
  assert.equal(bandit.mcp.result.structuredContent.selected_arm_id, "openai/test-model");
  assert.equal(updated.mcp.result.structuredContent.generation, 1);
  assert.equal(outcome.mcp.result.structuredContent.status, "recorded_evaluator_reward");
  assert.equal(submitted.mcp.result.structuredContent.created, true);
  assert.equal(status.mcp.result.structuredContent.job.job_id, "job-1");
  assert.deepEqual(events.mcp.result.structuredContent.events, []);
  assert.equal(approval.mcp.result.structuredContent.job.state, "waiting_approval");
  assert.equal(claimed.mcp.result.structuredContent.operation, "claim");
  assert.equal(claimedNext.mcp.result.structuredContent.operation, "claim_next");
  assert.equal(renewed.mcp.result.structuredContent.operation, "renew");
  assert.equal(checkpointed.mcp.result.structuredContent.operation, "checkpoint");
  assert.equal(completed.mcp.result.structuredContent.operation, "complete");
  assert.equal(failed.mcp.result.structuredContent.operation, "fail");
  assert.equal(reconciled.mcp.result.structuredContent.operation, "reconcile");
  assert.equal(cancelled.mcp.result.structuredContent.operation, "cancel");
  assert.equal(health.mcp.result.structuredContent.operation, "snapshot");
  assert.equal(replay.mcp.result.structuredContent.passed, true);
  assert.deepEqual(seen.find(({ path }) => path.endsWith("brain_model_select")).body.provider_health, {
    openai: { registered: true, circuit: "closed", credential_ready: true, eligible: true },
  });
  assert.deepEqual(seen.find(({ path }) => path.endsWith("brain_model_select")).body.model_health, {
    "openai/test-model": { attempts: 12, successes: 11, failures: 1, success_rate: 11 / 12, last_latency_ms: 42 },
  });
  assert.equal(seen.find(({ path }) => path.endsWith("brain_plan")).body.max_parallelism, 2);
  assert.equal(seen.length, 22);
  assert.ok(seen.every(({ body }) => !Object.prototype.hasOwnProperty.call(body, "api_key")));
  assert.ok(seen.every(({ body }) => !Object.prototype.hasOwnProperty.call(body, "prompt")));
});

test("brain client methods fail before transport on malformed input", async () => {
  const client = new ApiClient({ baseUrl: "http://127.0.0.1:18788", fetch: async () => { throw new Error("must not call transport"); } });
  await assert.rejects(() => client.brainPromptAssemble({ task: "", max_input_tokens: 10 }), ArgumentError);
  await assert.rejects(() => client.brainPlan({ objective: "x", steps: [{ id: "x", objective: "x", tool: "x" }], allowed_tools: ["x"], max_cost: 1, max_parallelism: 0 }), ArgumentError);
  await assert.rejects(() => client.brainModelSelect({ task: "x", input_tokens: 1, requested_output_tokens: 1, models: [] }), ArgumentError);
  await assert.rejects(() => client.brainModelSelect({
    task: "x",
    input_tokens: 1,
    requested_output_tokens: 1,
    models: [model],
    provider_health: { openai: { circuit: "" } },
  }), ArgumentError);
  await assert.rejects(() => client.brainModelSelect({
    task: "x",
    input_tokens: 1,
    requested_output_tokens: 1,
    models: [model],
    model_health: { "openai/test-model": { prior_adjustment_applied: "yes" } },
  }), ArgumentError);
  await assert.rejects(() => client.brainModelSelect({
    task: "x",
    input_tokens: 1,
    requested_output_tokens: 1,
    models: [model],
    provider_health: { openai: { circuit: "unknown" } },
  }), ArgumentError);
  await assert.rejects(() => client.brainBanditSelectContextual({ schema: "test", arms: [] }, "0".repeat(64), testContext), /does not match its context identity/);
  await assert.rejects(() => client.brainJobSubmit({
    idempotency_key: "request-1",
    spec_digest: "a".repeat(64),
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    prompt: "must be rejected",
  }), ArgumentError);
  await assert.rejects(() => client.brainJobApproval({ job_id: "job-1", action: "approve" }), ArgumentError);
  await assert.rejects(() => client.brainJobClaim({ job_id: "job-1", worker_id: "worker-a", lease_ms: 99 }), ArgumentError);
  await assert.rejects(() => client.brainJobClaimNext({ worker_id: "worker-a", lease_ms: 99 }), ArgumentError);
  await assert.rejects(() => client.brainJobCancel({ job_id: "job-1", reason: "" }), ArgumentError);
  await assert.rejects(() => client.brainJobCheckpoint({ job_id: "job-1", worker_id: "worker-a", phase: "x", checkpoint_digest: "not-a-digest" }), ArgumentError);
  await assert.rejects(() => client.brainJobReconcile({ job_id: "job-1", outcome: "not_executed", evidence_digest: "a".repeat(64) }), ArgumentError);
  await assert.rejects(() => client.brainReplayEvaluate({
    case_id: "case-1",
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    evidence_digest: "b".repeat(64),
    signals: { schema_valid: 2 },
  }), ArgumentError);
});
