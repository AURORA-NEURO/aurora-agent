import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousMemoryPersistenceCoordinator,
  CredentialStore,
  InMemoryAutonomousEpisodicMemory,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  digestJson,
  openaiCompatibleProvider,
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousDecisionCycle,
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
