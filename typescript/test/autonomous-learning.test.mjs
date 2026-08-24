import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousEvaluatorMesh,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowEvaluator,
  AutonomousEvaluatorCalibrationHarness,
  AutonomousValueEvaluatorRegistry,
  CredentialStore,
  InMemoryAutonomousLearningEpisodeStore,
  InMemoryAutonomousLearningSettlementReceiptStore,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  InMemoryAutonomousLearningStateStore,
  InMemoryAutonomousLearningTrajectoryStore,
  AutonomousLearningPersistenceCoordinator,
  JsonAutonomousLearningStatePersistence,
  TransactionalJsonAutonomousLearningStatePersistence,
  WebStorageAutonomousLearningSnapshotTextStore,
  validateAutonomousLearningStateSnapshot,
  AutonomousLearningSettlementReceiptPersistenceCoordinator,
  JsonAutonomousLearningSettlementReceiptPersistence,
  TransactionalJsonAutonomousLearningSettlementReceiptPersistence,
  WebStorageAutonomousLearningSettlementReceiptTextStore,
  validateAutonomousLearningSettlementReceiptSnapshot,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  AutonomousWorkflowExecutor,
  InMemoryAutonomousEpisodicMemory,
  InMemoryAutonomousModelHealthStore,
  builtinAutonomousDomainEvaluatorProfiles,
  builtinAutonomousDomainProfiles,
  builtinAutonomousValueEvaluatorProfiles,
  openaiCompatibleProvider,
  digestJson,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), { status, headers: { "content-type": "application/json" } });
}

