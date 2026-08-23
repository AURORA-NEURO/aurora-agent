import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousLearningFeedbackWorker,
  AutonomousLearningFeedbackOutboxPersistenceCoordinator,
  AutonomousOnlineLearner,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  JsonAutonomousLearningFeedbackOutboxPersistence,
  LLMRuntime,
  TransactionalJsonAutonomousLearningFeedbackOutboxPersistence,
  WebStorageAutonomousLearningFeedbackOutboxTextStore,
  validateAutonomousLearningFeedbackOutboxSnapshot,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function agentFor(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: "private feedback worker provider output" };
  });
  const agent = new AutonomousAgent(runtime, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  return agent;
}

function reward(domain) {
  return {
    evaluator_id: `worker-evaluator-${domain}`,
    evaluator_version: "1",
    reward: 0.8,
    passed: true,
    evidence_digest: "2".repeat(64),
  };
}

async function enqueueAll(controller) {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const run = await controller.agent.run(`private worker task for ${domain}`, {
      domain,
      approveProviderCall: true,
      learning: controller,
      learningEpisodeId: `worker-episode-${domain}`,
    });
    assert.equal(run.learning_episode_status, "prepared");
    await controller.enqueueRunSettlement(run.learning_episode_id, reward(domain), { idempotencyKey: `worker-command-${domain}` });
  }
}

test("learning feedback worker drains explicit commands across every domain with bounded rounds", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const learning = new AutonomousLearningController(agent);
  await enqueueAll(learning);
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length);

  const worker = new AutonomousLearningFeedbackWorker(learning);
  const first = await worker.run({ workerId: "feedback-worker-a", limit: 4, maxRounds: 1, maxCommands: 4, now: Date.now() + 1_000 });
  assert.equal(first.status, "bounded");
  assert.equal(first.applied, 4);
  assert.equal(first.remaining, AUTONOMOUS_DOMAIN_NAMES.length - 4);
  assert.doesNotMatch(JSON.stringify(first), /private worker task|private feedback worker provider output/);

  const second = await worker.run({ workerId: "feedback-worker-a", limit: 4, maxRounds: 4, maxCommands: 16, now: Date.now() + 2_000 });
  assert.equal(second.status, "drained");
  assert.equal(second.applied, AUTONOMOUS_DOMAIN_NAMES.length - 4);
  assert.equal(second.remaining, 0);
  assert.equal((await learning.episodes.pending()).length, 0);
  assert.equal(providerCalls, AUTONOMOUS_DOMAIN_NAMES.length, "feedback worker must never replay provider runs");
});

test("learning feedback worker reclaims an expired lease after a simulated crash and stays idempotent", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const learning = new AutonomousLearningController(agent);
  const run = await agent.run("private lease recovery task", {
    domain: "coding",
    approveProviderCall: true,
    learning,
    learningEpisodeId: "worker-lease-episode",
  });
  const command = await learning.enqueueRunSettlement(run.learning_episode_id, reward("coding"), { idempotencyKey: "worker-lease-command" });
  const now = Date.now();
  const claimed = learning.feedbackOutbox.claim(command.command_id, "crashed-worker", 1_000, now);
  assert.ok(claimed);

  const worker = new AutonomousLearningFeedbackWorker(learning);
  const recovered = await worker.run({ workerId: "recovery-worker", limit: 1, now: now + 2_000 });
  assert.equal(recovered.status, "drained");
  assert.equal(recovered.applied, 1);
  assert.equal(providerCalls, 1);
  const replay = await worker.run({ workerId: "recovery-worker", limit: 1, now: now + 3_000 });
  assert.equal(replay.status, "drained");
  assert.equal(replay.inspected, 0);
  assert.equal((await learning.episodes.pending()).length, 0);
});

test("learning feedback worker preserves terminal settlement failures as metadata", async () => {
  const agent = agentFor();
  const learning = new AutonomousLearningController(agent);
  const run = await agent.run("private remote settlement task", {
    domain: "science",
    approveProviderCall: true,
    learning,
    learningEpisodeId: "worker-remote-episode",
  });
  await learning.enqueueRunSettlement(run.learning_episode_id, reward("science"), { idempotencyKey: "worker-remote-command", remote: true });
  const worker = new AutonomousLearningFeedbackWorker(learning);
  const result = await worker.run({ workerId: "remote-worker", limit: 1, now: Date.now() + 1_000 });

  assert.equal(result.status, "failed");
  assert.equal(result.failed, 1);
  assert.equal(result.rows[0].error_class, "ArgumentError");
  assert.equal(result.remaining, 0);
  assert.equal((await learning.episodes.pending()).length, 1);
});

