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
  AutonomousEvaluatorCalibrationHarness,
  AutonomousEvaluatorCalibrationRegistry,
  AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator,
  AutonomousValueEvaluatorRegistry,
  InMemoryAutonomousEvaluatorCalibrationStore,
  AutonomousDecisionCyclePersistenceCoordinator,
  AutonomousOnlineLearnerPersistenceCoordinator,
  AutonomousPromptLearningPersistenceCoordinator,
  TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence,
  TransactionalJsonAutonomousPromptLearningSnapshotPersistence,
  TransactionalJsonAutonomousToolSelectionPersistence,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  InMemoryAutonomousDecisionCycleStateStore,
  InMemoryAutonomousEpisodicMemory,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  InMemoryAutonomousConnectorReceiptJournal,
  InMemoryAutonomousRunTraceStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  builtinAutonomousPromptRegistry,
  builtinAutonomousValueEvaluatorProfiles,
  LLMRuntime,
  createBuiltinAutonomousConnectorRuntime,
  AutonomousProtectedRehydrationAdapter,
  AutonomousProtectedRehydrationBoundary,
  AutonomousProtectedRehydrationContext,
  protectedValueDigest,
  digestJsonSync,
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

function calibrationCasesForDomains(domains = AUTONOMOUS_DOMAIN_NAMES) {
  const profiles = new Map(builtinAutonomousValueEvaluatorProfiles().map((profile) => [profile.domain, profile]));
  return domains.flatMap((domain) => {
    const profile = profiles.get(domain);
    const makeEvidence = (value) => ({
      schema: "bioprism-brain-domain-evaluator/0.1",
      domain,
      capability: "calibration-fixture",
      risk_class: "read_only",
      signals: Object.fromEntries(profile.required_signals.map((signal) => [signal, value])),
      references: [],
      limitations: [],
      retention: "value_only_digests_and_signal_scores",
    });
    return [
      { case_id: `${domain}-calibration-positive`, domain, evidence: makeEvidence(1), label: 1, split: "calibration" },
      { case_id: `${domain}-calibration-negative`, domain, evidence: makeEvidence(0), label: 0, split: "calibration" },
      { case_id: `${domain}-calibration-reference-abstained`, domain, evidence: makeEvidence(1), label: null, split: "calibration" },
      { case_id: `${domain}-holdout-positive`, domain, evidence: makeEvidence(1), label: 1, split: "holdout" },
      { case_id: `${domain}-holdout-negative`, domain, evidence: makeEvidence(0), label: 0, split: "holdout" },
    ];
  });
}

function transactionalTextStore() {
  let encoded = null;
  return {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
}

function localRuntime(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline:${request.model}` };
  });
  return runtime;
}

function workflowRuntime(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("workflow HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    const prompt = JSON.stringify(request.messages);
    const stageId = prompt.match(/Execute workflow stage ([A-Za-z0-9_.:-]+) for task/)?.[1] ?? "unknown-stage";
    return {
      structured: {
        stage_id: stageId,
        status: "completed",
        evidence: [`evidence-${stageId}`],
        uncertainty: [],
        notes: `completed ${stageId}`,
        next_actions: [],
      },
    };
  });
  return runtime;
}

function perfectWorkflowEvidence(execution) {
  return {
    stages: execution.blueprint.workflow.stages.map((stage) => ({
      stage_id: stage.id,
      signals: Object.fromEntries(stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  };
}

function workflowPortfolioRequests(domains = AUTONOMOUS_DOMAIN_NAMES) {
  return domains.map((domain, index) => ({
    id: `facade-portfolio-${domain}`,
    task: `private portfolio task for ${domain} must remain transient`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`facade-portfolio-${domains[index - 1]}`] }),
    hints: [`private portfolio hint for ${domain}`],
  }));
}

function portfolioEvidenceRequests(evidencePlan, domains = AUTONOMOUS_DOMAIN_NAMES) {
  return domains.map((domain) => ({
    item_id: `facade-portfolio-${domain}`,
    requests: evidencePlan.requirements
      .filter((requirement) => requirement.domain === domain)
      .map((requirement, index) => ({
        requirement_id: requirement.requirement_id,
        source_id: `facade-evidence-source-${domain}-${index}`,
        request_id: `facade-evidence-request-${domain}-${index}`,
        metadata: { purpose: "bounded-facade-portfolio-evidence" },
      })),
  }));
}

function portfolioEvidenceRuntime() {
  return {
    acquirer: {
      async acquire(context) {
        return {
          transient_value: "caller-owned evidence",
          item_id: context.request.metadata.portfolio_item_id,
          requirement_id: context.requirement.requirement_id,
        };
      },
    },
    projector: {
      project(_value, context) {
        return [{ label: context.requirement.label, kind: "fact", status: "observed" }];
      },
    },
    evaluator: {
      evaluator_id: "facade-portfolio-evaluator",
      evaluator_version: "1",
      evaluate() {
        return {
          evaluator_id: "facade-portfolio-evaluator",
          evaluator_version: "1",
          verdict: "accepted",
          score: 1,
          evidence_digest: "d".repeat(64),
        };
      },
    },
  };
}

async function approvedLaunchAdmission(brain) {
  const profiles = await builtinAutonomousDomainProfiles();
  const ready = { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true };
  const preflight = await brain.launchPreflight({
    availableToolNames: profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name)),
    availableEvidence: profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`))),
    deploymentCapabilities: {
      persistence: ready,
      queue: ready,
      approval_authority: ready,
      external_auth: ready,
      telemetry: ready,
    },
  });
  return brain.admitLaunchPreflight(preflight, { decision: "approve", authorizationDigest: "c".repeat(64) });
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

