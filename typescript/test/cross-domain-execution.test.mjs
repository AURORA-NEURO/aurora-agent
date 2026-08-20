import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousCrossDomainExecutor,
  InMemoryAutonomousCrossDomainCheckpointStore,
  CredentialStore,
  LLMRuntime,
  digestJson,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(text) {
  return new Response(JSON.stringify({ choices: [{ message: { role: "assistant", content: text }, finish_reason: "stop" }] }), { status: 200, headers: { "content-type": "application/json" } });
}

function model() {
  return {
    provider: "durable-cross",
    model: "durable-cross-model",
    capabilities: ["reasoning", "coordination", "biomedical", "neuroscience", "science", "structured_output"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function makeAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse(calls === 3 ? "durable synthesis" : `durable child ${calls}`);
    },
  });
  llm.registerProvider(openaiCompatibleProvider("durable-cross", "https://durable-cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  return { agent, calls: () => calls };
}

const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
const subtasks = [
  { id: "bio", domain: "biomedical", task: "Review the biomedical evidence and safety boundary." },
  { id: "neuro", domain: "neuroscience", task: "Analyze the EEG neuroscience design and signal limits." },
];

test("durable cross-domain execution advances one bounded step and rehydrates caller-owned children", async () => {
  const { agent, calls } = makeAgent();
  const preview = await agent.blueprint(task, { subtasks });
  assert.ok(preview.cross_domain_blueprint);
  const blueprint = preview.cross_domain_blueprint;
  const acceptedPlan = {
    schema: "bioprism-python-autonomous-cross-domain-plan-refinement/0.1",
    status: "completed",
    task_digest: blueprint.task_digest,
    base_plan_digest: blueprint.plan_digest,
    priority_child_ids: [...blueprint.child_ids].reverse(),
    focus_child_ids: [blueprint.child_ids.at(-1)],
    review_required: false,
    confidence: 0.96,
    selected_model: { provider: "durable-cross", model: "durable-cross-model" },
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "child_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_or_effects_authorized",
  };
  const planDigest = await digestJson(acceptedPlan);
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  const results = new Map();
  const options = { candidates: agent.models(), subtasks, acceptedCrossDomainPlanRefinement: acceptedPlan, approveProviderCall: true, maxSteps: 1, jobId: "durable-cross-1" };

  const first = await executor.start(task, options);
  assert.equal(first.status, "paused");
  assert.equal(first.completed_children, 1);
  assert.equal(first.plan_refinement_digest, planDigest);
  assert.equal(calls(), 1);
  results.set(first.step_results[0].item_id, first.step_results[0].run);
  assert.equal(JSON.stringify(first.checkpoint).includes("durable child 1"), false);
  assert.equal(JSON.stringify(first.checkpoint).includes(subtasks[0].task), false);

  const resolveChildResult = (childId) => results.get(childId) ?? null;
  const second = await executor.resume("durable-cross-1", task, { ...options, jobId: undefined, resolveChildResult });
  assert.equal(second.status, "paused");
  assert.equal(second.completed_children, 2);
  assert.equal(second.checkpoint.status, "synthesis_pending");
  results.set(second.step_results[0].item_id, second.step_results[0].run);
  assert.equal(calls(), 2);

  const completed = await executor.resume("durable-cross-1", task, { ...options, jobId: undefined, maxSteps: 1, resolveChildResult });
  assert.equal(completed.status, "completed");
  assert.equal(completed.synthesis.response.text, "durable synthesis");
  assert.equal(completed.checkpoint.status, "completed");
  assert.equal(completed.checkpoint.synthesis_result_digest.length, 64);
  assert.equal(calls(), 3);
  assert.ok(completed.events.some((event) => event.event_type === "synthesis_completed"));
  assert.ok(completed.events.every((event) => !JSON.stringify(event).includes("durable child")));

  const snapshot = await store.snapshot();
  const restored = new InMemoryAutonomousCrossDomainCheckpointStore();
  await restored.restore(snapshot);
  const integrity = await restored.verifyIntegrity();
  assert.equal(integrity.verified, true);
  assert.equal(integrity.jobs, 1);
  assert.ok(integrity.events >= 4);
});
test("durable cross-domain execution pauses before dispatch and refuses a tampered rehydrated child", async () => {
  const { agent, calls } = makeAgent();
  const store = new InMemoryAutonomousCrossDomainCheckpointStore();
  const executor = new AutonomousCrossDomainExecutor(agent, store);
  await assert.rejects(executor.start(task, { candidates: agent.models(), subtasks, maxParallelChildren: 2 }), /sequential/);
  await assert.rejects(executor.start(task, { candidates: agent.models(), subtasks, allowPartial: true }), /does not synthesize partial/);
  const gated = await executor.start(task, { candidates: agent.models(), subtasks, approveProviderCall: false, jobId: "durable-cross-approval", maxSteps: 1 });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.checkpoint.status, "paused");
  assert.equal(calls(), 0);

  const first = await executor.resume("durable-cross-approval", task, { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 1 });
  assert.equal(first.completed_children, 1);
  assert.equal(calls(), 1);
  const childId = first.step_results[0].item_id;
  const tampered = { ...first.step_results[0].run, response: { ...first.step_results[0].run.response, text: "tampered" } };
  await assert.rejects(
    executor.resume("durable-cross-approval", task, { candidates: agent.models(), subtasks, approveProviderCall: true, maxSteps: 1, resolveChildResult: (id) => id === childId ? tampered : null }),
    /result digest does not match/,
  );
  assert.equal(calls(), 1, "digest refusal must happen before the next provider dispatch");
});
