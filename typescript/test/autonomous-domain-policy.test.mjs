import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  LLMRuntime,
  AUTONOMOUS_DOMAIN_NAMES,
  autonomousDomainPolicy,
  builtinAutonomousDomainPolicies,
  evaluateAutonomousDomainPolicy,
} from "../dist/index.js";

test("domain policies cover every domain with bounded, digest-addressed defaults", () => {
  const policies = builtinAutonomousDomainPolicies();
  assert.deepEqual(policies.map((policy) => policy.domain), AUTONOMOUS_DOMAIN_NAMES);
  assert.equal(new Set(policies.map((policy) => policy.policy_digest)).size, policies.length);
  for (const policy of policies) {
    assert.equal(policy.schema, "bioprism-autonomous-domain-policy/0.1");
    assert.ok(policy.max_input_tokens > 0);
    assert.ok(policy.max_output_tokens > 0);
    assert.match(policy.policy_digest, /^[0-9a-f]{64}$/);
    assert.equal(autonomousDomainPolicy(policy.domain).policy_digest, policy.policy_digest);
  }
});

test("policy admission distinguishes complete review from hard budget and safety blocks", () => {
  const coding = autonomousDomainPolicy("coding");
  const admitted = evaluateAutonomousDomainPolicy(coding, {
    route_confidence: 1,
    selection_confidence: 1,
    selection_margin: 1,
    estimated_input_tokens: 100,
    requested_output_tokens: 100,
    estimated_cost_units: 1,
    structured_response: true,
    evidence_ready: true,
    evaluator_configured: true,
    plan_accepted: true,
    effects_requested: true,
    effects_approved: true,
  });
  assert.equal(admitted.decision, "admitted");
  const review = evaluateAutonomousDomainPolicy(coding, { route_confidence: 0.1 });
  assert.equal(review.decision, "review_required");
  assert.ok(review.reasons.includes("selection_confidence_below_policy_floor") === false);
  const biomedical = evaluateAutonomousDomainPolicy(autonomousDomainPolicy("biomedical"), {
    route_confidence: 1,
    selection_confidence: 1,
    selection_margin: 1,
    structured_response: true,
    evidence_ready: true,
    evaluator_configured: true,
    plan_accepted: true,
    effects_requested: true,
  });
  assert.equal(biomedical.decision, "blocked");
  assert.ok(biomedical.reasons.includes("effects_forbidden_by_policy"));
  const overBudget = evaluateAutonomousDomainPolicy(coding, { estimated_input_tokens: coding.max_input_tokens + 1 });
  assert.equal(overBudget.decision, "blocked");
  assert.ok(overBudget.reasons.includes("input_budget_exceeded"));
});

test("every generated blueprint binds the same domain policy into its plan", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("policy test must not contact a provider"); } });
  const agent = new AutonomousAgent(runtime);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const envelope = await agent.blueprint(`prepare a bounded ${domain} review`, { domain });
    assert.ok(envelope.blueprint);
    assert.equal(envelope.blueprint.domain_policy.domain, domain);
    assert.equal(envelope.blueprint.domain_policy.policy_digest, envelope.blueprint.plan.domain_policy_digest);
  }
});
