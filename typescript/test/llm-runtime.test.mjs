import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CredentialError,
  CredentialProvisioner,
  CredentialStore,
  AutonomousAgent,
  AutonomousCostBudget,
  AutonomousCostBudgetError,
  AutonomousRuntime,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  TransactionalJsonLLMRuntimeHealthSnapshotPersistence,
  LLMRuntime,
  LLMRuntimeHealthPersistenceCoordinator,
  ProviderRuntimeError,
  anthropicProvider,
  openaiCompatibleProvider,
  openaiProvider,
  providerModelsToCandidates,
  providerTextPart,
  providerImageUrlPart,
  providerImageBase64Part,
  validateLLMRuntimeHealthSnapshot,
} from "../dist/index.js";

function jsonResponse(payload, status = 200, headers = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function requestRecord(url, init) {
  return { url: String(url), method: init?.method, headers: new Headers(init?.headers), body: init?.body === undefined ? undefined : JSON.parse(String(init.body)) };
}

function request(model = "test-model", overrides = {}) {
  return {
    model,
    messages: [{ role: "user", content: "Return a bounded answer." }],
    maxOutputTokens: 128,
    ...overrides,
  };
}

function transactionalRuntimeHealthTextStore() {
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

test("bounded multimodal content translates across provider protocols without leaking into metadata", async () => {
  const messages = [
    { role: "system", content: "Use the evidence contract." },
    {
      role: "user",
      content: [
        providerTextPart("Inspect this image."),
        providerImageUrlPart("https://evidence.example/image.png", "high"),
        providerImageBase64Part("iVBORw0KGgo=", "image/png"),
      ],
    },
  ];

  const openaiCalls = [];
  const openaiRuntime = new LLMRuntime({
    fetch: async (_url, init) => {
      openaiCalls.push(requestRecord(_url, init));
      return jsonResponse({ output_text: "openai" });
    },
  });
  openaiRuntime.registerProvider(openaiProvider({ baseUrl: "https://vision.test", requiresCredential: false }));
  await openaiRuntime.invoke("openai", request("vision-model", { messages }));
  assert.equal(openaiCalls[0].body.input[1].content[0].type, "input_text");
  assert.equal(openaiCalls[0].body.input[1].content[1].type, "input_image");
  assert.equal(openaiCalls[0].body.input[1].content[1].detail, "high");
  assert.match(openaiCalls[0].body.input[1].content[2].image_url, /^data:image\/png;base64,/);
  await openaiRuntime.invoke("openai", request("vision-model", {
    messages: [{
      role: "assistant",
      content: [providerImageUrlPart("https://evidence.example/follow-up.png")],
      toolCalls: [{ id: "call-vision", name: "inspect", arguments: { image: true } }],
    }],
  }));
  assert.equal(openaiCalls[1].body.input[0].content[0].type, "input_image");

  const chatCalls = [];
  const chatRuntime = new LLMRuntime({
    fetch: async (_url, init) => {
      chatCalls.push(requestRecord(_url, init));
      return jsonResponse({ choices: [{ message: { content: "chat" }, finish_reason: "stop" }] });
    },
  });
  chatRuntime.registerProvider(openaiCompatibleProvider("gateway", "https://vision.test", { requiresCredential: false }));
  await chatRuntime.invoke("gateway", request("vision-model", { messages }));
  assert.equal(chatCalls[0].body.messages[1].content[1].type, "image_url");
  assert.equal(chatCalls[0].body.messages[1].content[1].image_url.detail, "high");

  const anthropicCalls = [];
  const anthropicRuntime = new LLMRuntime({
    fetch: async (_url, init) => {
      anthropicCalls.push(requestRecord(_url, init));
      return jsonResponse({ content: [{ type: "text", text: "anthropic" }], stop_reason: "end_turn" });
    },
  });
  anthropicRuntime.registerProvider(anthropicProvider({ baseUrl: "https://vision.test", requiresCredential: false }));
  await anthropicRuntime.invoke("anthropic", request("vision-model", { messages }));
  assert.equal(anthropicCalls[0].body.system, "Use the evidence contract.");
  assert.equal(anthropicCalls[0].body.messages[0].content[1].type, "image");
  assert.equal(anthropicCalls[0].body.messages[0].content[2].source.type, "base64");

  assert.equal(JSON.stringify(openaiRuntime.providerStatus("openai")).includes("evidence.example"), false);
});

test("multimodal content refuses insecure URLs, malformed base64, secret-shaped fields, and non-text policy messages", async () => {
  assert.throws(() => providerImageUrlPart("http://insecure.example/image.png"), ProviderRuntimeError);
  assert.throws(() => providerImageBase64Part("not-base64", "image/png"), ProviderRuntimeError);
  const runtime = new LLMRuntime({ fetch: async () => jsonResponse({ output_text: "must not dispatch" }) });
  runtime.registerProvider(openaiProvider({ baseUrl: "https://vision.test", requiresCredential: false }));
  await assert.rejects(
    runtime.invoke("openai", request("vision-model", { messages: [{ role: "system", content: [providerImageUrlPart("https://evidence.example/image.png")] }] })),
    ProviderRuntimeError,
  );
  await assert.rejects(
    runtime.invoke("openai", request("vision-model", { messages: [{ role: "user", content: [{ type: "image_url", url: "https://evidence.example/image.png", apiKey: "must-refuse" }] }] })),
    ProviderRuntimeError,
  );
});

test("autonomous facade carries transient multimodal evidence through every domain and cross-domain synthesis", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls.push(requestRecord(_url, init));
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded answer" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("facade-vision", "https://facade-vision.test", { requiresCredential: false }));
  const model = {
    provider: "facade-vision",
    model: "vision-model",
    capabilities: ["reasoning", "code", "science", "data", "web", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 10,
    reliability: 0.95,
  };
  const agent = new AutonomousAgent(runtime);
  const domains = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  for (const [domain, task] of Object.entries(domains)) {
    const url = `https://evidence.example/${domain}.png`;
    const result = await agent.run(task, {
      domain,
      candidates: [model],
      approveProviderCall: true,
      contentParts: [providerTextPart(`Evidence for ${domain}`), providerImageUrlPart(url)],
    });
    assert.equal(result.status, "completed", domain);
    const body = calls.at(-1).body;
    const message = body.messages.find((item) => Array.isArray(item.content));
    assert.ok(message, `${domain} should send a content array`);
    assert.equal(message.content.some((part) => part.type === "image_url" && part.image_url.url === url), true, domain);
    assert.equal(JSON.stringify(result).includes(url), false, `${domain} must not retain transient evidence`);
  }

  const beforeCross = calls.length;
  const crossUrl = "https://evidence.example/cross-domain.png";
  const cross = await agent.runCrossDomain("research a biomedical neuroscience experiment with EEG patient evidence", {
    allowCrossDomain: true,
    candidates: [model],
    approveProviderCall: true,
    contentParts: [providerImageUrlPart(crossUrl)],
  });
  assert.ok(["completed", "children_partial"].includes(cross.status));
  const crossCalls = calls.slice(beforeCross);
  assert.ok(crossCalls.length >= 3, "cross-domain fan-out should include specialists and synthesis");
  for (const call of crossCalls) {
    const message = call.body.messages.find((item) => Array.isArray(item.content));
    assert.ok(message, "cross-domain provider call should carry task content parts");
    assert.equal(message.content.some((part) => part.type === "image_url" && part.image_url.url === crossUrl), true);
  }
  assert.equal(JSON.stringify(cross).includes(crossUrl), false);
  const rejectedCalls = calls.length;
  await assert.rejects(
    agent.run("Reject malformed image evidence before planning.", {
      domain: "multimodal",
      candidates: [model],
      approveProviderCall: true,
      contentParts: [{ type: "image_url", url: "http://insecure.example/image.png", apiKey: "must-refuse" }],
    }),
    ProviderRuntimeError,
  );
  assert.equal(calls.length, rejectedCalls, "invalid façade content must not dispatch");
});

test("BYOK credentials are opaque, provider-scoped, and revocable", async () => {
  const calls = [];
  const credentials = new CredentialStore();
  const runtime = new LLMRuntime({
    credentials,
    fetch: async (url, init) => {
      calls.push(requestRecord(url, init));
      return jsonResponse({ id: "resp-1", model: "test-model", output_text: "hello" });
    },
  });
  runtime.registerProvider(openaiProvider({ baseUrl: "https://provider.test" }));
  const handle = credentials.register("openai", "user-secret-that-must-not-leak");

  assert.doesNotMatch(JSON.stringify(handle), /user-secret/);
  assert.equal(credentials.status("openai").ready, true);
  const response = await runtime.invoke("openai", request(), { credential: handle });
  assert.equal(response.text, "hello");
  assert.equal(calls[0].headers.get("authorization"), "Bearer user-secret-that-must-not-leak");

  credentials.revoke(handle);
  await assert.rejects(
    runtime.invoke("openai", request(), { credential: handle }),
    (error) => error instanceof CredentialError && !error.message.includes("user-secret"),
  );
  assert.equal(calls.length, 1, "revoked credentials must fail before network dispatch");
});

test("non-interactive provisioning resolves deployment sources into a short-lived session", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ output_text: "not called" }),
  });
  runtime.registerProvider(openaiProvider({ baseUrl: "https://provider.test" }));
  const onboarding = runtime.onboarding;
  const provisioner = new CredentialProvisioner(onboarding);
  provisioner.registerEnvironment("openai", { variable: "AURORA_OPENAI_KEY", sourceLabel: "deployment environment" });
  await provisioner.registerResolver("openai", "vault/prod/aurora/openai", async () => "resolver-secret", { sourceLabel: "deployment secret manager" });

  const plan = JSON.stringify(provisioner.plan());
  assert.doesNotMatch(plan, /vault\/prod\/aurora\/openai/);
  assert.match(plan, /external_secret_resolver/);

  const session = onboarding.startSession({ ttlMs: 60_000, sessionId: "request-session" });
  const result = await provisioner.provision(session, { environment: { AURORA_OPENAI_KEY: "environment-secret" } });
  assert.equal(result.ready, true);
  assert.equal(result.receipts[0].status, "provisioned");
  assert.equal(session.status().active, true);
  assert.equal(session.handle("openai").provider, "openai");
  assert.doesNotMatch(JSON.stringify(result), /environment-secret|resolver-secret/);
  assert.equal(onboarding.instructions("openai").secret_material, "never_returned");

  session.close();
  assert.throws(() => session.handle("openai"), CredentialError);
});

