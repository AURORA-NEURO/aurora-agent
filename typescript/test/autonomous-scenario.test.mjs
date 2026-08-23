import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousOfflineScenarioHarness,
  AutonomousOnlineLearner,
  AutonomousEvaluatorCalibrationHarness,
  AutonomousValueEvaluatorRegistry,
  LLMRuntime,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "operations", "enterprise", "coordination",
    "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function localRuntime(seen) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    seen.push(request);
    return { output_text: `private-provider-output:${request.model}` };
  });
  return runtime;
}

function perfectEvidence(profile, domain = profile.domain) {
  const signals = new Set([...profile.required_signals, ...Object.keys(profile.signal_weights)]);
  return {
    domain,
    capability: `${domain}-caller-review`,
    risk_class: "bounded-review",
    signals: Object.fromEntries([...signals].map((signal) => [signal, true])),
    references: ["a".repeat(64)],
    limitations: ["caller-declared signals only"],
    selected_tool_names: [`${domain}.verify`],
  };
}

test("offline scenario matrix closes selection, exact invocation, evaluation, learning, and replay for every domain", async () => {
  const seen = [];
  const runtime = localRuntime(seen);
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const harness = new AutonomousOfflineScenarioHarness(agent, { evaluatorRegistry: registry });
  const privateTasks = Object.fromEntries(AUTONOMOUS_DOMAIN_NAMES.map((domain) => [domain, `private-task-${domain}-must-not-be-retained`])) ;

  const report = await harness.runAll({
    tasks: privateTasks,
    evidenceFor: ({ preview }) => ({ evidence: perfectEvidence(registry.resolveForAutonomousDomain(preview.domain).profile) }),
  });

  assert.equal(report.schema, "bioprism-autonomous-offline-scenario/0.1");
  assert.equal(report.status, "completed");
  assert.equal(report.case_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.refused_count, 0);
  assert.deepEqual(report.domains, [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.ok(report.cases.every((row) => row.status === "completed" && row.evaluation?.passed === true));
  assert.ok(report.cases.every((row) => /^[0-9a-f]{64}$/.test(row.task_digest)));
  assert.ok(report.cases.every((row) => row.learning.outcome_digest?.length === 64));
  assert.equal(agent.learner.snapshot().generation, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(seen.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(seen.every((request) => request.model === "offline-model"));
  assert.doesNotMatch(JSON.stringify(report), /private-task-|private-provider-output/);

  const beforeAttempts = runtime.providerStatus("offline").attempts;
  const replay = harness.replay(report);
  assert.equal(replay.schema, "bioprism-autonomous-offline-scenario-replay/0.1");
  assert.equal(replay.verified_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(replay.replayed_count, 0);
  assert.equal(replay.idempotent, true);
  assert.equal(replay.learner_generation_before, replay.learner_generation_after);
  assert.equal(runtime.providerStatus("offline").attempts, beforeAttempts);
});

test("offline scenario replay rejects metadata tampering before learning settlement", async () => {
  const runtime = localRuntime([]);
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const harness = new AutonomousOfflineScenarioHarness(agent, { evaluatorRegistry: registry });
  const report = await harness.run({
    cases: [{ domain: "coding", task: "a transient coding scenario", id: "coding" }],
    evidenceFor: ({ preview }) => ({ evidence: perfectEvidence(registry.resolveForAutonomousDomain(preview.domain).profile) }),
  });
  const tampered = structuredClone(report);
  tampered.cases[0].evaluation.reward = 0;
  assert.throws(() => harness.replay(tampered), /report digest/);
  assert.equal(agent.learner.snapshot().generation, 1);
});

test("offline scenario can require calibrated evaluators before provider execution or learning", async () => {
  const seen = [];
  const runtime = localRuntime(seen);
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const profile = registry.resolveForAutonomousDomain("coding").profile;
  const calibration = new AutonomousEvaluatorCalibrationHarness(registry).run({
    domains: ["coding"],
    cases: [
      { case_id: "coding-calibration-positive", domain: "coding", evidence: perfectEvidence(profile), label: 1, split: "calibration" },
      { case_id: "coding-holdout-false-positive", domain: "coding", evidence: perfectEvidence(profile), label: 0, split: "holdout" },
    ],
    minCalibrationCasesPerDomain: 1,
    minHoldoutCasesPerDomain: 1,
    maxExpectedCalibrationError: 0.01,
    maxBrierScore: 0.01,
  });
  assert.equal(calibration.status, "miscalibrated");
  const harness = new AutonomousOfflineScenarioHarness(agent, { evaluatorRegistry: registry });
  await assert.rejects(() => harness.run({
    cases: [{ domain: "coding", task: "calibration-gated coding task", id: "coding" }],
    evidenceFor: ({ preview }) => ({ evidence: perfectEvidence(registry.resolveForAutonomousDomain(preview.domain).profile) }),
    calibrationReport: calibration,
    requireCalibratedLearning: true,
  }), /calibration holds learning/);
  assert.equal(seen.length, 0);
  assert.equal(agent.learner.snapshot().generation, 0);
});
