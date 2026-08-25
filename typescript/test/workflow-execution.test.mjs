import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousDurableJobController,
  AutonomousDurableJobWorker,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  AutonomousWorkflowPersistenceCoordinator,
  TransactionalJsonAutonomousWorkflowSnapshotPersistence,
  AutonomousWorkflowExecutor,
  AutonomousPromptTemplate,
  AUTONOMOUS_WORKFLOW_EXECUTION_RECEIPT_SCHEMA,
  validateAutonomousWorkflowExecutionReceipt,
  CredentialStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  digestJson,
  openaiCompatibleProvider,
  ProviderRuntimeError,
  builtinAutonomousPromptRegistry,
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
    capabilities: ["reasoning", "code", "coordination", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multimodal", "evaluation", "structured_output"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 50,
    cost_per_million_tokens: 10,
    reliability: 0.99,
  };
}

function workflowStagePayload(init, fallbackStage = "stage") {
  let body = {};
  try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* the provider test still returns a bounded fixture */ }
  const prompt = JSON.stringify(body.messages ?? []);
  const stageId = prompt.match(/Execute workflow stage ([A-Za-z0-9_.:-]+)/)?.[1] ?? fallbackStage;
  return {
    stage_id: stageId,
    status: "completed",
    evidence: [`evidence-${stageId}`],
    uncertainty: [],
    notes: `completed ${stageId} with bounded test evidence`,
    next_actions: [],
  };
}

function remoteJobFixture() {
  let state = "queued";
  let job = {
    schema: "brain-job",
    job_id: "remote-worker-job-1",
    idempotency_key_digest: "a".repeat(64),
    spec_digest: "b".repeat(64),
    domain: "coding",
    capability: "coding_delivery",
    risk_class: "engineering_change",
    priority: 10,
    max_attempts: 3,
    state,
    attempts: 0,
    lease_owner: null,
    lease_expires_ns: null,
    side_effect_boundary: "not_started",
    recovered_after_restart: false,
    created_sequence: 1,
    updated_sequence: 1,
    record_digest: "c".repeat(64),
    spec: "not_returned",
    retention: "metadata_only",
  };
  const seen = [];
  const projection = (structuredContent) => ({ ok: true, mcp: { result: { structuredContent } } });
  const withJob = (extra = {}) => ({ ...job, state, ...extra });
  const api = {
    async brainJobSubmit(args) {
      seen.push({ operation: "submit", args });
      job = { ...job, spec_digest: args.spec_digest };
      return projection({ job: withJob() });
    },
    async brainJobStatus(args) {
      seen.push({ operation: "status", args });
      return projection({ job: withJob() });
    },
    async brainJobEvents(args) {
      seen.push({ operation: "events", args });
      return projection({ events: [], after: args.after ?? 0, next_after: args.after ?? 0, head_digest: "d".repeat(64), chain: "sha256_prev_digest", retention: "metadata_only" });
    },
    async brainJobApproval(args) {
      seen.push({ operation: "approval", args });
      state = args.action === "request" ? "waiting_approval" : "queued";
      return projection({ job: withJob(), authorization: { posture: "caller_proof", verified_by_server: false, execution: "not_started" } });
    },
    async brainJobClaimNext(args) {
      seen.push({ operation: "claim_next", args });
      if (state !== "queued") return projection({ operation: "claim_next", claimed: false, idempotent: false, job: null, event: null });
      state = "leased";
      job = { ...job, attempts: job.attempts + 1, lease_owner: args.worker_id, lease_expires_ns: 1 };
      return projection({ operation: "claim_next", claimed: true, idempotent: false, job: withJob(), event: null });
    },
    async brainJobClaim(args) {
      seen.push({ operation: "claim", args });
      if (state === "leased" && job.lease_owner === args.worker_id) return projection({ operation: "claim", idempotent: true, job: withJob(), event: null });
      state = "leased";
      job = { ...job, lease_owner: args.worker_id, lease_expires_ns: 1 };
      return projection({ operation: "claim", idempotent: false, job: withJob(), event: null });
    },
    async brainJobRenew(args) {
      seen.push({ operation: "renew", args });
      job = { ...job, lease_expires_ns: 2 };
      return projection({ operation: "renew", idempotent: false, job: withJob(), event: null });
    },
    async brainJobCheckpoint(args) {
      seen.push({ operation: "checkpoint", args });
      state = args.waiting_for_approval ? "waiting_approval" : "running";
      job = { ...job, side_effect_boundary: args.side_effect_boundary, checkpoint_digest: args.checkpoint_digest };
      return projection({ operation: "checkpoint", idempotent: false, job: withJob(), event: null });
    },
    async brainJobComplete(args) {
      seen.push({ operation: "complete", args });
      state = "succeeded";
      job = { ...job, result_digest: args.result_digest, lease_owner: null, lease_expires_ns: null };
      return projection({ operation: "complete", idempotent: false, job: withJob(), event: null });
    },
    async brainJobFail(args) {
      seen.push({ operation: "fail", args });
      state = args.retryable ? "queued" : "failed";
      job = { ...job, lease_owner: null, lease_expires_ns: null };
      return projection({ operation: "fail", idempotent: false, job: withJob(), event: null });
    },
    async brainJobCancel(args) {
      seen.push({ operation: "cancel", args });
      state = "cancelled";
      return projection({ operation: "cancel", cancelled: true, reconciliation_required: false, idempotent: false, job: withJob(), event: null });
    },
  };
  return { api, seen, get state() { return state; }, get job() { return job; } };
}

