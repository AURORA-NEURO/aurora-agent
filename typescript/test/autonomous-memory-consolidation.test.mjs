import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousMemoryConsolidationError,
  AutonomousMemoryConsolidationPersistenceCoordinator,
  AutonomousMemoryConsolidator,
  JsonAutonomousMemoryConsolidationLessonTextStore,
  LLMRuntime,
  TransactionalJsonAutonomousMemoryConsolidationPersistence,
  createAutonomousMemoryConsolidationLessonResolver,
  validateAutonomousMemoryConsolidationReport,
  validateAutonomousMemoryConsolidationSnapshot,
} from "../dist/index.js";

const digest = (value) => {
  let hash = 0x811c9dc5;
  for (const character of value) hash = Math.imul(hash ^ character.charCodeAt(0), 0x01000193);
  return `${Math.abs(hash).toString(16).padStart(8, "0")}${"0".repeat(56)}`;
};

function observation({
  episodeId,
  domain,
  conceptId = "portable-review",
  variantId = "bounded-v1",
  lessonId = "lesson-bounded-review",
  reward = 1,
  passed = true,
  transferable = true,
  observedAt = 100,
} = {}) {
  return {
    episode_id: episodeId,
    lesson_id: lessonId,
    concept_id: conceptId,
    variant_id: variantId,
    domain,
    capability: "evidence_review",
    risk_class: "read_only",
    evaluator_id: `evaluator-${episodeId}`,
    evaluator_version: "v1",
    reward,
    passed,
    evidence_digest: digest(`evidence-${episodeId}`),
    lesson_digest: digest(`${lessonId}-${variantId}`),
    decision_digest: digest(`decision-${episodeId}`),
    observed_at: observedAt,
    transferable,
  };
}

class CasStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expected, value) {
    const observed = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (observed !== expected) return false;
    this.value = value;
    return true;
  }
}

test("support, transfer, conflict, and prompt scope cover every built-in domain", () => {
  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 3, minSupportLowerBound: 0.4, conflictDominance: 0.75, clock: () => 100 });
  const observations = AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => observation({ episodeId: `portable-${index}`, domain }));
  for (let index = 0; index < 3; index += 1) observations.push(observation({ episodeId: `conflict-a-${index}`, domain: "evaluation", conceptId: "conflicting-review", variantId: "variant-a", lessonId: "lesson-conflicting-review", reward: 1 }));
  for (let index = 0; index < 3; index += 1) observations.push(observation({ episodeId: `conflict-b-${index}`, domain: "evaluation", conceptId: "conflicting-review", variantId: "variant-b", lessonId: "lesson-conflicting-review", reward: 0 }));

  const report = consolidator.consolidate(observations);
  assert.equal(report.observation_count, observations.length);
  assert.equal(report.deduplicated_observation_count, observations.length);
  assert.deepEqual(report.domains.map((row) => row.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.deepEqual(report.domains.slice(0, -1).map((row) => row.lesson_count), Array(AUTONOMOUS_DOMAIN_NAMES.length - 1).fill(1));
  assert.equal(report.domains.at(-1).lesson_count, 3);

  const portable = consolidator.recall({ domain: "biomedical", capability: "evidence_review" });
  assert.equal(portable.length, 1);
  assert.equal(portable[0].status, "stable");
  assert.equal(portable[0].scope, "cross_domain");
  const references = consolidator.promptReferences({
    domain: "biomedical",
    capability: "evidence_review",
    lessonResolver: (lessonDigest) => lessonDigest === portable[0].lesson_digest ? "Keep evaluator-backed evidence bounded." : null,
  });
  assert.equal(references[0].source, "evaluator_gated_memory_consolidation");
  assert.equal(JSON.stringify(report).includes("Keep evaluator-backed"), false);
  assert.equal(report.conflicts.length, 1);
  assert.deepEqual(report.conflicts[0].variant_ids, ["variant-a", "variant-b"]);
  assert.equal(report.lessons.filter((row) => row.concept_id === "conflicting-review").every((row) => row.status === "conflicted"), true);
});

test("duplicate replay is idempotent and contradictory evaluator identity is rejected", () => {
  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  const row = observation({ episodeId: "replay-1", domain: "coding" });
  const report = consolidator.consolidate([row, { ...row }]);
  assert.equal(report.observation_count, 2);
  assert.equal(report.deduplicated_observation_count, 1);
  const missingOptional = { ...row };
  delete missingOptional.decision_digest;
  assert.equal(new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 }).consolidate([missingOptional]).deduplicated_observation_count, 1);
  assert.throws(() => consolidator.consolidate([row, { ...row, reward: 0 }]), AutonomousMemoryConsolidationError);
});

test("snapshot validation, rehydration, and CAS fencing reject tampering and stale writers", () => {
  const source = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  source.consolidate([observation({ episodeId: "persist-1", domain: "operations" })]);
  const store = new CasStore();
  const persistence = new TransactionalJsonAutonomousMemoryConsolidationPersistence(store);
  const coordinator = new AutonomousMemoryConsolidationPersistenceCoordinator(source, persistence);
  const snapshot = coordinator.flush();
  assert.equal(validateAutonomousMemoryConsolidationSnapshot(snapshot).snapshot_digest, snapshot.snapshot_digest);
  assert.equal(validateAutonomousMemoryConsolidationReport(snapshot.report).report_digest, snapshot.report.report_digest);

  const restored = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  const restoredCoordinator = new AutonomousMemoryConsolidationPersistenceCoordinator(restored, persistence);
  assert.equal(restoredCoordinator.restore().snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restored.recall({ domain: "operations" })[0].lesson_id, "lesson-bounded-review");

  const tampered = structuredClone(snapshot);
  tampered.report.lessons[0].lesson_id = "tampered-lesson";
  assert.throws(() => validateAutonomousMemoryConsolidationSnapshot(tampered), AutonomousMemoryConsolidationError);

  persistence.write(source.snapshot());
  assert.throws(() => restoredCoordinator.flush(), AutonomousMemoryConsolidationError);
});

