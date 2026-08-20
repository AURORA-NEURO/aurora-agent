import { test } from "node:test";
import assert from "node:assert/strict";

import {
  AutonomousExecutionController,
  AutonomousExecutionPolicyError,
  InMemoryAutonomousExecutionJournal,
} from "../dist/index.js";

const digest = (character) => character.repeat(64);

test("execution controller enforces provider/tool budgets and records metadata-only state", async () => {
  const journal = new InMemoryAutonomousExecutionJournal({ maxEvents: 32 });
  const controller = await AutonomousExecutionController.create({
    executionId: "execution-budget-1",
    domain: "coding",
    capability: "code_review",
    riskClass: "read_only",
    policy: { max_steps: 4, max_provider_calls: 1, max_tool_calls: 1, max_replans: 1, max_cost_units: 2 },
    journal,
  });
  await controller.admitProviderCall({ provider: "test-provider", model: "test-model", invocationKind: "autonomous_selected_model", attempt: 1, turn: 1, selectionDigest: digest("a"), estimatedCostUnits: 1, costUnits: 1 });
  await controller.recordProviderOutcome({ provider: "test-provider", model: "test-model", invocationKind: "autonomous_selected_model", attempt: 1, turn: 1, status: "completed", outcome: "success", latencyMs: 4, inputTokens: 10, outputTokens: 20, estimatedCostUnits: 1, actualCostUnits: 1, selectionDigest: digest("a"), outcomeDigest: digest("b") });
  await controller.recordEvaluation({ evaluatorId: "reviewer", evaluatorVersion: "1", reward: 0.8, passed: true, evaluationDigest: digest("c") });
  await controller.complete();

  assert.equal(controller.state.status, "completed");
  assert.equal(controller.state.provider_calls, 1);
  assert.equal(controller.state.cost_units, 1);
  assert.equal((await journal.verifyIntegrity()).verified, true);
  assert.equal((await journal.events({ executionId: "execution-budget-1" })).length, 5);
  assert.equal(JSON.stringify(await journal.events()).includes("test-provider"), true);
  assert.equal(JSON.stringify(await journal.events()).includes("Debug this private task"), false);
  const firstEvent = (await journal.events({ executionId: "execution-budget-1" }))[0].event;
  await assert.rejects(
    journal.append({ ...firstEvent, prompt: "raw provider prompt" }),
    /unsupported fields/,
  );
  await assert.rejects(
    controller.admitProviderCall({ provider: "test-provider", model: "test-model", costUnits: 0 }),
    AutonomousExecutionPolicyError,
  );
});

test("execution journal resumes only with the same policy and rejects terminal resumes", async () => {
  const journal = new InMemoryAutonomousExecutionJournal();
  const policy = { max_steps: 8, max_provider_calls: 4, max_cost_units: 8 };
  const first = await AutonomousExecutionController.create({ executionId: "execution-resume-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy, journal });
  await first.checkpoint({ status: "paused", reason: "awaiting_review" });
  const resumed = await AutonomousExecutionController.create({ executionId: "execution-resume-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy, journal, resume: true });
  assert.equal(resumed.state.status, "resumed");
  await assert.rejects(
    AutonomousExecutionController.create({ executionId: "execution-resume-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy: { ...policy, max_steps: 9 }, journal, resume: true }),
    /policy digest/,
  );
  await resumed.complete();
  await assert.rejects(
    AutonomousExecutionController.create({ executionId: "execution-resume-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy, journal, resume: true }),
    /terminal execution/,
  );

  const nonStandardTerminal = await AutonomousExecutionController.create({ executionId: "execution-resume-limit-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy, journal });
  await nonStandardTerminal.complete("replan_limit_reached");
  await assert.rejects(
    AutonomousExecutionController.create({ executionId: "execution-resume-limit-1", domain: "science", capability: "experiment_design", riskClass: "read_only", policy, journal, resume: true }),
    /terminal execution/,
  );
});

test("execution controller refuses unapproved effectful tools", async () => {
  const controller = await AutonomousExecutionController.create({ executionId: "execution-tools-1", domain: "operations", capability: "incident_response", riskClass: "operational_effect", policy: { max_effectful_calls: 0, allow_side_effects: false } });
  await assert.rejects(
    controller.admitToolCall({ tool: "incident_write", callId: "call-1", readOnly: false, approvalRequired: true }),
    /side effects are disabled/,
  );
  await controller.admitToolCall({ tool: "read_catalog", callId: "call-2", readOnly: true, approvalRequired: false });
  await controller.recordToolOutcome({ tool: "read_catalog", callId: "call-2", status: "completed", outcomeDigest: digest("d") });
  assert.equal(controller.state.tool_calls, 1);
});

test("shared execution journals serialize concurrent session starts", async () => {
  const journal = new InMemoryAutonomousExecutionJournal();
  await Promise.all([
    AutonomousExecutionController.create({ executionId: "execution-concurrent-a", domain: "coding", capability: "review", riskClass: "read_only", journal }),
    AutonomousExecutionController.create({ executionId: "execution-concurrent-b", domain: "science", capability: "analysis", riskClass: "read_only", journal }),
  ]);
  const rows = await journal.events();
  assert.deepEqual(rows.map((row) => row.sequence), [1, 2]);
  assert.equal((await journal.verifyIntegrity()).verified, true);
});
