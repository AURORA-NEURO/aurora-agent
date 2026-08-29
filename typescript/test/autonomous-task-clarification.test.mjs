import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  ArgumentError,
  AutonomousTaskClarificationError,
  LLMRuntime,
  autonomousDomainPolicy,
  autonomousDomainTaskLens,
  digestJsonSync,
  inferAutonomousTaskDecision,
  inferAutonomousTaskIntent,
  planAutonomousTaskClarification,
  resolveAutonomousTaskClarification,
  validateAutonomousTaskClarificationPlan,
  validateAutonomousTaskClarificationResolution,
} from "../dist/index.js";

function artifacts(task = "analyze the dataset lineage", domain = "data") {
  const lens = autonomousDomainTaskLens(domain);
  const intent = inferAutonomousTaskIntent({
    task,
    taskDigest: digestJsonSync({ task }),
    domain,
    capability: domain === "data" ? "data_analysis" : "reasoning",
    riskClass: "read_only",
    workflowId: domain === "data" ? "data_workflow" : `${domain}_workflow`,
    lens,
  });
  const policy = autonomousDomainPolicy(domain);
  const decision = inferAutonomousTaskDecision({ intent, lens, policy, requiredModelCapabilities: ["reasoning", "structured_output"] });
  return { intent, lens, policy, decision };
}

test("clarification plan is cross-runtime digest-bound", () => {
  const { intent, lens, policy, decision } = artifacts();
  const plan = planAutonomousTaskClarification({ intent, lens, policy, decision });
  assert.equal(plan.plan_digest, "56d71e320c406c0266f5c25150c7b1107c7e766e75a75b56cef89df38d7392f6");
  assert.equal(plan.status, "required");
  assert.deepEqual(plan.questions.map((question) => question.kind), ["output", "evidence"]);
  assert.equal(JSON.stringify(plan).includes("analyze the dataset lineage"), false);
  assert.equal(plan.authorization, "interaction_guidance_only;does_not_authorize_provider_source_tool_or_effect_actions");
  assert.equal(validateAutonomousTaskClarificationPlan(plan).plan_digest, plan.plan_digest);
});

test("clarification answers are transient and require complete contracts", () => {
  const { intent, lens, policy, decision } = artifacts();
  const plan = planAutonomousTaskClarification({ intent, lens, policy, decision });
  const [output, evidence] = plan.questions;
  const partial = resolveAutonomousTaskClarification(plan, { taskDigest: intent.task_digest, answers: { [output.question_id]: "PRIVATE output answer" } });
  assert.equal(partial.status, "still_required");
  assert.equal(partial.answered_count, 1);
  assert.ok(partial.unanswered_question_ids.includes(evidence.question_id));
  assert.equal(JSON.stringify(partial).includes("PRIVATE output answer"), false);
  const resolved = resolveAutonomousTaskClarification(plan, { taskDigest: intent.task_digest, answers: { [output.question_id]: "PRIVATE output answer", [evidence.question_id]: "caller catalogue" } });
  assert.equal(resolved.status, "resolved");
  assert.equal(resolved.required_answer_count, 2);
  assert.equal(resolved.answer_digests.length, 2);
  assert.equal(validateAutonomousTaskClarificationResolution(resolved, plan).resolution_digest, resolved.resolution_digest);
  const tampered = { ...plan, plan_digest: "0".repeat(64) };
  assert.throws(() => validateAutonomousTaskClarificationPlan(tampered), AutonomousTaskClarificationError);
  assert.throws(() => resolveAutonomousTaskClarification(plan, { taskDigest: "0".repeat(64), answers: {} }), AutonomousTaskClarificationError);
  assert.throws(() => resolveAutonomousTaskClarification(plan, { taskDigest: intent.task_digest, answers: { unknown: "x" } }), AutonomousTaskClarificationError);
  assert.throws(() => validateAutonomousTaskClarificationResolution({ ...resolved, resolution_digest: "0".repeat(64) }, plan), AutonomousTaskClarificationError);
});

