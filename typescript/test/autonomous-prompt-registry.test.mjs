import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousPromptRegistry,
  AutonomousPromptTemplate,
  AutonomousPromptLearningState,
  selectAdaptiveAutonomousPrompts,
  settleAutonomousPromptSelection,
  LLMRuntime,
  builtinAutonomousPromptRegistry,
  builtinAutonomousPromptTemplates,
  JsonAutonomousPromptLearningSnapshotPersistence,
  TransactionalJsonAutonomousPromptLearningSnapshotPersistence,
  AutonomousPromptLearningPersistenceCoordinator,
  createAutonomousLLMEvidenceAdapterRegistration,
  digestJsonSync,
} from "../dist/index.js";

function template(domain, content = "transient prompt", promptId = `prompt-${domain}`) {
  return new AutonomousPromptTemplate({
    promptId,
    version: "1.0.0",
    domain,
    capabilities: ["analysis", "llm_evidence"],
    stages: ["answer"],
    templateDigest: digestJsonSync({ promptId, version: "1.0.0", content }),
    render: () => [{ role: "user", content }],
  });
}

function context(domain) {
  return {
    plan_digest: digestJsonSync({ domain }),
    requirement: { domain, stage_id: "answer", requirement_id: `${domain}:answer:answer` },
    request: { source_id: "prompt-fixture", request_id: `request-${domain}`, metadata: { fixture: "offline" } },
  };
}

class CasTextStore {
  value = null;
  read() { return this.value; }
  write(value) { this.value = value; }
  writeIfUnchanged(expectedSnapshotDigest, value) {
    const observed = this.value === null ? null : JSON.parse(this.value).snapshot_digest;
    if (observed !== expectedSnapshotDigest) return false;
    this.value = value;
    return true;
  }
}

test("prompt registry selects and renders every autonomous domain without projecting messages", async () => {
  const registry = new AutonomousPromptRegistry(AUTONOMOUS_DOMAIN_NAMES.map((domain) => template(domain)));
  const plan = registry.selectFor(AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ domain, stage: "answer", requiredCapabilities: ["analysis"] })));
  assert.equal(plan.rows.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(plan.registryDigest, registry.registryDigest);
  assert.match(plan.planDigest, /^[0-9a-f]{64}$/);

  const rendered = await registry.render(plan, context("science"));
  assert.equal(rendered.messages[0].content, "transient prompt");
  assert.doesNotMatch(JSON.stringify(rendered.metadata), /transient prompt/);
  assert.equal(rendered.metadata.retention, "rendered_messages_transient;digest_only_projection");
});

test("built-in specialist prompt pack covers every domain with domain-specific capabilities", async () => {
  const registry = builtinAutonomousPromptRegistry();
  assert.equal(registry.manifests.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(new Set(registry.manifests.map((manifest) => manifest.domain)), new Set(AUTONOMOUS_DOMAIN_NAMES));
  const plan = registry.selectFor(AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ domain, stage: "answer", requiredCapabilities: ["analysis", `domain:${domain}`] })));
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const rendered = await registry.render(plan, {
      ...context(domain),
      requirement: { domain, stage_id: "answer", requirement_id: `${domain}:answer:answer`, objective: `Produce a useful reviewed result for ${domain}.` },
    });
    assert.match(rendered.messages[0].content, new RegExp(`${domain} specialist`));
    assert.match(rendered.messages[1].content, new RegExp(`Produce a useful reviewed result for ${domain}\\.`));
    assert.doesNotMatch(JSON.stringify(rendered.metadata), /Produce a useful reviewed result/);
  }
});

test("built-in specialist prompt pack rejects duplicates, unsupported domains, and missing objectives", async () => {
  assert.equal(builtinAutonomousPromptTemplates(["science", "evaluation"]).length, 2);
  assert.throws(() => builtinAutonomousPromptTemplates(["science", "science"]), /duplicate/);
  assert.throws(() => builtinAutonomousPromptTemplates(["not-a-domain"]), /unsupported/);
  const registry = builtinAutonomousPromptRegistry(["science"]);
  const plan = registry.selectFor([{ domain: "science", stage: "answer", requiredCapabilities: ["analysis"] }]);
  await assert.rejects(() => registry.render(plan, context("science")), /requires a bounded objective/);
});

