import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousCrossDomainExecutor,
  AutonomousCrossDomainPersistenceCoordinator,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  InMemoryAutonomousCrossDomainCheckpointStore,
  TransactionalJsonAutonomousCrossDomainSnapshotPersistence,
  CredentialStore,
  LLMRuntime,
  buildAutonomousDomainResponseContract,
  builtinAutonomousDomainProfiles,
  digestJson,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(text) {
  return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: text }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
}

function structuredResponse(contract) {
  return {
    schema: "bioprism-typescript-autonomous-domain-response/0.1",
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: "complete",
    answer: `Durable bounded answer for ${contract.domain}.`,
    observations: ["The reviewed durable fixture was inspected."],
    inferences: ["The result is limited to the supplied contract."],
    uncertainty: ["External-world truth remains caller-owned."],
    evidence_gaps: ["No live source was contacted in this test."],
    next_actions: ["Review the declared evidence before action."],
    stages: contract.stage_ids.map((stage_id) => ({ stage_id, status: "complete", evidence: [`evidence:${stage_id}`], findings: [`finding:${stage_id}`], uncertainty: [`uncertainty:${stage_id}`], open_questions: [] })),
    domain_details: Object.fromEntries(contract.domain_fields.map((field) => [field, [`detail:${field}`]])),
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
}

function model() {
  return {
    provider: "durable-cross",
    model: "durable-cross-model",
    capabilities: ["reasoning", "coordination", "biomedical", "neuroscience", "science", "structured_output"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function makeAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse(calls === 3 ? "durable synthesis" : `durable child ${calls}`);
    },
  });
  llm.registerProvider(openaiCompatibleProvider("durable-cross", "https://durable-cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  return { agent, calls: () => calls };
}

const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
const subtasks = [
  { id: "bio", domain: "biomedical", task: "Review the biomedical evidence and safety boundary." },
  { id: "neuro", domain: "neuroscience", task: "Analyze the EEG neuroscience design and signal limits." },
];

test("durable cross-domain execution composes semantic routing with route-bound restart recovery", async () => {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      let body = {};
      try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
      bodies.push(body);
      const isRouter = JSON.stringify(body.messages ?? []).includes("bounded autonomous task router");
      const content = isRouter
        ? JSON.stringify({
          selected_domains: [
            { domain: "biomedical", score: 0.98, rationale: "The task contains biomedical evidence and safety review." },
            { domain: "neuroscience", score: 0.96, rationale: "The task contains EEG and neuroscience analysis." },
          ],
          confidence: 0.98,
          abstain: false,
          abstain_reason: null,
        })
        : calls === 4 ? "semantic durable synthesis" : `semantic durable child ${calls}`;
      return jsonResponse(content);
    },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-durable-cross", "https://semantic-durable-cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-durable-cross", model: "semantic-durable-cross-model" };
  agent.registerModel(candidate);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  const first = await executor.start(task, {
    jobId: "semantic-durable-cross-1",
    candidates: [candidate],
    subtasks,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true, maxDomains: 2 },
    approveProviderCall: true,
    maxSteps: 2,
  });
  assert.equal(first.status, "paused");
  assert.equal(first.semantic_route_status, "completed");
  assert.equal(first.route.route_digest, first.checkpoint.route_digest);
  assert.equal(first.completed_children, 2);
  assert.equal(calls, 3, "one semantic route call plus two child calls");
  assert.match(JSON.stringify(bodies[0].messages), /bounded autonomous task router/);

  const childResults = new Map(first.step_results.map((step) => [step.item_id, step.run]));
  const changedDescriptor = { ...(({ route_digest: _routeDigest, ...descriptor }) => descriptor)(first.route), confidence: 0.75 };
  const changedRoute = { ...changedDescriptor, route_digest: await digestJson(changedDescriptor) };
  await assert.rejects(
    () => executor.resume("semantic-durable-cross-1", task, { routeOverride: changedRoute, candidates: [candidate], subtasks, approveProviderCall: true, maxSteps: 1, resolveChildResult: (id) => childResults.get(id) ?? null }),
    /checkpoint/,
  );
  assert.equal(calls, 3, "a changed route identity must fail before synthesis dispatch");

  const completed = await executor.resume("semantic-durable-cross-1", task, {
    routeOverride: first.route,
    candidates: [candidate],
    subtasks,
    approveProviderCall: true,
    maxSteps: 1,
    resolveChildResult: (id) => childResults.get(id) ?? null,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.semantic_route_status, null, "restart consumes the reviewed route handoff without replaying classification");
  assert.equal(completed.synthesis.response.text, "semantic durable synthesis");
  assert.equal(calls, 4);
});

test("durable cross-domain semantic routing remains review-only until classifier approval", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { calls += 1; return jsonResponse("must not dispatch"); },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-cross-review", "https://semantic-cross-review.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-cross-review", model: "semantic-cross-review-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousCrossDomainExecutor(agent, new InMemoryAutonomousCrossDomainCheckpointStore());
  const result = await executor.start(task, {
    jobId: "semantic-cross-review-1",
    candidates: [candidate],
    subtasks,
    semanticRouting: { enabled: true, approveProviderCall: false },
    approveProviderCall: true,
  });
  assert.equal(result.status, "route_review_required");
  assert.equal(result.semantic_route_status, "approval_required");
  assert.equal(result.checkpoint, null);
  assert.equal(calls, 0);
});

