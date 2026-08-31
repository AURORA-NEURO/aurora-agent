import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousToolSelectionPersistenceCoordinator,
  JsonAutonomousToolSelectionPersistence,
  LLMRuntime,
  TransactionalJsonAutonomousToolSelectionPersistence,
  canonicalJson,
  normalizeAutonomousToolSelectionState,
  settleAutonomousToolSelectionOutcome,
  validateAutonomousToolSelectionSnapshot,
} from "../dist/index.js";

function store() {
  let value = null;
  return {
    read: () => value,
    write: (next) => { value = next; },
    writeIfUnchanged: (expected, next) => {
      const observed = value === null ? null : JSON.parse(value).snapshot_digest;
      if (observed !== expected) return false;
      value = next;
      return true;
    },
    value: () => value,
  };
}

test("tool selection snapshots are canonical, chained, bounded, and CAS-fenced", async () => {
  let state = normalizeAutonomousToolSelectionState();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    state = settleAutonomousToolSelectionOutcome(state, {
      domain,
      capability: "read_only_analysis",
      tool: `fixture_${domain}`,
      reward: 0.5,
      outcomeDigest: `${String(AUTONOMOUS_DOMAIN_NAMES.indexOf(domain) + 1).padStart(2, "0")}${"a".repeat(62)}`,
    });
  }
  const textStore = store();
  const persistence = new TransactionalJsonAutonomousToolSelectionPersistence(textStore);
  const binding = { get: () => state, set: (next) => { state = next; } };
  const coordinator = new AutonomousToolSelectionPersistenceCoordinator(binding, persistence);
  assert.equal(await coordinator.restore(), null);
  const first = await coordinator.flush();
  assert.equal(first.snapshot_generation, 1);
  assert.equal(first.previous_snapshot_digest, null);
  assert.equal(first.state.arms.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(canonicalJson(first), textStore.value());
  const second = await coordinator.flush();
  assert.equal(second.snapshot_generation, 2);
  assert.equal(second.previous_snapshot_digest, first.snapshot_digest);
  await assert.rejects(
    () => validateAutonomousToolSelectionSnapshot({ ...second, state_digest: "b".repeat(64) }),
    /state digest does not match/,
  );
  const stale = new TransactionalJsonAutonomousToolSelectionPersistence(textStore);
  const staleCoordinator = new AutonomousToolSelectionPersistenceCoordinator(binding, stale);
  await staleCoordinator.restore();
  const third = await coordinator.flush();
  assert.equal(third.snapshot_generation, 3);
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap conflict/);
});
test("AutonomousAgent owns tool learning, uses it by default, and lifecycle restores it", async () => {
  const textStore = store();
  const persistence = new TransactionalJsonAutonomousToolSelectionPersistence(textStore);
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } });
  const source = new AutonomousAgent(runtime, { toolSelectionPersistence: persistence });
  const recorded = source.recordToolSelectionReward({
    domain: "coding",
    capability: "read_only_analysis",
    tool: "fixture_tool",
    reward: 1,
    outcomeDigest: "c".repeat(64),
  });
  assert.equal(recorded.generation, 1);
  const flushed = await source.flushToolSelection();
  assert.equal(flushed.state.arms[0].arm_id, "coding.read_only_analysis.fixture_tool");

  const restarted = new AutonomousAgent(runtime, { toolSelectionPersistence: new TransactionalJsonAutonomousToolSelectionPersistence(textStore) });
  const restored = await restarted.restorePersistedState({ strict: false });
  const row = restored.components.find((component) => component.component_id === "tool_selection");
  assert.equal(row.status, "restored");
  assert.equal(restarted.toolSelectionState().generation, 1);
  const lifecycle = await restarted.flushPersistedState({ strict: false });
  const flushedRow = lifecycle.components.find((component) => component.component_id === "tool_selection");
  assert.equal(flushedRow.status, "flushed");
  assert.equal(flushedRow.generation, 2);
});

test("plain JSON tool selection persistence remains available without CAS", async () => {
  let encoded = null;
  const persistence = new JsonAutonomousToolSelectionPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
  });
  const state = settleAutonomousToolSelectionOutcome(undefined, {
    domain: "cross_domain",
    capability: "synthesis",
    tool: "fixture_synthesis",
    reward: 0,
  });
  const binding = { get: () => state, set: () => {} };
  const coordinator = new AutonomousToolSelectionPersistenceCoordinator(binding, persistence);
  await coordinator.flush();
  assert.equal((await persistence.read()).state.generation, 1);
});
