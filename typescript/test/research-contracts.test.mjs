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