test("model discovery projects bounded metadata and feeds explicit selection priors", async () => {
  let captured;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      captured = requestRecord(url, init);
      return jsonResponse({
        data: [{
          id: "qwen/qwen3.6-27b",
          created: 1_725_000_000,
          owned_by: "groq",
          active: true,
          context_window: 131_072,
          max_completion_tokens: 8_192,
          supported_parameters: ["tools", "response_format", "private_internal_field"],
          private_prompt: "must not cross the projection boundary",
        }],
        raw_provider_secret: "must not cross the projection boundary",
      });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("groq", "https://api.groq.test/openai/v1", { modelsPath: "/models" }));
  const handle = runtime.credentials.register("groq", "groq-secret");
  const discovery = await runtime.discoverModels("groq", { credential: handle });

  assert.equal(captured.method, "GET");
  assert.equal(captured.url, "https://api.groq.test/openai/v1/models");
  assert.equal(captured.headers.get("authorization"), "Bearer groq-secret");
  assert.equal(discovery.model_count, 1);
  assert.deepEqual(discovery.models[0], {
    schema: "bioprism-typescript-llm-provider-model-discovery/0.1",
    provider: "groq",
    model: "qwen/qwen3.6-27b",
    active: true,
    created_at: 1_725_000_000,
    owned_by: "groq",
    context_window_tokens: 131_072,
    max_output_tokens: 8_192,
    capabilities: ["structured_output", "tool_use"],
    metadata_only: true,
  });
  assert.doesNotMatch(JSON.stringify(discovery), /private_internal_field|raw_provider_secret|groq-secret/);

  const candidates = providerModelsToCandidates(discovery.models, {
    context_window_tokens: 8_000,
    max_output_tokens: 512,
    quality: 0.8,
    latency_ms: 400,
    cost_per_million_tokens: 25,
    reliability: 0.9,
  });
  assert.deepEqual(candidates[0], {
    provider: "groq",
    model: "qwen/qwen3.6-27b",
    capabilities: ["structured_output", "tool_use"],
    context_window_tokens: 131_072,
    max_output_tokens: 8_192,
    quality: 0.8,
    latency_ms: 400,
    cost_per_million_tokens: 25,
    reliability: 0.9,
    enabled: true,
  });
});