function candidate() {
  return {
    provider: "learning-provider",
    model: "learning-model",
    capabilities: ["reasoning", "code", "science", "biomedical", "coordination", "data", "web", "operations", "enterprise", "multimodal", "evaluation", "structured_output"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 10,
    reliability: 0.95,
  };
}

function workflowStagePayload(init) {
  let body = {};
  try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
  const stageId = JSON.stringify(body.messages ?? []).match(/Execute workflow stage ([A-Za-z0-9_.:-]+)/)?.[1] ?? "stage";
  return { stage_id: stageId, status: "completed", evidence: [`evidence-${stageId}`], uncertainty: [], notes: `verified ${stageId}`, next_actions: [] };
}

async function learningAgent(memory) {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("learning-provider", "https://learning.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), ...(memory ? { memoryStore: memory } : {}) });
  agent.registerModel(candidate());
  return agent;
}

function completeExecution(blueprint) {
  return {
    schema: "bioprism-typescript-autonomous-workflow-execution/0.1",
    status: "completed",
    job_id: "evaluation-job",
    blueprint,
    checkpoint: null,
    events: [],
    stage_results: [],
    completed_stage_count: blueprint.workflow.stages.length,
    total_stage_count: blueprint.workflow.stages.length,
    recovery: "caller_rehydrates_task_and_credentials",
    retention: "provider_responses_local;checkpoint_metadata_only",
  };
}

function perfectEvidence(blueprint) {
  return blueprint.workflow.stages.map((stage) => ({
    stage_id: stage.id,
    signals: Object.fromEntries(stage.evaluator_signals.map((signal) => [signal, 1])),
  }));
}

function codingCalibrationCases({ miscalibrated = false } = {}) {
  const profile = builtinAutonomousValueEvaluatorProfiles().find((candidate) => candidate.domain === "coding");
  const positiveSignals = Object.fromEntries(profile.required_signals.map((signal) => [signal, 1]));
  const negativeSignals = Object.fromEntries(profile.required_signals.map((signal) => [signal, 0]));
  return [
    { case_id: "learning-calibration-positive", domain: "coding", evidence: { domain: "coding", capability: "code", risk_class: "review_required", signals: positiveSignals }, label: 1, split: "calibration" },
    { case_id: "learning-calibration-negative", domain: "coding", evidence: { domain: "coding", capability: "code", risk_class: "review_required", signals: negativeSignals }, label: 0, split: "calibration" },
    { case_id: "learning-holdout-positive", domain: "coding", evidence: { domain: "coding", capability: "code", risk_class: "review_required", signals: positiveSignals }, label: miscalibrated ? 0 : 1, split: "holdout" },
    { case_id: "learning-holdout-negative", domain: "coding", evidence: { domain: "coding", capability: "code", risk_class: "review_required", signals: negativeSignals }, label: 0, split: "holdout" },
  ];
}

function codingCalibrationReport(options = {}) {
  return new AutonomousEvaluatorCalibrationHarness(AutonomousValueEvaluatorRegistry.withBuiltinProfiles()).run({
    domains: ["coding"],
    cases: codingCalibrationCases(options),
    minCalibrationCasesPerDomain: 2,
    minHoldoutCasesPerDomain: 2,
    maxExpectedCalibrationError: 0.1,
    maxBrierScore: 0.1,
    requireAllDomains: false,
  });
}

test("every built-in domain has an explicit evaluator signal contract", async () => {
  const profiles = await builtinAutonomousDomainEvaluatorProfiles();
  assert.equal(profiles.length, 12);
  assert.deepEqual(new Set(profiles.map((profile) => profile.domain)), new Set([
    "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation",
  ]));
  for (const profile of profiles) {
    assert.ok(profile.evaluator_id.includes(profile.domain));
    assert.ok(profile.required_signals.length >= 1);
    assert.equal(profile.execution, "caller_declared_signal_scoring_only");
    assert.equal(profile.retention, "value_only;task_prompt_response_credentials_and_evidence_not_retained");
  }
});

test("workflow evaluation requires explicit evidence and preserves only digests", async () => {
  const agent = await learningAgent();
  const blueprint = (await agent.blueprint("Implement and verify a bounded change.", { domain: "coding" })).blueprint;
  assert.ok(blueprint);
  const evaluator = new AutonomousWorkflowEvaluator();
  const complete = await evaluator.evaluate(completeExecution(blueprint), { stages: perfectEvidence(blueprint) });
  assert.equal(complete.status, "passed");
  assert.equal(complete.reward, 1);
  assert.equal(complete.missing_signals.length, 0);
  assert.equal(complete.evaluation_digest.length, 64);
  assert.equal(complete.evidence_digest.length, 64);

  const partial = await evaluator.evaluate(completeExecution(blueprint), {
    stages: [{ stage_id: blueprint.workflow.stages[0].id, signals: { [blueprint.workflow.stages[0].evaluator_signals[0]]: 1, unreviewed_signal: 1 } }],
  });
  assert.equal(partial.status, "incomplete");
  assert.ok(partial.missing_signals.length > 0);
  assert.deepEqual(partial.rejected_signals, [`${blueprint.workflow.stages[0].id}/unreviewed_signal`]);
  assert.equal(JSON.stringify(complete).includes("Implement and verify"), false);
});

test("the evaluator contract is executable for all twelve domain workflows", async () => {
  const agent = await learningAgent();
  const profiles = await builtinAutonomousDomainProfiles();
  const evaluator = new AutonomousWorkflowEvaluator();
  for (const profile of profiles) {
    const blueprint = (await agent.blueprint(`Review a ${profile.domain} workflow with explicit evidence.`, { domain: profile.domain })).blueprint;
    const result = await evaluator.evaluate(completeExecution(blueprint), { stages: perfectEvidence(blueprint) });
    assert.equal(result.domain, profile.domain);
    assert.equal(result.status, "passed", profile.domain);
    assert.equal(result.reward, 1, profile.domain);
  }
});

test("workflow evidence digests are canonical, order-independent, and tamper-bound", async () => {
  const agent = await learningAgent();
  const blueprint = (await agent.blueprint("Verify canonical evaluator evidence.", { domain: "coding" })).blueprint;
  const evaluator = new AutonomousWorkflowEvaluator();
  const execution = completeExecution(blueprint);
  const stages = perfectEvidence(blueprint);
  const first = await evaluator.evaluate(execution, { stages });
  const reordered = await evaluator.evaluate(execution, { stages: [...stages].reverse(), evidence_digest: first.evidence_digest });
  assert.equal(reordered.evidence_digest, first.evidence_digest);
  assert.equal(reordered.context_digest, blueprint.learning_context_digest);
  assert.ok(reordered.learning_context);
  await assert.rejects(() => evaluator.evaluate(execution, { stages, evidence_digest: "a".repeat(64) }), /does not match the normalized evidence packet/);
});

test("independent evaluator mesh accepts agreement and refuses disagreement for every domain", async () => {
  const agent = await learningAgent();
  const members = [
    { evaluator_id: "mesh-reviewer-a", evaluator_version: "1", evaluate: async () => ({ evaluator_id: "mesh-reviewer-a", evaluator_version: "1", reward: 0.9, passed: true, evidence_digest: "a".repeat(64) }) },
    { evaluator_id: "mesh-reviewer-b", evaluator_version: "1", evaluate: async () => ({ evaluator_id: "mesh-reviewer-b", evaluator_version: "1", reward: 0.86, passed: true, evidence_digest: "b".repeat(64) }) },
  ];
  const mesh = new AutonomousEvaluatorMesh({ members, maxRewardSpread: 0.1 });
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const blueprint = (await agent.blueprint(`Evaluate ${profile.domain} with independent reviewers.`, { domain: profile.domain })).blueprint;
    const detailed = await mesh.evaluateDetailed({ ...completeExecution(blueprint), response: { role: "assistant", content: "private output" } });
    assert.equal(detailed.status, "accepted", profile.domain);
    assert.equal(detailed.reward, 0.88, profile.domain);
    assert.equal(detailed.member_results.length, 2, profile.domain);
    assert.equal(JSON.stringify(detailed).includes("private output"), false);
    const reward = await mesh.evaluate(completeExecution(blueprint));
    assert.equal(reward.reward, 0.88, profile.domain);
    assert.equal(reward.passed, true, profile.domain);
  }
  const disagreement = new AutonomousEvaluatorMesh({ members: [
    members[0],
    { evaluator_id: "mesh-reviewer-c", evaluator_version: "1", evaluate: async () => ({ evaluator_id: "mesh-reviewer-c", evaluator_version: "1", reward: 0.2, passed: false, failed: true, failure_class: "quality_gate", evidence_digest: "c".repeat(64) }) },
  ] });
  const blueprint = (await agent.blueprint("Test evaluator disagreement.", { domain: "coding" })).blueprint;
  const refused = await disagreement.evaluateDetailed(completeExecution(blueprint));
  assert.equal(refused.status, "disagreement");
  assert.equal(refused.reward, null);
  await assert.rejects(() => disagreement.evaluate(completeExecution(blueprint)), /refused learning credit/);
  const memberError = new AutonomousEvaluatorMesh({ members: [members[0], { evaluator_id: "mesh-error", evaluator_version: "1", evaluate: async () => { throw new Error("private evaluator transport detail"); } }] });
  const errored = await memberError.evaluateDetailed(completeExecution(blueprint));
  assert.equal(errored.status, "member_error");
  assert.equal(JSON.stringify(errored).includes("private evaluator transport detail"), false);
});

test("learning episodes rehydrate by digest and settle through the local bandit", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const controller = new AutonomousLearningController(agent, { episodes });
  const run = await agent.run("Implement and verify this learning test.", { domain: "coding", approveProviderCall: true });
  const episode = await controller.prepareRun(run, { episodeId: "episode-local-1", planRefinementDigest: "a".repeat(64) });
  assert.equal(episode.status, "pending");
  assert.equal(episode.run.provider, "learning-provider");
  assert.equal(episode.plan_refinement_digest, "a".repeat(64));
  assert.equal(Object.prototype.hasOwnProperty.call(episode, "response"), false);
  assert.equal(JSON.stringify(episode).includes("verified learning response"), false);
  const replayed = await controller.prepareRun(run, { episodeId: "episode-local-1", planRefinementDigest: "a".repeat(64) });
  assert.deepEqual(replayed, episode);
  const settlement = await controller.settleRun("episode-local-1", {
    evaluator_id: "coding-reviewer",
    evaluator_version: "1",
    reward: 0.9,
    passed: true,
    evidence_digest: "a".repeat(64),
  });
  assert.equal(settlement.remote, false);
  assert.equal(settlement.assessment.reward, 0.9);
  assert.equal(settlement.episode.status, "settled");
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal(episodes.pending().length, 0);
  await assert.rejects(() => controller.settleRun("episode-local-1", { evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 0.8, passed: true }), /already been settled/);
});

