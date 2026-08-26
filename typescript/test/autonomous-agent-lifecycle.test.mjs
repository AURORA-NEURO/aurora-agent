import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_AGENT_LIFECYCLE_FLUSH_ORDER,
  AUTONOMOUS_AGENT_LIFECYCLE_RESTORE_ORDER,
  AutonomousAgent,
  AutonomousAgentPersistenceLifecycleCoordinator,
  AutonomousAgentPersistenceLifecycleError,
  AutonomousCapabilityActivationStore,
  AutonomousCapabilityJournalPersistenceCoordinator,
  InMemoryAutonomousCapabilityJournalStore,
  AutonomousDecisionCyclePersistenceCoordinator,
  InMemoryAutonomousDecisionCycleStateStore,
  AutonomousExecutionPersistenceCoordinator,
  InMemoryAutonomousExecutionJournal,
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
    capabilityJournalPersistence: {},
    decisionCyclePersistence: {},
    executionPersistence: {},
    restoreModelInventory: async () => { calls.push("restore:model_inventory"); return value("model_inventory", "restore"); },
    flushModelInventory: async () => { calls.push("flush:model_inventory"); return value("model_inventory", "flush"); },
    restoreActivation: async () => { calls.push("restore:activation"); return value("activation", "restore"); },
    saveActivation: async () => { calls.push("flush:activation"); return value("activation", "flush"); },
    restoreSelectionPromotion: async () => { calls.push("restore:selection_promotion"); return value("selection_promotion", "restore"); },
    saveSelectionPromotion: async () => { calls.push("flush:selection_promotion"); return value("selection_promotion", "flush"); },
  };
  agent.restoreCapabilityJournalPersistence = async () => { calls.push("restore:capability_journal"); if (failure === "capability_journal") throw new Error("private task/prompt/provider payload must not escape"); return value("capability_journal", "restore"); };
  agent.flushCapabilityJournalPersistence = async () => { calls.push("flush:capability_journal"); if (failure === "capability_journal") throw new Error("private task/prompt/provider payload must not escape"); return value("capability_journal", "flush"); };
  agent.restoreDecisionCyclePersistence = async () => { calls.push("restore:decision_cycle"); if (failure === "decision_cycle") throw new Error("private task/prompt/provider payload must not escape"); return value("decision_cycle", "restore"); };
  agent.flushDecisionCyclePersistence = async () => { calls.push("flush:decision_cycle"); if (failure === "decision_cycle") throw new Error("private task/prompt/provider payload must not escape"); return value("decision_cycle", "flush"); };
  agent.restoreExecutionPersistence = async () => { calls.push("restore:execution"); if (failure === "execution") throw new Error("private task/prompt/provider payload must not escape"); return value("execution", "restore"); };
  agent.flushExecutionPersistence = async () => { calls.push("flush:execution"); if (failure === "execution") throw new Error("private task/prompt/provider payload must not escape"); return value("execution", "flush"); };
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
  const decisionStore = new InMemoryAutonomousDecisionCycleStateStore();
  const decisionSnapshotStore = { value: null, read() { return this.value; }, write(snapshot) { this.value = structuredClone(snapshot); } };
  const decisionPersistence = new AutonomousDecisionCyclePersistenceCoordinator(decisionStore, decisionSnapshotStore);
  const agent = new AutonomousAgent(llm, { selectionPromotion: new AutonomousSelectionPromotionLifecycle(), decisionCyclePersistence: decisionPersistence });
  const snapshot = await agent.refreshModelInventory([{
    provider: "offline",
    defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.9 },
  }], { persistence: store, refreshId: "lifecycle-inventory" });
  const automatic = await agent.runAutoCycle("Review a bounded coding change.", {
    domain: "coding",
    cycleId: "lifecycle-auto-cycle",
    approveProviderCall: true,
    maxOutputTokens: 512,
  });
  assert.equal(automatic.status, "completed");
  assert.ok(await decisionStore.load("lifecycle-auto-cycle"));
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
  assert.equal(flushed.components[11].status, "flushed");
  assert.equal(flushed.components[11].snapshot_digest, snapshot.inventory_digest);
  assert.doesNotMatch(JSON.stringify(restored), /credentials|lifecycle-model/);
});

test("high-level agent lifecycle carries capability and execution restart barriers", async () => {
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  const capabilitySnapshotStore = { value: null, read() { return this.value; }, write(snapshot) { this.value = structuredClone(snapshot); } };
  const executionSnapshotStore = { value: null, read() { return this.value; }, write(snapshot) { this.value = structuredClone(snapshot); } };
  const capabilityJournal = new InMemoryAutonomousCapabilityJournalStore();
  const capabilityPersistence = new AutonomousCapabilityJournalPersistenceCoordinator(capabilityJournal, capabilitySnapshotStore);
  const executionJournal = new InMemoryAutonomousExecutionJournal();
  const executionPersistence = new AutonomousExecutionPersistenceCoordinator(executionJournal, executionSnapshotStore);
  const decisionJournal = new InMemoryAutonomousDecisionCycleStateStore();
  const decisionSnapshotStore = { value: null, read() { return this.value; }, write(snapshot) { this.value = structuredClone(snapshot); } };
  const decisionPersistence = new AutonomousDecisionCyclePersistenceCoordinator(decisionJournal, decisionSnapshotStore);
  const source = new AutonomousAgent(llm, { capabilityJournal, capabilityJournalPersistence: capabilityPersistence, executionJournal, executionPersistence, decisionCyclePersistence: decisionPersistence });
  const flushed = await source.flushPersistedState({ strict: false });
  assert.deepEqual(flushed.ordered_component_ids.slice(0, 3), ["execution", "decision_cycle", "capability_journal"]);
  assert.equal(flushed.components[0].status, "flushed");
  assert.equal(flushed.components[1].status, "flushed");
  assert.ok(flushed.components[1].snapshot_digest);

  const restoredCapabilityJournal = new InMemoryAutonomousCapabilityJournalStore();
  const restoredExecutionJournal = new InMemoryAutonomousExecutionJournal();
  const restoredDecisionJournal = new InMemoryAutonomousDecisionCycleStateStore();
  const restored = new AutonomousAgent(llm, {
    capabilityJournal: restoredCapabilityJournal,
    capabilityJournalPersistence: new AutonomousCapabilityJournalPersistenceCoordinator(restoredCapabilityJournal, capabilitySnapshotStore),
    executionJournal: restoredExecutionJournal,
    executionPersistence: new AutonomousExecutionPersistenceCoordinator(restoredExecutionJournal, executionSnapshotStore),
    decisionCyclePersistence: new AutonomousDecisionCyclePersistenceCoordinator(restoredDecisionJournal, decisionPersistence.persistence),
  });
  const report = await restored.restorePersistedState({ strict: false });
  assert.equal(report.components[9].status, "restored");
  assert.equal(report.components[10].status, "restored");
  assert.equal(report.components[11].status, "restored");
  assert.equal(report.components[10].snapshot_digest, flushed.components[1].snapshot_digest);
  assert.equal(report.components[9].generation, 1);
  assert.doesNotMatch(JSON.stringify(report), /entries|rows|credentials/);
});
