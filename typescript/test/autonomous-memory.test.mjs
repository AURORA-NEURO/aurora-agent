import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  createAutonomousBrainFacade,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousMemoryPersistenceCoordinator,
  CredentialStore,
  InMemoryAutonomousEpisodicMemory,
  TransactionalJsonAutonomousMemoryPersistence,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  digestJson,
  taskFacetDigests,
  openaiCompatibleProvider,
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousDecisionCycle,
  validateAutonomousMemorySnapshot,
} from "../dist/index.js";

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

const capabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation"];

function candidate() {
  return {
    provider: "memory-provider",
    model: "memory-model",
    capabilities,
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function transactionalMemoryTextStore() {
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

function memoryAgent(text = "memory answer") {
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      return jsonResponse({ choices: [{ message: { role: "assistant", content: text }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("memory-provider", "https://memory.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, bodies };
}

async function episodeInput(domain, id = `episode-${domain}`) {
  return {
    episode_id: id,
    run_id: id,
    result_kind: "test_run",
    status: "completed",
    task_digest: await digestJson({ task: `${domain} private task` }),
    context: { domain, capability: `${domain}_capability`, risk_class: `${domain}_risk` },
    selected_model: { provider: "provider", model: "model" },
    digests: { route_digest: await digestJson({ route: domain }), outcome_digest: await digestJson({ outcome: id }) },
    route: { route_digest: await digestJson({ route: domain }), source: "deterministic_vocabulary", selected_domains: [domain], primary_domain: domain, confidence: 0.8 },
    tags: ["domain-review"],
    lesson: "Caller-authored metadata lesson: verify evidence before reuse.",
    provenance: { source: "test" },
  };
}

test("episodic memory records value-only episodes, evaluates them, queries every domain, and rejects secrets", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) await memory.recordEpisode(await episodeInput(profile.domain));
  const coding = await memory.retrieve({ domain: "coding", tags: ["domain-review"], limit: 4 });
  assert.equal(coding.length, 1);
  assert.equal(coding[0].context.domain, "coding");
  assert.equal(coding[0].context.task_family, null);
  assert.match(coding[0].context_digest, /^[0-9a-f]{64}$/);
  assert.equal(coding[0].episode_digest.length, 64);
  await memory.recordEvaluation(coding[0].episode_id, { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.9, passed: true, evidence_digest: "a".repeat(64) });
  assert.equal((await memory.stats()).evaluated, 1);
  assert.equal((await memory.verifyIntegrity()).episodes, 12);
  const serialized = JSON.stringify(await memory.snapshot());
  assert.equal(serialized.includes("private task"), false);
  assert.equal(serialized.includes("api_key"), false);
  await assert.rejects(memory.recordEpisode({ ...(await episodeInput("coding", "secret-episode")), prompt: "private prompt" }), /forbidden/);
  await assert.rejects(memory.recordEpisode({ ...(await episodeInput("coding", "secret-episode-2")), provenance: { api_key: "sk-secret-value" } }), /forbidden/);
  await assert.rejects(memory.recordEpisode({ ...(await episodeInput("coding", "secret-episode-3")), lesson: "authorization: top-secret-value" }), /secret material/);
  const codingContextDigest = coding[0].context_digest;
  assert.equal((await memory.retrieve({ context_digest: codingContextDigest })).length, 1);
  assert.equal((await memory.retrieve({ task_family: "unmatched_family" })).length, 0);
  await assert.rejects(memory.recordEpisode({ ...(await episodeInput("coding", "context-mismatch")), context_digest: "0".repeat(64) }), /does not match its context identity/);
});

test("episodic memory retrieves related digest-only task facets without retaining task vocabulary", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const relatedTask = "review release evidence and validate implementation contract";
  const unrelatedTask = "compare imaging modalities and quantify signal reproducibility";
  const relatedFacets = taskFacetDigests(relatedTask);
  assert.ok(relatedFacets.length > 0);
  assert.equal(JSON.stringify(relatedFacets).includes("release"), false);
  const related = await episodeInput("coding", "related-facets");
  related.task_digest = await digestJson({ task: relatedTask });
  related.task_facets = relatedFacets;
  const unrelated = await episodeInput("coding", "unrelated-facets");
  unrelated.task_digest = await digestJson({ task: unrelatedTask });
  unrelated.task_facets = taskFacetDigests(unrelatedTask);
  await memory.recordEpisode(related);
  await memory.recordEpisode(unrelated);
  const recalled = await memory.retrieve({ domain: "coding", task_facets: relatedFacets, limit: 4 });
  assert.deepEqual(recalled.map((episode) => episode.episode_id), ["related-facets"]);
  assert.deepEqual(recalled[0].task_facets, relatedFacets);
  assert.equal(JSON.stringify(recalled).includes(relatedTask), false);
  assert.equal(JSON.stringify(recalled).includes(unrelatedTask), false);
  assert.equal((await memory.verifyIntegrity()).episodes, 2);
});

test("planning recall ranks reviewed plans by evaluator quality and supports fail-closed filters", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const highPlan = await episodeInput("coding", "planning-high");
  highPlan.digests.plan_refinement_digest = "a".repeat(64);
  const lowPlan = await episodeInput("coding", "planning-low");
  lowPlan.digests.plan_refinement_digest = "b".repeat(64);
  const noPlan = await episodeInput("coding", "planning-none");
  await memory.recordEpisode(highPlan);
  await memory.recordEpisode(lowPlan);
  await memory.recordEpisode(noPlan);
  await memory.recordEvaluation("planning-high", { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.95, passed: true });
  await memory.recordEvaluation("planning-low", { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false });
  await memory.recordEvaluation("planning-none", { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.99, passed: true });

  const ranked = await memory.retrieve({ domain: "coding", ranking: "planning", limit: 3 });
  assert.deepEqual(ranked.map((episode) => episode.episode_id), ["planning-high", "planning-low", "planning-none"]);
  assert.deepEqual((await memory.retrieve({ domain: "coding", ranking: "quality", min_reward: 0.9, limit: 4 })).map((episode) => episode.episode_id), ["planning-none", "planning-high"]);
  assert.deepEqual((await memory.retrieve({ domain: "coding", require_plan_refinement: true, limit: 4 })).map((episode) => episode.episode_id), ["planning-high", "planning-low"]);
  assert.throws(() => memory.retrieve({ ranking: "unknown" }), /ranking is unsupported/);
  assert.throws(() => memory.retrieve({ min_reward: 1.1 }), /min_reward/);
});

test("episodic memory snapshots restore integrity and refuse tampering", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  await memory.recordEpisode(await episodeInput("science", "science-memory"));
  await memory.recordEvaluation("science-memory", { evaluator_id: "science-reviewer", evaluator_version: "1", reward: 0.75, passed: true });
  const snapshot = await memory.snapshot();
  let persisted = null;
  const coordinator = new AutonomousMemoryPersistenceCoordinator(memory, { read: () => persisted, write: (next) => { persisted = next; } });
  await coordinator.flush();
  const restored = new InMemoryAutonomousEpisodicMemory();
  await new AutonomousMemoryPersistenceCoordinator(restored, { read: () => persisted, write: () => {} }).restore();
  assert.equal((await restored.verifyIntegrity()).evaluated, 1);
  assert.equal(restored.get("science-memory").evaluation.reward, 0.75);
  const tampered = structuredClone(snapshot);
  tampered.episodes[0].tags = ["tampered"];
  await assert.rejects(restored.restore(tampered), /snapshot digest mismatch/);
});

test("episodic memory JSON persistence is canonical, restart-safe, serialized, and CAS-fenced", async () => {
  const textStore = transactionalMemoryTextStore();
  const persistence = new TransactionalJsonAutonomousMemoryPersistence(textStore);
  const source = new InMemoryAutonomousEpisodicMemory({ clock: () => 10 });
  const coordinator = new AutonomousMemoryPersistenceCoordinator(source, persistence);
  await source.recordEpisode(await episodeInput("science", "durable-memory-1"));
  const first = await coordinator.flush();
  assert.equal(textStore.encoded(), JSON.stringify(JSON.parse(textStore.encoded())));
  assert.deepEqual(await validateAutonomousMemorySnapshot(JSON.parse(textStore.encoded())), first);

  const restartedStore = new InMemoryAutonomousEpisodicMemory({ clock: () => 11 });
  const restarted = new AutonomousMemoryPersistenceCoordinator(restartedStore, persistence);
  assert.deepEqual(await restarted.restore(), first);
  assert.equal(restartedStore.get("durable-memory-1").context.domain, "science");

  const staleStore = new InMemoryAutonomousEpisodicMemory({ clock: () => 12 });
  const stale = new AutonomousMemoryPersistenceCoordinator(staleStore, persistence);
  await stale.restore();
  await source.recordEpisode(await episodeInput("operations", "durable-memory-2"));
  await coordinator.flush();
  await staleStore.recordEpisode(await episodeInput("coding", "durable-memory-3"));
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const canonical = textStore.encoded();
  textStore.write(JSON.stringify(JSON.parse(canonical), null, 2));
  await assert.rejects(() => persistence.read(), /not canonical/);
  textStore.write(canonical);
});

test("AutonomousAgent and the brain facade compose restart-safe memory across every built-in domain", async () => {
  const textStore = transactionalMemoryTextStore();
  const memory = new InMemoryAutonomousEpisodicMemory({ clock: () => 20 });
  const persistence = new AutonomousMemoryPersistenceCoordinator(memory, new TransactionalJsonAutonomousMemoryPersistence(textStore));
  const agent = new AutonomousAgent(new LLMRuntime(), { memoryStore: memory, memoryPersistence: persistence });
  const brain = createAutonomousBrainFacade({ agent });
  const profiles = await builtinAutonomousDomainProfiles();

  for (const [index, profile] of profiles.entries()) {
    await memory.recordEpisode(await episodeInput(profile.domain, `agent-memory-${index}`));
  }

  const flushed = await brain.flushMemory();
  assert.equal(flushed.episodes.length, profiles.length);
  assert.equal(flushed.events.length, profiles.length);
  assert.equal((await memory.verifyIntegrity()).episodes, profiles.length);
  assert.equal(JSON.stringify(flushed).includes("private task"), false);
  assert.equal(JSON.stringify(flushed).includes("api_key"), false);

  const restartedMemory = new InMemoryAutonomousEpisodicMemory({ clock: () => 21 });
  const restartedPersistence = new AutonomousMemoryPersistenceCoordinator(restartedMemory, new TransactionalJsonAutonomousMemoryPersistence(textStore));
  const restartedAgent = new AutonomousAgent(new LLMRuntime(), { memoryStore: restartedMemory, memoryPersistence: restartedPersistence });
  const restored = await createAutonomousBrainFacade({ agent: restartedAgent }).restoreMemory();
  assert.deepEqual(restored, flushed);
  assert.equal((await restartedMemory.verifyIntegrity()).episodes, profiles.length);
  assert.deepEqual(await restartedMemory.retrieve({ domain: "biomedical" }), [restartedMemory.get("agent-memory-4")]);

  const foreignMemory = new InMemoryAutonomousEpisodicMemory();
  const foreignPersistence = new AutonomousMemoryPersistenceCoordinator(foreignMemory, new TransactionalJsonAutonomousMemoryPersistence(transactionalMemoryTextStore()));
  assert.throws(() => new AutonomousAgent(new LLMRuntime(), { memoryStore: memory, memoryPersistence: foreignPersistence }), /bound to the supplied memoryStore/);
  await assert.rejects(() => new AutonomousAgent(new LLMRuntime()).flushMemory(), /no episodic memory store/);
});

test("decision cycles recall memory into the next prompt and persist only digest metadata", async () => {
  const { agent, bodies } = memoryAgent();
  const memory = new InMemoryAutonomousEpisodicMemory();
  const learning = new AutonomousLearningController(agent);
  const task = "Debug this coding repository and verify the tests.";
  const first = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    approveProviderCall: true,
    memory: { store: memory, episodeId: "memory-cycle-1", tags: ["coding"] },
    learning: {
      controller: learning,
      episodeId: "learning-cycle-1",
      evaluate: () => ({ evaluator_id: "coding-reviewer", evaluator_version: "1", reward: 0.9, passed: true }),
    },
  });
  assert.deepEqual(first.memory.recorded_episode_ids, ["memory-cycle-1"]);
  assert.deepEqual(first.memory.evaluation_recorded_episode_ids, ["memory-cycle-1"]);
  assert.equal(JSON.stringify(memory.get("memory-cycle-1")).includes(task), false);
  assert.deepEqual(memory.get("memory-cycle-1").task_facets, taskFacetDigests(task));
  const unrelated = await episodeInput("coding", "memory-cycle-unrelated");
  unrelated.task_digest = await digestJson({ task: "compare imaging modalities and quantify reproducibility" });
  unrelated.task_facets = taskFacetDigests("compare imaging modalities and quantify reproducibility");
  await memory.recordEpisode(unrelated);

  const second = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    approveProviderCall: true,
    memory: { store: memory, episodeId: "memory-cycle-2", tags: ["coding"] },
  });
  assert.deepEqual(second.memory.recalled_episode_ids, ["memory-cycle-1"]);
  assert.ok(bodies[1].messages.some((message) => String(message.content).includes("autonomous-memory")));
  assert.equal(JSON.stringify(second.memory).includes(task), false);
});

test("cross-domain cycles recall and persist specialist metadata without raw synthesis payloads", async () => {
  const { agent } = memoryAgent("cross answer");
  const memory = new InMemoryAutonomousEpisodicMemory();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "biomedical neuroscience", {
    approveProviderCall: true,
    synthesize: false,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "bio specialist" },
      { id: "neuro", domain: "neuroscience", task: "neuro specialist" },
    ],
    memory: { store: memory, episodePrefix: "memory-cross", tags: ["cross-domain"] },
    learning: {
      controller: learning,
      trajectoryId: "memory-cross-trajectory",
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((id) => [id, { evaluator_id: "cross-reviewer", evaluator_version: "1", reward: 0.8, passed: true }])),
    },
  });
  assert.equal(result.status, "children_completed");
  assert.equal(result.memory.recorded_episode_ids.length, 2);
  assert.equal(result.memory.evaluation_recorded_episode_ids.length, 2);
  assert.equal((await memory.retrieve({ domain: "biomedical" })).length, 1);
  assert.equal(JSON.stringify(await memory.snapshot()).includes("bio specialist"), false);
});