test("durable cross-domain strict semantic routing pauses at policy admission", async () => {
  const { agent, calls } = makeAgent();
  const executor = new AutonomousCrossDomainExecutor(agent, new InMemoryAutonomousCrossDomainCheckpointStore());
  const result = await executor.start(task, {
    jobId: "semantic-cross-policy-1",
    candidates: [model()],
    subtasks,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true, domainPolicyMode: "strict" },
    approveProviderCall: true,
  });
  assert.equal(result.status, "policy_review_required");
  assert.equal(result.semantic_route_status, "policy_review_required");
  assert.equal(result.checkpoint, null);
  assert.equal(calls(), 0, "policy admission must precede classifier and fan-out dispatch");
});

test("durable cross-domain execution advances one bounded step and rehydrates caller-owned children", async () => {
  const { agent, calls } = makeAgent();
  const preview = await agent.blueprint(task, { subtasks });
  assert.ok(preview.cross_domain_blueprint);
  const blueprint = preview.cross_domain_blueprint;
  const acceptedPlan = {
    schema: "bioprism-python-autonomous-cross-domain-plan-refinement/0.1",
    status: "completed",
    task_digest: blueprint.task_digest,
    base_plan_digest: blueprint.plan_digest,
    priority_child_ids: [...blueprint.child_ids].reverse(),
    focus_child_ids: [blueprint.child_ids.at(-1)],
    review_required: false,
    confidence: 0.96,
    selected_model: { provider: "durable-cross", model: "durable-cross-model" },
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "child_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_or_effects_authorized",
  };
  const planDigest = await digestJson(acceptedPlan);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  const results = new Map();
  const options = { candidates: agent.models(), subtasks, acceptedCrossDomainPlanRefinement: acceptedPlan, approveProviderCall: true, maxSteps: 1, jobId: "durable-cross-1" };

  const first = await executor.start(task, options);
  assert.equal(first.status, "paused");
  assert.equal(first.completed_children, 1);
  assert.equal(first.plan_refinement_digest, planDigest);
  assert.equal(calls(), 1);
  results.set(first.step_results[0].item_id, first.step_results[0].run);
  assert.equal(JSON.stringify(first.checkpoint).includes("durable child 1"), false);
  assert.equal(JSON.stringify(first.checkpoint).includes(subtasks[0].task), false);

  const resolveChildResult = (childId) => results.get(childId) ?? null;
  const second = await executor.resume("durable-cross-1", task, { ...options, jobId: undefined, resolveChildResult });
  assert.equal(second.status, "paused");
  assert.equal(second.completed_children, 2);
  assert.equal(second.checkpoint.status, "synthesis_pending");
  results.set(second.step_results[0].item_id, second.step_results[0].run);
  assert.equal(calls(), 2);

  const completed = await executor.resume("durable-cross-1", task, { ...options, jobId: undefined, maxSteps: 1, resolveChildResult });
  assert.equal(completed.status, "completed");
  assert.equal(completed.synthesis.response.text, "durable synthesis");
  assert.equal(completed.checkpoint.status, "completed");
  assert.equal(completed.checkpoint.synthesis_result_digest.length, 64);
  assert.equal(calls(), 3);
  assert.ok(completed.events.some((event) => event.event_type === "synthesis_completed"));
  assert.ok(completed.events.every((event) => !JSON.stringify(event).includes("durable child")));

  const snapshot = await store.snapshot();
  const restored = new InMemoryAutonomousCrossDomainCheckpointStore();
  await restored.restore(snapshot);
  const integrity = await restored.verifyIntegrity();
  assert.equal(integrity.verified, true);
  assert.equal(integrity.jobs, 1);
  assert.ok(integrity.events >= 4);
});
test("durable cross-domain execution pauses before dispatch and refuses a tampered rehydrated child", async () => {
  const { agent, calls } = makeAgent();
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  await assert.rejects(executor.start(task, { candidates: agent.models(), subtasks, maxParallelChildren: 2 }), /sequential/);
  await assert.rejects(executor.start(task, { candidates: agent.models(), subtasks, allowPartial: true }), /does not synthesize partial/);
  await assert.rejects(executor.start(task, { candidates: agent.models(), subtasks, retryReconciliation: "yes" }), /must be a boolean/);
  const gated = await executor.start(task, { candidates: agent.models(), subtasks, approveProviderCall: false, jobId: "durable-cross-approval", maxSteps: 1 });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.checkpoint.status, "paused");
  assert.equal(calls(), 0);

  const first = await executor.resume("durable-cross-approval", task, { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 1 });
  assert.equal(first.completed_children, 1);
  assert.equal(calls(), 1);
  const childId = first.step_results[0].item_id;
  const tampered = { ...first.step_results[0].run, response: { ...first.step_results[0].run.response, text: "tampered" } };
  await assert.rejects(
    executor.resume("durable-cross-approval", task, { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 1, resolveChildResult: (id) => id === childId ? tampered : null }),
    /result digest does not match/,
  );
  assert.equal(calls(), 1, "digest refusal must happen before the next provider dispatch");
});

