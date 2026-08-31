import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousOnlineLearner,
  AutonomousOnlineLearnerPersistenceCoordinator,
  AutonomousAgent,
  AUTONOMOUS_DOMAIN_NAMES,
  LLMRuntime,
  JsonAutonomousOnlineLearnerSnapshotPersistence,
  TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence,
  WebStorageAutonomousOnlineLearnerSnapshotTextStore,
  snapshotAutonomousOnlineLearner,
  validateAutonomousOnlineLearnerSnapshot,
  digestJson,
  digestCanonicalJsonTextSync,
} from "../dist/index.js";

const digestA = "a".repeat(64);
const digestB = "b".repeat(64);

function transactionalTextStore() {
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

test("online learner snapshots preserve deterministic bandit state without private values", async () => {
  const learner = new AutonomousOnlineLearner();
  learner.update({ arm_id: "offline/offline-model", reward: 0.75, failed: false, outcome_digest: digestA });
  const snapshot = await snapshotAutonomousOnlineLearner(learner);

  assert.equal(snapshot.state.arms[0].arm_id, "offline/offline-model");
  assert.equal(snapshot.state.arms[0].pulls, 1);
  assert.match(snapshot.state_digest, /^[a-f0-9]{64}$/);
  assert.match(snapshot.snapshot_digest, /^[a-f0-9]{64}$/);
  assert.equal(snapshot.snapshot_generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  assert.equal(snapshot.retention, "bandit_arm_and_evaluator_digest_metadata_only");
  assert.equal(snapshot.secret_material, "never_returned");
  assert.doesNotMatch(JSON.stringify(snapshot), /prompt|offline task|credential|api[_-]?key/i);
  assert.deepEqual(await validateAutonomousOnlineLearnerSnapshot(snapshot), snapshot);
});

test("online learner persistence restores state, fences stale writers, and round-trips browser storage", async () => {
  const store = transactionalTextStore();
  const persistence = new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(store);
  const learner = new AutonomousOnlineLearner();
  const coordinator = new AutonomousOnlineLearnerPersistenceCoordinator(learner, persistence);
  learner.update({ arm_id: "offline/model-a", reward: 0.5, outcome_digest: digestA });
  const first = await coordinator.flush();
  assert.equal(first.snapshot_generation, 1);
  assert.equal(first.previous_snapshot_digest, null);


  const restoredLearner = new AutonomousOnlineLearner();
  const restored = new AutonomousOnlineLearnerPersistenceCoordinator(restoredLearner, persistence);
  assert.deepEqual(await restored.restore(), first);
  assert.deepEqual(restoredLearner.snapshot(), learner.snapshot());

  const second = await coordinator.flush();
  assert.equal(second.snapshot_generation, 2);
  assert.equal(second.previous_snapshot_digest, first.snapshot_digest);

  const staleLearner = new AutonomousOnlineLearner();
  const stale = new AutonomousOnlineLearnerPersistenceCoordinator(staleLearner, persistence);
  await stale.restore();
  learner.update({ arm_id: "offline/model-b", reward: -0.25, failed: true, outcome_digest: digestB });
  await coordinator.flush();
  staleLearner.update({ arm_id: "offline/model-c", reward: 0.1, outcome_digest: "c".repeat(64) });
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const values = new Map();
  const browserStore = new WebStorageAutonomousOnlineLearnerSnapshotTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  }, "online-learner-state");
  const browserPersistence = new JsonAutonomousOnlineLearnerSnapshotPersistence(browserStore);
  await browserPersistence.write(await snapshotAutonomousOnlineLearner(learner));
  assert.deepEqual((await browserPersistence.read()).state, learner.snapshot());
  const canonical = values.get("online-learner-state");
  values.set("online-learner-state", JSON.stringify(JSON.parse(canonical), null, 2));
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  values.set("online-learner-state", canonical);
});