test("explicit evaluator settlement feeds model quality health without confusing transport success", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(workflowStagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("learning-provider", "https://learning.test", { requiresCredential: false }));
  const health = new InMemoryAutonomousModelHealthStore();
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  agent.registerModel(candidate());
  const controller = new AutonomousLearningController(agent);
  const run = await agent.run("Evaluate quality feedback separately from transport.", { domain: "coding", approveProviderCall: true });
  const episode = await controller.prepareRun(run, { episodeId: "quality-health-1" });
  const settlement = await controller.settleRun(episode.episode_id, {
    evaluator_id: "quality-reviewer",
    evaluator_version: "1",
    reward: 0.2,
    passed: false,
    failure_class: "answer_quality",
    evidence_digest: "b".repeat(64),
  });
  assert.equal(settlement.model_quality?.status, "recorded");
  assert.equal(settlement.model_quality?.reward, 0.2);
  assert.equal(settlement.model_quality?.provider, "learning-provider");
  const model = health.health({ model: "learning-model" })[0];
  assert.equal(model?.attempts, 1);
  assert.equal(model?.successes, 1);
  assert.equal(model?.quality_observations, 1);
  assert.equal(model?.quality_mean, 0.2);
  assert.equal(JSON.stringify(settlement).includes("Evaluate quality feedback"), false);
});

test("high-level runLearning evaluates and settles every built-in domain with replay-safe model credit", async () => {
  const agent = await learningAgent();
  const runEvaluator = new AutonomousEvaluatorMesh({
    members: [{
      evaluator_id: "all-domain-quality-reviewer",
      evaluator_version: "1",
      evaluate: async (result) => ({
        evaluator_id: "all-domain-quality-reviewer",
        evaluator_version: "1",
        reward: result.status === "completed" ? 0.75 : 0,
        passed: result.status === "completed",
        failed: result.status !== "completed",
        evidence_digest: "e".repeat(64),
      }),
    }, {
      evaluator_id: "all-domain-quality-reviewer-2",
      evaluator_version: "1",
      evaluate: async (result) => ({ evaluator_id: "all-domain-quality-reviewer-2", evaluator_version: "1", reward: result.status === "completed" ? 0.75 : 0, passed: result.status === "completed", failed: result.status !== "completed", evidence_digest: "e".repeat(64) }),
    }],
  });
  const controller = new AutonomousLearningController(agent, { runEvaluator });
  const profiles = await builtinAutonomousDomainProfiles();
  const settled = [];
  for (const profile of profiles) {
    const result = await controller.runLearning(`Produce and verify a ${profile.domain} result.`, {
      episodeId: `high-level-${profile.domain}`,
      run: { domain: profile.domain, approveProviderCall: true },
    });
    assert.equal(result.status, "settled", profile.domain);
    assert.equal(result.evaluation.reward, 0.75, profile.domain);
    assert.equal(result.settlement.episode.status, "settled", profile.domain);
    assert.equal(JSON.stringify(result.settlement).includes("verified"), false, profile.domain);
    settled.push(result);
  }
  assert.equal(agent.learner.snapshot().generation, profiles.length);
  const contextualPulls = (agent.learner.snapshot().contextual_states ?? []).flatMap((state) => state.arms).reduce((sum, arm) => sum + arm.pulls, 0);
  assert.equal(contextualPulls, profiles.length);

  const beforeReplay = agent.learner.snapshot();
  const replay = await controller.evaluateAndSettleRun(settled[0].run);
  assert.equal(replay.status, "settled");
  assert.deepEqual(agent.learner.snapshot(), beforeReplay);
  assert.equal(replay.settlement.episode.settlement.settlement_digest, settled[0].settlement.episode.settlement.settlement_digest);
});

test("provider-planned learning settles planner quality and execution as one replay-safe transaction", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (planningMessage) {
        const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
        const ids = contract.stage_catalogue.map((row) => row.id);
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: ids.slice(0, 1), review_required: false, confidence: 0.96, abstain: false }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "planned learning execution" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("planned-learning", "https://planned-learning.test", { requiresCredential: false }));
  const health = new InMemoryAutonomousModelHealthStore();
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  agent.registerModel({ ...candidate(), provider: "planned-learning", model: "planner-executor", capabilities: ["reasoning", "code", "structured_output", "planning"] });
  const controller = new AutonomousLearningController(agent);
  const planned = await agent.planAndRun("Plan and verify a coding change.", {
    domain: "coding",
    planning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    learning: controller,
    learningEpisodeId: "planned-learning-episode",
  });
  assert.equal(planned.status, "completed");
  const settled = await controller.evaluateAndSettlePlanAndRun(planned, {
    evaluator: async (run) => ({ evaluator_id: "execution-reviewer", evaluator_version: "1", reward: run.status === "completed" ? 0.72 : 0, passed: run.status === "completed", evidence_digest: "a".repeat(64) }),
    plannerEvaluator: async (plan) => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: plan.status === "completed" ? 0.88 : 0, passed: plan.status === "completed", evidence_digest: "b".repeat(64) }),
  });
  assert.equal(settled.status, "settled");
  assert.equal(settled.planner_settlement?.status, "settled");
  assert.equal(settled.execution_settlement?.episode.status, "settled");
  assert.equal(settled.planner_settlement?.model_quality?.reward, 0.88);
  assert.equal(health.health({ model: "planner-executor" })[0]?.quality_observations, 2);
  const beforeReplay = agent.learner.snapshot();
  const replay = await controller.evaluateAndSettlePlanAndRun(planned, {
    evaluator: async () => ({ evaluator_id: "execution-reviewer", evaluator_version: "1", reward: 0.72, passed: true, evidence_digest: "a".repeat(64) }),
    plannerEvaluator: async () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.88, passed: true, evidence_digest: "b".repeat(64) }),
  });
  assert.equal(replay.status, "settled");
  assert.deepEqual(agent.learner.snapshot(), beforeReplay);
  assert.equal(health.health({ model: "planner-executor" })[0]?.quality_observations, 2);
  assert.equal(JSON.stringify(settled.planner_settlement).includes("planned learning execution"), false);
  assert.equal(JSON.stringify(settled.execution_settlement).includes("planned learning execution"), false);
});