test("cross-domain snapshot JSON persistence is canonical, restart-safe, and CAS-fenced", async () => {
  const { agent } = makeAgent();
  const sourceStore = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, sourceStore);
  const first = await executor.start(task, { candidates: agent.models(), subtasks, approveProviderCall: false, jobId: "cross-domain-persistence", maxSteps: 1 });
  const snapshot = await sourceStore.snapshot();
  assert.equal(first.checkpoint.status, "paused");

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
  const persistence = new TransactionalJsonAutonomousCrossDomainSnapshotPersistence(textStore);
  const writer = new AutonomousCrossDomainPersistenceCoordinator(sourceStore, persistence);
  await writer.restore();
  const flushed = await writer.flush();
  assert.equal(flushed.snapshot_digest, snapshot.snapshot_digest);
  assert.equal(encoded, JSON.stringify(JSON.parse(encoded)));

  const restored = new InMemoryAutonomousCrossDomainCheckpointStore();
  const reader = new AutonomousCrossDomainPersistenceCoordinator(restored, persistence);
  const rehydrated = await reader.restore();
  assert.equal(rehydrated.snapshot_digest, snapshot.snapshot_digest);
  assert.equal((await restored.verifyIntegrity()).verified, true);

  const forgedCheckpoint = { ...snapshot.checkpoints[0], generation: 2, previous_checkpoint_digest: null };
  const { checkpoint_digest: _checkpointDigest, ...forgedCheckpointBody } = forgedCheckpoint;
  forgedCheckpoint.checkpoint_digest = await digestJson(forgedCheckpointBody);
  const forgedSnapshot = { ...snapshot, checkpoints: [forgedCheckpoint] };
  const { snapshot_digest: _snapshotDigest, ...forgedSnapshotBody } = forgedSnapshot;
  forgedSnapshot.snapshot_digest = await digestJson(forgedSnapshotBody);
  await assert.rejects(new InMemoryAutonomousCrossDomainCheckpointStore().restore(forgedSnapshot), /generation and predecessor/);

  const stale = new AutonomousCrossDomainPersistenceCoordinator(new InMemoryAutonomousCrossDomainCheckpointStore(), persistence);
  await assert.rejects(stale.flush(), /compare-and-swap conflict/);
  encoded = ` ${encoded}`;
  await assert.rejects(() => persistence.read(), /not canonical/);
});

