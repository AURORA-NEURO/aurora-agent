import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowEvaluator,
  CredentialStore,
  InMemoryAutonomousLearningEpisodeStore,
  InMemoryAutonomousLearningStateStore,
  InMemoryAutonomousLearningTrajectoryStore,
  AutonomousLearningPersistenceCoordinator,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  AutonomousWorkflowExecutor,
  builtinAutonomousDomainEvaluatorProfiles,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), { status, headers: { "content-type": "application/json" } });
}

function candidate() {
  return {
    provider: "learning-provider",
    model: "learning-model",
    capabilities: ["reasoning", "code", "science", "biomedical", "coordination", "data", "web", "operations", "enterprise", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 10,
    reliability: 0.95,
  };
}

async function learningAgent() {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "verified learning response" }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("learning-provider", "https://learning.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
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
  assert.ok(settled.trajectory.settlements.every((row) => row.assessment.passed));
  assert.equal(episodes.pending().length, 0);

  const resumed = await executor.resume("learning-workflow-1", "Implement and verify this staged learning workflow.", {
    candidates: agent.models(),
    approveProviderCall: true,
    maxStages: 1,
  });
  assert.equal(resumed.status, "paused");
  assert.equal(resumed.learning_episode_ids.length, 3);
  assert.equal(episodes.pending().length, 1);
  const resumedSettlement = await controller.settleWorkflow(resumed, {
    stages: resumed.stage_results.map((stage) => ({
      stage_id: stage.stage.id,
      signals: Object.fromEntries(stage.stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  }, { trajectoryId: "learning-workflow-trajectory-2" });
  assert.equal(resumedSettlement.trajectory.settlements.length, 1);
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
  const controller = new AutonomousLearningController(agent, { store: state });
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
  assert.equal(settled.trajectory.settlements.length, 3);
  assert.equal(state.pendingEpisodes().length, 0);
  assert.ok(settled.trajectory.settlements.every((row) => JSON.stringify(row.episode).includes("biomedical evidence") === false));
  assert.equal(agent.learner.snapshot().generation, 3);
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
  assert.equal(snapshot.episodes[0].status, "settled");
  const restoredState = new InMemoryAutonomousLearningStateStore();
  const restoredCoordinator = new AutonomousLearningPersistenceCoordinator(restoredState, {
    read: () => persisted,
    write: () => {},
  });
  await restoredCoordinator.restore();
  assert.equal(restoredState.loadEpisode("persisted-episode").status, "settled");
  const tampered = structuredClone(persisted);
  tampered.episodes[0].episode_id = "tampered-episode";
  await assert.rejects(() => restoredState.restore(tampered), /snapshot digest does not match/);
});
