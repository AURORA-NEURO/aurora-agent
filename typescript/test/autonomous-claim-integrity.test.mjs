import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousClaimIntegrityClaim,
  AutonomousClaimIntegrityEvidence,
  AutonomousClaimIntegrityPolicy,
  AutonomousInformationAcquisitionCandidate,
  LLMRuntime,
  assessAutonomousClaimIntegrity,
  bindAutonomousClaimIntegrityAcquisitionRequests,
  digestJsonSync,
  planAutonomousClaimIntegrityAcquisition,
  reassessAutonomousClaimIntegrity,
  validateAutonomousClaimIntegrity,
  validateAutonomousClaimIntegritySnapshot,
  validateAutonomousClaimIntegrityAcquisitionBridge,
  validateAutonomousClaimIntegrityAcquisitionBinding,
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

test("integrity evidence quality and posture must be explicit", () => {
  const complete = {
    evidence_id: "explicit-evidence",
    domain: "science",
    claim_ids: ["explicit-claim"],
    source_id: "explicit-source",
    evidence_digest: digest("explicit-evidence"),
    observed_at: "2026-08-25T12:00:00Z",
    reliability: 0.9,
    support: 0.9,
    status: "accepted",
    stance: "support",
  };
  for (const missingField of ["reliability", "support", "status", "stance"]) {
    const incomplete = { ...complete };
    delete incomplete[missingField];
    assert.throws(() => assessAutonomousClaimIntegrity({
      contextDigest: digest("explicit-task"),
      claims: [claim("explicit-claim", "science")],
      evidence: [incomplete],
      referenceTime: REFERENCE,
    }));
  }
});

test("integrity rejects replayed evidence but accepts distinct independent observations", () => {
  const repeatedDigest = digest("same-observation");
  assert.throws(() => assessAutonomousClaimIntegrity({
    contextDigest: digest("replayed-task"),
    claims: [claim("replayed-claim", "science")],
    evidence: [
      evidence("replay-a", "replayed-claim", "science", { source: "source-a", evidenceDigest: repeatedDigest }),
      evidence("replay-b", "replayed-claim", "science", { source: "source-b", evidenceDigest: repeatedDigest }),
    ],
    referenceTime: REFERENCE,
  }), /duplicate evidence digests/);

  const result = assessAutonomousClaimIntegrity({
    contextDigest: digest("independent-task"),
    claims: [claim("independent-claim", "science", { requiredSupport: 0.8, requiredIndependentSources: 2 })],
    evidence: [
      evidence("independent-a", "independent-claim", "science", { source: "source-a", reliability: 0.8, support: 0.5 }),
      evidence("independent-b", "independent-claim", "science", { source: "source-b", reliability: 0.8, support: 0.5 }),
    ],
    referenceTime: REFERENCE,
  });
  assert.equal(result.claims[0].status, "supported");
  assert.equal(result.claims[0].independent_source_count, 2);
  assert.equal(result.claims[0].support_score, 0.8);
});

test("agent facade binds task digest without provider or source dispatch", () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const result = agent.assessClaimIntegrity("decide whether a bounded science claim may be used", { claims: [claim("science-claim", "science")], evidence: [evidence("science-evidence", "science-claim", "science")], referenceTime: REFERENCE });
  assert.equal(result.status, "ready");
  assert.equal(result.toJSON().context_digest, digestJsonSync({ task: "decide whether a bounded science claim may be used" }));
  assert.equal(JSON.stringify(result.toJSON()).includes("decide whether"), false);
});

