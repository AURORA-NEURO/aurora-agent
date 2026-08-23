import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowPortfolioItemExecutionResult,
  InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore,
  LLMRuntime,
  digestJson,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science",
    "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function agentFor(onRequest = () => {}, learner = new AutonomousOnlineLearner()) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline result for ${request.model}` };
  });
  const agent = new AutonomousAgent(runtime, { learner });
  agent.registerModel(model);
  return agent;
}

function allDomainRequests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `learning-${domain}`,
    task: `private evaluator task for ${domain}`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`learning-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
  }));
}

function rewardFor(domain) {
  return {
    evaluator_id: `portfolio-evaluator-${domain}`,
    evaluator_version: "portfolio-test-1",
    reward: 0.75,
    passed: true,
    evidence_digest: "1".repeat(64),
  };
}

async function rehydratedPending(itemId, domain, run, dependsOn = []) {
  const output = run.response?.text ?? "";
  return new AutonomousWorkflowPortfolioItemExecutionResult(
    itemId,
    domain,
    dependsOn,
    "succeeded",
    run,
    output ? await digestJson({ item_id: itemId, output }) : null,
    new TextEncoder().encode(output).byteLength,
    null,
    null,
    true,
    output,
    "pending_evaluation",
    run.learning_episode_id,
    null,
    null,
    null,
  );
}

test("portfolio learning settles explicit evaluator credit across every autonomous domain", async () => {
  const agent = agentFor();
  const learning = new AutonomousLearningController(agent);
  const requests = allDomainRequests();
  const seen = [];
  const result = await agent.executeWorkflowPortfolio(requests, {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    learning,
    learningPolicyDigest: "a".repeat(64),
    evaluateItem: ({ domain, run, outputText }) => {
      seen.push({ domain, runStatus: run.status, outputBytes: new TextEncoder().encode(outputText).byteLength });
      return rewardFor(domain);
    },
  });

  assert.equal(result.status, "completed");
  assert.equal(seen.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.toJSON().learning_settled_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.toJSON().learning_pending_count, 0);
  assert.equal(result.toJSON().learning_failed_count, 0);
  assert.equal((await learning.episodes.pending()).length, 0);
  assert.ok(result.items.every((item) => item.learningStatus === "settled" && item.learningEpisodeId && item.evaluationDigest && item.settlementDigest));
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /private evaluator task|offline result/);
});

test("portfolio learning keeps evaluator work pending without claiming a completed run", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const learning = new AutonomousLearningController(agent);
  const result = await agent.executeWorkflowPortfolio([{ id: "pending", task: "private pending evaluator task", domain: "coding" }], {
    approveProviderCall: true,
    learning,
    learningPolicyDigest: "b".repeat(64),
  });

  assert.equal(providerCalls, 1);
  assert.equal(result.status, "partial");
  assert.equal(result.items[0].learningStatus, "pending_evaluation");
  assert.equal(result.toJSON().learning_pending_count, 1);
  assert.equal((await learning.episodes.pending()).length, 1);
});

test("resumed portfolio learning settles a rehydrated pending episode without replaying its provider call", async () => {
  const requests = [
    { id: "learn-first", task: "private first learning task", domain: "coding" },
    { id: "learn-second", task: "private second learning task", domain: "data", dependsOn: ["learn-first"] },
  ];
  let firstProviderCalls = 0;
  const firstAgent = agentFor(() => { firstProviderCalls += 1; });
  const originalRun = firstAgent.run.bind(firstAgent);
  const firstRuns = new Map();
  firstAgent.run = async (task, options) => {
    const run = await originalRun(task, options);
    firstRuns.set(task, run);
    return run;
  };
  const firstLearning = new AutonomousLearningController(firstAgent);
  const plan = await firstAgent.planWorkflowPortfolio(requests);
  const policyDigest = "c".repeat(64);
  let checkpoint = null;
  await assert.rejects(
    () => firstAgent.executeWorkflowPortfolioResumable(requests, {
      jobId: "learning-restart",
      plan,
      approveProviderCall: true,
      learning: firstLearning,
      learningPolicyDigest: policyDigest,
      checkpointSink: async (value) => {
        checkpoint = value;
        if (value.settled_item_ids.length > 0) throw new Error("synthetic learning interruption");
      },
    }),
    /synthetic learning interruption/,
  );
  assert.equal(firstProviderCalls, 1);
  assert.deepEqual(checkpoint.settled_item_ids, ["learn-first"]);

  await assert.rejects(
    () => firstAgent.executeWorkflowPortfolioResumable(requests, {
      jobId: "learning-restart",
      plan,
      checkpoint,
      approveProviderCall: true,
      learning: firstLearning,
      learningPolicyDigest: "d".repeat(64),
      rehydrateItem: async () => { throw new Error("must not rehydrate on policy drift"); },
    }),
    /checkpoint controls do not match/,
  );
  assert.equal(firstProviderCalls, 1);

  assert.ok(firstRuns.get("private first learning task"));
  const firstExecutionRun = firstRuns.get("private first learning task");

  let resumedProviderCalls = 0;
  const resumedAgent = agentFor(() => { resumedProviderCalls += 1; });
  const resumedLearning = new AutonomousLearningController(resumedAgent, {
    episodes: firstLearning.episodes,
    settlementReceipts: firstLearning.settlementReceipts,
  });
  const store = new InMemoryAutonomousWorkflowPortfolioExecutionCheckpointStore(checkpoint);
  const resumed = await resumedAgent.executeWorkflowPortfolioResumable(requests, {
    jobId: "learning-restart",
    plan,
    approveProviderCall: true,
    learning: resumedLearning,
    learningPolicyDigest: policyDigest,
    checkpoint: await store.read(),
    evaluateItem: ({ domain }) => rewardFor(domain),
    rehydrateItem: async (context) => rehydratedPending(context.item_id, context.domain, firstExecutionRun),
  });

  assert.equal(resumed.status, "completed");
  assert.equal(resumedProviderCalls, 1, "only the second item may invoke the provider after restart");
  assert.equal(resumed.toJSON().learning_settled_count, 2);
  assert.ok(resumed.items.every((item) => item.learningStatus === "settled"));
  assert.equal((await resumedLearning.episodes.pending()).length, 0);
});

test("portfolio evaluator and settlement failures remain explicit and do not replay provider work", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const learning = new AutonomousLearningController(agent);
  const result = await agent.executeWorkflowPortfolio([{ id: "failed-feedback", task: "private feedback failure task", domain: "science" }], {
    approveProviderCall: true,
    learning,
    evaluateItem: () => { throw new Error("evaluator unavailable"); },
  });

  assert.equal(providerCalls, 1);
  assert.equal(result.status, "partial");
  assert.equal(result.items[0].learningStatus, "evaluation_failed");
  assert.equal(result.items[0].learningErrorClass, "Error");
  assert.equal(result.toJSON().learning_failed_count, 1);
  assert.equal((await learning.episodes.pending()).length, 1);
});
