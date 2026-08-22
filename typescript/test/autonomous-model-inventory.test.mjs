import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousModelInventoryCoordinator,
  LLMRuntime,
} from "../dist/index.js";

const capabilities = [
  "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
  "operations", "enterprise", "coordination", "multimodal", "evaluation",
];

function runtime() {
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", (request) => ({ output_text: `offline:${request.model}` }), {
    discoverModels: async () => ({ data: [{ id: "discovered-model", active: true, context_window: 32_000, max_output_tokens: 2_000, capabilities }] }),
  });
  return llm;
}

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
