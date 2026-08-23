import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ApiClient,
  AutonomousAgent,
  AutonomousCapabilityActivation,
  AutonomousCapabilityActivationPersistenceCoordinator,
  AutonomousCapabilityActivationStore,
  AutonomousCapabilityPersistenceError,
  AutonomousCapabilityRuntime,
  InMemoryAutonomousCapabilityLearningSettlementStore,
  AutonomousCapabilityLearningPersistenceCoordinator,
  validateAutonomousCapabilityLearningSnapshot,
  AutonomousCostBudget,
  AutonomousCostBudgetError,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceReadinessPolicy,
  AutonomousEvaluatorCalibrationHarness,
  AutonomousValueEvaluatorRegistry,
  AutonomousDomainToolRegistry,
  AutonomousDomainToolRuntime,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  InMemoryAutonomousEpisodicMemory,
  AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA,
  AUTONOMOUS_CAPABILITY_PLAN_SCHEMA,
  AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA,
  InMemoryAutonomousCapabilityJournalStore,
  AutonomousCapabilityJournalPersistenceCoordinator,
  validateAutonomousCapabilityJournalSnapshot,
  AUTONOMOUS_READINESS_SCHEMA,
  AUTONOMOUS_DOMAIN_NAMES,
  CredentialStore,
  LLMRuntime,
  ToolCatalogue,
  builtinAutonomousDomainProfiles,
  builtinAutonomousValueEvaluatorProfiles,
  assembleAutonomousPrompt,
  compileAutonomousPlan,
  digestCanonicalJsonTextSync,
  digestJson,
  createAutonomousApiToolExecutor,
  openaiCompatibleProvider,
  routeAutonomousTask,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

const candidate = (provider, model, capabilities = ["reasoning", "code"]) => ({
  provider,
  model,
  capabilities,
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
});

const learningContextDigest = (context) => digestCanonicalJsonTextSync(JSON.stringify({
  domain: context.domain,
  capability: context.capability,
  risk_class: context.risk_class,
  task_family: context.task_family ?? null,
}));

function readinessCalibrationReport({ weak = false } = {}) {
  const profiles = builtinAutonomousValueEvaluatorProfiles();
  const cases = profiles.flatMap((profile) => {
    const evidence = (value) => ({
      schema: "bioprism-brain-domain-evaluator/0.1",
      domain: profile.domain,
      capability: "readiness-calibration",
      risk_class: "read_only",
      signals: Object.fromEntries(profile.required_signals.map((signal) => [signal, value])),
      references: [],
      limitations: [],
      retention: "value_only_digests_and_signal_scores",
    });
    return [
      { case_id: `${profile.domain}-readiness-calibration-positive`, domain: profile.domain, evidence: evidence(1), label: 1, split: "calibration" },
      { case_id: `${profile.domain}-readiness-calibration-negative`, domain: profile.domain, evidence: evidence(0), label: 0, split: "calibration" },
      { case_id: `${profile.domain}-readiness-holdout-positive`, domain: profile.domain, evidence: evidence(1), label: weak && profile.domain === "coding" ? 0 : 1, split: "holdout" },
      { case_id: `${profile.domain}-readiness-holdout-negative`, domain: profile.domain, evidence: evidence(0), label: 0, split: "holdout" },
    ];
  });
  return new AutonomousEvaluatorCalibrationHarness(AutonomousValueEvaluatorRegistry.withBuiltinProfiles()).run({
    cases,
    bins: 5,
    minCalibrationCasesPerDomain: 2,
    minHoldoutCasesPerDomain: 2,
    maxExpectedCalibrationError: 0.01,
    maxBrierScore: 0.01,
  });
}

test("synchronous control-plane SHA-256 matches the standard digest", () => {
  assert.equal(digestCanonicalJsonTextSync("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
});

test("all twelve built-in domains expose profiles, workflows, tools, and deterministic routing", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  assert.equal(profiles.length, 12);
  assert.equal(new Set(profiles.map((profile) => profile.domain)).size, 12);
  const examples = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review this patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  for (const [domain, task] of Object.entries(examples)) {
    const route = await routeAutonomousTask(task);
    assert.equal(route.abstained, false, `${domain} should route`);
    assert.equal(route.primary_domain, domain);
    assert.equal(route.route_digest.length, 64);
    const profile = profiles.find((row) => row.domain === domain);
    assert.ok(profile);
    assert.ok(profile.workflow.stages.length >= 4);
    assert.ok(profile.tool_profile.bindings.length >= 10);
    assert.equal(profile.workflow.workflow_digest.length, 64);
  }
});

test("built-in workflows preserve domain-specific objectives, evidence, and safety gates", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const expected = {
    coding: { stages: ["scope", "inspect", "implement", "verify", "handoff"], signals: ["schema_valid", "tests_passed", "evidence_complete"], evidence: ["scope", "acceptance_criteria", "test_results", "residual_risks"] },
    browser: { stages: ["scope", "retrieve", "compare", "synthesize"], signals: ["evidence_traceable", "uncertainty_reported", "claim_scope_respected"], evidence: ["sources", "retrieval_gaps", "citations"] },
    data: { stages: ["schema", "lineage", "quality", "transform", "report"], signals: ["schema_valid", "lineage_complete", "quality_gate_passed"], evidence: ["schema_contract", "lineage", "quality_metrics"] },
    science: { stages: ["question", "evidence", "hypothesis", "design", "reproduce"], signals: ["evidence_traceable", "uncertainty_reported", "claim_scope_respected"], evidence: ["question", "evidence_map", "reproduction_plan"] },
    biomedical: { stages: ["scope", "provenance", "review", "escalate", "communicate"], signals: ["boundary_compliant", "provenance_complete", "human_review_ready"], evidence: ["boundary", "provenance", "review_questions"] },
    neuroscience: { stages: ["measurement", "preprocess", "model", "biology", "reproduce"], signals: ["evidence_traceable", "uncertainty_reported", "claim_scope_respected"], evidence: ["measurement_contract", "confounds", "validation_plan"] },
    operations: { stages: ["observe", "impact", "rollback", "approval", "handoff"], signals: ["safety_gate_passed", "approval_complete", "rollback_plan_present"], evidence: ["observations", "rollback", "approval_request"] },
    enterprise: { stages: ["request", "policy", "options", "decision", "audit"], signals: ["schema_valid", "approval_complete", "evidence_complete"], evidence: ["stakeholders", "policy_map", "approver"] },
    multi_agent: { stages: ["decompose", "delegate", "reconcile", "synthesize"], signals: ["schema_valid", "evidence_complete", "claim_scope_respected"], evidence: ["subtasks", "assignments", "conflicts"] },
    multimodal: { stages: ["inventory", "extract", "align", "uncertainty", "synthesize"], signals: ["evidence_traceable", "uncertainty_reported", "claim_scope_respected"], evidence: ["modality_inventory", "mismatches", "blind_spots"] },
    cross_domain: { stages: ["decompose", "route", "align", "synthesize", "gate"], signals: ["schema_valid", "evidence_traceable", "evidence_complete", "uncertainty_reported"], evidence: ["domain_questions", "disagreements", "decision_gate"] },
    evaluation: { stages: ["rubric", "cases", "replay", "failure", "report"], signals: ["schema_valid", "evidence_complete", "tests_passed", "claim_scope_respected"], evidence: ["rubric", "coverage", "evaluation_report"] },
  };
  for (const profile of profiles) {
    const contract = expected[profile.domain];
    assert.deepEqual(profile.workflow.stages.map((stage) => stage.id), contract.stages, profile.domain);
    assert.deepEqual(profile.workflow.evaluator_signals, contract.signals, profile.domain);
    const outputs = new Set(profile.workflow.stages.flatMap((stage) => stage.evidence_outputs));
    for (const output of contract.evidence) assert.ok(outputs.has(output), `${profile.domain} missing ${output}`);
    assert.ok(profile.workflow.stages.every((stage) => stage.objective.length > 32), `${profile.domain} still has a generic stage objective`);
  }
  const operations = profiles.find((profile) => profile.domain === "operations");
  assert.equal(operations.workflow.stages.find((stage) => stage.id === "approval").approval_required, true);
  assert.equal(operations.workflow.stages.find((stage) => stage.id === "rollback").depends_on[0], "impact");
});

test("routing abstains on weak evidence and permits explicit cross-domain review", async () => {
  const unknown = await routeAutonomousTask("please help me with something");
  assert.equal(unknown.abstained, true);
  assert.equal(unknown.reason, "no_matching_evidence");

  const cross = await routeAutonomousTask("research a biomedical neuroscience experiment with EEG patient evidence", { allowCrossDomain: true });
  assert.equal(cross.abstained, false);
  assert.equal(cross.cross_domain, true);
  assert.ok(cross.selected_domains.length >= 2);
  assert.equal(cross.reason, "cross_domain");
});

test("prompt and plan construction preserve budgets, omissions, dependencies, and digests", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((row) => row.domain === "coding");
  assert.ok(profile);
  const prompt = await assembleAutonomousPrompt(profile, "Implement and verify the requested change.", {
    maxInputTokens: 512,
    context: [
      { id: "required", content: "This small required acceptance criterion must remain.", required: true, priority: 10 },
      { id: "optional-large", content: "optional evidence ".repeat(300), priority: 1 },
    ],
  });
  assert.equal(prompt.complete, false);
  assert.deepEqual(prompt.included_context_ids, ["required", "autonomy-evidence-plan"]);
  assert.deepEqual(prompt.omitted_context_ids, ["optional-large"]);
  assert.equal(prompt.prompt_digest.length, 64);

  const plan = await compileAutonomousPlan(profile, "Implement and verify the requested change.", {
    taskDigest: "a".repeat(64),
    activeToolNames: ["repository_catalog"],
    selectedToolNames: ["repository_catalog"],
  });
  assert.deepEqual(plan.ordered_step_ids, profile.workflow.stages.map((stage) => stage.id));
  assert.equal(plan.steps[1].depends_on[0], plan.steps[0].id);
  assert.equal(plan.steps[0].tool, "provider.invoke");
  assert.equal(plan.steps[1].tool, "repository_catalog");
  assert.equal(plan.plan_digest.length, 64);
});