test("durable cross-domain child reconciliation is quarantined until an explicit retry decision", async () => {
  const { agent, calls } = makeAgent();
  let runCalls = 0;
  const controlledAgent = {
    models: () => agent.models(),
    route: (...args) => agent.route(...args),
    blueprint: (...args) => agent.blueprint(...args),
    run: async (...args) => {
      runCalls += 1;
      if (runCalls === 1) return { status: "reconciliation_required", response: null };
      return agent.run(...args);
    },
  };
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(controlledAgent, store);
  const options = { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 1, jobId: "durable-cross-child-reconcile" };

  const first = await executor.start(task, options);
  assert.equal(first.status, "reconciliation_required");
  assert.equal(first.checkpoint.status, "reconciliation_required");
  assert.equal(runCalls, 1);
  assert.equal(calls(), 0);

  const held = await executor.resume("durable-cross-child-reconcile", task, { ...options, jobId: undefined });
  assert.equal(held.status, "reconciliation_required");
  assert.equal(held.checkpoint.generation, first.checkpoint.generation);
  assert.equal(runCalls, 1, "ordinary resume must not replay an uncertain child");
  assert.equal(calls(), 0);

  const retried = await executor.resume("durable-cross-child-reconcile", task, { ...options, jobId: undefined, retryReconciliation: true });
  assert.equal(retried.status, "paused");
  assert.equal(retried.checkpoint.status, "paused");
  assert.ok(retried.checkpoint.next_child_id);
  assert.ok(retried.checkpoint.generation > first.checkpoint.generation);
  assert.equal(runCalls, 2);
  assert.equal(calls(), 1);
  assert.ok(retried.events.some((event) => event.event_type === "reconciliation_retry_authorized"));
});

test("durable cross-domain synthesis reconciliation is quarantined before child rehydration", async () => {
  const { agent, calls } = makeAgent();
  let runCalls = 0;
  const controlledAgent = {
    models: () => agent.models(),
    route: (...args) => agent.route(...args),
    blueprint: (...args) => agent.blueprint(...args),
    run: async (...args) => {
      runCalls += 1;
      if (runCalls === 3) return { status: "reconciliation_required", response: null };
      return agent.run(...args);
    },
  };
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(controlledAgent, store);
  const options = { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 2, jobId: "durable-cross-synthesis-reconcile" };
  const first = await executor.start(task, options);
  const children = new Map(first.step_results.map((step) => [step.item_id, step.run]));
  assert.equal(first.completed_children, 2);
  assert.equal(calls(), 2);

  const resolveChildResult = (childId) => children.get(childId) ?? null;
  const blocked = await executor.resume("durable-cross-synthesis-reconcile", task, { ...options, jobId: undefined, maxSteps: 1, resolveChildResult });
  assert.equal(blocked.status, "reconciliation_required");
  assert.equal(blocked.checkpoint.status, "reconciliation_required");
  assert.equal(runCalls, 3);
  assert.equal(calls(), 2);

  let resolverCalls = 0;
  const held = await executor.resume("durable-cross-synthesis-reconcile", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    resolveChildResult: () => {
      resolverCalls += 1;
      throw new Error("reconciliation gate should run before child rehydration");
    },
  });
  assert.equal(held.status, "reconciliation_required");
  assert.equal(resolverCalls, 0);
  assert.equal(runCalls, 3);
  assert.equal(calls(), 2);

  const completed = await executor.resume("durable-cross-synthesis-reconcile", task, { ...options, jobId: undefined, maxSteps: 1, resolveChildResult, retryReconciliation: true });
  assert.equal(completed.status, "completed");
  assert.equal(completed.checkpoint.status, "completed");
  assert.equal(completed.synthesis.response.text, "durable synthesis");
  assert.equal(runCalls, 4);
  assert.equal(calls(), 3);
});