test("provider-planned cross-domain learning settles planner, specialists, and synthesis together", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (planningMessage) {
        const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
        const ids = (contract.child_catalogue ?? contract.stage_catalogue).map((row) => row.id);
        const focusField = contract.child_catalogue ? "focus_child_ids" : "focus_stage_ids";
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.91, abstain: false }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "cross-domain planned execution" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cross-planned-learning", "https://cross-planned-learning.test", { requiresCredential: false }));
  const health = new InMemoryAutonomousModelHealthStore();
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  agent.registerModel({ ...candidate(), provider: "cross-planned-learning", model: "cross-planner", capabilities: ["reasoning", "code", "science", "biomedical", "coordination", "structured_output"] });
  const controller = new AutonomousLearningController(agent);
  const planned = await agent.planAndRun("Research a biomedical neuroscience experiment with EEG patient evidence.", {
    allowCrossDomain: true,
    planning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    learning: controller,
    maxParallelChildren: 1,
  });
  assert.equal(planned.status, "completed");
  assert.equal(planned.result.child_runs.length, 2);
  const settled = await controller.evaluateAndSettlePlanAndRun(planned, {
    trajectoryId: "cross-planned-learning-trajectory",
    evaluator: async () => ({ evaluator_id: "cross-execution-reviewer", evaluator_version: "1", reward: 0.74, passed: true, evidence_digest: "c".repeat(64) }),
    plannerEvaluator: async () => ({ evaluator_id: "cross-planner-reviewer", evaluator_version: "1", reward: 0.83, passed: true, evidence_digest: "d".repeat(64) }),
  });
  assert.equal(settled.status, "settled");
  assert.equal(settled.planner_settlement?.status, "settled");
  assert.equal(settled.execution_settlement?.trajectory.status, "settled");
  assert.equal(Object.keys(settled.rewards).length, planned.result.learning_episode_ids.length);
  assert.deepEqual(settled.planner_settlement?.planner_context, planned.plan_refinement.planner_context);
  assert.equal(settled.planner_settlement?.planner_context_digest, planned.plan_refinement.planner_context_digest);
  assert.equal(health.health({ model: "cross-planner" })[0]?.quality_observations, planned.result.learning_episode_ids.length + 1);
});

test("high-level cross-domain learning evaluates every specialist and synthesis episode as one trajectory", async () => {
  const agent = await learningAgent();
  const controller = new AutonomousLearningController(agent, {
    runEvaluator: new AutonomousEvaluatorMesh({
      members: [{
        evaluator_id: "cross-domain-quality-reviewer",
        evaluator_version: "1",
        evaluate: async () => ({ evaluator_id: "cross-domain-quality-reviewer", evaluator_version: "1", reward: 0.8, passed: true, evidence_digest: "f".repeat(64) }),
      }, {
        evaluator_id: "cross-domain-quality-reviewer-2",
        evaluator_version: "1",
        evaluate: async () => ({ evaluator_id: "cross-domain-quality-reviewer-2", evaluator_version: "1", reward: 0.8, passed: true, evidence_digest: "f".repeat(64) }),
      }],
    }),
  });
  const result = await controller.runCrossDomainLearning("Integrate domains coding data synthesis findings.", {
    trajectoryId: "high-level-cross-domain-trajectory",
    run: {
      allowCrossDomain: true,
      approveProviderCall: true,
      subtasks: [
        { id: "coding-specialist", task: "Review the implementation and tests.", domain: "coding" },
        { id: "data-specialist", task: "Review the data pipeline and schema.", domain: "data" },
      ],
    },
  });
  assert.equal(result.status, "settled");
  assert.equal(result.settlement.trajectory.status, "settled");
  assert.equal(Object.keys(result.rewards).length, result.run.learning_episode_ids.length);
  assert.equal(result.settlement.trajectory.steps.length, result.run.learning_episode_ids.length);
  assert.equal(agent.learner.snapshot().generation, result.run.learning_episode_ids.length);
  assert.equal(JSON.stringify(result.settlement).includes("Integrate domains"), false);
});

test("the learning controller gates direct and outbox settlement on evaluator calibration", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const run = await agent.run("Implement and verify calibration-gated learning.", { domain: "coding", approveProviderCall: true });
  const seedController = new AutonomousLearningController(agent, { episodes });
  await seedController.prepareRun(run, { episodeId: "calibration-gated-episode" });
  const blocked = new AutonomousLearningController(agent, {
    episodes,
    outbox,
    calibrationReport: codingCalibrationReport({ miscalibrated: true }),
    requireCalibratedLearning: true,
  });
  await assert.rejects(
    () => blocked.settleRun("calibration-gated-episode", { evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 1, passed: true }, { outbox: { workerId: "blocked-worker" } }),
    /calibration holds learning/,
  );
  assert.equal(outbox.pending().length, 0);
  assert.equal(agent.learner.snapshot().generation, 0);

  const admitted = new AutonomousLearningController(agent, {
    episodes,
    outbox,
    calibrationReport: codingCalibrationReport(),
    requireCalibratedLearning: true,
  });
  const settlement = await admitted.settleRun("calibration-gated-episode", { evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 1, passed: true }, { outbox: { workerId: "admitted-worker" } });
  assert.equal(settlement.episode.status, "settled");
  assert.equal(agent.learner.snapshot().generation, 1);
});

test("direct learning settlement annotates linked episodic memory and survives controller restart", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const agent = await learningAgent(memory);
  const controller = new AutonomousLearningController(agent);
  const run = await agent.run("Implement and verify the memory feedback bridge.", {
    domain: "coding",
    approveProviderCall: true,
    memoryRunId: "memory-feedback-bridge",
    learning: controller,
    learningEpisodeId: "learning-feedback-bridge",
    memoryLesson: "prefer the reviewed plan and verify independently",
  });
  assert.equal(run.memory.recorded_episode_id, "episode:memory-feedback-bridge");
  const episode = await controller.episodes.load("learning-feedback-bridge");
  assert.equal(episode.memory_episode_id, "episode:memory-feedback-bridge");
  const reward = { evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 0.92, passed: true, evidence_digest: "e".repeat(64) };
  const settled = await controller.settleRun("learning-feedback-bridge", reward);
  assert.equal(settled.memory_evaluation.status, "recorded");
  assert.equal(memory.get("episode:memory-feedback-bridge").evaluation.reward, 0.92);

  const snapshot = await memory.snapshot();
  const restoredMemory = new InMemoryAutonomousEpisodicMemory();
  await restoredMemory.restore(snapshot);
  const restarted = new AutonomousLearningController(agent, { episodes: controller.episodes, settlementReceipts: controller.settlementReceipts, memoryStore: restoredMemory });
  const replayed = await restarted.settleRun("learning-feedback-bridge", reward);
  assert.deepEqual(replayed, settled);
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal((await restoredMemory.verifyIntegrity()).evaluated, 1);
});