test("provider planning is approval-gated, dependency-closed, and domain-neutral", async () => {
  const calls = [];
  const allCapabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (!planningMessage) return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "domain execution" }) }, finish_reason: "stop" }] });
      const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
      const ids = (contract.stage_catalogue ?? contract.child_catalogue).map((row) => row.id);
      const focusField = contract.stage_catalogue ? "focus_stage_ids" : "focus_child_ids";
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.91, abstain: false }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("planner", "https://planner.test", { requiresCredential: false, structuredOutputMode: "json_schema" }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("planner", "planner-model", allCapabilities));

  const blueprint = await agent.blueprint("Debug this coding repository and report verified tests.", { domain: "coding" });
  assert.ok(blueprint.blueprint);
  const refused = await agent.planWithProvider(blueprint.blueprint);
  assert.equal(refused.status, "approval_required");
  assert.equal(calls.length, 0);
  assert.doesNotMatch(JSON.stringify(refused), /Debug this coding repository/);

  const planningBudget = new AutonomousCostBudget(1);
  await assert.rejects(
    () => agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true, costBudget: new AutonomousCostBudget(0) }),
    (error) => error instanceof AutonomousCostBudgetError,
  );
  assert.equal(calls.length, 0, "a zero planning budget refuses before provider dispatch");
  const planned = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true, costBudget: planningBudget });
  assert.equal(planned.status, "completed");
  assert.deepEqual(planned.priority_stage_ids, blueprint.blueprint.workflow.stages.map((stage) => stage.id));
  assert.equal(planned.focus_stage_ids.length, 1);
  assert.equal(planned.planner_prompt_digest.length, 64);
  assert.equal(planned.selection_digest.length, 64);
  assert.equal(planned.cost_budget.max_cost_units, 1);
  assert.ok(planned.cost_budget.consumed_cost_units > 0);
  assert.equal(calls[0].response_format.type, "json_schema");

  const crossBlueprint = await agent.blueprint("Write Python code for this dataset pipeline.");
  assert.ok(crossBlueprint.cross_domain_blueprint);
  const cross = await agent.planCrossDomainWithProvider(crossBlueprint.cross_domain_blueprint, { approveProviderCall: true, costBudget: planningBudget });
  assert.equal(cross.status, "completed");
  assert.deepEqual(cross.priority_child_ids, crossBlueprint.cross_domain_blueprint.child_ids);
  assert.equal(cross.planner_prompt_digest.length, 64);
  assert.equal(cross.cost_budget.max_cost_units, 1);
  assert.ok(cross.cost_budget.consumed_cost_units > planned.cost_budget.consumed_cost_units, "planning and cross-domain planning share one aggregate budget");
  assert.doesNotMatch(JSON.stringify(cross), /Write Python code/);

  const domains = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  for (const [domain, task] of Object.entries(domains)) {
    const routed = await agent.blueprint(task, { domain });
    const domainBudget = new AutonomousCostBudget(1);
    let result;
    if (routed.cross_domain_blueprint) {
      result = await agent.planCrossDomainWithProvider(routed.cross_domain_blueprint, { approveProviderCall: true, costBudget: domainBudget });
    } else {
      assert.ok(routed.blueprint, domain);
      result = await agent.planWithProvider(routed.blueprint, { approveProviderCall: true, costBudget: domainBudget });
    }
    assert.equal(result.status, "completed", domain);
    assert.equal(result.cost_budget.max_cost_units, 1, domain);
    assert.ok(result.cost_budget.consumed_cost_units > 0, domain);
  }

  for (const [domain, task] of Object.entries(domains)) {
    const budget = new AutonomousCostBudget(2);
    const twoPhase = await agent.planAndRun(task, {
      domain,
      planning: { approveProviderCall: true, costBudget: budget },
      costBudget: budget,
      acceptPlan: true,
      approveProviderCall: true,
    });
    assert.equal(twoPhase.status, "completed", domain);
    assert.equal(twoPhase.plan_refinement.status, "completed", domain);
    assert.equal(twoPhase.result.status, "completed", domain);
    assert.equal(twoPhase.result.plan_refinement_digest.length, 64, domain);
  }
});

test("planAndRun requires explicit planning acceptance and binds the accepted proposal into direct execution", async () => {
  const calls = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (planningMessage) {
        const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
        const ids = (contract.stage_catalogue ?? contract.child_catalogue).map((row) => row.id);
        const focusField = contract.stage_catalogue ? "focus_stage_ids" : "focus_child_ids";
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.97, abstain: false }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "executed under accepted plan" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("planner-runner", "https://planner-runner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("planner-runner", "planner-model", ["reasoning", "code", "structured_output"]));

  const reviewBudget = new AutonomousCostBudget(2);
  const review = await agent.planAndRun("Debug this coding repository and report verified tests.", {
    domain: "coding",
    planning: { approveProviderCall: true, costBudget: reviewBudget },
    costBudget: reviewBudget,
    approveProviderCall: true,
  });
  assert.equal(review.status, "plan_review_required");
  assert.equal(review.plan_refinement.status, "completed");
  assert.equal(review.result, null);
  assert.equal(calls.length, 1, "planning acceptance pauses before execution dispatch");

  const executeBudget = new AutonomousCostBudget(2);
  const executed = await agent.planAndRun("Debug this coding repository and report verified tests.", {
    domain: "coding",
    planning: { approveProviderCall: true, costBudget: executeBudget },
    costBudget: executeBudget,
    acceptPlan: true,
    approveProviderCall: true,
  });
  assert.equal(executed.status, "completed");
  assert.equal(executed.plan_refinement.status, "completed");
  assert.equal(executed.result.status, "completed");
  assert.equal(executed.result.plan_refinement_digest.length, 64);
  assert.ok(executed.result.response.text.includes("executed under accepted plan"));
  assert.equal(calls.length, 3, "accepted planning performs exactly one planner and one execution dispatch");
  assert.ok(executeBudget.snapshot().consumed_cost_units > 0);

  const invalid = structuredClone(executed.plan_refinement);
  invalid.priority_stage_ids.reverse();
  await assert.rejects(
    () => agent.run("Debug this coding repository and report verified tests.", {
      domain: "coding",
      acceptedSingleDomainPlanRefinement: invalid,
      approveProviderCall: true,
    }),
    /accepted plan violates workflow dependencies/,
  );
  assert.equal(calls.length, 3, "dependency-invalid accepted plans fail before a second provider dispatch");
});

test("planAndRun applies the same accepted planning contract to cross-domain fan-out and synthesis", async () => {
  const calls = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (planningMessage) {
        const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
        const ids = (contract.stage_catalogue ?? contract.child_catalogue).map((row) => row.id);
        const focusField = contract.stage_catalogue ? "focus_stage_ids" : "focus_child_ids";
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.94, abstain: false }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "cross-domain execution" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cross-planner", "https://cross-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("cross-planner", "cross-model", ["reasoning", "structured_output", "biomedical", "science", "coordination"]));
  const budget = new AutonomousCostBudget(8);
  const result = await agent.planAndRun("Research a biomedical neuroscience experiment with EEG patient evidence.", {
    allowCrossDomain: true,
    planning: { approveProviderCall: true, costBudget: budget },
    costBudget: budget,
    acceptPlan: true,
    approveProviderCall: true,
    maxParallelChildren: 1,
  });
  assert.equal(result.route.cross_domain, true);
  assert.equal(result.status, "completed");
  assert.equal(result.plan_refinement.status, "completed");
  assert.equal(result.result.status, "completed");
  assert.equal(result.result.plan_refinement_digest.length, 64);
  assert.equal(result.result.child_runs.length, 2);
  assert.equal(result.result.child_runs.every((child) => child.result.status === "completed"), true);
  assert.equal(calls.length, 4, "cross-domain planning dispatches one planner, two specialists, and one synthesis call");
  assert.ok(budget.snapshot().consumed_cost_units > 0);
});

test("ordinary runs close the episodic-memory retrieval and recording loop across every domain", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const domains = {
    coding: "debug this Rust repository",
    browser: "navigate the browser and compare sources",
    data: "validate this parquet dataset lineage",
    science: "design a hypothesis experiment",
    biomedical: "review patient treatment evidence",
    neuroscience: "analyze EEG preprocessing",
    operations: "plan a rollback after an outage",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this subtask to a specialist agent",
    multimodal: "inspect this image and transcript",
    cross_domain: "perform an interdisciplinary synthesis",
    evaluation: "run a benchmark holdout replay",
  };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: "transient-provider-output" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("memory-provider", "https://memory-provider.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { memoryStore: memory });
  agent.registerModel(candidate("memory-provider", "memory-model", ["reasoning", "code", "structured_output"]));

  for (const [domain, task] of Object.entries(domains)) {
    const result = await agent.run(task, {
      domain,
      approveProviderCall: false,
      memoryRunId: `domain-${domain}`,
      memoryLesson: `review:${domain}`,
    });
    assert.equal(result.status, "approval_required", domain);
    assert.equal(result.memory.status, "recorded", domain);
    assert.equal(result.memory.recorded_episode_id, `episode:domain-${domain}`, domain);
  }
  assert.equal((await memory.stats()).episodes, 12);
  const snapshot = await memory.snapshot();
  assert.doesNotMatch(JSON.stringify(snapshot), /transient-provider-output/);
  assert.doesNotMatch(JSON.stringify(snapshot), /debug this Rust repository/);

  const calls = [];
  const retrievingLlm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "retrieved" }, finish_reason: "stop" }] });
    },
  });
  retrievingLlm.registerProvider(openaiCompatibleProvider("memory-provider", "https://memory-provider.test", { requiresCredential: false }));
  const retrievingAgent = new AutonomousAgent(retrievingLlm, { memoryStore: memory });
  retrievingAgent.registerModel(candidate("memory-provider", "memory-model", ["reasoning", "code"]));
  const retrieved = await retrievingAgent.run(domains.coding, {
    domain: "coding",
    approveProviderCall: true,
    memoryRunId: "coding-retrieval",
  });
  assert.equal(retrieved.status, "completed");
  assert.equal(retrieved.memory.status, "recorded");
  assert.ok(retrieved.memory.retrieved_episode_ids.includes("episode:domain-coding"));
  assert.ok(calls[0].messages.some((message) => message.content.includes("autonomous-memory-")), "retrieved memory must be visible to the prompt compiler");
  assert.equal((await memory.verifyIntegrity()).episodes, 13);
});

test("cross-domain memory is retrieved once, shared with specialists and synthesis, and recorded once", async () => {
  const memory = new InMemoryAutonomousEpisodicMemory();
  const calls = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      calls.push(body);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "cross-domain" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cross-memory", "https://cross-memory.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { memoryStore: memory });
  agent.registerModel(candidate("cross-memory", "cross-model", ["reasoning", "biomedical", "science", "coordination"]));
  const seed = await agent.runCrossDomain("Research a biomedical neuroscience experiment with EEG patient evidence.", {
    approveProviderCall: false,
    allowCrossDomain: true,
    memoryRunId: "cross-seed",
    maxParallelChildren: 1,
  });
  assert.equal(seed.memory.status, "recorded");
  const result = await agent.runCrossDomain("Research a biomedical neuroscience experiment with EEG patient evidence.", {
    approveProviderCall: true,
    allowCrossDomain: true,
    memoryRunId: "cross-parent",
    maxParallelChildren: 1,
  });
  assert.equal(result.status, "completed");
  assert.equal(result.memory.status, "recorded");
  assert.equal(result.memory.recorded_episode_id, "episode:cross-parent");
  assert.ok(result.memory.retrieved_episode_ids.includes("episode:cross-seed"));
  assert.equal(result.child_runs.length, 2);
  assert.equal(calls.length, 3);
  const providerMessagesWithMemory = calls.filter((body) => body.messages.some((message) => message.content.includes("autonomous-memory-")));
  assert.equal(providerMessagesWithMemory.length, 3, "memory must flow to both specialists and synthesis");
  assert.equal((await memory.stats()).episodes, 2);
});

test("episodic-memory failures remain explicit without changing the provider execution result", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  const seed = new InMemoryAutonomousEpisodicMemory();
  const retrievalFailure = {
    retrieve: async () => { throw new Error("retrieval backend unavailable"); },
    recordEpisode: seed.recordEpisode.bind(seed),
    get: seed.get.bind(seed),
  };
  const retrievalAgent = new AutonomousAgent(llm, { memoryStore: retrievalFailure });
  const retrievalResult = await retrievalAgent.run("debug a bounded coding task", {
    domain: "coding",
    approveProviderCall: false,
    memoryRunId: "retrieval-failure",
  });
  assert.equal(retrievalResult.status, "approval_required");
  assert.equal(retrievalResult.memory.status, "recorded");
  assert.equal(retrievalResult.memory.error_class, "Error");

  const recordFailure = {
    retrieve: seed.retrieve.bind(seed),
    recordEpisode: async () => { throw new Error("record backend unavailable"); },
    get: seed.get.bind(seed),
  };
  const recordAgent = new AutonomousAgent(llm, { memoryStore: recordFailure });
  const recordResult = await recordAgent.run("debug a bounded coding task", {
    domain: "coding",
    approveProviderCall: false,
    memoryRunId: "record-failure",
  });
  assert.equal(recordResult.status, "approval_required");
  assert.equal(recordResult.memory.status, "record_failed");
  assert.equal(recordResult.memory.error_class, "Error");
});

