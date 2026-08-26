import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER,
  AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER,
  AutonomousAgent,
  AutonomousAgentPersistenceLifecycleCoordinator,
  AutonomousAgentPersistenceLifecycleError,
  AutonomousCapabilityActivationStore,
  AutonomousSelectionPromotionLifecycle,
  AutonomousSelectionPromotionLifecycleStore,
  LLMRuntime,
} from "../dist/index.js";

function value(component, operation) {
  return {
    schema: `test/${component}/${operation}`,
    snapshot_digest: "a".repeat(64),
    state_digest: "b".repeat(64),
    generation: 3,
  };
}

function fakeAgent(calls, failure = null) {
  const agent = {
    selectionPromotion: {},
    runtimeHealthPersistence: {},
    healthPersistence: {},
    evaluatorCalibrationPersistence: {},
    memoryPersistence: {},
    learnerPersistence: {},
    promptLearningCoordinator: {},
    restoreModelInventory: async () => { calls.push("restore:model_inventory"); return value("model_inventory", "restore"); },
    flushModelInventory: async () => { calls.push("flush:model_inventory"); return value("model_inventory", "flush"); },
    restoreActivation: async () => { calls.push("restore:activation"); return value("activation", "restore"); },
    saveActivation: async () => { calls.push("flush:activation"); return value("activation", "flush"); },
    restoreSelectionPromotion: async () => { calls.push("restore:selection_promotion"); return value("selection_promotion", "restore"); },
    saveSelectionPromotion: async () => { calls.push("flush:selection_promotion"); return value("selection_promotion", "flush"); },
  };
  for (const component of ["runtime_health", "health", "evaluator_calibration", "memory", "learning", "prompt_learning"]) {
    const suffix = component === "learning" ? "OnlineLearning" : component.replace(/(^|_)([a-z])/g, (_, prefix, letter) => letter.toUpperCase());
    agent[`restore${suffix}`] = async () => {
      calls.push(`restore:${component}`);
      if (failure === component) throw new Error("private task/prompt/provider payload must not escape");
      return value(component, "restore");
    };
    agent[`flush${suffix}`] = async () => {
      calls.push(`flush:${component}`);
      if (failure === component) throw new Error("private task/prompt/provider payload must not escape");
      return value(component, "flush");
    };
  }
  return agent;
}

function persistence() {
  let stored = null;
  return {
    read: () => stored,
    write: (snapshot) => { stored = structuredClone(snapshot); },
    writeIfUnchanged: (expected, snapshot) => {
      const observed = stored?.inventory_digest ?? null;
      if (observed !== expected) return false;
      stored = structuredClone(snapshot);
      return true;
    },
  };
}

test("agent persistence lifecycle restores and flushes in explicit dependency order", async () => {
  const calls = [];
  const coordinator = new AutonomousAgentPersistenceLifecycleCoordinator(fakeAgent(calls), {
    modelInventoryPersistence: { read: () => null, write: () => {} },
    activationStore: { load: () => null, save: () => {} },
    selectionPromotionStore: { load: () => null, save: () => {} },
    requireAll: true,
  });
  const restored = await coordinator.restore();
  assert.equal(restored.status, "completed");
  assert.deepEqual(restored.ordered_component_ids, [...AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER]);
  assert.deepEqual(calls, AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER.map((component) => `restore:${component}`));
  calls.length = 0;
  const flushed = await coordinator.flush();
  assert.equal(flushed.status, "completed");
  assert.deepEqual(flushed.ordered_component_ids, [...AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER]);
  assert.deepEqual(calls, AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER.map((component) => `flush:${component}`));
  assert.equal(flushed.atomicity, "per_component_cas_only;cross_store_atomicity_caller_owned");
});

test("strict lifecycle failure preserves a redacted report and stops after the failed component", async () => {
  const calls = [];
  const coordinator = new AutonomousAgentPersistenceLifecycleCoordinator(fakeAgent(calls, "health"), {
    modelInventoryPersistence: { read: () => null, write: () => {} },
    activationStore: { load: () => null, save: () => {} },
    selectionPromotionStore: { load: () => null, save: () => {} },
    requireAll: true,
  });
  await assert.rejects(
    () => coordinator.restore({ strict: true }),
    (error) => {
      assert.ok(error instanceof AutonomousAgentPersistenceLifecycleError);
      assert.equal(error.report.failed_component_id, "health");
      assert.equal(error.report.components[2].status, "failed");
      assert.equal(error.report.components[2].error_class, "Error");
      assert.equal(error.report.components[5].status, "not_attempted");
      assert.doesNotMatch(JSON.stringify(error.report), /private task\/prompt\/provider payload/);
      return true;
    },
  );
  assert.deepEqual(calls, ["restore:model_inventory", "restore:runtime_health", "restore:health"]);
});

test("high-level agent lifecycle composes model inventory restart and flush without rediscovery", async () => {
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", () => "unused", {
    discoverModels: async () => ({ data: [{ id: "lifecycle-model", active: true, context_window: 16_000, max_output_tokens: 1_000, capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"] }] }),
  });
  const store = persistence();
  const activationStore = new AutonomousCapabilityActivationStore();
  const selectionStore = new AutonomousSelectionPromotionLifecycleStore();
  const agent = new AutonomousAgent(llm, { selectionPromotion: new AutonomousSelectionPromotionLifecycle() });
  const snapshot = await agent.refreshModelInventory([{
    provider: "offline",
    defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.9 },
  }], { persistence: store, refreshId: "lifecycle-inventory" });
  await agent.saveActivation(activationStore);
  await agent.saveSelectionPromotion(selectionStore);
  const restarted = new AutonomousAgent(llm, { selectionPromotion: new AutonomousSelectionPromotionLifecycle() });
  const restored = await restarted.restorePersistedState({
    modelInventoryPersistence: store,
    activationStore,
    selectionPromotionStore: selectionStore,
    strict: false,
  });
  assert.equal(restored.components[0].status, "restored");
  assert.equal(restored.components[3].status, "restored");
  assert.equal(restored.components[4].status, "restored");
  assert.equal(restored.components[0].snapshot_digest, snapshot.inventory_digest);
  assert.equal(restarted.models()[0].model, "lifecycle-model");
  const flushed = await restarted.flushPersistedState({
    modelInventoryPersistence: store,
    activationStore,
    selectionPromotionStore: selectionStore,
    strict: false,
  });
  assert.equal(flushed.components[8].status, "flushed");
  assert.equal(flushed.components[8].snapshot_digest, snapshot.inventory_digest);
  assert.doesNotMatch(JSON.stringify(restored), /credentials|lifecycle-model/);
});
