import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousClaimIntegrityClaim,
  AutonomousClaimIntegrityEvidence,
  AutonomousClaimIntegrityPolicy,
  LLMRuntime,
  assessAutonomousOutcomeIntegrity,
  bindAutonomousOutcomeIntegrityClaims,
  digestJsonSync,
  projectAutonomousOutcomeIntegrityRun,
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

test("the autonomous facade projects a transient direct result without provider dispatch", () => {
  const raw = {
    status: "completed",
    route: { task_digest: digest("facade-task"), route_digest: digest("facade-route") },
    blueprint: { domain_profile: { domain: "science" } },
    response: { text: "answer", structured: null },
  };
  const projected = projectAutonomousOutcomeIntegrityRun(raw);
  const result = new AutonomousAgent(new LLMRuntime()).assessOutcomeIntegrity(raw, {
    claims: [claim()],
    evidence: [evidence()],
    claimBindings: [binding({ output_digest: projected.output_digest, response_digest: projected.response_digest })],
    referenceTime: REFERENCE,
  });
  assert.equal(result.status, "ready");
  assert.equal(result.run.task_digest, digest("facade-task"));
});