test("ordinary direct runs prepare explicit online-learning episodes across every domain", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "direct-learning-response" }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("direct-learning-provider", "https://direct-learning.test", { requiresCredential: false }));
  const memory = new InMemoryAutonomousEpisodicMemory();
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner(), memoryStore: memory });
  agent.registerModel(candidate("direct-learning-provider", "direct-learning-model", [
    "reasoning", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ]));
  const controller = new AutonomousLearningController(agent);
  const tasks = {
    coding: "debug and test this repository",
    browser: "compare current web sources",
    data: "validate dataset schema and lineage",
    science: "design a reproducible experiment",
    biomedical: "review treatment evidence with safety boundaries",
    neuroscience: "analyze EEG preprocessing limits",
    operations: "plan a reversible outage rollback",
    enterprise: "map governance ownership and approvals",
    multi_agent: "delegate a bounded specialist task",
    multimodal: "align an image with a transcript",
    cross_domain: "synthesize interdisciplinary evidence",
    evaluation: "replay a benchmark holdout",
  };
  for (const [domain, task] of Object.entries(tasks)) {
    const result = await agent.run(task, {
      domain,
      approveProviderCall: true,
      learning: controller,
      learningEpisodeId: `direct-learning-${domain}`,
      memoryRunId: `direct-memory-${domain}`,
      memoryLesson: `verified:${domain}`,
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.learning_episode_status, "prepared", domain);
    assert.equal(result.learning_episode_id, `direct-learning-${domain}`, domain);
    assert.equal(result.memory.status, "recorded", domain);
    assert.equal(result.memory.recorded_episode_id, `episode:direct-memory-${domain}`, domain);
    const episode = await controller.episodes.load(result.learning_episode_id);
    assert.equal(episode.status, "pending", domain);
    assert.equal(episode.memory_episode_id, `episode:direct-memory-${domain}`, domain);
    assert.equal(JSON.stringify(episode).includes("direct-learning-response"), false, domain);
    const settlement = await controller.settleRun(result.learning_episode_id, {
      evaluator_id: `${domain}-reviewer`,
      evaluator_version: "1",
      reward: 0.8,
      passed: true,
      evidence_digest: "a".repeat(64),
    }, { outbox: { workerId: `domain-worker-${domain}` } });
    assert.equal(settlement.episode.status, "settled", domain);
    assert.equal(settlement.memory_evaluation.status, "recorded", domain);
    assert.equal(memory.get(`episode:direct-memory-${domain}`).evaluation.reward, 0.8, domain);
  }
  assert.equal(agent.learner.snapshot().generation, 12);
  assert.equal((await memory.stats()).evaluated, 12);
  const replay = await agent.run(tasks.coding, {
    domain: "coding",
    approveProviderCall: true,
    learning: controller,
    learningEpisodeId: "direct-learning-replay",
    memoryRunId: "direct-memory-replay",
    retrieveMemory: false,
  });
  const replayed = await agent.run(tasks.coding, {
    domain: "coding",
    approveProviderCall: true,
    learning: controller,
    learningEpisodeId: "direct-learning-replay",
    memoryRunId: "direct-memory-replay",
    retrieveMemory: false,
  });
  assert.equal(replay.learning_episode_status, "prepared");
  assert.equal(replayed.learning_episode_status, "prepared");
  assert.equal((await controller.episodes.pending()).filter((episode) => episode.episode_id === "direct-learning-replay").length, 1);
});

test("direct learning adapter failures are explicit and do not replay a valid provider result", async () => {
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  const agent = new AutonomousAgent(llm);
  const result = await agent.run("prepare a bounded direct learning episode", {
    domain: "coding",
    approveProviderCall: false,
    learning: { prepareRun: async () => { throw new Error("learning adapter unavailable"); } },
    learningEpisodeId: "direct-learning-failure",
  });
  assert.equal(result.status, "approval_required");
  assert.equal(result.learning_episode_status, "not_eligible");

  const completedLlm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "ok" }, finish_reason: "stop" }] }) });
  completedLlm.registerProvider(openaiCompatibleProvider("direct-learning-provider", "https://direct-learning.test", { requiresCredential: false }));
  const completedAgent = new AutonomousAgent(completedLlm);
  completedAgent.registerModel(candidate("direct-learning-provider", "direct-learning-model", ["reasoning", "code"]));
  const completed = await completedAgent.run("prepare a bounded direct learning episode", {
    domain: "coding",
    approveProviderCall: true,
    learning: { prepareRun: async () => { throw new Error("learning adapter unavailable"); } },
    learningEpisodeId: "direct-learning-failure",
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.learning_episode_status, "failed");
  assert.equal(completed.learning_episode_id, null);
  assert.equal(completed.learning_error_class, "Error");
});

test("provider planning refuses dependency-invalid proposals without retaining provider output", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const message = body.messages.find((row) => row.content.startsWith("Context planning-contract:\n"));
      const contract = JSON.parse(message.content.slice("Context planning-contract:\n".length));
      const ids = contract.stage_catalogue.map((row) => row.id).reverse();
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: [ids[0]], review_required: false, confidence: 1, abstain: false }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("invalid-planner", "https://invalid-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("invalid-planner", "planner-model", ["reasoning", "code", "structured_output"]));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const result = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true });
  assert.equal(result.status, "provider_disagreement");
  assert.equal(result.review_required, true);
  assert.doesNotMatch(JSON.stringify(result), /provider_private_text/);
});

test("provider planning converts malformed structured output into a digest-only refusal", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      const body = JSON.parse(String(init.body));
      const message = body.messages.find((row) => row.content.startsWith("Context planning-contract:\n"));
      const contract = JSON.parse(message.content.slice("Context planning-contract:\n".length));
      const ids = contract.stage_catalogue.map((row) => row.id);
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, focus_stage_ids: [ids[0]], review_required: false, confidence: 1, abstain: false, provider_private_text: "must not be projected" }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("malformed-planner", "https://malformed-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("malformed-planner", "planner-model", ["reasoning", "code", "structured_output"]));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const result = await agent.planWithProvider(blueprint.blueprint, { approveProviderCall: true });
  assert.equal(result.status, "provider_invalid");
  assert.equal(result.review_required, true);
  assert.equal(result.planner_plan_digest, null);
  assert.equal(result.outcome_digest.length, 64);
  assert.doesNotMatch(JSON.stringify(result), /provider_private_text/);
});

test("provider planning rejects a broken blueprint dependency closure before dispatch", async () => {
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
  const blueprint = await agent.blueprint("Debug this coding repository.", { domain: "coding" });
  const malformed = structuredClone(blueprint.blueprint);
  malformed.workflow.stages[0].depends_on = ["missing-stage"];
  await assert.rejects(() => agent.planWithProvider(malformed), /dependencies are not closed/);
});

test("live catalogue binding covers every domain and effectful tools remain approval-gated", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => {
    const binding = profile.tool_profile.bindings[0];
    return [{ name: binding.name, description: `Test ${binding.name}`, inputSchema: { type: "object", additionalProperties: true } }];
  }).map((definition) => [definition.name, definition])).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  assert.equal(plan.coverage.length, 12);
  assert.equal(plan.domains.length, 12);
  assert.equal(plan.available_curated_tools.length, definitions.length);
  assert.equal(plan.secret_material, "never_returned");

  const coding = profiles.find((profile) => profile.domain === "coding");
  const effectfulDefinition = coding.tool_profile.bindings.find((binding) => binding.name === "agent_mission");
  const effectfulCatalogue = await ToolCatalogue.fromDefinitions([{ name: effectfulDefinition.name, description: "Effectful", inputSchema: { type: "object", additionalProperties: true } }]);
  const effectfulRegistry = await AutonomousDomainToolRegistry.create(effectfulCatalogue, [coding.tool_profile]);
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(effectfulRegistry, async () => { executions += 1; return { ok: true }; });
  const refused = await runtime.authorizeAndExecute([{ id: "call-1", name: "agent_mission", arguments: {} }], { domains: ["coding"], approveEffects: false });
  assert.equal(refused[0].approved, false);
  assert.equal(executions, 0);
  const approved = await runtime.authorizeAndExecute([{ id: "call-2", name: "agent_mission", arguments: {} }], { domains: ["coding"], approveEffects: true });
  assert.equal(approved[0].approved, true);
  assert.equal(executions, 1);
});

test("capability portfolio planning covers all domains without widening authority", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({
    name: binding.name,
    description: `Test ${binding.name}`,
    inputSchema: { type: "object", additionalProperties: true },
  }))).map((definition) => [definition.name, definition])).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.planForTask("debug the repository, validate the evidence, verify CI, and report reproducible findings", {
    domains: profiles.map((profile) => profile.domain),
    maxTools: 16,
  });

  assert.equal(plan.schema, AUTONOMOUS_CAPABILITY_PLAN_SCHEMA);
  assert.deepEqual(plan.domains, profiles.map((profile) => profile.domain));
  assert.equal(new Set(plan.coverage.map((row) => row.domain)).size, 12);
  assert.ok(plan.coverage.length >= 48);
  assert.ok(plan.selected_tool_names.length > 0);
  assert.ok(plan.selected_tool_names.length <= 16);
  assert.ok(plan.omissions.length > 0);
  assert.equal(plan.plan_digest.length, 64);
  assert.equal(plan.execution, "metadata_only; no_provider_or_tool_calls");
  assert.equal(plan.authorization, "selection_does_not_authorize_tools_or_effects");
  assert.equal(plan.secret_material, "never_returned");
  assert.doesNotMatch(JSON.stringify(plan), /debug the repository/);
  assert.doesNotMatch(JSON.stringify(plan), /api[_ -]?key|authorization\s*:/i);

  const repeated = await registry.planForTask("debug the repository, validate the evidence, verify CI, and report reproducible findings", {
    domains: profiles.map((profile) => profile.domain),
    maxTools: 16,
  });
  assert.deepEqual(repeated, plan);

  const activationBlocked = await registry.planForTask("review the repository", {
    domains: ["coding"],
    allowedTools: [],
    maxTools: 4,
  });
  assert.deepEqual(activationBlocked.selected_tool_names, []);
  assert.ok(activationBlocked.coverage.some((row) => row.status === "activation_required"));
  assert.ok(activationBlocked.omissions.some((row) => row.reason === "activation_required"));

  const sparseCatalogue = await ToolCatalogue.fromDefinitions([{ name: "repository_catalog", description: "Inspect repository", inputSchema: { type: "object", additionalProperties: true } }]);
  const sparseRegistry = await AutonomousDomainToolRegistry.create(sparseCatalogue, [profiles.find((profile) => profile.domain === "coding").tool_profile]);
  const sparsePlan = await sparseRegistry.planForTask("review this coding repository", { domains: ["coding"], maxTools: 4 });
  assert.ok(sparsePlan.coverage.some((row) => row.status === "catalogue_missing"));
  assert.ok(sparsePlan.missing_tools.length > 0);
});