test("prompt learning explores registry arms and settles idempotently without retaining prompt text", () => {
  const registry = new AutonomousPromptRegistry([
    template("science", "variant A transient", "prompt-science-a"),
    template("science", "variant B transient", "prompt-science-b"),
  ]);
  const state = new AutonomousPromptLearningState(registry.registryDigest);
  const request = [{ domain: "science", stage: "answer", requiredCapabilities: [] }];
  const first = selectAdaptiveAutonomousPrompts(registry, request, { state });
  assert.match(first.armIds[0], /^[0-9a-f]{64}$/);
  const settled = settleAutonomousPromptSelection(registry, state, first, {
    armId: first.armIds[0],
    evaluatorId: "science-rubric",
    evaluatorVersion: "1",
    reward: 0.9,
    passed: true,
    settlementKey: "a".repeat(64),
  });
  assert.equal(settled.status, "settled");
  assert.equal(settled.nextState.generation, 1);
  const second = selectAdaptiveAutonomousPrompts(registry, request, { state: settled.nextState });
  assert.notEqual(second.armIds[0], first.armIds[0]);
  const replay = settleAutonomousPromptSelection(registry, settled.nextState, first, {
    armId: first.armIds[0],
    evaluatorId: "science-rubric",
    evaluatorVersion: "1",
    reward: 0.9,
    passed: true,
    settlementKey: "a".repeat(64),
  });
  assert.equal(replay.status, "replayed");
  assert.equal(replay.nextState.stateDigest, settled.nextState.stateDigest);
  assert.doesNotMatch(JSON.stringify(settled.nextState.toJSON()), /variant [AB] transient/);
});

test("prompt learning rejects stale registries and untrusted ledger fields", () => {
  const registry = new AutonomousPromptRegistry([template("science", "variant A", "prompt-science-a")]);
  const state = new AutonomousPromptLearningState(registry.registryDigest);
  const selection = selectAdaptiveAutonomousPrompts(registry, [{ domain: "science", stage: "answer", requiredCapabilities: [] }], { state });
  registry.register(template("science", "replacement", "prompt-science-a"), { replace: true });
  assert.throws(() => settleAutonomousPromptSelection(registry, state, selection, {
    armId: selection.armIds[0], evaluatorId: "science-rubric", evaluatorVersion: "1", reward: 0.5, passed: true,
  }), /stale/);
  assert.throws(() => AutonomousPromptLearningState.fromJSON({ ...state.toJSON(), settlements: [{ secret: "must-not-cross" }] }), /fields/);
});

test("prompt learning persistence recovers every domain and settles through CAS", async () => {
  const registry = builtinAutonomousPromptRegistry();
  const store = new CasTextStore();
  const persistence = new TransactionalJsonAutonomousPromptLearningSnapshotPersistence(store);
  const requests = AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({ domain, stage: "answer", requiredCapabilities: [] }));
  const controller = new AutonomousPromptLearningPersistenceCoordinator(registry, { persistence });
  const selection = controller.select(requests);
  assert.equal(selection.armIds.length, AUTONOMOUS_DOMAIN_NAMES.length);
  const settlement = await controller.settle(selection, {
    armId: selection.armIds[0], evaluatorId: "all-domain-rubric", evaluatorVersion: "1", reward: 0.8, passed: true, outcomeDigest: "b".repeat(64),
  });
  assert.equal(settlement.status, "settled");
  assert.equal(controller.state.generation, 1);
  assert.equal(typeof store.value, "string");
  assert.doesNotMatch(store.value, /transient prompt|provider response/);

  const recovered = new AutonomousPromptLearningPersistenceCoordinator(registry, { persistence });
  const snapshot = await recovered.restore();
  assert.equal(snapshot.snapshotGeneration, 1);
  assert.equal(recovered.state.stateDigest, controller.state.stateDigest);
  const replay = await recovered.settle(selection, {
    armId: selection.armIds[0], evaluatorId: "all-domain-rubric", evaluatorVersion: "1", reward: 0.8, passed: true, outcomeDigest: "b".repeat(64),
  });
  assert.equal(replay.status, "replayed");
  assert.equal(recovered.state.generation, 1);
});

