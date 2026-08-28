import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainBatchJobController,
  AutonomousBrainBatchProtectedRehydrator,
  AutonomousBrainAutoBatchProtectedRehydrator,
  InMemoryAutonomousBrainBatchCheckpointStore,
  AutonomousBrainPlan,
  AutonomousCapabilityActivation,
  AutonomousCapabilityActivationStore,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  InMemoryAutonomousDecisionCycleStateStore,
  InMemoryAutonomousEpisodicMemory,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  InMemoryAutonomousConnectorReceiptJournal,
  InMemoryAutonomousRunTraceStore,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  LLMRuntime,
  createBuiltinAutonomousConnectorRuntime,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  protectedValueDigest,
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

function localRuntime(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline:${request.model}` };
  });
  return runtime;
}

function semanticRuntime(payloads, onRequest = () => {}) {
  let calls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("semantic HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    calls += 1;
    onRequest(request);
    const isRouter = request.messages.some((message) => String(message.content).includes("bounded autonomous task router"));
    if (isRouter) return { structured: payloads.shift() };
    return { output_text: "bounded facade execution" };
  });
  return { runtime, calls: () => calls };
}

test("brain facade exposes protected provider onboarding without retaining user credentials", async () => {
  let networkCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { networkCalls += 1; throw new Error("onboarding must not contact HTTP"); } });
  const agent = new AutonomousAgent(runtime);
  const brain = new AutonomousBrainFacade({ agent });
  const setup = brain.providerSetup;

  assert.equal(setup.runtime, runtime);
  setup.registerProvider("groq");
  const session = setup.startSession({ sessionId: "facade-onboarding", ttlMs: 60_000, clock: () => 100 });
  const before = setup.instructions("groq");
  assert.equal(before.provider_registered, true);
  assert.equal(before.ready, false);
  assert.equal(before.next_action, "collect_user_credential");

  const handle = setup.collectUserCredential(session, "groq", "offline-fixture-secret", { ttlMs: 30_000 });
  assert.equal(handle.provider, "groq");
  assert.equal(setup.instructions("groq").ready, true);
  const readiness = await brain.readiness();
  const provider = readiness.providers.find((row) => row.provider === "groq");
  assert.equal(provider.credential_ready, true);
  assert.equal(provider.credential.active_handles, 1);
  assert.doesNotMatch(JSON.stringify({ before, readiness, handle }), /offline-fixture-secret/);
  assert.equal(networkCalls, 0);

  session.close();
  assert.equal(runtime.credentials.status("groq").active_handles, 0);
  assert.equal(session.status().active, false);
  assert.throws(() => setup.collectUserCredential(session, "groq", "another-fixture-secret"), /closed or expired/);
});

test("brain facade owns provisioned direct, cycle, and adaptive execution wrappers", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const provisioning = { credentialProviders: ["offline"], approveProviderCall: true };

  const direct = await brain.executeWithProvisionedCredentials(
    { task: "produce a bounded coding review", domain: "coding" },
    provisioning,
  );
  assert.equal(direct.status, "completed");
  assert.equal(direct.result.run?.status, "completed");
  assert.equal(direct.result.run?.response.provider, "offline");
  assert.equal(direct.toJSON().result_metadata.serialized, false);

  const automatic = await brain.executeAutoWithProvisionedCredentials(
    { task: "automatically route a bounded coding review", domain: "coding" },
    provisioning,
  );
  assert.equal(automatic.status, "completed");
  assert.equal(automatic.result.automatic?.status, "completed");
  assert.equal(automatic.result.automatic?.result?.response.provider, "offline");

  const cycle = await brain.executeCycleWithProvisionedCredentials(
    { task: "close a bounded science review", domain: "science" },
    provisioning,
  );
  assert.ok(["completed", "children_completed"].includes(cycle.status));
  assert.ok(cycle.result.cycle);

  const adaptive = await brain.executeAdaptiveCycleWithProvisionedCredentials(
    { task: "close a bounded evaluation review", domain: "evaluation" },
    {
      ...provisioning,
      adaptive: {
        maxReplans: 0,
        evaluate: () => ({
          evaluator_id: "facade-provisioned-evaluator",
          evaluator_version: "1",
          reward: 0.8,
          passed: true,
          replan_requested: false,
        }),
      },
    },
  );
  assert.equal(adaptive.status, "completed");
  assert.equal(adaptive.result.adaptive.attempts.length, 1);
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
  assert.doesNotMatch(JSON.stringify(direct.toJSON()), /offline:offline-model/);

  const heldAdmission = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  await assert.rejects(
    () => brain.executeWithProvisionedCredentialsWithLaunchAdmission(
      { task: "held direct run", domain: "coding" },
      heldAdmission,
      provisioning,
    ),
    /not approved/,
  );
  await assert.rejects(
    () => brain.executeAutoWithProvisionedCredentialsWithLaunchAdmission(
      { task: "held automatic run", domain: "coding" },
      heldAdmission,
      provisioning,
    ),
    /not approved/,
  );
  await assert.rejects(
    () => brain.executeCycleWithProvisionedCredentialsWithLaunchAdmission(
      { task: "held cycle run", domain: "science" },
      heldAdmission,
      provisioning,
    ),
    /not approved/,
  );
  await assert.rejects(
    () => brain.executeAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(
      { task: "held adaptive run", domain: "evaluation" },
      heldAdmission,
      {
        ...provisioning,
        adaptive: {
          maxReplans: 0,
          evaluate: () => ({ evaluator_id: "unused", evaluator_version: "1", reward: 0, passed: false, replan_requested: false }),
        },
      },
    ),
    /not approved/,
  );
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
});

test("brain facade creates request-free plans for every built-in domain and executes a bounded batch", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await brain.plan({ task: tasks[domain], domain });
    assert.equal(plan.status, "ready", domain);
    assert.equal(plan.domain_plan.domain, domain);
    assert.equal(plan.route.task_digest.length, 64);
    assert.equal(plan.toJSON().task_digest, plan.route.task_digest);
    assert.doesNotMatch(JSON.stringify(plan), new RegExp(tasks[domain].replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "i"), domain);
  }

  const batch = await brain.executeBatch(
    Object.entries(tasks).map(([domain, task]) => ({ task, domain })),
    { maxParallelism: 4, execution: { approveProviderCall: true } },
  );
  assert.equal(batch.status, "completed");
  assert.equal(batch.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(batch.failed_count, 0);
  assert.equal(batch.omitted_count, 0);
  assert.deepEqual(batch.items.map((item) => item.index), [...Array(AUTONOMOUS_DOMAIN_NAMES.length).keys()]);
  assert.ok(batch.items.every((item) => item.status === "succeeded" && item.execution?.run?.status === "completed"));
  assert.equal(runtime.providerStatus("offline").successes, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(batch.items.map((item) => item.execution?.plan ?? null)), /debug and verify a bounded repository change/);
});

test("brain facade exposes automatic route-to-invocation execution across every built-in domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const held = await brain.executeAuto({ task: tasks[domain], domain }, { approveProviderCall: false });
    assert.equal(held.status, "approval_required", domain);
    assert.equal(held.automatic?.planning_mode, "deterministic", domain);
    assert.equal(held.automatic?.route.route_digest, held.plan.route.route_digest, domain);
    assert.equal(held.automatic?.next_action, "review_provider_or_effect_approval", domain);
    assert.equal(held.automatic?.result?.status, "approval_required", domain);
  }

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const completed = await brain.executeAuto({ task: tasks[domain], domain }, { approveProviderCall: true });
    assert.equal(completed.status, "completed", domain);
    assert.equal(completed.automatic?.status, "completed", domain);
    assert.equal(completed.automatic?.result?.status, "completed", domain);
    assert.equal(completed.automatic?.route.primary_domain, domain, domain);
    assert.ok(completed.plan.domain_plan || completed.plan.cross_domain_plan, domain);
    assert.ok(!JSON.stringify(completed.plan).includes(tasks[domain]), domain);
  }
});

test("brain facade automatic execution preserves the separate provider-planning acceptance gate", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    const planningMessage = request.messages.find((message) => String(message.content).startsWith("Context planning-contract:\n"));
    if (!planningMessage) return { output_text: "automatic provider execution" };
    const contract = JSON.parse(String(planningMessage.content).slice("Context planning-contract:\n".length));
    const ids = contract.stage_catalogue.map((row) => row.id);
    return { structured: { priority_order: ids, focus_stage_ids: ids.slice(0, 1), review_required: false, confidence: 0.95, abstain: false } };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const held = await brain.executeAuto({ task: tasks.coding, domain: "coding" }, {
    planningMode: "provider",
    planning: { approveProviderCall: false },
    approveProviderCall: false,
  });
  assert.equal(held.status, "approval_required");
  assert.equal(held.automatic?.planning_mode, "provider");
  assert.equal(held.automatic?.planning?.status, "approval_required");
  assert.equal(held.automatic?.result, null);

  const completed = await brain.executeAuto({ task: tasks.coding, domain: "coding" }, {
    planningMode: "provider",
    planning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.automatic?.planning?.status, "completed");
  assert.equal(completed.automatic?.result?.status, "completed");
});

test("brain facade automatic batches preserve per-item policy and deterministic all-domain accounting", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ task: tasks[domain], domain }));
  const preflight = await brain.launchPreflight();
  const heldAdmission = brain.admitLaunchPreflight(preflight, { decision: "hold" });
  await assert.rejects(
    () => brain.executeAutoBatchWithLaunchAdmission(inputs, heldAdmission, { execution: { approveProviderCall: true } }),
    /not approved/,
  );

  const batch = await brain.executeAutoBatch(inputs, {
    maxParallelism: 4,
    execution: (_input, index) => ({ approveProviderCall: true, executionAttempt: index + 1 }),
  });
  assert.equal(batch.schema, "bioprism-typescript-autonomous-brain-auto-batch/0.1");
  assert.equal(batch.status, "completed");
  assert.equal(batch.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(batch.failed_count, 0);
  assert.equal(batch.omitted_count, 0);
  assert.deepEqual(batch.items.map((item) => item.index), [...Array(AUTONOMOUS_DOMAIN_NAMES.length).keys()]);
  assert.ok(batch.items.every((item) => item.status === "succeeded" && item.execution?.automatic?.status === "completed"));
  assert.match(batch.batch_digest, /^[0-9a-f]{64}$/);
});

test("brain facade traced automatic batches expose one redacted all-domain lifecycle", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ task: tasks[domain], domain }));
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.executeAutoBatchWithTrace(inputs, {
    runId: "automatic-batch-trace",
    traceStore,
    maxParallelism: 1,
    execution: (_input, index) => ({ approveProviderCall: true, executionAttempt: index + 1 }),
  });
  assert.equal(traced.schema, "bioprism-typescript-autonomous-brain-traced-auto-batch/0.1");
  assert.equal(traced.batch.status, "completed");
  assert.equal(traced.batch.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(traced.trace.status, "completed");
  assert.match(traced.trace.trace_digest, /^[0-9a-f]{64}$/);
  const phases = traceStore.events({ run_id: "automatic-batch-trace" }).map((event) => event.phase);
  assert.equal(phases[0], "started");
  assert.equal(phases.at(-1), "completed");
  assert.ok(phases.filter((phase) => phase === "plan_compiled").length >= AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(phases.includes("provider_invocation_finished"));
  const persisted = JSON.stringify(traceStore.snapshot());
  assert.doesNotMatch(persisted, /debug and verify|offline:offline-model/);
});

test("brain facade traced automatic resumable batches trace rehydration without replay", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = [
    { task: tasks.coding, domain: "coding" },
    { task: tasks.data, domain: "data" },
  ];
  const checkpoints = [];
  const firstTraceStore = new InMemoryAutonomousRunTraceStore();
  const first = await brain.executeAutoBatchResumableWithTrace(inputs, {
    jobId: "traced-resumable-batch",
    runId: "traced-resumable-first",
    traceStore: firstTraceStore,
    maxParallelism: 1,
    execution: { approveProviderCall: true },
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(first.batch.status, "completed");
  assert.equal(first.trace.status, "completed");
  assert.equal(checkpoints.at(-1).status, "completed");

  const secondTraceStore = new InMemoryAutonomousRunTraceStore();
  let rehydrated = 0;
  const resumed = await brain.executeAutoBatchResumableWithTrace(inputs, {
    jobId: "traced-resumable-batch",
    runId: "traced-resumable-second",
    traceStore: secondTraceStore,
    maxParallelism: 1,
    execution: { approveProviderCall: true },
    checkpoint: checkpoints.at(-1),
    rehydrateExecution: (context) => {
      rehydrated += 1;
      return first.batch.items[context.index].execution;
    },
  });
  assert.equal(resumed.batch.status, "completed");
  assert.equal(rehydrated, inputs.length);
  assert.equal(resumed.trace.status, "completed");
  assert.ok(secondTraceStore.events({ run_id: "traced-resumable-second" }).some((event) => event.detail_digest));
  assert.equal(runtime.providerStatus("offline").attempts, inputs.length, "rehydrated items must not invoke the provider again");
  assert.doesNotMatch(JSON.stringify(resumed), /debug and verify|offline:offline-model/);
});

test("brain facade automatic resumable batches rehydrate completed envelopes without direct-run fallback", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const initialBrain = new AutonomousBrainFacade({ agent });
  const requests = [
    { task: tasks.coding, domain: "coding" },
    {
      task: "review science evidence through a caller-owned connector",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: { hypothesis: "automatic-restart", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
        approved: true,
      },
    },
  ];
  const checkpoints = [];
  const first = await initialBrain.executeAutoBatchResumable(requests, {
    jobId: "automatic-resumable-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(first.status, "partial");
  assert.deepEqual(first.items.map((item) => item.status), ["succeeded", "failed"]);
  assert.deepEqual(checkpoints.at(-1).completed_indices, [0]);
  assert.equal(checkpoints.at(-1).mode, "automatic");
  assert.match(checkpoints.at(-1).automatic_execution_policy_digest, /^[0-9a-f]{64}$/);
  assert.doesNotMatch(JSON.stringify(checkpoints.at(-1)), /debug and verify|automatic-restart|offline:offline-model/);
  await assert.rejects(
    initialBrain.executeBatchResumable(requests, {
      jobId: "automatic-resumable-batch",
      maxParallelism: 1,
      stopOnError: true,
      execution: { approveProviderCall: true },
      checkpoint: checkpoints.at(-1),
      rehydrateExecution: (context) => first.items[context.index].execution,
    }),
    /mode/,
  );

  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const resumedBrain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const resumed = await resumedBrain.executeAutoBatchResumable(requests, {
    jobId: "automatic-resumable-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    checkpoint: checkpoints.at(-1),
    rehydrateExecution: (context) => first.items[context.index].execution,
  });
  assert.equal(resumed.status, "completed");
  assert.deepEqual(resumed.items.map((item) => item.status), ["succeeded", "succeeded"]);
  assert.equal(runtime.providerStatus("offline").attempts, 2, "the completed automatic item must not be invoked again");

  await assert.rejects(
    resumedBrain.executeAutoBatchResumable(requests, {
      jobId: "automatic-resumable-batch",
      maxParallelism: 1,
      stopOnError: true,
      execution: { approveProviderCall: true, includeConnectorObservation: false },
      checkpoint: checkpoints.at(-1),
      rehydrateExecution: (context) => resumed.items[context.index].execution,
    }),
    /policy/,
  );

  const tampered = structuredClone(checkpoints.at(-1));
  tampered.request_digests[0] = "0".repeat(64);
  await assert.rejects(
    resumedBrain.executeAutoBatchResumable(requests, {
      jobId: "automatic-resumable-batch",
      maxParallelism: 1,
      stopOnError: true,
      execution: { approveProviderCall: true },
      checkpoint: tampered,
      rehydrateExecution: (context) => resumed.items[context.index].execution,
    }),
    /checkpoint/i,
  );
});

test("automatic batch controller rehydrates protected results and preserves automatic mode identity", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const initialBrain = new AutonomousBrainFacade({ agent });
  const requests = [
    { task: tasks.coding, domain: "coding" },
    {
      task: "force an automatic restartable connector failure",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: { hypothesis: "automatic-controller", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
        approved: true,
      },
    },
  ];
  const store = new InMemoryAutonomousBrainBatchCheckpointStore();
  const firstController = new AutonomousBrainBatchJobController(initialBrain, store);
  assert.equal((await firstController.restore()).status, "empty");
  const first = await firstController.runAutomatic(requests, {
    jobId: "automatic-controller-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
  });
  assert.equal(first.batch.status, "partial");
  assert.equal(store.read().mode, "automatic");

  const protectedValues = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(
    new AutonomousProtectedRehydrationContext({ tenantId: "automatic-tenant", actorId: "automatic-worker", sessionId: "automatic-session", authorizationDigest: "e".repeat(64) }),
    (reference) => protectedValues.get(reference.value_digest),
    { authorizer: () => true, clock: () => 300 },
  );
  const completedValue = first.batch.items[0].execution;
  const valueDigest = protectedValueDigest(completedValue);
  protectedValues.set(valueDigest, completedValue);
  const protectedRehydrator = new AutonomousBrainAutoBatchProtectedRehydrator({
    adapter: new AutonomousProtectedRehydrationAdapter(boundary),
    receiptResolver: (context) => ({ ...context, domain: "coding", value_digest: valueDigest }),
  });
  await assert.rejects(
    protectedRehydrator.resolve({ job_id: "automatic-controller-batch", index: 0, mode: "brain", request_digest: "a".repeat(64), task_digest: "b".repeat(64), expected_result_digest: "c".repeat(64) }),
    /automatic checkpoint context/,
  );

  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const resumedBrain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const restarted = new AutonomousBrainBatchJobController(resumedBrain, store, { automaticProtectedRehydration: protectedRehydrator });
  assert.equal((await restarted.restore()).status, "restored");
  const completed = await restarted.runAutomatic(requests, {
    jobId: "automatic-controller-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
  });
  assert.equal(completed.batch.status, "completed");
  assert.deepEqual(completed.batch.items.map((item) => item.status), ["succeeded", "succeeded"]);
  assert.equal(completed.controller.status, "completed");
  assert.equal(store.read().mode, "automatic");
});

test("brain facade automatic execution composes connector observation, launch admission, and metadata tracing", async () => {
  const seen = [];
  const runtime = localRuntime((request) => seen.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false, receiptStore: journal });
  const brain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const request = {
    task: tasks.science,
    domain: "science",
    connector: {
      domain: "science",
      capability: "literature",
      operation_id: "science.reproducible_evidence_acquisition",
      subject_digest: "a".repeat(64),
      request: { hypothesis: "automatic-hypothesis", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
      approved: true,
    },
  };
  const preflight = await brain.launchPreflight();
  const heldAdmission = brain.admitLaunchPreflight(preflight, { decision: "hold" });
  await assert.rejects(
    () => brain.executeAutoWithLaunchAdmission(request, heldAdmission, { approveProviderCall: true }),
    /not approved/,
  );
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const executed = await brain.executeAutoWithTrace(request, {
    approveProviderCall: true,
    runId: "automatic-facade-trace",
    traceStore,
  });
  assert.equal(executed.execution.status, "completed");
  assert.equal(executed.execution.automatic?.status, "completed");
  assert.equal(executed.execution.connector?.status, "observed");
  assert.ok(seen.some((item) => item.messages.some((message) => message.content.includes("autonomous-connector-observation"))));
  assert.deepEqual(
    traceStore.events({ run_id: "automatic-facade-trace" }).map((event) => event.phase),
    ["started", "plan_compiled", "connector_started", "connector_finished", "model_selection_started", "model_selection_finished", "provider_invocation_started", "provider_invocation_finished", "completed"],
  );
  assert.doesNotMatch(JSON.stringify(executed.execution.plan), /automatic-hypothesis|b{64}|c{64}/);
});

test("brain facade composes semantic routing across every built-in domain and keeps execution approval separate", async () => {
  const payloads = AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({
    selected_domains: [{ domain, score: 0.94, rationale: `catalogue route for ${domain}` }],
    confidence: 0.93,
    abstain: false,
    abstain_reason: null,
  }));
  const { runtime, calls } = semanticRuntime(payloads);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const execution = await brain.execute({ task: tasks[domain] }, {
      semanticRouting: { enabled: true, approveProviderCall: true },
      approveProviderCall: false,
    });
    assert.equal(execution.semantic_route.status, "completed", domain);
    assert.equal(execution.plan.semantic_route.status, "completed", domain);
    assert.equal(execution.plan.route.route_digest, execution.plan.semantic_route.route.route_digest, domain);
    assert.equal(execution.status, "approval_required", domain);
    assert.equal(execution.run?.status, "approval_required", domain);
  }
  assert.equal(calls(), AUTONOMOUS_DOMAIN_NAMES.length, "only the approved classifier should have run");
});

test("brain facade retains semantic route identity through persisted plan replay without reclassifying", async () => {
  const payloads = [{ selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.93, abstain: false, abstain_reason: null }];
  const { runtime, calls } = semanticRuntime(payloads);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const task = tasks.coding;
  const held = await brain.execute({ task }, { semanticRouting: { enabled: true, approveProviderCall: true }, approveProviderCall: false });
  assert.equal(held.status, "approval_required");
  const restored = AutonomousBrainPlan.fromJSON(held.plan);
  assert.equal(restored.semantic_route.status, "completed");
  const resumed = await brain.executePlanned(restored, { task }, { approveProviderCall: true });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.semantic_route.route.route_digest, restored.route.route_digest);
  assert.equal(calls(), 2, "plan replay must reuse the classifier route and only invoke execution");
});

test("brain facade classifier approval is a distinct gate for cycles and adaptive loops", async () => {
  const payloads = [
    { selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.93, abstain: false, abstain_reason: null },
    { selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.93, abstain: false, abstain_reason: null },
  ];
  const { runtime, calls } = semanticRuntime(payloads);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const cycle = await brain.executeCycle({ task: tasks.coding }, {
    semanticRouting: { enabled: true, approveProviderCall: true },
    approveProviderCall: false,
  });
  assert.equal(cycle.semantic_route.status, "completed");
  assert.equal(cycle.status, "approval_required");
  assert.equal(cycle.cycle.status, "approval_required");
  assert.equal(cycle.cycle.run.status, "approval_required");

  const adaptive = await brain.executeAdaptiveCycle({ task: tasks.coding }, {
    semanticRouting: { enabled: true, approveProviderCall: true },
    approveProviderCall: false,
    adaptive: {
      evaluate: () => { throw new Error("adaptive evaluator must not run before execution approval"); },
    },
  });
  assert.equal(adaptive.semantic_route.status, "completed");
  assert.equal(adaptive.status, "approval_required");
  assert.equal(adaptive.adaptive.final.status, "approval_required");
  assert.equal(adaptive.adaptive.final.run.status, "approval_required");
  assert.equal(calls(), 2);
});

test("brain facade previews provider-free model selection for every built-in domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const beforeAttempts = runtime.providerStatus("offline").attempts;

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = tasks[domain];
    const preview = await brain.modelSelectionPreview({ task, domain });
    assert.equal(preview.schema, "bioprism-typescript-autonomous-model-selection-preview/0.1", domain);
    assert.equal(preview.status, "selected", domain);
    assert.equal(preview.domain, domain);
    assert.equal(preview.candidate_count, 1);
    assert.equal(preview.eligible_candidate_count, 1);
    assert.equal(preview.selection_audit.selected_model?.provider, "offline");
    assert.equal(preview.review.provider_call, "not_started");
    assert.equal(preview.execution, "preview_only; no_provider_or_domain_tool_invocation");
    assert.equal(preview.authority_posture, "selection_review_only; preview_does_not_authorize_provider_or_effects");
    assert.equal(preview.secret_material, "never_returned");
    assert.equal(preview.selection_context_digest.length, 64);
    assert.equal(preview.execution_plan_digest.length, 64);
    assert.equal(preview.task_intent_digest.length, 64);
    assert.equal(preview.task_decision_digest.length, 64);
    assert.ok(["admitted", "review_required", "blocked"].includes(preview.task_decision_posture));
    assert.equal(preview.selection_contract.task_decision_digest, preview.task_decision_digest);
    assert.ok(!JSON.stringify(preview).includes(task), domain);
  }
  assert.equal(runtime.providerStatus("offline").attempts, beforeAttempts);

  const unconfiguredRuntime = new LLMRuntime({ fetch: async () => { throw new Error("provider must not be contacted"); } });
  const unconfiguredAgent = new AutonomousAgent(unconfiguredRuntime);
  unconfiguredAgent.registerModel(model);
  const refused = await new AutonomousBrainFacade({ agent: unconfiguredAgent }).modelSelectionPreview({
    task: tasks.coding,
    domain: "coding",
  });
  assert.equal(refused.status, "refused_no_eligible_model");
  assert.equal(refused.eligible_candidate_count, 0);
  assert.equal(refused.review.next_action, "resolve_model_provider_or_credential_gates");
});

test("brain facade blocks approved provider dispatch when task posture is forbidden", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const task = "deploy the biomedical report and verify safety";
  const preview = await brain.modelSelectionPreview({ task, domain: "biomedical" });
  assert.equal(preview.status, "selected");
  assert.equal(preview.task_decision_posture, "blocked");
  assert.equal(preview.review.next_action, "resolve_task_decision_block");
  const attempts = runtime.providerStatus("offline").attempts;
  await assert.rejects(
    () => brain.executeApprovedSelection({ task, domain: "biomedical" }, preview),
    /blocked by the task decision posture/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, attempts);
  await assert.rejects(
    () => brain.execute({ task, domain: "biomedical" }, { approveProviderCall: true }),
    /blocked by the task decision posture/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, attempts);
});

test("brain facade revalidates approved model previews and invokes one exact local arm across every domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const previews = new Map();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = tasks[domain];
    const preview = await brain.modelSelectionPreview({ task, domain });
    previews.set(domain, preview);
    assert.deepEqual(preview.selection_contract.candidate_ids, ["offline/offline-model"]);
    const execution = await brain.executeApprovedSelection({ task, domain }, preview);
    assert.equal(execution.status, "completed", domain);
    assert.equal(execution.run?.status, "completed", domain);
    assert.equal(execution.run?.selection.selected_model?.provider, "offline", domain);
    assert.equal(execution.run?.selection.selected_model?.model, "offline-model", domain);
    assert.equal(execution.plan.task_digest, preview.task_digest, domain);
    assert.ok(!JSON.stringify(execution.plan).includes(task), domain);
  }
  assert.equal(runtime.providerStatus("offline").attempts, AUTONOMOUS_DOMAIN_NAMES.length);

  const stale = structuredClone(previews.get("coding"));
  stale.selection_contract.requested_output_tokens += 1;
  await assert.rejects(
    () => brain.executeApprovedSelection({ task: tasks.coding, domain: "coding" }, stale),
    /output budget changed|stale|changed/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade binds weighted selection policy and observations into approval integrity", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const weights = { quality: 1, reliability: 0, cost: 0, latency: 0, exploration: 0 };
  const observations = [{ arm_id: "offline/offline-model", pulls: 2, reward_sum: 1.5, failures: 0 }];
  const preview = await brain.modelSelectionPreview(
    { task: tasks.evaluation, domain: "evaluation" },
    { selectionWeights: weights, selectionObservations: observations },
  );
  assert.deepEqual(preview.selection_contract.selection_weights, weights);
  assert.equal(preview.selection_contract.selection_observations_digest.length, 64);
  await assert.rejects(
    () => brain.executeApprovedSelection(
      { task: tasks.evaluation, domain: "evaluation" },
      preview,
      { run: { selectionWeights: { cost: 1 }, selectionObservations: observations } },
    ),
    /weights changed|re-review required/,
  );
  await assert.rejects(
    () => brain.executeApprovedSelection(
      { task: tasks.evaluation, domain: "evaluation" },
      preview,
      { run: { selectionWeights: weights, selectionObservations: [] } },
    ),
    /observations changed|re-review required/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, 0);
});

test("brain facade closed-loop execution accepts every built-in domain through one provider-neutral entry point", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await brain.executeCycle({ task: tasks[domain], domain }, { approveProviderCall: true });
    assert.equal(result.plan.selected_domains.includes(domain), true, domain);
    assert.ok(["completed", "children_completed"].includes(result.status), `${domain}: ${result.status}`);
    assert.ok(result.cycle !== null, domain);
  }
  assert.ok(runtime.providerStatus("offline").successes >= AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade exposes bounded evaluator-guided replanning across every built-in domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await brain.executeAdaptiveCycle(
      { task: tasks[domain], domain },
      {
        approveProviderCall: true,
        adaptive: {
          maxReplans: 0,
          evaluate: () => ({ evaluator_id: `${domain}-evaluator`, evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
        },
      },
    );
    assert.equal(result.status, "completed", domain);
    assert.equal(result.adaptive.attempts.length, 1, domain);
    assert.equal(result.adaptive.replan_count, 0, domain);
    assert.equal(result.adaptive.final.status, "completed", domain);
  }
  assert.equal(runtime.providerStatus("offline").successes, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade keeps adaptive replanning bounded and routes cross-domain fan-out through the same boundary", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  let evaluations = 0;
  const single = await brain.executeAdaptiveCycle(
    { task: "design a reproducible science experiment", domain: "science" },
    {
      approveProviderCall: true,
      adaptive: {
        maxReplans: 1,
        evaluate: () => {
          evaluations += 1;
          return { evaluator_id: "bounded-reviewer", evaluator_version: "1", reward: evaluations === 1 ? 0.35 : 0.91, passed: evaluations > 1, replan_requested: evaluations === 1, replan_instruction: evaluations === 1 ? "tighten reproducibility and state uncertainty" : null };
        },
      },
    },
  );
  assert.equal(single.status, "completed");
  assert.equal(single.adaptive.attempts.length, 2);
  assert.equal(single.adaptive.replan_count, 1);
  assert.equal(single.adaptive.status, "completed");

  const cross = await brain.executeAdaptiveCycle(
    { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
    {
      approveProviderCall: true,
      adaptive: {
        maxReplans: 0,
        synthesize: false,
        maxParallelChildren: 2,
        subtasks: [
          { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
          { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
        ],
        evaluate: (run) => ({ evaluator_id: "cross-reviewer", evaluator_version: "1", reward: 0.82, passed: true, replan_requested: false, rewards: {} }),
      },
    },
  );
  assert.equal(cross.status, "completed");
  assert.equal(cross.adaptive.attempts.length, 1);
  assert.equal(cross.adaptive.final.run.child_runs.length, 2);
});

test("brain facade batches closed-loop work across every domain with deterministic accounting", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const batch = await brain.executeCycleBatch(
    AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ task: tasks[domain], domain })),
    { maxParallelism: 3, cycle: { approveProviderCall: true } },
  );
  assert.equal(batch.schema, "bioprism-typescript-autonomous-brain-cycle-batch/0.1");
  assert.equal(batch.status, "completed");
  assert.equal(batch.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(batch.failed_count, 0);
  assert.equal(batch.omitted_count, 0);
  assert.deepEqual(batch.items.map((item) => item.index), [...Array(AUTONOMOUS_DOMAIN_NAMES.length).keys()]);
  assert.ok(batch.items.every((item) => item.status === "succeeded" && item.execution?.cycle !== null));
  assert.equal(batch.batch_digest.length, 64);

  const refused = await brain.executeCycleBatch(
    [
      { task: "prepare a science review", domain: "science" },
      { task: "prepare a biomedical review", domain: "biomedical" },
      { task: "prepare an operations review", domain: "operations" },
    ],
    { maxParallelism: 1, stopOnError: true, cycle: {} },
  );
  assert.equal(refused.status, "failed");
  assert.equal(refused.items[0].status, "refused");
  assert.deepEqual(refused.items.slice(1).map((item) => item.status), ["omitted", "omitted"]);
});

test("brain facade batches adaptive single and cross-domain loops through per-item policies", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = [
    { task: "design a reproducible science experiment", domain: "science" },
    { task: "review biomedical evidence", domain: "biomedical" },
    { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
  ];
  const batch = await brain.executeAdaptiveCycleBatch(inputs, {
    maxParallelism: 2,
    adaptive: (input) => input.allow_cross_domain
      ? {
        approveProviderCall: true,
        adaptive: {
          maxReplans: 0,
          synthesize: false,
          maxParallelChildren: 2,
          subtasks: [
            { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
            { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
          ],
          evaluate: () => ({ evaluator_id: "batch-cross-reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false, rewards: {} }),
        },
      }
      : {
        approveProviderCall: true,
        adaptive: {
          maxReplans: 0,
          evaluate: () => ({ evaluator_id: "batch-reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
        },
      },
  });
  assert.equal(batch.schema, "bioprism-typescript-autonomous-brain-adaptive-batch/0.1");
  assert.equal(batch.status, "completed");
  assert.equal(batch.completed_count, 3);
  assert.ok(batch.items.every((item) => item.status === "succeeded" && item.execution?.adaptive !== null));
  assert.equal(batch.items[2].execution.adaptive.final.run.child_runs.length, 2);
});

test("brain facade resumes metadata-only batches through caller-owned rehydration and rejects tampering", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const initialBrain = new AutonomousBrainFacade({ agent });
  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const resumedBrain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const requests = [
    { task: tasks.coding, domain: "coding" },
    {
      task: "review scientific evidence and state reproducibility gaps",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: {
          hypothesis: "h1",
          evidence_digests: ["b".repeat(64)],
          analysis_digest: "c".repeat(64),
        },
        approved: true,
      },
    },
  ];
  const checkpoints = [];
  const first = await initialBrain.executeBatchResumable(requests, {
    jobId: "typescript-restartable-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(first.status, "partial");
  assert.deepEqual(first.items.map((item) => item.status), ["succeeded", "failed"]);
  assert.deepEqual(checkpoints.at(-1).completed_indices, [0]);
  assert.doesNotMatch(JSON.stringify(checkpoints.at(-1)), /debug and verify|offline:offline-model|hypothesis/);

  const restored = await resumedBrain.executeBatchResumable(requests, {
    jobId: "typescript-restartable-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    checkpoint: checkpoints.at(-1),
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
    rehydrateExecution: (context) => first.items[context.index].execution,
  });
  assert.equal(restored.status, "completed");
  assert.deepEqual(restored.items.map((item) => item.status), ["succeeded", "succeeded"]);
  assert.equal(checkpoints.at(-1).status, "completed");

  const tampered = structuredClone(checkpoints.at(-1));
  tampered.request_digests[0] = "0".repeat(64);
  await assert.rejects(
    resumedBrain.executeBatchResumable(requests, {
      jobId: "typescript-restartable-batch",
      maxParallelism: 1,
      stopOnError: true,
      execution: { approveProviderCall: true },
      checkpoint: tampered,
      rehydrateExecution: (context) => restored.items[context.index].execution,
    }),
    /checkpoint/i,
  );
});

test("brain batch checkpoints bind semantic-routing policy and refuse drift or legacy rebinding", async () => {
  const payloads = [
    { selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.93, abstain: false, abstain_reason: null },
    { selected_domains: [{ domain: "science", score: 0.94, rationale: "evidence" }], confidence: 0.93, abstain: false, abstain_reason: null },
  ];
  const { runtime } = semanticRuntime(payloads);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const requests = [
    { task: tasks.coding },
    {
      task: tasks.science,
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: { hypothesis: "h1", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
        approved: true,
      },
    },
  ];
  const checkpoints = [];
  const execution = { semanticRouting: { enabled: true, approveProviderCall: true, minSemanticConfidence: 0.5 }, approveProviderCall: true };
  const first = await brain.executeBatchResumable(requests, {
    jobId: "semantic-routing-policy-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution,
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(first.status, "partial");
  const checkpoint = checkpoints.at(-1);
  assert.match(checkpoint.semantic_routing_policy_digest, /^[0-9a-f]{64}$/);
  assert.doesNotMatch(JSON.stringify(checkpoint), /debug and verify|implementation|hypothesis|offline-model/);
  await assert.rejects(
    brain.executeBatchResumable(requests, {
      jobId: "semantic-routing-policy-batch",
      maxParallelism: 1,
      stopOnError: true,
      execution: { semanticRouting: { enabled: true, approveProviderCall: true, minSemanticConfidence: 0.8 }, approveProviderCall: true },
      checkpoint,
      rehydrateExecution: (context) => first.items[context.index].execution,
    }),
    /semantic-routing policy|execution policy|checkpoint/i,
  );

  const legacyCheckpoints = [];
  const legacy = await brain.executeBatchResumable([{ task: tasks.coding }], {
    jobId: "legacy-semantic-routing-batch",
    execution: { approveProviderCall: false },
    checkpointSink: (value) => legacyCheckpoints.push(value),
  });
  assert.equal(legacy.status, "failed");
  assert.equal(legacyCheckpoints.at(-1).semantic_routing_policy_digest, undefined);
  await assert.rejects(
    brain.executeBatchResumable([{ task: tasks.coding }], {
      jobId: "legacy-semantic-routing-batch",
      execution: { semanticRouting: { enabled: true, approveProviderCall: true } },
      checkpoint: legacyCheckpoints.at(-1),
    }),
    /legacy.*semantic-routing|checkpoint/i,
  );
});

test("brain batch controller owns restore, persistence, restart rehydration, and checkpoint tamper rejection", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const initialBrain = new AutonomousBrainFacade({ agent });
  const requests = [
    { task: tasks.coding, domain: "coding" },
    {
      task: "review science evidence through a caller-owned connector",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: { hypothesis: "controller-hypothesis", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
        approved: true,
      },
    },
  ];
  const store = new InMemoryAutonomousBrainBatchCheckpointStore();
  const controller = new AutonomousBrainBatchJobController(initialBrain, store);
  await assert.rejects(
    controller.run(requests, { jobId: "typescript-controller-job", maxParallelism: 1, stopOnError: true, execution: { approveProviderCall: true } }),
    /restore/i,
  );
  const empty = await controller.restore();
  assert.equal(empty.status, "empty");
  const first = await controller.run(requests, {
    jobId: "typescript-controller-job",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
  });
  assert.equal(first.batch.status, "partial");
  assert.deepEqual(first.batch.items.map((item) => item.status), ["succeeded", "failed"]);
  const persisted = store.read();
  assert.ok(persisted);
  assert.equal(first.controller.completed_items, 1);
  assert.doesNotMatch(JSON.stringify(persisted), /debug and verify|controller-hypothesis|offline:offline-model/);

  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const resumedBrain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const restarted = new AutonomousBrainBatchJobController(resumedBrain, store);
  const restored = await restarted.restore();
  assert.equal(restored.status, "restored");
  const completed = await restarted.run(requests, {
    jobId: "typescript-controller-job",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    rehydrateExecution: (context) => first.batch.items[context.index].execution,
  });
  assert.equal(completed.batch.status, "completed");
  assert.deepEqual(completed.batch.items.map((item) => item.status), ["succeeded", "succeeded"]);
  assert.equal(completed.controller.status, "completed");
  assert.equal(store.read().status, "completed");

  const tampered = structuredClone(store.read());
  tampered.request_digests[0] = "0".repeat(64);
  const invalid = new AutonomousBrainBatchJobController(resumedBrain, {
    read: () => tampered,
    write: () => {},
  });
  await assert.rejects(invalid.restore(), /checkpoint/i);
});

test("brain batch controller resolves protected receipts after restart and preserves explicit callback precedence", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const connector = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false });
  const resumedBrain = new AutonomousBrainFacade({ agent, connectorOperations: connector.operationFacade });
  const requests = [
    { task: "rehydrate a protected coding result", domain: "coding" },
    {
      task: "force a restartable protected batch failure",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        subject_digest: "a".repeat(64),
        request: { hypothesis: "protected-batch", evidence_digests: ["b".repeat(64)], analysis_digest: "c".repeat(64) },
        approved: true,
      },
    },
  ];
  const values = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(
    new AutonomousProtectedRehydrationContext({ tenantId: "tenant-batch", actorId: "worker-batch", sessionId: "session-batch", authorizationDigest: "a".repeat(64) }),
    (reference) => values.get(reference.value_digest),
    { authorizer: () => true, clock: () => 100 },
  );
  let protectedCalls = 0;
  let protectedReceiptCalls = 0;
  const protectedRehydrator = new AutonomousBrainBatchProtectedRehydrator({
    adapter: new AutonomousProtectedRehydrationAdapter(boundary),
    receiptResolver: (context) => {
      protectedReceiptCalls += 1;
      return {
        job_id: context.job_id,
        index: context.index,
        mode: context.mode,
        request_digest: context.request_digest,
        task_digest: context.task_digest,
        expected_result_digest: context.expected_result_digest,
        domain: "coding",
        value_digest: [...values.keys()][0],
      };
    },
  });
  const store = new InMemoryAutonomousBrainBatchCheckpointStore();
  const firstController = new AutonomousBrainBatchJobController(brain, store, { protectedRehydration: protectedRehydrator });
  assert.equal((await firstController.restore()).status, "empty");
  const first = await firstController.run(requests, {
    jobId: "protected-typescript-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
  });
  // The first item is the only one that executes successfully in this deliberate restart fixture.
  assert.equal(first.batch.status, "partial");
  const value = first.batch.items[0].execution;
  values.set(protectedValueDigest(value), value);
  const firstCheckpoint = store.read();
  assert.ok(firstCheckpoint);
  assert.doesNotMatch(JSON.stringify(store.read()), /rehydrate a protected coding result|offline:offline-model/);

  const restarted = new AutonomousBrainBatchJobController(resumedBrain, store, { protectedRehydration: protectedRehydrator });
  assert.equal((await restarted.restore()).status, "restored");
  const completed = await restarted.run(requests, {
    jobId: "protected-typescript-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
  });
  assert.equal(completed.batch.status, "completed");
  assert.equal(completed.batch.items[0].execution.status, "completed");

  const explicitStore = new InMemoryAutonomousBrainBatchCheckpointStore(firstCheckpoint);
  const explicit = new AutonomousBrainBatchJobController(resumedBrain, explicitStore, { protectedRehydration: protectedRehydrator });
  assert.equal((await explicit.restore()).status, "restored");
  const protectedCallsBeforeExplicit = protectedReceiptCalls;
  const explicitCompleted = await explicit.run(requests, {
    jobId: "protected-typescript-batch",
    maxParallelism: 1,
    stopOnError: true,
    execution: { approveProviderCall: true },
    rehydrateExecution: (context) => { protectedCalls += 1; return first.batch.items[context.index].execution; },
  });
  assert.equal(explicitCompleted.batch.status, "completed");
  assert.equal(protectedCalls, 1);
  assert.equal(protectedReceiptCalls, protectedCallsBeforeExplicit);
});

test("brain batch protected receipts cover every built-in domain and fail closed on identity drift", async () => {
  const values = new Map();
  const receipts = new Map();
  const boundary = new AutonomousProtectedRehydrationBoundary(
    new AutonomousProtectedRehydrationContext({ tenantId: "tenant-all", actorId: "worker-all", sessionId: "session-all", authorizationDigest: "b".repeat(64) }),
    (reference) => values.get(reference.value_digest),
    { authorizer: () => true, clock: () => 200 },
  );
  const contexts = AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => {
    const requestDigest = `${(index + 3) % 10}`.repeat(64);
    const taskDigest = `${(index + 1) % 10}`.repeat(64);
    const resultDigest = `${(index + 2) % 10}`.repeat(64);
    const value = { status: "completed", domain };
    const valueDigest = protectedValueDigest(value);
    values.set(valueDigest, value);
    const context = { job_id: "all-domain-protected-typescript", index, mode: "brain", request_digest: requestDigest, task_digest: taskDigest, expected_result_digest: resultDigest };
    receipts.set(index, { ...context, domain, value_digest: valueDigest });
    return context;
  });
  const rehydrator = new AutonomousBrainBatchProtectedRehydrator({
    adapter: new AutonomousProtectedRehydrationAdapter(boundary),
    receiptResolver: (context) => receipts.get(context.index),
  });
  const resolved = await Promise.all(contexts.map((context) => rehydrator.resolve(context)));
  assert.deepEqual(resolved.map((value) => value.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  receipts.set(0, { ...receipts.get(0), request_digest: "0".repeat(64) });
  await assert.rejects(rehydrator.resolve(contexts[0]), /request_digest/);
});

test("brain facade exposes a keyless readiness and activation lifecycle for onboarding", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const report = await brain.readiness();
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.execution, "not_started; no_provider_or_tool_calls");
  assert.equal(report.secret_material, "never_returned");
  assert.doesNotMatch(JSON.stringify(report), /gsk_[A-Za-z0-9]|sk-[A-Za-z0-9]/i);

  const refreshed = await brain.refreshActivation();
  assert.equal(refreshed.secret_material, "never_returned");
  assert.equal(brain.activationState().state_digest, refreshed.state_digest);
  const store = new AutonomousCapabilityActivationStore();
  await brain.saveActivation(store);
  const savedState = brain.activationState();
  brain.revokeActivation("onboarding-review-reset");
  assert.equal(brain.activationState().status, "revoked");
  const restoredAgent = new AutonomousAgent(runtime, { activation: new AutonomousCapabilityActivation({ activationId: "onboarding-restore" }) });
  restoredAgent.registerModel(model);
  const restoredBrain = new AutonomousBrainFacade({ agent: restoredAgent });
  const restored = await restoredBrain.restoreActivation(store);
  assert.equal(restored.state_digest, savedState.state_digest);
  assert.equal(restoredBrain.activationState().status, savedState.status);
});

test("brain facade runs a connector observation before provider invocation and supports plan replay", async () => {
  const seen = [];
  const runtime = localRuntime((request) => seen.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const offline = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false, receiptStore: journal });
  const brain = new AutonomousBrainFacade({ agent, connectorOperations: offline.operationFacade });
  const request = {
    task: "review scientific evidence and state reproducibility gaps",
    domain: "science",
    connector: {
      domain: "science",
      capability: "literature",
      operation_id: "science.reproducible_evidence_acquisition",
      subject_digest: "a".repeat(64),
      request: {
        hypothesis: "h1",
        evidence_digests: ["b".repeat(64)],
        analysis_digest: "c".repeat(64),
      },
      approved: true,
    },
  };

  const plan = await brain.plan(request);
  assert.equal(plan.status, "ready");
  assert.ok(plan.connector_plan);
  assert.equal(plan.connector_plan.selected_connector_id, "builtin.offline-evidence.science");
  const serializedPlan = JSON.stringify(plan);
  assert.doesNotMatch(serializedPlan, /"h1"/);
  assert.doesNotMatch(serializedPlan, new RegExp("b".repeat(64)));
  assert.doesNotMatch(serializedPlan, new RegExp("c".repeat(64)));
  const restored = AutonomousBrainPlan.fromJSON(plan.toJSON());
  assert.equal(restored.plan_digest, plan.plan_digest);

  const first = await brain.executePlanned(restored, request, { approveProviderCall: true });
  assert.equal(first.status, "completed");
  assert.equal(first.connector.status, "observed");
  assert.equal(first.connector.replay, "fresh");
  assert.ok(seen.some((item) => item.messages.some((message) => message.content.includes("autonomous-connector-observation"))));

  const replayed = await brain.executePlanned(restored, request, { approveProviderCall: true });
  assert.equal(replayed.status, "completed");
  assert.equal(replayed.connector.replay, "replayed");
  assert.equal(replayed.connector.dispatch.value, null);
  assert.equal(journal.verifyIntegrity().entries, 1);

  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.executePlannedWithTrace(restored, request, {
    traceStore,
    runId: "connector-brain-trace",
    approveProviderCall: true,
  });
  assert.equal(traced.execution.status, "completed");
  assert.deepEqual(
    traceStore.events({ run_id: "connector-brain-trace" }).map((event) => event.phase),
    ["started", "plan_compiled", "connector_started", "connector_finished", "model_selection_started", "model_selection_finished", "provider_invocation_started", "provider_invocation_finished", "completed"],
  );
});

test("brain facade composes connector evidence, evaluator reward, online learning, memory, and restart-safe cycle replay", async () => {
  const seen = [];
  const runtime = localRuntime((request) => seen.push(request));
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const connectorJournal = new InMemoryAutonomousConnectorReceiptJournal();
  const offline = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: false, receiptStore: connectorJournal });
  const brain = new AutonomousBrainFacade({ agent, connectorOperations: offline.operationFacade });
  const learningOutbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const learning = new AutonomousLearningController(agent, { feedbackOutbox: learningOutbox });
  const memory = new InMemoryAutonomousEpisodicMemory();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const task = "review scientific evidence and report reproducibility gaps";
  const connector = {
    domain: "science",
    capability: "literature",
    operation_id: "science.reproducible_evidence_acquisition",
    subject_digest: "d".repeat(64),
    request: { evidence_digests: ["e".repeat(64)], analysis_digest: "f".repeat(64) },
    approved: true,
  };
  const cycleOptions = {
    approveProviderCall: true,
    cycle: {
      cycleId: "brain-cycle-science",
      decisionStateStore: stateStore,
      learning: {
        controller: learning,
        episodeId: "brain-learning-science",
        outbox: { workerId: "brain-cycle-worker" },
        evaluate: () => ({ evaluator_id: "science-reviewer", evaluator_version: "1", reward: 0.87, passed: true }),
      },
      memory: { store: memory, episodeId: "brain-memory-science", tags: ["science"] },
    },
  };
  const reviewedPlan = await brain.plan({ task, domain: "science", connector });
  const first = await brain.executePlannedCycle(reviewedPlan, { task, domain: "science", connector }, cycleOptions);
  assert.equal(first.status, "completed");
  assert.equal(first.cycle.status, "completed");
  assert.equal(first.cycle.evaluation.reward, 0.87);
  assert.equal(first.cycle.learning_episode_id, "brain-learning-science");
  assert.equal(first.cycle.settlement.episode.status, "settled");
  assert.equal(first.connector.replay, "fresh");
  assert.equal(first.connector.dispatch.value.status, "partial", "the connector value is caller-transient and never enters the persisted plan");
  assert.ok(seen.some((request) => request.messages.some((message) => message.content.includes("autonomous-connector-observation"))));
  assert.equal(JSON.stringify(first.plan).includes(task), false);
  assert.equal(JSON.stringify(await memory.snapshot()).includes(task), false);
  assert.equal(learningOutbox.rows().filter((row) => row.status === "applied").length, 1);

  const providerCallsAfterFirstCycle = seen.length;
  cycleOptions.cycle.rehydrateResult = () => first.cycle;
  const replayed = await brain.executePlannedCycle(reviewedPlan, { task, domain: "science", connector }, cycleOptions);
  assert.equal(replayed.status, "completed");
  assert.equal(replayed.cycle.status, "completed");
  assert.equal(replayed.connector.replay, "replayed");
  assert.equal(seen.length, providerCallsAfterFirstCycle, "a terminal persisted cycle rehydrates without another provider invocation");
  assert.equal(connectorJournal.verifyIntegrity().entries, 1);
});

test("brain facade sends cross-domain fan-out through the same closed-loop evaluator and learner boundary", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const learning = new AutonomousLearningController(agent);
  const result = await brain.executeCycle(
    { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
    {
      approveProviderCall: true,
      cycle: {
        synthesize: false,
        maxParallelChildren: 2,
        subtasks: [
          { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
          { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
        ],
        learning: {
          controller: learning,
          trajectoryId: "brain-cross-trajectory",
          evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "cross-reviewer", evaluator_version: "1", reward: 0.79, passed: true }])),
        },
      },
    },
  );
  assert.equal(result.status, "children_completed");
  assert.equal(result.cycle.status, "children_completed");
  assert.equal(result.cycle.run.child_runs.length, 2);
  assert.equal(Object.keys(result.cycle.evaluation).length, 2);
  assert.equal(result.cycle.settlement.trajectory.trajectory.trajectory_id, "brain-cross-trajectory");
  assert.equal(result.plan.cross_domain_plan.children.length, 2);
});

test("brain facade fails closed on route, connector, and plan identity boundaries", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const offline = createBuiltinAutonomousConnectorRuntime({ domainScoped: true, approvalRequired: true });
  const brain = new AutonomousBrainFacade({ agent, connectorOperations: offline.operationFacade });

  const review = await brain.execute({ task: "ambiguous request with no known domain terms" });
  assert.equal(review.status, "route_review_required");
  assert.equal(review.run, null);

  await assert.rejects(
    brain.plan({
      task: "review science",
      domain: "science",
      connector: {
        domain: "science",
        capability: "literature",
        operation_id: "science.reproducible_evidence_acquisition",
        request: { api_key: "must never be accepted" },
      },
    }),
    /credential-shaped|api_key|secret/i,
  );

  const valid = await brain.plan({ task: tasks.coding, domain: "coding" });
  await assert.rejects(
    brain.executePlanned(valid, { task: "a different task", domain: "coding" }, { approveProviderCall: true }),
    /does not match the transient request/,
  );
});

test("brain facade mission boundary invokes exact tools and traces every built-in domain without retaining values", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("mission provider must stay local"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    providerCalls += 1;
    if (request.messages.some((message) => message.role === "tool")) return { output_text: "mission completion" };
    const tool = request.tools?.[0];
    return tool === undefined
      ? { output_text: "mission completion" }
      : { tool_calls: [{ call_id: `mission-call-${providerCalls}`, name: tool.name, arguments: {} }] };
  });
  const profiles = await builtinAutonomousDomainProfiles();
  const domainTools = Object.fromEntries(profiles.map((profile) => [profile.domain, profile.tool_profile.bindings[0].name]));
  const toolNames = [...new Set(Object.values(domainTools))];
  const toolCatalogue = await ToolCatalogue.fromDefinitions(toolNames.map((name) => ({
    name,
    description: `bounded ${name} mission probe`,
    inputSchema: { type: "object", additionalProperties: true },
  })));
  const agent = new AutonomousAgent(runtime, {
    toolCatalogue,
    toolExecutor: async () => ({ ok: true, transient_value: "caller-owned" }),
  });
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const mission = {
      mission_id: `facade-mission-${domain}`,
      goal: `verify the bounded ${domain} mission contract`,
      steps: [{ id: `step-${domain}`, domain, capability: "verification", objective: `verify ${domain}`, tool: domainTools[domain], arguments: {} }],
      policy: { execute: true, stop_on_error: true, allow_side_effects: false, max_steps: 8, max_step_output_bytes: 100_000, max_total_output_bytes: 1_000_000, execution_mode: "serial", max_parallelism: 1, allowed_tools: [domainTools[domain]] },
    };
    const result = await brain.runMissionReplanCycle(mission, {
      evaluate: () => ({ evaluator_id: "facade-reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
      stepRun: { candidates: agent.models() },
      approveEffects: true,
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.final_execution.results[0].status, "succeeded", domain);
    assert.equal(result.final_execution.results[0].decision.provider, "offline", domain);
  }

  const tracedMission = {
    mission_id: "facade-traced-mission",
    goal: "retain no raw traced mission goal or tool output",
    steps: [{ id: "trace-step", domain: "coding", capability: "verification", objective: "execute a transient probe", tool: domainTools.coding, arguments: {} }],
    policy: { execute: true, stop_on_error: true, allow_side_effects: false, max_steps: 8, max_step_output_bytes: 100_000, max_total_output_bytes: 1_000_000, execution_mode: "serial", max_parallelism: 1, allowed_tools: [domainTools.coding] },
  };
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.runMissionReplanCycleWithTrace(tracedMission, {
    traceStore,
    runId: "facade-traced-mission-run",
    evaluate: () => ({ evaluator_id: "facade-reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
    stepRun: { candidates: agent.models() },
    approveEffects: true,
  });
  assert.equal(traced.result.status, "completed");
  assert.equal(traced.trace.status, "completed");
  assert.equal(traced.trace.provider_invocations, 2);
  assert.equal(traced.result.final_execution.results[0].value.transient_value, "caller-owned");
  assert.doesNotMatch(JSON.stringify(traced), /retain no raw traced mission goal|caller-owned|mission completion/);
  assert.doesNotMatch(JSON.stringify(await traceStore.snapshot()), /retain no raw traced mission goal|caller-owned|mission completion/);
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length * 2 + 2);
});

test("brain facade mission boundary rejects malformed graphs and unadmitted semantic launch paths", async () => {
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime()) });
  const validMission = {
    mission_id: "facade-validation-mission",
    goal: "validate the mission boundary",
    steps: [{ id: "step", domain: "coding", capability: "verification", objective: "validate one step", tool: "mission_probe", arguments: {} }],
  };
  const evaluate = () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false });

  await assert.rejects(() => brain.runMissionReplanCycle(validMission, undefined), /evaluator callback/);
  assert.throws(() => brain.authorizeMissionLaunchAdmission(validMission, {}), /launch admission/);
  await assert.rejects(
    () => brain.runMissionReplanCycleWithLaunchAdmission(validMission, {}, { evaluate, execute: { semanticRouting: { enabled: true } } }),
    /provider-free routing/,
  );
  assert.throws(
    () => brain.authorizeMissionLaunchAdmission({ ...validMission, steps: [{ ...validMission.steps[0], id: "step" }, { ...validMission.steps[0], id: "step" }] }, {}),
    /duplicate step id/,
  );
});
