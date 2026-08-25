import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_ACTION_PLAN_SCHEMA,
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousActionAdmission,
  AutonomousActionPlan,
  AutonomousAgent,
  AutonomousBrainFacade,
  LLMRuntime,
  admitAutonomousActionPlan,
} from "../dist/index.js";

const tasks = {
  coding: "debug a bounded repository change",
  browser: "compare web sources and citation gaps",
  data: "profile a dataset schema and missingness",
  science: "design a reproducible experiment and uncertainty report",
  biomedical: "review biomedical evidence with safety boundaries",
  neuroscience: "analyze neural signal preprocessing and limitations",
  operations: "prepare a reversible incident rollback runbook",
  enterprise: "map governance ownership and approvals",
  multi_agent: "delegate specialists and reconcile evidence",
  multimodal: "align document image and audio observations",
  cross_domain: "synthesize evidence across several disciplines",
  evaluation: "replay a benchmark and analyze evaluator failures",
};

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function brain() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("provider must not be reached"); } });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return new AutonomousBrainFacade({ agent });
}

test("action plan covers every built-in domain without provider dispatch", async () => {
  const facade = brain();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = tasks[domain];
    const plan = await facade.actionPlan({ task, domain, allow_cross_domain: false });
    assert.equal(plan.toJSON().schema, AUTONOMOUS_ACTION_PLAN_SCHEMA, domain);
    assert.deepEqual(plan.selected_domains, [domain], domain);
    assert.equal(plan.candidates[0].domain, domain, domain);
    assert.equal(plan.candidates[0].route_digest, plan.route_digest, domain);
    assert.equal(plan.plan_digest.length, 64, domain);
    assert.equal(JSON.stringify(plan), JSON.stringify(plan).includes(task) ? "task leaked" : JSON.stringify(plan));
  }
});

test("action plan aggregates cross-domain children and synthesis", async () => {
  const plan = await brain().actionPlan({
    task: "coordinate coding and biomedical evidence across disciplines",
    hints: ["coding", "biomedical"],
    allow_cross_domain: true,
  });
  assert.equal(plan.cross_domain, true);
  assert.deepEqual(new Set(plan.selected_domains), new Set(["coding", "biomedical"]));
  assert.deepEqual(plan.candidates.map((candidate) => candidate.role), ["child", "child", "synthesis"]);
  assert.equal(plan.recommended_path, "cross_domain");
  assert.equal(plan.next_action, "review_task_decision");
  assert.ok(plan.required_approvals.includes("plan_acceptance"));
});

test("action plan replay is digest-bound and blocks forbidden biomedical effects", async () => {
  const facade = brain();
  const plan = await facade.actionPlan({ task: "analyze a bounded data workflow", domain: "data" });
  const restored = AutonomousActionPlan.fromJSON(plan.toJSON());
  assert.deepEqual(restored.toJSON(), plan.toJSON());

  const tampered = structuredClone(plan.toJSON());
  tampered.next_action = "approve_provider_call";
  assert.throws(() => AutonomousActionPlan.fromJSON(tampered), /digest/);

  const blocked = await facade.actionPlan({ task: "deploy the biomedical report and verify safety", domain: "biomedical" });
  assert.equal(blocked.status, "blocked");
  assert.equal(blocked.next_action, "resolve_policy_block");
  assert.ok(blocked.blocking_reasons.some((reason) => reason.includes("requested_effect_forbidden_by_domain_policy")));
});

test("action plan preserves route abstention as a review action", async () => {
  const plan = await brain().actionPlan({ task: "zzzz qqqq an unclassified request", allow_cross_domain: false });
  assert.equal(plan.status, "route_review_required");
  assert.equal(plan.next_action, "review_route");
  assert.deepEqual(plan.candidates, []);
});

test("action-plan admission covers every domain without credentials or provider dispatch", async () => {
  const facade = brain();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await facade.actionPlan({ task: tasks[domain], domain, allow_cross_domain: false });
    const approvals = Object.fromEntries(plan.required_approvals.map((gate) => [gate, true]));
    const admission = admitAutonomousActionPlan(plan, { approvals, reviewed: true });
    if (plan.status === "blocked") assert.equal(admission.status, "blocked", domain);
    else {
      assert.equal(admission.status, "admitted", domain);
      assert.equal(admission.execution_path, plan.candidates[0].recommended_path, domain);
    }
    assert.equal(JSON.stringify(admission).includes(tasks[domain]), false, domain);
  }
});

test("action-plan execution returns a metadata-only review handoff before provider dispatch", async () => {
  const facade = brain();
  const input = { task: "debug a bounded repository change", domain: "coding", allow_cross_domain: false };
  const plan = await facade.actionPlan(input);
  const admission = admitAutonomousActionPlan(plan);
  assert.equal(admission.status, "review_required");
  const execution = await facade.executeActionPlan(input, plan);
  assert.equal(execution.status, "review_required");
  assert.equal(execution.result, null);
  assert.equal(execution.execution_status, "review_required");
  assert.equal(JSON.stringify(execution).includes(input.task), false);
});

test("action-plan admission handles cross-domain synthesis and rejects stale or tampered replay", async () => {
  const facade = brain();
  const input = { task: "coordinate coding and biomedical evidence across disciplines", hints: ["coding", "biomedical"], allow_cross_domain: true };
  const plan = await facade.actionPlan(input);
  const approvals = Object.fromEntries(plan.required_approvals.map((gate) => [gate, true]));
  const admission = admitAutonomousActionPlan(plan, { approvals, reviewed: true });
  assert.equal(admission.status, "admitted");
  assert.equal(admission.execution_path, "cross_domain");
  const restored = AutonomousActionAdmission.fromJSON(admission.toJSON());
  assert.deepEqual(restored.toJSON(), admission.toJSON());

  const tampered = structuredClone(admission.toJSON());
  tampered.approved_approvals = [];
  assert.throws(() => AutonomousActionAdmission.fromJSON(tampered), /digest/);
  await assert.rejects(
    () => facade.executeActionPlan({ ...input, task: "coordinate a different evidence route" }, plan, { approvals, reviewed: true }),
    /stale|match/,
  );
});

test("action-plan admission preserves forbidden biomedical effects", async () => {
  const facade = brain();
  const plan = await facade.actionPlan({ task: "deploy the biomedical report and verify safety", domain: "biomedical" });
  const admission = admitAutonomousActionPlan(plan, { approvals: Object.fromEntries(plan.required_approvals.map((gate) => [gate, true])), reviewed: true });
  assert.equal(admission.status, "blocked");
  assert.equal(admission.next_action, "resolve_policy_block");
});