test("model discovery fails closed on malformed rows and missing credentials", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ data: [{ id: "duplicate" }, { id: "duplicate" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("groq", "https://groq.test/openai/v1"));
  await assert.rejects(runtime.discoverModels("groq"), CredentialError);
  const handle = runtime.credentials.register("groq", "groq-secret");
  await assert.rejects(runtime.discoverModels("groq", { credential: handle }), ProviderRuntimeError);
  assert.equal(calls, 1);
});

test("provider failures expose redacted context and observers receive stable failure metadata", async () => {
  let observed;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ error: "unauthorized" }, 401, { "x-request-id": "request-401" }),
  });
  runtime.registerProvider(openaiCompatibleProvider("diagnostic", "https://diagnostic.test", { requiresCredential: false }));
  await assert.rejects(
    runtime.invoke("diagnostic", request("diagnostic-model"), {
      observer: { after: async (_metadata, outcome) => { observed = outcome; } },
    }),
    (error) => error instanceof ProviderRuntimeError
      && error.code === "http_4xx"
      && error.provider === "diagnostic"
      && error.operation === "invoke"
      && error.requestId === "request-401"
      && error.retryable === false,
  );
  assert.equal(observed.failureClass, "http_4xx");
  assert.equal(observed.failureCode, "http_4xx");
  assert.equal(observed.requestId, "request-401");
  assert.equal(observed.retryable, false);
  assert.doesNotMatch(JSON.stringify(observed), /authorization|credential|secret|api[-_]?key|gsk_/i);
});

test("retry-after is bounded and caller aborts never dispatch or open a circuit", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return calls === 1
        ? jsonResponse({ error: "busy" }, 429, { "retry-after": "0", "x-request-id": "retry-1" })
        : jsonResponse({ choices: [{ message: { role: "assistant", content: "recovered" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("retryable", "https://retryable.test", {
    requiresCredential: false,
    maxAttempts: 2,
    retryBackoffMs: 0,
    circuitBreakerFailureThreshold: 1,
  }));
  const recovered = await runtime.invoke("retryable", request());
  assert.equal(recovered.text, "recovered");
  assert.equal(calls, 2);
  assert.equal(runtime.providerStatus("retryable").successes, 1);
  assert.equal(runtime.providerStatus("retryable").circuit, "closed");

  const controller = new AbortController();
  controller.abort();
  let abortedCalls = 0;
  const abortedRuntime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { abortedCalls += 1; return jsonResponse({ output_text: "must not run" }); },
  });
  abortedRuntime.registerProvider(openaiProvider({ baseUrl: "https://aborted.test" }));
  let abortedOutcome;
  await assert.rejects(
    abortedRuntime.invoke("openai", request(), {
      signal: controller.signal,
      credential: abortedRuntime.credentials.register("openai", "opaque-secret"),
      observer: { after: async (_metadata, outcome) => { abortedOutcome = outcome; } },
    }),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted" && error.provider === "openai" && error.operation === "invoke",
  );
  assert.equal(abortedCalls, 0);
  assert.equal(abortedOutcome.failureClass, "aborted");
  assert.equal(abortedOutcome.failureCode, "aborted");
  assert.equal(abortedRuntime.providerStatus("openai").circuit, "closed");
});

test("transport aborts are classified without leaking the thrown error", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { throw new DOMException("aborted", "AbortError"); },
  });
  runtime.registerProvider(openaiCompatibleProvider("aborted-transport", "https://aborted-transport.test", { requiresCredential: false }));
  await assert.rejects(
    runtime.invoke("aborted-transport", request()),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted" && error.retryable === false,
  );
});

test("caller aborts interrupt retry backoff before another provider dispatch", async () => {
  const controller = new AbortController();
  let calls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      controller.abort();
      return jsonResponse({ error: "temporarily unavailable" }, 503);
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("backoff-abort", "https://backoff-abort.test", {
    requiresCredential: false,
    maxAttempts: 2,
    retryBackoffMs: 50,
  }));
  await assert.rejects(
    runtime.invoke("backoff-abort", request(), { signal: controller.signal }),
    (error) => error instanceof ProviderRuntimeError
      && error.code === "aborted"
      && error.provider === "backoff-abort"
      && error.operation === "invoke",
  );
  assert.equal(calls, 1);
});

test("provider deadlines are classified as retryable timeouts", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => await new Promise((_, reject) => {
      init.signal.addEventListener("abort", () => reject(new DOMException("aborted", "AbortError")), { once: true });
    }),
  });
  runtime.registerProvider(openaiCompatibleProvider("timed", "https://timed.test", {
    requiresCredential: false,
    timeoutMs: 1,
    maxAttempts: 1,
  }));
  await assert.rejects(
    runtime.invoke("timed", request()),
    (error) => error instanceof ProviderRuntimeError
      && error.code === "timeout"
      && error.provider === "timed"
      && error.operation === "invoke"
      && error.retryable === true,
  );
});

