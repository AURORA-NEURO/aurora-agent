import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CredentialError,
  CredentialProvisioner,
  CredentialStore,
  AutonomousAuthorizationContext,
  AutonomousAuthorizationError,
  AutonomousAuthorizationGate,
  AutonomousAuthorizationLedger,
  AutonomousAgent,
  AutonomousCostBudget,
  AutonomousCostBudgetError,
  AutonomousRuntime,
  AutonomousExecutionController,
  advanceAutonomousModelContinuationState,
  AutonomousEffectBoundary,
  AutonomousEffectReconciliationRequiredError,
  InMemoryAutonomousEffectJournal,
  InMemoryAutonomousExecutionJournal,
  compileAutonomousModelContinuationPlan,
  completeAutonomousModelContinuationState,
  createAutonomousModelContinuationState,
  validateAutonomousModelContinuationPlan,
  validateAutonomousModelContinuationState,
  TransactionalJsonLLMRuntimeHealthSnapshotPersistence,
  LLMRuntime,
  LLMRuntimeHealthPersistenceCoordinator,
  digestJson,
  ProviderQuotaController,
  ProviderRuntimeError,
  anthropicProvider,
  ollamaProvider,
  openaiCompatibleProvider,
  openaiProvider,
  providerModelsToCandidates,
  providerTextPart,
  providerImageUrlPart,
  providerImageBase64Part,
  DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS,
  normalizeAutonomousSelectionWeights,
  rankAutonomousModels,
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