test("workflow executor checkpoints stages, pauses at a bounded budget, and resumes by digest", async () => {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
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
  assert.equal(first.execution_receipt.next_action, "continue_workflow");
  assert.equal(first.execution_receipt.completed_stage_ids.length, 2);
  assert.equal(first.execution_receipt.incomplete_stage_ids.length, 3);
  assert.equal(first.execution_receipt.progress, 0.4);
  assert.equal(first.execution_receipt.safe_to_continue, true);
  await validateAutonomousWorkflowExecutionReceipt(first.execution_receipt);
  assert.equal(JSON.stringify(first.checkpoint).includes(task), false);
  assert.equal(calls, 2);
  assert.match(JSON.stringify(bodies[1].messages), /evidence-scope/, "dependent stages receive bounded prior-stage evidence");

  const resumed = await executor.resume("workflow-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.completed_stage_count, 5);
  assert.equal(resumed.checkpoint.next_stage_id, null);
  assert.equal(resumed.execution_receipt.next_action, "complete");
  assert.equal(resumed.execution_receipt.progress, 1);
  assert.equal(resumed.execution_receipt.safe_to_continue, false);
  await validateAutonomousWorkflowExecutionReceipt(resumed.execution_receipt);
  assert.equal(calls, 5);
  assert.deepEqual(resumed.checkpoint.completed_stage_ids, ["scope", "inspect", "implement", "verify", "handoff"]);
  assert.ok(resumed.events.length >= 6);
  for (let index = 1; index < resumed.events.length; index += 1) {
    assert.equal(resumed.events[index].previous_event_digest, resumed.events[index - 1].event_digest);
    assert.equal(resumed.events[index].sequence, resumed.events[index - 1].sequence + 1);
  }
  await assert.rejects(() => executor.resume("workflow-job-1", "A different task", { candidates: agent.models(), approveProviderCall: true }), /digest/);
});

test("durable workflows use approved semantic routing once and persist the route identity", async () => {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      const isRouter = JSON.stringify(bodies.at(-1).messages ?? []).includes("bounded autonomous task router");
      const content = isRouter
        ? JSON.stringify({ selected_domains: [{ domain: "coding", score: 0.98, rationale: "The task is a coding migration." }], confidence: 0.98, abstain: false, abstain_reason: null })
        : JSON.stringify(workflowStagePayload(init, `stage-${calls}`));
      return jsonResponse({ choices: [{ message: { role: "assistant", content }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-workflow", "https://semantic-workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-workflow", model: "semantic-workflow-model" };
  agent.registerModel(candidate);
  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const executor = new AutonomousWorkflowExecutor(agent, store);
  const task = "Help with an unfamiliar coding migration after a worker restart.";

  const first = await executor.start(task, {
    jobId: "workflow-semantic-1",
    candidates: [candidate],
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false, maxDomains: 1 },
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.equal(first.status, "paused");
  assert.equal(first.semantic_route_status, "completed");
  assert.equal(first.route.primary_domain, "coding");
  assert.equal(first.checkpoint.route_digest, first.route.route_digest);
  assert.equal(calls, 2, "one semantic route call plus one stage call");
  assert.match(JSON.stringify(bodies[0].messages), /bounded autonomous task router/);

  const resumed = await executor.resume("workflow-semantic-1", task, { candidates: [candidate], approveProviderCall: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.semantic_route_status, null, "resume uses the persisted domain without replaying the semantic classifier");
  assert.equal(calls, 6);
});

test("durable semantic routing remains review-only until its separate approval is granted", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { calls += 1; return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] }); },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-review", "https://semantic-review.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-review", model: "semantic-review-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const result = await executor.start("Classify this unfamiliar coding migration.", {
    jobId: "workflow-semantic-review-1",
    candidates: [candidate],
    semanticRouting: { enabled: true, approveProviderCall: false },
    approveProviderCall: true,
  });
  assert.equal(result.status, "route_review_required");
  assert.equal(result.semantic_route_status, "approval_required");
  assert.equal(result.checkpoint, null);
  assert.equal(calls, 0);
});

test("durable workflow strict semantic routing pauses at policy admission", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { calls += 1; return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] }); },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-workflow-policy", "https://semantic-workflow-policy.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-workflow-policy", model: "semantic-workflow-policy-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const result = await executor.start("Classify this unfamiliar coding migration.", {
    jobId: "workflow-semantic-policy-1",
    candidates: [candidate],
    semanticRouting: { enabled: true, approveProviderCall: true, domainPolicyMode: "strict" },
    approveProviderCall: true,
  });
  assert.equal(result.status, "policy_review_required");
  assert.equal(result.semantic_route_status, "policy_review_required");
  assert.equal(result.checkpoint, null);
  assert.equal(calls, 0, "policy admission must precede workflow classifier and stages");
});

test("durable workflows honor route handoffs and reject a changed persisted route identity", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("route-handoff", "https://route-handoff.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "route-handoff", model: "route-handoff-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const task = "Apply this coding migration through the durable workflow.";
  const route = await agent.route(task, { domain: "coding" });
  const first = await executor.start(task, { routeOverride: route, jobId: "workflow-route-handoff-1", candidates: [candidate], approveProviderCall: true, maxStages: 1 });
  assert.equal(first.status, "paused");
  assert.equal(first.route.route_digest, route.route_digest);
  assert.equal(first.checkpoint.route_digest, route.route_digest);
  const { route_digest: _routeDigest, ...routeDescriptor } = route;
  const changedDescriptor = { ...routeDescriptor, confidence: 0.75 };
  const changedRoute = { ...changedDescriptor, route_digest: await digestJson(changedDescriptor) };
  await assert.rejects(
    () => executor.resume("workflow-route-handoff-1", task, { routeOverride: changedRoute, candidates: [candidate], approveProviderCall: true }),
    /persisted route digest/,
  );
  assert.equal(calls, 1, "route identity mismatch must fail before provider stage dispatch");
});