test("brain facade exposes the provider-free clarification lifecycle across every domain", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("clarification must not contact a provider"); } });
  const agent = new AutonomousAgent(runtime);
  const brain = new AutonomousBrainFacade({ agent });
  const privateContext = "clarification context must remain caller-owned";
  let inferredDomainPlan;

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = tasks[domain];
    const plan = await brain.clarificationPlan({
      task,
      domain,
      context: [{ id: `clarification-${domain}`, content: privateContext }],
    });
    assert.equal(plan.domain, domain);
    assert.equal(plan.authorization, "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions");
    assert.equal(plan.secret_material, "never_returned");
    assert.equal(JSON.stringify(plan).includes(privateContext), false);

    const answers = Object.fromEntries(plan.questions.filter((question) => question.required).map((question) => [
      question.question_id,
      question.answer_kind === "choice" ? question.options[0] : "bounded caller clarification",
    ]));
    const receipt = await brain.resolveClarification(plan, task, answers);
    assert.equal(receipt.status, "resolved", domain);
    assert.equal(receipt.required_answer_count, plan.questions.filter((question) => question.required).length, domain);
    assert.equal(JSON.stringify(receipt).includes("bounded caller clarification"), false);
    const restored = await brain.validateClarification(plan, receipt);
    assert.deepEqual(restored, receipt, domain);

    const recompiled = await brain.recompileClarification(
      plan,
      receipt,
      { task, domain, context: [{ id: `clarification-${domain}`, content: privateContext }] },
      `${task}; clarified output and acceptance criteria are explicit`,
    );
    assert.equal(recompiled.domain, domain);
    assert.equal(recompiled.status, "ready");
    const projection = recompiled.toJSON();
    assert.equal(projection.execution, "not_started; fresh_blueprint_requires_existing_gates");
    assert.equal(projection.secret_material, "never_returned");
    assert.equal(JSON.stringify(projection).includes("clarified output and acceptance criteria"), false);
    const restoredProjection = await brain.validateClarificationRecompile(projection, plan, receipt);
    assert.equal(restoredProjection.recompile_digest, projection.recompile_digest, domain);

    if (domain === "coding") inferredDomainPlan = await brain.clarificationPlan({ task });
  }

  assert.equal(inferredDomainPlan?.domain, "coding");
  const codingPlan = await brain.clarificationPlan({ task: tasks.coding, domain: "coding" });
  const codingReceipt = await brain.resolveClarification(codingPlan, tasks.coding, Object.fromEntries(codingPlan.questions.map((question) => [question.question_id, question.answer_kind === "choice" ? question.options[0] : "complete"])));
  await assert.rejects(
    brain.validateClarification(codingPlan, { ...codingReceipt, resolution_digest: "0".repeat(64) }),
    /digest/,
  );
  await assert.rejects(
    brain.clarificationPlan({ task: tasks.coding, connector: { domain: "coding", operation: "unknown", request: {} } }),
    /does not execute connector inputs|connector/i,
  );
});