test("integrity actions drive the reviewed acquisition queue", () => {
  const assessment = assessAutonomousClaimIntegrity({ contextDigest: digest("acquisition-task"), claims: [claim("blocked-claim", "science")], evidence: [], referenceTime: REFERENCE });
  const scienceCandidate = new AutonomousInformationAcquisitionCandidate({ candidateId: "science-next", domain: "science", capability: "evidence_acquisition", sourceId: "science-source", informationGain: 0.4, uncertaintyReduction: 0.4, reliability: 0.9, freshness: 0.9, coverage: 0.5, cost: 0.1, latencyMs: 100, risk: 0.05, conflictRisk: 0.05, metadata: { claim_ids: ["blocked-claim"] } });
  const unrelatedCandidate = new AutonomousInformationAcquisitionCandidate({ candidateId: "coding-next", domain: "coding", capability: "evidence_acquisition", sourceId: "coding-source", informationGain: 1, uncertaintyReduction: 1, reliability: 0.9, freshness: 0.9, coverage: 1, cost: 0.1, latencyMs: 100, risk: 0.01, conflictRisk: 0.01 });
  const bridge = planAutonomousClaimIntegrityAcquisition(assessment, { candidates: [unrelatedCandidate, scienceCandidate], policy: { maxItems: 1, exploration: 0 } });
  assert.equal(bridge.status, "planned");
  assert.deepEqual(bridge.targetedCandidateIds, ["science-next"]);
  assert.equal(bridge.unmatchedActionCount, 0);
  assert.equal(bridge.acquisitionPlan.selected[0].candidate_id, "science-next");
  assert.equal(bridge.candidateActionMatches[0].match_strength, "claim_and_capability");
  assert.equal(validateAutonomousClaimIntegrityAcquisitionBridge(bridge), bridge);
  assert.equal(bridge.toJSON().secret_material, "never_returned");
});

test("integrity bridge is explicitly empty or blocked without dispatch", () => {
  const ready = assessAutonomousClaimIntegrity({ contextDigest: digest("ready-task"), claims: [claim("ready-claim", "science")], evidence: [evidence("ready-evidence", "ready-claim", "science")], referenceTime: REFERENCE });
  const empty = planAutonomousClaimIntegrityAcquisition(ready, { candidates: [] });
  assert.equal(empty.status, "no_action_required");
  assert.equal(empty.acquisitionPlan, null);
  const blocked = assessAutonomousClaimIntegrity({ contextDigest: digest("no-candidates-task"), claims: [claim("missing-claim", "science")], evidence: [], referenceTime: REFERENCE });
  const noQueue = planAutonomousClaimIntegrityAcquisition(blocked, { candidates: [] });
  assert.equal(noQueue.status, "blocked");
  assert.equal(noQueue.unmatchedActionCount, blocked.actions.length);
});

test("integrity binding is ordered, digest-bound, and source-exact", () => {
  const assessment = assessAutonomousClaimIntegrity({ contextDigest: digest("binding-task"), claims: [claim("binding-claim", "science")], evidence: [], referenceTime: REFERENCE });
  const candidate = new AutonomousInformationAcquisitionCandidate({ candidateId: "binding-candidate", domain: "science", capability: "evidence_acquisition", sourceId: "science-source", informationGain: 0.8, uncertaintyReduction: 0.8, reliability: 0.9, freshness: 0.9, coverage: 0.9, cost: 0.1, latencyMs: 100, risk: 0.01, conflictRisk: 0.01, metadata: { claim_ids: ["binding-claim"] } });
  const bridge = planAutonomousClaimIntegrityAcquisition(assessment, { candidates: [candidate], policy: { maxItems: 1, exploration: 0 } });
  const binding = bindAutonomousClaimIntegrityAcquisitionRequests(bridge, [{ candidate_id: "binding-candidate", requirement_id: "science-evidence", source_id: "science-source", metadata: { caller_locator: "caller-owned" } }]);
  assert.deepEqual(binding.candidateIds, ["binding-candidate"]);
  assert.deepEqual(binding.domains, ["science"]);
  assert.equal(binding.requests[0].metadata.claim_integrity_bridge_digest, bridge.bridgeDigest);
  assert.equal(binding.toJSON().request_count, 1);
  assert.equal(JSON.stringify(binding.toJSON()).includes("caller-owned"), false);
  assert.equal(validateAutonomousClaimIntegrityAcquisitionBinding(binding), binding);
  assert.throws(() => bindAutonomousClaimIntegrityAcquisitionRequests(bridge, [{ candidate_id: "binding-candidate", requirement_id: "science-evidence", source_id: "wrong-source" }]));
  assert.throws(() => bindAutonomousClaimIntegrityAcquisitionRequests(bridge, [{ candidate_id: "binding-candidate", requirement_id: "science-evidence", source_id: "science-source", metadata: { claim_integrity_bridge_digest: "tampered" } }]));
});
