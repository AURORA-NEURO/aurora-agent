import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA,
  AUTONOMOUS_RUN_STREAM_SCHEMA,
  ArgumentError,
  AutonomousAgent,
  AutonomousBrainFacade,
  LLMRuntime,
  routeAutonomousEvidenceScope,
} from "../dist/index.js";

const model = {
  provider: "stream-agent-offline",
  model: "stream-agent-model",
  capabilities: ["reasoning", "structured_output", "code", "science", "coordination", "cross_domain", "evaluation", "operations", "data", "web"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.95,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

const event = (input, sequence, textDelta, done = false) => ({
  provider: "stream-agent-offline",
  model: input.model,
  sequence,
  eventType: done ? "fixture.done" : "fixture.text",
  textDelta,
  requestId: null,
  usage: done ? { output_tokens: 1 } : {},
  done,
});

function agent() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("stream-agent-offline", () => "unused", {
    stream: (input) => [event(input, 0, "high-level "), event(input, 1, "stream", true)],
  });
  const value = new AutonomousAgent(runtime);
  value.registerModel(model);
  return value;
}

test("AutonomousAgent exposes direct and automatic live streams after approval", async () => {
  const value = agent();
  const handle = await value.runStream("debug and verify a bounded repository change", {
    domain: "coding",
    approveProviderCall: true,
  });
  assert.equal(handle.schema, AUTONOMOUS_RUN_STREAM_SCHEMA);
  assert.equal(handle.selection.selected_model.provider, model.provider);
  assert.equal(handle.blueprint.domain_profile.domain, "coding");
  const events = [];
  for await (const item of handle.events) events.push(item);
  const completion = await handle.completion;
  assert.deepEqual(events.map((item) => item.event.textDelta), ["high-level ", "stream"]);
  assert.equal(events.every((item) => item.kind === "provider" && item.stage === "direct"), true);
  assert.equal(completion.schema, AUTONOMOUS_RUN_STREAM_COMPLETION_SCHEMA);
  assert.equal(completion.status, "completed");
  assert.equal(completion.event_count, 2);
  assert.equal(completion.text_delta_bytes, "high-level stream".length);
  assert.equal(completion.stage_count, 1);
  assert.equal(JSON.stringify(completion).includes("high-level"), false);
  await assert.rejects(value.runAutoStream("debug and verify a bounded repository change", {
    domain: "coding",
    planningMode: "provider",
    approveProviderCall: true,
  }), ArgumentError);
});

test("AutonomousAgent stream preflight refuses without provider approval and never dispatches", async () => {
  const value = agent();
  const handle = await value.runStream("debug and verify a bounded repository change", { domain: "coding" });
  assert.equal(handle.selection, null);
  const events = [];
  for await (const item of handle.events) events.push(item);
  const completion = await handle.completion;
  assert.deepEqual(events, []);
  assert.equal(completion.status, "approval_required");
  assert.equal(completion.event_count, 0);
});

test("cross-domain stream multiplexes bounded specialists before synthesis", async () => {
  const value = agent();
  const route = await routeAutonomousEvidenceScope("compare coding and science evidence", ["coding", "science"]);
  const handle = await value.runCrossDomainStream("compare coding and science evidence", {
    routeOverride: route,
    approveProviderCall: true,
    maxParallelChildren: 2,
  });
  const events = [];
  for await (const item of handle.events) events.push(item);
  const completion = await handle.completion;
  assert.equal(handle.blueprint.child_blueprints.length, 2);
  assert.equal(events.filter((item) => item.kind === "lifecycle" && item.phase === "child_started").length, 2);
  assert.equal(events.filter((item) => item.kind === "lifecycle" && item.phase === "child_completed").length, 2);
  assert.equal(events.some((item) => item.kind === "provider" && item.stage === "child"), true);
  assert.equal(events.some((item) => item.kind === "provider" && item.stage === "synthesis"), true);
  assert.equal(completion.status, "completed");
  assert.equal(completion.stage_count, 3);
  assert.equal(completion.inner_completions.length, 3);
  assert.equal(JSON.stringify(completion).includes("high-level"), false);
});

test("brain facade exposes validated direct and automatic stream entrypoints", async () => {
  const brain = new AutonomousBrainFacade({ agent: agent() });
  const handle = await brain.executeAutoStream({
    task: "debug and verify a bounded repository change",
    domain: "coding",
  }, { approveProviderCall: true });
  let text = "";
  for await (const item of handle.events) if (item.kind === "provider") text += item.event.textDelta;
  assert.equal(text, "high-level stream");
  assert.equal((await handle.completion).status, "completed");
  await assert.rejects(brain.executeStream({
    task: "connector-bearing stream must not skip setup",
    domain: "coding",
    connector: { domain: "coding", operation: "review" },
  }, { approveProviderCall: true }), ArgumentError);
});