test("memory feedback failures are explicit without rolling back valid bandit credit", async () => {
  const backing = new InMemoryAutonomousEpisodicMemory();
  const failingMemory = {
    recordEpisode: backing.recordEpisode.bind(backing),
    recordEvaluation: async () => { throw new Error("memory evaluation backend unavailable"); },
    get: backing.get.bind(backing),
    retrieve: backing.retrieve.bind(backing),
    stats: backing.stats.bind(backing),
    verifyIntegrity: backing.verifyIntegrity.bind(backing),
    snapshot: backing.snapshot.bind(backing),
    restore: backing.restore.bind(backing),
  };
  const agent = await learningAgent(failingMemory);
  const controller = new AutonomousLearningController(agent, { memoryStore: failingMemory });
  const run = await agent.run("Test explicit memory feedback failure.", { domain: "coding", approveProviderCall: true, memoryRunId: "memory-feedback-failure", learning: controller, learningEpisodeId: "learning-feedback-failure" });
  const settled = await controller.settleRun(run.learning_episode_id, { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.7, passed: true });
  assert.equal(settled.memory_evaluation.status, "failed");
  assert.equal(settled.memory_evaluation.error_class, "Error");
  assert.equal(agent.learner.snapshot().generation, 1);
});

test("settlement receipts make single-episode replay idempotent across controller restart", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const receipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  const firstController = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts });
  const run = await agent.run("Implement this receipt replay test.", { domain: "coding", approveProviderCall: true });
  await firstController.prepareRun(run, { episodeId: "receipt-episode-1" });
  const reward = { evaluator_id: "receipt-reviewer", evaluator_version: "1", reward: 0.75, passed: true, evidence_digest: "d".repeat(64) };
  const first = await firstController.settleRun("receipt-episode-1", reward);
  const restartedController = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts });
  const replayed = await restartedController.settleRun("receipt-episode-1", reward);
  assert.deepEqual(replayed, first);
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal(receipts.rows().length, 1);
  assert.equal(JSON.stringify(receipts.rows()[0]).includes("Implement this receipt replay test"), false);
  assert.equal(JSON.stringify(receipts.rows()[0]).includes("verified learning response"), false);
  const contaminated = structuredClone(receipts.rows()[0]);
  contaminated.settlement.response = "private provider response";
  assert.throws(() => receipts.save(contaminated), /cannot retain response/);
});

test("receipt publication recovers a transient journal failure without double-crediting", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const durable = new InMemoryAutonomousLearningSettlementReceiptStore();
  let failNextWrite = true;
  const receipts = {
    load: (key) => durable.load(key),
    save: (receipt) => {
      if (failNextWrite) {
        failNextWrite = false;
        throw new Error("temporary settlement journal failure");
      }
      return durable.save(receipt);
    },
  };
  const controller = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts });
  const run = await agent.run("Recover this interrupted settlement.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "receipt-retry-1" });
  const reward = { evaluator_id: "receipt-retry-reviewer", evaluator_version: "1", reward: 0.65, passed: true };
  await assert.rejects(() => controller.settleRun("receipt-retry-1", reward), /temporary settlement journal failure/);
  assert.equal(episodes.load("receipt-retry-1").status, "pending");
  assert.equal(agent.learner.snapshot().generation, 1);
  const recovered = await controller.settleRun("receipt-retry-1", reward);
  assert.equal(recovered.episode.status, "settled");
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal(durable.rows().length, 1);
});

test("feedback outbox dispatches evaluator settlement exactly once across worker leases and restart", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const receipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const controller = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts, feedbackOutbox: outbox });
  const run = await agent.run("Dispatch this evaluator packet through a durable queue.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "outbox-episode-1" });
  const command = await controller.enqueueRunSettlement("outbox-episode-1", { evaluator_id: "outbox-reviewer", evaluator_version: "1", reward: 0.84, passed: true });
  assert.equal(command.status, "pending");
  assert.equal(agent.learner.snapshot().generation, 0);
  assert.doesNotMatch(JSON.stringify(command), /Dispatch this evaluator packet|provider response/);
  const held = outbox.claim(command.command_id, "worker-a", 30_000, command.created_at);
  assert.equal(held.status, "leased");
  assert.equal(outbox.claim(command.command_id, "worker-b", 30_000, command.created_at), null);
  const released = outbox.markFailed(command.command_id, "worker-a", "WorkerShutdown", true, command.created_at);
  assert.equal(released.status, "pending");
  const dispatch = await controller.dispatchFeedback({ workerId: "worker-b", now: released.available_at, leaseMs: 30_000 });
  assert.equal(dispatch.applied, 1);
  assert.equal(dispatch.failed, 0);
  assert.equal(dispatch.rows[0].result_digest.length, 64);
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal(episodes.load("outbox-episode-1").status, "settled");
  const restarted = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts, feedbackOutbox: outbox });
  const replay = await restarted.dispatchFeedback({ workerId: "worker-c", now: released.available_at + 60_000 });
  assert.equal(replay.inspected, 0);
  assert.equal(outbox.load(command.command_id).status, "applied");
  assert.equal(agent.learner.snapshot().generation, 1);
});

test("feedback outbox retries a post-credit journal failure through an idempotent receipt", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const durableReceipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  let failNextWrite = true;
  const receipts = {
    load: (key) => durableReceipts.load(key),
    save: (receipt) => {
      if (failNextWrite) {
        failNextWrite = false;
        throw new Error("temporary outbox journal failure");
      }
      return durableReceipts.save(receipt);
    },
  };
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const controller = new AutonomousLearningController(agent, { episodes, settlementReceipts: receipts, feedbackOutbox: outbox });
  const run = await agent.run("Retry this value-only evaluator settlement.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "outbox-retry-1" });
  const command = await controller.enqueueRunSettlement("outbox-retry-1", { evaluator_id: "outbox-retry-reviewer", evaluator_version: "1", reward: 0.61, passed: true });
  const first = await controller.dispatchFeedback({ workerId: "worker-a", now: command.created_at });
  assert.equal(first.failed, 1);
  assert.equal(outbox.load(command.command_id).status, "pending");
  assert.equal(agent.learner.snapshot().generation, 1);
  const second = await controller.dispatchFeedback({ workerId: "worker-a", now: command.created_at + 1_000 });
  assert.equal(second.applied, 1);
  assert.equal(agent.learner.snapshot().generation, 1);
  assert.equal(durableReceipts.rows().length, 1);
});

test("trajectory settlement uses the same outbox boundary as single-episode settlement", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const controller = new AutonomousLearningController(agent, { episodes, trajectories, feedbackOutbox: outbox });
  const run = await agent.run("Settle this delayed-credit trajectory through the worker boundary.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "outbox-trajectory-1" });
  await controller.prepareRun(run, { episodeId: "outbox-trajectory-2" });
  await controller.prepareTrajectory(["outbox-trajectory-1", "outbox-trajectory-2"], { trajectoryId: "outbox-trajectory" });
  const settled = await controller.settleTrajectory("outbox-trajectory", {
    "outbox-trajectory-1": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.4, passed: false },
    "outbox-trajectory-2": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.9, passed: true },
  }, { outbox: { workerId: "trajectory-worker" } });
  assert.equal(settled.trajectory.status, "settled");
  assert.equal(settled.settlements.length, 2);
  assert.equal(outbox.rows().filter((command) => command.status === "applied").length, 1);
  assert.equal(agent.learner.snapshot().generation, 2);
  const replayed = await controller.settleTrajectory("outbox-trajectory", {
    "outbox-trajectory-1": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.4, passed: false },
    "outbox-trajectory-2": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.9, passed: true },
  }, { outbox: { workerId: "trajectory-worker-replay" } });
  assert.deepEqual(replayed, settled);
  assert.equal(agent.learner.snapshot().generation, 2);
});

