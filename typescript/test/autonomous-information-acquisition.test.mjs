import assert from "node:assert/strict";
import { test } from "node:test";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousInformationAcquisitionCandidate,
  AutonomousInformationAcquisitionObservation,
  AutonomousInformationAcquisitionPolicy,
  LLMRuntime,
  digestJsonSync,
  planAutonomousInformationAcquisition,
  replanAutonomousInformationAcquisition,
  validateAutonomousInformationAcquisitionPlan,
} from "../dist/index.js";

function candidate(candidateId, domain, { score = 0.8, cost = 0.1, dependsOn = [], status = "available" } = {}) {
  return new AutonomousInformationAcquisitionCandidate({
    candidateId,
    domain,
    capability: "evidence_acquisition",
    sourceId: `source-${candidateId}`,
    informationGain: score,
    uncertaintyReduction: score,
    reliability: 0.9,
    freshness: 0.95,
    coverage: 0.8,
    cost,
    latencyMs: 100,
    risk: 0.05,
    conflictRisk: 0.05,
    priority: 0.5,
    status,
    dependsOn,
  });
}

const taskDigest = digestJsonSync({ task: "choose the next bounded evidence acquisition" });

test("information planner covers all domains deterministically without dispatch", () => {
  const candidates = AUTONOMOUS_DOMAIN_NAMES.map((domain) => candidate(`candidate-${domain}`, domain));
  const plan = planAutonomousInformationAcquisition({
    taskDigest,
    candidates,
    requestedDomains: AUTONOMOUS_DOMAIN_NAMES,
    policy: new AutonomousInformationAcquisitionPolicy({ maxCost: 2, maxItems: AUTONOMOUS_DOMAIN_NAMES.length, requireDomainCoverage: true, exploration: 0 }),
  });
  assert.equal(plan.status, "ready");
  assert.deepEqual(plan.selectedDomains, AUTONOMOUS_DOMAIN_NAMES);
  assert.equal(plan.selected.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(plan.missingDomains, []);
  assert.equal(plan.coverageRatio, 1);
  const projection = plan.toJSON();
  assert.match(String(projection.execution), /^planning_only/);
  assert.match(String(projection.retention), /^metadata_only/);
  assert.equal(JSON.stringify(projection).includes("choose the next"), false);
});

test("dependency order and budget omissions remain explicit", () => {
  const base = candidate("base", "coding", { score: 0.2, cost: 0.2 });
  const dependent = candidate("dependent", "coding", { score: 1, cost: 0.2, dependsOn: ["base"] });
  const plan = planAutonomousInformationAcquisition({ taskDigest, candidates: [dependent, base], requestedDomains: ["coding"], policy: { maxCost: 0.4, maxItems: 2, exploration: 0 } });
  assert.deepEqual(plan.selected.map((item) => item.candidate_id), ["base", "dependent"]);
  assert.equal(plan.consumedCost, 0.4);

  const held = planAutonomousInformationAcquisition({ taskDigest, candidates: [candidate("too-expensive", "coding", { cost: 0.8 })], requestedDomains: ["coding"], policy: { maxCost: 0.2, maxItems: 1, exploration: 0 } });
  assert.ok(["blocked", "empty", "partial"].includes(held.status));
  assert.equal(held.omissions[0].reason, "budget_exceeded");
});

test("replan uses value-only observations and fences candidate drift", () => {
  const first = candidate("first", "science", { score: 0.95 });
  const second = candidate("second", "science", { score: 0.6 });
  const plan = planAutonomousInformationAcquisition({ taskDigest, candidates: [first, second], requestedDomains: ["science"], policy: { maxCost: 0.2, maxItems: 1, exploration: 0 } });
  const observation = new AutonomousInformationAcquisitionObservation({ candidateId: "first", status: "failed", valueDigest: "a".repeat(64), evaluatorDigest: "b".repeat(64) });
  const replanned = replanAutonomousInformationAcquisition({ previousPlan: plan, candidates: [first, second], observations: [observation] });
  assert.equal(replanned.generation, 2);
  assert.equal(replanned.priorPlanDigest, plan.planDigest);
  assert.ok(replanned.observationsDigest);
  assert.equal(replanned.selected[0].candidate_id, "second");
  assert.equal(validateAutonomousInformationAcquisitionPlan(replanned).planDigest, replanned.planDigest);
  const repeated = replanAutonomousInformationAcquisition({ previousPlan: replanned, candidates: [first, second], observations: [new AutonomousInformationAcquisitionObservation({ candidateId: "second", status: "accepted" })] });
  assert.equal(repeated.generation, 3);
  assert.equal(repeated.priorPlanDigest, replanned.planDigest);
  assert.throws(() => replanAutonomousInformationAcquisition({ previousPlan: plan, candidates: [candidate("first", "science", { score: 0.1 }), second], observations: [observation] }));
});

test("secret metadata and stale source posture fail closed at the correct boundary", () => {
  assert.throws(() => new AutonomousInformationAcquisitionCandidate({
    candidateId: "secret",
    domain: "coding",
    capability: "evidence_acquisition",
    sourceId: "source-secret",
    informationGain: 0.5,
    uncertaintyReduction: 0.5,
    reliability: 0.9,
    freshness: 0.9,
    coverage: 0.8,
    cost: 0.1,
    latencyMs: 100,
    risk: 0.1,
    conflictRisk: 0.1,
    metadata: { api_key: "must never enter a plan" },
  }));
  const stale = planAutonomousInformationAcquisition({ taskDigest, candidates: [candidate("stale", "coding", { status: "stale" })], requestedDomains: ["coding"] });
  assert.equal(stale.omissions[0].reason, "stale_not_allowed");
});

test("high-level agent facade binds explicit domains without provider or source dispatch", async () => {
  const agent = new AutonomousAgent(new LLMRuntime());
  const plan = await agent.planInformationAcquisition("choose a bounded coding evidence acquisition", {
    domains: ["coding"],
    candidates: [candidate("coding-next", "coding")],
    policy: { maxItems: 1, exploration: 0 },
  });
  assert.deepEqual(plan.requestedDomains, ["coding"]);
  assert.equal(plan.selected[0].candidate_id, "coding-next");
  assert.ok(plan.routeDigest);
  assert.match(plan.toJSON().execution, /^planning_only/);
  assert.equal(plan.toJSON().secret_material, "never_returned");
});