test("workflow resume rehydrates caller-owned stage evidence by checkpoint digest", async () => {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow-rehydrate", "https://workflow-rehydrate.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "workflow-rehydrate", model: "workflow-rehydrate-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const task = "Resume this workflow with caller-owned evidence";
  const first = await executor.start(task, { domain: "coding", jobId: "workflow-rehydrate-1", candidates: [candidate], approveProviderCall: true, maxStages: 1 });
  const scopeOutput = first.stage_results[0].run.response.text;
  await assert.rejects(
    () => executor.resume("workflow-rehydrate-1", task, { candidates: [candidate], approveProviderCall: true, maxStages: 1, stageOutputs: { scope: "{}" } }),
    /digest/,
  );
  assert.equal(calls, 1, "invalid rehydrated evidence must fail before provider dispatch");
  const resumed = await executor.resume("workflow-rehydrate-1", task, {
    candidates: [candidate],
    approveProviderCall: true,
    maxStages: 1,
    stageOutputs: { scope: scopeOutput },
  });
  assert.equal(resumed.status, "paused");
  assert.equal(calls, 2);
  assert.match(JSON.stringify(bodies[1].messages), /evidence-scope/, "rehydrated evidence must reach the dependent stage");
});

test("accepted provider plan refinement is checkpoint-bound and required for replay", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("accepted-plan", "https://accepted-plan.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const acceptedModel = { ...model(), provider: "accepted-plan", model: "accepted-plan-model" };
  agent.registerModel(acceptedModel);
  const task = "Apply this accepted workflow ordering and verify the bounded result";
  const preview = await agent.blueprint(task, { domain: "coding" });
  assert.ok(preview.blueprint);
  const blueprint = preview.blueprint;
  const acceptedPlan = {
    schema: "bioprism-python-autonomous-plan-refinement/0.1",
    status: "completed",
    task_digest: blueprint.task_digest,
    base_plan_digest: await digestJson(blueprint.plan),
    workflow_digest: blueprint.workflow.workflow_digest,
    priority_stage_ids: blueprint.workflow.stages.map((stage) => stage.id),
    focus_stage_ids: [blueprint.workflow.stages[0].id],
    review_required: false,
    confidence: 0.97,
    selected_model: { provider: acceptedModel.provider, model: acceptedModel.model },
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "stage_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_or_effects_authorized",
  };
  const acceptedPlanDigest = await digestJson(acceptedPlan);
  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const executor = new AutonomousWorkflowExecutor(agent, store);
  const first = await executor.start(task, {
    domain: "coding",
    jobId: "workflow-accepted-plan-1",
    candidates: [acceptedModel],
    approveProviderCall: true,
    maxStages: 1,
    acceptedPlanRefinement: acceptedPlan,
  });
  assert.equal(first.status, "paused");
  assert.equal(first.plan_refinement_digest, acceptedPlanDigest);
  assert.equal(first.checkpoint.plan_refinement_digest, acceptedPlanDigest);
  assert.equal(first.checkpoint.next_stage_id, acceptedPlan.priority_stage_ids[1]);
  assert.equal(calls, 1);

  await assert.rejects(
    () => executor.resume("workflow-accepted-plan-1", task, { candidates: [acceptedModel], approveProviderCall: true, maxStages: 32 }),
    /plan refinement/,
  );
  const changedPlan = { ...acceptedPlan, focus_stage_ids: [acceptedPlan.priority_stage_ids.at(-1)] };
  await assert.rejects(
    () => executor.resume("workflow-accepted-plan-1", task, { candidates: [acceptedModel], approveProviderCall: true, maxStages: 32, acceptedPlanRefinement: changedPlan }),
    /plan refinement/,
  );
  assert.equal(calls, 1, "plan identity mismatches must be rejected before dispatch");

  const resumed = await executor.resume("workflow-accepted-plan-1", task, { candidates: [acceptedModel], approveProviderCall: true, maxStages: 32, acceptedPlanRefinement: acceptedPlan });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.plan_refinement_digest, acceptedPlanDigest);
  assert.equal(resumed.completed_stage_count, 5);
  assert.equal(calls, 5);
});

test("workflow resume refuses a changed selection contract before provider dispatch", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const promptRegistry = builtinAutonomousPromptRegistry(["coding"]);
  const task = "Resume only with the same model-selection contract";
  const first = await executor.start(task, { domain: "coding", jobId: "workflow-contract-1", candidates: agent.models(), promptRegistry, approveProviderCall: true, maxStages: 1, maxCostPerMillionTokens: 10 });
  assert.equal(first.status, "paused");
  assert.equal(typeof first.checkpoint.execution_contract_digest, "string");
  await assert.rejects(
    () => executor.resume("workflow-contract-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32, maxCostPerMillionTokens: 1 }),
    /execution contract/,
  );
  promptRegistry.register(new AutonomousPromptTemplate({
    promptId: "builtin.coding.specialist",
    version: "1.0.1",
    domain: "coding",
    capabilities: ["implementation", "debugging", "testing"],
    stages: ["*"],
    templateDigest: "f".repeat(64),
    render: () => [{ role: "system", content: "replacement coding guidance" }, { role: "user", content: "bounded objective" }],
  }), { replace: true });
  await assert.rejects(
    () => executor.resume("workflow-contract-1", task, { candidates: agent.models(), promptRegistry, approveProviderCall: true, maxStages: 32, maxCostPerMillionTokens: 10 }),
    /execution contract/,
  );
  assert.equal(calls, 1, "contract mismatch must be rejected before the next stage dispatch");
});

