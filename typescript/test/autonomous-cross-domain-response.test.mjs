import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA,
  AUTONOMOUS_DOMAIN_NAMES,
  ArgumentError,
  assessAutonomousCrossDomainResponseSet,
  buildAutonomousDomainResponseContract,
  builtinAutonomousDomainProfiles,
  evaluateAutonomousDomainResponse,
  replayAutonomousCrossDomainResponseAssessment,
  validateAutonomousCrossDomainResponseAssessment,
} from "../dist/index.js";

function responseFor(contract, complete = true) {
  return {
    schema: "bioprism-typescript-autonomous-domain-response/0.1",
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: complete ? "complete" : "partial",
    answer: complete ? `Bounded answer for ${contract.domain}.` : "Partial answer.",
    observations: complete ? ["bounded observation"] : [],
    inferences: complete ? ["bounded inference"] : [],
    uncertainty: complete ? ["bounded uncertainty"] : [],
    evidence_gaps: complete ? ["bounded evidence gap"] : [],
    next_actions: complete ? ["review the next action"] : [],
    stages: contract.stage_ids.map((stage_id) => ({
      stage_id,
      status: complete ? "complete" : "not_attempted",
      evidence: complete ? [`evidence:${stage_id}`] : [],
      findings: complete ? [`finding:${stage_id}`] : [],
      uncertainty: complete ? [`uncertainty:${stage_id}`] : [],
      open_questions: [],
    })),
    domain_details: Object.fromEntries(contract.domain_fields.map((field) => [field, complete ? [`detail:${field}`] : []])),
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
}

async function fixtureEntries({ complete = true, includeSynthesis = false } = {}) {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  const domains = AUTONOMOUS_DOMAIN_NAMES.filter((domain) => domain !== "cross_domain");
  const entries = domains.map((domain) => ({ domain, contract: contracts.get(domain), response: responseFor(contracts.get(domain), complete), role: "specialist" }));
  if (includeSynthesis) entries.push({ domain: "cross_domain", contract: contracts.get("cross_domain"), response: responseFor(contracts.get("cross_domain")), role: "synthesis" });
  return { entries, contracts };
}

function alignmentsFor(entries, contracts, stance = "support") {
  const specialists = entries.filter((entry) => entry.domain !== "cross_domain");
  const digests = new Map(specialists.map((entry) => [entry.domain, evaluateAutonomousDomainResponse(entry.response, contracts.get(entry.domain)).response_digest]));
  const alignments = [];
  let index = 0;
  for (let leftIndex = 0; leftIndex < specialists.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < specialists.length; rightIndex += 1) {
      const left = specialists[leftIndex];
      const right = specialists[rightIndex];
      alignments.push({
        alignment_id: `alignment-${String(index).padStart(3, "0")}`,
        left_domain: left.domain,
        right_domain: right.domain,
        stance,
        confidence: 0.95,
        topic_digest: String(index + 1).padStart(64, "0"),
        rationale_digest: String(index + 10_000).padStart(64, "0"),
        left_response_digest: digests.get(left.domain),
        right_response_digest: digests.get(right.domain),
      });
      index += 1;
    }
  }
  return alignments;
}

test("all specialist domains pass a complete pairwise synthesis gate", async () => {
  const { entries, contracts } = await fixtureEntries();
  const domains = entries.map((entry) => entry.domain);
  const alignments = alignmentsFor(entries, contracts);
  const assessment = assessAutonomousCrossDomainResponseSet(entries, {
    requestedDomains: domains,
    contextDigest: "a".repeat(64),
    alignments,
  });
  assert.equal(assessment.schema, AUTONOMOUS_CROSS_DOMAIN_RESPONSE_SCHEMA);
  assert.equal(assessment.status, "ready_to_synthesize");
  assert.equal(assessment.ready_to_synthesize, true);
  assert.equal(assessment.rows.length, domains.length);
  assert.equal(assessment.alignment_pairs_expected, alignments.length);
  assert.equal(assessment.alignment_pairs_observed, alignments.length);
  assert.equal(assessment.alignments[0].schema, "bioprism-typescript-autonomous-cross-domain-response-alignment/0.1");
  assert.equal(JSON.stringify(assessment).includes("Bounded answer"), false);
  assert.equal(validateAutonomousCrossDomainResponseAssessment(assessment).assessment_digest, assessment.assessment_digest);
});