test("OpenAI Responses parsing preserves structured output and tool calls", async () => {
  let captured;
  const credentials = new CredentialStore();
  const runtime = new LLMRuntime({
    credentials,
    fetch: async (url, init) => {
      captured = requestRecord(url, init);
      return jsonResponse({
        id: "resp-2",
        model: "reasoning-model",
        output: [
          { type: "message", content: [{ type: "output_text", text: "I found a result." }] },
          { type: "function_call", call_id: "call-1", name: "lookup", arguments: JSON.stringify({ query: "aurora" }) },
        ],
        usage: { input_tokens: 9, output_tokens: 7, total_tokens: 16 },
      });
    },
  });
  runtime.registerProvider(openaiProvider({ baseUrl: "https://api.test" }));
  const handle = credentials.register("openai", "opaque-secret");
  const response = await runtime.invoke("openai", request("reasoning-model", {
    tools: [{ name: "lookup", description: "Look up a fact.", parameters: { type: "object", properties: { query: { type: "string" } } } }],
  }), { credential: handle });

  assert.equal(captured.url, "https://api.test/v1/responses");
  assert.equal(captured.body.input[0].content, "Return a bounded answer.");
  assert.equal(response.requestId, "resp-2");
  assert.equal(response.text, "I found a result.");
  assert.deepEqual(response.toolCalls, [{ id: "call-1", name: "lookup", arguments: { query: "aurora" } }]);
  assert.deepEqual(response.usage, { input_tokens: 9, output_tokens: 7, total_tokens: 16 });
});

test("OpenAI-compatible chat providers preserve gateway paths and parse tool calls", async () => {
  let captured;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      captured = requestRecord(url, init);
      return jsonResponse({
        id: "chat-1",
        model: "gateway-model",
        choices: [{
          message: {
            role: "assistant",
            content: "delegating",
            tool_calls: [{ id: "call-2", type: "function", function: { name: "lookup", arguments: '{"query":"gateway"}' } }],
          },
          finish_reason: "tool_calls",
        }],
      });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("gateway", "https://gateway.test/root", { requiresCredential: false }));
  const response = await runtime.invoke("gateway", request("gateway-model", {
    tools: [{ name: "lookup", description: "Look up a fact.", parameters: { type: "object" } }],
  }));

  assert.equal(captured.url, "https://gateway.test/root/v1/chat/completions");
  assert.equal(captured.body.max_tokens, 128);
  assert.equal(response.text, "delegating");
  assert.equal(response.toolCalls[0].name, "lookup");
});

test("Anthropic Messages separates system prompts and uses its credential header", async () => {
  let captured;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      captured = requestRecord(url, init);
      return jsonResponse({
        id: "msg-1",
        model: "claude-test",
        content: [{ type: "text", text: "anthropic answer" }],
        stop_reason: "end_turn",
        usage: { input_tokens: 4, output_tokens: 5 },
      });
    },
  });
  runtime.registerProvider(anthropicProvider({ baseUrl: "https://anthropic.test" }));
  const handle = runtime.credentials.register("anthropic", "anthropic-secret");
  const response = await runtime.invoke("anthropic", request("claude-test", {
    messages: [
      { role: "system", content: "You are precise." },
      { role: "user", content: "Explain the result." },
    ],
  }), { credential: handle });

  assert.equal(captured.url, "https://anthropic.test/v1/messages");
  assert.equal(captured.headers.get("x-api-key"), "anthropic-secret");
  assert.equal(captured.headers.get("anthropic-version"), "2023-06-01");
  assert.equal(captured.body.system, "You are precise.");
  assert.deepEqual(captured.body.messages, [{ role: "user", content: "Explain the result." }]);
  assert.equal(response.text, "anthropic answer");
});

test("stream collection projects SSE deltas and bounded completion", async () => {
  const sse = [
    'data: {"choices":[{"delta":{"content":"hel"}}]}',
    'data: {"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}]}',
    "data: [DONE]",
    "",
  ].join("\n\n");
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(sse, { headers: { "content-type": "text/event-stream" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("stream-gateway", "https://stream.test", { requiresCredential: false }));
  const response = await runtime.collectStream("stream-gateway", request());
  assert.equal(response.text, "hello");
  assert.equal(response.statusCode, 200);
});

test("authorized tool loops append tool results and stop at the final answer", async () => {
  const bodies = [];
  let call = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      bodies.push(requestRecord(url, init).body);
      call += 1;
      if (call === 1) {
        return jsonResponse({
          choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "call-3", type: "function", function: { name: "lookup", arguments: '{"query":"safe"}' } }] }, finish_reason: "tool_calls" }],
        });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "tool result incorporated" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("loop-gateway", "https://loop.test", { requiresCredential: false }));
  const result = await runtime.invokeToolLoop("loop-gateway", request("loop-model", {
    tools: [{ name: "lookup", description: "Look up a fact.", parameters: { type: "object" } }],
  }), {
    authorizeAndExecute: async (toolCalls) => toolCalls.map((toolCall) => ({ callId: toolCall.id, approved: true, content: { value: 42 } })),
  });

  assert.equal(result.status, "completed");
  assert.equal(result.turns, 2);
  assert.equal(result.toolCalls, 1);
  assert.equal(result.finalResponse.text, "tool result incorporated");
  assert.equal(bodies[1].messages.at(-1).role, "tool");
  assert.equal(bodies[1].messages.at(-1).content, '{"value":42}');
});

test("retryable provider errors open a circuit and prevent the next dispatch", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ error: "busy" }, 503);
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("unstable", "https://unstable.test", {
    requiresCredential: false,
    maxAttempts: 1,
    circuitBreakerFailureThreshold: 1,
    circuitBreakerResetMs: 60_000,
  }));

  await assert.rejects(runtime.invoke("unstable", request()), (error) => error instanceof ProviderRuntimeError && error.statusCode === 503);
  await assert.rejects(runtime.invoke("unstable", request()), (error) => error instanceof ProviderRuntimeError && error.circuitOpen);
  assert.equal(calls, 1);
  assert.equal(runtime.providerStatus("unstable").circuit, "open");
  assert.equal(runtime.providerStatus("unstable").failures, 2);
});