test("durable cross-domain structured-response learning survives restart and settles independently", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("durable-cross-structured", (request) => {
    const domain = request.responseSchema?.properties?.domain?.const;
    return { structured: structuredResponse(contracts.get(domain)) };
  }, { structuredOutputMode: "json_schema" });
  const candidate = { ...model(), provider: "durable-cross-structured", model: "durable-cross-structured-model" };
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate);
  const learning = new AutonomousLearningController(agent);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store, { learning });
  const options = { candidates: [candidate], subtasks, structuredDomainResponse: true, approveProviderCall: true, maxSteps: 2, jobId: "durable-cross-structured-1" };

  const first = await executor.start(task, options);
  assert.equal(first.status, "paused");
  assert.equal(first.completed_children, 2);
  assert.equal(first.learning_episode_ids.length, 2);
  assert.equal(first.response_learning_episode_ids.length, 2);
  assert.deepEqual(first.checkpoint.learning_episode_ids, first.learning_episode_ids);
  assert.deepEqual(first.checkpoint.response_learning_episode_ids, first.response_learning_episode_ids);
  const children = new Map(first.step_results.map((step) => [step.item_id, step.run]));

  const completed = await executor.resume("durable-cross-structured-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    resolveChildResult: (id) => children.get(id) ?? null,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.learning_episode_ids.length, 3);
  assert.equal(completed.response_learning_episode_ids.length, 3);
  assert.equal(completed.checkpoint.learning_episode_ids.length, 3);
  assert.equal(completed.checkpoint.response_learning_episode_ids.length, 3);
  assert.equal(completed.synthesis.response.structured.domain, "cross_domain");

  const rewards = Object.fromEntries(completed.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "durable-cross-task-reviewer", evaluator_version: "1", reward: 0.8, passed: true }]));
  const settled = await learning.settleCrossDomainExecution(completed, rewards, {
    trajectoryId: "durable-cross-structured-trajectory",
    resolveResult: (id, phase) => phase === "child" ? children.get(id) ?? null : completed.synthesis,
  });
  assert.equal(settled.trajectory.settlements.length, 3);
  assert.equal(settled.response_settlements.length, 3);
  assert.ok(settled.response_settlements.every((row) => row.assessment.evaluator_id.endsWith("response-integrity")));

  const tampered = structuredClone(completed);
  tampered.synthesis.response.structured.answer = "tampered after durable provider execution";
  await assert.rejects(
    () => learning.settleCrossDomainExecution(tampered, rewards, { trajectoryId: "durable-cross-structured-trajectory", resolveResult: (id, phase) => phase === "child" ? children.get(id) ?? null : tampered.synthesis }),
    /result digest does not match/,
  );
});

test("durable cross-domain execution cannot bypass structured response review after restart", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  llm.registerInMemoryProvider("durable-cross-gate", (request) => {
    const domain = request.responseSchema?.properties?.domain?.const;
    return { structured: structuredResponse(contracts.get(domain)) };
  });
  const candidate = { ...model(), provider: "durable-cross-gate", model: "durable-cross-gate-model" };
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  const options = {
    candidates: [candidate],
    subtasks,
    structuredDomainResponse: true,
    requireResponseAlignment: true,
    approveProviderCall: true,
    maxSteps: 2,
    jobId: "durable-cross-response-gate-1",
  };

  const first = await executor.start(task, options);
  assert.equal(first.status, "paused");
  assert.equal(first.completed_children, 2);
  assert.equal(first.checkpoint.status, "synthesis_pending");
  const children = new Map(first.step_results.map((step) => [step.item_id, step.run]));

  const blocked = await executor.resume("durable-cross-response-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    resolveChildResult: (id) => children.get(id) ?? null,
  });
  assert.equal(blocked.status, "response_review_required");
  assert.equal(blocked.synthesis, null);
  assert.equal(blocked.response_assessment?.status, "needs_alignment_review");
  assert.ok(blocked.response_assessment?.gate_reasons.includes("pairwise_alignment_incomplete"));
  assert.equal(blocked.checkpoint.status, "response_review_required");
  assert.equal(blocked.checkpoint.response_assessment_digest, blocked.response_assessment.assessment_digest);
  assert.equal(blocked.events.some((event) => event.event_type === "response_review_required"), true);

  const rows = blocked.response_assessment.rows.filter((row) => row.role === "specialist");
  const alignment = {
    alignment_id: "bio-neuro-review",
    left_domain: rows[0].domain,
    right_domain: rows[1].domain,
    stance: "neutral",
    confidence: 1,
    topic_digest: await digestJson({ topic: "bounded EEG review" }),
    rationale_digest: null,
    left_response_digest: rows[0].response_digest,
    right_response_digest: rows[1].response_digest,
  };
  const completed = await executor.resume("durable-cross-response-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    responseAlignments: [alignment],
    resolveChildResult: (id) => children.get(id) ?? null,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.checkpoint.status, "completed");
  assert.equal(completed.synthesis?.response?.structured?.domain, "cross_domain");
});