test("brain facade binds protected onboarding to model discovery and all-domain inventory", async () => {
  let networkCalls = 0;
  const runtime = new LLMRuntime({
    fetch: async (url, init) => {
      networkCalls += 1;
      assert.equal(String(url), "https://groq.test/openai/v1/models");
      assert.equal(new Headers(init?.headers).get("authorization"), "Bearer facade-inventory-secret");
      return new Response(JSON.stringify({
        data: [{
          id: "facade-discovered-model",
          active: true,
          context_window: 32_000,
          max_output_tokens: 2_000,
          capabilities: [
            "reasoning", "structured_output", "code", "web", "data", "science",
            "biomedical", "operations", "enterprise", "coordination", "multimodal",
            "evaluation",
          ],
        }],
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  const agent = new AutonomousAgent(runtime);
  const brain = new AutonomousBrainFacade({ agent });
  const setup = brain.providerSetup;
  setup.registerProvider("groq", { baseUrl: "https://groq.test/openai/v1" });
  const session = setup.startSession({ sessionId: "facade-inventory-session", ttlMs: 60_000, clock: () => 100 });
  const secret = "facade-inventory-secret";
  setup.collectUserCredential(session, "groq", secret, { ttlMs: 30_000 });

  const discovery = await brain.discoverModels(session, "groq");
  assert.equal(discovery.provider, "groq");
  assert.equal(discovery.models.length, 1);
  assert.equal(discovery.models[0].model, "facade-discovered-model");

  const defaults = {
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 25,
    cost_per_million_tokens: 0,
    reliability: 0.95,
  };
  const candidates = brain.modelCandidates(discovery, defaults);
  assert.equal(candidates.length, 1);
  assert.equal(candidates[0].provider, "groq");

  const inventory = await brain.refreshModelInventory(
    session,
    [{ provider: "groq", defaults }],
    { refreshId: "facade-inventory-refresh" },
  );
  assert.equal(inventory.status, "completed");
  assert.equal(inventory.models.length, 1);
  assert.equal(inventory.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(inventory.domains.every((row) => row.eligible_model_count === 1), true);
  assert.equal(inventory.readiness, "ready");
  assert.doesNotMatch(JSON.stringify({ discovery, candidates, inventory }), /facade-inventory-secret/);
  assert.doesNotMatch(JSON.stringify({ discovery, candidates, inventory }), /api[_-]?key|credential[_-]?value/i);
  assert.equal(networkCalls, 2);

  session.close();
  await assert.rejects(
    () => brain.discoverModels(session, "groq"),
    /closed or expired/,
  );
  await assert.rejects(
    () => brain.refreshModelInventory(session, [{ provider: "groq", defaults }]),
    /closed or expired/,
  );
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

test("brain facade exposes durable task workflows across every built-in domain", async () => {
  const requests = [];
  const runtime = workflowRuntime((request) => requests.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  let expectedDispatches = 0;

  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const task = `Run a bounded ${domain} workflow`;
    const store = new InMemoryAutonomousWorkflowCheckpointStore();
    const first = await brain.runWorkflow(task, {
      checkpointStore: store,
      domain,
      candidates: [model],
      approveProviderCall: true,
      maxStages: 1,
      jobId: `facade-workflow-${domain}-${index}`,
    });
    expectedDispatches += 1;
    assert.equal(first.status, first.total_stage_count > 1 ? "paused" : "completed", domain);
    assert.equal(first.completed_stage_count, 1, domain);
    assert.equal(first.blueprint?.domain_profile.domain, domain, domain);
    assert.equal(JSON.stringify(first.checkpoint).includes(task), false, domain);

    const resumed = await brain.resumeWorkflow(first.job_id, task, {
      checkpointStore: store,
      domain,
      candidates: [model],
      approveProviderCall: true,
      maxStages: 32,
    });
    assert.equal(resumed.status, "completed", domain);
    assert.equal(resumed.completed_stage_count, first.total_stage_count, domain);
    expectedDispatches += first.total_stage_count - 1;
  }

  assert.equal(requests.length, expectedDispatches);
  assert.equal(runtime.providerStatus("offline").successes, expectedDispatches);
});

test("brain facade durable workflow launch admission gates dispatch and resume", async () => {
  const requests = [];
  const runtime = workflowRuntime((request) => requests.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const task = "run an admitted coding workflow";
  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });

  await assert.rejects(
    () => brain.runWorkflowWithLaunchAdmission(task, held, {
      checkpointStore: store,
      domain: "coding",
      candidates: [model],
      approveProviderCall: true,
      maxStages: 1,
      jobId: "held-facade-workflow",
    }),
    /not approved/,
  );
  assert.equal(requests.length, 0);

  const admission = await approvedLaunchAdmission(brain);
  const first = await brain.runWorkflowWithLaunchAdmission(task, admission, {
    checkpointStore: store,
    domain: "coding",
    candidates: [model],
    approveProviderCall: true,
    maxStages: 1,
    jobId: "admitted-facade-workflow",
  });
  assert.equal(first.status, "paused");
  assert.equal(requests.length, 1);

  const resumed = await brain.resumeWorkflowWithLaunchAdmission(first.job_id, task, admission, {
    checkpointStore: store,
    domain: "coding",
    candidates: [model],
    approveProviderCall: true,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(requests.length, first.total_stage_count);

  await assert.rejects(
    () => brain.runWorkflowWithLaunchAdmission("semantic route must be separately admitted", admission, {
      checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
      semanticRouting: { enabled: true, approveProviderCall: true },
      candidates: [model],
      approveProviderCall: true,
    }),
    /provider-free routing/,
  );
  assert.equal(requests.length, first.total_stage_count);
});

test("brain facade exposes evaluator-guided workflow cycles across every built-in domain", async () => {
  const requests = [];
  const runtime = workflowRuntime((request) => requests.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const cycle = await brain.runWorkflowCycle(`Evaluate a bounded ${domain} workflow`, {
      checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
      domain,
      candidates: [model],
      approveProviderCall: true,
      maxReplans: 0,
      cycleId: `facade-cycle-${domain}-${index}`,
      evaluate: async (execution) => ({ evidence: perfectWorkflowEvidence(execution) }),
    });
    assert.equal(cycle.status, "completed", domain);
    assert.equal(cycle.attempts.length, 1, domain);
    assert.equal(cycle.evaluations[0].status, "passed", domain);
    assert.equal(cycle.evaluations[0].reward, 1, domain);
    assert.equal(cycle.final?.blueprint?.domain_profile.domain, domain, domain);
  }

  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  const attempts = requests.length;
  await assert.rejects(
    () => brain.runWorkflowCycleWithLaunchAdmission("held evaluator cycle", held, {
      checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
      domain: "evaluation",
      candidates: [model],
      approveProviderCall: true,
      evaluate: async (execution) => ({ evidence: perfectWorkflowEvidence(execution) }),
    }),
    /not approved/,
  );
  assert.equal(requests.length, attempts);
});

test("brain facade composes the reviewed workflow portfolio lifecycle across every built-in domain", async () => {
  let providerCalls = 0;
  const agent = new AutonomousAgent(localRuntime(() => { providerCalls += 1; }));
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const requests = workflowPortfolioRequests();

  const plan = await brain.planWorkflowPortfolio(requests, { requireAllDomains: true });
  assert.equal(plan.status, "ready");
  assert.equal(plan.coverage.complete, true);
  assert.equal(plan.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual([...new Set(plan.items.map((item) => item.domain))].sort(), [...AUTONOMOUS_DOMAIN_NAMES].sort());
  assert.doesNotMatch(JSON.stringify(plan), /private portfolio task|private portfolio hint/);

  const verified = await brain.verifyWorkflowPortfolio(plan, requests, { requireAllDomains: true });
  assert.equal(verified.status, "verified");
  assert.equal(verified.replayed_item_count, AUTONOMOUS_DOMAIN_NAMES.length);

  const launchAdmission = await approvedLaunchAdmission(brain);
  const execution = await brain.executeWorkflowPortfolioWithLaunchAdmission(requests, launchAdmission, {
    plan,
    approveProviderCall: true,
    maxParallelism: 3,
  });
  assert.equal(execution.status, "completed");
  assert.equal(execution.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(execution.items.every((item) => item.status === "succeeded" && item.run?.status === "completed"));
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(execution), /private portfolio task|private portfolio hint|offline:/);

  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const evidence = await brain.executeWorkflowPortfolioEvidenceWithLaunchAdmission(execution, launchAdmission, {
    evidencePlan,
    items: portfolioEvidenceRequests(evidencePlan),
    runtime: portfolioEvidenceRuntime(),
    maxParallelism: 3,
  });
  assert.equal(evidence.status, "completed");
  assert.equal(evidence.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(evidence.items.every((item) => item.status === "completed"));
  assert.doesNotMatch(JSON.stringify(evidence), /caller-owned evidence|private portfolio task/);

  let checkpoint = null;
  const resumable = await brain.executeWorkflowPortfolioResumable(
    [{ id: "facade-resumable-coding", task: "resume a bounded coding portfolio item", domain: "coding" }],
    {
      jobId: "facade-portfolio-resumable",
      approveProviderCall: true,
      checkpointSink: (value) => { checkpoint = value; },
    },
  );
  assert.equal(resumable.status, "completed");
  assert.ok(checkpoint?.checkpoint_digest?.length === 64);
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length + 1);

  const heldPreflight = await brain.launchPreflight();
  const heldAdmission = brain.admitLaunchPreflight(heldPreflight, { decision: "hold" });
  await assert.rejects(
    () => brain.executeWorkflowPortfolioWithLaunchAdmission(
      [{ id: "held-coding", task: "held portfolio task", domain: "coding" }],
      heldAdmission,
      { approveProviderCall: true },
    ),
    /launch admission is not approved/,
  );
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length + 1, "held admission must prevent dispatch");
});

test("brain facade traces durable workflow start and restart across every built-in domain", async () => {
  const requests = [];
  const runtime = workflowRuntime((request) => requests.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const traceStore = new InMemoryAutonomousRunTraceStore();

  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const task = `Trace a bounded ${domain} workflow`;
    const traced = await brain.runWorkflowWithTrace(task, {
      checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
      traceStore,
      runId: `facade-traced-workflow-${domain}-${index}`,
      domain,
      candidates: [model],
      approveProviderCall: true,
      maxStages: 1,
    });
    assert.ok(["completed", "paused"].includes(traced.execution.status), domain);
    assert.equal(traced.trace.provider_invocations, 1, domain);
    assert.ok(traced.trace.domains.includes(domain), domain);
    assert.equal(JSON.stringify(traced.trace).includes(task), false, domain);
  }

  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const task = "Trace restart recovery for coding";
  const first = await brain.runWorkflow(task, {
    checkpointStore: store,
    domain: "coding",
    candidates: [model],
    approveProviderCall: true,
    maxStages: 1,
    jobId: "facade-trace-restart",
  });
  const resumed = await brain.resumeWorkflowWithTrace(first.job_id, task, {
    checkpointStore: store,
    traceStore,
    runId: "facade-trace-restart-resume",
    domain: "coding",
    candidates: [model],
    approveProviderCall: true,
  });
  assert.equal(resumed.execution.status, "completed");
  assert.equal(resumed.trace.status, "completed");
  assert.equal(resumed.trace.provider_invocations, first.total_stage_count - 1);

  const admission = await approvedLaunchAdmission(brain);
  const admitted = await brain.runWorkflowWithLaunchAdmissionAndTrace("admitted traced workflow", admission, {
    checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
    traceStore,
    runId: "facade-admitted-trace",
    domain: "evaluation",
    candidates: [model],
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.ok(["completed", "paused"].includes(admitted.execution.status));
  assert.equal(admitted.trace.provider_invocations, 1);
});

test("brain facade traces evaluator-guided workflow cycles before and after launch admission", async () => {
  const requests = [];
  const runtime = workflowRuntime((request) => requests.push(request));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const cycle = await brain.runWorkflowCycleWithTrace("trace an evaluated science workflow", {
    checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
    traceStore,
    runId: "facade-cycle-trace",
    domain: "science",
    candidates: [model],
    approveProviderCall: true,
    evaluate: async (execution) => ({ evidence: perfectWorkflowEvidence(execution) }),
  });
  assert.equal(cycle.cycle.status, "completed");
  assert.equal(cycle.trace.status, "completed");
  assert.equal(cycle.trace.provider_invocations, cycle.cycle.final.stage_results.length);
  assert.equal(JSON.stringify(cycle.trace).includes("trace an evaluated science workflow"), false);

  const admission = await approvedLaunchAdmission(brain);
  const admitted = await brain.runWorkflowCycleWithLaunchAdmissionAndTrace("admitted evaluated cycle", admission, {
    checkpointStore: new InMemoryAutonomousWorkflowCheckpointStore(),
    traceStore,
    runId: "facade-admitted-cycle-trace",
    domain: "evaluation",
    candidates: [model],
    approveProviderCall: true,
    evaluate: async (execution) => ({ evidence: perfectWorkflowEvidence(execution) }),
  });
  assert.equal(admitted.cycle.status, "completed");
  assert.equal(admitted.trace.status, "completed");
  assert.equal(admitted.trace.provider_invocations, admitted.cycle.final.stage_results.length);
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

test("brain facade traces approved model-arm execution across every built-in domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const store = new InMemoryAutonomousRunTraceStore();

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = tasks[domain];
    const preview = await brain.modelSelectionPreview({ task, domain });
    const traced = await brain.executeApprovedSelectionWithTrace({ task, domain }, preview, {
      traceStore: store,
      runId: `approved-selection-trace-${domain}`,
    });
    assert.equal(traced.execution.status, "completed", domain);
    assert.equal(traced.execution.run?.selection.selected_model?.provider, "offline", domain);
    assert.equal(traced.trace.status, "completed", domain);
    assert.equal(traced.trace.provider_invocations, 1, domain);
    assert.equal(traced.trace.selection_digests.length > 0, true, domain);
    assert.ok(store.events({ run_id: `approved-selection-trace-${domain}` }).some((event) => event.phase === "model_selection_started"), domain);
    assert.doesNotMatch(JSON.stringify(traced.trace), new RegExp(task.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")), domain);
  }

  const preview = await brain.modelSelectionPreview({ task: tasks.coding, domain: "coding" });
  const admission = await approvedLaunchAdmission(brain);
  const admitted = await brain.executeApprovedSelectionWithLaunchAdmissionAndTrace(
    { task: tasks.coding, domain: "coding" },
    preview,
    admission,
    { traceStore: store, runId: "approved-selection-launch-trace" },
  );
  assert.equal(admitted.execution.status, "completed");
  assert.equal(admitted.trace.status, "completed");

  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold", reason: "selection review pending" });
  const attempts = runtime.providerStatus("offline").attempts;
  await assert.rejects(
    () => brain.executeApprovedSelectionWithLaunchAdmissionAndTrace(
      { task: tasks.coding, domain: "coding" },
      preview,
      held,
      { traceStore: store, runId: "approved-selection-held" },
    ),
    /not approved/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, attempts);
  assert.equal(store.events({ run_id: "approved-selection-held" }).length, 0);
  assert.equal(store.verifyIntegrity().verified, true);
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

test("brain facade exposes automatic decision and replan kernels with route binding across every domain", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const learning = new AutonomousLearningController(agent);

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await brain.executeAutoCycle(
      { task: tasks[domain], domain },
      {
        approveProviderCall: true,
        learning: {
          controller: learning,
          episodeId: `facade-auto-cycle-${domain}`,
          evaluate: () => ({ evaluator_id: "facade-auto-cycle-reviewer", evaluator_version: "1", reward: 0.83, passed: true }),
        },
      },
    );
    assert.equal(result.schema, "bioprism-typescript-autonomous-auto-decision-cycle/0.1", domain);
    assert.equal(result.mode, "single_domain", domain);
    assert.equal(result.status, "completed", domain);
    assert.equal(result.next_action, "complete", domain);
    assert.equal(result.route.primary_domain, domain, domain);
    assert.equal(result.cycle.settlement.episode.status, "settled", domain);
  }

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await brain.executeAutoReplanCycle(
      { task: tasks[domain], domain },
      {
        approveProviderCall: true,
        maxReplans: 0,
        evaluate: () => ({ evaluator_id: "facade-auto-replan-reviewer", evaluator_version: "1", reward: 0.84, passed: true, replan_requested: false }),
      },
    );
    assert.equal(result.schema, "bioprism-typescript-autonomous-auto-replan-cycle/0.1", domain);
    assert.equal(result.mode, "single_domain", domain);
    assert.equal(result.status, "completed", domain);
    assert.equal(result.next_action, "complete", domain);
    assert.equal(result.route.primary_domain, domain, domain);
    assert.equal(result.cycle.attempts.length, 1, domain);
  }

  const cross = await brain.executeAutoCycle(
    { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
    {
      approveProviderCall: true,
      synthesize: false,
      maxParallelChildren: 2,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
        { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
      ],
    },
  );
  assert.equal(cross.mode, "cross_domain");
  assert.ok(["completed", "children_completed"].includes(cross.status));
  assert.equal(cross.cycle.run.child_runs.length, 2);

  const crossReplan = await brain.executeAutoReplanCycle(
    { task: "research a biomedical neuroscience experiment with patient EEG evidence", allow_cross_domain: true },
    {
      approveProviderCall: true,
      synthesize: false,
      maxParallelChildren: 2,
      maxReplans: 0,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "review biomedical evidence" },
        { id: "neuro", domain: "neuroscience", task: "analyze EEG limitations" },
      ],
      evaluate: () => ({ evaluator_id: "facade-cross-replan-reviewer", evaluator_version: "1", reward: 0.82, passed: true, replan_requested: false, rewards: {} }),
    },
  );
  assert.equal(crossReplan.mode, "cross_domain");
  assert.equal(crossReplan.status, "completed");
  assert.equal(crossReplan.cycle.final.run.child_runs.length, 2);

  const admission = await approvedLaunchAdmission(brain);
  const admitted = await brain.executeAutoCycleWithLaunchAdmission(
    { task: tasks.coding, domain: "coding" },
    admission,
    { approveProviderCall: true },
  );
  assert.equal(admitted.status, "completed");

  const admittedReplan = await brain.executeAutoReplanCycleWithLaunchAdmission(
    { task: tasks.science, domain: "science" },
    admission,
    {
      approveProviderCall: true,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "facade-admitted-replan-reviewer", evaluator_version: "1", reward: 0.9, passed: true, replan_requested: false }),
    },
  );
  assert.equal(admittedReplan.status, "completed");

  await assert.rejects(
    () => brain.executeAutoCycle({ task: tasks.coding, domain: "coding" }, { routeOverride: {} }),
    /owns routeOverride/,
  );
  await assert.rejects(
    () => brain.executeAutoCycleWithLaunchAdmission(
      { task: tasks.coding, domain: "coding" },
      admission,
      { semanticRouting: { enabled: true, approveProviderCall: true }, approveProviderCall: true },
    ),
    /requires provider-free routing/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, runtime.providerStatus("offline").successes);
});

test("brain facade batches automatic evaluator cycles with bounded admission and deterministic accounting", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ task: tasks[domain], domain }));

  const automatic = await brain.executeAutoCycleBatch(inputs, {
    maxParallelism: 3,
    cycle: (_input, index) => ({ approveProviderCall: true, maxOutputTokens: 768 + index }),
  });
  assert.equal(automatic.schema, "bioprism-typescript-autonomous-brain-auto-cycle-batch/0.1");
  assert.equal(automatic.status, "completed");
  assert.equal(automatic.completed_count, inputs.length);
  assert.equal(automatic.failed_count, 0);
  assert.equal(automatic.omitted_count, 0);
  assert.deepEqual(automatic.items.map((item) => item.index), [...Array(inputs.length).keys()]);
  assert.ok(automatic.items.every((item, index) => item.status === "succeeded" && item.execution.route.primary_domain === inputs[index].domain));
  assert.match(automatic.batch_digest, /^[0-9a-f]{64}$/);
  assert.doesNotMatch(automatic.batch_digest, /debug and verify|offline:offline-model/);

  const replans = await brain.executeAutoReplanCycleBatch(inputs, {
    maxParallelism: 3,
    replan: (input) => ({
      approveProviderCall: true,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: `batch-${input.domain}-reviewer`, evaluator_version: "1", reward: 0.86, passed: true, replan_requested: false }),
    }),
  });
  assert.equal(replans.schema, "bioprism-typescript-autonomous-brain-auto-replan-batch/0.1");
  assert.equal(replans.status, "completed");
  assert.equal(replans.completed_count, inputs.length);
  assert.deepEqual(replans.items.map((item) => item.index), [...Array(inputs.length).keys()]);
  assert.ok(replans.items.every((item) => item.status === "succeeded" && item.execution.cycle.attempts.length === 1));
  assert.match(replans.batch_digest, /^[0-9a-f]{64}$/);

  const admission = await approvedLaunchAdmission(brain);
  const admitted = await brain.executeAutoCycleBatchWithLaunchAdmission(inputs.slice(0, 3), admission, {
    maxParallelism: 2,
    cycle: { approveProviderCall: true },
  });
  assert.equal(admitted.status, "completed");
  assert.equal(admitted.completed_count, 3);

  const admittedReplans = await brain.executeAutoReplanCycleBatchWithLaunchAdmission(inputs.slice(0, 2), admission, {
    maxParallelism: 2,
    replan: {
      approveProviderCall: true,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "admitted-batch-reviewer", evaluator_version: "1", reward: 0.9, passed: true, replan_requested: false }),
    },
  });
  assert.equal(admittedReplans.status, "completed");
  assert.equal(admittedReplans.completed_count, 2);

  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  const beforeHeld = runtime.providerStatus("offline").attempts;
  await assert.rejects(
    () => brain.executeAutoCycleBatchWithLaunchAdmission(inputs.slice(0, 2), held, { cycle: { approveProviderCall: true } }),
    /not approved/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, beforeHeld);

  const stopped = await brain.executeAutoCycleBatch(inputs.slice(0, 3), {
    maxParallelism: 1,
    stopOnError: true,
    cycle: { approveProviderCall: false },
  });
  assert.equal(stopped.status, "failed");
  assert.equal(stopped.items[0].status, "refused");
  assert.deepEqual(stopped.items.slice(1).map((item) => item.status), ["omitted", "omitted"]);

  const conflict = await brain.executeAutoCycleBatch([inputs[0]], { cycle: { domain: "science" } });
  assert.equal(conflict.status, "failed");
  assert.equal(conflict.items[0].status, "refused");
  await assert.rejects(
    () => brain.executeAutoCycleBatchWithLaunchAdmission(
      inputs.slice(0, 1),
      admission,
      { cycle: { semanticRouting: { enabled: true, approveProviderCall: true } } },
    ),
    /requires provider-free routing/,
  );
});

test("brain facade traces and resumes automatic evaluator batches without provider replay", async () => {
  const runtime = localRuntime();
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });
  const inputs = [
    { task: tasks.science, domain: "science" },
    { task: tasks.biomedical, domain: "biomedical" },
  ];
  const cyclePolicy = {
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "trace-resume-reviewer", evaluator_version: "1", reward: 0.9, passed: true, replan_requested: false }),
  };
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.executeAutoCycleBatchWithTrace(inputs, {
    maxParallelism: 1,
    cycle: cyclePolicy,
    traceStore,
    runId: "automatic-cycle-trace",
  });
  assert.equal(traced.schema, "bioprism-typescript-autonomous-brain-traced-auto-cycle-batch/0.1");
  assert.equal(traced.batch.status, "completed");
  assert.equal(traced.trace.status, "completed");
  assert.equal(traced.trace.provider_invocations, inputs.length);
  assert.doesNotMatch(JSON.stringify(traced), /design a reproducible experiment|review biomedical evidence/);
  assert.ok(traceStore.events({ run_id: "automatic-cycle-trace" }).some((event) => event.route_digest));

  const checkpoints = [];
  const first = await brain.executeAutoCycleBatchResumable(inputs, {
    jobId: "automatic-cycle-resume",
    maxParallelism: 1,
    cycle: cyclePolicy,
    checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
  });
  assert.equal(first.status, "completed");
  const beforeResume = runtime.providerStatus("offline").attempts;
  const resumed = await brain.executeAutoCycleBatchResumable(inputs, {
    jobId: "automatic-cycle-resume",
    maxParallelism: 1,
    cycle: cyclePolicy,
    checkpoint: checkpoints.at(-1),
    rehydrateCycle: (context) => first.items[context.index].execution,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(runtime.providerStatus("offline").attempts, beforeResume);
  assert.deepEqual(resumed.items.map((item) => item.status), ["succeeded", "succeeded"]);

  const replanPolicy = {
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "trace-replan-reviewer", evaluator_version: "1", reward: 0.88, passed: true, replan_requested: false }),
  };
  const replans = await brain.executeAutoReplanCycleBatchResumable(inputs.slice(0, 1), {
    jobId: "automatic-replan-resume",
    maxParallelism: 1,
    replan: replanPolicy,
  });
  assert.equal(replans.status, "completed");
  const replannedTrace = await brain.executeAutoReplanCycleBatchWithTrace(inputs.slice(0, 1), {
    maxParallelism: 1,
    replan: replanPolicy,
    traceStore: new InMemoryAutonomousRunTraceStore(),
    runId: "automatic-replan-trace",
  });
  assert.equal(replannedTrace.schema, "bioprism-typescript-autonomous-brain-traced-auto-replan-batch/0.1");
  assert.equal(replannedTrace.batch.status, "completed");
  assert.equal(replannedTrace.trace.status, "completed");
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

