import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousClaimIntegrityClaim,
  AutonomousClaimIntegrityEvidence,
  AutonomousClaimIntegrityPolicy,
  LLMRuntime,
  assessAutonomousClaimIntegrity,
  digestJsonSync,
  reassessAutonomousClaimIntegrity,
  validateAutonomousClaimIntegrity,
  validateAutonomousClaimIntegritySnapshot,
} from "../dist/index.js";

const REFERENCE = "2026-08-26T12:00:00Z";
const digest = (value) => digestJsonSync({ value });
function claim(claimId, domain, overrides = {}) {
  return new AutonomousClaimIntegrityClaim({ claimId, domain, claimDigest: digest(`claim:${claimId}`), ...overrides });
}
function evidence(evidenceId, claimId, domain, { source, observedAt = "2026-08-25T12:00:00Z", reliability = 0.9, support = 0.9, stance = "support", modality = "primary", reproducibility = "reproduced", status = "accepted", ...overrides } = {}) {
  return new AutonomousClaimIntegrityEvidence({ evidenceId, domain, claimIds: [claimId], sourceId: source ?? `source-${evidenceId}`, sourceDigest: digest(`source:${source ?? evidenceId}`), evidenceDigest: digest(`evidence:${evidenceId}`), observedAt, reliability, support, stance, modality, reproducibility, status, ...overrides });
}

test("integrity fuses all domains without retaining claim text", () => {
  const claims = AUTONOMOUS_DOMAIN_NAMES.map((domain) => claim(`claim-${domain}`, domain));
  const evidenceRows = AUTONOMOUS_DOMAIN_NAMES.map((domain) => evidence(`evidence-${domain}`, `claim-${domain}`, domain));
  const result = assessAutonomousClaimIntegrity({ contextDigest: digest("all-domain-task"), claims, evidence: evidenceRows, referenceTime: REFERENCE, policy: new AutonomousClaimIntegrityPolicy({ minSupport: 0.5 }) });
  assert.equal(result.status, "ready");
  assert.equal(result.ready, true);
  assert.equal(result.summary.supported_claim_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.actions.length, 0);
  const projection = result.toJSON();
  assert.match(projection.execution, /^provider_free/);
  assert.equal(projection.secret_material, "never_returned");
  assert.equal(JSON.stringify(projection).includes("all-domain-task"), false);
});

test("integrity makes temporal, conflict, independence, modality, and reproduction actions explicit", () => {
  const claims = [
    claim("conflict", "science"),
    claim("stale", "coding"),
    claim("independent", "data", { requiredIndependentSources: 2 }),
    claim("modal", "multimodal", { requiredModalities: ["imaging", "omics"] }),
    claim("repro", "evaluation", { requiredReproducibility: true }),
  ];
  const evidenceRows = [
    evidence("conflict-support", "conflict", "science", { source: "source-a" }),
    evidence("conflict-contradiction", "conflict", "science", { source: "source-b", stance: "contradict" }),
    evidence("stale-evidence", "stale", "coding", { observedAt: "2026-01-01T12:00:00Z" }),
    evidence("one-source", "independent", "data", { source: "single-source" }),
    evidence("imaging-only", "modal", "multimodal", { modality: "imaging" }),
    evidence("observed-only", "repro", "evaluation", { reproducibility: "observed" }),
  ];
  const result = assessAutonomousClaimIntegrity({ contextDigest: digest("blocked-task"), claims, evidence: evidenceRows, referenceTime: REFERENCE, policy: { maxActions: 10 } });
  const byId = new Map(result.claims.map((item) => [item.claim_id, item]));
  assert.equal(byId.get("conflict").status, "conflicted");
  assert.equal(byId.get("conflict").next_action_type, "resolve_contradiction");
  assert.equal(byId.get("stale").status, "stale");
  assert.equal(byId.get("stale").next_action_type, "acquire_fresh_evidence");
  assert.equal(byId.get("independent").status, "insufficient_independence");
  assert.equal(byId.get("modal").status, "insufficient_modalities");
  assert.equal(byId.get("repro").status, "unreproducible");
  assert.equal(result.status, "blocked");
  assert.deepEqual(new Set(result.actions.map((item) => item.actionType)), new Set(["resolve_contradiction", "acquire_fresh_evidence", "acquire_independent_source", "acquire_cross_modal_evidence", "reproduce_evidence"]));
});

test("integrity reassessment is generation and digest fenced", () => {
  const initialClaim = claim("recover", "science");
  const first = assessAutonomousClaimIntegrity({ contextDigest: digest("recover-task"), claims: [initialClaim], evidence: [], referenceTime: REFERENCE });
  assert.equal(first.status, "blocked");
  const second = reassessAutonomousClaimIntegrity({ previous: first, claims: [initialClaim], evidence: [evidence("recovered", "recover", "science")], referenceTime: REFERENCE });
  assert.equal(second.generation, 2);
  assert.equal(second.priorAssessmentDigest, first.assessmentDigest);
  assert.equal(second.claims[0].status, "supported");
  assert.equal(validateAutonomousClaimIntegrity(second), second);
  assert.equal(validateAutonomousClaimIntegritySnapshot(second.toJSON()).assessment_digest, second.assessmentDigest);
  const wire = second.toJSON();
  wire.summary = { tampered: true };
  assert.throws(() => validateAutonomousClaimIntegritySnapshot(wire));
});

test("integrity temporal firewall and secret metadata fail closed", () => {
  assert.throws(() => claim("secret", "coding", { metadata: { api_key: "never accepted" } }));
  const future = evidence("future", "future-claim", "coding", { observedAt: "2027-01-01T00:00:00Z" });
  const expired = evidence("expired", "expired-claim", "science", { validUntil: "2026-08-01T00:00:00Z" });
  const result = assessAutonomousClaimIntegrity({ contextDigest: digest("temporal-task"), claims: [claim("future-claim", "coding"), claim("expired-claim", "science")], evidence: [future, expired], referenceTime: REFERENCE });
  const byId = new Map(result.evidence.map((item) => [item.evidence_id, item]));
  assert.equal(byId.get("future").temporal_state, "future");
  assert.equal(byId.get("future").usable, false);
  assert.equal(byId.get("expired").temporal_state, "expired");
  assert.ok(result.claims.every((item) => ["blocked", "missing"].includes(item.status)));
});

test("agent facade binds task digest without provider or source dispatch", () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const result = agent.assessClaimIntegrity("decide whether a bounded science claim may be used", { claims: [claim("science-claim", "science")], evidence: [evidence("science-evidence", "science-claim", "science")], referenceTime: REFERENCE });
  assert.equal(result.status, "ready");
  assert.equal(result.toJSON().context_digest, digestJsonSync({ task: "decide whether a bounded science claim may be used" }));
  assert.equal(JSON.stringify(result.toJSON()).includes("decide whether"), false);
});