function providerAuthorizationContext(maxUses = 2, domains = ["coding"]) {
  const ledger = new AutonomousAuthorizationLedger(16, 64);
  const issued = ledger.issue({
    grant_id: "runtime-grant",
    tenant_id: "tenant-a",
    actor_id: "actor-a",
    session_id: "session-a",
    authorization_digest: "a".repeat(64),
    allowed_domains: domains,
    allowed_operations: ["provider_invocation"],
    allowed_capabilities: [],
    allowed_risk_classes: [],
    issued_at: 1000,
    expires_at: 2000,
    max_uses: maxUses,
  });
  return {
    ledger,
    context: new AutonomousAuthorizationContext(
      new AutonomousAuthorizationGate(ledger),
      issued.grant_id,
      issued.tenant_id,
      issued.actor_id,
      issued.session_id,
      issued.authorization_digest,
      [...domains],
      null,
      "provider_invocation",
      "provider",
      () => 1200,
    ),
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

test("Ollama preset uses the credentialless loopback OpenAI-compatible surface", async () => {
  const calls = [];
  const runtime = new LLMRuntime({
    fetch: async (_url, init) => {
      calls.push(requestRecord(_url, init));
      return jsonResponse({ choices: [{ message: { content: "local model" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(ollamaProvider());
  assert.equal(runtime.providerMetadata()[0].provider, "ollama");
  assert.equal(runtime.providerMetadata()[0].requires_credential, false);
  assert.equal(runtime.providerMetadata()[0].transport, "http");
  const response = await runtime.invoke("ollama", request("llama3.1"));
  assert.equal(response.text, "local model");
  assert.equal(calls[0].url, "http://127.0.0.1:11434/v1/chat/completions");
  assert.equal(calls[0].headers.has("authorization"), false);
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

test("TTL credential registration rejects non-finite clock values", () => {
  for (const now of [Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY]) {
    const store = new CredentialStore({ clock: () => now });
    assert.throws(
      () => store.register("openai", "bounded-secret", { ttlMs: 1_000 }),
      /clock must return a finite number|expiry must be finite/,
    );
  }
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

test("local model discovery honors caller cancellation before and during its handler", async () => {
  let calls = 0;
  const beforeRuntime = new LLMRuntime();
  beforeRuntime.registerInMemoryProvider("cancelled-discovery-before", () => "unused", {
    discoverModels: () => {
      calls += 1;
      return { data: [] };
    },
  });
  const alreadyAborted = new AbortController();
  alreadyAborted.abort();
  await assert.rejects(
    beforeRuntime.discoverModels("cancelled-discovery-before", { signal: alreadyAborted.signal }),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
  assert.equal(calls, 0);

  const duringAbort = new AbortController();
  const duringRuntime = new LLMRuntime();
  duringRuntime.registerInMemoryProvider("cancelled-discovery-during", () => "unused", {
    discoverModels: async () => {
      calls += 1;
      duringAbort.abort();
      await Promise.resolve();
      return { data: [] };
    },
  });
  await assert.rejects(
    duringRuntime.discoverModels("cancelled-discovery-during", { signal: duringAbort.signal }),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
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

test("provider deadlines remain active while a response body is being consumed", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => new Response(new ReadableStream({
      start(controller) {
        init.signal.addEventListener(
          "abort",
          () => controller.error(new DOMException("aborted", "AbortError")),
          { once: true },
        );
      },
    }), { headers: { "content-type": "application/json" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("timed-body", "https://timed-body.test", {
    requiresCredential: false,
    timeoutMs: 1,
    maxAttempts: 1,
  }));
  await assert.rejects(
    runtime.invoke("timed-body", request()),
    (error) => error instanceof ProviderRuntimeError
      && error.code === "timeout"
      && error.provider === "timed-body"
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

test("stream collection accepts a terminal sentinel at body EOF", async () => {
  const sse = [
    'data: {"choices":[{"delta":{"content":"complete"}}]}',
    "data: [DONE]",
  ].join("\n\n");
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(sse, { headers: { "content-type": "text/event-stream" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("stream-eof-gateway", "https://stream-eof.test", { requiresCredential: false }));
  const response = await runtime.collectStream("stream-eof-gateway", request());
  assert.equal(response.text, "complete");
});

test("caller abort remains active for the full HTTP stream body lifetime", async () => {
  const encoder = new TextEncoder();
  let bodyCancelled = 0;
  const body = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode('data: {"choices":[{"delta":{"content":"first"}}]}\n\n'));
    },
    cancel() { bodyCancelled += 1; },
  });
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(body, { headers: { "content-type": "text/event-stream" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("abort-stream-gateway", "https://abort-stream.test", { requiresCredential: false }));
  const abort = new AbortController();
  const iterator = runtime.invokeStream(
    "abort-stream-gateway",
    request(),
    { signal: abort.signal },
  )[Symbol.asyncIterator]();
  const first = await iterator.next();
  assert.equal(first.value.textDelta, "first");
  abort.abort();
  await assert.rejects(
    () => iterator.next(),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
  assert.equal(bodyCancelled, 1);
});

test("closing an HTTP stream early cancels its response body", async () => {
  const encoder = new TextEncoder();
  let bodyCancelled = 0;
  const body = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode('data: {"choices":[{"delta":{"content":"first"}}]}\n\n'));
    },
    cancel() { bodyCancelled += 1; },
  });
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => new Response(body, { headers: { "content-type": "text/event-stream" } }),
  });
  runtime.registerProvider(openaiCompatibleProvider("closed-stream-gateway", "https://closed-stream.test", { requiresCredential: false }));
  const iterator = runtime.invokeStream("closed-stream-gateway", request())[Symbol.asyncIterator]();
  const first = await iterator.next();
  assert.equal(first.value.textDelta, "first");
  await iterator.return?.();
  assert.equal(bodyCancelled, 1);
});

test("caller abort stops and closes a local provider stream between events", async () => {
  let localClosed = false;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("abort-local-stream", () => "unused", {
    stream: async function* (input) {
      try {
        yield { provider: "abort-local-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "first", requestId: null, usage: {}, done: false };
        yield { provider: "abort-local-stream", model: input.model, sequence: 1, eventType: "done", textDelta: "second", requestId: null, usage: {}, done: true };
      } finally {
        localClosed = true;
      }
    },
  });
  const abort = new AbortController();
  const iterator = runtime.invokeStream(
    "abort-local-stream",
    request("local-stream-model"),
    { signal: abort.signal },
  )[Symbol.asyncIterator]();
  const first = await iterator.next();
  assert.equal(first.value.textDelta, "first");
  abort.abort();
  await assert.rejects(
    () => iterator.next(),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
  assert.equal(localClosed, true);
});

test("closing a dispatched stream finalizes its quota reservation", async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "quota-stream", model: "quota-stream-model", windowMs: 10_000, maxRequests: 4, maxOutputTokens: 512, maxConcurrent: 1 });
  const runtime = new LLMRuntime({ providerQuota: quota, fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("quota-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "quota-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "first", requestId: null, usage: {}, done: false };
      yield { provider: "quota-stream", model: input.model, sequence: 1, eventType: "done", textDelta: "", requestId: null, usage: {}, done: true };
    },
  });
  const iterator = runtime.invokeStream("quota-stream", request("quota-stream-model"))[Symbol.asyncIterator]();
  await iterator.next();
  await iterator.return?.();
  const [status] = quota.status("quota-stream", "quota-stream-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
});

test("closing a dispatched stream records an aborted observer, health, and execution outcome", async () => {
  const journal = new InMemoryAutonomousExecutionJournal();
  const execution = await AutonomousExecutionController.create({
    executionId: "closed-stream-execution",
    domain: "general",
    capability: "reasoning",
    riskClass: "read_only",
    policy: { max_steps: 4, max_provider_calls: 2, stop_on_error: false },
    journal,
  });
  const outcomes = [];
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("closed-outcome-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "closed-outcome-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "first", requestId: null, usage: {}, done: false };
      yield { provider: "closed-outcome-stream", model: input.model, sequence: 1, eventType: "done", textDelta: "", requestId: null, usage: {}, done: true };
    },
  });

  const iterator = runtime.invokeStream("closed-outcome-stream", request("closed-outcome-model"), {
    observer: { after: (_metadata, outcome) => outcomes.push({ ...outcome }) },
    execution,
  })[Symbol.asyncIterator]();
  assert.equal((await iterator.next()).value.textDelta, "first");
  await iterator.return?.();

  assert.equal(outcomes.length, 1);
  assert.equal(outcomes[0].success, false);
  assert.equal(outcomes[0].failureClass, "aborted");
  assert.equal(outcomes[0].failureCode, "aborted");
  const health = runtime.providerStatus("closed-outcome-stream");
  assert.equal(health.attempts, 1);
  assert.equal(health.failures, 1);
  const providerEvents = (await journal.events({ executionId: "closed-stream-execution" }))
    .filter((row) => row.event.kind === "provider_call");
  assert.equal(providerEvents.length, 2);
  assert.equal(providerEvents.at(-1).event.status, "provider_refused");
  assert.equal(providerEvents.at(-1).event.failure_class, "aborted");
});

test("outcome observer failures do not relabel or retry a successful provider response", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "settled" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("observer-success", "https://observer-success.test", { requiresCredential: false }));
  const response = await runtime.invoke("observer-success", request("observer-model"), {
    observer: { after: () => { throw new Error("diagnostic sink failed"); } },
  });
  assert.equal(response.text, "settled");
  assert.equal(calls, 1);
  const health = runtime.providerStatus("observer-success");
  assert.equal(health.attempts, 1);
  assert.equal(health.successes, 1);
  assert.equal(health.failures, 0);
});

test("stream outcome observer failures cannot leak concurrent quota", async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "observer-quota-stream", model: "observer-quota-model", windowMs: 10_000, maxRequests: 4, maxOutputTokens: 512, maxConcurrent: 1 });
  const runtime = new LLMRuntime({ providerQuota: quota, fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("observer-quota-stream", () => "unused", {
    stream: (input) => [
      { provider: "observer-quota-stream", model: input.model, sequence: 0, eventType: "done", textDelta: "done", requestId: null, usage: {}, done: true },
    ],
  });
  const events = [];
  for await (const event of runtime.invokeStream("observer-quota-stream", request("observer-quota-model"), {
    observer: { after: () => { throw new Error("stream diagnostic sink failed"); } },
  })) events.push(event);
  assert.equal(events.length, 1);
  const [status] = quota.status("observer-quota-stream", "observer-quota-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
});

test("malformed provider usage cannot strand a quota reservation or mask its failure", async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "malformed-usage", model: "usage-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  let malformed = true;
  const runtime = new LLMRuntime({ providerQuota: quota });
  runtime.registerInMemoryProvider("malformed-usage", () => malformed
    ? { output_text: "invalid counters", usage: { input_tokens: 2_000_000_001, output_tokens: 0 } }
    : { output_text: "bounded counters", usage: { input_tokens: 4, output_tokens: 2 } });

  await assert.rejects(
    runtime.invoke("malformed-usage", request("usage-model")),
    (error) => error instanceof ProviderRuntimeError && !error.message.includes("reservation was released"),
  );
  let [status] = quota.status("malformed-usage", "usage-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);

  malformed = false;
  assert.equal((await runtime.invoke("malformed-usage", request("usage-model"))).text, "bounded counters");
  [status] = quota.status("malformed-usage", "usage-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 2);
});

test("provider invocation effect boundary projects transient responses and blocks blind replay", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  let calls = 0;
  const runtime = new LLMRuntime({ effectBoundary: new AutonomousEffectBoundary({ journal }) });
  runtime.registerInMemoryProvider("offline-effect", (input) => {
    calls += 1;
    return { output_text: `private answer for ${input.model}`, request_id: "provider-request-1" };
  });
  const input = request("offline-effect-model", { idempotencyKey: "caller-owned-provider-key" });
  const response = await runtime.invoke("offline-effect", input);
  assert.equal(response.text, "private answer for offline-effect-model");
  assert.equal(calls, 1);
  const snapshot = await journal.snapshot();
  const encoded = JSON.stringify(snapshot);
  assert.equal(encoded.includes("Return a bounded answer."), false);
  assert.equal(encoded.includes("private answer"), false);
  assert.equal(encoded.includes("request_id"), false);
  assert.deepEqual((await journal.events()).map((row) => row.event.status), ["prepared", "dispatching", "dispatched", "completed"]);
  await assert.rejects(() => runtime.invoke("offline-effect", input), AutonomousEffectReconciliationRequiredError);
  assert.equal(calls, 1);
});

test("effect reconciliation releases the new invocation's quota and cost admission", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "effect-quota", model: "effect-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const runtime = new LLMRuntime({
    effectBoundary: new AutonomousEffectBoundary({ journal }),
    providerQuota: quota,
  });
  let calls = 0;
  runtime.registerInMemoryProvider("effect-quota", () => {
    calls += 1;
    return { output_text: "bounded" };
  });
  const first = request("effect-model", { idempotencyKey: "effect-quota-first" });
  await runtime.invoke("effect-quota", first);
  await assert.rejects(
    runtime.invoke("effect-quota", first),
    AutonomousEffectReconciliationRequiredError,
  );
  let [status] = quota.status("effect-quota", "effect-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);

  await runtime.invoke("effect-quota", request("effect-model", { idempotencyKey: "effect-quota-second" }));
  [status] = quota.status("effect-quota", "effect-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_used, 2);
  assert.equal(calls, 2);
});

test("provider authorization rejects before credential resolution or transport", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("transport must not run"); } });
  runtime.registerProvider(openaiCompatibleProvider("credentialed", "https://credentialed.invalid"));
  const { context } = providerAuthorizationContext(2, ["coding"]);
  await assert.rejects(
    () => runtime.invoke("credentialed", request("credentialed-model"), { authorizationContext: context, authorizationDomain: "science" }),
    AutonomousAuthorizationError,
  );
});

test("provider authorization consumes one fresh budget entry per invocation and stream", async () => {
  const { ledger, context } = providerAuthorizationContext();
  const calls = { invoke: 0, stream: 0 };
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("authorized-offline", (input) => {
    calls.invoke += 1;
    return `answer-${input.model}`;
  }, {
    stream: async function* (input) {
      calls.stream += 1;
      yield { provider: "authorized-offline", model: input.model, sequence: 0, eventType: "done", textDelta: "", requestId: null, usage: {}, done: true };
    },
  });
  const input = request("authorized-model");
  const response = await runtime.invoke("authorized-offline", input, { authorizationContext: context, authorizationDomain: "coding" });
  assert.equal(response.text, "answer-authorized-model");
  const events = [];
  for await (const event of runtime.invokeStream("authorized-offline", input, { authorizationContext: context, authorizationDomain: "coding" })) events.push(event);
  assert.equal(events[0].done, true);
  await assert.rejects(
    () => runtime.invoke("authorized-offline", input, { authorizationContext: context, authorizationDomain: "coding" }),
    AutonomousAuthorizationError,
  );
  assert.deepEqual(calls, { invoke: 1, stream: 1 });
  assert.equal(ledger.grants()[0].used_count, 2);
});

test("local provider dispatch digests normalize explicit undefined option fields", async () => {
  const dispatches = [];
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("normalized-dispatch", () => "bounded", {
    stream: (input) => [{
      provider: "normalized-dispatch",
      model: input.model,
      sequence: 0,
      eventType: "done",
      textDelta: "",
      requestId: null,
      usage: {},
      done: true,
    }],
  });
  const input = request("normalized-model", {
    temperature: undefined,
    requireJson: undefined,
    messages: [{ role: "user", content: "Return a bounded answer.", name: undefined }],
  });
  const options = { providerDispatchFence: async (context) => { dispatches.push(context); } };

  await runtime.invoke("normalized-dispatch", input, options);
  for await (const _event of runtime.invokeStream("normalized-dispatch", input, options)) { /* consume */ }

  assert.equal(dispatches.length, 2);
  assert.match(dispatches[0].requestDigest, /^[0-9a-f]{64}$/);
  assert.equal(dispatches[1].requestDigest, dispatches[0].requestDigest);
});

test("local transport consumes the detached request whose dispatch digest was fenced", async () => {
  let observedContent = null;
  const dispatches = [];
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("detached-local-dispatch", (input) => {
    observedContent = input.messages[0].content;
    return "bounded";
  });
  const input = request("detached-model");

  await runtime.invoke("detached-local-dispatch", input, {
    observer: {
      dispatch: () => {
        input.messages[0].content = "mutated after request validation";
      },
    },
    providerDispatchFence: (context) => {
      dispatches.push(context);
    },
  });

  assert.equal(observedContent, "Return a bounded answer.");
  assert.match(dispatches[0].requestDigest, /^[0-9a-f]{64}$/);
});

test("credential resolution is snapshotted before ordinary observers can replace it", async () => {
  let authorization = null;
  const runtime = new LLMRuntime({
    fetch: async (_url, init) => {
      authorization = new Headers(init.headers).get("Authorization");
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("credential-snapshot", "https://credential-snapshot.test"));
  const handle = runtime.credentials.register("credential-snapshot", "original-secret");
  const originalResolve = CredentialStore.prototype.resolve;
  try {
    await runtime.invoke("credential-snapshot", request("credential-model"), {
      credential: handle,
      observer: {
        before: () => {
          CredentialStore.prototype.resolve = () => "replacement-secret";
        },
      },
    });
  } finally {
    CredentialStore.prototype.resolve = originalResolve;
  }
  assert.equal(authorization, "Bearer original-secret");
});

test("provider wire bytes and endpoint ignore globals replaced by awaited callbacks", async () => {
  let observed = null;
  let dispatch = null;
  const runtime = new LLMRuntime({
    fetch: async (url, init) => {
      observed = { url: String(url), body: String(init.body) };
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("intrinsic-snapshot", "https://intrinsic-snapshot.test", { requiresCredential: false }));
  const originalStringify = JSON.stringify;
  const originalObjectKeys = Object.keys;
  const originalFreeze = Object.freeze;
  const originalTextEncoderEncode = TextEncoder.prototype.encode;
  const originalUrlToString = Object.getOwnPropertyDescriptor(URL.prototype, "toString");
  try {
    await runtime.invoke("intrinsic-snapshot", request("expected-model"), {
      observer: {
        before: () => {
          JSON.stringify = () => '{"model":"forged-model"}';
          Object.keys = (value) => {
            const keys = originalObjectKeys(value);
            return value?.provider === "intrinsic-snapshot" && value?.protocol === "openai_chat_completions" && "body" in value
              ? keys.filter((key) => key !== "body")
              : keys;
          };
          TextEncoder.prototype.encode = () => originalTextEncoderEncode.call(new TextEncoder(), "");
        },
        dispatch: () => {
          JSON.stringify = originalStringify;
          Object.keys = originalObjectKeys;
          TextEncoder.prototype.encode = originalTextEncoderEncode;
          Object.freeze = (value) => {
            Object.freeze = originalFreeze;
            return originalFreeze({ ...value, requestDigest: "0".repeat(64) });
          };
        },
      },
      providerDispatchFence: (context) => {
        Object.freeze = originalFreeze;
        dispatch = context;
        Object.defineProperty(URL.prototype, "toString", {
          configurable: true,
          value: () => "https://attacker.test/",
          writable: true,
        });
      },
    });
  } finally {
    JSON.stringify = originalStringify;
    Object.keys = originalObjectKeys;
    Object.freeze = originalFreeze;
    TextEncoder.prototype.encode = originalTextEncoderEncode;
    Object.defineProperty(URL.prototype, "toString", originalUrlToString);
  }
  assert.equal(observed.url, "https://intrinsic-snapshot.test/v1/chat/completions");
  assert.equal(JSON.parse(observed.body).model, "expected-model");
  assert.equal(dispatch.requestDigest, await digestJson({
    provider: "intrinsic-snapshot",
    protocol: "openai_chat_completions",
    stream: false,
    body: JSON.parse(observed.body),
  }));
});

test("credential revocation queued by the private fence is rechecked before transport", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not run" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("credential-final-probe", "https://credential-final-probe.test"));
  const handle = runtime.credentials.register("credential-final-probe", "short-lived-secret");

  await assert.rejects(
    () => runtime.invoke("credential-final-probe", request("credential-model"), {
      credential: handle,
      providerDispatchFence: () => {
        queueMicrotask(() => runtime.credentials.revoke(handle));
      },
    }),
    CredentialError,
  );
  assert.equal(calls, 0);
});

test("caller abort during the durable fence releases quota without charging transport", async () => {
  let calls = 0;
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "fence-abort", model: "fence-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const runtime = new LLMRuntime({
    providerQuota: quota,
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not run" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("fence-abort", "https://fence-abort.test", { requiresCredential: false }));
  const abort = new AbortController();

  await assert.rejects(
    runtime.invoke("fence-abort", request("fence-model"), {
      signal: abort.signal,
      providerDispatchFence: async () => {
        abort.abort();
        await Promise.resolve();
      },
    }),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
  assert.equal(calls, 0);
  const [status] = quota.status("fence-abort", "fence-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 0);
});

test("cost admission rejection unwinds quota for invoke and stream", async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "cost-admission", model: "cost-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const runtime = new LLMRuntime({ providerQuota: quota });
  runtime.registerInMemoryProvider("cost-admission", () => "bounded", {
    stream: (input) => [{
      provider: "cost-admission",
      model: input.model,
      sequence: 0,
      eventType: "done",
      textDelta: "bounded",
      requestId: null,
      usage: {},
      done: true,
    }],
  });
  const rejectCost = () => { throw new Error("cost admission refused"); };

  await assert.rejects(
    runtime.invoke("cost-admission", request("cost-model"), { reserveCost: rejectCost }),
    /cost admission refused/,
  );
  let [status] = quota.status("cost-admission", "cost-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 0);

  const iterator = runtime.invokeStream("cost-admission", request("cost-model"), {
    reserveCost: rejectCost,
  })[Symbol.asyncIterator]();
  await assert.rejects(() => iterator.next(), /cost admission refused/);
  [status] = quota.status("cost-admission", "cost-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 0);

  assert.equal((await runtime.invoke("cost-admission", request("cost-model"))).text, "bounded");
});

test("a later retry fence refusal retains quota and cost for an entered attempt", async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "retry-fence", model: "retry-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const budget = new AutonomousCostBudget(1);
  const fenceFailure = new Error("second retry fence refused");
  let calls = 0;
  const runtime = new LLMRuntime({
    providerQuota: quota,
    fetch: async () => {
      calls += 1;
      return jsonResponse({ error: "retry" }, 503);
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("retry-fence", "https://retry-fence.test", {
    requiresCredential: false,
    maxAttempts: 2,
    retryBackoffMs: 0,
  }));

  await assert.rejects(
    runtime.invoke("retry-fence", request("retry-model"), {
      estimatedCostUnits: 0.25,
      reserveCost: (costUnits) => budget.reserve(costUnits),
      providerDispatchFence: (context) => {
        if (context.transportAttempt === 2) throw fenceFailure;
      },
    }),
    (error) => error === fenceFailure,
  );

  assert.equal(calls, 1);
  const [status] = quota.status("retry-fence", "retry-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
  assert.equal(budget.consumedCostUnits, 0.25);
});

test("pending failure diagnostics observe settled quota before they resume", { timeout: 2_000 }, async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "pending-failure", model: "pending-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const budget = new AutonomousCostBudget(1);
  let enterObserver;
  let resumeObserver;
  const observerEntered = new Promise((resolve) => { enterObserver = resolve; });
  const observerResume = new Promise((resolve) => { resumeObserver = resolve; });
  const runtime = new LLMRuntime({
    providerQuota: quota,
    fetch: async () => jsonResponse({ error: "unavailable" }, 503),
  });
  runtime.registerProvider(openaiCompatibleProvider("pending-failure", "https://pending-failure.test", {
    requiresCredential: false,
    maxAttempts: 1,
  }));

  const failure = runtime.invoke("pending-failure", request("pending-model"), {
    estimatedCostUnits: 0.25,
    reserveCost: (costUnits) => budget.reserve(costUnits),
    observer: {
      after: async () => {
        enterObserver();
        await observerResume;
      },
    },
  }).then(() => null, (error) => error);

  await observerEntered;
  const [status] = quota.status("pending-failure", "pending-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
  assert.equal(budget.consumedCostUnits, 0.25);
  resumeObserver();
  const error = await failure;
  assert.ok(error instanceof ProviderRuntimeError);
  assert.equal(error.statusCode, 503);
});

test("pending pretransport diagnostics observe released quota and cost", { timeout: 2_000 }, async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "pending-predispatch", model: "pending-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const budget = new AutonomousCostBudget(1);
  let calls = 0;
  let enterObserver;
  let resumeObserver;
  const observerEntered = new Promise((resolve) => { enterObserver = resolve; });
  const observerResume = new Promise((resolve) => { resumeObserver = resolve; });
  const runtime = new LLMRuntime({
    providerQuota: quota,
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not run" } }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("pending-predispatch", "https://pending-predispatch.test"));
  const credential = runtime.credentials.register("pending-predispatch", "short-lived-secret");

  const failure = runtime.invoke("pending-predispatch", request("pending-model"), {
    credential,
    estimatedCostUnits: 0.25,
    reserveCost: (costUnits) => budget.reserve(costUnits),
    observer: {
      before: () => runtime.credentials.revoke(credential),
      after: async () => {
        enterObserver();
        await observerResume;
      },
    },
  }).then(() => null, (error) => error);

  await observerEntered;
  const [status] = quota.status("pending-predispatch", "pending-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 0);
  assert.equal(budget.consumedCostUnits, 0);
  assert.equal(calls, 0);
  resumeObserver();
  assert.ok((await failure) instanceof CredentialError);
});

test("pending early-stream diagnostics observe finalized quota", { timeout: 2_000 }, async () => {
  const quota = new ProviderQuotaController({ clock: () => 1_000 });
  quota.setPolicy({ provider: "pending-stream", model: "pending-model", windowMs: 10_000, maxRequests: 4, maxConcurrent: 1 });
  const budget = new AutonomousCostBudget(1);
  let enterObserver;
  let resumeObserver;
  const observerEntered = new Promise((resolve) => { enterObserver = resolve; });
  const observerResume = new Promise((resolve) => { resumeObserver = resolve; });
  const runtime = new LLMRuntime({ providerQuota: quota });
  runtime.registerInMemoryProvider("pending-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "pending-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "first", requestId: null, usage: {}, done: false };
      yield { provider: "pending-stream", model: input.model, sequence: 1, eventType: "done", textDelta: "", requestId: null, usage: {}, done: true };
    },
  });
  const iterator = runtime.invokeStream("pending-stream", request("pending-model"), {
    estimatedCostUnits: 0.25,
    reserveCost: (costUnits) => budget.reserve(costUnits),
    observer: {
      after: async () => {
        enterObserver();
        await observerResume;
      },
    },
  })[Symbol.asyncIterator]();

  assert.equal((await iterator.next()).value.textDelta, "first");
  const closed = iterator.return().then(() => null, (error) => error);
  await observerEntered;
  const [status] = quota.status("pending-stream", "pending-model");
  assert.equal(status.concurrent, 0);
  assert.equal(status.requests_reserved, 0);
  assert.equal(status.requests_used, 1);
  assert.equal(budget.consumedCostUnits, 0.25);
  resumeObserver();
  assert.equal(await closed, null);
});

test("caller abort during a local handler is authoritative after transport starts", async () => {
  const abort = new AbortController();
  let calls = 0;
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("local-midflight-abort", async () => {
    calls += 1;
    abort.abort();
    await Promise.resolve();
    return { output_text: "must not be accepted" };
  });

  await assert.rejects(
    runtime.invoke("local-midflight-abort", request("local-midflight-model"), { signal: abort.signal }),
    (error) => error instanceof ProviderRuntimeError && error.code === "aborted",
  );
  assert.equal(calls, 1);
  const health = runtime.providerStatus("local-midflight-abort");
  assert.equal(health.attempts, 1);
  assert.equal(health.failures, 1);
});

test("outcome observers and feedback cannot rewrite authoritative invocation receipts", async () => {
  const runtime = new LLMRuntime({
    fetch: async () => jsonResponse({
      choices: [{ message: { role: "assistant", content: "authoritative result" }, finish_reason: "stop" }],
      usage: { prompt_tokens: 7, completion_tokens: 3, total_tokens: 10 },
    }),
  });
  runtime.registerProvider(openaiCompatibleProvider("outcome-integrity", "https://outcome-integrity.test", { requiresCredential: false }));
  const agent = new AutonomousRuntime(runtime);
  const feedback = [];
  const result = await agent.invoke({
    task: "Keep provider outcome accounting authoritative.",
    domain: "general",
    capability: "reasoning",
    candidates: [{
      provider: "outcome-integrity",
      model: "outcome-model",
      context_window_tokens: 8_000,
      max_output_tokens: 256,
      quality: 0.9,
      latency_ms: 20,
      cost_per_million_tokens: 1,
      reliability: 0.99,
    }],
    request: request("selection-placeholder"),
  }, {
    observer: {
      after: (_metadata, outcome) => {
        outcome.success = false;
        outcome.status = "provider_refused";
        outcome.inputTokens = 999_999;
      },
    },
    feedback: (_selection, outcome) => feedback.push({ ...outcome }),
  });

  assert.equal(result.response.text, "authoritative result");
  assert.equal(result.provider_invocations.length, 1);
  assert.equal(result.provider_invocations[0].outcome, "success");
  assert.equal(result.provider_invocations[0].status, "completed");
  assert.equal(result.provider_invocations[0].input_tokens, 7);
  assert.equal(feedback[0].success, true);
  assert.equal(feedback[0].status, "completed");
  assert.equal(feedback[0].inputTokens, 7);
});

test("diagnostic observer failures cannot suppress autonomous feedback or receipts", async () => {
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("feedback-isolation", () => ({ output_text: "authoritative result", usage: { input_tokens: 3, output_tokens: 2 } }));
  const agent = new AutonomousRuntime(runtime);
  const feedback = [];
  const result = await agent.invoke({
    task: "Preserve internal learning when diagnostics fail.",
    domain: "general",
    capability: "reasoning",
    candidates: [{
      provider: "feedback-isolation",
      model: "feedback-model",
      context_window_tokens: 8_000,
      max_output_tokens: 256,
      quality: 0.9,
      latency_ms: 20,
      cost_per_million_tokens: 1,
      reliability: 0.99,
    }],
    request: request("selection-placeholder"),
  }, {
    observer: { after: () => { throw new Error("diagnostic observer failed"); } },
    feedback: (_selection, outcome) => feedback.push({ ...outcome }),
  });

  assert.equal(result.response.text, "authoritative result");
  assert.equal(result.provider_invocations.length, 1);
  assert.equal(result.provider_invocations[0].outcome, "success");
  assert.equal(feedback.length, 1);
  assert.equal(feedback[0].success, true);
});

test("provider authorization is consumed for each tool-loop turn", async () => {
  const { ledger, context } = providerAuthorizationContext();
  let calls = 0;
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("authorized-loop", () => {
    calls += 1;
    return calls === 1
      ? { output_text: "", tool_calls: [{ id: "call-1", name: "lookup", arguments: { query: "safe" } }] }
      : { output_text: "done" };
  });
  const result = await runtime.invokeToolLoop("authorized-loop", {
    ...request("authorized-loop-model"),
    tools: [{ name: "lookup", description: "Look up a fact.", parameters: { type: "object" } }],
  }, {
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    maxTurns: 3,
    authorizationContext: context,
    authorizationDomain: "coding",
  });
  assert.equal(result.status, "completed");
  assert.equal(result.turns, 2);
  assert.equal(calls, 2);
  assert.equal(ledger.grants()[0].used_count, 2);
});

test("provider effect boundary preserves definite local provider refusals", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const runtime = new LLMRuntime({ effectBoundary: new AutonomousEffectBoundary({ journal }) });
  runtime.registerInMemoryProvider("denied-effect", () => {
    throw new ProviderRuntimeError("denied", { statusCode: 401 });
  });
  await assert.rejects(
    () => runtime.invoke("denied-effect", request("denied-model", { idempotencyKey: "denied-key" })),
    (error) => error instanceof ProviderRuntimeError && error.statusCode === 401,
  );
  assert.equal((await journal.events()).at(-1).event.status, "failed");
});

test("live provider stream boundary reconciles partial consumption without retaining deltas", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const runtime = new LLMRuntime({ effectBoundary: new AutonomousEffectBoundary({ journal }) });
  runtime.registerInMemoryProvider("offline-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "offline-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "private delta", requestId: null, usage: {}, done: false };
      throw new Error("connection lost after first delta");
    },
  });
  const input = request("stream-model", { idempotencyKey: "stream-owner-key" });
  await assert.rejects(async () => {
    for await (const _event of runtime.invokeStream("offline-stream", input)) { /* consume */ }
  }, AutonomousEffectReconciliationRequiredError);
  assert.deepEqual((await journal.events()).map((row) => row.event.status), ["prepared", "dispatching", "dispatched", "uncertain"]);
  const encoded = JSON.stringify(await journal.snapshot());
  assert.doesNotMatch(encoded, /Return a bounded answer\.|private delta/);
});

test("live provider stream boundary completes only after exhaustion and blocks replay", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const runtime = new LLMRuntime({ effectBoundary: new AutonomousEffectBoundary({ journal }) });
  runtime.registerInMemoryProvider("offline-complete-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "offline-complete-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "bounded", requestId: null, usage: {}, done: false };
      yield { provider: "offline-complete-stream", model: input.model, sequence: 1, eventType: "done", textDelta: "", requestId: null, usage: {}, done: true };
    },
  });
  const input = request("stream-model", { idempotencyKey: "stream-complete-key" });
  const events = [];
  for await (const event of runtime.invokeStream("offline-complete-stream", input)) events.push(event);
  assert.deepEqual(events.map((event) => event.textDelta), ["bounded", ""]);
  assert.deepEqual((await journal.events()).map((row) => row.event.status), ["prepared", "dispatching", "dispatched", "completed"]);
  await assert.rejects(async () => {
    for await (const _event of runtime.invokeStream("offline-complete-stream", input)) { /* replay */ }
  }, AutonomousEffectReconciliationRequiredError);
});

test("collected provider streams require a terminal done event", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline-incomplete-stream", () => "unused", {
    stream: (input) => [{ provider: "offline-incomplete-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "partial output", requestId: null, usage: {}, done: false }],
  });
  await assert.rejects(
    () => runtime.collectStream("offline-incomplete-stream", request("stream-model")),
    (error) => error instanceof ProviderRuntimeError && error.code === "invalid_response" && error.message.includes("ended without a done event"),
  );
});

test("provider stream terminal events may carry finalized tool calls", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline-tool-stream", () => "unused", {
    stream: (input) => [{
      provider: "offline-tool-stream",
      model: input.model,
      sequence: 0,
      eventType: "tool.done",
      textDelta: "",
      requestId: null,
      usage: {},
      done: true,
      toolCall: { id: "call-1", name: "lookup", arguments: { query: "safe" } },
    }],
  });
  const response = await runtime.collectStream("offline-tool-stream", {
    ...request("stream-model"),
    tools: [{ name: "lookup", description: "lookup", parameters: { type: "object" } }],
  });
  assert.deepEqual(response.toolCalls.map((call) => call.name), ["lookup"]);
});