test("clarification handles every domain and blocks forbidden effects without a bypass", () => {
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const { intent, lens, policy, decision } = artifacts(`review the ${domain} workflow and report verification gaps`, domain);
    const plan = planAutonomousTaskClarification({ intent, lens, policy, decision });
    assert.equal(plan.domain, domain);
    assert.ok(plan.review_dimensions.length > 0);
    assert.equal(plan.plan_digest.length, 64);
  }
  const { intent, lens, policy, decision } = artifacts("deploy the biomedical report and verify safety", "biomedical");
  const blocked = planAutonomousTaskClarification({ intent, lens, policy, decision });
  assert.equal(blocked.status, "blocked");
  assert.deepEqual(blocked.questions, []);
  assert.ok(blocked.missing_contracts.includes("policy_blocker"));
  assert.throws(() => resolveAutonomousTaskClarification(blocked, { taskDigest: intent.task_digest, answers: { bypass: "yes" } }), AutonomousTaskClarificationError);
});

test("agent facade uses the same preflight and answer receipt", async () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const task = "analyze the dataset lineage";
  const plan = await agent.clarificationPlan(task, { domain: "data" });
  const blueprint = await agent.blueprint(task, { domain: "data" });
  assert.ok(blueprint.blueprint);
  assert.equal(plan.intent_digest, blueprint.blueprint.task_intent.intent_digest);
  assert.equal(plan.plan_digest.length, 64);
  const answers = Object.fromEntries(plan.questions.map((question) => [question.question_id, "caller-owned boundary"]));
  const receipt = await agent.resolveClarification(plan, task, answers);
  assert.equal(receipt.status, "resolved");
  const restored = await agent.validateClarification(plan, receipt);
  assert.equal(restored.resolution_digest, receipt.resolution_digest);
  await assert.rejects(() => agent.validateClarification({ ...plan, plan_digest: "0".repeat(64) }, receipt), AutonomousTaskClarificationError);
});

test("clarification recompile rebuilds a fresh blueprint for every domain", async () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = `review the ${domain} workflow`;
    const plan = await agent.clarificationPlan(task, { domain });
    const answers = Object.fromEntries(plan.questions.map((question) => [question.question_id, question.answer_kind === "choice" ? question.options[0] : "caller-owned clarified boundary"]));
    const receipt = await agent.resolveClarification(plan, task, answers);
    const recompiled = await agent.recompileClarification(plan, receipt, task, `review the ${domain} workflow and produce a bounded verification report`, { desiredOutputs: ["bounded verification report"] });
    assert.equal(recompiled.domain, domain);
    assert.equal(recompiled.blueprint.domain_profile.domain, domain);
    assert.equal(recompiled.recompiled_task_digest, recompiled.blueprint.task_digest);
    const publicProjection = JSON.stringify(recompiled);
    assert.equal(publicProjection.includes(task), false);
    assert.equal(publicProjection.includes("caller-owned clarified boundary"), false);
    assert.equal(recompiled.toJSON().authorization, "recompile_only; provider_source_tool_and_effect_gates_remain_required");
    const restored = await agent.validateClarificationRecompile(recompiled, plan, receipt);
    assert.equal(restored.recompile_digest, recompiled.recompile_digest);
    await assert.rejects(() => agent.validateClarificationRecompile({ ...recompiled.toJSON(), recompile_digest: "0".repeat(64) }, plan, receipt), ArgumentError);
  }

  const task = "review the data workflow";
  const plan = await agent.clarificationPlan(task, { domain: "data" });
  const partial = await agent.resolveClarification(plan, task, {});
  await assert.rejects(() => agent.recompileClarification(plan, partial, task, "review the data workflow and produce a report"), ArgumentError);
  const answers = Object.fromEntries(plan.questions.map((question) => [question.question_id, "caller-owned boundary"]));
  const receipt = await agent.resolveClarification(plan, task, answers);
  await assert.rejects(() => agent.recompileClarification(plan, receipt, "a different task", "review the data workflow and produce a report"), ArgumentError);
});
