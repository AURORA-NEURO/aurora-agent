import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_CONTEXT_BUDGET_SCHEMA,
  AutonomousRuntime,
  LLMRuntime,
  ProviderRuntimeError,
  compactAutonomousProviderRequest,
} from "../dist/index.js";

const request = (messages, overrides = {}) => ({
  model: "offline-model",
  messages,
  maxOutputTokens: 128,
  ...overrides,
});

test("context budgeting drops the oldest removable messages while protecting instructions and the task", async () => {
  const result = await compactAutonomousProviderRequest(request([
    { role: "system", content: "Never disclose credentials." },
    { role: "user", content: "old task context that can be removed" },
    { role: "assistant", content: "old answer that can be removed" },
    { role: "user", content: "latest user task that must remain" },
  ]), { maxInputTokens: 75, preserveRecentMessages: 1 });

  assert.equal(result.plan.schema, AUTONOMOUS_CONTEXT_BUDGET_SCHEMA);
  assert.equal(result.plan.status, "compacted");
  assert.ok(result.plan.dropped_message_count > 0);
  assert.deepEqual(result.request.messages.map((message) => message.content), [
    "Never disclose credentials.",
    "latest user task that must remain",
  ]);
  assert.equal(result.plan.protected_instruction_count, 1);
  assert.equal(result.plan.messages_after, 2);
  assert.ok(result.plan.plan_digest.length >= 32);
});

test("context budgeting removes assistant tool calls and their results as one atomic unit", async () => {
  const result = await compactAutonomousProviderRequest(request([
    { role: "system", content: "Use only approved tools." },
    {
      role: "assistant",
      content: "",
      toolCalls: [{ id: "call-old", name: "lookup", arguments: { query: "private" } }],
    },
    { role: "tool", toolCallId: "call-old", content: "private result" },
    { role: "assistant", content: "old synthesis" },
  ]), { maxInputTokens: 45, preserveRecentMessages: 0 });

  assert.deepEqual(result.request.messages.map((message) => message.role), ["system"]);
  assert.deepEqual(result.plan.dropped_message_indexes, [1, 2, 3]);
  assert.equal(result.plan.tool_turns_dropped, 1);
});

test("context budgeting fails closed when protected context cannot fit", async () => {
  await assert.rejects(
    compactAutonomousProviderRequest(request([
      { role: "system", content: "This protected instruction is intentionally too large." },
    ]), { maxInputTokens: 1, preserveRecentMessages: 0 }),
    (error) => error instanceof ProviderRuntimeError && error.code === "invalid_request",
  );
});

test("unchanged context returns the original request and a digest-only plan", async () => {
  const secret = "https://private.example/opaque-image.png";
  const original = request([
    { role: "system", content: "Protect private data." },
    { role: "user", content: [{ type: "image_url", url: secret, detail: "high" }] },
  ]);
  const result = await compactAutonomousProviderRequest(original, {
    maxInputTokens: 10_000,
    preserveRecentMessages: 1,
  });

  assert.equal(result.plan.status, "unchanged");
  assert.equal(result.request, original);
  assert.equal(JSON.stringify(result.plan).includes(secret), false);
  assert.equal(JSON.stringify(result.plan).includes("Protect private data."), false);
  assert.equal(result.plan.content_retention, "provider_content_not_retained_in_plan");
});

test("autonomous invocation selects and dispatches the same compacted request", async () => {
  const seen = [];
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (input) => {
    seen.push(input);
    return { output_text: "bounded answer" };
  });
  const agent = new AutonomousRuntime(runtime);
  const result = await agent.invoke({
    task: "select and invoke within one context contract",
    domain: "research",
    capability: "reasoning",
    candidates: [{
      provider: "offline",
      model: "offline-model",
      capabilities: ["reasoning"],
      context_window_tokens: 100_000,
      max_output_tokens: 1_000,
      quality: 0.9,
      latency_ms: 10,
      cost_per_million_tokens: 0,
      reliability: 0.9,
    }],
    request: request("offline-model", {
      messages: [
        { role: "system", content: "Keep the contract." },
        { role: "user", content: "old context to remove" },
        { role: "assistant", content: "old response to remove" },
        { role: "user", content: "current task" },
      ],
    }),
    contextBudget: { maxInputTokens: 75, preserveRecentMessages: 1 },
  });

  assert.equal(result.response.text, "bounded answer");
  assert.equal(result.context_budget.status, "compacted");
  assert.deepEqual(seen[0].messages.map((message) => message.content), ["Keep the contract.", "current task"]);
});

test("tool-loop budgeting protects the newest approved continuation when the recent tail is zero", async () => {
  const seen = [];
  let calls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (input) => {
    seen.push(input);
    calls += 1;
    if (calls === 1) {
      return { tool_calls: [{ call_id: "call-1", name: "lookup", arguments: { query: "current" } }] };
    }
    assert.equal(input.messages.at(-1).role, "tool");
    return { output_text: "tool result incorporated" };
  });
  const result = await runtime.invokeToolLoop("offline", request("offline-model", {
    messages: [
      { role: "system", content: "Keep the contract." },
      { role: "user", content: "old context to remove" },
      { role: "user", content: "current task" },
    ],
    tools: [{ name: "lookup", description: "Read bounded data", parameters: { type: "object" } }],
  }), {
    contextBudget: { maxInputTokens: 150, preserveRecentMessages: 0 },
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { value: 42 } })),
  });

  assert.equal(result.status, "completed");
  assert.equal(result.turns, 2);
  assert.equal(seen.length, 2);
  assert.equal(seen[1].messages.at(-1).role, "tool");
});
