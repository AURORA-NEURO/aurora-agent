import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  CredentialError,
  LLMRuntime,
  ProviderRuntimeError,
} from "../dist/index.js";

const modelCandidate = (provider, model) => ({
  provider,
  model,
  capabilities: [
    "reasoning", "code", "web", "data", "science", "biomedical", "operations", "enterprise",
    "coordination", "multimodal", "evaluation", "structured_output",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 20,
  cost_per_million_tokens: 0,
  reliability: 0.99,
});

const request = (model = "offline-model", overrides = {}) => ({
  model,
  messages: [{ role: "user", content: "Return a bounded local answer." }],
  maxOutputTokens: 128,
  ...overrides,
});

test("explicit in-memory providers are credentialless, offline, discoverable, and redacted", async () => {
  const seen = [];
  const runtime = new LLMRuntime({
    fetch: async () => { throw new Error("HTTP must not be reached by an in-memory provider"); },
  });
  runtime.registerInMemoryProvider("offline", (input) => {
    seen.push(input);
    return {
      model: input.model,
      output_text: "offline answer",
      request_id: "local-request-1",
      usage: { input_tokens: 4, output_tokens: 3, private_counter: 99 },
      raw_secret: "must never be retained",
    };
  }, {
    discoverModels: () => ({ data: [{ id: "offline-model", context_window_tokens: 16_000, max_output_tokens: 2_048, capabilities: ["reasoning", "structured_output"] }] }),
  });

  assert.deepEqual(runtime.providerMetadata()[0], {
    provider: "offline",
    protocol: "openai_responses",
    transport: "in_memory",
    base_url: "https://in-memory.invalid",
    path: "/v1/responses",
    models_path: "/models",
    requires_credential: false,
    structured_output_mode: "json_schema",
    credential_posture: "caller_supplied_opaque_handle_not_returned",
    secret_material: "never_returned",
  });
  assert.equal(runtime.onboarding.status("offline").ready, true);
  assert.equal(runtime.onboarding.instructions("offline").next_action, "ready");

  const response = await runtime.invoke("offline", request());
  assert.equal(response.text, "offline answer");
  assert.equal(response.schema, "bioprism-typescript-llm-in-memory-provider/0.1");
  assert.equal(response.transport, "caller_owned");
  assert.equal(response.requestId, "local-request-1");
  assert.deepEqual(response.usage, { input_tokens: 4, output_tokens: 3 });
  assert.doesNotMatch(JSON.stringify(response), /raw_secret|private_counter/);
  assert.equal(seen.length, 1);
  assert.equal(runtime.providerStatus("offline").credential_posture, "caller_supplied_in_memory_handle");
  assert.equal(runtime.providerStatus("offline").successes, 1);

  const discovery = await runtime.discoverModels("offline");
  assert.equal(discovery.model_count, 1);
  assert.deepEqual(discovery.models[0].capabilities, ["reasoning", "structured_output"]);
  assert.equal(discovery.models[0].context_window_tokens, 16_000);
  await assert.rejects(runtime.discoverModels("offline", { credential: {} }), CredentialError);
});

test("in-memory responses preserve structured validation, tool authorization, retries, and failure redaction", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (input) => {
    calls += 1;
    if (calls === 1) throw new ProviderRuntimeError("upstream api-key=local-secret", { retryable: true, statusCode: 503 });
    if (input.messages.some((message) => message.role === "tool")) return { output_text: "tool result accepted" };
    if (input.tools?.length) return { tool_calls: [{ call_id: "call-1", name: "read_status", arguments: { scope: "workspace" } }] };
    return { output_text: JSON.stringify({ answer: "structured local answer" }) };
  }, { maxAttempts: 2, retryBackoffMs: 0 });

  const structured = await runtime.invoke("offline", request("offline-model", {
    requireJson: true,
    responseSchema: { type: "object", required: ["answer"], properties: { answer: { type: "string" } }, additionalProperties: false },
  }));
  assert.deepEqual(structured.structured, { answer: "structured local answer" });

  const loop = await runtime.invokeToolLoop("offline", request("offline-model", {
    tools: [{ name: "read_status", description: "Read bounded status", parameters: { type: "object" } }],
  }), {
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { status: "healthy" } })),
  });
  assert.equal(loop.status, "completed");
  assert.equal(loop.toolCalls, 1);
  assert.equal(loop.finalResponse.text, "tool result accepted");
  assert.equal(calls, 4, "one retry plus two tool-loop turns");

  const failing = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  failing.registerInMemoryProvider("failing", () => {
    throw new ProviderRuntimeError("provider secret=do-not-leak", { retryable: true, statusCode: 503 });
  });
  await assert.rejects(
    failing.invoke("failing", request("failing-model")),
    (error) => error instanceof ProviderRuntimeError
      && error.message === "in-memory provider handler failed"
      && error.statusCode === 503
      && error.retryable === true
      && !error.message.includes("do-not-leak"),
  );
});

test("in-memory streaming supports typed handlers and a deterministic response fallback", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("streaming", () => "fallback", {
    stream: (input) => [
      { provider: "streaming", model: input.model, sequence: 0, eventType: "local.text", textDelta: "hello", requestId: null, usage: {}, done: false },
      { provider: "streaming", model: input.model, sequence: 1, eventType: "local.done", textDelta: "", requestId: null, usage: { output_tokens: 1 }, done: true },
    ],
  });
  const events = [];
  for await (const event of runtime.invokeStream("streaming", request())) events.push(event);
  assert.deepEqual(events.map((event) => event.textDelta), ["hello", ""]);
  assert.equal(events.at(-1).done, true);

  const fallback = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  fallback.registerInMemoryProvider("fallback", () => "fallback text");
  const fallbackEvents = [];
  for await (const event of fallback.invokeStream("fallback", request())) fallbackEvents.push(event);
  assert.deepEqual(fallbackEvents.map((event) => event.textDelta), ["fallback text", ""]);
  assert.deepEqual(fallbackEvents.map((event) => event.eventType), ["in_memory.text", "in_memory.done"]);
});

test("the autonomous façade can execute every built-in domain through one local model arm", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (input) => ({ output_text: `local:${input.model}:${input.messages.at(-1)?.content.slice(0, 32)}` }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(modelCandidate("offline", "offline-model"));
  const tasks = {
    coding: "debug and test this repository",
    browser: "compare current web sources",
    data: "validate dataset schema and lineage",
    science: "design a reproducible experiment",
    biomedical: "review treatment evidence with safety boundaries",
    neuroscience: "analyze EEG preprocessing limits",
    operations: "plan a reversible outage rollback",
    enterprise: "map governance ownership and approvals",
    multi_agent: "delegate a bounded specialist task",
    multimodal: "align an image with a transcript",
    cross_domain: "synthesize interdisciplinary evidence",
    evaluation: "replay a benchmark holdout",
  };
  assert.deepEqual(Object.keys(tasks).sort(), [...AUTONOMOUS_DOMAIN_NAMES].sort());
  for (const [domain, task] of Object.entries(tasks)) {
    const result = await agent.run(task, { domain, approveProviderCall: true });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.response.provider, "offline", domain);
    assert.equal(result.selection.selected_model.model, "offline-model", domain);
  }
  assert.equal(runtime.providerStatus("offline").successes, 12);
});
