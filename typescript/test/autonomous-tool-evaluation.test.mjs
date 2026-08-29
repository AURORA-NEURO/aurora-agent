import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousDomainToolRegistry,
  AutonomousToolOutcomeEvaluator,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  autonomousToolOutcomeEvaluationInput,
  builtinAutonomousDomainProfiles,
  digestJson,
} from "../dist/index.js";

test("evaluated tool receipts advance idempotent value-only bandits across every domain", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({
    name: binding.name,
    description: `Evaluation ${binding.name}`,
    inputSchema: { type: "object", additionalProperties: true },
  }))).map((definition) => [definition.name, definition])).values()];
  const rawValue = { private_result: "must never reach the evaluator" };
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: await ToolCatalogue.fromDefinitions(definitions),
    toolExecutor: async () => rawValue,
  });
  const brain = new AutonomousBrainFacade({ agent });
  const registry = await AutonomousDomainToolRegistry.create(await ToolCatalogue.fromDefinitions(definitions));
  for (const profile of profiles) {
    const plan = await registry.planForTask(`evaluate a reviewed ${profile.domain} capability`, {
      domains: [profile.domain],
      maxTools: 128,
    });
    const selected = plan.coverage.find((row) => row.domain === profile.domain && row.status === "selected");
    assert.ok(selected?.selected_tool, `${profile.domain} must have a selected tool`);
    const stage = profile.workflow.stages.find((candidate) => candidate.id === selected.stage_id);
    assert.ok(stage, `${profile.domain} must have a selected stage`);
    await brain.executeCapability({
      call_id: `evaluate-${profile.domain}`,
      tool: selected.selected_tool,
      arguments: {},
      workflow_context: {
        domain: profile.domain,
        workflow_id: profile.workflow.workflow_id,
        workflow_digest: profile.workflow.workflow_digest,
        stage_id: stage.id,
      },
      input_digest: await digestJson({ task: `evaluate ${profile.domain}` }),
    }, {
      approveEffects: true,
    });
  }

  const receipts = brain.toolExecutionEvidence();
  assert.equal(receipts.length, profiles.length);
  assert.ok(receipts.every((receipt) => typeof receipt.call_id === "string"));
  const callbackInputs = [];
  const evaluator = new AutonomousToolOutcomeEvaluator({
    evaluator_id: "tool-quality",
    evaluator_version: "2026-08-26",
    evaluate(input) {
      callbackInputs.push(input);
      assert.equal(input.status, "executed");
      assert.equal(input.evidence.quality_gate, "passed");
      assert.match(input.arguments_digest, /^[0-9a-f]{64}$/);
      assert.match(input.output_digest, /^[0-9a-f]{64}$/);
      assert.equal(input.private_result, undefined);
      assert.equal(input.arguments, undefined);
      assert.equal(input.output, undefined);
      assert.equal(input.response, undefined);
      assert.doesNotMatch(JSON.stringify(input), /must never reach the evaluator/);
      return { reward: 0.8, passed: true };
    },
  });
  const evidence = Object.fromEntries(receipts.map((receipt) => [receipt.call_id, { quality_gate: "passed" }]));
  const settled = await brain.evaluateToolReceipts({ evaluator, receipts, evidence });
  assert.equal(settled.status, "completed");
  assert.equal(settled.receipts, profiles.length);
  assert.equal(settled.evaluations.length, profiles.length);
  assert.equal(callbackInputs.length, profiles.length);
  assert.equal(new Set(settled.evaluations.map((evaluation) => evaluation.domain)).size, profiles.length);
  assert.equal(settled.next_tool_selection_state.generation, profiles.length);
  assert.equal(settled.next_tool_selection_state.arms.length, profiles.length);
  assert.equal(settled.next_tool_selection_state.credited_outcomes.length, profiles.length);
  assert.doesNotMatch(JSON.stringify(settled), /private_result|must never reach the evaluator/);

  const replayed = await brain.evaluateToolReceipts({
    evaluator,
    receipts,
    evidence,
    toolSelectionState: settled.next_tool_selection_state,
  });
  assert.equal(replayed.evaluations.every((evaluation) => evaluation.idempotent_replay), true);
  assert.deepEqual(replayed.next_tool_selection_state, settled.next_tool_selection_state);
  assert.equal(replayed.learning_digest, settled.learning_digest);
});

