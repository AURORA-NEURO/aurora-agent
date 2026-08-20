import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

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
        return new Response(JSON.stringify({ ok: true, tool: "brain_model_select_contextual", mcp: { result: { structuredContent: { context_digest: "c".repeat(64), selection: { selected_model_id: "openai/test-model" }, selection_status: "contextual_selection_exact_history" } } } }), { status: 200, headers: { "content-type": "application/json" } });
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
    context: { domain: "engineering", capability: "platform_status", risk_class: "low" },
    base: { task: "reason", input_tokens: 100, requested_output_tokens: 100, models: [model] },
    observations: [{ context_digest: "c".repeat(64), arm_id: "openai/test-model", pulls: 2, reward_sum: 1.5 }],
  });
  const prompt = await client.brainPromptAssemble({ task: "reason", max_input_tokens: 100 });
  const plan = await client.brainPlan({
    objective: "reason",
    steps: [{ id: "invoke", objective: "invoke", tool: "provider.invoke" }],
    allowed_tools: ["provider.invoke"],
    max_cost: 10,
  });
  const state = { schema: "bioprism-brain-bandit/0.1", arms: [{ arm_id: "openai/test-model" }] };
  const bandit = await client.brainBanditSelect(state);
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
  assert.equal(contextual.mcp.result.structuredContent.context_digest, "c".repeat(64));
  assert.equal(prompt.mcp.result.structuredContent.prompt_digest, "p");
  assert.equal(plan.mcp.result.structuredContent.ok, true);
  assert.equal(bandit.mcp.result.structuredContent.selected_arm_id, "openai/test-model");
  assert.equal(updated.mcp.result.structuredContent.generation, 1);
  assert.equal(outcome.mcp.result.structuredContent.status, "recorded_evaluator_reward");
  assert.equal(submitted.mcp.result.structuredContent.created, true);
  assert.equal(status.mcp.result.structuredContent.job.job_id, "job-1");
  assert.deepEqual(events.mcp.result.structuredContent.events, []);
  assert.equal(approval.mcp.result.structuredContent.job.state, "waiting_approval");
  assert.equal(health.mcp.result.structuredContent.operation, "snapshot");
  assert.equal(replay.mcp.result.structuredContent.passed, true);
  assert.deepEqual(seen.find(({ path }) => path.endsWith("brain_model_select")).body.provider_health, {
    openai: { registered: true, circuit: "closed", credential_ready: true, eligible: true },
  });
  assert.deepEqual(seen.find(({ path }) => path.endsWith("brain_model_select")).body.model_health, {
    "openai/test-model": { attempts: 12, successes: 11, failures: 1, success_rate: 11 / 12, last_latency_ms: 42 },
  });
  assert.equal(seen.length, 13);
  assert.ok(seen.every(({ body }) => !Object.prototype.hasOwnProperty.call(body, "api_key")));
  assert.ok(seen.every(({ body }) => !Object.prototype.hasOwnProperty.call(body, "prompt")));
});

test("brain client methods fail before transport on malformed input", async () => {
  const client = new ApiClient({ baseUrl: "http://127.0.0.1:18788", fetch: async () => { throw new Error("must not call transport"); } });
  await assert.rejects(() => client.brainPromptAssemble({ task: "", max_input_tokens: 10 }), ArgumentError);
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
  await assert.rejects(() => client.brainJobSubmit({
    idempotency_key: "request-1",
    spec_digest: "a".repeat(64),
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    prompt: "must be rejected",
  }), ArgumentError);
  await assert.rejects(() => client.brainJobApproval({ job_id: "job-1", action: "approve" }), ArgumentError);
  await assert.rejects(() => client.brainReplayEvaluate({
    case_id: "case-1",
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    evidence_digest: "b".repeat(64),
    signals: { schema_valid: 2 },
  }), ArgumentError);
});
