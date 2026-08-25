import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousPromptRegistry,
  AutonomousPromptTemplate,
  LLMRuntime,
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