test("closing a dispatched live provider stream records uncertainty", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const runtime = new LLMRuntime({ effectBoundary: new AutonomousEffectBoundary({ journal }) });
  runtime.registerInMemoryProvider("offline-abandoned-stream", () => "unused", {
    stream: async function* (input) {
      yield { provider: "offline-abandoned-stream", model: input.model, sequence: 0, eventType: "text", textDelta: "first", requestId: null, usage: {}, done: false };
      yield { provider: "offline-abandoned-stream", model: input.model, sequence: 1, eventType: "text", textDelta: "second", requestId: null, usage: {}, done: false };
    },
  });
  const iterator = runtime.invokeStream("offline-abandoned-stream", request("stream-model", { idempotencyKey: "stream-abandoned-key" }))[Symbol.asyncIterator]();
  const first = await iterator.next();
  assert.equal(first.done, false);
  await iterator.return?.();
  assert.deepEqual((await journal.events()).map((row) => row.event.status), ["prepared", "dispatching", "dispatched", "uncertain"]);
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
  assert.equal(snapshot.snapshot_generation, 1);
  assert.equal(snapshot.previous_snapshot_digest, null);
  assert.deepEqual(await source.snapshotHealth(), snapshot);
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
  const forgedGeneration = structuredClone(snapshot);
  forgedGeneration.snapshot_generation = 2;
  forgedGeneration.previous_snapshot_digest = null;
  const { snapshot_digest: _forgedDigest, ...forgedBody } = forgedGeneration;
  forgedGeneration.snapshot_digest = await digestJson(forgedBody);
  await assert.rejects(validateLLMRuntimeHealthSnapshot(forgedGeneration), /generation and previous_snapshot_digest/);
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
  assert.equal(first.snapshot_generation, 1);
  assert.equal(first.previous_snapshot_digest, null);
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
  const second = await coordinator.flush();
  assert.equal(second.snapshot_generation, 2);
  assert.equal(second.previous_snapshot_digest, first.snapshot_digest);

  const legacy = structuredClone(first);
  delete legacy.snapshot_generation;
  delete legacy.previous_snapshot_digest;
  legacy.schema = "bioprism-typescript-llm-runtime-health-snapshot/0.1";
  const { snapshot_digest: _legacyDigest, ...legacyBody } = legacy;
  legacy.snapshot_digest = await digestJson(legacyBody);
  const legacyRuntime = new LLMRuntime({ fetch: async () => jsonResponse({ output_text: "legacy" }) });
  legacyRuntime.registerProvider(config);
  await legacyRuntime.restoreHealth(legacy);
  const upgraded = await legacyRuntime.snapshotHealth();
  assert.equal(upgraded.snapshot_generation, 1);
  assert.equal(upgraded.previous_snapshot_digest, null);
  assert.notEqual(upgraded.snapshot_digest, legacy.snapshot_digest);
  await staleRuntime.invoke("durable-health", request("model-c"));
  await assert.rejects(() => stale.flush(), /compare-and-swap conflict/);

  const canonical = textStore.encoded();
  textStore.write(JSON.stringify(JSON.parse(canonical), null, 2));
  await assert.rejects(() => persistence.read(), /not canonical/);
  textStore.write(canonical);
});

