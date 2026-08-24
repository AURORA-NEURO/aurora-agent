import test from "node:test";
import assert from "node:assert/strict";
import {
  PRECLINICAL_BOUNDARY,
  validateEvidenceReceipt,
  validatePolicyReceipt,
  researchArtifactDigest,
  RELEASE_REVIEW_FEATURE_ID,
  releaseReviewDigest,
  validateReleaseReview,
  RESEARCH_INGESTION_FEATURE_ID,
  researchIngestionBundleDigest,
  validateResearchIngestionBundle,
  EXPERIMENT_DESIGN_FEATURE_ID,
  experimentDesignPlanDigest,
  validateExperimentDesignPlan,
} from "../dist/index.js";

test("empty evidence is explicit unknown", () => {
  assert.doesNotThrow(() => validateEvidenceReceipt({
    schema_version: "aurora-research-contract/1.0",
    receipt_id: "evidence:q1",
    intent: "retrieve",
    sources: [],
    derivation: ["feature:AFA-bioir-P02-F01"],
    uncertainty: [{ kind: "epistemic", statement: "no evidence" }],
    omissions: [{ item: "query:q1", reason: "empty", could_change_decision: "unknown" }],
    competing_explanations: [],
    negative_evidence: [],
    conclusion_state: "unknown",
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("unresolved policy cannot allow", () => {
  assert.throws(() => validatePolicyReceipt({
    schema_version: "aurora-research-contract/1.0",
    receipt_id: "policy:q1",
    decision: "allow",
    reasons: ["unresolved"],
    evaluated_artifacts: [],
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("artifact digest is key-order stable", () => {
  assert.equal(researchArtifactDigest({ b: 2, a: 1 }), researchArtifactDigest({ a: 1, b: 2 }));
});

test("a passing release review requires provenance", () => {
  assert.throws(() => validateReleaseReview({
    schema_version: "aurora-research-contract/1.0",
    feature_id: RELEASE_REVIEW_FEATURE_ID,
    capability_id: "capability:demo",
    card_digest: "a".repeat(64),
    verdict: "pass",
    reasons: ["all gates passed"],
    replications: [],
    checks: [],
    provenance_complete: false,
    boundary: PRECLINICAL_BOUNDARY,
  }));
});

test("release review digest is deterministic", () => {
  const review = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RELEASE_REVIEW_FEATURE_ID,
    capability_id: "capability:demo",
    card_digest: "a".repeat(64),
    verdict: "blocked",
    reasons: ["replication floor unmet"],
    replications: [],
    checks: [],
    provenance_complete: false,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.equal(releaseReviewDigest(review), releaseReviewDigest(review));
});

test("research ingestion bundle keeps raw data local", () => {
  const bundle = {
    schema_version: "aurora-research-contract/1.0",
    feature_id: RESEARCH_INGESTION_FEATURE_ID,
    source_id: "study-a",
    adapter: "tabular",
    adapter_version: "0.1.0",
    source_digest: "a".repeat(64),
    ingestion_digest: "b".repeat(64),
    artifact: { content_hash: "b".repeat(64) },
    conformance: { verified: true },
    raw_data_local: true,
    boundary: PRECLINICAL_BOUNDARY,
  };
  assert.doesNotThrow(() => validateResearchIngestionBundle(bundle));
  assert.equal(researchIngestionBundleDigest(bundle), researchIngestionBundleDigest(bundle));
});

test("experiment design plan preserves allocation total", () => {
  const plan = {
    payload: {
      schema_version: "aurora-research-contract/1.0",
      feature_id: EXPERIMENT_DESIGN_FEATURE_ID,
      boundary: PRECLINICAL_BOUNDARY,
      allocations: [{ arm_id: "control", units: 4 }, { arm_id: "treatment", units: 4 }],
      total_units: 8,
    },
    artifact: { content_hash: "c".repeat(64) },
  };
  assert.doesNotThrow(() => validateExperimentDesignPlan(plan));
  assert.equal(experimentDesignPlanDigest(plan), experimentDesignPlanDigest(plan));
});
