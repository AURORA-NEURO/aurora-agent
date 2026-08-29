import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousClaimIntegrityClaim,
  AutonomousClaimIntegrityEvidence,
  AutonomousClaimIntegrityPolicy,
  LLMRuntime,
  assessAutonomousOutcomeIntegrity,
  bindAutonomousOutcomeIntegrityClaims,
  buildAutonomousDomainResponseContract,
  builtinAutonomousDomainProfiles,
  digestJsonSync,
  validateAutonomousOutcomeIntegrity,
  validateAutonomousOutcomeIntegritySnapshot,
} from "../dist/index.js";

const REFERENCE = "2026-08-26T12:00:00Z";
const digest = (value) => digestJsonSync({ value });
const run = (overrides = {}) => ({
  task_digest: digest("outcome-task"),
  route_digest: digest("route"),
  status: "completed",
  mode: "single_domain",
  domains: ["science"],
  output_digest: digest("answer"),
  response_digest: digest("response"),
  outcome_digest: digest("outcome"),
  ...overrides,
});
function claim(claimId = "claim-1") {
  return new AutonomousClaimIntegrityClaim({ claimId, domain: "science", claimDigest: digest(`claim:${claimId}`) });
}
function evidence(claimId = "claim-1") {
  return new AutonomousClaimIntegrityEvidence({
    evidenceId: "evidence-1",
    domain: "science",
    claimIds: [claimId],
    sourceId: "source-1",
    sourceDigest: digest("source-1"),
    evidenceDigest: digest("evidence-1"),
    observedAt: "2026-08-25T12:00:00Z",
    reliability: 0.9,
    support: 0.9,
    status: "accepted",
    stance: "support",
    modality: "primary",
    reproducibility: "reproduced",
  });
}
function binding(overrides = {}) {
  return {
    claim_id: "claim-1",
    domain: "science",
    role: "run_output",
    output_digest: digest("answer"),
    response_digest: digest("response"),
    ...overrides,
  };
}