test("AutonomousAgent composes restart-safe transport health with exact runtime binding", async () => {
  const config = openaiCompatibleProvider("agent-health", "https://agent-health.test", {
    requiresCredential: false,
    maxAttempts: 1,
    circuitBreakerFailureThreshold: 1,
    circuitBreakerResetMs: 60_000,
  });
  const source = new LLMRuntime({ fetch: async () => jsonResponse({ error: "busy" }, 503) });
  source.registerProvider(config);
  let persisted = null;
  const persistence = {
    read: () => persisted,
    write: (snapshot) => { persisted = structuredClone(snapshot); },
  };
  const sourceAgent = new AutonomousAgent(source, {
    runtimeHealthPersistence: new LLMRuntimeHealthPersistenceCoordinator(source, persistence),
  });

  await assert.rejects(source.invoke("agent-health", request("agent-model")), /503/);
  const flushed = await sourceAgent.flushRuntimeHealth();
  assert.equal(flushed.providers[0].attempts, 1);
  assert.equal(flushed.providers[0].consecutive_failures, 1);
  assert.deepEqual(await sourceAgent.flushTransportHealth(), flushed);
  assert.equal(JSON.stringify(flushed).includes("authorization"), false);

  let restartedCalls = 0;
  const restarted = new LLMRuntime({ fetch: async () => { restartedCalls += 1; throw new Error("must not dispatch"); } });
  restarted.registerProvider(config);
  const restartedAgent = new AutonomousAgent(restarted, {
    runtimeHealthPersistence: new LLMRuntimeHealthPersistenceCoordinator(restarted, persistence),
  });
  const restored = await restartedAgent.restoreTransportHealth();
  assert.equal(restored?.snapshot_digest, flushed.snapshot_digest);
  assert.equal(restarted.providerStatus("agent-health").circuit, "open");
  await assert.rejects(restarted.invoke("agent-health", request("agent-model")), (error) => error instanceof ProviderRuntimeError && error.circuitOpen);
  assert.equal(restartedCalls, 0);

  const foreignRuntime = new LLMRuntime();
  assert.throws(
    () => new AutonomousAgent(foreignRuntime, { runtimeHealthPersistence: new LLMRuntimeHealthPersistenceCoordinator(source, persistence) }),
    /bound to the supplied LLMRuntime/,
  );
  await assert.rejects(new AutonomousAgent(foreignRuntime).restoreRuntimeHealth(), /not configured/);
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
  assert.equal(local.selection.selected_model.provider, "fast");
  assert.equal(feedback[0].success, true);
  assert.equal(calls[0], "https://fast.test/v1/chat/completions");

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
  await assert.rejects(
    gatedAgent.invoke({ ...plan, candidates: [{ ...candidates[0], provider: "openai", model: "gated-model", requires_credential: true }] }),
    CredentialError,
  );
});