test("brain facade exposes evaluator calibration admission and restart controls across every domain", async () => {
  const report = new AutonomousEvaluatorCalibrationHarness(
    AutonomousValueEvaluatorRegistry.withBuiltinProfiles(),
  ).run({
    cases: calibrationCasesForDomains(),
    bins: 5,
    minCalibrationCasesPerDomain: 2,
    minHoldoutCasesPerDomain: 2,
    maxExpectedCalibrationError: 0.01,
    maxBrierScore: 0.01,
  });
  const store = new InMemoryAutonomousEvaluatorCalibrationStore();
  const registry = new AutonomousEvaluatorCalibrationRegistry();
  const persistence = new AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(registry, store);
  const agent = new AutonomousAgent(new LLMRuntime(), {
    evaluatorCalibrationRegistry: registry,
    evaluatorCalibrationPersistence: persistence,
  });
  const brain = new AutonomousBrainFacade({ agent });

  const imported = brain.registerEvaluatorCalibration(report);
  assert.equal(imported.created, true);
  assert.deepEqual(brain.evaluatorCalibrationReport(report.report_digest), report);
  assert.equal(brain.evaluatorCalibrationReports({ decision: "admit_learning" }).length, 1);
  assert.deepEqual(brain.validateEvaluatorCalibration(report), report);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    assert.equal(brain.evaluatorCalibrationAdmission(report, domain).decision, "admit_learning", domain);
  }

  const flushed = await brain.flushEvaluatorCalibration();
  assert.equal(flushed.reports.length, 1);
  assert.doesNotMatch(JSON.stringify(flushed), /calibration-positive|calibration-fixture|signals/);

  const restoredRegistry = new AutonomousEvaluatorCalibrationRegistry();
  const restoredPersistence = new AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(restoredRegistry, store);
  const restoredAgent = new AutonomousAgent(new LLMRuntime(), {
    evaluatorCalibrationRegistry: restoredRegistry,
    evaluatorCalibrationPersistence: restoredPersistence,
  });
  const restoredBrain = new AutonomousBrainFacade({ agent: restoredAgent });
  const restored = await restoredBrain.restoreEvaluatorCalibration();
  assert.equal(restored?.reports.length, 1);
  assert.deepEqual(restoredBrain.evaluatorCalibrationReport(report.report_digest), report);

  const sparse = new AutonomousEvaluatorCalibrationHarness(
    AutonomousValueEvaluatorRegistry.withBuiltinProfiles(),
  ).run({ cases: calibrationCasesForDomains(["coding"]), domains: AUTONOMOUS_DOMAIN_NAMES });
  assert.equal(brain.evaluatorCalibrationAdmission(sparse, "science").decision, "hold_learning");
  await assert.rejects(
    new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime()) }).flushEvaluatorCalibration(),
    /evaluator calibration registry is not configured/,
  );
});

