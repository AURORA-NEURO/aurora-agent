import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  assembleAutonomousPrompt,
  buildAutonomousEvidencePlan,
  builtinAutonomousDomainProfiles,
} from "../dist/index.js";

test("evidence planning covers every built-in domain and exposes dependency-safe next stages", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const plan = await buildAutonomousEvidencePlan(profiles.map((profile) => profile.workflow));
  assert.equal(profiles.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(new Set(plan.domains).size, 12);
  assert.ok(plan.requirements.length > 40);
  assert.equal(plan.coverage_status, "not_evaluated");
  assert.equal(plan.missing_requirement_ids.length, plan.requirements.length);
  assert.equal(plan.next_stage_ids.length, 12);
  assert.equal(plan.plan_digest.length, 64);
  assert.equal(plan.toJSON().execution, "planning_only;no_source_or_provider_dispatch");
  assert.equal(plan.toJSON().secret_material, "never_returned");
});

test("evidence planning requires fully qualified IDs when output labels are shared", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const workflows = profiles.map((profile) => profile.workflow);
  const base = await buildAutonomousEvidencePlan(workflows);
  const complete = await buildAutonomousEvidencePlan(workflows, { availableEvidence: base.requirements.map((item) => item.requirement_id) });
  assert.equal(complete.coverage_status, "complete");
  assert.equal(complete.coverage_ratio, 1);
  const ambiguous = await buildAutonomousEvidencePlan(workflows, { availableEvidence: ["observations"] });
  assert.equal(ambiguous.coverage_status, "missing");
  assert.equal(ambiguous.covered_requirement_ids.length, 0);
});

test("prompt assembly includes the evidence contract while keeping acquisition and authority explicit", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === "science");
  assert.ok(profile);
  const prompt = await assembleAutonomousPrompt(profile, "Design a reproducible experiment.");
  const evidence = prompt.messages.find((message) => message.source_id === "autonomy-evidence-plan");
  assert.ok(evidence);
  assert.match(evidence.content, /planning_only;no_source_or_provider_dispatch/);
  assert.match(evidence.content, /evidence was acquired/);
});
