import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousModelInventoryCoordinator,
  AutonomousBrainFacade,
  TransactionalJsonAutonomousModelInventorySnapshotPersistence,
  validateAutonomousModelInventoryReadiness,
  LLMRuntime,
  ProviderSetup,
} from "../dist/index.js";

const capabilities = [
  "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
  "operations", "enterprise", "coordination", "multimodal", "evaluation",
];

function runtime(onRequest = () => {}) {
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline:${request.model}` };
  }, {
    discoverModels: async () => ({ data: [{ id: "discovered-model", active: true, context_window: 32_000, max_output_tokens: 2_000, capabilities }] }),
  });
  return llm;
}

const allDomainModel = {
  provider: "offline",
  model: "readiness-model",
  capabilities,
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.8,
  latency_ms: 20,
  cost_per_million_tokens: 0,
  reliability: 0.95,
  enabled: true,
};

test("model inventory readiness is a provider-free all-domain eligibility projection", async () => {
  let invocations = 0;
  const agent = new AutonomousAgent(runtime(() => { invocations += 1; }));
  agent.registerModel(allDomainModel);
  const before = agent.models();
  const report = await agent.modelInventoryReadiness();

  assert.equal(report.schema, "bioprism-typescript-autonomous-model-inventory-readiness/0.1");
  assert.equal(report.readiness, "ready");
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(new Set(report.domains.map((row) => row.domain)), new Set(AUTONOMOUS_DOMAIN_NAMES));
  assert.ok(report.domains.every((row) => row.coverage_state === "complete" && row.compatible_model_count === 1 && row.eligible_model_count === 1));
  assert.equal(report.domains[0].provider_readiness.offline.registered, true);
  assert.equal(report.domains[0].provider_readiness.offline.credential_ready, true);
  assert.equal(report.domains[0].provider_readiness.offline.circuit, "closed");
  assert.equal(invocations, 0);
  assert.deepEqual(agent.models(), before);

  const validated = await validateAutonomousModelInventoryReadiness(report);
  assert.equal(validated.readiness_digest, report.readiness_digest);
  const facade = new AutonomousBrainFacade({ agent });
  const facadeReport = await facade.modelInventoryReadiness();
  assert.equal(facadeReport.readiness_digest, report.readiness_digest);
  assert.equal(invocations, 0);
});

test("model inventory readiness distinguishes capacity and provider eligibility failures", async () => {
  const undersizedAgent = new AutonomousAgent(runtime());
  undersizedAgent.registerModel({ ...allDomainModel, model: "too-small", context_window_tokens: 512, max_output_tokens: 64 });
  const undersized = await undersizedAgent.modelInventoryReadiness({ estimatedInputTokens: 1_024, requestedOutputTokens: 128 });
  assert.equal(undersized.readiness, "missing");
  assert.ok(undersized.domains.every((row) => row.coverage_state === "missing" && row.compatible_model_count === 0));

  const unregisteredAgent = new AutonomousAgent(runtime());
  unregisteredAgent.registerModel({ ...allDomainModel, provider: "not-registered", model: "unconfigured-model" });
  const unregistered = await unregisteredAgent.modelInventoryReadiness();
  assert.equal(unregistered.readiness, "partial");
  assert.ok(unregistered.domains.every((row) => row.coverage_state === "partial" && row.compatible_model_count === 1 && row.eligible_model_count === 0));
  assert.equal(unregistered.domains[0].provider_readiness["not-registered"].registered, false);
  assert.equal(unregistered.domains[0].provider_readiness["not-registered"].credential_ready, false);
  assert.equal(unregistered.domains[0].provider_readiness["not-registered"].circuit, "unconfigured");
});

test("model inventory readiness rejects stale, tampered, and malformed projections", async () => {
  const agent = new AutonomousAgent(runtime());
  agent.registerModel(allDomainModel);
  const report = await agent.modelInventoryReadiness();

  const tamperedCount = structuredClone(report);
  tamperedCount.domains[0].eligible_model_count = 0;
  await assert.rejects(() => validateAutonomousModelInventoryReadiness(tamperedCount), /counts do not match|digest mismatch/);

  const tamperedArm = structuredClone(report);
  tamperedArm.domains[0].eligible_model_ids = ["offline/unknown"];
  await assert.rejects(() => validateAutonomousModelInventoryReadiness(tamperedArm), /eligible models must be compatible|unknown model|digest mismatch/);

  const wrongBudget = structuredClone(report);
  wrongBudget.estimated_input_tokens = 10_000_001;
  await assert.rejects(() => validateAutonomousModelInventoryReadiness(wrongBudget), /outside its bounds|digest mismatch/);

  await assert.rejects(() => agent.modelInventoryReadiness({ domainRequirements: { unknown: ["reasoning"] } }), /unknown domain/);
  await assert.rejects(() => agent.modelInventoryReadiness({ estimatedInputTokens: 0 }), /outside its bounds/);
});

test("model inventory reconciles discovery into all-domain coverage without claiming quality", async () => {
  const agent = new AutonomousAgent(runtime());
  const persistence = { value: null, read() { return this.value; }, write(snapshot) { this.value = structuredClone(snapshot); } };
  const inventory = new AutonomousModelInventoryCoordinator(agent, persistence);
  const snapshot = await inventory.refresh([
    {
      provider: "offline",
      defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
    },
  ], { refreshId: "offline-inventory-1" });

  assert.equal(snapshot.schema, "bioprism-typescript-autonomous-model-inventory/0.1");
  assert.equal(snapshot.status, "completed");
  assert.equal(snapshot.readiness, "ready");
  assert.equal(snapshot.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(snapshot.domains.every((row) => row.coverage_state === "complete" && row.eligible_model_count === 1));
  assert.equal(snapshot.models[0].model, "discovered-model");
  assert.equal(snapshot.models[0].quality, 0.8);
  assert.equal(snapshot.selection_posture, "candidate_metadata_and_provider_readiness_only; evaluator_evidence_still_required");
  assert.doesNotMatch(JSON.stringify(snapshot), /offline:discovered-model/);

  const restoredAgent = new AutonomousAgent(runtime());
  const restored = await new AutonomousModelInventoryCoordinator(restoredAgent, persistence).restore();
  assert.equal(restored.inventory_digest, snapshot.inventory_digest);
  assert.equal(restoredAgent.models()[0].model, "discovered-model");
});
test("model inventory reports partial provider discovery and rejects tampered snapshots", async () => {
  const agent = new AutonomousAgent(runtime());
  const snapshot = await new AutonomousModelInventoryCoordinator(agent).refresh([
    {
      provider: "offline",
      defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
    },
    {
      provider: "missing-provider",
      defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
    },
  ]);
  assert.equal(snapshot.status, "partial");
  assert.equal(snapshot.refresh.failed_provider_count, 1);
  assert.equal(snapshot.refresh.failures[0].provider, "missing-provider");
  assert.equal(snapshot.refresh.failures[0].error_class, "ProviderRuntimeError");
  const tampered = structuredClone(snapshot);
  tampered.models[0].quality = 1;
  await assert.rejects(() => new AutonomousModelInventoryCoordinator(agent).restore({ read: () => tampered, write: () => {} }), /digest mismatch/);
});

test("model inventory JSON persistence fences stale refresh writers", async () => {
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = encoded === null ? null : JSON.parse(encoded).inventory_digest;
      if (observed !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const persistence = new TransactionalJsonAutonomousModelInventorySnapshotPersistence(textStore);
  const specs = [{
    provider: "offline",
    defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
  }];
  const coordinator = new AutonomousModelInventoryCoordinator(new AutonomousAgent(runtime()), persistence);
  const first = await coordinator.refresh(specs, { refreshId: "inventory-cas-1" });
  assert.equal(JSON.parse(encoded).inventory_digest, first.inventory_digest);

  const stale = new AutonomousModelInventoryCoordinator(new AutonomousAgent(runtime()), persistence);
  await assert.rejects(() => stale.refresh(specs, { refreshId: "inventory-cas-stale" }), /compare-and-swap/);
  assert.equal(JSON.parse(encoded).inventory_digest, first.inventory_digest);

  const restored = new AutonomousModelInventoryCoordinator(new AutonomousAgent(runtime()), persistence);
  const recovered = await restored.restore();
  assert.equal(recovered.inventory_digest, first.inventory_digest);
  const next = await restored.refresh(specs, { refreshId: "inventory-cas-2" });
  assert.notEqual(next.inventory_digest, first.inventory_digest);
});

test("AutonomousAgent reuses its inventory coordinator across refreshes and restore", async () => {
  let stored = null;
  const persistence = {
    read: () => stored,
    write: (snapshot) => { stored = structuredClone(snapshot); },
    writeIfUnchanged: (expected, snapshot) => {
      const observed = stored?.inventory_digest ?? null;
      if (observed !== expected) return false;
      stored = structuredClone(snapshot);
      return true;
    },
  };
  const specs = [{
    provider: "offline",
    defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
  }];
  const agent = new AutonomousAgent(runtime());
  const first = await agent.refreshModelInventory(specs, { persistence, refreshId: "agent-inventory-1" });
  const second = await agent.refreshModelInventory(specs, { persistence, replaceExisting: true, refreshId: "agent-inventory-2" });
  assert.notEqual(second.inventory_digest, first.inventory_digest);
  assert.equal(stored.inventory_digest, second.inventory_digest);

  const restarted = new AutonomousAgent(runtime());
  const restored = await restarted.restoreModelInventory(persistence);
  assert.equal(restored.inventory_digest, second.inventory_digest);
  assert.equal(restarted.models()[0].model, "discovered-model");
  const resumed = await restarted.refreshModelInventory(specs, { persistence, replaceExisting: true, refreshId: "agent-inventory-3" });
  assert.notEqual(resumed.inventory_digest, second.inventory_digest);
  assert.equal(stored.inventory_digest, resumed.inventory_digest);
});

test("provider setup bridges an opaque session into agent inventory refresh", async () => {
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  const setup = new ProviderSetup(llm);
  setup.registerProvider("groq", {
    transport: {
      invoke: () => ({ output_text: "transient" }),
      discoverModels: async () => ({ data: [{ id: "session-model", context_window: 32_000, max_output_tokens: 2_000, capabilities }] }),
    },
  });
  const session = setup.startSession({ sessionId: "inventory-session" });
  const agent = new AutonomousAgent(llm);
  const snapshot = await setup.refreshModelInventory(agent, session, [{
    provider: "groq",
    defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.75, latency_ms: 30, cost_per_million_tokens: 1, reliability: 0.9 },
  }], { refreshId: "session-inventory-1" });
  assert.equal(snapshot.readiness, "ready");
  assert.equal(snapshot.models[0].model, "session-model");
  assert.doesNotMatch(JSON.stringify(snapshot), /transient|inventory-session/);
  session.close();
});