test("legacy workflow checkpoints require explicit contract rebinding", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const source = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start("Migrate this legacy workflow checkpoint", { domain: "coding", jobId: "workflow-legacy-1", candidates: agent.models(), approveProviderCall: true, maxStages: 1 });
  const { execution_contract_digest: _contract, checkpoint_digest: _digest, ...legacyDescriptor } = source.checkpoint;
  const legacyCheckpoint = { ...legacyDescriptor, checkpoint_digest: await digestJson(legacyDescriptor) };
  let checkpoint = structuredClone(legacyCheckpoint);
  let eventRows = [];
  let previousEventDigest = null;
  for (const event of source.events) {
    const { event_digest: _eventDigest, ...eventDescriptor } = event;
    const descriptor = { ...eventDescriptor, checkpoint_digest: checkpoint.checkpoint_digest, previous_event_digest: previousEventDigest };
    const legacyEvent = { ...descriptor, event_digest: await digestJson(descriptor) };
    eventRows.push(legacyEvent);
    previousEventDigest = legacyEvent.event_digest;
  }
  const legacyStore = {
    load: () => structuredClone(checkpoint),
    save: (value) => { checkpoint = structuredClone(value); },
    appendEvent: (value) => { eventRows.push(structuredClone(value)); },
    events: (_jobId, after = 0, limit = 256) => eventRows.filter((event) => event.sequence > after).slice(0, limit).map((event) => structuredClone(event)),
  };
  const executor = new AutonomousWorkflowExecutor(agent, legacyStore);
  await assert.rejects(
    () => executor.resume("workflow-legacy-1", "Migrate this legacy workflow checkpoint", { candidates: agent.models(), approveProviderCall: true }),
    /predates execution-contract binding/,
  );
  const resumed = await executor.resume("workflow-legacy-1", "Migrate this legacy workflow checkpoint", { candidates: agent.models(), approveProviderCall: true, rebindLegacyExecutionContract: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(typeof resumed.checkpoint.execution_contract_digest, "string");
  assert.equal(resumed.checkpoint.generation > source.checkpoint.generation, true);
  assert.equal(calls, 5);
});

test("workflow stages preserve structured output and selection constraints", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow-structured", "https://workflow-structured.test", { requiresCredential: false, structuredOutputMode: "json_object" }));
  const agent = new AutonomousAgent(llm);
  const structuredModel = { ...model(), provider: "workflow-structured", model: "workflow-structured-model", capabilities: [...model().capabilities] };
  agent.registerModel(structuredModel);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const result = await executor.start("Implement this structured workflow.", {
    domain: "coding",
    jobId: "workflow-structured-1",
    candidates: [structuredModel],
    approveProviderCall: true,
    maxStages: 1,
    maxCostPerMillionTokens: 10,
    maxLatencyMs: 50,
    minQuality: 0.9,
    requireJson: true,
    responseSchema,
  });
  assert.equal(result.status, "paused");
  assert.equal(result.stage_results[0].run.response.structured.stage_id, "scope");
  assert.equal(result.stage_results[0].run.response.structured.status, "completed");
  assert.deepEqual(result.stage_results[0].validation_errors, []);
  assert.deepEqual(bodies[0].response_format, { type: "json_object" });
  const structuredResumed = await executor.resume("workflow-structured-1", "Implement this structured workflow.", {
    candidates: [structuredModel],
    approveProviderCall: true,
    maxStages: 32,
    maxCostPerMillionTokens: 10,
    maxLatencyMs: 50,
    minQuality: 0.9,
  });
  assert.equal(structuredResumed.status, "completed");
  assert.equal(calls, 5, "workflow-owned stage schema must remain stable across resume");

  const refusedCalls = [];
  const refusedRuntime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async (_url, init) => { refusedCalls.push(init); return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] }); } });
  refusedRuntime.registerProvider(openaiCompatibleProvider("workflow-budget", "https://workflow-budget.test", { requiresCredential: false }));
  const refusedAgent = new AutonomousAgent(refusedRuntime);
  const refusedModel = { ...model(), provider: "workflow-budget", model: "workflow-budget-model", cost_per_million_tokens: 10 };
  refusedAgent.registerModel(refusedModel);
  const refused = await new AutonomousWorkflowExecutor(refusedAgent, new InMemoryAutonomousWorkflowCheckpointStore()).start("Budget must be enforced at the workflow stage.", { domain: "coding", jobId: "workflow-budget-1", candidates: [refusedModel], approveProviderCall: true, maxStages: 1, maxCostPerMillionTokens: 1 });
  assert.equal(refused.status, "failed");
  assert.equal(refusedCalls.length, 0, "workflow budget refusal must happen before provider dispatch");
});

test("workflow stages forward and persist the selection confidence floor", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload()) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow-confidence-a", "https://workflow-confidence-a.test", { requiresCredential: false }));
  llm.registerProvider(openaiCompatibleProvider("workflow-confidence-b", "https://workflow-confidence-b.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidateA = { ...model(), provider: "workflow-confidence-a", model: "same-prior-model" };
  const candidateB = { ...model(), provider: "workflow-confidence-b", model: "same-prior-model" };
  agent.registerModel(candidateA);
  agent.registerModel(candidateB);
  const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start("Do not dispatch an ambiguous workflow.", {
    domain: "coding",
    jobId: "workflow-confidence-floor-1",
    candidates: [candidateA, candidateB],
    approveProviderCall: true,
    maxStages: 1,
    minSelectionConfidence: 0.1,
  });
  assert.equal(result.status, "failed");
  assert.equal(calls, 0, "selection confidence abstention must happen before provider dispatch");
  assert.equal(result.checkpoint.stage_outcomes[0].error_class, "ProviderRuntimeError");
  assert.equal(result.checkpoint.stage_outcomes[0].error_code, "provider_error");
});