test("LLM transport health survives restart without restoring credentials or dispatching an open circuit", async () => {
  let sourceCalls = 0;
  const source = new LLMRuntime({
    credentials: new CredentialStore(),
    clock: () => 1_000,
    fetch: async () => {
      sourceCalls += 1;
      return jsonResponse({ error: "busy" }, 503);
    },
  });
  const config = openaiCompatibleProvider("restart-health", "https://restart-health.test", {
    requiresCredential: false,
    maxAttempts: 1,
    circuitBreakerFailureThreshold: 1,
    circuitBreakerResetMs: 60_000,
  });
  source.registerProvider(config);
  await assert.rejects(source.invoke("restart-health", request("restart-model")), /503/);
  assert.equal(sourceCalls, 1);

  let persisted = null;
  const persistence = { read: () => persisted, write: (snapshot) => { persisted = structuredClone(snapshot); } };
  const snapshot = await new LLMRuntimeHealthPersistenceCoordinator(source, persistence).flush();
  assert.equal(snapshot.providers[0].attempts, 1);
  assert.equal(snapshot.providers[0].failures, 1);
  assert.equal(snapshot.providers[0].consecutive_failures, 1);
  assert.equal(JSON.stringify(snapshot).includes("authorization"), false);

  let restoredCalls = 0;
  const restarted = new LLMRuntime({
    credentials: new CredentialStore(),
    clock: () => 1_000,
    fetch: async () => {
      restoredCalls += 1;
      throw new Error("restored open circuit must not dispatch");
    },
  });
  restarted.registerProvider(config);
  const restored = await new LLMRuntimeHealthPersistenceCoordinator(restarted, persistence).restore();
  assert.equal(restored?.snapshot_digest, snapshot.snapshot_digest);
  assert.equal(restarted.providerStatus("restart-health").circuit, "open");
  assert.equal(restarted.providerStatus("restart-health").attempts, 1);
  await assert.rejects(restarted.invoke("restart-health", request("restart-model")), (error) => error instanceof ProviderRuntimeError && error.circuitOpen);
  assert.equal(restoredCalls, 0);

  const tampered = structuredClone(snapshot);
  tampered.providers[0].attempts = 99;
  tampered.providers[0].successes = 98;
  await assert.rejects(validateLLMRuntimeHealthSnapshot(tampered), /digest mismatch/);
  assert.equal(restarted.providerStatus("restart-health").attempts, 2);
});

test("LLM transport health JSON persistence is canonical, serialized, and CAS-fenced", async () => {
  const config = openaiCompatibleProvider("durable-health", "https://durable-health.test", { requiresCredential: false, maxAttempts: 1 });
  const source = new LLMRuntime({ fetch: async () => jsonResponse({ output_text: "bounded" }) });
  source.registerProvider(config);
  const textStore = transactionalRuntimeHealthTextStore();
  const persistence = new TransactionalJsonLLMRuntimeHealthSnapshotPersistence(textStore);
  const coordinator = new LLMRuntimeHealthPersistenceCoordinator(source, persistence);
  await source.invoke("durable-health", request("model-a"));
  const first = await coordinator.flush();
  assert.equal(textStore.encoded(), JSON.stringify(JSON.parse(textStore.encoded())));

  const restarted = new LLMRuntime({ fetch: async () => jsonResponse({ output_text: "restarted" }) });
  restarted.registerProvider(config);
  const restored = new LLMRuntimeHealthPersistenceCoordinator(restarted, persistence);
  assert.deepEqual(await restored.restore(), first);
  assert.equal(restarted.providerStatus("durable-health").attempts, 1);

  const staleRuntime = new LLMRuntime({ fetch: async () => jsonResponse({ output_text: "stale" }) });
  staleRuntime.registerProvider(config);
  const stale = new LLMRuntimeHealthPersistenceCoordinator(staleRuntime, persistence);
  await stale.restore();
  await source.invoke("durable-health", request("model-b"));
  await coordinator.flush();
  await staleRuntime.invoke("durable-health", request("model-c"));
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const canonical = textStore.encoded();
  textStore.write(JSON.stringify(JSON.parse(canonical), null, 2));
  await assert.rejects(() => persistence.read(), /not canonical/);
  textStore.write(canonical);
});

test("autonomous runtime gates candidates on provider readiness and feeds health back to selection", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => {
      calls.push(String(url));
      return jsonResponse({ output_text: "selected answer" });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("fast", "https://fast.test", { requiresCredential: false }));
  runtime.registerProvider(openaiCompatibleProvider("slow", "https://slow.test", { requiresCredential: false }));
  const candidates = [
    { provider: "fast", model: "fast-1", context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.72, latency_ms: 20, cost_per_million_tokens: 10, reliability: 0.85 },
    { provider: "slow", model: "slow-1", context_window_tokens: 32_000, max_output_tokens: 1_000, quality: 0.94, latency_ms: 800, cost_per_million_tokens: 80, reliability: 0.96 },
  ];
  const plan = { task: "Choose a provider for this bounded task.", domain: "general", capability: "reasoning", candidates, request: request("placeholder") };
  const feedback = [];
  const agent = new AutonomousRuntime(runtime);
  const local = await agent.invoke(plan, { feedback: async (_selection, outcome) => feedback.push(outcome) });
  assert.equal(local.selection.strategy, "deterministic_health_utility");
  assert.equal(local.selection.selected_model.provider, "slow");
  assert.equal(feedback[0].success, true);
  assert.equal(calls[0], "https://slow.test/v1/chat/completions");

  let selectionInput;
  const delegated = new AutonomousRuntime(runtime, {
    selector: async (input) => {
      selectionInput = input;
      return { selected_model: { provider: "fast", model: "fast-1" }, ranking: [], strategy: "caller_selector", abstention_reason: null };
    },
  });
  const selected = await delegated.invoke(plan);
  assert.equal(selected.selection.selected_model.provider, "fast");
  assert.equal(selectionInput.task, plan.task);
  assert.equal(selectionInput.request, undefined, "selectors receive no provider request or prompt transcript");
  assert.equal(calls[1], "https://fast.test/v1/chat/completions");

  const gatedRuntime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => jsonResponse({ output_text: "must not run" }) });
  gatedRuntime.registerProvider(openaiProvider({ baseUrl: "https://gated.test" }));
  const gatedAgent = new AutonomousRuntime(gatedRuntime);
  await assert.rejects(gatedAgent.invoke({ ...plan, candidates: [{ ...candidates[0], provider: "openai", model: "gated-model", requires_credential: true }] }), ProviderRuntimeError);
});

