import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA,
  AutonomousAgent,
  LLMRuntime,
  validateAutonomousWorkflowPortfolioPlan,
} from "../dist/index.js";

function localAgent() {
  return new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("portfolio planning must not invoke a provider"); } }));
}

function allDomainRequests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `portfolio-${domain}`,
    task: `private task payload for ${domain} must remain transient`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`portfolio-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
    hints: [`private hint for ${domain}`],
  }));
}

test("workflow portfolio compiles every domain into a dependency-closed metadata plan", async () => {
  const agent = localAgent();
  const plan = await agent.planWorkflowPortfolio(allDomainRequests(), { requireAllDomains: true });

  assert.equal(plan.schema, AUTONOMOUS_WORKFLOW_PORTFOLIO_SCHEMA);
  assert.equal(plan.status, "ready");
  assert.equal(plan.coverage.complete, true);
  assert.deepEqual(plan.coverage.requested_domains, [...AUTONOMOUS_DOMAIN_NAMES].sort());
  assert.deepEqual(plan.coverage.missing_domains, []);
  assert.equal(plan.coverage.ready_item_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(plan.dependency_graph.cycle_item_ids.length, 0);
  assert.equal(plan.dependency_graph.edge_count, AUTONOMOUS_DOMAIN_NAMES.length - 1);
  assert.equal(plan.dependency_graph.topological_order.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(plan.items.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(plan.items.every((item) => item.status === "ready" && item.workflow_digest?.length === 64 && item.plan_digest?.length === 64));
  assert.ok(plan.items.every((item) => item.request_digest.length === 64));
  assert.equal(plan.execution, "not_started;planning_and_verification_only");
  assert.equal(plan.authorization, "portfolio_selection_does_not_authorize_provider_tools_or_effects");
  assert.doesNotMatch(JSON.stringify(plan), /private task payload|private hint/);

  const restored = await validateAutonomousWorkflowPortfolioPlan(plan);
  assert.deepEqual(restored, plan);

  const verified = await agent.verifyWorkflowPortfolio(plan, allDomainRequests());
  assert.equal(verified.status, "verified");
  assert.equal(verified.observed_portfolio_digest, plan.portfolio_digest);
  assert.equal(verified.replayed_item_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(verified.mismatches, []);
});

test("workflow portfolio replay exposes task/context drift without executing anything", async () => {
  const agent = localAgent();
  const requests = allDomainRequests();
  const plan = await agent.planWorkflowPortfolio(requests, { requireAllDomains: true });
  const changed = requests.map((request, index) => index === 3 ? { ...request, task: "changed task must trigger portfolio drift" } : request);
  const verification = await agent.verifyWorkflowPortfolio(plan, changed);

  assert.equal(verification.status, "mismatch");
  const row = verification.mismatches.find((item) => item.item_id === requests[3].id);
  assert.ok(row);
  assert.ok(row.codes.includes("task_digest"));
  assert.ok(row.codes.includes("request_digest"));
  assert.notEqual(verification.observed_portfolio_digest, plan.portfolio_digest);

  const tampered = structuredClone(plan);
  tampered.items[0].task_digest = "0".repeat(64);
  await assert.rejects(() => validateAutonomousWorkflowPortfolioPlan(tampered), /portfolio plan digest is invalid/);
});

test("workflow portfolio blocks dependency cycles and rejects unknown dependencies", async () => {
  const agent = localAgent();
  const cycle = await agent.planWorkflowPortfolio([
    { id: "a", task: "cycle a", domain: "coding", dependsOn: ["b"] },
    { id: "b", task: "cycle b", domain: "evaluation", dependsOn: ["a"] },
  ]);
  assert.equal(cycle.status, "blocked");
  assert.deepEqual(cycle.dependency_graph.cycle_item_ids, ["a", "b"]);
  assert.equal(cycle.coverage.blocked_item_count, 2);
  assert.ok(cycle.items.every((item) => item.error_class === "dependency_cycle"));

  await assert.rejects(
    () => agent.planWorkflowPortfolio([{ id: "orphan", task: "orphan", domain: "coding", dependsOn: ["missing"] }]),
    /unknown item/,
  );
  await assert.rejects(
    () => agent.planWorkflowPortfolio([{ id: "duplicate", task: "one", domain: "coding" }, { id: "duplicate", task: "two", domain: "data" }]),
    /duplicated/,
  );
});
