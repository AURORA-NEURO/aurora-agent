import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousModelHealthController,
  AutonomousModelHealthPersistenceCoordinator,
  AutonomousOfflineReplayEngine,
  InMemoryAutonomousModelHealthStore,
  builtinAutonomousDomainEvaluatorProfiles,
  digestJson,
} from "../dist/index.js";

const digest = "a".repeat(64);

function invocation(domain, model = "model-a") {
  return {
    provider: "provider-a",
    model,
    domain,
    capability: "reasoning",
    risk_class: "review_required",
    status: "completed",
    outcome: "success",
    latency_ms: 120,
    input_tokens: 100,
    output_tokens: 40,
    outcome_digest: digest,
  };
}

test("model health ledger aggregates every domain and refuses transient metadata", async () => {
  const store = new InMemoryAutonomousModelHealthStore({ clock: () => 100 });
  const profiles = await builtinAutonomousDomainEvaluatorProfiles();
  for (const profile of profiles) {
    const observation = invocation(profile.domain, `model-${profile.domain}`);
    await store.recordInvocation(observation);
    await store.recordEvaluation({
      ...observation,
      quality_reward: 0.8,
      quality_passed: true,
      outcome_digest: digest,
    });
  }
  const rows = await store.health({ limit: 32 });
  assert.equal(rows.length, 12);
  assert.equal(rows.every((row) => row.attempts === 1 && row.quality_observations === 1 && row.quality_mean === 0.8), true);
  assert.equal((await store.verifyIntegrity()).events, 24);
  const serialized = JSON.stringify(await store.snapshot());
  assert.equal(serialized.includes("private task"), false);
  assert.equal(serialized.includes("api_key"), false);
  await assert.rejects(store.record({ ...invocation("coding"), prompt: "raw provider prompt" }), /transient or secret/);
});

test("health snapshots restore through a caller adapter and refuse tampering", async () => {
  const source = new InMemoryAutonomousModelHealthStore({ clock: () => 101 });
  await source.recordInvocation(invocation("coding"));
  const snapshot = await source.snapshot();
  let persisted = null;
  await new AutonomousModelHealthPersistenceCoordinator(source, { read: () => persisted, write: (next) => { persisted = next; } }).flush();
  const restored = new InMemoryAutonomousModelHealthStore({ clock: () => 102 });
  await new AutonomousModelHealthPersistenceCoordinator(restored, { read: () => persisted, write: () => {} }).restore();
  assert.equal((await restored.health({ domain: "coding" }))[0].attempts, 1);
  const tampered = structuredClone(snapshot);
  tampered.events[0].observation.status = "tampered";
  await assert.rejects(restored.restore(tampered), /snapshot digest mismatch/);
});

test("persisted health drives selection and invocation observers without provider payloads", async () => {
  const store = new InMemoryAutonomousModelHealthStore();
  const controller = new AutonomousModelHealthController(store);
  await controller.recordEvaluation({ provider: "provider-a", model: "model-a", domain: "coding", capability: "reasoning", riskClass: "review_required", evaluatorId: "coding-reviewer", evaluatorVersion: "0.1", reward: 0.1, passed: false, evidenceDigest: digest });
  await controller.recordEvaluation({ provider: "provider-b", model: "model-b", domain: "coding", capability: "reasoning", riskClass: "review_required", evaluatorId: "coding-reviewer", evaluatorVersion: "0.1", reward: 0.95, passed: true, evidenceDigest: digest });
  const request = {
    task: "bounded task metadata",
    domain: "coding",
    capability: "reasoning",
    risk_class: "review_required",
    required_capabilities: [],
    estimated_input_tokens: 100,
    requested_output_tokens: 100,
    candidates: [
      { provider: "provider-a", model: "model-a", capabilities: ["reasoning"], context_window_tokens: 10_000, max_output_tokens: 1_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.9 },
      { provider: "provider-b", model: "model-b", capabilities: ["reasoning"], context_window_tokens: 10_000, max_output_tokens: 1_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.9 },
    ],
    provider_health: {
      "provider-a": { provider: "provider-a", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true },
      "provider-b": { provider: "provider-b", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true },
    },
    model_health: {},
  };
  const decision = await controller.selector()(request);
  assert.deepEqual(decision.selected_model, { provider: "provider-b", model: "model-b" });
  assert.equal(decision.strategy, "caller_selector");
  await controller.observer({ domain: "coding", capability: "reasoning", riskClass: "review_required" }).after(
    { provider: "provider-b", model: "model-b", kind: "autonomous_selected_model", inputTokens: 100, requestedOutputTokens: 100, toolCount: 0 },
    { success: true, status: "completed", latencyMs: 55, inputTokens: 100, outputTokens: 50 },
  );
  assert.equal((await store.health({ model: "model-b" }))[0].attempts, 1);
});

test("offline replay evaluates all twelve domains and detects expected-evidence drift", async () => {
  const profiles = await builtinAutonomousDomainEvaluatorProfiles();
  const engine = new AutonomousOfflineReplayEngine();
  const cases = profiles.map((profile, index) => ({
    run_id: `replay-${index}`,
    domain: profile.domain,
    capability: "reasoning",
    risk_class: "review_required",
    evaluator_id: profile.evaluator_id,
    evaluator_version: profile.evaluator_version,
    execution_status: "completed",
    signals: Object.fromEntries(profile.required_signals.map((signal) => [signal, 1])),
    evidence_digest: digest,
  }));
  const first = await engine.replay(cases);
  assert.equal(first.status, "completed");
  assert.equal(first.case_count, 12);
  assert.equal(first.passed_count, 12);
  assert.equal(first.cases.every((row) => row.mismatch_codes.length === 0), true);
  const drifted = await engine.replay(cases.map((entry, index) => index === 0 ? { ...entry, expected_reward: 0.1 } : entry));
  assert.equal(drifted.status, "mismatch");
  assert.equal(drifted.mismatch_count, 1);
  assert.equal(JSON.stringify(drifted).includes("bounded task"), false);
});