test("autonomous selection applies caller budget, latency, and quality gates before dispatch", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ output_text: "constrained answer" }),
  });
  runtime.registerProvider(openaiCompatibleProvider("fast", "https://fast-constraints.test", { requiresCredential: false }));
  runtime.registerProvider(openaiCompatibleProvider("slow", "https://slow-constraints.test", { requiresCredential: false }));
  const agent = new AutonomousRuntime(runtime);
  const plan = {
    task: "Choose within the caller's explicit operating envelope.",
    domain: "operations",
    capability: "reasoning",
    requiredCapabilities: [],
    maxCostPerMillionTokens: 20,
    maxLatencyMs: 100,
    minQuality: 0.7,
    candidates: [
      { provider: "fast", model: "fast-1", context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.72, latency_ms: 20, cost_per_million_tokens: 10, reliability: 0.85 },
      { provider: "slow", model: "slow-1", context_window_tokens: 32_000, max_output_tokens: 1_000, quality: 0.94, latency_ms: 800, cost_per_million_tokens: 80, reliability: 0.96 },
    ],
    request: request("selection-placeholder"),
  };
  const selected = await agent.select(plan);
  assert.deepEqual(selected.selected_model, { provider: "fast", model: "fast-1" });
  assert.equal(selected.ranking.find((row) => row.provider === "slow").eligible, false);
  assert.deepEqual(selected.ranking.find((row) => row.provider === "slow").reasons, ["model cost exceeds the caller budget", "model latency exceeds the caller bound"]);
  const refused = await agent.select({ ...plan, maxLatencyMs: 10 });
  assert.equal(refused.selected_model, null);
  assert.match(refused.abstention_reason, /latency exceeds the caller bound/);
  await assert.rejects(agent.invoke({ ...plan, maxLatencyMs: 10 }), /autonomous selection abstained/);
});

test("autonomous structured output is opt-in, schema-checked, and capability-gated before dispatch", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls.push(requestRecord(_url, init));
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "structured" }) }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("structured", "https://structured.test", { requiresCredential: false, structuredOutputMode: "json_object" }));
  const agent = new AutonomousAgent(runtime);
  const model = { provider: "structured", model: "structured-1", capabilities: ["reasoning", "code", "structured_output"], context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 10, reliability: 0.95 };
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string", minLength: 1 } }, required: ["answer"] };
  const result = await agent.run("Return a structured coding answer.", { domain: "coding", candidates: [model], approveProviderCall: true, requireJson: true, responseSchema });
  assert.equal(result.status, "completed");
  assert.deepEqual(result.response.structured, { answer: "structured" });
  assert.deepEqual(calls[0].body.response_format, { type: "json_object" });

  await assert.rejects(agent.run("This schema is invalid.", { domain: "coding", candidates: [model], approveProviderCall: true, requireJson: true, responseSchema: { type: "not-a-json-type" } }), /responseSchema\.type is invalid/);
  assert.equal(calls.length, 1, "invalid response schemas must fail before dispatch");

  const missingCapability = { ...model, capabilities: ["reasoning", "code"] };
  await assert.rejects(agent.run("This must refuse before dispatch.", { domain: "coding", candidates: [missingCapability], approveProviderCall: true, requireJson: true }), (error) => error instanceof ProviderRuntimeError && error.message.includes("structured output capability"));
  assert.equal(calls.length, 1, "missing candidate capability must not dispatch");

  const disabledRuntime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("structured-disabled provider must not be called"); } });
  disabledRuntime.registerProvider(openaiCompatibleProvider("disabled-structured", "https://disabled-structured.test", { requiresCredential: false, structuredOutputMode: "disabled" }));
  const disabledAgent = new AutonomousAgent(disabledRuntime);
  await assert.rejects(disabledAgent.run("This must refuse on provider capability.", { domain: "coding", candidates: [{ ...model, provider: "disabled-structured" }], approveProviderCall: true, requireJson: true }), (error) => error instanceof ProviderRuntimeError && error.message.includes("structured output is disabled"));
});