test("brain facade launch admission stops capability dispatch before the agent boundary", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "coding");
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
  const brain = new AutonomousBrainFacade({ agent });
  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  await assert.rejects(() => brain.executeCapabilityWithLaunchAdmission({
    call_id: "held-capability",
    tool: "not-dispatched",
    arguments: {},
    workflow_context: {
      domain: profile.domain,
      workflow_id: profile.workflow.workflow_id,
      workflow_digest: profile.workflow.workflow_digest,
      stage_id: profile.workflow.stages[0].id,
    },
    input_digest: "a".repeat(64),
  }, held), /launch admission is not approved/);
  await assert.rejects(() => brain.executeCapabilityBatchWithLaunchAdmission([{
    call_id: "held-capability-batch",
    tool: "not-dispatched",
    arguments: {},
    workflow_context: {
      domain: profile.domain,
      workflow_id: profile.workflow.workflow_id,
      workflow_digest: profile.workflow.workflow_digest,
      stage_id: profile.workflow.stages[0].id,
    },
    input_digest: "b".repeat(64),
  }], held), /launch admission is not approved/);
  await assert.rejects(() => brain.executeToolCallsWithLaunchAdmission([], held, { domains: ["coding"] }), /launch admission is not approved/);
  assert.deepEqual(brain.capabilityExecutionEvidence(), []);
});

test("brain facade preserves ordered capability batches and transient values", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((item) => item.domain === "coding");
  const binding = profile.tool_profile.bindings.find((row) => row.name === "conformance_run");
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: binding.name, inputSchema: { type: "object", additionalProperties: true } }]);
  let executions = 0;
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: catalogue,
    toolExecutor: async () => { executions += 1; return { private_batch_value: `value-${executions}` }; },
  });
  const brain = new AutonomousBrainFacade({ agent });
  const request = (index) => ({
    call_id: `facade-batch-${index}`,
    tool: binding.name,
    arguments: {},
    workflow_context: {
      domain: profile.domain,
      workflow_id: profile.workflow.workflow_id,
      workflow_digest: profile.workflow.workflow_digest,
      stage_id: "scope",
    },
    input_digest: "c".repeat(64),
    execution_id: `facade-execution-${index}`,
  });
  const batch = await brain.executeCapabilityBatch([request(1), request(2)], { approveEffects: true });
  assert.equal(batch.status, "completed");
  assert.equal(batch.completed_count, 2);
  assert.equal(batch.items.length, 2);
  assert.equal(batch.items.every((item) => item.result?.record.status === "completed"), true);
  assert.equal(executions, 2);
  assert.equal(batch.items[0].result.value.private_batch_value, "value-1");
  assert.doesNotMatch(JSON.stringify(batch.items.map((item) => item.result?.record)), /private_batch_value|value-1|value-2/);
  assert.equal(brain.capabilityExecutionEvidence().length, 2);
});

test("tool receipt evaluation fails closed on ambiguity, unsafe evidence, and invalid evaluator decisions", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const coding = profiles.find((profile) => profile.domain === "coding");
  const binding = coding.tool_profile.bindings.find((row) => row.read_only);
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: binding.name, inputSchema: { type: "object", additionalProperties: true } }]);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, [coding.tool_profile]);
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: catalogue,
    toolExecutor: async () => ({ ok: true }),
  });
  const plan = await registry.planForTask("evaluate an ambiguous coding capability", { domains: ["coding"], maxTools: 128 });
  const selected = plan.coverage.find((row) => row.domain === "coding" && row.status === "selected");
  assert.ok(selected?.selected_tool);
  const selectedBinding = coding.tool_profile.bindings.find((row) => row.name === selected.selected_tool);
  assert.ok(selectedBinding);
  const stage = coding.workflow.stages.find((candidate) => candidate.id === selected.stage_id);
  const result = await agent.executeCapability({
    call_id: "ambiguous-call",
    tool: selected.selected_tool,
    arguments: {},
    workflow_context: { domain: "coding", workflow_id: coding.workflow.workflow_id, workflow_digest: coding.workflow.workflow_digest, stage_id: stage.id },
    input_digest: await digestJson({ task: "ambiguous" }),
  });
  const receipt = agent.toolExecutionEvidence()[0];
  const evaluator = new AutonomousToolOutcomeEvaluator({
    evaluator_id: "strict-tool-quality",
    evaluator_version: "1",
    evaluate: () => ({ reward: 0, passed: false, failed: true }),
  });
  await assert.rejects(() => agent.evaluateToolReceipts({ evaluator, receipts: [receipt, receipt] }), /duplicate execution_id\/call_id/);
  await assert.rejects(() => agent.evaluateToolReceipts({ evaluator, receipts: [receipt], evidence: { [receipt.call_id]: { response: "forbidden" } } }), /transient or secret-shaped/);
  const badEvaluator = new AutonomousToolOutcomeEvaluator({
    evaluator_id: "bad-tool-quality",
    evaluator_version: "1",
    evaluate: () => ({ reward: 2, passed: true }),
  });
  await assert.rejects(() => agent.evaluateToolReceipts({ evaluator: badEvaluator, receipts: [receipt] }), /within \[-1, 1\]/);
  const input = await autonomousToolOutcomeEvaluationInput(receipt, { quality_gate: "failed" });
  assert.equal(input.domain, "coding");
  assert.equal(input.arguments, undefined);
  assert.equal(input.output, undefined);
  assert.equal(input.evidence.quality_gate, "failed");
  assert.equal(result.record.status, "completed");
});