test("workflow checkpoint snapshots restore a paused job without admitting payloads", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, `stage-${calls}`)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const task = "Persist and resume this bounded workflow";
  const sourceStore = new InMemoryAutonomousWorkflowCheckpointStore();
  const sourceExecutor = new AutonomousWorkflowExecutor(agent, sourceStore);
  const first = await sourceExecutor.start(task, { domain: "coding", jobId: "workflow-snapshot-1", candidates: agent.models(), approveProviderCall: true, maxStages: 1 });
  assert.equal(first.status, "paused");
  const snapshot = await sourceStore.snapshot();
  assert.equal(snapshot.checkpoints.length, 1);
  assert.equal(snapshot.event_rows.length, 1);
  assert.doesNotMatch(JSON.stringify(snapshot), /Persist and resume this bounded workflow/);

  let durableSnapshot = null;
  const persistence = new AutonomousWorkflowPersistenceCoordinator(sourceStore, {
    read: () => durableSnapshot,
    write: (value) => { durableSnapshot = structuredClone(value); },
  });
  await persistence.flush();
  assert.equal(durableSnapshot.snapshot_digest, snapshot.snapshot_digest);

  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const actual = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (actual !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const transactional = new TransactionalJsonAutonomousWorkflowSnapshotPersistence(textStore);
  const transactionalWriter = new AutonomousWorkflowPersistenceCoordinator(sourceStore, transactional);
  await transactionalWriter.restore();
  await transactionalWriter.flush();
  assert.equal(encoded, JSON.stringify(JSON.parse(encoded)));
  const transactionalReaderStore = new InMemoryAutonomousWorkflowCheckpointStore();
  const transactionalReader = new AutonomousWorkflowPersistenceCoordinator(transactionalReaderStore, transactional);
  await transactionalReader.restore();
  assert.equal((await transactionalReaderStore.verifyIntegrity()).verified, true);
  const stale = new AutonomousWorkflowPersistenceCoordinator(new InMemoryAutonomousWorkflowCheckpointStore(), transactional);
  await assert.rejects(stale.flush(), /compare-and-swap conflict/);
  encoded = ` ${encoded}`;
  await assert.rejects(() => transactional.read(), /not canonical/);

  const tampered = structuredClone(durableSnapshot);
  tampered.event_rows[0].events[0].event_type = "completed";
  const restoredStore = new InMemoryAutonomousWorkflowCheckpointStore();
  await assert.rejects(restoredStore.restore(tampered), /snapshot digest does not match/);
  const restoredPersistence = new AutonomousWorkflowPersistenceCoordinator(restoredStore, { read: () => durableSnapshot, write: () => {} });
  await restoredPersistence.restore();
  assert.equal((await restoredStore.verifyIntegrity()).verified, true);
  await assert.rejects(restoredStore.appendEvent({ ...durableSnapshot.event_rows[0].events[0], prompt: "private payload" }), /unsupported fields/);

  const resumed = await new AutonomousWorkflowExecutor(agent, restoredStore).resume("workflow-snapshot-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.completed_stage_count, 5);
  assert.equal(calls, 5);
});

test("workflow stages share the caller execution admission boundary", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const journal = new InMemoryAutonomousExecutionJournal();
  const execution = await AutonomousExecutionController.create({
    executionId: "workflow-execution-boundary-1",
    domain: "coding",
    capability: "coding_delivery",
    riskClass: "engineering_change",
    policy: { max_steps: 8, max_provider_calls: 2, max_provider_failovers: 1, max_cost_units: 8 },
    journal,
  });
  const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start("Account this workflow stage", {
    domain: "coding",
    jobId: "workflow-execution-boundary-job",
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 1,
    execution,
  });
  assert.equal(result.status, "paused");
  assert.equal(execution.state.provider_calls, 1);
  assert.equal((await journal.events({ executionId: "workflow-execution-boundary-1" })).filter((row) => row.event.kind === "provider_call").length, 2);
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
    assert.equal(result.execution_receipt.schema, AUTONOMOUS_WORKFLOW_EXECUTION_RECEIPT_SCHEMA, profile.domain);
    assert.equal(result.execution_receipt.next_action, "approve_provider_call", profile.domain);
    assert.equal(result.execution_receipt.progress, 0, profile.domain);
    assert.equal(result.execution_receipt.safe_to_continue, false, profile.domain);
    await validateAutonomousWorkflowExecutionReceipt(result.execution_receipt);
  }
});

