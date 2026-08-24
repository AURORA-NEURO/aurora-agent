import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  LLMRuntime,
  ProviderRuntimeError,
  buildAutonomousDomainResponseContract,
  builtinAutonomousDomainProfiles,
  evaluateAutonomousDomainResponse,
  replayAutonomousDomainResponseEvaluation,
  validateAutonomousDomainResponse,
} from "../dist/index.js";

function responseFor(contract) {
  return {
    schema: "bioprism-typescript-autonomous-domain-response/0.1",
    domain: contract.domain,
    workflow_id: contract.workflow_id,
    status: "complete",
    answer: `Bounded answer for ${contract.domain}.`,
    observations: ["The supplied offline observation was inspected."],
    inferences: ["The result is limited to the supplied contract."],
    uncertainty: ["External-world truth remains caller-owned."],
    evidence_gaps: ["No live source was contacted in this test."],
    next_actions: ["Review the declared evidence and approve the next action."],
    stages: contract.stage_ids.map((stage_id) => ({
      stage_id,
      status: "complete",
      evidence: [`evidence:${stage_id}`],
      findings: [`finding:${stage_id}`],
      uncertainty: [`uncertainty:${stage_id}`],
      open_questions: [],
    })),
    domain_details: Object.fromEntries(contract.domain_fields.map((field) => [field, [`detail:${field}`]])),
    retention: "transient_provider_response_only;validated_against_reviewed_domain_contract",
    secret_material: "never_returned",
  };
}

const model = {
  provider: "offline",
  model: "structured-domain-model",
  capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
  context_window_tokens: 64_000,
  max_output_tokens: 8_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
  requires_credential: false,
};

test("every built-in domain has a digest-bound structured response contract", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = await Promise.all(profiles.map((profile) => buildAutonomousDomainResponseContract(profile)));
  assert.equal(contracts.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(contracts.map((contract) => contract.domain).sort(), [...AUTONOMOUS_DOMAIN_NAMES].sort());
  for (const contract of contracts) {
    assert.match(contract.contract_digest, /^[0-9a-f]{64}$/);
    assert.equal(contract.response_schema.properties.domain.const, contract.domain);
    assert.equal(contract.response_schema.properties.workflow_id.const, contract.workflow_id);
    assert.deepEqual(contract.response_schema.properties.stages.items.properties.stage_id.enum, contract.stage_ids);
    assert.deepEqual(validateAutonomousDomainResponse(responseFor(contract), contract), responseFor(contract));
  }
});

test("structured domain responses flow through real selection and invocation across all domains", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  const calls = [];
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", (request) => {
    calls.push({ model: request.model, schema: request.responseSchema });
    const domain = request.responseSchema?.properties?.domain?.const;
    return { structured: responseFor(contracts.get(domain)) };
  }, { structuredOutputMode: "json_schema" });
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model);

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const result = await agent.run(`Produce a bounded structured answer for ${domain}.`, {
      domain,
      approveProviderCall: true,
      structuredDomainResponse: true,
    });
    assert.equal(result.status, "completed");
    assert.equal(result.blueprint.response_contract.domain, domain);
    assert.equal(result.response.structured.domain, domain);
    assert.equal(result.response.structured.stages.length, contracts.get(domain).stage_ids.length);
    assert.deepEqual(result.response.structured.domain_details, responseFor(contracts.get(domain)).domain_details);
    assert.equal(result.response_evaluation.domain, domain);
    assert.equal(result.response_evaluation.reward, 1);
    assert.equal(result.response_evaluation.reward_input.evidence_digest, result.response_evaluation.response_digest);
  }
  assert.equal(calls.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(calls.every((call) => call.schema?.type === "object"));
});

