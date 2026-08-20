import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CredentialError,
  CredentialProvisioner,
  CredentialStore,
  AutonomousRuntime,
  LLMRuntime,
  ProviderRuntimeError,
  anthropicProvider,
  openaiCompatibleProvider,
  openaiProvider,
  providerModelsToCandidates,
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
