import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainBatchJobController,
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
  LLMRuntime,
  createBuiltinAutonomousConnectorRuntime,
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
