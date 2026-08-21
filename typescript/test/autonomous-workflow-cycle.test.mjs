import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousLearningEpisodeStore,
  InMemoryAutonomousLearningTrajectoryStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
  runAutonomousWorkflowCycle,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), { status, headers: { "content-type": "application/json" } });
}

function model() {
  return {
    provider: "cycle-provider",
    model: "cycle-model",
    capabilities: [
      "reasoning", "code", "web", "data", "science", "biomedical", "neuroscience",
      "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output",
    ],
    context_window_tokens: 64_000,
    max_output_tokens: 4_000,
    quality: 0.95,
    latency_ms: 40,
    cost_per_million_tokens: 5,
    reliability: 0.99,
  };
}

function stagePayload(init) {
  let body = {};
  try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
  const prompt = JSON.stringify(body.messages ?? []);
  const stageId = prompt.match(/Execute workflow stage ([A-Za-z0-9_.:-]+)/)?.[1] ?? "stage";
  return {
    stage_id: stageId,
    status: "completed",
    evidence: [`evidence-${stageId}`],
    uncertainty: [],
    notes: `verified ${stageId}`,
    next_actions: [],
  };
}

async function makeAgent(withLearning = false) {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(stagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, withLearning ? { learner: new AutonomousOnlineLearner() } : {});
  agent.registerModel(model());
  return agent;
}

function perfectEvidence(execution) {
  return {
    stages: execution.blueprint.workflow.stages.map((stage) => ({
      stage_id: stage.id,
      signals: Object.fromEntries(stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  };
}

test("workflow cycle supervises every built-in domain with explicit evidence", async () => {
  const agent = await makeAgent();
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
    const cycle = await runAutonomousWorkflowCycle(`Run a verified ${profile.domain} workflow.`, executor, {
      domain: profile.domain,
      candidates: agent.models(),
      approveProviderCall: true,
      jobId: `cycle-${profile.domain}`,
      evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
    });
    assert.equal(cycle.status, "completed", profile.domain);
    assert.equal(cycle.attempts.length, 1, profile.domain);
    assert.equal(cycle.evaluations[0].status, "passed", profile.domain);
    assert.equal(cycle.evaluations[0].reward, 1, profile.domain);
  }
});

test("workflow cycle gives the evaluator a bounded replan path and settles stage trajectories", async () => {
  const agent = await makeAgent(true);
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const learning = new AutonomousLearningController(agent, { episodes, trajectories });
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore(), { learning });
  let evaluations = 0;
  const cycle = await runAutonomousWorkflowCycle("Replan this verified coding workflow once.", executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    jobId: "cycle-replan-coding",
    maxReplans: 1,
    learning: { controller: learning, trajectoryIdPrefix: "cycle-replan-trajectory" },
    evaluate: async (execution) => {
      evaluations += 1;
      return {
        evidence: perfectEvidence(execution),
        replan_requested: evaluations === 1,
        replan_instruction: evaluations === 1 ? "Add one independent verification pass." : null,
        feedback_digest: evaluations === 1 ? "a".repeat(64) : null,
      };
    },
  });
  assert.equal(cycle.status, "completed");
  assert.equal(cycle.replan_count, 1);
  assert.equal(cycle.attempts.length, 2);
  assert.equal(cycle.evaluations.length, 2);
  assert.equal(cycle.evaluations[0].replan_requested, true);
  assert.equal(cycle.evaluations[1].passed, true);
  assert.equal(cycle.settlements.length, 2);
  assert.equal(episodes.pending().length, 0);
  assert.equal(agent.learner.snapshot().generation, 10);
  assert.match(cycle.attempts[1].job_id, /:attempt-2$/);
});

test("workflow cycle refuses credential-shaped evaluator guidance before another provider attempt", async () => {
  const agent = await makeAgent();
  let calls = 0;
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  await assert.rejects(
    () => runAutonomousWorkflowCycle("Reject unsafe workflow feedback.", executor, {
      domain: "coding",
      candidates: agent.models(),
      approveProviderCall: true,
      jobId: "cycle-unsafe-feedback",
      evaluate: async (execution) => {
        calls += 1;
        return { evidence: perfectEvidence(execution), replan_requested: true, replan_instruction: "Use api_key=never." };
      },
    }),
    /credential-shaped material/,
  );
  assert.equal(calls, 1);
});
