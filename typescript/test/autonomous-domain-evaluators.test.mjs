import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousCompositeValueEvaluator,
  AutonomousValueEvaluatorAdapter,
  AutonomousValueEvaluatorRegistry,
  builtinAutonomousValueEvaluatorProfiles,
} from "../dist/index.js";

function perfectEvidence(profile, domain = profile.domain) {
  const signalNames = new Set([...profile.required_signals, ...Object.keys(profile.signal_weights)]);
  return {
    domain,
    capability: `${domain}-capability`,
    risk_class: "bounded-review",
    signals: Object.fromEntries([...signalNames].map((signal) => [signal, true])),
    references: ["a".repeat(64)],
    limitations: ["caller-declared signals only"],
    selected_tool_names: [`${domain}.verify`],
  };
}

test("specialized value-only profiles cover every autonomous domain", () => {
  const profiles = builtinAutonomousValueEvaluatorProfiles();
  assert.deepEqual(profiles.map((profile) => profile.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  assert.equal(new Set(profiles.map((profile) => profile.evaluator_id)).size, AUTONOMOUS_DOMAIN_NAMES.length);
  for (const profile of profiles) {
    assert.equal(profile.execution, "caller_declared_signal_scoring_only");
    assert.ok(profile.required_signals.length > 0);
    assert.ok(Object.values(profile.signal_weights).every((weight) => weight > 0));
    assert.equal(new AutonomousValueEvaluatorAdapter(profile).catalogueEntry().domain, profile.domain);
  }
});

test("every specialized evaluator produces deterministic passing reward from bounded evidence", () => {
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const evaluator = registry.resolveForAutonomousDomain(domain);
    const result = evaluator.assess({ evidence: perfectEvidence(evaluator.profile) });
    assert.equal(result.domain, domain);
    assert.equal(result.reward, 1, domain);
    assert.equal(result.passed, true, domain);
    assert.equal(result.failed, false, domain);
    assert.equal(result.replan_requested, false, domain);
    assert.match(result.evidence_digest, /^[0-9a-f]{64}$/, domain);
    assert.match(result.evaluation_digest, /^[0-9a-f]{64}$/, domain);
    assert.equal(result.secret_material, "never_returned", domain);
  }
  assert.equal(registry.catalogue().length, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("missing and below-threshold signals fail closed with actionable bounded replanning", () => {
  const evaluator = AutonomousValueEvaluatorRegistry.withBuiltinProfiles().resolve("coding");
  const missing = evaluator.assess({});
  assert.equal(missing.failure_class, "missing_domain_evidence");
  assert.equal(missing.reward, 0);
  assert.equal(missing.replan_requested, true);

  const evidence = perfectEvidence(evaluator.profile);
  evidence.signals.tests_passed = 0.9;
  const failed = evaluator.assess({ evidence });
  assert.equal(failed.failure_class, "domain_evidence_gate");
  assert.equal(failed.passed, false);
  assert.equal(failed.replan_requested, true);
  assert.deepEqual(failed.below_threshold_signals, ["tests_passed"]);
  assert.match(failed.replan_instruction, /^Address bounded coding evaluation gaps:/);
  const reward = evaluator.toRewardInput({ evidence });
  assert.equal(reward.evaluator_id, "autonomous-coding-quality");
  assert.equal(reward.failed, true);
});

test("composite evaluator routes explicit domains without mixing rubrics", () => {
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  const composite = AutonomousCompositeValueEvaluator.fromRegistry(registry, ["coding", "biomedical"]);
  const coding = composite.assess({ context: { domain: "coding" }, evidence: perfectEvidence(registry.resolve("coding").profile) });
  assert.equal(coding.evaluator_id, "composite-domain-quality");
  assert.equal(coding.domain, "coding");
  assert.equal(coding.passed, true);

  const unmapped = composite.assess({ context: { domain: "science" }, evidence: perfectEvidence(registry.resolve("coding").profile) });
  assert.equal(unmapped.failure_class, "unmapped_domain_evaluator");
  assert.equal(unmapped.replan_requested, true);
  assert.equal(composite.catalogueEntry().domains.length, 2);
});

test("evidence rejects secrets, unsupported payloads, and cross-domain scope violations", () => {
  const evaluator = AutonomousValueEvaluatorRegistry.withBuiltinProfiles().resolve("coding");
  assert.throws(() => evaluator.assess({ evidence: { ...perfectEvidence(evaluator.profile), api_key: "must-not-enter" } }), /forbidden secret-shaped|unsupported fields/);
  assert.throws(() => evaluator.assess({ evidence: { ...perfectEvidence(evaluator.profile), prompt: "raw provider prompt" } }), /unsupported fields/);
  assert.throws(() => evaluator.assess({ evidence: perfectEvidence(evaluator.profile, "biomedical") }), /cannot evaluate/);
  assert.throws(() => evaluator.assess({ evidence: { ...perfectEvidence(evaluator.profile), references: ["not-a-digest"] } }), /SHA-256/);
  const registry = AutonomousValueEvaluatorRegistry.withBuiltinProfiles();
  assert.equal(registry.resolveForReplay("coding", { evaluator_id: "autonomous-coding-quality", evaluator_version: "1" }).evaluatorId, "autonomous-coding-quality");
  assert.throws(() => registry.resolveForReplay("coding", { evaluator_id: "drifted-rubric", evaluator_version: "1" }), /identity does not match/);
});