test("brain facade exposes explicit adaptive settlement and per-store restart controls across every domain", async () => {
  const learnerStore = transactionalTextStore();
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0.2, seed: 23 } });
  const learnerPersistence = new AutonomousOnlineLearnerPersistenceCoordinator(
    learner,
    new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(learnerStore),
  );
  const toolStore = transactionalTextStore();
  const toolPersistence = new TransactionalJsonAutonomousToolSelectionPersistence(toolStore);
  const promptRegistry = builtinAutonomousPromptRegistry();
  const promptStore = transactionalTextStore();
  const promptPersistence = new TransactionalJsonAutonomousPromptLearningSnapshotPersistence(promptStore);
  const promptCoordinator = new AutonomousPromptLearningPersistenceCoordinator(promptRegistry, { persistence: promptPersistence });
  const decisionStore = new InMemoryAutonomousDecisionCycleStateStore();
  const decisionSnapshotStore = transactionalTextStore();
  const decisionPersistence = new AutonomousDecisionCyclePersistenceCoordinator(decisionStore, decisionSnapshotStore);
  const agent = new AutonomousAgent(localRuntime(), {
    learner,
    learnerPersistence,
    promptLearningCoordinator: promptCoordinator,
    toolSelectionPersistence: toolPersistence,
    decisionCyclePersistence: decisionPersistence,
  });
  agent.registerModel(model);
  const brain = new AutonomousBrainFacade({ agent });

  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const reward = await brain.recordEvaluatorReward(`offline/${domain}-model`, 0.5 + index / 100, {
      outcomeDigest: `${String(index + 1).padStart(2, "0")}${"a".repeat(62)}`,
    });
    assert.equal(reward.generation, index + 1, domain);
    const toolState = brain.recordToolSelectionReward({
      domain,
      capability: "read_only_analysis",
      tool: `fixture_${domain}`,
      reward: 0.5 + index / 100,
      outcomeDigest: `${String(index + 1).padStart(2, "0")}${"b".repeat(62)}`,
    });
    assert.equal(toolState.generation, index + 1, domain);

    const execution = await brain.execute({ task: tasks[domain], domain }, { approveProviderCall: true });
    assert.equal(execution.status, "completed", domain);
    assert.ok(execution.run, domain);
    const selections = brain.promptLearningSelections(execution.run);
    assert.equal(selections.length, 1, domain);
    const settled = await brain.settlePromptLearning(selections[0], {
      armId: selections[0].armIds[0],
      evaluatorId: `${domain}-facade-rubric`,
      evaluatorVersion: "1",
      reward: 0.8,
      passed: true,
      outcomeDigest: digestJsonSync({ domain, selection: selections[0].selectionDigest }),
    });
    assert.equal(settled.status, "settled", domain);
  }

  const learnerSnapshot = await brain.flushOnlineLearning();
  assert.equal(learnerSnapshot.state.generation, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(learnerSnapshot), /debug and verify|credential|response/);
  const toolSnapshot = await brain.flushToolSelection();
  assert.equal(toolSnapshot.state.arms.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(toolSnapshot), /debug and verify|fixture_.*task/);
  const promptSnapshot = await brain.flushPromptLearning();
  assert.equal(promptSnapshot.state.generation, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(promptSnapshot), /debug and verify|offline:/);

  const cycle = await brain.executeCycle(
    { task: tasks.coding, domain: "coding" },
    { approveProviderCall: true, cycle: { cycleId: "facade-decision-cycle", decisionStateStore: decisionStore } },
  );
  assert.equal(cycle.status, "completed", "decision-cycle persistence must be reachable through the facade");
  const decisionSnapshot = await brain.flushDecisionCyclePersistence();
  assert.equal(decisionSnapshot.cycles, 1);
  assert.equal(decisionSnapshot.terminal_cycles, 1);
  assert.doesNotMatch(JSON.stringify(decisionSnapshot), /debug and verify|offline:/);

  const restoredLearner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0.2, seed: 23 } });
  const restoredLearnerPersistence = new AutonomousOnlineLearnerPersistenceCoordinator(
    restoredLearner,
    new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(learnerStore),
  );
  const restoredPromptCoordinator = new AutonomousPromptLearningPersistenceCoordinator(promptRegistry, {
    persistence: new TransactionalJsonAutonomousPromptLearningSnapshotPersistence(promptStore),
  });
  const restoredDecisionStore = new InMemoryAutonomousDecisionCycleStateStore();
  const restoredDecisionPersistence = new AutonomousDecisionCyclePersistenceCoordinator(restoredDecisionStore, decisionSnapshotStore);
  const restoredAgent = new AutonomousAgent(localRuntime(), {
    learner: restoredLearner,
    learnerPersistence: restoredLearnerPersistence,
    promptLearningCoordinator: restoredPromptCoordinator,
    toolSelectionPersistence: new TransactionalJsonAutonomousToolSelectionPersistence(toolStore),
    decisionCyclePersistence: restoredDecisionPersistence,
  });
  const restoredBrain = new AutonomousBrainFacade({ agent: restoredAgent });
  assert.deepEqual(await restoredBrain.restoreOnlineLearning(), learnerSnapshot);
  assert.deepEqual(await restoredBrain.restoreToolSelection(), toolSnapshot);
  assert.deepEqual((await restoredBrain.restorePromptLearning()).state, promptSnapshot.state);
  const restoredDecision = await restoredBrain.restoreDecisionCyclePersistence();
  assert.equal(restoredDecision.cycles, 1);
  assert.equal(restoredDecision.terminal_cycles, 1);
  assert.throws(() => new AutonomousAgent(new LLMRuntime(), { learner: restoredLearner, learnerPersistence }), /bound to the supplied learner/);
  await assert.rejects(
    new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime()) }).flushOnlineLearning(),
    /no AutonomousOnlineLearner/,
  );
});