test("the same domain response contract propagates through specialist fan-out and synthesis", async () => {
  const profiles = await builtinAutonomousDomainProfiles();
  const contracts = new Map();
  for (const profile of profiles) contracts.set(profile.domain, await buildAutonomousDomainResponseContract(profile));
  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", (request) => {
    const domain = request.responseSchema?.properties?.domain?.const;
    return { structured: responseFor(contracts.get(domain)) };
  }, { structuredOutputMode: "json_schema" });
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const learning = new AutonomousLearningController(agent);
  const task = "Research a biomedical neuroscience experiment with patient EEG evidence.";
  const route = await agent.route(task, { allowCrossDomain: true });
  assert.equal(route.cross_domain, true);
  const result = await agent.runCrossDomain(task, {
    routeOverride: route,
    structuredDomainResponse: true,
    approveProviderCall: true,
    maxParallelChildren: 2,
    learning,
  });
  assert.equal(result.status, "completed");
  assert.ok(result.child_runs.length >= 2);
  assert.ok(result.child_runs.every((child) => child.result.response.structured.domain === child.domain));
  assert.equal(result.synthesis.response.structured.domain, "cross_domain");
  assert.equal(result.learning_episode_ids.length, result.child_runs.length + 1);
  assert.equal(result.response_learning_episode_ids.length, result.child_runs.length + 1);
  const rewards = Object.fromEntries(result.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "cross-domain-task-reviewer", evaluator_version: "1", reward: 0.8, passed: true }]));
  const settled = await learning.settleCrossDomain(result, rewards, { trajectoryId: "structured-cross-domain-trajectory" });
  assert.equal(settled.trajectory.settlements.length, result.learning_episode_ids.length);
  assert.equal(settled.response_settlements.length, result.response_learning_episode_ids.length);
  assert.ok(settled.response_settlements.every((row) => row.assessment.evaluator_id.endsWith("response-integrity")));
  const tampered = structuredClone(result);
  tampered.child_runs[0].result.response.structured.answer = "tampered after provider execution";
  await assert.rejects(
    () => learning.settleCrossDomain(tampered, rewards, { trajectoryId: "structured-cross-domain-trajectory" }),
    /replay drifted/,
  );
});

test("domain response validation rejects stage drift, unknown fields, and credential-shaped material", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === "operations");
  const contract = await buildAutonomousDomainResponseContract(profile);
  const wrongOrder = responseFor(contract);
  wrongOrder.stages.reverse();
  assert.throws(() => validateAutonomousDomainResponse(wrongOrder, contract), /reviewed workflow order/);

  const unknown = responseFor(contract);
  unknown.unexpected = true;
  assert.throws(() => validateAutonomousDomainResponse(unknown, contract), /unsupported fields/);

  const secret = responseFor(contract);
  secret.domain_details.approval_request = ["gsk_test_secret_should_never_enter_a_response"];
  assert.throws(() => validateAutonomousDomainResponse(secret, contract), /credential-shaped/);

  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", () => {
    const invalid = responseFor(contract);
    invalid.stages.reverse();
    return { structured: invalid };
  }, { structuredOutputMode: "json_schema" });
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model);
  await assert.rejects(
    () => agent.run("Return an invalid operations response.", { domain: "operations", structuredDomainResponse: true, approveProviderCall: true }),
    (error) => error instanceof ProviderRuntimeError && error.code === "invalid_response",
  );
});

test("structured response evaluation is deterministic, replayable, and explicitly settles bandit feedback", async () => {
  const profile = (await builtinAutonomousDomainProfiles()).find((candidate) => candidate.domain === "coding");
  const contract = await buildAutonomousDomainResponseContract(profile);
  const response = responseFor(contract);
  const evaluation = evaluateAutonomousDomainResponse(response, contract);
  assert.equal(evaluation.evaluator_id, "autonomous-coding-response-integrity");
  assert.equal(evaluation.passed, true);
  assert.equal(evaluation.evaluator_authority, "structural_response_contract_only;not_external_truth");
  assert.deepEqual(replayAutonomousDomainResponseEvaluation(response, contract, evaluation), evaluation);

  const degraded = responseFor(contract);
  degraded.domain_details.tests_and_verification = [];
  degraded.domain_details.rollback_or_follow_up = [];
  assert.notEqual(evaluateAutonomousDomainResponse(degraded, contract).evaluation_digest, evaluation.evaluation_digest);
  assert.throws(() => replayAutonomousDomainResponseEvaluation(degraded, contract, evaluation), /replay drifted/);

  const llm = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  llm.registerInMemoryProvider("offline", () => ({ structured: responseFor(contract) }), { structuredOutputMode: "json_schema" });
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(model);
  const learning = new AutonomousLearningController(agent);
  const run = await agent.run("Return a learning-safe structured coding handoff.", {
    domain: "coding",
    structuredDomainResponse: true,
    approveProviderCall: true,
    learning,
    learningEpisodeId: "structured-response-learning-1",
  });
  assert.equal(run.learning_episode_status, "prepared");
  const settlement = await learning.settleStructuredResponse(run);
  assert.equal(settlement.episode.status, "settled");
  assert.equal(settlement.assessment.reward, run.response_evaluation.reward);
  assert.equal(settlement.assessment.failure_class, null);
});
