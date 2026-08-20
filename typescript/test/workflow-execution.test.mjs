import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function model() {
  return {
    provider: "workflow",
    model: "workflow-model",
    capabilities: ["reasoning", "code", "coordination", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 50,
    cost_per_million_tokens: 10,
    reliability: 0.99,
  };
}

test("workflow executor checkpoints stages, pauses at a bounded budget, and resumes by digest", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: `stage-output-${calls}` }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const store = new InMemoryAutonomousWorkflowCheckpointStore();
  const executor = new AutonomousWorkflowExecutor(agent, store);
  const task = "Implement and verify this repository change";

  const first = await executor.start(task, { domain: "coding", jobId: "workflow-job-1", candidates: agent.models(), approveProviderCall: true, maxStages: 2 });
  assert.equal(first.status, "paused");
  assert.equal(first.completed_stage_count, 2);
  assert.equal(first.total_stage_count, 5);
  assert.equal(first.checkpoint.status, "paused");
  assert.equal(JSON.stringify(first.checkpoint).includes(task), false);
  assert.equal(calls, 2);

  const resumed = await executor.resume("workflow-job-1", task, { candidates: agent.models(), approveProviderCall: true, maxStages: 32 });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.completed_stage_count, 5);
  assert.equal(resumed.checkpoint.next_stage_id, null);
  assert.equal(calls, 5);
  assert.deepEqual(resumed.checkpoint.completed_stage_ids, ["scope", "inspect", "implement", "verify", "handoff"]);
  assert.ok(resumed.events.length >= 6);
  for (let index = 1; index < resumed.events.length; index += 1) {
    assert.equal(resumed.events[index].previous_event_digest, resumed.events[index - 1].event_digest);
    assert.equal(resumed.events[index].sequence, resumed.events[index - 1].sequence + 1);
  }
  await assert.rejects(() => executor.resume("workflow-job-1", "A different task", { candidates: agent.models(), approveProviderCall: true }), /digest/);
});

test("workflow executor exposes approval pauses and checkpoint readiness for every built-in domain", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("provider must not be called before approval"); } });
  llm.registerProvider(openaiCompatibleProvider("workflow", "https://workflow.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
    const result = await executor.start(`Review a ${profile.domain} workflow`, { domain: profile.domain, jobId: `domain-${profile.domain}`, candidates: agent.models() });
    assert.equal(result.status, "approval_required", profile.domain);
    assert.equal(result.checkpoint.domain, profile.domain);
    assert.equal(result.checkpoint.workflow_digest, profile.workflow.workflow_digest);
    assert.equal(result.checkpoint.completed_stage_ids.length, 0);
    assert.equal(result.events.at(-1).event_type, "approval_required");
  }
});
