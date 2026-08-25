import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  CredentialStore,
  LLMRuntime,
  InMemoryAutonomousCycleReplanStateStore,
  openaiCompatibleProvider,
  createAutonomousCycleEvaluatorBridge,
  runAutonomousAutoReplanCycle,
  runAutonomousCrossDomainDecisionCycle,
} from "../dist/index.js";

const singleDomains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "evaluation"];
const capabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

function candidate() {
  return {
    provider: "bridge-provider",
    model: "bridge-model",
    capabilities,
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function bridgeAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded bridge result" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("bridge-provider", "https://bridge.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

function evidenceFor(context) {
  assert.equal(context.schema, "bioprism-typescript-autonomous-cycle-evaluator-bridge/0.1");
  assert.equal(context.retention, "metadata_only;caller_evidence_factory_owns_values");
  assert.equal(context.secret_material, "never_returned");
  assert.equal(typeof context.route_digest, "string");
  assert.equal(context.learning_episode_ids.some((value) => value.includes("bounded bridge result")), false);
  return {
    evidence: {
      schema: "bioprism-brain-domain-evaluator/0.1",
      domain: context.domain,
      capability: "bounded_review",
      risk_class: "read_only",
      signals: Object.fromEntries(context.required_signals.map((signal) => [signal, 1])),
      references: [],
      limitations: ["Caller-owned fixture evidence only."],
      stage_plan_digest: null,
      capability_contract_digests: [],
      selected_tool_names: [],
      retention: "value_only_digests_and_signal_scores",
    },
  };
}

test("cycle evaluator bridge maps built-in value evaluators into every single-domain replan", async () => {
  const fixture = bridgeAgent();
  const contexts = [];
  const bridge = createAutonomousCycleEvaluatorBridge({
    evidenceFor: async (context) => {
      contexts.push(structuredClone(context));
      return evidenceFor(context);
    },
  });
  assert.match(bridge.evaluator_catalogue_digest, /^[0-9a-f]{64}$/);
  assert.match(bridge.policy_digest, /^[0-9a-f]{64}$/);

  const learning = new AutonomousLearningController(fixture.agent);
  for (const domain of singleDomains) {
    const result = await runAutonomousAutoReplanCycle(fixture.agent, `bridge evaluation for ${domain}`, {
      domain,
      approveProviderCall: true,
      maxReplans: 0,
      learning: { controller: learning, episodePrefix: `bridge-${domain}` },
      evaluate: bridge.evaluateReplan,
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.cycle.evaluations[0].evaluator_id, bridge.registry.resolveForAutonomousDomain(domain).evaluatorId, domain);
    assert.equal(result.cycle.evaluations[0].replan_requested, false, domain);
  }
  assert.equal(fixture.calls(), singleDomains.length);
  assert.equal(contexts.length, singleDomains.length);
  assert.equal(contexts.every((context) => context.role === "single" && !JSON.stringify(context).includes("bridge evaluation")), true);
  assert.equal(learning.episodes.snapshotRows().length, singleDomains.length);
});

test("cycle evaluator bridge maps specialist and synthesis credit for ordinary and adaptive cross-domain cycles", async () => {
  const fixture = bridgeAgent();
  const bridge = createAutonomousCycleEvaluatorBridge({ evidenceFor });
  const learning = new AutonomousLearningController(fixture.agent);
  const ordinary = await runAutonomousCrossDomainDecisionCycle(fixture.agent, "bridge biomedical neuroscience review", {
    allowCrossDomain: true,
    approveProviderCall: true,
    maxParallelChildren: 1,
    learning: {
      controller: learning,
      trajectoryId: "bridge-ordinary-cross",
      evaluate: bridge.evaluateCrossDomain,
    },
  });
  assert.equal(ordinary.status, "completed");
  assert.equal(Object.keys(ordinary.evaluation).length, 3);
  assert.equal(ordinary.settlement.trajectory.settlements.length, 3);

  const adaptive = await runAutonomousAutoReplanCycle(fixture.agent, "bridge biomedical neuroscience adaptive review", {
    allowCrossDomain: true,
    approveProviderCall: true,
    maxParallelChildren: 1,
    maxReplans: 0,
    learning: {
      controller: learning,
      episodePrefix: "bridge-adaptive-cross",
      trajectoryIdPrefix: "bridge-adaptive-trajectory",
    },
    evaluate: bridge.evaluateCrossDomainReplan,
  });
  assert.equal(adaptive.status, "completed");
  assert.equal(adaptive.mode, "cross_domain");
  assert.equal(adaptive.cycle.evaluations[0].reward_episode_count, 3);
  assert.equal(adaptive.cycle.settlements[0].trajectory.settlements.length, 3);
  assert.equal(fixture.calls(), 6);
});

test("cycle evaluator bridge preserves explicit evaluator failure and restart idempotency", async () => {
  const fixture = bridgeAgent();
  const missingEvidenceBridge = createAutonomousCycleEvaluatorBridge({
    evidenceFor: (context) => ({
      evidence: {
        schema: "bioprism-brain-domain-evaluator/0.1",
        domain: context.domain,
        capability: "bounded_review",
        risk_class: "read_only",
        signals: { unavailable_signal: 0 },
        references: [],
        limitations: ["No caller-owned signal was available."],
        stage_plan_digest: null,
        capability_contract_digests: [],
        selected_tool_names: [],
        retention: "value_only_digests_and_signal_scores",
      },
    }),
  });
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  const options = {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 0,
    cycleId: "bridge-restart",
    stateStore,
    evaluate: missingEvidenceBridge.evaluateReplan,
  };
  const task = "bridge restart with missing evaluator evidence";
  const first = await runAutonomousAutoReplanCycle(fixture.agent, task, options);
  assert.equal(first.status, "replan_limit_reached");
  assert.equal(first.cycle.evaluations[0].replan_requested, true);
  assert.equal(first.cycle.evaluations[0].passed, false);
  const providerCalls = fixture.calls();
  const replay = await runAutonomousAutoReplanCycle(fixture.agent, task, options);
  assert.equal(replay.status, "replan_limit_reached");
  assert.equal(replay.cycle.final, null);
  assert.equal(fixture.calls(), providerCalls);
});
