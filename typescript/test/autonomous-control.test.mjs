import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainControlPlaneBridge,
  AutonomousModelHealthController,
  AutonomousModelHealthPersistenceCoordinator,
  AutonomousModelCataloguePersistenceCoordinator,
  AutonomousOnlineLearner,
  AutonomousOfflineReplayEngine,
  InMemoryAutonomousModelHealthStore,
  TransactionalJsonAutonomousModelHealthSnapshotPersistence,
  validateAutonomousModelHealthSnapshot,
  LLMRuntime,
  builtinAutonomousDomainEvaluatorProfiles,
  digestCanonicalJsonTextSync,
  autonomousReplayEvidenceDigest,
  digestJson,
  openaiCompatibleProvider,
  validateAutonomousModelCatalogueSnapshot,
  validateAutonomousReplayReport,
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

function transactionalHealthTextStore() {
  let encoded = null;
  return {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
    encoded: () => encoded,
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
  assert.equal(snapshot.snapshot_generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  let persisted = null;
  await new AutonomousModelHealthPersistenceCoordinator(source, { read: () => persisted, write: (next) => { persisted = next; } }).flush();
  const restored = new InMemoryAutonomousModelHealthStore({ clock: () => 102 });
  await new AutonomousModelHealthPersistenceCoordinator(restored, { read: () => persisted, write: () => {} }).restore();
  assert.equal((await restored.health({ domain: "coding" }))[0].attempts, 1);
  const tampered = structuredClone(snapshot);
  tampered.events[0].observation.status = "tampered";
  await assert.rejects(restored.restore(tampered), /snapshot digest mismatch/);

  const forged = structuredClone(snapshot);
  forged.snapshot_generation = 2;
  forged.previous_snapshot_digest = null;
  const { snapshot_digest: _ignored, ...forgedDescriptor } = forged;
  forged.snapshot_digest = await digestJson(forgedDescriptor);
  await assert.rejects(validateAutonomousModelHealthSnapshot(forged), /generation and previous_snapshot_digest/);

  const legacy = structuredClone(snapshot);
  legacy.schema = "bioprism-typescript-autonomous-model-health-snapshot/0.1";
  delete legacy.snapshot_generation;
  delete legacy.previous_snapshot_digest;
  const { snapshot_digest: _legacyIgnored, ...legacyDescriptor } = legacy;
  legacy.snapshot_digest = await digestJson(legacyDescriptor);
  const legacyStore = new InMemoryAutonomousModelHealthStore({ clock: () => 103 });
  await legacyStore.restore(legacy);
  const upgraded = await legacyStore.snapshot();
  assert.equal(upgraded.schema, "bioprism-typescript-autonomous-model-health-snapshot/0.2");
  assert.equal(upgraded.snapshot_generation, 1);
  assert.equal(upgraded.previous_snapshot_digest, null);
});

test("health JSON persistence is canonical, restart-safe, serialized, and CAS-fenced", async () => {
  const textStore = transactionalHealthTextStore();
  const persistence = new TransactionalJsonAutonomousModelHealthSnapshotPersistence(textStore);
  const source = new InMemoryAutonomousModelHealthStore({ clock: () => 200 });
  const coordinator = new AutonomousModelHealthPersistenceCoordinator(source, persistence);
  await source.recordInvocation(invocation("coding"));
  const first = await coordinator.flush();
  assert.equal(textStore.encoded(), JSON.stringify(JSON.parse(textStore.encoded())));
  assert.deepEqual(await validateAutonomousModelHealthSnapshot(JSON.parse(textStore.encoded())), first);

  const restartedStore = new InMemoryAutonomousModelHealthStore({ clock: () => 201 });
  const restarted = new AutonomousModelHealthPersistenceCoordinator(restartedStore, persistence);
  assert.deepEqual(await restarted.restore(), first);
  assert.equal((await restartedStore.health({ domain: "coding" }))[0].attempts, 1);

  const staleStore = new InMemoryAutonomousModelHealthStore({ clock: () => 202 });
  const stale = new AutonomousModelHealthPersistenceCoordinator(staleStore, persistence);
  await stale.restore();
  await source.recordInvocation(invocation("science", "model-b"));
  const second = await coordinator.flush();
  assert.equal(second.snapshot_generation, 2);
  assert.equal(second.previous_snapshot_digest, first.snapshot_digest);
  await staleStore.recordInvocation(invocation("operations", "model-c"));
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const canonical = textStore.encoded();
  textStore.write(` ${canonical}`);
  await assert.rejects(() => persistence.read(), /not canonical/);
  textStore.write(canonical);
});

test("model catalogue snapshots survive restart and fail closed atomically", async () => {
  const source = new AutonomousAgent(new LLMRuntime({ fetch: async () => new Response("{}", { status: 200 }) }));
  source.registerModel({ provider: "catalogue-provider", model: "catalogue-model", capabilities: ["reasoning", "code"], context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 1, reliability: 0.95, requires_credential: true, enabled: true });
  let persisted = null;
  const persistence = { read: () => persisted, write: (snapshot) => { persisted = structuredClone(snapshot); } };
  const snapshot = await new AutonomousModelCataloguePersistenceCoordinator(source, persistence).flush();
  assert.equal(snapshot.models.length, 1);
  assert.equal(JSON.stringify(snapshot).includes("api_key"), false);
  assert.equal(JSON.stringify(snapshot).includes("catalogue-provider"), true);

  const restarted = new AutonomousAgent(new LLMRuntime({ fetch: async () => new Response("{}", { status: 200 }) }));
  restarted.registerModel({ provider: "stale-provider", model: "stale-model", capabilities: ["reasoning"], context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.5, latency_ms: 200, cost_per_million_tokens: 10, reliability: 0.5 });
  const restored = await new AutonomousModelCataloguePersistenceCoordinator(restarted, persistence).restore();
  assert.equal(restored?.snapshot_digest, snapshot.snapshot_digest);
  assert.deepEqual(restarted.models().map((model) => `${model.provider}/${model.model}`), ["catalogue-provider/catalogue-model"]);
  assert.equal((await restarted.readiness()).domains.length, 12);

  const beforeTamper = restarted.models();
  const tampered = structuredClone(snapshot);
  tampered.models[0].model = "tampered-model";
  await assert.rejects(restarted.restoreModels(tampered), /catalogue digest mismatch/);
  assert.deepEqual(restarted.models(), beforeTamper);

  const duplicate = structuredClone(snapshot);
  duplicate.models.push(structuredClone(duplicate.models[0]));
  await assert.rejects(validateAutonomousModelCatalogueSnapshot(duplicate), /duplicate model/);

  const secretShaped = structuredClone(snapshot);
  secretShaped.models[0].api_key = "must-not-enter-metadata";
  await assert.rejects(validateAutonomousModelCatalogueSnapshot(secretShaped), /unsupported or secret-shaped metadata/);
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

test("AutonomousAgent wires a persisted health store into selection and invocation automatically", async () => {
  const store = new InMemoryAutonomousModelHealthStore();
  const llm = new LLMRuntime({
    fetch: async () => new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "bounded answer" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } }),
  });
  llm.registerProvider(openaiCompatibleProvider("health-provider", "https://health.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { modelHealthStore: store });
  agent.registerModel({ provider: "health-provider", model: "health-model", capabilities: ["reasoning", "code"], context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 1, reliability: 0.95 });
  const result = await agent.run("Debug this coding repository.", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal((await store.health({ model: "health-model" }))[0].attempts, 1);
  assert.equal(JSON.stringify(await store.snapshot()).includes("bounded answer"), false);
});

test("online learner adapts through persisted health gates across every built-in domain", async () => {
  const llm = new LLMRuntime();
  for (const provider of ["adaptive-alpha", "adaptive-beta"]) {
    llm.registerInMemoryProvider(provider, () => ({ output_text: "bounded adaptive result" }));
  }
  const store = new InMemoryAutonomousModelHealthStore();
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0.5, seed: 7 } });
  const agent = new AutonomousAgent(llm, { learner, modelHealthStore: store });
  const capabilities = ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"];
  for (const provider of ["adaptive-alpha", "adaptive-beta"]) {
    agent.registerModel({
      provider,
      model: "adaptive-model",
      capabilities,
      context_window_tokens: 64_000,
      max_output_tokens: 8_000,
      quality: 0.8,
      latency_ms: 20,
      cost_per_million_tokens: 1,
      reliability: 0.9,
      requires_credential: false,
    });
  }
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = `Execute a bounded adaptive task for ${domain}.`;
    const envelope = await agent.blueprint(task, { domain });
    const blueprint = envelope.blueprint;
    const context = {
      domain: blueprint.domain_profile.domain,
      capability: blueprint.selection_context.capability,
      risk_class: blueprint.domain_profile.risk_class,
      task_family: blueprint.selection_context.task_family ?? null,
    };
    const contextDigest = digestCanonicalJsonTextSync(JSON.stringify(context));
    await agent.recordEvaluatorReward("adaptive-beta/adaptive-model", 0.95, {
      contextDigest,
      context,
      outcomeDigest: digestCanonicalJsonTextSync(JSON.stringify({ domain, seed: "adaptive-beta" })),
    });
    const result = await agent.run(task, { domain, approveProviderCall: true });
    assert.equal(result.status, "completed", domain);
    assert.deepEqual(result.selection.selected_model, { provider: "adaptive-beta", model: "adaptive-model" }, domain);
  }
  assert.equal(learner.snapshot().generation, AUTONOMOUS_DOMAIN_NAMES.length);
  const healthRows = await store.health({ model: "adaptive-model" });
  assert.equal(healthRows.length, 1);
  assert.equal(healthRows[0].provider, "adaptive-beta");
  assert.equal(healthRows[0].attempts, AUTONOMOUS_DOMAIN_NAMES.length);
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
  assert.equal((await validateAutonomousReplayReport(first)).report_digest, first.report_digest);
  await assert.rejects(engine.replay([]), /1\.\.4096/);
  await assert.rejects(engine.replay([cases[0], cases[0]]), /run_id values must be unique/);
  const forged = { ...drifted, mismatch_count: 0 };
  const { report_digest: _reportDigest, ...forgedBody } = forged;
  forged.report_digest = await digestJson(forgedBody);
  await assert.rejects(validateAutonomousReplayReport(forged), /counts do not match/);
});

