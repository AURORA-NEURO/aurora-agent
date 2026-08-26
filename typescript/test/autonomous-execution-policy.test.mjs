import assert from "node:assert/strict";
import test from "node:test";
import {
  AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS,
  AutonomousBrainFacade,
  AutonomousJointExecutionPolicy,
  validateAutonomousJointExecutionPolicyDecision,
  validateAutonomousJointExecutionPolicyState,
} from "../dist/index.js";

const digest = (character) => character.repeat(64);

function candidate(arm_id, domain, overrides = {}) {
  return {
    arm_id,
    domain,
    path: "provider",
    capabilities: ["reasoning", "structured_output"],
    quality_prior: 0.7,
    reliability: 0.8,
    cost_units: 4,
    latency_ms: 120,
    risk: 0.1,
    structured_output: true,
    effects_supported: true,
    provider: "test-provider",
    model: `test-model-${arm_id}`,
    ...overrides,
  };
}

test("joint execution policy selects across every autonomous domain and settles evaluator credit", () => {
  const policy = new AutonomousJointExecutionPolicy({ exploration: 0.4 });
  const initialState = policy.snapshot();
  const candidates = AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS.map((domain) => candidate(`arm-${domain}`, domain));
  const decision = policy.select({
    context_digest: digest("a"),
    requested_domains: [...AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS],
    required_capabilities: ["reasoning"],
    preferred_capabilities: ["structured_output"],
    structured_output_required: true,
    max_cost_units: 10,
    max_latency_ms: 500,
    max_risk: 0.5,
  }, candidates);

  assert.equal(decision.posture, "selected");
  assert.equal(decision.selected_arm_id, "arm-biomedical");
  assert.equal(decision.rankings.length, AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS.length);
  assert.deepEqual(new Set(decision.rankings.map((row) => row.domain)), new Set(AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS));
  assert.equal(decision.context.context_digest, digest("a"));
  assert.equal(JSON.stringify(decision).includes("prompt text must not be retained"), false);

  const settlementInput = {
    settlement_id: "settle-1",
    arm_id: decision.selected_arm_id,
    decision_digest: decision.decision_digest,
    outcome_digest: digest("b"),
    reward: 0.92,
    passed: true,
    evaluator_id: "domain-evaluator",
    evaluator_version: "2026.08",
  };
  const settlement = policy.settle(decision, settlementInput);
  assert.equal(settlement.idempotent_replay, false);
  assert.equal(settlement.generation, 1);
  assert.equal(policy.snapshot().arms.find((arm) => arm.arm_id === decision.selected_arm_id).pulls, 1);

  const replay = policy.settle(decision, settlementInput);
  assert.equal(replay.idempotent_replay, true);
  assert.equal(replay.next_state_digest, settlement.next_state_digest);
  assert.equal(policy.snapshot().generation, 1);
  assert.throws(() => policy.restore(initialState), /roll back/);

  const restored = validateAutonomousJointExecutionPolicyState(policy.snapshot());
  assert.equal(restored.state_digest, policy.snapshot().state_digest);
  const restoredDecision = validateAutonomousJointExecutionPolicyDecision(JSON.parse(JSON.stringify(decision)));
  assert.equal(restoredDecision.decision_digest, decision.decision_digest);
});

test("joint execution policy applies hard gates before UCB scoring and preserves review posture", () => {
  const policy = new AutonomousJointExecutionPolicy();
  const decision = policy.select({
    requested_domains: ["coding"],
    required_capabilities: ["search"],
    required_path: "evidence_first",
    evidence_required: true,
    structured_output_required: true,
    max_cost_units: 20,
    max_latency_ms: 1_000,
    max_risk: 0.5,
  }, [
    candidate("blocked", "coding", { path: "provider", capabilities: ["reasoning"], evidence_ready: false }),
    candidate("eligible-review", "coding", { path: "evidence_first", capabilities: ["search"], evidence_ready: true, approval_required: true }),
    candidate("wrong-domain", "science", { path: "evidence_first", capabilities: ["search"], evidence_ready: true }),
  ]);

  assert.equal(decision.posture, "review_required");
  assert.equal(decision.selected_arm_id, "eligible-review");
  assert.deepEqual(decision.review_reasons, ["candidate_approval_required"]);
  const blocked = decision.rankings.find((row) => row.arm_id === "blocked");
  assert.equal(blocked.eligible, false);
  assert.ok(blocked.reasons.includes("path_not_requested"));
  assert.ok(blocked.reasons.includes("required_capability_missing"));
  assert.ok(blocked.reasons.includes("evidence_not_ready"));
  assert.equal(decision.rankings.find((row) => row.arm_id === "wrong-domain").eligible, false);
});

test("joint execution policy refuses impossible work and rejects forged evaluator state", () => {
  const policy = new AutonomousJointExecutionPolicy();
  const refused = policy.select({ requested_domains: ["neuroscience"], max_risk: 0.01 }, [candidate("unsafe", "neuroscience", { risk: 0.9 })]);
  assert.equal(refused.posture, "refused");
  assert.equal(refused.selected_arm_id, null);
  assert.ok(refused.refusal_reasons.includes("risk_budget_exceeded"));
  assert.throws(() => policy.settle(refused, {
    settlement_id: "bad",
    arm_id: "unsafe",
    decision_digest: refused.decision_digest,
    outcome_digest: digest("c"),
    reward: 1,
    passed: true,
    evaluator_id: "evaluator",
    evaluator_version: "1",
  }), /refused/);
  const state = policy.snapshot();
  assert.throws(() => validateAutonomousJointExecutionPolicyState({ ...state, generation: 1 }), /digest/);
});

test("brain facade composes route admission with joint execution policy before dispatch", async () => {
  const facade = new AutonomousBrainFacade({
    agent: {
      route: async () => ({ task_digest: digest("d"), route_digest: digest("e"), selected_domains: ["coding"], primary_domain: "coding", abstained: false }),
      blueprint: async () => { throw new Error("blueprint must not be called"); },
      run: async () => { throw new Error("provider must not be called"); },
      runCrossDomain: async () => { throw new Error("provider must not be called"); },
      readiness: async () => ({}),
      refreshActivation: async () => ({}),
      domainPolicy: () => ({ evidence_mode: "optional", response_mode: "structured_required", max_total_cost_units: 8 }),
    },
  });
  const plan = await facade.selectExecutionPolicy({ task: "private task", domain: "coding" }, {
    candidates: [candidate("facade-arm", "coding", { structured_output: true })],
  });
  assert.equal(plan.schema, "bioprism-typescript-autonomous-brain-execution-policy/0.1");
  assert.equal(plan.decision.selected_arm_id, "facade-arm");
  assert.equal(plan.route.route_digest, digest("e"));
  assert.equal(JSON.stringify(plan).includes("private task"), false);
});