test("a validated cross-domain synthesis row completes and replays the gate", async () => {
  const { entries, contracts } = await fixtureEntries({ includeSynthesis: true });
  const specialists = entries.filter((entry) => entry.domain !== "cross_domain");
  const alignments = alignmentsFor(specialists, contracts);
  const assessment = assessAutonomousCrossDomainResponseSet(entries, {
    requestedDomains: specialists.map((entry) => entry.domain),
    alignments,
    requireSynthesis: true,
  });
  assert.equal(assessment.status, "completed");
  assert.equal(assessment.synthesis_domain_present, true);
  assert.deepEqual(assessment.next_actions, []);
  assert.equal(replayAutonomousCrossDomainResponseAssessment(entries, assessment, {
    requestedDomains: specialists.map((entry) => entry.domain),
    alignments,
    requireSynthesis: true,
  }).assessment_digest, assessment.assessment_digest);
});

test("missing domain coverage, weak responses, and contradiction metadata stop synthesis", async () => {
  const { entries, contracts } = await fixtureEntries();
  const selected = entries.slice(0, 2);
  const contradiction = alignmentsFor(selected, contracts, "contradict")[0];
  contradiction.confidence = 0.99;
  const assessment = assessAutonomousCrossDomainResponseSet(selected, {
    requestedDomains: [selected[0].domain, selected[1].domain, "evaluation"],
    alignments: [contradiction],
  });
  assert.equal(assessment.status, "partial");
  assert.equal(assessment.ready_to_synthesize, false);
  assert.deepEqual(assessment.missing_domains, ["evaluation"]);
  assert.deepEqual(assessment.contradictory_alignment_ids, [contradiction.alignment_id]);
  assert.ok(assessment.next_actions.includes("resolve_cross_domain_contradiction"));

  const lowConfidence = { ...contradiction, alignment_id: "low-confidence-1", stance: "support", confidence: 0.5 };
  const review = assessAutonomousCrossDomainResponseSet(selected, {
    requestedDomains: selected.map((entry) => entry.domain),
    alignments: [lowConfidence],
  });
  assert.equal(review.status, "needs_alignment_review");
  assert.deepEqual(review.low_confidence_alignment_ids, ["low-confidence-1"]);
  assert.ok(review.next_actions.includes("review_low_confidence_cross_domain_alignment"));

  const weak = await fixtureEntries({ complete: false });
  const weakAssessment = assessAutonomousCrossDomainResponseSet(weak.entries.slice(0, 2), {
    requestedDomains: weak.entries.slice(0, 2).map((entry) => entry.domain),
    alignments: [],
  });
  assert.equal(weakAssessment.status, "partial");
  assert.ok(weakAssessment.next_actions.includes("repair_domain_response_integrity"));
});

test("projection tampering and credential-shaped response values fail closed", async () => {
  const { entries, contracts } = await fixtureEntries();
  const selected = entries.slice(0, 2);
  const assessment = assessAutonomousCrossDomainResponseSet(selected, {
    requestedDomains: selected.map((entry) => entry.domain),
    alignments: [],
  });
  const tampered = structuredClone(assessment);
  tampered.status = "ready_to_synthesize";
  assert.throws(() => validateAutonomousCrossDomainResponseAssessment(tampered), /digest/);

  const secret = responseFor(contracts.get(selected[0].domain));
  secret.domain_details[contracts.get(selected[0].domain).domain_fields[0]] = ["gsk_should_never_be_accepted"];
  assert.throws(() => assessAutonomousCrossDomainResponseSet([
    { ...selected[0], response: secret },
    selected[1],
  ], {
    requestedDomains: selected.map((entry) => entry.domain),
  }), (error) => error instanceof ArgumentError && /credential/.test(error.message));
});