test("workflow executor runs every built-in single-domain workflow through the stage contract", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("all-domain-workflows", "https://all-domain-workflows.test", { requiresCredential: false }));
  const profiles = (await builtinAutonomousDomainProfiles()).filter((profile) => profile.domain !== "cross_domain");
  const capabilities = new Set(["reasoning", "structured_output"]);
  for (const profile of profiles) {
    for (const capability of profile.required_model_capabilities) capabilities.add(capability);
    for (const stage of profile.workflow.stages) for (const capability of stage.required_capabilities) capabilities.add(capability);
  }
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "all-domain-workflows", model: "all-domain-workflows-model", capabilities: [...capabilities] });
  const promptRegistry = builtinAutonomousPromptRegistry();
  for (const profile of profiles) {
    const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start(`Run a bounded ${profile.domain} workflow`, {
      domain: profile.domain,
      jobId: `all-domain-workflow-${profile.domain}`,
      candidates: agent.models(),
      promptRegistry,
      approveProviderCall: true,
      maxStages: 32,
    });
    assert.equal(result.status, "completed", profile.domain);
    assert.equal(result.completed_stage_count, profile.workflow.stages.length, profile.domain);
    assert.equal(result.stage_results.length, profile.workflow.stages.length, profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.declared_status === "completed" && stage.validation_errors.length === 0), profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.response_evaluation?.domain === profile.domain && stage.response_evaluation?.stage_id === stage.stage.id), profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.run?.prompt?.mode === "registry_selection" && stage.run.prompt.stage === stage.stage.id), profile.domain);
    assert.ok(result.checkpoint.stage_outcomes.filter((outcome) => outcome.status === "completed").every((outcome) => outcome.response_evaluation?.domain === profile.domain), profile.domain);
    assert.equal(result.response_learning_episode_ids.length, 0, "learning is disabled for this execution");
    assert.equal(result.execution_receipt.next_action, "complete", profile.domain);
    assert.equal(result.execution_receipt.completed_stage_ids.length, profile.workflow.stages.length, profile.domain);
    assert.equal(result.execution_receipt.incomplete_stage_ids.length, 0, profile.domain);
    assert.equal(result.execution_receipt.progress, 1, profile.domain);
    assert.equal(result.execution_receipt.safe_to_continue, false, profile.domain);
    assert.equal(Object.hasOwn(result.execution_receipt, "response"), false, profile.domain);
    await validateAutonomousWorkflowExecutionReceipt(result.execution_receipt);
    if (profile.domain === "coding") {
      await assert.rejects(
        () => validateAutonomousWorkflowExecutionReceipt({ ...result.execution_receipt, next_action: "retry_stage" }),
        /inconsistent|digest/,
      );
    }
  }
});

test("workflow executor forwards reviewed stage identity into live adapter dispatch", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      if (calls === 1) {
        return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "stage-tool-1", type: "function", function: { name: "conformance_run", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init, "scope")) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("stage-bound", "https://stage-bound.test", { requiresCredential: false }));
  const catalogueDefinition = { name: "conformance_run", description: "Run bounded conformance checks", inputSchema: { type: "object", additionalProperties: false } };
  const providerTool = { name: "conformance_run", description: "Run bounded conformance checks", parameters: { type: "object", additionalProperties: false } };
  const agent = new AutonomousAgent(llm, {
    toolCatalogue: await ToolCatalogue.fromDefinitions([catalogueDefinition]),
    toolExecutor: async () => ({ checked: true }),
  });
  agent.registerModel({ ...model(), provider: "stage-bound", model: "stage-bound-model", capabilities: [...model().capabilities, "review"] });
  const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start("Inspect this coding workflow with a reviewed adapter", {
    domain: "coding",
    jobId: "workflow-stage-bound-1",
    candidates: agent.models(),
    tools: [providerTool],
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.equal(result.status, "paused");
  assert.equal(result.stage_results[0].declared_status, "completed");
  const receipt = agent.toolExecutionEvidence().at(-1);
  assert.equal(receipt.domain, "coding");
  assert.equal(receipt.workflow_id, result.blueprint.workflow.workflow_id);
  assert.equal(receipt.workflow_digest, result.blueprint.workflow.workflow_digest);
  assert.equal(receipt.stage_id, "scope");
  assert.equal(receipt.status, "executed");
  assert.equal(receipt.required_evidence_outputs.includes("acceptance_criteria"), true);
  assert.equal(Object.prototype.hasOwnProperty.call(receipt, "arguments"), false);
});

test("workflow executor fails closed when a provider reports a blocked stage", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      const payload = workflowStagePayload(init);
      const response = calls === 1 ? { ...payload, status: "blocked", uncertainty: ["required evidence is unavailable"] } : payload;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(response) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("blocked-stage", "https://blocked-stage.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "blocked-stage", model: "blocked-stage-model" });
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const result = await executor.start("Do not advance without evidence", {
    domain: "coding",
    jobId: "workflow-blocked-stage-1",
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 32,
  });
  assert.equal(result.status, "stage_blocked");
  assert.equal(calls, 1);
  assert.equal(result.completed_stage_count, 0);
  assert.equal(result.stage_results[0].declared_status, "blocked");
  assert.deepEqual(result.stage_results[0].validation_errors, []);
  assert.equal(result.checkpoint.stage_outcomes.at(-1).error_class, "stage_blocked");
  assert.equal(result.checkpoint.next_stage_id, "scope");

  const held = await executor.resume("workflow-blocked-stage-1", "Do not advance without evidence", {
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 32,
  });
  assert.equal(held.status, "stage_blocked");
  assert.equal(calls, 1, "blocked stages must not replay without an explicit retry decision");
  await assert.rejects(
    () => executor.resume("workflow-blocked-stage-1", "Do not advance without evidence", { candidates: agent.models(), approveProviderCall: true, retryBlocked: "yes" }),
    /retryBlocked/,
  );

  const retried = await executor.resume("workflow-blocked-stage-1", "Do not advance without evidence", {
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 32,
    retryBlocked: true,
  });
  assert.equal(retried.status, "completed");
  assert.equal(retried.completed_stage_count, 5);
  assert.equal(calls, 6, "an explicit retry may redispatch the blocked stage and continue the DAG");
  assert.ok(retried.checkpoint.generation > result.checkpoint.generation);
});

test("workflow executor preserves proposed and not-attempted stage decisions", async () => {
  let declaredStatus = "proposed";
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ ...workflowStagePayload(init), status: declaredStatus }) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("noncompleted-stage", "https://noncompleted-stage.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "noncompleted-stage", model: "noncompleted-stage-model" });
  for (const [status, expected] of [["proposed", "stage_proposed"], ["not_attempted", "stage_not_attempted"]]) {
    declaredStatus = status;
    const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start(`Preserve ${status} workflow state`, {
      domain: "coding",
      jobId: `workflow-${status}-stage-1`,
      candidates: agent.models(),
      approveProviderCall: true,
      maxStages: 32,
    });
    assert.equal(result.status, expected);
    assert.equal(result.stage_results[0].declared_status, status);
    assert.equal(result.checkpoint.stage_outcomes.at(-1).error_class, expected);
  }
});

