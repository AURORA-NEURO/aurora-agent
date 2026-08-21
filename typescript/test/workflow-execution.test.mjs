import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousDurableJobController,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  AutonomousWorkflowPersistenceCoordinator,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  digestJson,
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
  assert.equal(JSON.stringify(first.checkpoint).includes(task), false);
  assert.equal(calls, 2);
  assert.match(JSON.stringify(bodies[1].messages), /evidence-scope/, "dependent stages receive bounded prior-stage evidence");

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
  const task = "Resume only with the same model-selection contract";
  const first = await executor.start(task, { domain: "coding", jobId: "workflow-contract-1", candidates: agent.models(), approveProviderCall: true, maxStages: 1, maxCostPerMillionTokens: 10 });
  assert.equal(first.status, "paused");
  assert.equal(typeof first.checkpoint.execution_contract_digest, "string");
  await assert.rejects(
    () => executor.resume("workflow-contract-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32, maxCostPerMillionTokens: 1 }),
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
  for (const profile of profiles) {
    const result = await new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore()).start(`Run a bounded ${profile.domain} workflow`, {
      domain: profile.domain,
      jobId: `all-domain-workflow-${profile.domain}`,
      candidates: agent.models(),
      approveProviderCall: true,
      maxStages: 32,
    });
    assert.equal(result.status, "completed", profile.domain);
    assert.equal(result.completed_stage_count, profile.workflow.stages.length, profile.domain);
    assert.equal(result.stage_results.length, profile.workflow.stages.length, profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.declared_status === "completed" && stage.validation_errors.length === 0), profile.domain);
  }
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