test("the high-level agent exposes the same consolidation boundary", () => {
  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } }), { memoryConsolidator: consolidator });
  const report = agent.consolidateMemory([observation({ episodeId: "agent-1", domain: "science" })]);
  const references = agent.memoryReferences({ domain: "science", lessonResolver: () => "Use a reproducible evidence trail." });
  assert.equal(report.domains[3].domain, "science");
  assert.equal(references[0].lesson_id, "lesson-bounded-review");
});

test("the lesson text adapter is bounded, canonical, and separate from the consolidation snapshot", () => {
  const lessonDigest = digest("lesson-bounded-review-bounded-v1");
  const rawStore = new CasStore();
  const textStore = new JsonAutonomousMemoryConsolidationLessonTextStore(rawStore);
  const lessonText = "Keep evaluator-backed lesson text outside the digest-only consolidation snapshot.";
  textStore.write(lessonDigest, lessonText);
  assert.equal(textStore.read(lessonDigest), lessonText);

  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  const report = consolidator.consolidate([observation({ episodeId: "text-store-1", domain: "coding" })]);
  assert.equal(JSON.stringify(report).includes(lessonText), false);
  const contexts = [];
  const resolver = createAutonomousMemoryConsolidationLessonResolver(textStore, {
    authorize: (context) => {
      contexts.push(context);
      return context.domains.includes(context.requested_domain);
    },
  });
  const references = consolidator.promptReferences({ domain: "coding", capability: "evidence_review", lessonContextResolver: resolver });
  assert.equal(references[0].text, lessonText);
  assert.equal(contexts[0].lesson_digest, lessonDigest);
  assert.equal(contexts[0].requested_domain, "coding");

  const tampered = JSON.parse(rawStore.value);
  tampered.entries[0].text = "tampered";
  rawStore.value = JSON.stringify(tampered);
  assert.throws(() => textStore.read(lessonDigest), AutonomousMemoryConsolidationError);
  assert.throws(() => textStore.write(lessonDigest, `gsk_${"a".repeat(32)}`), AutonomousMemoryConsolidationError);
});

test("local lessons do not transfer and stale status is explicit", () => {
  const local = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  const report = local.consolidate([
    observation({ episodeId: "local-coding", domain: "coding", transferable: false }),
    observation({ episodeId: "local-browser", domain: "browser", transferable: false }),
  ]);
  assert.equal(local.recall({ domain: "coding" }).length, 1);
  assert.equal(local.recall({ domain: "browser" }).length, 1);
  const stale = new AutonomousMemoryConsolidator({ minObservations: 3, minSupportLowerBound: 0, maxAgeSeconds: 10, clock: () => 100 });
  const staleReport = stale.consolidate([observation({ episodeId: "old", domain: "coding", observedAt: 80 })]);
  assert.equal(staleReport.lessons[0].status, "stale");
  assert.equal(report.domains[0].portable_count, 0);
});

test("high-level approval plans recall stable lessons across every domain without retaining text", async () => {
  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  consolidator.consolidate(AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => observation({ episodeId: `integrated-${index}`, domain })));
  const lessonText = "Use current evaluator-backed evidence and state uncertainty before acting.";
  const lessonDigest = digest("lesson-bounded-review-bounded-v1");
  const lessonStore = new JsonAutonomousMemoryConsolidationLessonTextStore(new CasStore());
  lessonStore.write(lessonDigest, lessonText);
  const contexts = [];
  const lessonContextResolver = createAutonomousMemoryConsolidationLessonResolver(lessonStore, {
    authorize: (context) => {
      contexts.push(context);
      return context.domains.includes(context.requested_domain);
    },
  });
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }), { memoryConsolidator: consolidator });
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await agent.run(`prepare a bounded ${domain} review`, {
      domain,
      memoryLessonContextResolver: lessonContextResolver,
      consolidatedMemoryRequired: true,
      approveProviderCall: false,
    });
    assert.equal(result.status, "approval_required", domain);
    assert.ok(result.blueprint);
    assert.ok(result.blueprint.prompt.messages.some((message) => String(message.content).includes(lessonText)), domain);
    assert.equal(result.memory.consolidated_lesson_ids.length, 1, domain);
    assert.equal(result.memory.consolidated_lesson_digests.length, 1, domain);
    assert.ok(result.memory.consolidated_retrieval_digest, domain);
    assert.ok(result.blueprint.selection_context.consolidated_memory_retrieval_digest, domain);
    assert.equal(JSON.stringify(result.memory).includes(lessonText), false, domain);
    assert.equal(contexts.at(-1).requested_domain, domain);
    assert.equal(contexts.at(-1).lesson_digest, lessonDigest);
  }
});

test("required consolidated recall fails closed when the resolver is unavailable", async () => {
  const consolidator = new AutonomousMemoryConsolidator({ minObservations: 1, minSupportLowerBound: 0, clock: () => 100 });
  consolidator.consolidate([observation({ episodeId: "required-lesson", domain: "coding" })]);
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } }), { memoryConsolidator: consolidator });
  await assert.rejects(
    agent.run("prepare a bounded coding review", { domain: "coding", consolidatedMemoryRequired: true }),
    /one lesson resolver/,
  );
});
