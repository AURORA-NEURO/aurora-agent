import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_TASK_LENS_DOMAINS,
  assembleAutonomousPrompt,
  autonomousDomainTaskLens,
  autonomousTaskLensPromptContract,
  builtinAutonomousDomainProfiles,
  builtinAutonomousDomainTaskLenses,
  compileAutonomousPlan,
} from "../dist/index.js";

test("built-in task lenses cover all domains with canonical cross-runtime digests", () => {
  const lenses = builtinAutonomousDomainTaskLenses();
  assert.deepEqual(lenses.map((lens) => lens.domain), AUTONOMOUS_DOMAIN_NAMES);
  assert.deepEqual(lenses.map((lens) => lens.domain), AUTONOMOUS_TASK_LENS_DOMAINS);
  assert.equal(new Set(lenses.map((lens) => lens.lens_id)).size, 12);
  assert.equal(new Set(lenses.map((lens) => lens.lens_digest)).size, 12);
  assert.equal(autonomousDomainTaskLens("coding").lens_digest, "616bf58c2e6dcfb4bb926477b692c9d28fa0a3737ce17279852a662bdee68a51");
  assert.equal(autonomousDomainTaskLens("evaluation").lens_digest, "065a919cc799ca3d2acbe95b5b98502b230c45f42cd5e79464db4d4725eb2136");

  for (const lens of lenses) {
    const contract = autonomousTaskLensPromptContract(lens);
    assert.equal(contract.lens_digest, lens.lens_digest);
    assert.equal(contract.model_hints_are, "preferences_only; they do not authorize or hard-gate a model");
    assert.equal(contract.execution, "guidance_only; provider_and_effect_boundaries_remain_separate");
    assert.equal(contract.secret_material, "never_returned");
    for (const group of ["planning_dimensions", "decision_checks", "evidence_priorities", "evaluator_signals", "model_capability_hints", "output_sections", "failure_modes"]) {
      assert.ok(Array.isArray(lens[group]) && lens[group].length > 0, `${lens.domain}.${group}`);
      assert.equal(new Set(lens[group]).size, lens[group].length, `${lens.domain}.${group} duplicates`);
    }
  }
});

test("task lenses bind to prompts and provider plans without changing authority", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((row) => row.domain === "coding");
  assert.ok(profile);
  const lens = autonomousDomainTaskLens("coding");
  const plan = await compileAutonomousPlan(profile, "Implement and verify the change.", {
    taskDigest: "a".repeat(64),
    activeToolNames: ["repository_catalog"],
    selectedToolNames: ["repository_catalog"],
  });
  assert.equal(plan.task_lens_digest, lens.lens_digest);
  assert.equal(typeof plan.task_intent_digest, "string");
  assert.ok(plan.steps.every((step) => step.arguments.task_lens_id === lens.lens_id));
  assert.ok(plan.steps.every((step) => step.arguments.task_lens_digest === lens.lens_digest));
  assert.ok(plan.steps.every((step) => typeof step.arguments.task_intent_digest === "string"));
  assert.equal(plan.requires_approval, true);
  assert.equal(plan.execution, "not_started");

  const prompt = await assembleAutonomousPrompt(profile, "Implement and verify the change.", { maxInputTokens: 4_096 });
  assert.ok(prompt.messages.some((message) => message.content.includes(lens.lens_id)));
  assert.ok(prompt.messages.some((message) => message.content.includes(lens.lens_digest)));
});