test("stage-bound adapter execution emits evidence receipts for every reviewed domain", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({
    name: binding.name,
    description: `Stage test ${binding.name}`,
    inputSchema: { type: "object", additionalProperties: true },
  }))).map((definition) => [definition.name, definition])).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue);
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(registry, async () => { executions += 1; return { ok: true }; });

  for (const profile of profiles) {
    const plan = await registry.planForTask(`exercise the reviewed ${profile.domain} workflow`, { domains: [profile.domain], maxTools: 128 });
    const coverage = plan.coverage.find((row) => row.domain === profile.domain && row.status === "selected");
    assert.ok(coverage?.selected_tool, `${profile.domain} must have a live stage-selected tool`);
    const stage = profile.workflow.stages.find((candidate) => candidate.id === coverage.stage_id);
    assert.ok(stage);
    const result = await runtime.authorizeAndExecute([{ id: `stage-${profile.domain}`, name: coverage.selected_tool, arguments: {} }], {
      domains: [profile.domain],
      approveEffects: true,
      workflowContext: {
        domain: profile.domain,
        workflow_id: profile.workflow.workflow_id,
        workflow_digest: profile.workflow.workflow_digest,
        stage_id: stage.id,
      },
    });
    assert.equal(result[0].approved, true, profile.domain);
    const receipt = runtime.receiptsSnapshot().at(-1);
    assert.equal(receipt.domain, profile.domain);
    assert.equal(receipt.workflow_id, profile.workflow.workflow_id);
    assert.equal(receipt.workflow_digest, profile.workflow.workflow_digest);
    assert.equal(receipt.stage_id, stage.id);
    assert.equal(receipt.stage_contract_digest.length, 64);
    assert.deepEqual(receipt.required_evidence_outputs, stage.evidence_outputs);
    assert.equal(receipt.evidence_status, "tool_execution_only");
    assert.equal(Object.prototype.hasOwnProperty.call(receipt, "arguments"), false);
    assert.equal(Object.prototype.hasOwnProperty.call(receipt, "result"), false);
    assert.match(JSON.stringify(receipt), /stage evidence outputs still require evaluator review/);
  }
  assert.equal(executions, profiles.length);
});

test("stage-bound adapter execution rejects a domain-valid but stage-incompatible tool before dispatch", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const coding = profiles.find((profile) => profile.domain === "coding");
  const definitions = coding.tool_profile.bindings.map((binding) => ({ name: binding.name, description: binding.name, inputSchema: { type: "object", additionalProperties: true } }));
  const registry = await AutonomousDomainToolRegistry.create(await ToolCatalogue.fromDefinitions(definitions), [coding.tool_profile]);
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(registry, async () => { executions += 1; return { ok: true }; });
  const refused = await runtime.authorizeAndExecute([{ id: "wrong-stage", name: "developer_platform_status", arguments: {} }], {
    domains: ["coding"],
    approveEffects: true,
    workflowContext: { domain: "coding", workflow_id: coding.workflow.workflow_id, workflow_digest: coding.workflow.workflow_digest, stage_id: "inspect" },
  });
  assert.equal(refused[0].approved, false);
  assert.equal(refused[0].content.status, "execution_failed");
  assert.equal(executions, 0);
  const receipt = runtime.receiptsSnapshot().at(-1);
  assert.equal(receipt.status, "execution_failed");
  assert.equal(receipt.domain, "coding");
  assert.equal(receipt.stage_id, "inspect");
  assert.equal(receipt.capability, null);
});

test("capability execution produces replayable evidence envelopes across every built-in domain", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({
    name: binding.name,
    description: `Capability ${binding.name}`,
    inputSchema: { type: "object", additionalProperties: true },
  }))).map((definition) => [definition.name, definition])).values()];
  const registry = await AutonomousDomainToolRegistry.create(await ToolCatalogue.fromDefinitions(definitions));
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(registry, async (binding) => { executions += 1; return { adapter: binding.name, ok: true }; });
  const capabilities = new AutonomousCapabilityRuntime(runtime);

  for (const profile of profiles) {
    const plan = await registry.planForTask(`execute a reviewed ${profile.domain} capability`, { domains: [profile.domain], maxTools: 128 });
    const coverage = plan.coverage.find((row) => row.domain === profile.domain && row.status === "selected");
    assert.ok(coverage?.selected_tool, `${profile.domain} needs a selected capability`);
    const stage = profile.workflow.stages.find((candidate) => candidate.id === coverage.stage_id);
    const inputDigest = await digestJson({ domain: profile.domain, task: `execute ${profile.domain}` });
    const result = await capabilities.execute({
      call_id: `cap-${profile.domain}`,
      tool: coverage.selected_tool,
      arguments: {},
      workflow_context: { domain: profile.domain, workflow_id: profile.workflow.workflow_id, workflow_digest: profile.workflow.workflow_digest, stage_id: stage.id },
      input_digest: inputDigest,
      subject_digest: await digestJson({ subject: profile.domain }),
    }, { projectObservations: async (value) => {
        const valueDigest = await digestJson(value);
        return stage.evidence_outputs.map((label, index) => ({ id: `e-${index}`, label, kind: "fact", status: "observed", value_digest: valueDigest, confidence: 0.9 }));
      }, approveEffects: true });
    assert.equal(result.schema, AUTONOMOUS_CAPABILITY_EXECUTION_SCHEMA);
    assert.equal(result.record.status, "completed", profile.domain);
    assert.equal(result.record.domain, profile.domain);
    assert.equal(result.record.stage_id, stage.id);
    assert.equal(result.record.stage_contract_digest.length, 64);
    assert.equal(result.record.evidence_status, "declared_for_evaluator");
    assert.deepEqual(result.record.missing_evidence_outputs, []);
    assert.equal(result.record.output_digest.length, 64);
    assert.equal(result.value.ok, true);
    assert.equal(Object.prototype.hasOwnProperty.call(result.record, "arguments"), false);
    assert.equal(Object.prototype.hasOwnProperty.call(result.record, "value"), false);
    assert.equal(Object.prototype.hasOwnProperty.call(result.record, "result"), false);
    assert.match(JSON.stringify(result.record), /require evaluator and provenance review/);
  }
  assert.equal(executions, profiles.length);
  assert.equal(capabilities.executionEvidence().length, profiles.length);
});

test("capability outcomes settle metadata-only evaluator credit across every domain and replay after restart", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({
    name: binding.name,
    description: `Capability ${binding.name}`,
    inputSchema: { type: "object", additionalProperties: true },
  }))).map((definition) => [definition.name, definition])).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue);
  const store = new InMemoryAutonomousCapabilityLearningSettlementStore();
  const learner = new AutonomousOnlineLearner();
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: catalogue,
    toolExecutor: async (binding) => ({ ok: true, domain: binding.domains[0], transient: "must never enter learning" }),
    learner,
    capabilityLearningSettlementStore: store,
  });
  const results = [];
  for (const profile of profiles) {
    const plan = await registry.planForTask(`execute a reviewed ${profile.domain} capability`, { domains: [profile.domain], maxTools: 128 });
    const coverage = plan.coverage.find((row) => row.domain === profile.domain && row.status === "selected");
    assert.ok(coverage?.selected_tool, `${profile.domain} needs a selected capability`);
    const stage = profile.workflow.stages.find((candidate) => candidate.id === coverage.stage_id);
    const result = await agent.executeCapability({
      call_id: `learn-${profile.domain}`,
      tool: coverage.selected_tool,
      arguments: {},
      workflow_context: { domain: profile.domain, workflow_id: profile.workflow.workflow_id, workflow_digest: profile.workflow.workflow_digest, stage_id: stage.id },
      input_digest: await digestJson({ task: `learn ${profile.domain}` }),
    }, {
      approveEffects: true,
      projectObservations: async (value) => {
        const valueDigest = await digestJson(value);
        return stage.evidence_outputs.map((label, index) => ({ id: `learn-${index}`, label, kind: "fact", status: "observed", value_digest: valueDigest, confidence: 0.9 }));
      },
    });
    assert.equal(result.record.status, "completed", profile.domain);
    results.push(result);
  }
  let evaluatorCalls = 0;
  const evaluator = {
    evaluator_id: "capability-quality",
    evaluator_version: "2026-08-21",
    evaluate(input) {
      evaluatorCalls += 1;
      assert.equal(input.value, undefined);
      assert.equal(input.arguments, undefined);
      assert.equal(input.response, undefined);
      assert.equal(input.caller_evidence.quality_gate, "passed");
      assert.doesNotMatch(JSON.stringify(input), /must never enter learning/);
      return { evaluator_id: "capability-quality", evaluator_version: "2026-08-21", reward: 1, passed: true };
    },
  };
  const settled = await agent.evaluateCapabilityExecutions(results, {
    evaluator,
    evidence: Object.fromEntries(results.map((result) => [result.record.request_digest, { quality_gate: "passed" }])),
    armIdFor: (record) => `local-model:${record.domain}`,
  });
  assert.equal(settled.settlements.length, profiles.length);
  assert.equal(learner.snapshot().generation, profiles.length);
  assert.equal(new Set(learner.snapshot().arms.map((arm) => arm.arm_id)).size, profiles.length);
  assert.equal(evaluatorCalls, profiles.length);
  assert.doesNotMatch(JSON.stringify(settled), /must never enter learning/);

  let persistedSnapshot = null;
  const persistence = new AutonomousCapabilityLearningPersistenceCoordinator(store, {
    read: () => persistedSnapshot,
    write: (snapshot) => { persistedSnapshot = structuredClone(snapshot); },
  });
  const flushed = await persistence.flush();
  assert.equal(flushed.receipts.length, profiles.length);
  assert.doesNotMatch(JSON.stringify(flushed), /must never enter learning/);
  const restoredStore = new InMemoryAutonomousCapabilityLearningSettlementStore();
  const restoredPersistence = new AutonomousCapabilityLearningPersistenceCoordinator(restoredStore, {
    read: () => persistedSnapshot,
    write: () => {},
  });
  const restoredSnapshot = await restoredPersistence.restore();
  assert.equal(restoredSnapshot?.snapshot_digest, flushed.snapshot_digest);
  const tamperedSnapshot = structuredClone(flushed);
  tamperedSnapshot.snapshot_digest = "0".repeat(64);
  await assert.rejects(() => validateAutonomousCapabilityLearningSnapshot(tamperedSnapshot), /digest/);
  await assert.rejects(() => restoredStore.restore(tamperedSnapshot), /digest/);
  assert.equal((await restoredStore.snapshot()).snapshot_digest, flushed.snapshot_digest);
  const tamperedReceipt = structuredClone(flushed);
  tamperedReceipt.receipts[0].settlement.reward = 0;
  await assert.rejects(() => validateAutonomousCapabilityLearningSnapshot(tamperedReceipt), /digest/);

  const restartedLearner = new AutonomousOnlineLearner();
  const restarted = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), {
    toolCatalogue: catalogue,
    toolExecutor: async () => ({ ok: true }),
    learner: restartedLearner,
    capabilityLearningSettlementStore: restoredStore,
  });
  const replayed = await restarted.evaluateCapabilityExecutions(results, {
    evaluator,
    evidence: Object.fromEntries(results.map((result) => [result.record.request_digest, { quality_gate: "passed" }])),
    armIdFor: (record) => `local-model:${record.domain}`,
  });
  assert.equal(replayed.settlements.length, profiles.length);
  assert.equal(replayed.settlements.every((settlement) => settlement.idempotent_replay), true);
  assert.equal(evaluatorCalls, profiles.length);
  assert.equal(restartedLearner.snapshot().generation, profiles.length);
  assert.equal(replayed.settlements[0].next_state.arms[0].pulls, 1);

  const uncertain = { ...results[0].record, status: "reconciliation_required", output_digest: null, output_bytes: 0, observations: [], evidence_digest: null, evidence_status: "not_evaluated", missing_evidence_outputs: [...results[0].record.required_evidence_outputs] };
  await assert.rejects(() => agent.evaluateCapabilityExecution(uncertain, { evaluator, callerEvidence: { quality_gate: "failed" }, armId: "local-model:reconciled" }), /reconciliation_required/);
  const reconciled = await agent.evaluateCapabilityExecution(uncertain, { evaluator: { ...evaluator, evaluate: () => ({ evaluator_id: "capability-quality", evaluator_version: "2026-08-21", reward: -1, passed: false }) }, callerEvidence: { quality_gate: "failed" }, armId: "local-model:reconciled", allowReconciliation: true });
  assert.equal(reconciled.failed, true);
});