function responseFor(contract) {
  return {
    schema: "bioprism-typescript-autonomous-domain-response/0.1",
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: "complete",
    answer: `Bounded answer for ${contract.domain}.`,
    observations: ["bounded observation"],
    inferences: ["bounded inference"],
    uncertainty: ["bounded uncertainty"],
    evidence_gaps: ["bounded evidence gap"],
    next_actions: ["review the next action"],
    stages: contract.stage_ids.map((stage_id) => ({
      stage_id,
      status: "complete",
      evidence: [`evidence:${stage_id}`],
      findings: [`finding:${stage_id}`],
      uncertainty: [`uncertainty:${stage_id}`],
      open_questions: [],
    })),
    domain_details: Object.fromEntries(contract.domain_fields.map((field) => [field, [`detail:${field}`]])),
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
}

test("outcome integrity emits a ready, metadata-only reliance contract", () => {
  const result = assessAutonomousOutcomeIntegrity({
    run: run(),
    claims: [claim()],
    evidence: [evidence()],
    claimBindings: [binding()],
    referenceTime: REFERENCE,
    policy: new AutonomousClaimIntegrityPolicy({ minSupport: 0.5 }),
  });
  assert.equal(result.status, "ready");
  assert.deepEqual(result.gate_reasons, []);
  assert.deepEqual(result.next_actions, []);
  assert.equal(result.claim_count, 1);
  assert.equal(result.evidence_count, 1);
  assert.equal(result.run.output_digest, digest("answer"));
  assert.equal(result.secret_material, "never_returned");
  assert.equal(JSON.stringify(result).includes("outcome-task"), false);
  assert.equal(validateAutonomousOutcomeIntegrity(result), result);
  assert.equal(validateAutonomousOutcomeIntegritySnapshot(result).assessment_digest, result.assessment_digest);
});

test("outcome integrity covers every built-in domain with deterministic ordering", () => {
  const claims = AUTONOMOUS_DOMAIN_NAMES.map((domain) => new AutonomousClaimIntegrityClaim({ claimId: `claim-${domain}`, domain, claimDigest: digest(`claim:${domain}`) }));
  const evidenceRows = AUTONOMOUS_DOMAIN_NAMES.map((domain) => new AutonomousClaimIntegrityEvidence({
    evidenceId: `evidence-${domain}`,
    domain,
    claimIds: [`claim-${domain}`],
    sourceId: `source-${domain}`,
    sourceDigest: digest(`source:${domain}`),
    evidenceDigest: digest(`evidence:${domain}`),
    observedAt: "2026-08-25T12:00:00Z",
    reliability: 0.9,
    support: 0.9,
    status: "accepted",
    stance: "support",
    modality: "primary",
    reproducibility: "reproduced",
  }));
  const projectedRun = run({ domains: [...AUTONOMOUS_DOMAIN_NAMES], mode: "cross_domain" });
  const bindings = AUTONOMOUS_DOMAIN_NAMES.map((domain) => binding({ claim_id: `claim-${domain}`, domain, role: domain === "cross_domain" ? "synthesis_response" : "specialist_response" }));
  const result = assessAutonomousOutcomeIntegrity({ run: projectedRun, claims, evidence: evidenceRows, claimBindings: bindings, referenceTime: REFERENCE, policy: { minSupport: 0.5 } });
  assert.equal(result.status, "ready");
  assert.equal(result.claim_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(result.run.domains, [...AUTONOMOUS_DOMAIN_NAMES]);
});

test("outcome integrity blocks incomplete runs and missing exact bindings", () => {
  const result = assessAutonomousOutcomeIntegrity({
    run: run({ status: "approval_required" }),
    claims: [claim()],
    evidence: [evidence()],
    claimBindings: [],
    referenceTime: REFERENCE,
  });
  assert.equal(result.status, "blocked");
  assert.ok(result.gate_reasons.includes("run_not_completed"));
  assert.ok(result.gate_reasons.includes("claim_bindings_incomplete"));
  assert.ok(result.next_actions.includes("inspect_incomplete_run"));
  assert.ok(result.next_actions.includes("rebind_claims_to_exact_run_output"));
});

test("outcome integrity requires cross-domain synthesis alignment when requested", () => {
  const crossRun = run({ mode: "cross_domain", domains: ["science", "data", "cross_domain"] });
  const result = assessAutonomousOutcomeIntegrity({
    run: crossRun,
    claims: [claim()],
    evidence: [evidence()],
    claimBindings: [binding()],
    referenceTime: REFERENCE,
    requireResponseAssessment: true,
    requireSynthesis: true,
  });
  assert.equal(result.status, "blocked");
  assert.ok(result.gate_reasons.includes("response_assessment_missing"));
  assert.ok(result.gate_reasons.includes("synthesis_not_completed"));
});

test("outcome integrity rejects output drift and tampered sealed metadata", () => {
  const exactRun = run();
  assert.throws(() => bindAutonomousOutcomeIntegrityClaims(exactRun, [binding({ output_digest: digest("other-answer") })]));
  const result = assessAutonomousOutcomeIntegrity({ run: exactRun, claims: [claim()], evidence: [evidence()], claimBindings: [binding()], referenceTime: REFERENCE });
  const tampered = { ...result, claim_count: 99 };
  assert.throws(() => validateAutonomousOutcomeIntegritySnapshot(tampered));
});

test("the autonomous brain facade projects and gates a transient direct result without provider dispatch", () => {
  const raw = {
    status: "completed",
    route: { task_digest: digest("facade-task"), route_digest: digest("facade-route") },
    blueprint: { domain_profile: { domain: "science" } },
    response: { text: "answer", structured: null },
  };
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime()) });
  const projected = brain.projectOutcomeIntegrityRun(raw);
  const claimBindings = brain.bindOutcomeIntegrityClaims(raw, [binding({ output_digest: projected.output_digest, response_digest: projected.response_digest })]);
  const result = brain.assessOutcomeIntegrity(raw, {
    claims: [claim()],
    evidence: [evidence()],
    claimBindings,
    referenceTime: REFERENCE,
  });
  assert.equal(result.status, "ready");
  assert.equal(result.run.task_digest, digest("facade-task"));
  assert.equal(brain.validateOutcomeIntegrity(result).assessment_digest, result.assessment_digest);
  assert.equal(brain.validateOutcomeIntegritySnapshot(result).assessment_digest, result.assessment_digest);
});

test("the autonomous brain facade gates and replays cross-domain responses without retaining values", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const selected = profiles.filter((profile) => profile.domain === "coding" || profile.domain === "science");
  const contracts = new Map(await Promise.all(selected.map(async (profile) => [profile.domain, await buildAutonomousDomainResponseContract(profile)])));
  const entries = selected.map((profile) => {
    const contract = contracts.get(profile.domain);
    return { domain: profile.domain, contract, response: responseFor(contract), role: "specialist" };
  });
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(new LLMRuntime()) });
  const assessment = brain.assessCrossDomainResponses(entries, {
    task: "private cross-domain response context must remain transient",
    requestedDomains: ["coding", "science"],
    requireCompleteAlignment: false,
  });
  assert.equal(assessment.status, "ready_to_synthesize");
  assert.equal(assessment.ready_to_synthesize, true);
  assert.equal(assessment.rows.length, 2);
  assert.doesNotMatch(JSON.stringify(assessment), /private cross-domain response context/);
  assert.equal(brain.validateCrossDomainResponseAssessment(assessment).assessment_digest, assessment.assessment_digest);
  assert.equal(brain.replayCrossDomainResponseAssessment(entries, assessment, {
    requestedDomains: ["coding", "science"],
    requireCompleteAlignment: false,
    contextDigest: assessment.context_digest,
  }).assessment_digest, assessment.assessment_digest);
  const tampered = structuredClone(assessment);
  tampered.status = "completed";
  assert.throws(() => brain.validateCrossDomainResponseAssessment(tampered), /digest/);
});
