import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousRuntime,
  CredentialError,
  CredentialStore,
  LLMRuntime,
  ProviderRuntimeError,
  ProviderSetup,
  SUPPORTED_PROVIDER_NAMES,
  providerConfig,
  providerPreset,
  providerPresets,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

test("the provider catalog covers every built-in BYOK transport without secret material", () => {
  const presets = providerPresets();
  assert.deepEqual(presets.map((preset) => preset.provider), [...SUPPORTED_PROVIDER_NAMES]);
  assert.equal(new Set(presets.map((preset) => preset.environment_variable)).size, presets.length);
  for (const preset of presets) {
    const config = providerConfig(preset.provider);
    assert.equal(config.provider, preset.provider);
    assert.equal(config.protocol, preset.protocol);
    assert.equal(config.baseUrl, preset.default_base_url);
    assert.equal(config.path, preset.default_path);
    assert.equal(config.modelsPath, preset.default_models_path);
    assert.doesNotMatch(JSON.stringify(preset), /user-entered-secret|authorization|credential_value/i);
  }
  assert.throws(() => providerPreset("unknown-provider"), CredentialError);
});

test("ProviderSetup discovers live model metadata through the protected session", async () => {
  const requests = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      requests.push({ url: String(url), method: init?.method, headers: new Headers(init?.headers) });
      return jsonResponse({ data: [{ id: "openai/gpt-oss-20b", context_window: 131_072, max_completion_tokens: 8_192, active: true }] });
    },
  });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("groq", { baseUrl: "https://groq.test/openai/v1" });
  const session = setup.startSession({ ttlMs: 60_000, sessionId: "discovery-session" });
  setup.collectUserCredential(session, "groq", "groq-secret");

  const discovery = await setup.discoverModels(session, "groq");
  assert.equal(requests[0].method, "GET");
  assert.equal(requests[0].url, "https://groq.test/openai/v1/models");
  assert.equal(requests[0].headers.get("authorization"), "Bearer groq-secret");
  assert.equal(discovery.models[0].model, "openai/gpt-oss-20b");
  const candidates = setup.modelCandidates(discovery, {
    context_window_tokens: 8_000,
    max_output_tokens: 512,
    quality: 0.9,
    latency_ms: 500,
    cost_per_million_tokens: 30,
    reliability: 0.9,
  });
  assert.equal(candidates[0].context_window_tokens, 131_072);
  assert.doesNotMatch(JSON.stringify(discovery), /groq-secret/);
  session.close();
});

test("ProviderSetup provides the real protected-input-to-session lifecycle", async () => {
  const requests = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      requests.push({ url: String(url), headers: new Headers(init?.headers), body: JSON.parse(String(init?.body)) });
      return jsonResponse({ output_text: "setup flow completed" });
    },
  });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("openai", { baseUrl: "https://setup.test" });

  const before = setup.plan(["openai"]);
  assert.equal(before.ready, false);
  assert.equal(before.next_action, "collect_user_credential");
  assert.equal(before.providers[0].secret_material, "never_returned");

  const session = setup.startSession({ ttlMs: 60_000, sessionId: "ui-request" });
  const handle = setup.collectUserCredential(session, "openai", "user-entered-secret");
  assert.equal(handle.provider, "openai");
  assert.equal(setup.instructions("openai").ready, true);
  assert.doesNotMatch(JSON.stringify(setup.plan(["openai"])), /user-entered-secret/);

  const agent = new AutonomousRuntime(runtime);
  const result = await agent.invoke({
    task: "Answer after the user has completed provider setup.",
    domain: "general",
    capability: "reasoning",
    candidates: [{
      provider: "openai",
      model: "setup-model",
      context_window_tokens: 8_000,
      max_output_tokens: 512,
      quality: 0.9,
      latency_ms: 100,
      cost_per_million_tokens: 10,
      reliability: 0.9,
    }],
    request: {
      model: "placeholder",
      messages: [{ role: "user", content: "Return a bounded answer." }],
      maxOutputTokens: 64,
    },
  }, { credential: session.handle("openai") });
  assert.equal(result.response.text, "setup flow completed");
  assert.equal(requests[0].headers.get("authorization"), "Bearer user-entered-secret");
  assert.equal(session.status().active, true);

  session.close();
  assert.equal(session.status().active, false);
  await assert.rejects(
    agent.invoke({
      task: "A closed setup session must not dispatch.",
      candidates: [{
        provider: "openai",
        model: "setup-model",
        context_window_tokens: 8_000,
        max_output_tokens: 512,
        quality: 0.9,
        latency_ms: 100,
        cost_per_million_tokens: 10,
        reliability: 0.9,
      }],
      request: {
        model: "placeholder",
        messages: [{ role: "user", content: "Do not run." }],
        maxOutputTokens: 64,
      },
    }, { credential: handle }),
    ProviderRuntimeError,
  );
  assert.equal(requests.length, 1);
});