test("prompt learning persistence rejects stale writers, registry drift, and tampering", async () => {
  const registry = builtinAutonomousPromptRegistry();
  const store = new CasTextStore();
  const persistence = new TransactionalJsonAutonomousPromptLearningSnapshotPersistence(store);
  const first = new AutonomousPromptLearningPersistenceCoordinator(registry, { persistence });
  await first.flush();
  const stale = new AutonomousPromptLearningPersistenceCoordinator(registry, { persistence });
  await stale.restore();
  const selection = first.select([{ domain: "science", stage: "answer", requiredCapabilities: [] }]);
  await first.settle(selection, { armId: selection.armIds[0], evaluatorId: "rubric", evaluatorVersion: "1", reward: 0.2, passed: true, outcomeDigest: "c".repeat(64) });
  const staleSelection = stale.select([{ domain: "science", stage: "answer", requiredCapabilities: [] }]);
  await assert.rejects(() => stale.settle(staleSelection, { armId: staleSelection.armIds[0], evaluatorId: "rubric", evaluatorVersion: "1", reward: 0.2, passed: true, outcomeDigest: "d".repeat(64) }), /compare-and-swap/);

  const replacement = new AutonomousPromptLearningPersistenceCoordinator(builtinAutonomousPromptRegistry(["coding"]), { persistence: new JsonAutonomousPromptLearningSnapshotPersistence(store) });
  await assert.rejects(() => replacement.restore(), /stale/);
  const payload = JSON.parse(store.value);
  payload.snapshot_digest = "f".repeat(64);
  store.value = JSON.stringify(payload);
  await assert.rejects(() => persistence.read(), /digest|canonical/);
});

test("prompt registry rejects stale plans and credential-shaped prompt fields", async () => {
  const registry = new AutonomousPromptRegistry([template("coding")]);
  const plan = registry.selectFor([{ domain: "coding", stage: "answer", requiredCapabilities: ["analysis"] }]);
  registry.register(template("coding", "replacement prompt"), { replace: true });
  assert.throws(() => registry.verifySelection(plan), /stale/);

  const unsafe = new AutonomousPromptTemplate({
    promptId: "unsafe-prompt",
    version: "1",
    domain: "science",
    capabilities: ["analysis"],
    stages: ["answer"],
    templateDigest: "a".repeat(64),
    render: () => [{ role: "user", content: { api_key: "must-not-cross" } }],
  });
  await assert.rejects(() => unsafe.renderTransient(context("science")), /credential-shaped/);
});

test("LLM evidence adapter can use a verified registry selection and binds the rendered digest", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  const requests = [];
  runtime.registerInMemoryProvider("prompt-fixture", (request) => {
    requests.push(request);
    return { structured: { answer: "ok" } };
  });
  const registry = new AutonomousPromptRegistry([template("science")]);
  const plan = registry.selectFor([{ domain: "science", stage: "answer", requiredCapabilities: ["analysis"] }]);
  const registration = createAutonomousLLMEvidenceAdapterRegistration({
    adapterId: "science-prompt-adapter",
    version: "1",
    domain: "science",
    provider: "prompt-fixture",
    runtime,
    model: "fixture-model",
    capabilities: ["llm_evidence"],
    promptRegistry: registry,
    promptSelection: plan,
    requireJson: true,
  });
  await registration.acquire(context("science"));
  assert.equal(requests[0].messages[0].content, "transient prompt");
  assert.match(requests[0].idempotencyKey, /^[0-9a-f]{64}$/);
});

test("built-in specialist prompt pack drives an offline provider invocation", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  const requests = [];
  runtime.registerInMemoryProvider("builtin-prompt-fixture", (request) => {
    requests.push(request);
    return { structured: { answer: "ok" } };
  });
  const registry = builtinAutonomousPromptRegistry(["biomedical"]);
  const plan = registry.selectFor([{ domain: "biomedical", stage: "answer", requiredCapabilities: ["analysis", "domain:biomedical"] }]);
  const registration = createAutonomousLLMEvidenceAdapterRegistration({
    adapterId: "biomedical-builtin-prompt",
    version: "1",
    domain: "biomedical",
    provider: "builtin-prompt-fixture",
    runtime,
    model: "fixture-model",
    capabilities: ["llm_evidence"],
    promptRegistry: registry,
    promptSelection: plan,
    requireJson: true,
  });
  await registration.acquire({
    ...context("biomedical"),
    requirement: { domain: "biomedical", stage_id: "answer", requirement_id: "biomedical:answer:answer", objective: "Compare bounded evidence without making a clinical recommendation." },
  });
  assert.equal(requests[0].messages[0].role, "system");
  assert.match(requests[0].messages[0].content, /never diagnose/);
});
