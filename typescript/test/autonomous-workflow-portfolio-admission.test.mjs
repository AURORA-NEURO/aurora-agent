import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_WORKFLOW_PORTFOLIO_ADMISSION_SCHEMA,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousWorkflowPortfolioAdmissionController,
  AutonomousWorkflowPortfolioItemExecutionResult,
  InMemoryAutonomousWorkflowPortfolioAdmissionPersistence,
  JsonAutonomousWorkflowPortfolioAdmissionPersistence,
  LLMRuntime,
  WebStorageAutonomousWorkflowPortfolioAdmissionTextStore,
  digestJson,
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

  const facadeAdmission = await new AutonomousBrainFacade({ agent }).admitWorkflowPortfolio(requests(), { plan: admission.plan });
  assert.equal(facadeAdmission.admission_digest, admission.admission_digest);
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

  const partialExecution = await agent.executeWorkflowPortfolio([
    { id: "coding-only", task: "bounded coding task", domain: "coding" },
  ], { plan: admission.plan, admission, approveProviderCall: true });
  assert.equal(partialExecution.status, "partial");
  assert.equal(partialExecution.admissionDigest, admission.admission_digest);

  const toolBlocked = await agent.admitWorkflowPortfolio([
    { id: "coding-only", task: "bounded coding task", domain: "coding" },
  ], {
    requireAvailableTools: true,
  });
  assert.equal(toolBlocked.status, "blocked");
  assert.equal(toolBlocked.items[0].blockers.includes("tools:missing"), true);
});

test("resumable portfolio execution binds admission to its checkpoint and refuses a held portfolio", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const requestsForRestart = [
    { id: "admission-first", task: "private first admitted task", domain: "coding" },
    { id: "admission-second", task: "private second admitted task", domain: "data", dependsOn: ["admission-first"] },
  ];
  const originalRun = agent.run.bind(agent);
  let firstRun;
  agent.run = async (task, options) => {
    const run = await originalRun(task, options);
    if (task === "private first admitted task") firstRun = run;
    return run;
  };
  const plan = await agent.planWorkflowPortfolio(requestsForRestart);
  const admission = await agent.admitWorkflowPortfolio(requestsForRestart, { plan });
  assert.equal(admission.status, "ready_for_approval");

  let checkpoint;
  await assert.rejects(
    () => agent.executeWorkflowPortfolioResumable(requestsForRestart, {
      jobId: "admission-restart",
      plan,
      admission,
      requireAdmission: true,
      approveProviderCall: true,
      checkpointSink: async (value) => {
        checkpoint = value;
        if (value.settled_item_ids.length > 0) throw new Error("synthetic admission interruption");
      },
    }),
    /synthetic admission interruption/,
  );
  assert.equal(providerCalls, 1);
  assert.equal(checkpoint.admission_digest, admission.admission_digest);
  assert.equal(checkpoint.schema, "bioprism-typescript-autonomous-workflow-portfolio-execution-checkpoint/0.3");

  const resumed = await agent.executeWorkflowPortfolioResumable(requestsForRestart, {
    jobId: "admission-restart",
    plan,
    admission,
    requireAdmission: true,
    checkpoint,
    approveProviderCall: true,
    rehydrateItem: async (context) => {
      const output = firstRun.response?.text ?? "";
      return new AutonomousWorkflowPortfolioItemExecutionResult(
        context.item_id,
        context.domain,
        [],
        "succeeded",
        firstRun,
        output ? await digestJson({ item_id: context.item_id, output }) : null,
        new TextEncoder().encode(output).byteLength,
        null,
        null,
        true,
        output,
      );
    },
  });
  assert.equal(resumed.status, "completed");
  assert.equal(providerCalls, 2);

  const heldAdmission = await agent.admitWorkflowPortfolio(requestsForRestart, { plan, run: { minQuality: 0.95 } });
  const heldResult = await agent.executeWorkflowPortfolioResumable(requestsForRestart, {
    jobId: "admission-held",
    plan,
    admission: heldAdmission,
    requireAdmission: true,
    approveProviderCall: true,
  });
  assert.equal(heldResult.status, "blocked");
  assert.equal(providerCalls, 2);
  assert.equal(heldResult.admissionDigest, heldAdmission.admission_digest);

  const tampered = structuredClone(admission);
  tampered.admission_digest = "0".repeat(64);
  await assert.rejects(
    () => agent.executeWorkflowPortfolioResumable(requestsForRestart, {
      jobId: "admission-restart",
      plan,
      admission: tampered,
      requireAdmission: true,
      checkpoint,
      approveProviderCall: true,
      rehydrateItem: async () => { throw new Error("tampered admission must fail before rehydration"); },
    }),
    /admission digest is invalid/,
  );
});

test("portfolio admission persistence restores redacted state and fences stale coordinators", async () => {
  const agent = agentFor();
  const persistence = new InMemoryAutonomousWorkflowPortfolioAdmissionPersistence();
  const controller = new AutonomousWorkflowPortfolioAdmissionController(agent, persistence);
  assert.equal((await controller.restore()).status, "empty");
  const stale = new AutonomousWorkflowPortfolioAdmissionController(agent, persistence);
  await stale.restore();
  const admission = await controller.admit(requests(), { planOptions: { requireAllDomains: true } });
  assert.equal(controller.projection().status, "admitted");
  assert.equal(controller.projection().admission_digest, admission.admission_digest);
  assert.doesNotMatch(JSON.stringify(await persistence.read()), /private admission task|private hint/);

  const restarted = new AutonomousWorkflowPortfolioAdmissionController(agent, persistence);
  const restoredProjection = await restarted.restore();
  assert.equal(restoredProjection.status, "restored");
  assert.deepEqual(restarted.admission(), admission);

  await assert.rejects(
    () => stale.admit(requests(), { planOptions: { requireAllDomains: true }, run: { minQuality: 0.95 } }),
    /stale|digest/,
  );

  let text = null;
  const json = new JsonAutonomousWorkflowPortfolioAdmissionPersistence({
    read: () => text,
    write: (value) => { text = value; },
  });
  await json.write(admission);
  assert.deepEqual(await json.read(), admission);

  const values = new Map();
  const web = new WebStorageAutonomousWorkflowPortfolioAdmissionTextStore({
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }, "admission-image");
  await web.write(text);
  assert.equal(web.read(), text);
});
