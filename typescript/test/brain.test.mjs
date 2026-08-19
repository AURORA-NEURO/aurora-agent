import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const model = {
  provider: "openai",
  model: "test-model",
  capabilities: ["reasoning"],
  context_window_tokens: 16000,
  max_output_tokens: 2000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
};

test("client exposes the autonomous brain value-only kernel", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const path = new URL(String(input)).pathname;
      seen.push({ path, body: JSON.parse(init.body) });
      if (path.endsWith("brain_model_select")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_model_select", mcp: { result: { structuredContent: { selected_model_id: "openai/test-model" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_prompt_assemble")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_prompt_assemble", mcp: { result: { structuredContent: { prompt_digest: "p" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_plan")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_plan", mcp: { result: { structuredContent: { ok: true } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      if (path.endsWith("brain_bandit_select")) {
        return new Response(JSON.stringify({ ok: true, tool: "brain_bandit_select", mcp: { result: { structuredContent: { selected_arm_id: "openai/test-model" } } } }), { status: 200, headers: { "content-type": "application/json" } });
      }
      return new Response(JSON.stringify({ ok: true, tool: "brain_bandit_update", mcp: { result: { structuredContent: { generation: 1 } } } }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const selected = await client.brainModelSelect({
    task: "reason",
    input_tokens: 100,
    requested_output_tokens: 100,
    models: [model],
  });
  const prompt = await client.brainPromptAssemble({ task: "reason", max_input_tokens: 100 });
  const plan = await client.brainPlan({
    objective: "reason",
    steps: [{ id: "invoke", objective: "invoke", tool: "provider.invoke" }],
    allowed_tools: ["provider.invoke"],
    max_cost: 10,
  });
  const state = { schema: "bioprism-brain-bandit/0.1", arms: [{ arm_id: "openai/test-model" }] };
  const bandit = await client.brainBanditSelect(state);
  const updated = await client.brainBanditUpdate(state, { arm_id: "openai/test-model", reward: 0.8 });

  assert.equal(selected.mcp.result.structuredContent.selected_model_id, "openai/test-model");
  assert.equal(prompt.mcp.result.structuredContent.prompt_digest, "p");
  assert.equal(plan.mcp.result.structuredContent.ok, true);
  assert.equal(bandit.mcp.result.structuredContent.selected_arm_id, "openai/test-model");
  assert.equal(updated.mcp.result.structuredContent.generation, 1);
  assert.equal(seen.length, 5);
  assert.ok(seen.every(({ body }) => !Object.prototype.hasOwnProperty.call(body, "api_key")));
});

test("brain client methods fail before transport on malformed input", async () => {
  const client = new ApiClient({ baseUrl: "http://127.0.0.1:18788", fetch: async () => { throw new Error("must not call transport"); } });
  await assert.rejects(() => client.brainPromptAssemble({ task: "", max_input_tokens: 10 }), ArgumentError);
  await assert.rejects(() => client.brainModelSelect({ task: "x", input_tokens: 1, requested_output_tokens: 1, models: [] }), ArgumentError);
});