test("capability execution fails closed, replays completed work, and makes batch omissions explicit", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const coding = profiles.find((profile) => profile.domain === "coding");
  const plan = await AutonomousDomainToolRegistry.create(await ToolCatalogue.fromDefinitions(coding.tool_profile.bindings.map((row) => ({ name: row.name, description: row.name, inputSchema: { type: "object", additionalProperties: true } }))));
  const selected = (await plan.planForTask("inspect and verify this coding repository", { domains: ["coding"], maxTools: 128 })).coverage.find((row) => row.domain === "coding" && row.status === "selected");
  const binding = coding.tool_profile.bindings.find((row) => row.name === selected.selected_tool);
  const registry = plan;
  let executions = 0;
  const runtime = new AutonomousDomainToolRuntime(registry, async () => { executions += 1; return { ok: true }; });
  const capabilities = new AutonomousCapabilityRuntime(runtime);
  const context = { domain: "coding", workflow_id: coding.workflow.workflow_id, workflow_digest: coding.workflow.workflow_digest, stage_id: selected.stage_id };
  const base = { call_id: "replayable", tool: binding.name, arguments: {}, workflow_context: context, input_digest: await digestJson({ task: "inspect" }), replay_key: "replayable-key" };
  const first = await capabilities.execute(base);
  const replay = await capabilities.execute(base);
  assert.equal(first.record.status, "completed");
  assert.equal(replay.record.replay, "replayed");
  assert.equal(executions, 1);
  assert.equal(replay.record.output_digest, first.record.output_digest);

  const concurrentRequest = { ...base, call_id: "concurrent", replay_key: "concurrent-key" };
  const [concurrentFirst, concurrentReplay] = await Promise.all([
    capabilities.execute(concurrentRequest),
    capabilities.execute(concurrentRequest),
  ]);
  assert.equal(concurrentFirst.record.status, "completed");
  assert.deepEqual(
    [concurrentFirst.record.replay, concurrentReplay.record.replay].sort(),
    ["fresh", "replayed"],
    "identical concurrent requests must produce one fresh result and one replay",
  );
  assert.equal(executions, 2, "identical concurrent requests must share one adapter dispatch");

  const wrongStage = await capabilities.execute({ ...base, call_id: "wrong-stage", replay_key: "wrong-stage", workflow_context: { ...context, stage_id: selected.stage_id === "implementation" ? "scope" : "implementation" } });
  assert.equal(wrongStage.record.status, "refused");
  assert.equal(executions, 2);

  const batch = await capabilities.executeBatch([
    { ...base, call_id: "batch-1", replay_key: "batch-1" },
    { ...base, call_id: "batch-2", replay_key: "batch-2", workflow_context: { ...context, stage_id: selected.stage_id === "implementation" ? "scope" : "implementation" } },
    { ...base, call_id: "batch-3", replay_key: "batch-3" },
  ], { stopOnFailure: true });
  assert.equal(batch.status, "partial");
  assert.equal(batch.completed_count, 1);
  assert.equal(batch.failed_count, 1);
  assert.equal(batch.omitted_count, 1);
  assert.equal(batch.items[2].omission_reason, "stopped_after_failure");
  assert.equal(executions, 3);
});

test("capability journal rehydrates replay identity without retaining adapter values", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const coding = profiles.find((profile) => profile.domain === "coding");
  const binding = coding.tool_profile.bindings.find((row) => row.name === "conformance_run");
  const definitions = [{ name: binding.name, description: "Conformance", inputSchema: { type: "object", additionalProperties: true } }];
  const registry = await AutonomousDomainToolRegistry.create(await ToolCatalogue.fromDefinitions(definitions));
  let executions = 0;
  const executor = async () => { executions += 1; return { checked: true, transient: "never stored" }; };
  const runtime = new AutonomousDomainToolRuntime(registry, executor);
  const journal = new InMemoryAutonomousCapabilityJournalStore();
  const capabilities = new AutonomousCapabilityRuntime(runtime, { journal });
  const request = {
    call_id: "journal-replay",
    tool: binding.name,
    arguments: { scope: "repository" },
    workflow_context: { domain: "coding", workflow_id: coding.workflow.workflow_id, workflow_digest: coding.workflow.workflow_digest, stage_id: "scope" },
    input_digest: await digestJson({ task: "journal restart" }),
    replay_key: "journal-replay-key",
  };
  const first = await capabilities.execute(request);
  assert.equal(first.record.status, "completed");
  assert.equal(first.value.transient, "never stored");
  const snapshot = await journal.snapshot();
  assert.equal(snapshot.entries.length, 1);
  assert.equal(snapshot.entries[0].record.replay, "fresh");
  assert.equal(Object.prototype.hasOwnProperty.call(snapshot.entries[0].record, "value"), false);

  const restoredJournal = new InMemoryAutonomousCapabilityJournalStore();
  await restoredJournal.restore(snapshot);
  const restarted = new AutonomousCapabilityRuntime(new AutonomousDomainToolRuntime(registry, executor), { journal: restoredJournal });
  const restoreReceipt = await restarted.rehydrate();
  assert.deepEqual(restoreReceipt, { restored: 1, replayable: 1, value_retention: "transient_caller_value_only" });
  const replay = await restarted.execute(request);
  assert.equal(replay.record.replay, "replayed");
  assert.equal(replay.record.output_digest, first.record.output_digest);
  assert.equal(replay.value, null);
  assert.match(replay.value_retention, /transient/);
  assert.equal(executions, 1, "rehydration must not redispatch the external tool");

  const tampered = structuredClone(snapshot);
  tampered.entries[0].record.value = { leaked: true };
  await assert.rejects(() => validateAutonomousCapabilityJournalSnapshot(tampered), AutonomousCapabilityPersistenceError);

  let persisted = null;
  const coordinator = new AutonomousCapabilityJournalPersistenceCoordinator(journal, {
    async read() { return persisted; },
    async write(value) { persisted = structuredClone(value); },
  });
  const flushed = await coordinator.flush();
  assert.equal(flushed.retention, "metadata_only");
  assert.equal(flushed.snapshot_digest, snapshot.snapshot_digest);
  const empty = new InMemoryAutonomousCapabilityJournalStore();
  const restoreCoordinator = new AutonomousCapabilityJournalPersistenceCoordinator(empty, {
    async read() { return persisted; },
    async write() {},
  });
  const restored = await restoreCoordinator.restore();
  assert.equal(restored.restored, true);
  assert.equal(restored.entry_count, 1);
});

test("AutonomousAgent exposes capability records through its activation-aware runtime", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const coding = profiles.find((profile) => profile.domain === "coding");
  const binding = coding.tool_profile.bindings.find((row) => row.name === "conformance_run");
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  let executions = 0;
  const agent = new AutonomousAgent(llm, {
    toolCatalogue: await ToolCatalogue.fromDefinitions([{ name: binding.name, description: "Conformance", inputSchema: { type: "object", additionalProperties: true } }]),
    toolExecutor: async () => { executions += 1; return { checked: true }; },
  });
  const result = await agent.executeCapability({
    call_id: "agent-capability",
    tool: binding.name,
    arguments: {},
    workflow_context: { domain: "coding", workflow_id: coding.workflow.workflow_id, workflow_digest: coding.workflow.workflow_digest, stage_id: "scope" },
    input_digest: await digestJson({ task: "conformance" }),
  });
  assert.equal(result.record.status, "completed");
  assert.equal(executions, 1);
  assert.equal(agent.capabilityExecutionEvidence().length, 1);
  assert.equal(agent.toolExecutionEvidence().length, 1);
});

test("ApiClient is a key-agnostic live executor bridge for reviewed capability calls", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => ({ name: binding.name, description: binding.name, inputSchema: { type: "object", additionalProperties: true } }))).map((definition) => [definition.name, definition])).values()];
  let transportCalls = 0;
  const client = new ApiClient({
    baseUrl: "https://prism.test",
    fetch: async (url, init) => {
      transportCalls += 1;
      const tool = new URL(String(url)).pathname.split("/").at(-1);
      assert.equal(typeof tool, "string");
      assert.equal(typeof JSON.parse(String(init.body)), "object");
      return jsonResponse({ ok: true, tool, request_id: `api-call-${transportCalls}`, guarantee: "bounded", mcp: { result: { structuredContent: { checked: true, source: "api", tool } } } });
    },
  });
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const explicitExecutor = createAutonomousApiToolExecutor(client, { catalogue });
  assert.equal(typeof explicitExecutor, "function");
  const llm = new LLMRuntime({ credentials: new CredentialStore() });
  const agent = new AutonomousAgent(llm, { apiClient: client, toolCatalogue: catalogue });
  const registry = await AutonomousDomainToolRegistry.create(catalogue);
  for (const profile of profiles) {
    const plan = await registry.planForTask(`exercise the live ${profile.domain} API capability`, { domains: [profile.domain], maxTools: 128 });
    const coverage = plan.coverage.find((row) => row.domain === profile.domain && row.status === "selected");
    assert.ok(coverage?.selected_tool, `${profile.domain} must have a stage-compatible API tool`);
    const stage = profile.workflow.stages.find((candidate) => candidate.id === coverage.stage_id);
    const result = await agent.executeCapability({
      call_id: `api-${profile.domain}`,
      tool: coverage.selected_tool,
      arguments: {},
      workflow_context: { domain: profile.domain, workflow_id: profile.workflow.workflow_id, workflow_digest: profile.workflow.workflow_digest, stage_id: stage.id },
      input_digest: await digestJson({ task: `live API bridge ${profile.domain}` }),
    });
    assert.equal(result.record.status, "completed", profile.domain);
    assert.equal(result.value.checked, true, profile.domain);
    assert.equal(result.value.tool, coverage.selected_tool, profile.domain);
    assert.equal(result.record.domain, profile.domain);
    assert.equal(result.record.secret_material, "never_returned");
    assert.equal(JSON.stringify(result.record).includes("bearer"), false);
  }
  assert.equal(transportCalls, profiles.length);
  assert.equal(AUTONOMOUS_API_TOOL_ADAPTER_SCHEMA, "bioprism-typescript-autonomous-api-tool-adapter/0.1");
});

