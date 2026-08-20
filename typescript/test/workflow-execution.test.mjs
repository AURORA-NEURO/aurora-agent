import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousDurableJobController,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function model() {
  return {
    provider: "workflow",
    model: "workflow-model",
    capabilities: ["reasoning", "code", "coordination", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 50,
    cost_per_million_tokens: 10,
    reliability: 0.99,
  };
}

test("workflow executor checkpoints stages, pauses at a bounded budget, and resumes by digest", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: `stage-output-${calls}` }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const executor = new AutonomousWorkflowExecutor(agent, store);
  const task = "Implement and verify this repository change";

  const first = await executor.start(task, { domain: "coding", jobId: "workflow-job-1", candidates: agent.models(), approveProviderCall: true, maxStages: 2 });
  assert.equal(first.status, "paused");
  assert.equal(first.completed_stage_count, 2);
  assert.equal(first.total_stage_count, 5);
  assert.equal(first.checkpoint.status, "paused");
  assert.equal(JSON.stringify(first.checkpoint).includes(task), false);
  assert.equal(calls, 2);

  const resumed = await executor.resume("workflow-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.completed_stage_count, 5);
  assert.equal(resumed.checkpoint.next_stage_id, null);
  assert.equal(calls, 5);
  assert.deepEqual(resumed.checkpoint.completed_stage_ids, ["scope", "inspect", "implement", "verify", "handoff"]);
  assert.ok(resumed.events.length >= 6);
  for (let index = 1; index < resumed.events.length; index += 1) {
    assert.equal(resumed.events[index].previous_event_digest, resumed.events[index - 1].event_digest);
    assert.equal(resumed.events[index].sequence, resumed.events[index - 1].sequence + 1);
  }
  await assert.rejects(() => executor.resume("workflow-job-1", "A different task", { candidates: agent.models(), approveProviderCall: true }), /digest/);
});

test("workflow stage failures checkpoint typed redacted retry metadata", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { throw new Error("provider body contained secret material"); },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const result = await executor.start("Implement and verify this failure path", {
    domain: "coding",
    jobId: "workflow-failure-1",
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.equal(result.status, "failed");
  const outcome = result.checkpoint.stage_outcomes.at(-1);
  assert.equal(outcome.status, "failed");
  assert.equal(outcome.error_class, "ProviderRuntimeError");
  assert.equal(outcome.error_code, "transport");
  assert.equal(outcome.retryable, true);
  assert.equal(outcome.status_code, null);
  assert.equal(JSON.stringify(result.checkpoint).includes("provider body contained secret material"), false);
  assert.equal(result.events.at(-1).event_type, "stage_failed");
});

test("workflow executor exposes approval pauses and checkpoint readiness for every built-in domain", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("provider must not be called before approval"); } });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
    const result = await executor.start(`Review a ${profile.domain} workflow`, { domain: profile.domain, jobId: `domain-${profile.domain}`, candidates: agent.models() });
    assert.equal(result.status, "approval_required", profile.domain);
    assert.equal(result.checkpoint.domain, profile.domain);
    assert.equal(result.checkpoint.workflow_digest, profile.workflow.workflow_digest);
    assert.equal(result.checkpoint.completed_stage_ids.length, 0);
    assert.equal(result.events.at(-1).event_type, "approval_required");
  }
});

test("durable job controller sends only metadata, preserves server approval, and rehydrates local execution", async () => {
  let calls = 0;
  let serverState = "queued";
  const seen = [];
  const job = {
    schema: "brain-job",
    job_id: "server-job-1",
    idempotency_key_digest: "a".repeat(64),
    spec_digest: "b".repeat(64),
    domain: "coding",
    capability: "coding_delivery",
    risk_class: "engineering_change",
    priority: 1,
    max_attempts: 3,
    state: serverState,
    attempts: 0,
    side_effect_boundary: "not_started",
    recovered_after_restart: false,
    created_sequence: 1,
    updated_sequence: 1,
    record_digest: "c".repeat(64),
    spec: "not_returned",
    retention: "metadata_only",
  };
  const projection = (structuredContent) => ({ ok: true, mcp: { result: { structuredContent } } });
  const api = {
    async brainJobSubmit(args) {
      seen.push({ operation: "submit", args });
      job.spec_digest = args.spec_digest;
      return projection({ job: { ...job } });
    },
    async brainJobStatus(args) {
      seen.push({ operation: "status", args });
      return projection({ job: { ...job, state: serverState } });
    },
    async brainJobEvents(args) {
      seen.push({ operation: "events", args });
      return projection({ events: [], after: args.after ?? 0, next_after: args.after ?? 0, head_digest: "d".repeat(64), chain: "sha256_prev_digest", retention: "metadata_only" });
    },
    async brainJobApproval(args) {
      seen.push({ operation: "approval", args });
      serverState = args.action === "request" ? "waiting_approval" : "queued";
      return projection({ job: { ...job, state: serverState }, authorization: { posture: "caller_proof", verified_by_server: false, execution: "not_started" } });
    },
  };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "stage complete" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const controller = new AutonomousDurableJobController(agent, api, new InMemoryAutonomousWorkflowCheckpointStore());
  const task = "Implement this durable coding workflow";
  const submitted = await controller.submit(task, { idempotencyKey: "durable-request-1", domain: "coding", candidates: agent.models() });
  assert.equal(submitted.status, "submitted");
  assert.equal(submitted.job.job_id, "server-job-1");
  assert.equal(seen[0].args.spec_digest, submitted.spec_digest);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].args, "task"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].args, "prompt"), false);
  await controller.approval("server-job-1", "request", { reason: "operator review" });
  const blocked = await controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true });
  assert.equal(blocked.local.status, "approval_required");
  assert.equal(calls, 0);
  await controller.approval("server-job-1", "approve", { authorizationDigest: "e".repeat(64) });
  const executed = await controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 1 });
  assert.equal(executed.local.status, "paused");
  assert.equal(executed.local.completed_stage_count, 1);
  assert.equal(calls, 1);
  assert.ok(seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "prompt")));

  serverState = "queued";
  job.domain = "not-a-built-in-domain";
  await assert.rejects(
    () => controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 1 }),
    /unsupported autonomous domain/,
  );
});