test("online learner persistence rejects tampered state and credential-shaped fields", async () => {
  const learner = new AutonomousOnlineLearner();
  learner.update({ arm_id: "offline/model", reward: 0.25, outcome_digest: digestA });
  const snapshot = await snapshotAutonomousOnlineLearner(learner);
  const tampered = structuredClone(snapshot);
  tampered.state.arms[0].api_key = "must-not-persist";
  await assert.rejects(() => validateAutonomousOnlineLearnerSnapshot(tampered), /credential-shaped|digest|unsupported/);

  const forged = structuredClone(snapshot);
  forged.snapshot_generation = 2;
  forged.previous_snapshot_digest = null;
  const { snapshot_digest: _ignored, ...forgedDescriptor } = forged;
  forged.snapshot_digest = await digestJson(forgedDescriptor);
  await assert.rejects(() => validateAutonomousOnlineLearnerSnapshot(forged), /generation and previous_snapshot_digest/);

  const legacy = structuredClone(snapshot);
  legacy.schema = "bioprism-typescript-autonomous-online-learner-snapshot/0.1";
  delete legacy.snapshot_generation;
  delete legacy.previous_snapshot_digest;
  const { snapshot_digest: _legacyIgnored, ...legacyDescriptor } = legacy;
  legacy.snapshot_digest = await digestJson(legacyDescriptor);
  const legacyLearner = new AutonomousOnlineLearner();
  const legacyCoordinator = new AutonomousOnlineLearnerPersistenceCoordinator(legacyLearner, {
    read: () => legacy,
    write: () => {},
  });
  assert.equal((await legacyCoordinator.restore()).schema, "bioprism-typescript-autonomous-online-learner-snapshot/0.1");
  const upgraded = await legacyCoordinator.flush();
  assert.equal(upgraded.schema, "bioprism-typescript-autonomous-online-learner-snapshot/0.2");
  assert.equal(upgraded.snapshot_generation, 1);
  assert.equal(upgraded.previous_snapshot_digest, null);
});

test("AutonomousAgent composes restart-safe learner persistence across every built-in domain", async () => {
  const store = transactionalTextStore();
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0.4, seed: 19 } });
  const persistence = new AutonomousOnlineLearnerPersistenceCoordinator(learner, new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(store));
  const agent = new AutonomousAgent(new LLMRuntime(), { learner, learnerPersistence: persistence });

  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    const context = { domain, capability: "reasoning", risk_class: "review_required", task_family: `family-${index}` };
    await agent.recordEvaluatorReward(`provider-${domain}/model-${index}`, 0.5 + index / 100, {
      context,
      contextDigest: digestCanonicalJsonTextSync(JSON.stringify(context)),
      outcomeDigest: await digestJson({ domain, outcome: "bounded-evaluator-observation" }),
    });
  }

  const flushed = await agent.flushOnlineLearning();
  assert.equal(flushed.snapshot_generation, 1);
  assert.equal(flushed.state.generation, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(flushed.state.contextual_states?.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(JSON.stringify(flushed).includes("bounded-evaluator-observation"), false);
  assert.equal(JSON.stringify(flushed).includes("api_key"), false);

  const restartedLearner = new AutonomousOnlineLearner({ policy: { strategy: "ucb1", exploration: 0.4, seed: 19 } });
  const restartedPersistence = new AutonomousOnlineLearnerPersistenceCoordinator(restartedLearner, new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(store));
  const restartedAgent = new AutonomousAgent(new LLMRuntime(), { learner: restartedLearner, learnerPersistence: restartedPersistence });
  const restored = await restartedAgent.restoreOnlineLearning();
  assert.deepEqual(restored, flushed);
  assert.deepEqual(restartedLearner.snapshot(), learner.snapshot());

  const foreignLearner = new AutonomousOnlineLearner();
  const foreignPersistence = new AutonomousOnlineLearnerPersistenceCoordinator(foreignLearner, new TransactionalJsonAutonomousOnlineLearnerSnapshotPersistence(transactionalTextStore()));
  assert.throws(() => new AutonomousAgent(new LLMRuntime(), { learner, learnerPersistence: foreignPersistence }), /bound to the supplied learner/);
});