test("AutonomousAgent performs a real selected-provider tool loop with domain policy", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      if (calls === 1) return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "tool-1", type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "repository inspected" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://autonomous.test", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const definition = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings.find((binding) => binding.name === "repository_catalog");
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: definition.name, description: "Inspect repository", inputSchema: { type: "object", additionalProperties: true } }]);
  const agent = new AutonomousAgent(llm, {
    toolCatalogue: catalogue,
    toolExecutor: async (tool) => ({ tool: tool.name, files: ["README.md"] }),
  });
  agent.registerModel(candidate("local", "local-model"));
  const result = await agent.run("Debug this Rust repository and report the tests", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.tool_loop.toolCalls, 1);
  assert.equal(result.response.text, "repository inspected");
  assert.equal(bodies[1].messages.at(-1).role, "tool");
  assert.equal(bodies[1].messages.at(-1).content, JSON.stringify({ tool: "repository_catalog", files: ["README.md"] }));
});

test("AutonomousAgent preserves authorization pauses instead of reporting tool success", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: "approval-tool-1", type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("approval-loop", "https://approval-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("approval-loop", "approval-model"));
  const result = await agent.run("Review this repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }],
    authorizeAndExecute: async () => [{ callId: "approval-tool-1", approved: false, isError: true, content: { status: "authorization_required", secret_material: "never_returned" } }],
  });
  assert.equal(result.status, "approval_required");
  assert.equal(result.tool_loop.status, "authorization_required");
});

test("AutonomousAgent reports bounded tool-loop exhaustion instead of completed", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: `limit-tool-${calls}`, type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("limit-loop", "https://limit-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("limit-loop", "limit-model"));
  const result = await agent.run("Review this repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }],
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.tool_loop.status, "turn_limit_reached");
  assert.equal(result.tool_loop.turns, 4);
  assert.equal(calls, 4);
});

test("cross-domain execution fans out to specialists, gates approval, and synthesizes bounded local outputs", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      const text = calls === 1 ? "biomedical evidence finding" : calls === 2 ? "neuroscience signal finding" : "integrated biomedical-neuroscience conclusion";
      return jsonResponse({ choices: [{ message: { role: "assistant", content: text }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cross", "https://cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const capabilities = ["reasoning", "coordination", "code", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multimodal", "evaluation"];
  agent.registerModel(candidate("cross", "cross-model", capabilities));
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
  const preview = await agent.blueprint(task);
  assert.equal(preview.route.cross_domain, true);
  assert.ok(preview.cross_domain_blueprint);
  assert.equal(preview.cross_domain_blueprint.route_digest, preview.route.route_digest);
  assert.ok(preview.cross_domain_blueprint.child_blueprints.every((child) => child.route_digest === preview.route.route_digest));
  assert.equal(preview.cross_domain_blueprint.child_blueprints.length, preview.route.selected_domains.length);
  assert.equal(preview.cross_domain_blueprint.execution, "not_started");
  const gated = await agent.run(task, { candidates: agent.models() });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.cross_domain?.status, "approval_required");
  assert.equal(calls, 0);

  const result = await agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review the biomedical evidence and safety boundary." },
      { id: "neuro", domain: "neuroscience", task: "Analyze the EEG neuroscience design and signal limits." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(result.route.cross_domain, true);
  assert.deepEqual(result.blueprint.child_ids, ["bio", "neuro"]);
  assert.equal(result.child_runs.length, 2);
  assert.equal(result.completed_children, 2);
  assert.equal(result.synthesis.response.text, "integrated biomedical-neuroscience conclusion");
  assert.equal(calls, 3);
  const synthesisBody = bodies[2];
  assert.ok(synthesisBody.messages.some((message) => String(message.content).includes("biomedical evidence finding")));
  assert.ok(synthesisBody.messages.some((message) => String(message.content).includes("neuroscience signal finding")));
  await assert.rejects(agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    maxTotalCostUnits: 0,
    synthesize: false,
    subtasks: [
      { id: "bio-budget", domain: "biomedical", task: "Review the biomedical evidence." },
      { id: "neuro-budget", domain: "neuroscience", task: "Review the neuroscience evidence." },
    ],
  }), (error) => error instanceof AutonomousCostBudgetError);
  assert.equal(calls, 3, "aggregate budget refusal must happen before another provider dispatch");
});

test("cross-domain structured output propagates through specialists and synthesis", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ answer: `structured-${calls}` }) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("structured-cross", "https://structured-cross.test", { requiresCredential: false, structuredOutputMode: "json_object" }));
  const agent = new AutonomousAgent(llm);
  const model = candidate("structured-cross", "structured-cross-model", ["reasoning", "coordination", "biomedical", "science", "structured_output"]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await agent.runCrossDomain("Research a biomedical neuroscience experiment with EEG patient evidence", {
    candidates: [model],
    approveProviderCall: true,
    requireJson: true,
    responseSchema,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review the biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Analyze the neuroscience signal limits." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(calls, 3);
  assert.deepEqual(result.child_runs.map((child) => child.result.response.structured), [{ answer: "structured-1" }, { answer: "structured-2" }]);
  assert.deepEqual(result.synthesis.response.structured, { answer: "structured-3" });
  assert.deepEqual(bodies.map((body) => body.response_format), [{ type: "json_object" }, { type: "json_object" }, { type: "json_object" }]);
});

test("cross-domain fan-out uses bounded concurrency and preserves deterministic child order", async () => {
  let active = 0;
  let maximumActive = 0;
  let started = 0;
  let release;
  let resolveStarted;
  const releaseGate = new Promise((resolve) => { release = resolve; });
  const startedGate = new Promise((resolve) => { resolveStarted = resolve; });
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      active += 1;
      maximumActive = Math.max(maximumActive, active);
      started += 1;
      if (started === 2) resolveStarted();
      await releaseGate;
      active -= 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "bounded specialist result" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("parallel", "https://parallel.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("parallel", "parallel-model", ["reasoning", "science", "coordination", "biomedical", "neuroscience"]));
  const runPromise = agent.runCrossDomain("Research a biomedical neuroscience study", {
    candidates: agent.models(),
    approveProviderCall: true,
    synthesize: false,
    maxParallelChildren: 2,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience signals." },
    ],
  });
  const observedParallelism = await Promise.race([
    startedGate.then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), 250)),
  ]);
  release();
  const result = await runPromise;
  assert.equal(observedParallelism, true);
  assert.equal(maximumActive, 2);
  assert.equal(result.status, "children_completed");
  assert.deepEqual(result.child_runs.map((child) => child.id), ["bio", "neuro"]);
  assert.equal(result.completed_children, 2);
  await assert.rejects(
    agent.runCrossDomain("Research a biomedical neuroscience study", {
      candidates: agent.models(),
      approveProviderCall: true,
      synthesize: false,
      maxParallelChildren: 5,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review neuroscience signals." },
      ],
    }),
    (error) => error?.name === "ArgumentError",
  );
});

test("accepted cross-domain plan refinement reorders bounded fan-out and carries digest metadata", async () => {
  const bodies = [];
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: `accepted-cross-child-${calls}` }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("accepted-cross", "https://accepted-cross.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("accepted-cross", "accepted-cross-model", ["reasoning", "coordination", "biomedical", "neuroscience", "science"]));
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
  const preview = await agent.blueprint(task);
  assert.ok(preview.cross_domain_blueprint);
  const blueprint = preview.cross_domain_blueprint;
  const acceptedPlan = {
    schema: "bioprism-python-autonomous-cross-domain-plan-refinement/0.1",
    status: "completed",
    task_digest: blueprint.task_digest,
    base_plan_digest: blueprint.plan_digest,
    priority_child_ids: [...blueprint.child_ids].reverse(),
    focus_child_ids: [blueprint.child_ids.at(-1)],
    review_required: false,
    confidence: 0.94,
    selected_model: { provider: "accepted-cross", model: "accepted-cross-model" },
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "child_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_or_effects_authorized",
  };
  const acceptedPlanDigest = await digestJson(acceptedPlan);
  const result = await agent.runCrossDomain(task, {
    candidates: agent.models(),
    approveProviderCall: true,
    synthesize: false,
    maxParallelChildren: 1,
    acceptedCrossDomainPlanRefinement: acceptedPlan,
  });
  assert.equal(result.status, "children_completed");
  assert.deepEqual(result.child_runs.map((child) => child.id), [...blueprint.child_ids].reverse());
  assert.equal(result.plan_refinement_digest, acceptedPlanDigest);
  assert.equal(calls, blueprint.child_ids.length);
  assert.match(bodies[0].messages.find((message) => message.content.startsWith("Context accepted-cross-domain-plan:\n"))?.content ?? "", /priority_rank/);

  const invalidPlan = { ...acceptedPlan, base_plan_digest: "0".repeat(64) };
  await assert.rejects(
    () => agent.runCrossDomain(task, { candidates: agent.models(), approveProviderCall: true, synthesize: false, acceptedCrossDomainPlanRefinement: invalidPlan }),
    /base does not match/,
  );
  assert.equal(calls, blueprint.child_ids.length, "invalid accepted plans must fail before child dispatch");
});

test("online learner adapts only from explicit evaluator rewards", async () => {
  const learner = new AutonomousOnlineLearner();
  const request = {
    task: "choose a reasoning model",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    candidates: [candidate("a", "one"), candidate("b", "two")],
    provider_health: {
      a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
      b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
    },
    model_health: {},
  };
  const first = learner.select(request);
  assert.equal(first.selected_model.provider, "a");
  learner.update({ arm_id: "b/two", reward: 1 });
  learner.update({ arm_id: "a/one", reward: 0.1 });
  const second = learner.select(request);
  assert.equal(second.selected_model.provider, "b");
  assert.equal(learner.snapshot().generation, 2);
  const constrained = learner.select({ ...request, max_latency_ms: 50 });
  assert.equal(constrained.selected_model, null);
  assert.match(constrained.abstention_reason, /no eligible candidate/);
  assert.equal(constrained.ranking.length, 2);
  assert.equal(constrained.ranking.every((row) => row.eligible === false), true);
  assert.match(constrained.ranking[0].reasons.join(";"), /latency exceeds the caller bound/);
  assert.throws(() => learner.select({ ...request, min_quality: 2 }), /min_quality is outside its bounds/);
});

test("selection confidence abstains on ambiguous ranking across every built-in domain", () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  for (const domain of domains) {
    const learner = new AutonomousOnlineLearner();
    const decision = learner.select({
      task: `choose a model for ${domain}`,
      domain,
      capability: "reasoning",
      risk_class: "review_required",
      required_capabilities: ["reasoning"],
      estimated_input_tokens: 10,
      requested_output_tokens: 50,
      min_selection_confidence: 0.1,
      candidates: [candidate("a", "same-prior"), candidate("b", "same-prior")],
      provider_health: {
        a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
        b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
      },
      model_health: {},
    });
    assert.equal(decision.selected_model, null, domain);
    assert.equal(decision.selection_confidence, 0, domain);
    assert.equal(decision.min_selection_confidence, 0.1, domain);
    assert.match(decision.abstention_reason, /selection confidence/, domain);
  }
  assert.throws(() => learnerSelectConfidenceFailure(), /min_selection_confidence is outside its bounds/);
});

function learnerSelectConfidenceFailure() {
  return new AutonomousOnlineLearner().select({
    task: "invalid confidence",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    min_selection_confidence: 2,
    candidates: [candidate("a", "one")],
    provider_health: { a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true } },
    model_health: {},
  });
}