test("durable cross-domain execution re-admits synthesis responses and requires explicit retry", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  let synthesisAttempts = 0;
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  llm.registerInMemoryProvider("durable-post-synthesis", (request) => {
    const domain = request.responseSchema?.properties?.domain?.const;
    const response = structuredResponse(contracts.get(domain));
    if (domain === "cross_domain") {
      synthesisAttempts += 1;
      if (synthesisAttempts === 1) response.status = "blocked";
    }
    return { structured: response };
  });
  const candidate = { ...model(), provider: "durable-post-synthesis", model: "durable-post-synthesis-model" };
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  const options = {
    candidates: [candidate],
    subtasks,
    structuredDomainResponse: true,
    requireResponseAlignment: true,
    approveProviderCall: true,
    maxSteps: 2,
    jobId: "durable-post-synthesis-gate-1",
  };

  const first = await executor.start(task, options);
  assert.equal(first.status, "paused");
  const children = new Map(first.step_results.filter((step) => step.phase === "child").map((step) => [step.item_id, step.run]));
  const preReview = await executor.resume("durable-post-synthesis-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    resolveChildResult: (id) => children.get(id) ?? null,
  });
  assert.equal(preReview.status, "response_review_required");
  assert.equal(preReview.checkpoint.status, "response_review_required");
  const rows = preReview.response_assessment.rows.filter((row) => row.role === "specialist");
  const alignment = {
    alignment_id: "post-synthesis-bio-neuro-review",
    left_domain: rows[0].domain,
    right_domain: rows[1].domain,
    stance: "neutral",
    confidence: 1,
    topic_digest: await digestJson({ topic: "bounded EEG review" }),
    rationale_digest: null,
    left_response_digest: rows[0].response_digest,
    right_response_digest: rows[1].response_digest,
  };
  const blocked = await executor.resume("durable-post-synthesis-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    responseAlignments: [alignment],
    resolveChildResult: (id) => children.get(id) ?? null,
  });
  assert.equal(blocked.status, "synthesis_response_review_required");
  assert.equal(blocked.synthesis?.response?.structured?.status, "blocked");
  assert.notEqual(blocked.response_assessment?.status, "completed");
  assert.equal(blocked.checkpoint.status, "synthesis_response_review_required");
  assert.equal(blocked.checkpoint.synthesis_result_digest, await digestJson(blocked.synthesis));
  assert.equal(blocked.checkpoint.response_assessment_digest, blocked.response_assessment.assessment_digest);
  assert.equal(blocked.events.some((event) => event.event_type === "synthesis_response_review_required"), true);
  const tamperedCheckpoint = structuredClone(blocked.checkpoint);
  tamperedCheckpoint.response_assessment_digest = "0".repeat(64);
  await assert.rejects(() => store.save(tamperedCheckpoint), /checkpoint digest/);
  const rehydratedReview = await executor.resume("durable-post-synthesis-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    responseAlignments: [alignment],
    resolveChildResult: (id) => children.get(id) ?? null,
    resolveSynthesisResult: () => blocked.synthesis,
  });
  assert.equal(rehydratedReview.status, "synthesis_response_review_required");
  assert.equal(rehydratedReview.synthesis?.response?.structured?.status, "blocked");
  assert.equal(synthesisAttempts, 1);

  const completed = await executor.resume("durable-post-synthesis-gate-1", task, {
    ...options,
    jobId: undefined,
    maxSteps: 1,
    responseAlignments: [alignment],
    retrySynthesisAfterResponseReview: true,
    resolveChildResult: (id) => children.get(id) ?? null,
    resolveSynthesisResult: () => blocked.synthesis,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.checkpoint.status, "completed");
  assert.equal(completed.synthesis?.response?.structured?.status, "complete");
  assert.equal(synthesisAttempts, 2);
  assert.equal(completed.events.some((event) => event.event_type === "synthesis_response_retry_authorized"), true);
  assert.equal(completed.events.some((event) => event.event_type === "synthesis_completed"), true);
});