test("accepted plan identity is validated for every single-domain workflow profile", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("provider must not be called before approval"); } });
  llm.registerProvider(openaiCompatibleProvider("all-domain-plans", "https://all-domain-plans.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const allCapabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "neuroscience", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];
  const registered = { ...model(), provider: "all-domain-plans", model: "all-domain-plans-model", capabilities: allCapabilities };
  agent.registerModel(registered);
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles.filter((candidate) => candidate.domain !== "cross_domain")) {
    const preview = await agent.blueprint(`Review a bounded ${profile.domain} workflow.`, { domain: profile.domain });
    assert.ok(preview.blueprint, profile.domain);
    const blueprint = preview.blueprint;
    const refinement = {
      schema: "bioprism-python-autonomous-plan-refinement/0.1",
      status: "completed",
      task_digest: blueprint.task_digest,
      base_plan_digest: await digestJson(blueprint.plan),
      workflow_digest: blueprint.workflow.workflow_digest,
      priority_stage_ids: blueprint.workflow.stages.map((stage) => stage.id),
      focus_stage_ids: [blueprint.workflow.stages[0].id],
      review_required: false,
      confidence: 1,
      selected_model: null,
      selection_digest: null,
      planner_prompt_digest: null,
      planner_plan_digest: null,
      outcome_digest: null,
      retention: "stage_ids_and_digests_only; planner_transcript_not_retained",
      authorization: "plan_proposal_only; no_tools_or_effects_authorized",
    };
    const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start(`Review a bounded ${profile.domain} workflow.`, {
      domain: profile.domain,
      jobId: `all-domain-accepted-${profile.domain}`,
      candidates: [registered],
      acceptedPlanRefinement: refinement,
    });
    assert.equal(result.status, "approval_required", profile.domain);
    assert.equal(result.plan_refinement_digest, await digestJson(refinement), profile.domain);
    assert.equal(result.checkpoint.domain, profile.domain);
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
    async brainJobClaim(args) {
      seen.push({ operation: "claim", args });
      serverState = "leased";
      job.attempts += 1;
      return projection({ operation: "claim", idempotent: false, job: { ...job, state: serverState, attempts: job.attempts, lease_owner: args.worker_id, lease_expires_ns: 1 } });
    },
    async brainJobClaimNext(args) {
      seen.push({ operation: "claim_next", args });
      return projection({ operation: "claim_next", claimed: true, idempotent: false, job: { ...job, state: "leased", lease_owner: args.worker_id, lease_expires_ns: 1 }, event: null });
    },
    async brainJobRenew(args) {
      seen.push({ operation: "renew", args });
      return projection({ operation: "renew", idempotent: false, job: { ...job, state: serverState, lease_owner: args.worker_id, lease_expires_ns: 2 } });
    },
    async brainJobCheckpoint(args) {
      seen.push({ operation: "checkpoint", args });
      serverState = args.waiting_for_approval ? "waiting_approval" : "running";
      job.side_effect_boundary = args.side_effect_boundary;
      return projection({ operation: "checkpoint", idempotent: false, job: { ...job, state: serverState, side_effect_boundary: args.side_effect_boundary, checkpoint_digest: args.checkpoint_digest } });
    },
    async brainJobComplete(args) {
      seen.push({ operation: "complete", args });
      serverState = "succeeded";
      return projection({ operation: "complete", idempotent: false, job: { ...job, state: serverState, result_digest: args.result_digest, lease_owner: null, lease_expires_ns: null } });
    },
    async brainJobFail(args) {
      seen.push({ operation: "fail", args });
      serverState = "reconciliation_required";
      return projection({ operation: "fail", idempotent: false, job: { ...job, state: serverState, lease_owner: null, lease_expires_ns: null } });
    },
    async brainJobCancel(args) {
      seen.push({ operation: "cancel", args });
      return projection({ operation: "cancel", cancelled: true, reconciliation_required: false, idempotent: false, job: { ...job, state: "cancelled", lease_owner: null, lease_expires_ns: null }, event: null });
    },
  };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] });
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
  const claimedNext = await controller.claimNext();
  assert.equal(claimedNext.operation, "claim_next");
  const cancelled = await controller.cancel("server-job-1", "operator stop");
  assert.equal(cancelled.cancelled, true);
  assert.equal(seen[0].args.spec_digest, submitted.spec_digest);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].args, "task"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].args, "prompt"), false);
  await controller.approval("server-job-1", "request", { reason: "operator review" });
  const blocked = await controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true });
  assert.equal(blocked.local.status, "approval_required");
  assert.equal(blocked.local.execution_receipt.next_action, "approve_provider_call");
  await validateAutonomousWorkflowExecutionReceipt(blocked.local.execution_receipt);
  assert.equal(calls, 0);
  await controller.approval("server-job-1", "approve", { authorizationDigest: "e".repeat(64) });
  const executed = await controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 1 });
  assert.equal(executed.local.status, "paused");
  assert.equal(executed.local.completed_stage_count, 1);
  assert.equal(executed.local.execution_receipt.next_action, "continue_workflow");
  await validateAutonomousWorkflowExecutionReceipt(executed.local.execution_receipt);
  assert.equal(calls, 1);
  assert.equal(executed.job.state, "running");
  assert.ok(seen.some((row) => row.operation === "claim"));
  assert.ok(seen.some((row) => row.operation === "renew"));
  assert.ok(seen.filter((row) => row.operation === "checkpoint").length >= 2);
  assert.ok(seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "prompt")));

  serverState = "queued";
  job.domain = "not-a-built-in-domain";
  await assert.rejects(
    () => controller.execute("server-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 1 }),
    /unsupported autonomous domain/,
  );
});

