import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
  AutonomousAgent,
  LLMRuntime,
  validateAutonomousWorkflowPortfolioAdmission,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function requests() {
  return AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => ({
    id: `admission-${domain}`,
    task: `private admission task for ${domain} must never be retained`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`admission-${AUTONOMOUS_DOMAIN_NAMES[index - 1]}`] }),
    hints: [`private hint for ${domain}`],
  }));
}

function agentFor(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("admission must not reach HTTP"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: "provider output must not be reached by admission" };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return agent;
}

test("portfolio admission is a keyless all-domain gate and never dispatches", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const admission = await agent.admitWorkflowPortfolio(requests(), { planOptions: { requireAllDomains: true } });

  assert.equal(admission.schema, AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA);
  assert.equal(admission.status, "ready_for_approval");
  assert.equal(admission.counts.eligible_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(admission.counts.blocked_count, 0);
  assert.equal(admission.items.every((item) => item.status === "eligible"), true);
  assert.deepEqual(admission.waves, AUTONOMOUS_DOMAIN_NAMES.map((domain) => [`admission-${domain}`]));
  assert.equal(providerCalls, 0);
  assert.equal(admission.execution, "admission_only;no_provider_tool_connector_or_effect_dispatch");
  assert.equal(admission.authorization, "admission_does_not_authorize_provider_tools_connectors_or_effects");
  assert.doesNotMatch(JSON.stringify(admission), /private admission task|private hint/);

  const restored = await validateAutonomousWorkflowPortfolioAdmission(admission);
  assert.deepEqual(restored, admission);
});

test("portfolio admission closes dependencies over missing model and provider readiness", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("admission must not reach HTTP"); } });
  const agent = new AutonomousAgent(runtime);
  const admission = await agent.admitWorkflowPortfolio(requests(), { planOptions: { requireAllDomains: true } });
  const byId = new Map(admission.items.map((item) => [item.item_id, item]));

  assert.equal(admission.status, "blocked");
  assert.equal(byId.get("admission-coding").status, "blocked");
  assert.equal(admission.items.filter((item) => item.item_id !== "admission-coding").every((item) => item.status === "dependency_blocked"), true);
  assert.equal(admission.counts.missing_model_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(byId.get("admission-coding").blockers.includes("readiness:model_catalogue_required"), true);
  assert.equal(byId.get("admission-browser").blockers.includes("dependency:not_eligible"), true);
});

test("portfolio admission applies shared model constraints before dependency dispatch", async () => {
  const agent = agentFor();
  const admission = await agent.admitWorkflowPortfolio(requests(), {
    planOptions: { requireAllDomains: true },
    run: { minQuality: 0.95 },
  });
  const byId = new Map(admission.items.map((item) => [item.item_id, item]));

  assert.equal(admission.status, "blocked");
  assert.equal(byId.get("admission-coding").status, "blocked");
  assert.equal(byId.get("admission-coding").blockers.includes("selection:no_model_matches_run_constraints"), true);
  assert.equal(admission.items.filter((item) => item.item_id !== "admission-coding").every((item) => item.status === "dependency_blocked"), true);
  assert.equal(admission.counts.eligible_count, 0);
});

test("portfolio admission verifies a reviewed plan before readiness projection", async () => {
  const agent = agentFor();
  const original = requests();
  const plan = await agent.planWorkflowPortfolio(original, { requireAllDomains: true });
  const changed = original.map((request, index) => index === 2 ? { ...request, task: "drifted task requires a new admission" } : request);

  await assert.rejects(
    () => agent.admitWorkflowPortfolio(changed, { plan }),
    /plan verification failed/,
  );
});

test("portfolio admission keeps incomplete required-domain coverage partial and tool policy explicit", async () => {
  const agent = agentFor();
  const admission = await agent.admitWorkflowPortfolio([
    { id: "coding-only", task: "bounded coding task", domain: "coding" },
  ], {
    planOptions: { requireAllDomains: true },
  });

  assert.equal(admission.status, "partial");
  assert.equal(admission.plan.coverage.complete, false);
  assert.equal(admission.items[0].status, "eligible");
  assert.equal(admission.next_actions.includes("resolve_missing_required_domain_coverage_before_full_portfolio_execution"), true);
  await validateAutonomousWorkflowPortfolioAdmission(admission);

  const toolBlocked = await agent.admitWorkflowPortfolio([
    { id: "coding-only", task: "bounded coding task", domain: "coding" },
  ], {
    requireAvailableTools: true,
  });
  assert.equal(toolBlocked.status, "blocked");
  assert.equal(toolBlocked.items[0].blockers.includes("tools:missing"), true);
});