test("weighted model selection is deterministic, auditable, and learning-policy aware", () => {
  const candidates = [
    { provider: "lab", model: "quality", context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.99, latency_ms: 800, cost_per_million_tokens: 100, reliability: 0.95 },
    { provider: "lab", model: "efficient", context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.78, latency_ms: 20, cost_per_million_tokens: 1, reliability: 0.9 },
  ];
  const base = {
    task: "choose a bounded model",
    domain: "evaluation",
    capability: "reasoning",
    risk_class: "review_required",
    required_capabilities: [],
    estimated_input_tokens: 100,
    requested_output_tokens: 100,
    candidates,
    provider_health: {
      lab: { provider: "lab", registered: true, circuit: "closed", credential_required: false, credential_ready: true, eligible: true, attempts: 0, successes: 0, failures: 0, success_rate: 0, mean_latency_ms: null, last_latency_ms: null, last_model: null, last_status_code: null },
    },
    model_health: {},
  };
  assert.deepEqual(normalizeAutonomousSelectionWeights(), DEFAULT_AUTONOMOUS_SELECTION_WEIGHTS);
  assert.deepEqual(normalizeAutonomousSelectionWeights({ cost: 2 }), { quality: 0.55, reliability: 0.25, cost: 2, latency: 0.1, exploration: 0.15 });
  assert.throws(() => normalizeAutonomousSelectionWeights({ quality: 0, reliability: 0, cost: 0, latency: 0, exploration: 0 }), /at least one positive/);

  const qualityFirst = rankAutonomousModels({ ...base, weights: { quality: 1, reliability: 0, cost: 0, latency: 0, exploration: 0 } });
  assert.equal(qualityFirst[0].model, "quality");
  assert.equal(typeof qualityFirst[0].base_score, "number");
  assert.equal(qualityFirst[0].observed_pulls, 0);

  const costFirst = rankAutonomousModels({ ...base, weights: { quality: 0.1, reliability: 0, cost: 10, latency: 0, exploration: 0 } });
  assert.equal(costFirst[0].model, "efficient");

  const disabledByLearning = rankAutonomousModels({
    ...base,
    observations: [{ arm_id: "lab/efficient", pulls: 12, reward_sum: 10, failures: 0, disabled: true }],
  });
  const efficient = disabledByLearning.find((row) => row.model === "efficient");
  assert.equal(efficient.eligible, false);
  assert.match(efficient.reasons.join(";"), /learning policy/);
  assert.equal(efficient.observed_pulls, 12);
});