test("autonomous invocation preserves learner exploration and ranking evidence", async () => {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "selected through the learner" }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("a", "https://learner-a.test", { requiresCredential: false }));
  llm.registerProvider(openaiCompatibleProvider("b", "https://learner-b.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, {
    learner: new AutonomousOnlineLearner({ policy: { strategy: "epsilon_greedy", epsilon: 1, seed: 7 } }),
  });
  agent.registerModel(candidate("a", "one"));
  agent.registerModel(candidate("b", "two"));
  agent.learner.update({ arm_id: "a/one", reward: 0.2 });
  agent.learner.update({ arm_id: "b/two", reward: 0.8 });
  const result = await agent.run("Choose a model for this bounded coding task.", { domain: "coding", approveProviderCall: true });
  assert.equal(result.status, "completed");
  assert.equal(result.selection.exploration_taken, true);
  assert.equal(typeof result.selection.exploration_draw, "number");
  assert.ok(result.selection.ranking.some((row) => row.reasons.some((reason) => reason.startsWith("history="))));
});

test("online learner honors seeded epsilon exploration, failure penalties, and signed rewards", () => {
  const request = {
    task: "choose a reasoning model",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    candidates: [candidate("a", "one"), candidate("b", "two")],
    provider_health: {
      a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
      b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
    },
    model_health: {},
  };
  const learner = new AutonomousOnlineLearner({ policy: { strategy: "epsilon_greedy", epsilon: 1, seed: 7, failure_penalty: 1 } });
  learner.update({ arm_id: "a/one", reward: -0.5, failed: true, outcome_digest: "5".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 0.8, outcome_digest: "6".repeat(64) });
  const decision = learner.select(request);
  assert.equal(decision.exploration_taken, true);
  assert.match(String(decision.exploration_draw), /^0\./);
  assert.equal(learner.snapshot().policy.strategy, "epsilon_greedy");
  assert.ok(decision.ranking.find((row) => row.provider === "a").reasons.some((reason) => reason.startsWith("failure_rate=")));
  const disabled = new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, policy: { strategy: "ucb1" }, arms: [{ arm_id: "a/one", disabled: true }] } });
  const disabledDecision = disabled.select(request);
  assert.deepEqual(disabledDecision.selected_model, { provider: "b", model: "two" });
  assert.match(disabledDecision.ranking.find((row) => row.provider === "a").reasons.join(";"), /bandit arm is disabled/);
});

test("online learner supports deterministic Thompson posteriors with auditable evidence for every domain", () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  for (const [domainIndex, domain] of domains.entries()) {
    const request = {
      task: `choose a reasoning model for ${domain}`,
      domain,
      capability: "reasoning",
      risk_class: "bounded_review",
      required_capabilities: ["reasoning"],
      estimated_input_tokens: 10,
      requested_output_tokens: 50,
      candidates: [candidate("a", "one"), candidate("b", "two")],
      provider_health: {
        a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
        b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
      },
      model_health: {},
    };
    const learner = new AutonomousOnlineLearner({ policy: { strategy: "thompson_sampling", seed: 19 } });
    learner.update({ arm_id: "a/one", reward: 0.9, outcome_digest: "7".repeat(63) + domainIndex.toString(16) });
    learner.update({ arm_id: "b/two", reward: -0.5, failed: true, outcome_digest: "8".repeat(63) + domainIndex.toString(16) });
    const first = learner.select(request);
    const replay = learner.select(request);
    assert.deepEqual(first, replay, domain);
    assert.equal(first.exploration_taken, true, domain);
    assert.equal(first.exploration_draw, null, domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_alpha="))), domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_beta="))), domain);
    assert.ok(first.ranking.every((row) => row.reasons.some((reason) => reason.startsWith("posterior_sample="))), domain);
    assert.equal(learner.snapshot().policy.strategy, "thompson_sampling", domain);
  }
});

test("online learner isolates evaluator rewards by domain learning context", async () => {
  const learner = new AutonomousOnlineLearner();
  const request = {
    task: "choose a reasoning model",
    domain: "coding",
    capability: "implementation",
    risk_class: "engineering_change",
    task_family: "coding_delivery",
    required_capabilities: ["reasoning"],
    estimated_input_tokens: 10,
    requested_output_tokens: 50,
    candidates: [candidate("a", "one"), candidate("b", "two")],
    provider_health: {
      a: { provider: "a", circuit: "closed", credential_required: false, credential_ready: true },
      b: { provider: "b", circuit: "closed", credential_required: false, credential_ready: true },
    },
    model_health: {},
  };
  const codingContext = { domain: "coding", capability: "implementation", risk_class: "engineering_change", task_family: "coding_delivery" };
  const biomedicalContext = { domain: "biomedical", capability: "biomedical_review", risk_class: "biomedical_safety", task_family: "biomedical_review" };
  const codingDigest = learningContextDigest(codingContext);
  const biomedicalDigest = learningContextDigest(biomedicalContext);
  learner.update({ arm_id: "a/one", reward: 1, context_digest: codingDigest, context: codingContext, outcome_digest: "1".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 0, context_digest: codingDigest, context: codingContext, outcome_digest: "2".repeat(64) });
  learner.update({ arm_id: "a/one", reward: 0, context_digest: biomedicalDigest, context: biomedicalContext, outcome_digest: "3".repeat(64) });
  learner.update({ arm_id: "b/two", reward: 1, context_digest: biomedicalDigest, context: biomedicalContext, outcome_digest: "4".repeat(64) });
  const state = learner.snapshot();
  assert.equal(state.arms.length, 0, "contextual rewards must not pollute the legacy global arm ledger");
  assert.deepEqual(state.contextual_states.map((row) => row.context_digest), [codingDigest, biomedicalDigest]);
  assert.equal(learner.select({ ...request, context_digest: codingDigest }).selected_model.provider, "a");
  assert.equal(learner.select({ ...request, domain: "biomedical", capability: "biomedical_review", risk_class: "biomedical_safety", task_family: "biomedical_review", context_digest: biomedicalDigest }).selected_model.provider, "b");
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.2, context_digest: codingDigest, context: codingContext, outcome_digest: "1".repeat(64) }), /contradictory evaluator evidence/);
});

test("online learner rejects malformed contextual snapshots with typed errors", () => {
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [], contextual_states: [null] } }), /bandit contextual state must contain context and arms/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [{ arm_id: "a/one", pulls: 1, reward_sum: 2 }] } }), /online learner arm is malformed/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: -1, arms: [] } }), /generation must be a non-negative safe integer/);
  assert.throws(() => new AutonomousOnlineLearner({ state: { schema: "test", generation: 0, arms: [{ arm_id: "a/one" }, { arm_id: "a/one" }] } }), /arm a\/one is duplicated/);
  const learner = new AutonomousOnlineLearner();
  assert.throws(() => learner.restore({ schema: "test", generation: 0, policy: { epsilon: 0.9 }, arms: [] }), /remote policy epsilon conflicts/);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.5, context: { domain: "coding", capability: "implementation", risk_class: "engineering_change" } }), /context requires a context_digest/);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.5, context_digest: "0".repeat(64), context: { domain: "coding", capability: "implementation", risk_class: "engineering_change" } }), /does not match its context identity/);
});

test("every built-in domain blueprint binds a distinct bounded learning context", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("blueprint must not invoke a provider"); } });
  const agent = new AutonomousAgent(runtime);
  const profiles = await builtinAutonomousDomainProfiles();
  const blueprints = await Promise.all(profiles.map((profile) => agent.blueprint(`Review the ${profile.domain} workflow.`, { domain: profile.domain })));
  const digests = blueprints.map((row) => row.blueprint.learning_context_digest);
  assert.equal(new Set(digests).size, profiles.length);
  assert.ok(digests.every((digest) => /^[0-9a-f]{64}$/.test(digest)));
  assert.deepEqual(blueprints.map((row) => row.blueprint.selection_context.domain).sort(), profiles.map((profile) => profile.domain).sort());
});

test("online learner does not double-credit a replayed evaluator outcome", async () => {
  const learner = new AutonomousOnlineLearner();
  const outcomeDigest = "a".repeat(64);
  const first = learner.update({ arm_id: "a/one", reward: 0.8, outcome_digest: outcomeDigest });
  const replay = learner.update({ arm_id: "a/one", reward: 0.8, outcome_digest: outcomeDigest });
  assert.equal(first.generation, 1);
  assert.deepEqual(replay, first);
  assert.equal(replay.arms[0].pulls, 1);
  assert.deepEqual(replay.credited_outcomes, [{ outcome_digest: outcomeDigest, arm_id: "a/one", reward: 0.8, failed: false, contract_digest: null }]);
  assert.throws(() => learner.update({ arm_id: "a/one", reward: 0.1, outcome_digest: outcomeDigest }), /contradictory evaluator evidence/);
});

test("remote evaluator reward adopts the projected state instead of replaying local credit", async () => {
  const apiClient = {
    async brainModelSelectContextual() {
      throw new Error("remote state test must not select a model");
    },
    async brainBanditUpdate(state, update) {
      assert.equal(state.generation, 0);
      assert.equal(update.arm_id, "remote/model");
      return {
        ok: true,
        mcp: {
          result: {
            structuredContent: {
              schema: "bioprism-brain-bandit/0.1",
              generation: 12,
              arms: [{ arm_id: "remote/model", pulls: 4, reward_sum: -0.2, failures: 2 }],
              credited_outcomes: [],
            },
          },
        },
      };
    },
  };
  const agent = new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("remote state test must not invoke a provider"); } }), {
    apiClient,
    learner: new AutonomousOnlineLearner(),
  });
  const projected = await agent.recordEvaluatorReward("remote/model", 0.9, { remote: true });
  assert.equal(projected.generation, 12);
  assert.deepEqual(agent.learner.snapshot().arms, [{ arm_id: "remote/model", pulls: 4, reward_sum: -0.2, failures: 2 }]);
});

test("contextual selector bridge sends only model and health metadata to the control plane", async () => {
  let received;
  const apiClient = {
    async brainModelSelectContextual(args) {
      received = args;
      return { ok: true, mcp: { result: { structuredContent: { selection: { selected_model_id: "remote/remote-model", selection_status: "selected" } } } } };
    },
  };
  const llm = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => jsonResponse({ choices: [{ message: { role: "assistant", content: "remote answer" }, finish_reason: "stop" }] }) });
  llm.registerProvider(openaiCompatibleProvider("remote", "https://remote.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { apiClient });
  agent.registerModel(candidate("remote", "remote-model"));
  const result = await agent.run("Implement this code change", { domain: "coding", approveProviderCall: true });
  assert.equal(result.response.text, "remote answer");
  assert.equal(received.context.domain, "coding");
  assert.equal(received.base.models[0].model_id, "remote/remote-model");
  assert.equal(received.base.models[0].provider, "remote");
  assert.equal(received.base.models[0].authorization, undefined);
});