test("trajectory receipts replay all delayed-credit settlements without provider or learner replay", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const receipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  const controller = new AutonomousLearningController(agent, { episodes, trajectories, settlementReceipts: receipts });
  const run = await agent.run("Reconstruct this delayed-credit trajectory.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "receipt-trajectory-episode-1" });
  await controller.prepareRun(run, { episodeId: "receipt-trajectory-episode-2" });
  const rewards = {
    "receipt-trajectory-episode-1": { evaluator_id: "trajectory-receipt-reviewer", evaluator_version: "1", reward: 0.4, passed: false },
    "receipt-trajectory-episode-2": { evaluator_id: "trajectory-receipt-reviewer", evaluator_version: "1", reward: 0.9, passed: true },
  };
  await controller.prepareTrajectory(Object.keys(rewards), { trajectoryId: "receipt-trajectory", discount: 0.9 });
  const first = await controller.settleTrajectory("receipt-trajectory", rewards);
  const restartedController = new AutonomousLearningController(agent, { episodes, trajectories, settlementReceipts: receipts });
  const replayed = await restartedController.settleTrajectory("receipt-trajectory", rewards);
  assert.deepEqual(replayed, first);
  assert.equal(replayed.settlements.length, 2);
  assert.equal(agent.learner.snapshot().generation, 2);
  assert.equal(receipts.rows().filter((row) => row.operation === "trajectory").length, 1);
});

test("learning refuses incomplete autonomous runs before creating an episode", async () => {
  const agent = await learningAgent();
  const controller = new AutonomousLearningController(agent);
  const completed = await agent.run("Implement this bounded learning test.", { domain: "coding", approveProviderCall: true });
  const incomplete = { ...completed, status: "turn_limit_reached", tool_loop: { status: "turn_limit_reached", turns: 4, toolCalls: 4 } };
  await assert.rejects(() => controller.prepareRun(incomplete, { episodeId: "episode-incomplete" }), /completed autonomous run/);
});

test("trajectory settlement assigns bounded discounted return-to-go across episodes", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const controller = new AutonomousLearningController(agent, { episodes, trajectories });
  const run = await agent.run("Review and verify this staged learning test.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "trajectory-episode-1" });
  await controller.prepareRun(run, { episodeId: "trajectory-episode-2" });
  const trajectory = await controller.prepareTrajectory(["trajectory-episode-1", "trajectory-episode-2"], { trajectoryId: "trajectory-1", discount: 0.9 });
  const settled = await controller.settleTrajectory("trajectory-1", {
    "trajectory-episode-1": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.4, passed: false },
    "trajectory-episode-2": { evaluator_id: "trajectory-reviewer", evaluator_version: "1", reward: 0.8, passed: true },
  });
  assert.equal(trajectory.status, "pending");
  assert.equal(settled.trajectory.status, "settled");
  assert.equal(settled.return_to_go["trajectory-episode-2"], 0.8);
  assert.equal(settled.return_to_go["trajectory-episode-1"], 1);
  assert.equal(settled.settlements.length, 2);
  assert.equal(agent.learner.snapshot().generation, 2);
});