test("autonomous runtime emits metadata-only selection lifecycle for selection and abstention", async () => {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ output_text: "selection lifecycle answer" }),
  });
  runtime.registerProvider(openaiCompatibleProvider("selection", "https://selection.test", { requiresCredential: false }));
  const candidate = { provider: "selection", model: "selection-1", context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.9, latency_ms: 25, cost_per_million_tokens: 5, reliability: 0.95 };
  const events = [];
  const agent = new AutonomousRuntime(runtime);
  const result = await agent.invoke({
    task: "Select one bounded model.",
    domain: "evaluation",
    capability: "reasoning",
    candidates: [candidate],
    request: request("selection-placeholder"),
  }, { selectionEventCallback: (event) => events.push(event) });
  assert.deepEqual(events.map((event) => event.phase), ["model_selection_started", "model_selection_finished"]);
  assert.equal(events[0].status, "running");
  assert.equal(events[0].selected_provider, null);
  assert.equal(events[1].status, "selected");
  assert.equal(events[1].selected_provider, "selection");
  assert.equal(events[1].selected_model, "selection-1");
  assert.equal(events[1].selection_digest.length, 64);
  assert.equal(events[1].candidate_count, 1);
  assert.equal(events[1].eligible_candidate_count, 1);
  assert.equal(result.selection.selected_model.provider, "selection");
  assert.ok(!JSON.stringify(events).includes("selection lifecycle answer"));

  const abstentionEvents = [];
  await assert.rejects(() => agent.invoke({
    task: "Abstain when confidence is insufficient.",
    domain: "evaluation",
    capability: "reasoning",
    minSelectionConfidence: 0.1,
    candidates: [candidate, { ...candidate, model: "selection-2" }],
    request: request("selection-placeholder"),
  }, { selectionEventCallback: (event) => abstentionEvents.push(event) }), /autonomous selection abstained/);
  assert.deepEqual(abstentionEvents.map((event) => event.status), ["running", "abstained"]);
  assert.equal(abstentionEvents[1].failure_code, "selection_abstained");
  assert.equal(abstentionEvents[1].selected_model, null);

  const failureEvents = [];
  const malformedSelector = new AutonomousRuntime(runtime, {
    selector: async () => ({ selected_model: { provider: "missing", model: "missing" }, ranking: [], strategy: "caller_selector", abstention_reason: null }),
  });
  await assert.rejects(() => malformedSelector.invoke({
    task: "Reject an invalid selector decision.",
    domain: "evaluation",
    capability: "reasoning",
    candidates: [candidate],
    request: request("selection-placeholder"),
  }, { selectionEventCallback: (event) => failureEvents.push(event) }), /ineligible model/);
  assert.deepEqual(failureEvents.map((event) => event.status), ["running", "failed"]);
  assert.equal(failureEvents[1].failure_code, "provider_error");
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
    fetch: async (url, init) => {
      calls.push({ url: String(url), idempotencyKey: new Headers(init.headers).get("Idempotency-Key") });
      if (String(url).startsWith("https://unstable.test")) return jsonResponse({ error: "busy" }, 503);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "stable answer" }, finish_reason: "stop" }] });
    },
  });
  runtime.registerProvider(openaiCompatibleProvider("unstable", "https://unstable.test", { requiresCredential: false, maxAttempts: 1 }));
  runtime.registerProvider(openaiCompatibleProvider("stable", "https://stable.test", { requiresCredential: false, maxAttempts: 1 }));
  const journal = new InMemoryAutonomousExecutionJournal();
  const selectionEvents = [];
  const execution = await AutonomousExecutionController.create({
    executionId: "autonomous-failover-1",
    domain: "general",
    capability: "reasoning",
    riskClass: "read_only",
    policy: { max_steps: 8, max_provider_calls: 2, max_provider_failovers: 1 },
    journal,
  });
  const agent = new AutonomousRuntime(runtime);
  const rootedRequest = request("selection-placeholder", { idempotencyKey: "operation-root-key" });
  const result = await agent.invoke({
    task: "Answer with one bounded sentence.",
    domain: "general",
    capability: "reasoning",
    candidates: [
      { provider: "unstable", model: "unstable-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "stable", model: "stable-model", context_window_tokens: 8_000, max_output_tokens: 512, quality: 0.6, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.7 },
    ],
    request: rootedRequest,
  }, { execution, selectionEventCallback: (event) => selectionEvents.push(event) });
  assert.equal(result.selection.selected_model.provider, "stable");
  assert.equal(result.response.text, "stable answer");
  assert.equal(result.provider_invocations.length, 2);
  assert.deepEqual(result.provider_invocations.map((receipt) => [receipt.attempt, receipt.provider, receipt.status, receipt.outcome]), [
    [0, "unstable", "provider_refused", "failure"],
    [1, "stable", "completed", "success"],
  ]);
  assert.equal(result.provider_invocations[0].execution_id, "autonomous-failover-1");
  assert.equal(result.provider_failover.fallback_count, 1);
  assert.equal(result.provider_failover.attempts.length, 2);
  assert.equal(result.continuation_plan.steps.map((step) => step.model_id).join(","), "unstable/unstable-model,stable/stable-model");
  assert.equal(result.provider_failover.continuation_plan_digest, result.continuation_plan.plan_digest);
  assert.doesNotMatch(JSON.stringify({ provider_invocations: result.provider_invocations, provider_failover: result.provider_failover }), /stable answer|\"busy\"|provider body|api[_ -]?key/i);
  const attemptKey = async (provider, model, attempt) => {
    const requestDigest = await digestJson({
      model,
      messages: rootedRequest.messages,
      max_output_tokens: rootedRequest.maxOutputTokens,
      temperature: rootedRequest.temperature ?? null,
      require_json: rootedRequest.requireJson ?? false,
      response_schema: rootedRequest.responseSchema ?? null,
      tools: rootedRequest.tools ?? [],
      tool_choice: rootedRequest.toolChoice ?? null,
    });
    return digestJson({
      schema: "bioprism-typescript-autonomous-provider-attempt-idempotency/0.1",
      root_idempotency_key: "operation-root-key",
      phase: "invoke",
      attempt,
      provider,
      model,
      request_digest: requestDigest,
    });
  };
  const firstAttemptKey = await attemptKey("unstable", "unstable-model", 1);
  const failoverKey = await attemptKey("stable", "stable-model", 2);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls, [
    { url: "https://unstable.test/v1/chat/completions", idempotencyKey: firstAttemptKey },
    { url: "https://stable.test/v1/chat/completions", idempotencyKey: failoverKey },
  ]);
  assert.notEqual(calls[0].idempotencyKey, calls[1].idempotencyKey);
  assert.equal(execution.state.provider_calls, 2);
  assert.equal(execution.state.provider_failovers, 1);
  assert.deepEqual(selectionEvents.map((event) => event.phase), ["model_selection_started", "model_selection_finished", "model_selection_started", "model_selection_finished"]);
  assert.deepEqual(selectionEvents.filter((event) => event.phase === "model_selection_finished").map((event) => [event.status, event.selected_provider, event.failover]), [["selected", "unstable", false], ["selected", "stable", true]]);
  assert.equal((await journal.verifyIntegrity()).verified, true);
  await execution.complete();
});