test("replay evidence digest matches the Python and Rust canonical contract", async () => {
  const evidenceDigest = await autonomousReplayEvidenceDigest({
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    signals: { schema_valid: true, tests_passed: true, evidence_complete: true },
  });
  assert.equal(evidenceDigest, "8456bae1d2a724352898c152ed09b4a9d2c0ffdd5442c83973914bf12fb2e1f4");
});

test("control-plane bridge sends health and replay metadata only", async () => {
  let healthArgs;
  let replayArgs;
  const bridge = new AutonomousBrainControlPlaneBridge({
    brainModelHealth: async (args) => { healthArgs = args; return { ok: true }; },
    brainReplayEvaluate: async (args) => { replayArgs = args; return { ok: true }; },
  });
  await bridge.recordObservation({
    provider: "provider-a",
    model: "model-a",
    domain: "coding",
    capability: "reasoning",
    risk_class: "review_required",
    status: "completed",
    outcome: "success",
    latency_ms: 101.4,
    input_tokens: 10,
    output_tokens: 20,
    outcome_digest: digest,
  });
  assert.deepEqual(healthArgs, { operation: "record", provider: "provider-a", model: "model-a", status: "success", latency_ms: 101, tokens: 30 });
  assert.equal(Object.keys(healthArgs).some((key) => /prompt|response|credential|token/i.test(key) && key !== "tokens"), false);

  await bridge.replay({
    run_id: "remote-case",
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    evaluator_id: "engineering-quality",
    evaluator_version: "1",
    execution_status: "completed",
    signals: { schema_valid: true, tests_passed: true, evidence_complete: true },
    references: ["b".repeat(64)],
    limitations: ["caller declared numeric signals"],
  });
  assert.equal(replayArgs.case_id, "remote-case");
  assert.equal(replayArgs.evidence_digest, await autonomousReplayEvidenceDigest({
    domain: "engineering",
    capability: "code_change",
    risk_class: "reversible",
    signals: { schema_valid: true, tests_passed: true, evidence_complete: true },
    references: ["b".repeat(64)],
    limitations: ["caller declared numeric signals"],
  }));
  assert.equal("evaluator_id" in replayArgs, false);
  assert.equal("task" in replayArgs, false);
  assert.equal("credential" in replayArgs, false);
});