test("contextual selector abstains on an ambiguous model-only id without dispatch", async () => {
  let calls = 0;
  const apiClient = {
    async brainModelSelectContextual() {
      return { ok: true, mcp: { result: { structuredContent: { selection: { selected_model_id: "shared-model", selection_status: "selected" } } } } };
    },
  };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "must not dispatch" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("provider-a", "https://provider-a.test", { requiresCredential: false }));
  llm.registerProvider(openaiCompatibleProvider("provider-b", "https://provider-b.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { apiClient });
  agent.registerModel(candidate("provider-a", "shared-model"));
  agent.registerModel(candidate("provider-b", "shared-model"));
  await assert.rejects(agent.run("Implement this code change", { domain: "coding", approveProviderCall: true }), /ambiguous model id/);
  assert.equal(calls, 0);
});

test("keyless readiness audits every built-in domain without contacting providers", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const report = await agent.readiness();

  assert.equal(report.schema, AUTONOMOUS_READINESS_SCHEMA);
  assert.equal(report.domains.length, 12);
  assert.equal(new Set(report.domains.map((row) => row.domain)).size, 12);
  assert.ok(report.domains.every((row) => row.state === "ready_for_caller_approval"));
  assert.equal(report.readiness_state, "ready_for_caller_approval");
  assert.deepEqual(report.models[0].compatible_domains, profiles.map((profile) => profile.domain));
  assert.deepEqual(report.models[0].eligible_domains, profiles.map((profile) => profile.domain));
  assert.equal(report.learning.configured, false);
  assert.equal(report.tooling.configured, false);
  assert.equal(report.execution, "not_started; no_provider_or_tool_calls");
  assert.equal(report.secret_material, "never_returned");
  assert.match(report.readiness_digest, /^[0-9a-f]{64}$/);
  assert.match(JSON.stringify(report), /attach AutonomousOnlineLearner/);
  assert.doesNotMatch(JSON.stringify(report), /api_key|Bearer|sk-|test-secret/i);
  assert.equal(fetchCalls, 0);
});

test("keyless readiness composes all-domain evidence routing posture without source dispatch", async () => {
  let fetchCalls = 0;
  let sourceCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("evidence readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({
    adapterId: "readiness.all-domains",
    version: "1.0.0",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: async () => {
      sourceCalls += 1;
      return { must_not_be_acquired: true };
    },
  });
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const report = await agent.readiness({
    evidenceReadiness: {
      registry,
      options: { policy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }) },
    },
  });

  assert.equal(report.evidence?.status, "degraded");
  assert.equal(report.evidence?.ready_count, 0);
  assert.equal(report.evidence?.degraded_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.evidence?.blocked_count, 0);
  assert.equal(report.evidence?.missing_count, 0);
  assert.equal(report.evidence?.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(report.evidence?.registry_digest.match(/^[0-9a-f]{64}$/));
  assert.ok(report.evidence?.report_digest.match(/^[0-9a-f]{64}$/));
  assert.ok(report.domains.every((row) => row.state === "partial"));
  assert.ok(report.domains.every((row) => row.evidence_readiness?.status === "degraded"));
  assert.match(JSON.stringify(report), /resolve evidence routing readiness before source dispatch/);
  assert.doesNotMatch(JSON.stringify(report), /must_not_be_acquired|api_key|Bearer|sk-|test-secret/i);
  assert.equal(fetchCalls, 0);
  assert.equal(sourceCalls, 0);
});

test("keyless readiness exposes all-domain calibration admission without provider calls", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("calibration readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const readyReport = await agent.readiness({ calibrationReport: readinessCalibrationReport(), requireCalibratedLearning: true });
  assert.equal(readyReport.learning.calibration.status, "ready");
  assert.equal(readyReport.learning.calibration.decision, "admit_learning");
  assert.equal(readyReport.learning.calibration.admitted_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(readyReport.learning.calibration.held_domain_count, 0);
  assert.ok(readyReport.domains.every((row) => row.calibration_admission?.decision === "admit_learning"));
  assert.ok(readyReport.domains.every((row) => row.state === "ready_for_caller_approval"));

  const heldReport = await agent.readiness({ calibrationReport: readinessCalibrationReport({ weak: true }), requireCalibratedLearning: true });
  assert.equal(heldReport.learning.calibration.decision, "hold_learning");
  assert.equal(heldReport.learning.calibration.held_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(heldReport.domains.find((row) => row.domain === "coding").state, "partial");
  assert.match(JSON.stringify(heldReport.domains.find((row) => row.domain === "coding")), /hold evaluator calibration/);
  await assert.rejects(() => agent.readiness({ requireCalibratedLearning: true }), /requires calibrationReport/);
  assert.equal(fetchCalls, 0);
});

test("readiness exposes model, provider, and credential gates as actionable states", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];

  const empty = await new AutonomousAgent(llm).readiness({ candidates: [] });
  assert.equal(empty.readiness_state, "model_catalogue_required");
  assert.ok(empty.domains.every((row) => row.state === "model_catalogue_required"));
  assert.equal(empty.models.length, 0);

  const unregistered = new AutonomousAgent(llm);
  unregistered.registerModel(candidate("unregistered", "model", capabilities));
  const registrationReport = await unregistered.readiness();
  assert.equal(registrationReport.readiness_state, "provider_registration_required");
  assert.equal(registrationReport.models[0].provider_registered, false);
  assert.equal(registrationReport.models[0].eligible_domains.length, 0);

  llm.registerProvider(openaiCompatibleProvider("credentialed", "https://credentialed.invalid", { requiresCredential: true }));
  const credentialed = new AutonomousAgent(llm);
  credentialed.registerModel(candidate("credentialed", "model", capabilities));
  const credentialReport = await credentialed.readiness();
  assert.equal(credentialReport.readiness_state, "credential_required");
  assert.equal(credentialReport.providers[0].credential_ready, false);
  assert.match(JSON.stringify(credentialReport), /collect_user_credential/);
  assert.equal(fetchCalls, 0);
});

test("readiness reports exact live tool metadata while keeping registration non-authorizing", async () => {
  let fetchCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://local.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const binding = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings[0];
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: "metadata-only test tool", inputSchema: { type: "object", additionalProperties: true } }]);
  const agent = new AutonomousAgent(llm, { toolCatalogue: catalogue });
  agent.registerModel(candidate("local", "ready-model", capabilities));

  const report = await agent.readiness();
  const coding = report.domains.find((row) => row.domain === "coding");

  assert.equal(report.tooling.configured, true);
  assert.equal(report.tooling.available_tool_count, 1);
  assert.equal(coding.available_tool_count, 1);
  assert.equal(coding.missing_tools.includes(binding.name), false);
  assert.ok(coding.missing_tools.length > 0);
  assert.equal(report.execution, "not_started; no_provider_or_tool_calls");
  assert.equal(fetchCalls, 0);
});

test("activation is a redacted digest-bound lifecycle across all twelve domains", async () => {
  let now = 100;
  const activation = new AutonomousCapabilityActivation({ activationId: "activation-test", clock: () => now });
  assert.equal(activation.state.status, "created");
  activation.recordProviderStatuses([{
    provider: "local",
    provider_registered: true,
    requires_credential: false,
    credential_ready: true,
    credential: { ready: true, active_handles: 0 },
    next_action: "ready",
    secret_material: "never_returned",
  }]);

  const profiles = await builtinAutonomousDomainProfiles();
  const definitions = [...new Map(profiles.map((profile) => {
    const binding = profile.tool_profile.bindings[0];
    return [binding.name, { name: binding.name, description: `Activation ${binding.name}`, inputSchema: { type: "object", additionalProperties: true } }];
  })).values()];
  const catalogue = await ToolCatalogue.fromDefinitions(definitions);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  assert.equal(plan.domains.length, 12);
  assert.equal(plan.coverage.length, 12);
  assert.equal(plan.plan_digest.length, 64);

  now += 10;
  const reviewed = activation.recordBindingPlan(plan);
  assert.equal(reviewed.domain_statuses.length, 12);
  assert.equal(reviewed.plan_digest, plan.plan_digest);
  const proposed = plan.proposed_bindings.map((binding) => binding.name);
  assert.ok(proposed.length > 0);
  const approved = activation.approveBindings(plan, [proposed[0]], definitions.length);
  assert.deepEqual(approved.approved_tools, [proposed[0]]);
  assert.equal(approved.authorization, "status_only; does_not_grant_provider_or_tool_authority");
  assert.equal(approved.secret_material, "never_returned");
  assert.doesNotMatch(JSON.stringify(approved), /api_key|Bearer|sk-[A-Za-z0-9]/i);
  assert.throws(() => activation.recordProviderStatuses([{ provider: "local", api_key: "must-not-enter-state" }]), /unsupported fields/);

  const store = new AutonomousCapabilityActivationStore();
  await store.save(approved);
  const snapshot = await store.snapshot();
  let persisted = null;
  const persistence = {
    read: () => persisted,
    write: (value) => { persisted = structuredClone(value); },
  };
  const coordinator = new AutonomousCapabilityActivationPersistenceCoordinator(store, persistence);
  const receipt = await coordinator.flush();
  assert.equal(receipt.state_digest, approved.state_digest);
  assert.equal(receipt.retention, "metadata_only");

  const restoredStore = new AutonomousCapabilityActivationStore();
  const restored = new AutonomousCapabilityActivation({ activationId: "activation-test", clock: () => now });
  const restoreCoordinator = new AutonomousCapabilityActivationPersistenceCoordinator(restoredStore, persistence);
  const restoreReceipt = await restoreCoordinator.restore();
  assert.equal(restoreReceipt.restored, true);
  assert.deepEqual((await restoredStore.load()).state_digest, approved.state_digest);
  restored.restore(await restoredStore.load());
  assert.deepEqual(restored.state.approved_tools, approved.approved_tools);
  await assert.rejects(() => restoredStore.restore({ ...snapshot, snapshot_digest: "0".repeat(64) }), /digest/);

  now += 10;
  activation.revoke("caller_revoked_for_test");
  assert.equal(activation.state.status, "revoked");
  assert.throws(() => activation.approveBindings(plan, [proposed[0]]), /revoked/);
});

test("agent activation refreshes keylessly and blocks unapproved custom tool calls", async () => {
  let fetchCalls = 0;
  let executions = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      fetchCalls += 1;
      throw new Error("activation readiness must not contact providers");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("local", "https://activation.invalid", { requiresCredential: false }));
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const binding = profiles.find((profile) => profile.domain === "coding").tool_profile.bindings.find((row) => row.name === "repository_catalog");
  const catalogue = await ToolCatalogue.fromDefinitions([{ name: binding.name, description: "Read repository metadata", inputSchema: { type: "object", additionalProperties: true } }]);
  const activation = new AutonomousCapabilityActivation({ activationId: "agent-activation", clock: () => 200 });
  const agent = new AutonomousAgent(llm, {
    activation,
    toolCatalogue: catalogue,
    toolExecutor: async (tool) => { executions += 1; return { tool: tool.name, ok: true }; },
  });
  agent.registerModel(candidate("local", "local-model", capabilities));

  const state = await agent.refreshActivation();
  assert.equal(state.domain_statuses.length, 12);
  const registry = await AutonomousDomainToolRegistry.create(catalogue, profiles.map((profile) => profile.tool_profile));
  const plan = await registry.plan();
  agent.approveActivationBindings(plan, [binding.name], 1);
  const report = await agent.readiness();
  assert.equal(report.activation.approved_tools[0], binding.name);
  assert.equal(report.activation.plan_digest, plan.plan_digest);

  const results = await agent.executeToolCalls([
    { id: "approved", name: binding.name, arguments: {} },
    { id: "blocked", name: "repository_impact_analysis", arguments: {} },
  ], { domains: ["coding"], approveEffects: true });
  assert.equal(results.find((row) => row.callId === "approved").approved, true);
  assert.equal(results.find((row) => row.callId === "blocked").content.status, "activation_required");
  assert.equal(executions, 1);
  assert.equal(fetchCalls, 0);
});