test("model continuation plans are immutable, failure-scoped, and resumable without reselection", async () => {
  const runtime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => jsonResponse({ output_text: "ok" }) });
  runtime.registerProvider(openaiCompatibleProvider("ladder", "https://ladder.test", { requiresCredential: false }));
  const agent = new AutonomousRuntime(runtime);
  const plan = {
    task: "Compile a reviewable model ladder.",
    candidates: [
      { provider: "ladder", model: "primary", context_window_tokens: 8_000, max_output_tokens: 256, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "ladder", model: "sibling", context_window_tokens: 8_000, max_output_tokens: 256, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 2, reliability: 0.9 },
      { provider: "ladder", model: "last", context_window_tokens: 8_000, max_output_tokens: 256, quality: 0.7, latency_ms: 30, cost_per_million_tokens: 3, reliability: 0.8 },
    ],
    request: request("primary"),
  };
  const selection = await agent.select(plan);
  const continuation = await compileAutonomousModelContinuationPlan(plan, selection, { maxFailovers: 2 });
  assert.deepEqual(await validateAutonomousModelContinuationPlan(continuation), continuation);
  assert.equal(continuation.steps.length, 3);
  assert.deepEqual(continuation.steps.map((step) => step.model_id), ["ladder/primary", "ladder/sibling", "ladder/last"]);
  const initialState = await createAutonomousModelContinuationState(continuation);
  const afterTimeout = await advanceAutonomousModelContinuationState(continuation, initialState, { provider: "ladder", model: "primary", failureScope: "model", failureCode: "timeout", statusCode: null });
  assert.deepEqual(await validateAutonomousModelContinuationState(continuation, afterTimeout), afterTimeout);
  assert.equal(afterTimeout.next_step_index, 1, "a model timeout preserves a sibling on the same provider");
  assert.deepEqual(afterTimeout.excluded_models, ["ladder/primary"]);
  const completed = await completeAutonomousModelContinuationState(continuation, afterTimeout, { provider: "ladder", model: "sibling", statusCode: 200 });
  assert.equal(completed.status, "completed");
  assert.equal(completed.attempts.length, 2);
  assert.deepEqual(await validateAutonomousModelContinuationState(continuation, completed), completed);
  await assert.rejects(advanceAutonomousModelContinuationState(continuation, { ...initialState, plan_digest: "0".repeat(64) }, { provider: "ladder", model: "primary", failureScope: "provider" }), /state digest mismatch|not bound/);
  await assert.rejects(compileAutonomousModelContinuationPlan({ ...plan, candidates: [...plan.candidates, { ...plan.candidates[0] }] }, selection, { maxFailovers: 2 }), /duplicate model/);

  const invalidPolicy = structuredClone(continuation);
  invalidPolicy.steps[0].failure_policy = { timeout_with_closed_circuit: "retry", retryable_provider_error: "exclude_provider" };
  const { plan_digest: _invalidPlanDigest, ...invalidPlanBody } = invalidPolicy;
  invalidPolicy.plan_digest = await digestJson(invalidPlanBody);
  await assert.rejects(validateAutonomousModelContinuationPlan(invalidPolicy), /failure policy/);

  const invalidAttempt = structuredClone(afterTimeout);
  invalidAttempt.attempts[0].provider = "other";
  const { state_digest: _invalidStateDigest, ...invalidStateBody } = invalidAttempt;
  invalidAttempt.state_digest = await digestJson(invalidStateBody);
  await assert.rejects(validateAutonomousModelContinuationState(continuation, invalidAttempt), /attempt identity/);
});