test("AutonomousAgent can mirror invocation health to the remote control plane", async () => {
  const healthCalls = [];
  const bridge = new AutonomousBrainControlPlaneBridge({
    brainModelHealth: async (args) => {
      if (args.operation === "snapshot") return { ok: true, mcp: { result: { structuredContent: { ok: true, operation: "snapshot", models: [{ provider: "remote-health-provider", model: "remote-health-model", attempts: 1, successes: 1, failures: 0, consecutive_failures: 0, average_latency_ms: 10, average_quality: 1, quality_observations: 1, last_status: "success", last_sequence: 1, registered: true, credential_ready: true, eligible: true }] } } } };
      healthCalls.push(args);
      return { ok: true };
    },
    brainReplayEvaluate: async () => ({ ok: true }),
  });
  const llm = new LLMRuntime({
    fetch: async () => new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: "remote health answer" }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } }),
  });
  llm.registerProvider(openaiCompatibleProvider("remote-health-provider", "https://remote-health.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { modelHealthBridge: bridge });
  agent.registerModel({ provider: "remote-health-provider", model: "remote-health-model", capabilities: ["reasoning", "code"], context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 1, reliability: 0.95 });
  const result = await agent.run("Review this bounded code change.", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(healthCalls.length, 1);
  assert.equal(healthCalls[0].provider, "remote-health-provider");
  assert.equal(healthCalls[0].model, "remote-health-model");
  assert.equal(healthCalls[0].status, "success");
  assert.equal("response" in healthCalls[0], false);
  assert.equal("prompt" in healthCalls[0], false);
});

test("remote persisted health drives selection for every built-in domain without bypassing local readiness", async () => {
  const profiles = await builtinAutonomousDomainEvaluatorProfiles();
  const remoteRows = [
    { provider: "provider-a", model: "model-a", attempts: 10, successes: 2, failures: 8, consecutive_failures: 4, average_latency_ms: 100, average_quality: 0.2, quality_observations: 10, last_status: "failure", last_sequence: 1, registered: true, credential_ready: true, eligible: false },
    { provider: "provider-b", model: "model-b", attempts: 10, successes: 9, failures: 1, consecutive_failures: 0, average_latency_ms: 80, average_quality: 0.95, quality_observations: 10, last_status: "success", last_sequence: 2, registered: true, credential_ready: true, eligible: true },
  ];
  const bridge = new AutonomousBrainControlPlaneBridge({
    brainModelHealth: async (args) => args.operation === "snapshot"
      ? { ok: true, mcp: { result: { structuredContent: { ok: true, operation: "snapshot", models: remoteRows } } } }
      : { ok: true },
    brainReplayEvaluate: async () => ({ ok: true }),
  });
  const select = bridge.selector();
  const providerHealth = {
    "provider-a": { provider: "provider-a", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true },
    "provider-b": { provider: "provider-b", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true },
  };
  for (const profile of profiles) {
    const decision = await select({
      task: `bounded ${profile.domain} task`,
      domain: profile.domain,
      capability: "reasoning",
      risk_class: "review_required",
      required_capabilities: [],
      estimated_input_tokens: 100,
      requested_output_tokens: 100,
      candidates: [
        { provider: "provider-a", model: "model-a", capabilities: ["reasoning"], context_window_tokens: 10_000, max_output_tokens: 1_000, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
        { provider: "provider-b", model: "model-b", capabilities: ["reasoning"], context_window_tokens: 10_000, max_output_tokens: 1_000, quality: 0.7, latency_ms: 300, cost_per_million_tokens: 50, reliability: 0.7 },
      ],
      provider_health: providerHealth,
      model_health: {},
    });
    assert.deepEqual(decision.selected_model, { provider: "provider-b", model: "model-b" });
    assert.equal(decision.ranking.find((row) => row.model === "model-a").eligible, false);
  }
});

test("remote selection fails closed on an incomplete health snapshot", async () => {
  const bridge = new AutonomousBrainControlPlaneBridge({
    brainModelHealth: async () => ({ ok: true }),
    brainReplayEvaluate: async () => ({ ok: true }),
  });
  await assert.rejects(bridge.selector()({
    task: "bounded task",
    domain: "coding",
    capability: "reasoning",
    risk_class: "review_required",
    required_capabilities: [],
    estimated_input_tokens: 1,
    requested_output_tokens: 1,
    candidates: [{ provider: "provider", model: "model", capabilities: ["reasoning"], context_window_tokens: 1_000, max_output_tokens: 100, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 1, reliability: 0.5 }],
    provider_health: { provider: { provider: "provider", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true } },
    model_health: {},
  }), /remote model health snapshot returned a refusal/);
});

test("remote selection refuses duplicate persisted health rows", async () => {
  const row = { provider: "provider", model: "model", attempts: 1, successes: 1, failures: 0, consecutive_failures: 0, average_latency_ms: 10, average_quality: null, quality_observations: 0, last_status: "success", last_sequence: 1, registered: true, credential_ready: true, eligible: true };
  const bridge = new AutonomousBrainControlPlaneBridge({
    brainModelHealth: async () => ({ ok: true, mcp: { result: { structuredContent: { ok: true, operation: "snapshot", models: [row, { ...row, last_sequence: 2 }] } } } }),
    brainReplayEvaluate: async () => ({ ok: true }),
  });
  await assert.rejects(bridge.selector()({
    task: "bounded task",
    domain: "coding",
    capability: "reasoning",
    risk_class: "review_required",
    required_capabilities: [],
    estimated_input_tokens: 1,
    requested_output_tokens: 1,
    candidates: [{ provider: "provider", model: "model", capabilities: ["reasoning"], context_window_tokens: 1_000, max_output_tokens: 100, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 1, reliability: 0.5 }],
    provider_health: { provider: { provider: "provider", circuit: "closed", consecutive_failures: 0, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null, credential_posture: "caller_supplied_opaque_handle", credential_required: false, credential_ready: true } },
    model_health: {},
  }), /duplicate model rows/);
});