test("aggregate cost budgets charge failed failover attempts before allowing another dispatch", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => {
      calls.push(String(url));
      if (String(url).startsWith("https://budget-unstable.test")) return jsonResponse({ error: "busy" }, 503);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("budget-unstable", "https://budget-unstable.test", { requiresCredential: false, maxAttempts: 1 }));
  runtime.registerProvider(openaiCompatibleProvider("budget-backup", "https://budget-backup.test", { requiresCredential: false, maxAttempts: 1 }));
  const agent = new AutonomousRuntime(runtime);
  const budget = new AutonomousCostBudget(0.2);
  const plan = {
    task: "Fail over only when the aggregate spend still permits it.",
    candidates: [
      { provider: "budget-unstable", model: "unstable-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1_000, reliability: 0.99 },
      { provider: "budget-backup", model: "backup-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 1_000, reliability: 0.5 },
    ],
    request: request("selection-placeholder"),
  };
  await assert.rejects(agent.invoke(plan, {
    maxProviderFailovers: 1,
    reserveCost: (costUnits) => budget.reserve(costUnits),
  }), (error) => error instanceof AutonomousCostBudgetError && error.maxCostUnits === 0.2);
  assert.equal(calls.length, 1);
  assert.match(calls[0], /budget-unstable/);
  assert.ok(budget.consumedCostUnits > 0);
  assert.equal(budget.snapshot().remaining_cost_units, 0.2 - budget.consumedCostUnits);
});

test("tool-loop aggregate budgets reserve each provider turn, not just the initial selection", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => {
      calls.push(String(url));
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "budget-tool", type: "function", function: { name: "lookup", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("budget-loop", "https://budget-loop.test", { requiresCredential: false }));
  const agent = new AutonomousRuntime(runtime);
  const budget = new AutonomousCostBudget(0.2);
  await assert.rejects(agent.invokeToolLoop({
    task: "Bound every tool-loop turn.",
    candidates: [{ provider: "budget-loop", model: "loop-model", context_window_tokens: 8_000, max_output_tokens: 128, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 1_000, reliability: 0.9 }],
    request: request("selection-placeholder", { tools: [{ name: "lookup", description: "Read a bounded value.", parameters: { type: "object" } }] }),
  }, {
    reserveCost: (costUnits) => budget.reserve(costUnits),
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    toolReadOnly: () => true,
  }), (error) => error instanceof AutonomousCostBudgetError);
  assert.equal(calls.length, 1);
  assert.ok(budget.consumedCostUnits > 0);
});

test("AutonomousAgent refreshes live model metadata with atomic catalogue reconciliation", async () => {
  let discoveryCalls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      assert.equal(init.method, "GET");
      discoveryCalls += 1;
      const data = discoveryCalls === 1
        ? [{ id: "model-a", context_window: 16_000, max_completion_tokens: 1_000, active: true }, { id: "model-b", context_window: 32_000, max_completion_tokens: 2_000, active: true }]
        : discoveryCalls === 4
          ? []
          : [{ id: "model-a", context_window: 64_000, max_completion_tokens: 4_000, active: true }, { id: "model-c", context_window: 32_000, max_completion_tokens: 2_000, active: true }];
      return jsonResponse({ data });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("catalog", "https://catalog.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(runtime);
  const defaults = { context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.8, latency_ms: 500, cost_per_million_tokens: 30, reliability: 0.9 };
  const first = await agent.refreshModels("catalog", defaults);
  assert.equal(first.execution, "not_started;catalogue_registration_only");
  assert.deepEqual(first.registered_model_ids, ["catalog/model-a", "catalog/model-b"]);
  assert.deepEqual(agent.models().map((model) => model.model), ["model-a", "model-b"]);
  await assert.rejects(agent.refreshModels("catalog", defaults), /already registered/);
  assert.deepEqual(agent.models().map((model) => model.model), ["model-a", "model-b"], "a conflicting refresh must not partially register new models");
  const replaced = await agent.refreshModels("catalog", defaults, { replaceExisting: true });
  assert.deepEqual(replaced.replaced_model_ids, ["catalog/model-a"]);
  assert.deepEqual(replaced.registered_model_ids, ["catalog/model-c"]);
  assert.deepEqual(replaced.removed_model_ids, ["catalog/model-b"]);
  assert.deepEqual(agent.models().map((model) => model.model), ["model-a", "model-c"]);
  assert.equal(agent.models().find((model) => model.model === "model-a").context_window_tokens, 64_000);
  const emptied = await agent.refreshModels("catalog", defaults, { replaceExisting: true });
  assert.deepEqual(emptied.removed_model_ids, ["catalog/model-a", "catalog/model-c"]);
  assert.deepEqual(agent.models(), []);
  assert.equal(discoveryCalls, 4);
});

test("AutonomousAgent refreshes multiple provider catalogues with bounded partial failure reporting", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      assert.equal(init.method, "GET");
      calls.push(String(url));
      if (String(url).startsWith("https://catalog-bad.test")) return jsonResponse({ error: "provider body must not escape" }, 503);
      return jsonResponse({ data: [{ id: "good-model", context_window: 64_000, max_completion_tokens: 4_000, active: true }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("catalog-good", "https://catalog-good.test", { requiresCredential: false }));
  runtime.registerProvider(openaiCompatibleProvider("catalog-bad", "https://catalog-bad.test", { requiresCredential: false, maxAttempts: 1 }));
  const agent = new AutonomousAgent(runtime);
  const defaults = { context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.8, latency_ms: 500, cost_per_million_tokens: 30, reliability: 0.9 };
  const result = await agent.refreshModelCatalogue([
    { provider: "catalog-good", defaults },
    { provider: "catalog-bad", defaults },
  ], { maxParallel: 2, replaceExisting: true });
  assert.equal(result.status, "partial");
  assert.equal(result.requested_provider_count, 2);
  assert.equal(result.successful_provider_count, 1);
  assert.equal(result.failed_provider_count, 1);
  assert.equal(result.refreshes[0].provider, "catalog-good");
  assert.deepEqual(result.failures, [{ provider: "catalog-bad", error_class: "ProviderRuntimeError", failure_code: "http_5xx", retryable: true }]);
  assert.deepEqual(agent.models().map((model) => `${model.provider}/${model.model}`), ["catalog-good/good-model"]);
  assert.doesNotMatch(JSON.stringify(result), /provider body must not escape/);
  assert.equal(calls.length, 2);
});

test("autonomous runtime performs bounded provider failover and journals the admission", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => {
      calls.push(String(url));
      if (String(url).startsWith("https://unstable.test")) return jsonResponse({ error: "busy" }, 503);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "stable answer" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("unstable", "https://unstable.test", { requiresCredential: false, maxAttempts: 1 }));
  runtime.registerProvider(openaiCompatibleProvider("stable", "https://stable.test", { requiresCredential: false, maxAttempts: 1 }));
  const journal = new InMemoryAutonomousExecutionJournal();
  const execution = await AutonomousExecutionController.create({
    executionId: "autonomous-failover-1",
    domain: "general",
    capability: "reasoning",
    riskClass: "read_only",
    policy: { max_steps: 8, max_provider_calls: 2, max_provider_failovers: 1 },
    journal,
  });
  const agent = new AutonomousRuntime(runtime);
  const result = await agent.invoke({
    task: "Answer with one bounded sentence.",
    domain: "general",
    capability: "reasoning",
    candidates: [
      { provider: "unstable", model: "unstable-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "stable", model: "stable-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.6, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.7 },
    ],
    request: request("selection-placeholder"),
  }, { execution });
  assert.equal(result.selection.selected_model.provider, "stable");
  assert.equal(result.response.text, "stable answer");
  assert.equal(calls.length, 2);
  assert.equal(calls[0], "https://unstable.test/v1/chat/completions");
  assert.equal(calls[1], "https://stable.test/v1/chat/completions");
  assert.equal(execution.state.provider_calls, 2);
  assert.equal(execution.state.provider_failovers, 1);
  assert.equal((await journal.verifyIntegrity()).verified, true);
  await execution.complete();
});

test("autonomous runtime isolates a model timeout and retries a healthy sibling on the same provider", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body.model);
      if (body.model === "slow-model") {
        return await new Promise((_, reject) => init.signal.addEventListener("abort", () => reject(new DOMException("timed out", "AbortError")), { once: true }));
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "sibling recovered" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("shared", "https://shared-models.test", {
    requiresCredential: false,
    timeoutMs: 5,
    maxAttempts: 1,
    circuitBreakerFailureThreshold: 4,
  }));
  const agent = new AutonomousRuntime(runtime);
  const result = await agent.invoke({
    task: "Choose a healthy model without discarding the provider after one model timeout.",
    domain: "general",
    capability: "reasoning",
    candidates: [
      { provider: "shared", model: "slow-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "shared", model: "backup-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.75, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.8 },
    ],
    request: request("selection-placeholder"),
  }, { maxProviderFailovers: 1 });
  assert.deepEqual(calls, ["slow-model", "backup-model"]);
  assert.equal(result.selection.selected_model.provider, "shared");
  assert.equal(result.selection.selected_model.model, "backup-model");
  assert.equal(result.response.text, "sibling recovered");
  assert.equal(runtime.providerStatus("shared").circuit, "closed");
});