test("autonomous failover follows the compiled ladder even when a selector would change its mind", async () => {
  let selectorCalls = 0;
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url) => String(url).includes("ladder-unavailable") ? jsonResponse({ error: "busy" }, 503) : jsonResponse({ choices: [{ message: { role: "assistant", content: "ladder winner" }, finish_reason: "stop" }] }),
  });
  runtime.registerProvider(openaiCompatibleProvider("ladder-unavailable", "https://ladder-unavailable.test", { requiresCredential: false, maxAttempts: 1 }));
  runtime.registerProvider(openaiCompatibleProvider("ladder-backup", "https://ladder-backup.test", { requiresCredential: false, maxAttempts: 1 }));
  const agent = new AutonomousRuntime(runtime, {
    selector: async (input) => {
      selectorCalls += 1;
      return { selected_model: { provider: selectorCalls === 1 ? "ladder-unavailable" : "ladder-backup", model: selectorCalls === 1 ? "primary" : "backup" }, ranking: input.candidates.map((candidate) => ({ provider: candidate.provider, model: candidate.model, score: candidate.provider === "ladder-unavailable" ? 100 : 1, eligible: true, reasons: [] })), strategy: "caller_selector", abstention_reason: null };
    },
  });
  const result = await agent.invoke({
    task: "Keep the initial decision's bounded fallback order.",
    candidates: [
      { provider: "ladder-unavailable", model: "primary", context_window_tokens: 8_000, max_output_tokens: 256, quality: 0.99, latency_ms: 10, cost_per_million_tokens: 1, reliability: 0.99 },
      { provider: "ladder-backup", model: "backup", context_window_tokens: 8_000, max_output_tokens: 256, quality: 0.5, latency_ms: 100, cost_per_million_tokens: 5, reliability: 0.5 },
    ],
    request: request("primary"),
  }, { maxProviderFailovers: 1 });
  assert.equal(selectorCalls, 1);
  assert.equal(result.response.text, "ladder winner");
  assert.deepEqual(result.provider_invocations.map((receipt) => receipt.model), ["primary", "backup"]);
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
