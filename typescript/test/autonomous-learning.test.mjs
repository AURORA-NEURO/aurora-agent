import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowEvaluator,
  CredentialStore,
  InMemoryAutonomousLearningEpisodeStore,
  InMemoryAutonomousLearningTrajectoryStore,
  LLMRuntime,
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
  const episode = await controller.prepareRun(run, { episodeId: "episode-local-1" });
  assert.equal(episode.status, "pending");
  assert.equal(episode.run.provider, "learning-provider");
  assert.equal(Object.prototype.hasOwnProperty.call(episode, "response"), false);
  assert.equal(JSON.stringify(episode).includes("verified learning response"), false);
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
              next_state: { schema: "bandit", generation: 1, arms: [{ arm_id: "learning-provider/learning-model", pulls: 1, reward_sum: 0.7, failures: 0 }] },
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
  assert.equal(seen.length, 1);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0], "prompt"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0], "response"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].run, "task"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(seen[0].run, "credentials"), false);
  assert.equal(seen[0].assessment.reward, 0.7);
});