test("autonomous failover stays fail-closed for non-retryable failures and exhausted budgets", async () => {
  const nonRetryableCalls = [];
  const nonRetryableRuntime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => { nonRetryableCalls.push(String(url)); return jsonResponse({ error: "unauthorized" }, 401); },
  });
  nonRetryableRuntime.registerProvider(openaiCompatibleProvider("denied", "https://denied.test", { requiresCredential: false, maxAttempts: 1 }));
  nonRetryableRuntime.registerProvider(openaiCompatibleProvider("backup", "https://backup.test", { requiresCredential: false, maxAttempts: 1 }));
  const plan = {
    task: "Do not retry an authorization refusal.",
    candidates: [
      { provider: "denied", model: "denied-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "backup", model: "backup-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.5 },
    ],
    request: request("selection-placeholder"),
  };
  const nonRetryableAgent = new AutonomousRuntime(nonRetryableRuntime);
  await assert.rejects(nonRetryableAgent.invoke(plan, { maxProviderFailovers: 1 }), (error) => error instanceof ProviderRuntimeError && error.statusCode === 401 && error.retryable === false);
  assert.deepEqual(nonRetryableCalls, ["https://denied.test/v1/chat/completions"]);

  const exhaustedCalls = [];
  const exhaustedRuntime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => { exhaustedCalls.push(String(url)); return jsonResponse({ error: "busy" }, 503); },
  });
  exhaustedRuntime.registerProvider(openaiCompatibleProvider("busy", "https://busy.test", { requiresCredential: false, maxAttempts: 1 }));
  exhaustedRuntime.registerProvider(openaiCompatibleProvider("unused", "https://unused.test", { requiresCredential: false, maxAttempts: 1 }));
  const exhaustedAgent = new AutonomousRuntime(exhaustedRuntime);
  await assert.rejects(exhaustedAgent.invoke({ ...plan, task: "Do not exceed the zero failover budget.", candidates: plan.candidates.map((candidate) => ({ ...candidate, provider: candidate.provider === "denied" ? "busy" : "unused", model: `${candidate.model}-busy` })) }, { maxProviderFailovers: 0 }), (error) => error instanceof ProviderRuntimeError && error.statusCode === 503);
  assert.deepEqual(exhaustedCalls, ["https://busy.test/v1/chat/completions"]);
});

test("autonomous tool loops never replay after a provider has requested a tool", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => {
      calls.push(String(url));
      if (calls.length === 1) return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "call-guarded", type: "function", function: { name: "lookup", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
      if (String(url).startsWith("https://unstable-loop.test")) return jsonResponse({ error: "busy" }, 503);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "unsafe replay" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("unstable-loop", "https://unstable-loop.test", { requiresCredential: false, maxAttempts: 1 }));
  runtime.registerProvider(openaiCompatibleProvider("backup-loop", "https://backup-loop.test", { requiresCredential: false, maxAttempts: 1 }));
  const execution = await AutonomousExecutionController.create({ executionId: "autonomous-tool-failover-1", domain: "coding", capability: "repository_inspection", riskClass: "read_only", policy: { max_steps: 8, max_provider_calls: 2, max_provider_failovers: 1 } });
  const agent = new AutonomousRuntime(runtime);
  await assert.rejects(agent.invokeToolLoop({
    task: "Inspect without replaying a tool request.",
    candidates: [
      { provider: "unstable-loop", model: "unstable-loop-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "backup-loop", model: "backup-loop-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.5 },
    ],
    request: request("selection-placeholder", { tools: [{ name: "lookup", description: "Read a bounded value.", parameters: { type: "object" } }] }),
  }, {
    execution,
    maxProviderFailovers: 1,
    authorizeAndExecute: async (toolCalls) => toolCalls.map((toolCall) => ({ callId: toolCall.id, approved: true, content: { ok: true } })),
    toolReadOnly: () => true,
  }), (error) => error instanceof ProviderRuntimeError && error.statusCode === 503);
  assert.deepEqual(calls, ["https://unstable-loop.test/v1/chat/completions", "https://unstable-loop.test/v1/chat/completions"]);
  assert.equal(execution.state.provider_failovers, 0);
  assert.equal(execution.state.tool_calls, 1);
  assert.equal(execution.state.status, "running");
});
