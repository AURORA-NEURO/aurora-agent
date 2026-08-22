import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousValueEvaluatorRegistry,
  LLMRuntime,
  createAutonomousWorkflowPortfolioEvaluatorBridge,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function agentFor() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", () => ({ output_text: "private bridge output" }));
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  return agent;
}

function requests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `bridge-${domain}`,
    task: `private bridge task for ${domain}`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`bridge-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
  }));
}

function evidenceFor(context, evidenceDomain = context.domain) {
  return {
    evidence: {
      schema: AUTONOMOUS_VALUE_EVALUATOR_SCHEMA,
      domain: evidenceDomain,
      capability: "value_only_portfolio_review",
      risk_class: "review_only",
      signals: Object.fromEntries(context.required_signals.map((signal) => [signal, 1])),
      references: [],
      limitations: [],
    },
  };
}

test("built-in evaluator bridge routes explicit evidence across every portfolio domain", async () => {
  const bridge = createAutonomousWorkflowPortfolioEvaluatorBridge({ evidenceFor });
  assert.equal(bridge.learningPolicyDigest.length, 64);
  assert.equal(bridge.registry.catalogue().length, AUTONOMOUS_DOMAIN_NAMES.length);
  const evaluatorIds = [];
  const agent = agentFor();
  const learning = new AutonomousLearningController(agent);
  const result = await agent.executeWorkflowPortfolio(requests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    learning,
    learningPolicyDigest: bridge.learningPolicyDigest,
    evaluateItem: async (context) => {
      evaluatorIds.push(context.domain);
      return bridge.evaluateItem(context);
    },
  });

  assert.equal(result.status, "completed");
  assert.deepEqual(evaluatorIds, AUTONOMOUS_DOMAIN_NAMES);
  assert.equal(result.toJSON().learning_settled_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /private bridge task|private bridge output/);
});

test("built-in evaluator bridge refuses cross-domain evidence instead of mixing rubrics", async () => {
  const bridge = createAutonomousWorkflowPortfolioEvaluatorBridge({ evidenceFor: (context) => evidenceFor(context, "science") });
  const agent = agentFor();
  const learning = new AutonomousLearningController(agent);
  const result = await agent.executeWorkflowPortfolio([{ id: "bridge-isolation", task: "private isolation task", domain: "coding" }], {
    approveProviderCall: true,
    learning,
    learningPolicyDigest: bridge.learningPolicyDigest,
    evaluateItem: bridge.evaluateItem,
  });

  assert.equal(result.status, "partial");
  assert.equal(result.items[0].learningStatus, "evaluation_failed");
  assert.equal(result.toJSON().learning_failed_count, 1);
  assert.equal((await learning.episodes.pending()).length, 1);
});

test("portfolio evaluator bridge fails closed when a custom registry does not cover all domains", () => {
  assert.throws(
    () => createAutonomousWorkflowPortfolioEvaluatorBridge({ registry: new AutonomousValueEvaluatorRegistry(), evidenceFor }),
    /no domain evaluator is registered/,
  );
});
