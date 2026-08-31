import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  assertAutonomousDomainQualityPolicyCoverage,
  autonomousDomainQualityPolicy,
  autonomousDomainQualityPrompt,
  buildAutonomousDomainResponseContract,
  builtinAutonomousDomainProfiles,
  builtinAutonomousDomainQualityPolicies,
  evaluateAutonomousDomainResponseQuality,
  validateAutonomousDomainQualityPolicy,
} from "../dist/index.js";

function completeResponse(contract) {
  return {
    schema: "bioprism-typescript-autonomous-domain-response/0.1",
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: "complete",
    answer: `A bounded ${contract.domain} answer.`,
    observations: ["Observed input was inspected."],
    inferences: ["This is a bounded inference from the observed input."],
    uncertainty: ["External-world validation remains caller-owned."],
    evidence_gaps: ["No unprovided source was treated as evidence."],
    next_actions: ["Review the evidence and approve any requested effect."],
    stages: contract.stage_ids.map((stage_id) => ({
      stage_id,
      status: "complete",
      evidence: [`evidence:${stage_id}`],
      findings: [`finding:${stage_id}`],
      uncertainty: [],
      open_questions: [],
    })),
    domain_details: Object.fromEntries(contract.domain_fields.map((field) => [field, [`bounded ${field}`]])),
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
}

test("every built-in domain has a tamper-evident quality policy with domain-specific controls", () => {
  assert.equal(assertAutonomousDomainQualityPolicyCoverage(), true);
  const policies = builtinAutonomousDomainQualityPolicies();
  assert.deepEqual(policies.map((policy) => policy.domain), [...AUTONOMOUS_DOMAIN_NAMES]);
  for (const policy of policies) {
    assert.match(policy.policy_digest, /^[0-9a-f]{64}$/);
    assert.ok(policy.critical_detail_fields.length >= 4);
    assert.ok(policy.safety_detail_fields.length >= 2);
    assert.ok(policy.prompt_instructions.length >= 4);
    assert.deepEqual(validateAutonomousDomainQualityPolicy(policy), policy);
    const reordered = Object.fromEntries(Object.entries(policy).reverse());
    assert.equal(validateAutonomousDomainQualityPolicy(reordered).policy_digest, policy.policy_digest);
    assert.match(autonomousDomainQualityPrompt(policy), new RegExp(policy.domain));
  }
});

test("quality policy produces perfect readiness for complete responses across all domains", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const contract = await buildAutonomousDomainResponseContract(profile);
    const report = evaluateAutonomousDomainResponseQuality(completeResponse(contract), contract);
    assert.equal(report.domain, profile.domain);
    assert.equal(report.score, 1);
    assert.equal(report.passed, true);
    assert.deepEqual(report.missing_signals, []);
    assert.match(report.report_digest, /^[0-9a-f]{64}$/);
  }
});

test("quality gate catches domain-specific omissions and unsafe stage completion claims", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === "operations");
  const contract = await buildAutonomousDomainResponseContract(profile);
  const response = completeResponse(contract);
  response.domain_details.rollback_and_recovery = [];
  response.stages[0].evidence = [];
  const report = evaluateAutonomousDomainResponseQuality(response, contract);
  assert.equal(report.passed, false);
  assert.ok(report.missing_signals.includes("quality_stage_contract_coverage"));
  assert.ok(report.missing_signals.includes("quality_safety_control_coverage"));
  assert.ok(report.recommendations.some((item) => item.includes("rollback_and_recovery")));

  const incoherent = completeResponse(contract);
  incoherent.status = "partial";
  const incoherentReport = evaluateAutonomousDomainResponseQuality(incoherent, contract);
  assert.equal(incoherentReport.signals.quality_status_coherence, 0);
  assert.equal(incoherentReport.passed, false);
});

test("quality policies remain provider-free and do not authorize effects", () => {
  const policy = autonomousDomainQualityPolicy("biomedical");
  assert.equal(policy.retention, "policy_metadata_only;does_not_establish_external_truth");
  assert.equal(policy.secret_material, "never_returned");
  assert.ok(policy.prompt_instructions.some((instruction) => instruction.includes("medical authorization")));
});
