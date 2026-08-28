import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_STREAM_COMPLETION_SCHEMA,
  ArgumentError,
  AutonomousEffectBoundary,
  AutonomousRuntime,
  InMemoryAutonomousEffectJournal,
  LLMRuntime,
  ProviderRuntimeError,
} from "../dist/index.js";

const request = (model = "stream-model", overrides = {}) => ({
  model,
  messages: [{ role: "user", content: "Return a bounded stream." }],
  maxOutputTokens: 128,
  ...overrides,
});

const candidate = (provider, model, overrides = {}) => ({
  provider,
  model,
  capabilities: ["reasoning", "structured_output"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 20,
  cost_per_million_tokens: 0,
  reliability: 0.99,
  ...overrides,
});

const streamEvent = (provider, model, sequence, textDelta, done = false) => ({
  provider,
  model,
  sequence,
  eventType: done ? "fixture.done" : "fixture.text",
  textDelta,
  requestId: null,
  usage: done ? { output_tokens: 1 } : {},
  done,
});

test("autonomous streaming selects once, compacts once, and returns metadata-only completion", async () => {
  const seen = [];
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("stream-offline", () => "unused", {
    stream: (input) => {
      seen.push(input);
      return [
        streamEvent("stream-offline", input.model, 0, "bounded "),
        streamEvent("stream-offline", input.model, 1, "answer", true),
      ];
    },
  });
  const agent = new AutonomousRuntime(runtime);
  const handle = await agent.invokeStream({
    task: "stream a bounded answer",
    domain: "cross_domain",
    capability: "synthesis",
    candidates: [candidate("stream-offline", "stream-model")],
    request: request("stream-model", {
      messages: [
        { role: "system", content: "Protect the contract." },
        { role: "user", content: "old context to remove" },
        { role: "assistant", content: "old answer to remove" },
        { role: "user", content: "current task" },
      ],
    }),
    contextBudget: { maxInputTokens: 75, preserveRecentMessages: 1 },
  });

  assert.equal(handle.selection.selected_model.provider, "stream-offline");
  assert.equal(handle.context_budget.status, "compacted");
  assert.equal(handle.completion instanceof Promise, true);
  const events = [];
  for await (const event of handle.events) events.push(event);
  const completion = await handle.completion;

  assert.deepEqual(events.map((event) => event.textDelta), ["bounded ", "answer"]);
  assert.equal(completion.schema, AUTONOMOUS_STREAM_COMPLETION_SCHEMA);
  assert.equal(completion.status, "completed");
  assert.equal(completion.event_count, 2);
  assert.equal(completion.text_delta_bytes, "bounded answer".length);
  assert.equal(completion.done_seen, true);
  assert.equal(completion.provider_invocations.length, 1);
  assert.equal(completion.provider_invocations[0].provider, "stream-offline");
  assert.equal(completion.provider_invocations[0].outcome, "success");
  assert.equal(completion.provider_failover, null);
  assert.equal(JSON.stringify(completion).includes("bounded"), false);
  assert.deepEqual(seen[0].messages.map((message) => message.content), ["Protect the contract.", "current task"]);
  const secondConsumer = handle.events[Symbol.asyncIterator]();
  await assert.rejects(secondConsumer.next(), ArgumentError);
});

test("autonomous streaming fails over before the first event and records the ladder", async () => {
  const calls = [];
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("stream-primary", () => "unused", {
    stream: () => {
      calls.push("primary");
      throw new ProviderRuntimeError("temporary outage", { retryable: true, statusCode: 503 });
    },
  });
  runtime.registerInMemoryProvider("stream-backup", () => "unused", {
    stream: (input) => {
      calls.push("backup");
      return [streamEvent("stream-backup", input.model, 0, "recovered", true)];
    },
  });
  const agent = new AutonomousRuntime(runtime);
  const handle = await agent.invokeStream({
    task: "recover a stream before any output is visible",
    domain: "operations",
    candidates: [
      candidate("stream-primary", "primary-model", { quality: 0.99, latency_ms: 1 }),
      candidate("stream-backup", "backup-model", { quality: 0.5, latency_ms: 100 }),
    ],
    request: request("primary-model"),
  }, { maxProviderFailovers: 1 });

  const events = [];
  for await (const event of handle.events) events.push(event);
  const completion = await handle.completion;
  assert.deepEqual(calls, ["primary", "backup"]);
  assert.equal(events[0].provider, "stream-backup");
  assert.equal(completion.status, "completed");
  assert.equal(completion.provider_failover.fallback_count, 1);
  assert.equal(completion.provider_invocations.length, 2);
  assert.equal(completion.provider_invocations[0].outcome, "failure");
  assert.equal(completion.provider_invocations[1].outcome, "success");
});

test("autonomous streaming never replays a partial provider stream", async () => {
  const calls = [];
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("stream-partial", () => "unused", {
    stream: function* (input) {
      calls.push("partial");
      yield streamEvent("stream-partial", input.model, 0, "partial");
      throw new ProviderRuntimeError("connection lost", { retryable: true, statusCode: 503 });
    },
  });
  runtime.registerInMemoryProvider("stream-unused-backup", () => "unused", {
    stream: (input) => {
      calls.push("backup");
      return [streamEvent("stream-unused-backup", input.model, 0, "unsafe", true)];
    },
  });
  const agent = new AutonomousRuntime(runtime);
  const handle = await agent.invokeStream({
    task: "preserve partial stream safety",
    domain: "coding",
    candidates: [
      candidate("stream-partial", "partial-model", { quality: 0.99 }),
      candidate("stream-unused-backup", "backup-model", { quality: 0.5 }),
    ],
    request: request("partial-model"),
  }, { maxProviderFailovers: 1 });

  await assert.rejects((async () => {
    for await (const _event of handle.events) { /* consume until the provider fails */ }
  })(), (error) => error instanceof ProviderRuntimeError && error.retryable === true);
  const completion = await handle.completion;
  assert.deepEqual(calls, ["partial"]);
  assert.equal(completion.status, "failed");
  assert.equal(completion.event_count, 1);
  assert.equal(completion.done_seen, false);
  assert.equal(completion.provider_failover, null);
});

test("autonomous stream completion exposes the effect identity needed for reconciliation", async () => {
  const journal = new InMemoryAutonomousEffectJournal();
  const boundary = new AutonomousEffectBoundary({ journal });
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); }, effectBoundary: boundary });
  runtime.registerInMemoryProvider("stream-recovery", () => "unused", {
    stream: function* (input) {
      yield streamEvent("stream-recovery", input.model, 0, "transient");
      throw new ProviderRuntimeError("connection lost after first delta", { retryable: true, statusCode: 503 });
    },
  });
  const agent = new AutonomousRuntime(runtime);
  const handle = await agent.invokeStream({
    task: "expose the recovery identity without retaining output",
    domain: "operations",
    candidates: [candidate("stream-recovery", "recovery-model")],
    request: request("recovery-model"),
  });
  const iterator = handle.events[Symbol.asyncIterator]();
  assert.equal((await iterator.next()).value.textDelta, "transient");
  await iterator.return();
  const completion = await handle.completion;
  assert.equal(completion.status, "abandoned");
  assert.equal(completion.effect_ids.length, 1);
  const record = await journal.get(completion.effect_ids[0]);
  assert.equal(record.status, "uncertain");
  assert.equal(JSON.stringify(completion).includes("transient"), false);
});

test("autonomous stream abandonment is explicit and all built-in domains can use the contract", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("stream-domains", () => "unused", {
    stream: (input) => [streamEvent("stream-domains", input.model, 0, "domain", true)],
  });
  const agent = new AutonomousRuntime(runtime);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const handle = await agent.invokeStream({
      task: `bounded ${domain} stream`,
      domain,
      candidates: [candidate("stream-domains", "domain-model")],
      request: request("domain-model"),
    });
    const iterator = handle.events[Symbol.asyncIterator]();
    await iterator.next();
    await iterator.return();
    const completion = await handle.completion;
    assert.equal(completion.status, "abandoned", domain);
  }
});
