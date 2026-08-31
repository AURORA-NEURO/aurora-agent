import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  LLMRuntime,
  AUTONOMOUS_DOMAIN_NAMES,
  autonomousDomainPolicy,
  builtinAutonomousDomainPolicies,
  evaluateAutonomousDomainPolicy,
  validateAutonomousDomainPolicy,
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

test("policy replay validation rejects tampering and cross-domain binding", () => {
  const policy = autonomousDomainPolicy("coding");
  assert.equal(validateAutonomousDomainPolicy({ ...policy }).policy_digest, policy.policy_digest);
  assert.throws(() => validateAutonomousDomainPolicy({ ...policy, max_tool_turns: policy.max_tool_turns + 1 }), /digest/);
  const missing = { ...policy };
  delete missing.policy_id;
  missing.unexpected = true;
  assert.throws(() => validateAutonomousDomainPolicy(missing), /missing or unsupported/);
  assert.throws(() => validateAutonomousDomainPolicy(policy, "science"), /expected domain/);
  assert.throws(() => validateAutonomousDomainPolicy({ ...policy, retention: "raw_policy" }), /markers/);
});

test("agent exposes policy replay validation", () => {
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("policy validation must not contact a provider"); } }));
  const policy = autonomousDomainPolicy("science");
  assert.deepEqual(agent.validateDomainPolicy({ ...policy }, "science"), policy);
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

test("strict policy mode blocks every domain before provider dispatch until all gates are explicit", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("strict policy test must not contact HTTP"); } });
  runtime.registerInMemoryProvider("local", async () => {
    providerCalls += 1;
    return { output_text: "unexpected provider dispatch" };
  });
  const agent = new AutonomousAgent(runtime);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    agent.registerModel({
      provider: "local",
      model: `local-${domain}`,
      capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
      context_window_tokens: 32_000,
      max_output_tokens: 4_000,
      quality: 0.95,
      latency_ms: 10,
      cost_per_million_tokens: 0,
      reliability: 0.99,
    });
    const result = await agent.run(`strictly review a bounded ${domain} task`, {
      domain,
      candidates: agent.models().filter((candidate) => candidate.model === `local-${domain}`),
      approveProviderCall: true,
      domainPolicyMode: "strict",
    });
    assert.equal(result.status, "policy_review_required");
    assert.equal(result.domain_policy_admission?.domain, domain);
    assert.ok(result.domain_policy_admission?.reasons.includes("structured_response_required"));
    assert.ok(result.domain_policy_admission?.reasons.includes("plan_acceptance_required"));
  }
  assert.equal(providerCalls, 0);
});

test("strict policy mode admits an explicitly reviewed, evidence-backed, evaluator-bound plan", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("strict admission test must not contact HTTP"); } });
  const agent = new AutonomousAgent(runtime);
  const result = await agent.run("prepare a reviewed coding plan", {
    domain: "coding",
    domainPolicyMode: "strict",
    domainPolicyEvidenceReady: true,
    domainPolicyEvaluatorConfigured: true,
    domainPolicyPlanAccepted: true,
    structuredDomainResponse: true,
    approveProviderCall: false,
  });
  assert.equal(result.status, "approval_required");
  assert.equal(result.domain_policy_admission?.decision, "admitted");
  assert.equal(result.domain_policy_admission?.domain, "coding");
});

test("strict policy mode gates provider-assisted planning across every domain", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("strict planning test must not contact HTTP"); } });
  runtime.registerInMemoryProvider("planner", async () => {
    providerCalls += 1;
    return { output_text: "unexpected planner dispatch" };
  });
  const agent = new AutonomousAgent(runtime);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const envelope = await agent.blueprint(`prepare a bounded ${domain} plan`, { domain });
    assert.ok(envelope.blueprint, domain);
    const held = await agent.planWithProvider(envelope.blueprint, {
      domainPolicyMode: "strict",
      approveProviderCall: true,
    });
    assert.equal(held.status, "policy_review_required", domain);
    assert.equal(held.domain_policy_admission?.domain, domain);
    assert.ok(held.domain_policy_admission?.reasons.includes("evaluator_required"), domain);

    const admitted = await agent.planWithProvider(envelope.blueprint, {
      domainPolicyMode: "strict",
      domainPolicyEvidenceReady: true,
      domainPolicyEvaluatorConfigured: true,
      approveProviderCall: false,
    });
    assert.equal(admitted.status, "approval_required", domain);
    assert.equal(admitted.domain_policy_admission?.decision, "admitted", domain);
  }
  assert.equal(providerCalls, 0);
});

test("strict planAndRun and cross-domain planning share the pre-dispatch policy gate", async () => {
  let providerCalls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("strict planAndRun test must not contact HTTP"); } });
  runtime.registerInMemoryProvider("planner", async () => {
    providerCalls += 1;
    return { output_text: "unexpected planner dispatch" };
  });
  const agent = new AutonomousAgent(runtime);

  const single = await agent.planAndRun("prepare a bounded coding plan", {
    domain: "coding",
    planning: { approveProviderCall: true },
    domainPolicyMode: "strict",
    approveProviderCall: true,
  });
  assert.equal(single.status, "policy_review_required");
  assert.equal(single.plan_refinement?.domain_policy_admission?.domain, "coding");

  const cross = await agent.blueprint("Research a biomedical neuroscience experiment with EEG patient evidence.", {
    allowCrossDomain: true,
  });
  assert.ok(cross.cross_domain_blueprint);
  const crossPlan = await agent.planCrossDomainWithProvider(cross.cross_domain_blueprint, {
    domainPolicyMode: "strict",
    approveProviderCall: true,
  });
  assert.equal(crossPlan.status, "policy_review_required");
  assert.equal(crossPlan.domain_policy_admission?.domain, "cross_domain");
  assert.equal(providerCalls, 0);
});