test("durable learning feedback outbox survives restart across every domain without provider replay", async () => {
  let encoded = null;
  const values = new Map();
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };
  const textStore = new WebStorageAutonomousLearningFeedbackOutboxTextStore(storage, "aurora-feedback-outbox");
  const persistence = new TransactionalJsonAutonomousLearningFeedbackOutboxPersistence({
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const observedDigest = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (observedDigest !== expectedDigest) return false;
      encoded = value;
      return true;
    },
  });
  const primaryOutbox = new AutonomousLearningFeedbackOutboxPersistenceCoordinator(new InMemoryAutonomousLearningFeedbackOutboxStore(), persistence);
  assert.equal(await primaryOutbox.restore(), null);
  const agent = agentFor();
  const learning = new AutonomousLearningController(agent, { feedbackOutbox: primaryOutbox });
  await enqueueAll(learning);
  const persisted = await persistence.read();
  assert.equal(persisted.commands.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(encoded, /private worker task|private feedback worker provider output/);
  const browserPersistence = new JsonAutonomousLearningFeedbackOutboxPersistence(textStore);
  await browserPersistence.write(persisted);
  assert.deepEqual(await browserPersistence.read(), persisted);
  const canonicalOutbox = values.get("aurora-feedback-outbox");
  values.set("aurora-feedback-outbox", JSON.stringify(JSON.parse(canonicalOutbox), null, 2));
  await assert.rejects(() => browserPersistence.read(), /not canonical/);
  values.set("aurora-feedback-outbox", canonicalOutbox);

  const staleOutbox = new AutonomousLearningFeedbackOutboxPersistenceCoordinator(new InMemoryAutonomousLearningFeedbackOutboxStore(), persistence);
  await staleOutbox.restore();
  const claimNow = Date.now() + 1_000;
  await primaryOutbox.claim("worker-command-coding", "worker-a", 1_000, claimNow);
  await assert.rejects(() => staleOutbox.claim("worker-command-coding", "worker-b", 1_000, claimNow), /compare-and-swap conflict/);

  const restartedOutbox = new AutonomousLearningFeedbackOutboxPersistenceCoordinator(new InMemoryAutonomousLearningFeedbackOutboxStore(), persistence);
  const restored = await restartedOutbox.restore();
  assert.equal(restored.commands.length, AUTONOMOUS_DOMAIN_NAMES.length);
  const restartedLearning = new AutonomousLearningController(agent, {
    episodes: learning.episodes,
    settlementReceipts: learning.settlementReceipts,
    feedbackOutbox: restartedOutbox,
  });
  const worker = new AutonomousLearningFeedbackWorker(restartedLearning);
  const result = await worker.run({ workerId: "restarted-feedback-worker", limit: AUTONOMOUS_DOMAIN_NAMES.length, maxRounds: 2, maxCommands: AUTONOMOUS_DOMAIN_NAMES.length, now: claimNow + 2_000 });
  assert.equal(result.status, "drained");
  assert.equal(result.applied, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.remaining, 0);
  assert.equal((await learning.episodes.pending()).length, 0);
});

test("feedback outbox snapshots reject unsupported fields and malformed command state", async () => {
  const store = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const snapshot = store.snapshot();
  const unsafe = structuredClone(snapshot);
  unsafe.api_key = "never persisted";
  assert.throws(() => validateAutonomousLearningFeedbackOutboxSnapshot(unsafe), /unsupported fields/);
  const malformed = structuredClone(snapshot);
  malformed.commands.push({ schema: "bad" });
  const { snapshot_digest: _digest, ...body } = malformed;
  malformed.snapshot_digest = "0".repeat(64);
  assert.throws(() => validateAutonomousLearningFeedbackOutboxSnapshot(malformed), /snapshot digest does not match/);
});