test("remote durable worker claims, rehydrates, verifies, and settles a private all-domain workflow", async () => {
  let providerCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      providerCalls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("remote-worker", "https://remote-worker.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "remote-worker", model: "remote-worker-model" });
  const fixture = remoteJobFixture();
  const controller = new AutonomousDurableJobController(agent, fixture.api, new InMemoryAutonomousWorkflowCheckpointStore(), "remote-worker-1");
  const task = "Review a bounded coding workflow through the remote queue";
  const submitted = await controller.submit(task, { idempotencyKey: "remote-worker-request", domain: "coding", candidates: agent.models() });
  const worker = new AutonomousDurableJobWorker(controller, ({ job }) => {
    assert.equal(job.spec, "not_returned");
    return { task, options: { candidates: agent.models(), approveProviderCall: true, maxStages: 1 } };
  });
  const run = await worker.runOnce();
  assert.equal(run.status, "paused");
  assert.equal(run.job_id, submitted.job.job_id);
  assert.equal(run.execution.local.completed_stage_count, 1);
  assert.equal(providerCalls, 1);
  assert.ok(fixture.seen.some((row) => row.operation === "claim_next"));
  assert.ok(fixture.seen.some((row) => row.operation === "checkpoint" && row.args.side_effect_boundary === "unknown"));
  assert.equal(await worker.runOnce(), null, "a running job is not duplicated by an empty dequeue");
  assert.ok(fixture.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "task")));
  assert.ok(fixture.seen.every((row) => !Object.prototype.hasOwnProperty.call(row.args, "prompt")));
  assert.ok(JSON.stringify(run).includes("never_returned"));
});

test("remote durable worker fails closed on private task/spec drift before provider dispatch", async () => {
  let providerCalls = 0;
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { providerCalls += 1; throw new Error("provider must not be called"); } });
  llm.registerProvider(openaiCompatibleProvider("remote-drift", "https://remote-drift.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "remote-drift", model: "remote-drift-model" });
  const fixture = remoteJobFixture();
  const controller = new AutonomousDurableJobController(agent, fixture.api, new InMemoryAutonomousWorkflowCheckpointStore(), "remote-drift-worker");
  const original = "Original task bound to the submitted durable job";
  await controller.submit(original, { idempotencyKey: "remote-drift-request", domain: "coding", candidates: agent.models() });
  const worker = new AutonomousDurableJobWorker(controller, () => ({ task: "Tampered private task must never dispatch", options: { candidates: agent.models(), approveProviderCall: true } }));
  const run = await worker.runOnce();
  assert.equal(run.status, "failed");
  assert.equal(run.error_class, "ProviderRuntimeError");
  assert.equal(run.job.state, "failed");
  assert.equal(providerCalls, 0);
  assert.equal(fixture.seen.some((row) => row.operation === "checkpoint"), false);
  assert.ok(fixture.seen.some((row) => row.operation === "fail"));
  assert.equal(JSON.stringify(run).includes("Tampered private task"), false);
  assert.equal(JSON.stringify(fixture.seen).includes("Tampered private task"), false);
});

test("remote durable worker preserves retryable preflight failures as queued work", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("provider must not be called"); } });
  llm.registerProvider(openaiCompatibleProvider("remote-retry", "https://remote-retry.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "remote-retry", model: "remote-retry-model" });
  const fixture = remoteJobFixture();
  const controller = new AutonomousDurableJobController(agent, fixture.api, new InMemoryAutonomousWorkflowCheckpointStore(), "remote-retry-worker");
  const task = "Retry this bounded remote preflight";
  await controller.submit(task, { idempotencyKey: "remote-retry-request", domain: "coding", candidates: agent.models() });
  const worker = new AutonomousDurableJobWorker(
    controller,
    () => { throw new ProviderRuntimeError("temporary resolver outage", { retryable: true, code: "transport" }); },
  );
  const run = await worker.runOnce();
  assert.equal(run.status, "retry_scheduled");
  assert.equal(run.retryable, true);
  assert.equal(run.job.state, "queued");
  assert.equal(fixture.seen.at(-1).args.retryable, true);
});

test("remote durable worker rejects resolver attempts to override the leased job identity", async () => {
  let providerCalls = 0;
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { providerCalls += 1; throw new Error("provider must not be called"); } });
  llm.registerProvider(openaiCompatibleProvider("remote-job-id", "https://remote-job-id.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "remote-job-id", model: "remote-job-id-model" });
  const fixture = remoteJobFixture();
  const controller = new AutonomousDurableJobController(agent, fixture.api, new InMemoryAutonomousWorkflowCheckpointStore(), "remote-job-id-worker");
  const task = "Reject a resolver that attempts to move execution to another durable job";
  await controller.submit(task, { idempotencyKey: "remote-job-id-request", domain: "coding", candidates: agent.models() });
  const worker = new AutonomousDurableJobWorker(controller, () => ({
    task,
    options: { jobId: "attacker-selected-job", candidates: agent.models(), approveProviderCall: true },
  }));
  const run = await worker.runOnce();
  assert.equal(run.status, "failed");
  assert.equal(run.error_class, "ArgumentError");
  assert.equal(run.job.state, "failed");
  assert.equal(providerCalls, 0);
  assert.ok(fixture.seen.some((row) => row.operation === "fail"));
  assert.equal(JSON.stringify(fixture.seen).includes("attacker-selected-job"), false);
});