test("workflow executor emits pending stage episodes and controller settles them from explicit signals", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const controller = new AutonomousLearningController(agent, { episodes, trajectories });
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore(), { learning: controller });
  const execution = await executor.start("Implement and verify this staged learning workflow.", {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 2,
    jobId: "learning-workflow-1",
  });
  assert.equal(execution.status, "paused");
  assert.equal(execution.learning_episode_ids.length, 2);
  assert.ok(execution.stage_results.every((stage) => stage.learning_episode_id));
  const settled = await controller.settleWorkflow(execution, {
    stages: execution.stage_results.map((stage) => ({
      stage_id: stage.stage.id,
      signals: Object.fromEntries(stage.stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  }, { trajectoryId: "learning-workflow-trajectory", discount: 0.9 });
  assert.equal(settled.evaluation.status, "incomplete");
  assert.equal(settled.trajectory.settlements.length, 2);
  assert.equal(settled.response_settlements.length, 2);
  assert.ok(settled.response_settlements.every((row) => row.assessment.evaluator_id.endsWith("workflow-stage-integrity")));
  assert.ok(settled.trajectory.settlements.every((row) => row.assessment.passed));
  assert.equal(episodes.pending().length, 0);

  const resumed = await executor.resume("learning-workflow-1", "Implement and verify this staged learning workflow.", {
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.equal(resumed.status, "paused");
  assert.equal(resumed.learning_episode_ids.length, 3);
  assert.equal(episodes.pending().length, 2, "a resumed stage has one task-quality and one composition episode");
  const resumedSettlement = await controller.settleWorkflow(resumed, {
    stages: resumed.stage_results.map((stage) => ({
      stage_id: stage.stage.id,
      signals: Object.fromEntries(stage.stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  }, { trajectoryId: "learning-workflow-trajectory-2" });
  assert.equal(resumedSettlement.trajectory.settlements.length, 1);
  assert.equal(resumedSettlement.response_settlements.length, 3, "response settlements include replay-safe historical stage projections");
  assert.equal(episodes.pending().length, 0);
});

test("remote learning settlement sends run identity and evaluator values only", async () => {
  const agent = await learningAgent();
  const seen = [];
  const apiClient = {
    async brainOutcomeRecord(args) {
      seen.push(args);
      return {
        ok: true,
        mcp: {
          result: {
            structuredContent: {
              ok: true,
              status: "recorded_evaluator_reward",
              next_state: { schema: "bandit", generation: 9, arms: [{ arm_id: "learning-provider/learning-model", pulls: 3, reward_sum: 0.2, failures: 1 }] },
              learning_evidence: { schema: "evidence", evidence_digest: "b".repeat(64) },
            },
          },
        },
      };
    },
  };
  const controller = new AutonomousLearningController(agent, { apiClient });
  const run = await agent.run("Implement this remote learning test.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "episode-remote-1" });
  const settlement = await controller.settleRun("episode-remote-1", {
    evaluator_id: "remote-reviewer",
    evaluator_version: "1",
    reward: 0.7,
    passed: true,
    evidence_digest: "c".repeat(64),
  }, { remote: true });
  assert.equal(settlement.remote, true);
  assert.equal(agent.learner.snapshot().generation, 9);
  assert.deepEqual(agent.learner.snapshot().arms, [{ arm_id: "learning-provider/learning-model", pulls: 3, reward_sum: 0.2, failures: 1 }]);
  assert.equal(seen.length, 1);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0], "prompt"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0], "response"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].run, "task"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].run, "credentials"), false);
  assert.equal(seen[0].assessment.reward, 0.7);
  assert.match(seen[0].context_digest, /^[0-9a-f]{64}$/);
  assert.equal(seen[0].context.domain, "coding");
});

test("trajectory settlement resumes after a transient later-episode failure", async () => {
  const agent = await learningAgent();
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  let calls = 0;
  const apiClient = {
    async brainOutcomeRecord(args) {
      calls += 1;
      if (calls === 2) throw new Error("temporary learning transport failure");
      const generation = agent.learner.snapshot().generation;
      return {
        ok: true,
        mcp: {
          result: {
            structuredContent: {
              ok: true,
              status: "recorded_evaluator_reward",
              next_state: { schema: "bandit", generation: generation + 1, arms: [{ arm_id: args.arm_id, pulls: generation + 1, reward_sum: args.assessment.reward, failures: 0 }] },
              learning_evidence: { schema: "evidence", evidence_digest: "b".repeat(64) },
            },
          },
        },
      };
    },
  };
  const controller = new AutonomousLearningController(agent, { episodes, trajectories, apiClient });
  const run = await agent.run("Implement this resumable learning test.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "resume-episode-1" });
  await controller.prepareRun(run, { episodeId: "resume-episode-2" });
  await controller.prepareTrajectory(["resume-episode-1", "resume-episode-2"], { trajectoryId: "resume-trajectory", discount: 0.9 });
  const rewards = {
    "resume-episode-1": { evaluator_id: "resume-reviewer", evaluator_version: "1", reward: 0.4, passed: false },
    "resume-episode-2": { evaluator_id: "resume-reviewer", evaluator_version: "1", reward: 0.8, passed: true },
  };
  await assert.rejects(() => controller.settleTrajectory("resume-trajectory", rewards, { remote: true }), /temporary learning transport failure/);
  assert.equal(episodes.load("resume-episode-1").status, "settled");
  const resumed = await controller.settleTrajectory("resume-trajectory", rewards, { remote: true });
  assert.equal(resumed.trajectory.status, "settled");
  assert.equal(resumed.settlements.length, 1);
  assert.equal(episodes.pending().length, 0);
  assert.equal(agent.learner.snapshot().generation, 2);
  assert.equal(calls, 3);
});

test("cross-domain learning tracks specialists and synthesis as one delayed-credit trajectory", async () => {
  const agent = await learningAgent();
  const state = new InMemoryAutonomousLearningStateStore();
  const receipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  const controller = new AutonomousLearningController(agent, { store: state, settlementReceipts: receipts });
  const result = await agent.runCrossDomain("Research a biomedical neuroscience experiment with EEG patient evidence", {
    candidates: agent.models(),
    approveProviderCall: true,
    learning: controller,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence and safety." },
      { id: "neuro", domain: "neuroscience", task: "Analyze EEG study design and signal limits." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(result.learning_episode_ids.length, 3);
  assert.equal(state.pendingEpisodes().length, 3);
  const rewards = Object.fromEntries(result.learning_episode_ids.map((episodeId, index) => [episodeId, {
    evaluator_id: "cross-domain-reviewer",
    evaluator_version: "1",
    reward: index === 2 ? 0.8 : 0.7,
    passed: true,
    evidence_digest: String.fromCharCode(100 + index).repeat(64),
  }]));
  const settled = await controller.settleCrossDomain(result, rewards, { trajectoryId: "cross-domain-trajectory", discount: 0.9 });
  const replayed = await controller.settleCrossDomain(result, rewards, { trajectoryId: "cross-domain-trajectory", discount: 0.9 });
  assert.equal(settled.trajectory.settlements.length, 3);
  assert.deepEqual(replayed.trajectory, settled.trajectory);
  assert.equal(state.pendingEpisodes().length, 0);
  assert.ok(settled.trajectory.settlements.every((row) => JSON.stringify(row.episode).includes("biomedical evidence") === false));
  assert.equal(agent.learner.snapshot().generation, 3);
  const trajectoryReceipt = receipts.rows().find((row) => row.operation === "trajectory");
  assert.ok(trajectoryReceipt);
  assert.equal(JSON.stringify(trajectoryReceipt).includes("verified learning response"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(trajectoryReceipt.settlement, "result"), false);
});

test("learning state snapshots restore pending/settled rows and refuse tampering", async () => {
  const agent = await learningAgent();
  const state = new InMemoryAutonomousLearningStateStore();
  const controller = new AutonomousLearningController(agent, { store: state });
  const run = await agent.run("Implement and verify persistence recovery.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "persisted-episode" });
  await controller.settleRun("persisted-episode", { evaluator_id: "persistence-reviewer", evaluator_version: "1", reward: 0.6, passed: true });
  let persisted = null;
  const coordinator = new AutonomousLearningPersistenceCoordinator(state, {
    read: () => persisted,
    write: (snapshot) => { persisted = structuredClone(snapshot); },
  });
  const snapshot = await coordinator.flush();
  assert.equal(snapshot.snapshot_digest.length, 64);
  assert.equal(snapshot.generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  assert.equal(snapshot.episodes[0].status, "settled");
  const restoredState = new InMemoryAutonomousLearningStateStore();
  const restoredCoordinator = new AutonomousLearningPersistenceCoordinator(restoredState, {
    read: () => persisted,
    write: () => {},
  });
  await restoredCoordinator.restore();
  assert.equal(restoredState.loadEpisode("persisted-episode").status, "settled");
  const nextSnapshot = await coordinator.flush();
  assert.equal(nextSnapshot.generation, 2);
  assert.equal(nextSnapshot.previous_snapshot_digest, snapshot.snapshot_digest);
  const tampered = structuredClone(persisted);
  tampered.episodes[0].episode_id = "tampered-episode";
  await assert.rejects(() => restoredState.restore(tampered), /snapshot digest does not match/);

  const forged = structuredClone(snapshot);
  forged.generation = 2;
  forged.previous_snapshot_digest = null;
  const { snapshot_digest: _ignored, ...forgedDescriptor } = forged;
  forged.snapshot_digest = await digestJson(forgedDescriptor);
  await assert.rejects(() => validateAutonomousLearningStateSnapshot(forged), /generation and previous_snapshot_digest/);

  const legacy = structuredClone(snapshot);
  legacy.schema = "bioprism-typescript-autonomous-learning-snapshot/0.1";
  delete legacy.previous_snapshot_digest;
  const { snapshot_digest: _legacyIgnored, ...legacyDescriptor } = legacy;
  legacy.snapshot_digest = await digestJson(legacyDescriptor);
  const legacyState = new InMemoryAutonomousLearningStateStore();
  await legacyState.restore(legacy);
  const upgraded = await legacyState.snapshot();
  assert.equal(upgraded.schema, "bioprism-typescript-autonomous-learning-snapshot/0.2");
  assert.equal(upgraded.generation, 2);
  assert.equal(upgraded.previous_snapshot_digest, legacy.snapshot_digest);
});

test("settlement receipts persist through browser JSON and fence stale workers", async () => {
  const agent = await learningAgent();
  const receipts = new InMemoryAutonomousLearningSettlementReceiptStore();
  const controller = new AutonomousLearningController(agent, { settlementReceipts: receipts });
  const run = await agent.run("Persist a value-only settlement receipt.", { domain: "coding", approveProviderCall: true });
  await controller.prepareRun(run, { episodeId: "receipt-persist-1" });
  await controller.settleRun("receipt-persist-1", { evaluator_id: "receipt-reviewer", evaluator_version: "1", reward: 0.7, passed: true });

  let encoded = null;
  const persistence = new TransactionalJsonAutonomousLearningSettlementReceiptPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const observedDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observedDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  });
  const primary = new AutonomousLearningSettlementReceiptPersistenceCoordinator(receipts, persistence);
  const first = await primary.flush();
  assert.equal(first.receipts.length, 1);
  assert.doesNotMatch(encoded, /Persist a value-only settlement receipt/);

  const stale = new AutonomousLearningSettlementReceiptPersistenceCoordinator(new InMemoryAutonomousLearningSettlementReceiptStore(), persistence);
  await stale.restore();
  const secondController = new AutonomousLearningController(agent, { episodes: controller.episodes, settlementReceipts: primary });
  const secondRun = await agent.run("Persist a second value-only receipt.", { domain: "science", approveProviderCall: true });
  await secondController.prepareRun(secondRun, { episodeId: "receipt-persist-2" });
  await secondController.settleRun("receipt-persist-2", { evaluator_id: "receipt-reviewer", evaluator_version: "1", reward: 0.8, passed: true });
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const values = new Map();
  const browserPersistence = new JsonAutonomousLearningSettlementReceiptPersistence(new WebStorageAutonomousLearningSettlementReceiptTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }, "aurora-receipts"));
  const persisted = await persistence.read();
  await browserPersistence.write(persisted);
  assert.deepEqual(await browserPersistence.read(), persisted);
  const canonicalReceipt = values.get("aurora-receipts");
  values.set("aurora-receipts", JSON.stringify(JSON.parse(canonicalReceipt), null, 2));
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  values.set("aurora-receipts", canonicalReceipt);
  const restored = new InMemoryAutonomousLearningSettlementReceiptStore();
  const recovered = new AutonomousLearningSettlementReceiptPersistenceCoordinator(restored, persistence);
  const recoverySnapshot = await recovered.restore();
  assert.equal(recoverySnapshot.receipts.length, 2);
  assert.equal(restored.rows().length, 2);

  const unsafe = structuredClone(recoverySnapshot);
  unsafe.authorization = "never retained";
  assert.throws(() => validateAutonomousLearningSettlementReceiptSnapshot(unsafe), /unsupported fields/);
});

test("learning episode and trajectory state persists through JSON/browser CAS recovery", async () => {
  const agent = await learningAgent();
  const state = new InMemoryAutonomousLearningStateStore();
  const controller = new AutonomousLearningController(agent, { store: state });
  const firstRun = await agent.run("Prepare a restart-safe coding trajectory.", { domain: "coding", approveProviderCall: true });
  const secondRun = await agent.run("Prepare a restart-safe science trajectory.", { domain: "science", approveProviderCall: true });
  await controller.prepareRun(firstRun, { episodeId: "state-episode-1" });
  await controller.prepareRun(secondRun, { episodeId: "state-episode-2" });
  await controller.prepareTrajectory(["state-episode-1", "state-episode-2"], { trajectoryId: "state-trajectory", discount: 0.9 });

  let encoded = null;
  const persistence = new TransactionalJsonAutonomousLearningStatePersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const observedDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observedDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  });
  const primary = new AutonomousLearningPersistenceCoordinator(state, persistence);
  const first = await primary.flush();
  assert.equal(first.episodes.length, 2);
  assert.equal(first.trajectories.length, 1);
  assert.doesNotMatch(encoded, /restart-safe coding trajectory|restart-safe science trajectory/);

  const staleState = new InMemoryAutonomousLearningStateStore();
  const stale = new AutonomousLearningPersistenceCoordinator(staleState, persistence);
  await stale.restore();
  const thirdRun = await agent.run("Create another persisted operations episode.", { domain: "operations", approveProviderCall: true });
  await controller.prepareRun(thirdRun, { episodeId: "state-episode-3" });
  await primary.flush();
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const values = new Map();
  const browserPersistence = new JsonAutonomousLearningStatePersistence(new WebStorageAutonomousLearningSnapshotTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }, "aurora-learning-state"));
  const persisted = await persistence.read();
  await browserPersistence.write(persisted);
  assert.deepEqual(await browserPersistence.read(), persisted);
  const canonicalState = values.get("aurora-learning-state");
  values.set("aurora-learning-state", JSON.stringify(JSON.parse(canonicalState), null, 2));
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  values.set("aurora-learning-state", canonicalState);

  const recoveredState = new InMemoryAutonomousLearningStateStore();
  const recovered = new AutonomousLearningPersistenceCoordinator(recoveredState, persistence);
  const recoverySnapshot = await recovered.restore();
  assert.equal(recoverySnapshot.episodes.length, 3);
  assert.equal(recoveredState.loadTrajectory("state-trajectory").steps.length, 2);

  const unsafe = structuredClone(recoverySnapshot);
  unsafe.authorization = "never retained";
  await assert.rejects(() => validateAutonomousLearningStateSnapshot(unsafe), /unsupported fields/);
});
