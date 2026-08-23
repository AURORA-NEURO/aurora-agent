import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousExecutionController,
  InMemoryAutonomousExecutionJournal,
  AutonomousLearningController,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  AutonomousOnlineLearner,
  InMemoryAutonomousModelHealthStore,
  AutonomousCostBudget,
  CredentialStore,
  AutonomousDecisionCyclePersistenceCoordinator,
  InMemoryAutonomousDecisionCycleStateStore,
  TransactionalJsonAutonomousDecisionCycleSnapshotPersistence,
  InMemoryAutonomousCycleReplanStateStore,
  LLMRuntime,
  openaiCompatibleProvider,
  runAutonomousCrossDomainDecisionCycle,
  runAutonomousCrossDomainReplanCycle,
  runAutonomousDecisionCycle,
  runAutonomousReplanCycle,
  validateAutonomousDecisionCycleSnapshot,
  validateAutonomousCycleReplanSnapshot,
} from "../dist/index.js";

function providerPlanningCycleAgent(agentOptions = {}) {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      const body = JSON.parse(String(init.body));
      const planningMessage = body.messages.find((message) => message.content.startsWith("Context planning-contract:\n"));
      if (planningMessage) {
        const contract = JSON.parse(planningMessage.content.slice("Context planning-contract:\n".length));
        const ids = (contract.stage_catalogue ?? contract.child_catalogue).map((row) => row.id);
        const focusField = contract.stage_catalogue ? "focus_stage_ids" : "focus_child_ids";
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ priority_order: ids, [focusField]: ids.slice(0, 1), review_required: false, confidence: 0.98, abstain: false }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "cycle execution after accepted plan" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-planner.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, agentOptions);
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), { status: 200, headers: { "content-type": "application/json" } });
}

const capabilities = ["reasoning", "code", "web", "data", "science", "biomedical", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"];
const loopTools = [{ name: "repository_catalog", description: "Inspect repository", parameters: { type: "object", additionalProperties: false } }];

function candidate() {
  return {
    provider: "cycle-provider",
    model: "cycle-model",
    capabilities,
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 100,
    cost_per_million_tokens: 5,
    reliability: 0.95,
  };
}

function cycleAgent(payloads = [{ text: "cycle answer" }]) {
  let calls = 0;
  const bodies = [];
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      bodies.push(JSON.parse(String(init.body)));
      const payload = payloads[Math.min(calls, payloads.length - 1)];
      calls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: payload.route ? JSON.stringify(payload.route) : payload.text }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, bodies, calls: () => calls };
}

function failingCycleAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      throw new Error("simulated provider interruption");
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-interrupted.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

function interruptedSemanticCycleAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      if (calls === 1) throw new Error("simulated semantic routing interruption");
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "cycle answer" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-semantic-interrupted.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

function retryingSemanticCycleAgent() {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      if (calls === 1) throw new Error("simulated semantic routing interruption");
      if (calls === 2) {
        return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify({ selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.94, abstain: false, abstain_reason: null }) }, finish_reason: "stop" }] });
      }
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "cycle answer" }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-semantic-retry.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, { learner: new AutonomousOnlineLearner() });
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

function toolLoopAgent(stopResponses = 0) {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      calls += 1;
      if (calls <= stopResponses) return jsonResponse({ choices: [{ message: { role: "assistant", content: "child answer" }, finish_reason: "stop" }] });
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "", tool_calls: [{ id: `cycle-tool-${calls}`, type: "function", function: { name: "repository_catalog", arguments: "{}" } }] }, finish_reason: "tool_calls" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle-tool-loop.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(candidate());
  return { agent, calls: () => calls };
}

test("decision cycle connects approval, invocation, evaluator settlement, and bandit adaptation", async () => {
  const { agent, calls } = cycleAgent();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const learning = new AutonomousLearningController(agent, { feedbackOutbox: outbox });
  const task = "Debug this coding repository and report the verified tests.";
  const result = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    approveProviderCall: true,
    learning: {
      controller: learning,
      episodeId: "cycle-coding-1",
      outbox: { workerId: "decision-cycle-worker" },
      evaluate: (run) => ({ evaluator_id: "coding-reviewer", evaluator_version: "1", reward: run.response?.text === "cycle answer" ? 0.9 : 0, passed: true }),
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.run.response.text, "cycle answer");
  assert.equal(result.learning_episode_id, "cycle-coding-1");
  assert.equal(result.evaluation.reward, 0.9);
  assert.equal(result.settlement.episode.status, "settled");
  assert.equal(result.settlement.next_state.generation, 1);
  assert.equal(outbox.rows().filter((command) => command.status === "applied").length, 1);
  assert.equal(calls(), 1);
  assert.equal(JSON.stringify(result.settlement).includes(task), false);
});

test("decision-cycle provider planning pauses, persists a digest, and resumes only with the accepted proposal", async () => {
  const { agent, calls } = providerPlanningCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const task = "Debug this coding repository and report verified tests.";
  const reviewed = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    cycleId: "planned-decision-cycle",
    decisionStateStore: stateStore,
    providerPlanning: { approveProviderCall: true },
    approveProviderCall: true,
  });
  assert.equal(reviewed.status, "plan_review_required");
  assert.equal(reviewed.plan_refinement.status, "completed");
  assert.equal(reviewed.run, null);
  const pending = await stateStore.load("planned-decision-cycle");
  assert.equal(pending.phase, "planning_pending");
  assert.equal(pending.plan_refinement_digest.length, 64);
  assert.equal(calls(), 1);
  await assert.rejects(
    () => runAutonomousDecisionCycle(agent, task, {
      domain: "coding",
      cycleId: "planned-decision-cycle",
      decisionStateStore: stateStore,
      rehydrateRoute: () => reviewed.route,
      approveProviderCall: true,
    }),
    /rehydrate the accepted single-domain plan refinement/,
  );
  const resumed = await runAutonomousDecisionCycle(agent, task, {
    domain: "coding",
    cycleId: "planned-decision-cycle",
    decisionStateStore: stateStore,
    rehydrateRoute: () => reviewed.route,
    acceptedSingleDomainPlanRefinement: reviewed.plan_refinement,
    approveProviderCall: true,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.run.plan_refinement_digest, pending.plan_refinement_digest);
  assert.equal(resumed.plan_refinement.status, "completed");
  assert.equal(resumed.run.response.text, "cycle execution after accepted plan");
  assert.equal(calls(), 2, "restart resumes the accepted proposal without replaying the planner");
  const terminal = await stateStore.load("planned-decision-cycle");
  assert.equal(terminal.phase, "terminal");
  assert.equal(terminal.plan_refinement_digest, pending.plan_refinement_digest);
});

test("provider-planned decision cycles apply the same reviewed contract to every single-domain profile", async () => {
  const { agent } = providerPlanningCycleAgent();
  const tasks = {
    coding: "debug this repository and verify the tests",
    browser: "research current sources and compare citations",
    data: "validate dataset schema lineage and quality",
    science: "design a reproducible hypothesis experiment",
    biomedical: "review biomedical evidence with safety boundaries",
    neuroscience: "analyze EEG preprocessing and signal confounds",
    operations: "prepare an outage rollback runbook",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this specialist subtask and synthesize findings",
    multimodal: "inspect this image transcript and evidence gaps",
    evaluation: "run a benchmark holdout replay and report uncertainty",
  };
  for (const [domain, task] of Object.entries(tasks)) {
    const result = await runAutonomousDecisionCycle(agent, task, {
      domain,
      providerPlanning: { approveProviderCall: true },
      acceptPlan: true,
      approveProviderCall: true,
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.plan_refinement.status, "completed", domain);
    assert.equal(result.run.plan_refinement_digest.length, 64, domain);
  }
});

test("provider-planned decision cycles settle planner and execution quality across every single-domain profile", async () => {
  const health = new InMemoryAutonomousModelHealthStore();
  const { agent } = providerPlanningCycleAgent({ learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  const learning = new AutonomousLearningController(agent);
  const tasks = {
    coding: "debug this repository and verify the tests",
    browser: "research current sources and compare citations",
    data: "validate dataset schema lineage and quality",
    science: "design a reproducible hypothesis experiment",
    biomedical: "review biomedical evidence with safety boundaries",
    neuroscience: "analyze EEG preprocessing and signal confounds",
    operations: "prepare an outage rollback runbook",
    enterprise: "review governance compliance ownership",
    multi_agent: "delegate this specialist subtask and synthesize findings",
    multimodal: "inspect this image transcript and evidence gaps",
    evaluation: "run a benchmark holdout replay and report uncertainty",
  };
  for (const [domain, task] of Object.entries(tasks)) {
    const result = await runAutonomousDecisionCycle(agent, task, {
      domain,
      providerPlanning: { approveProviderCall: true },
      acceptPlan: true,
      approveProviderCall: true,
      learning: {
        controller: learning,
        episodeId: `planner-cycle-${domain}`,
        evaluate: () => ({ evaluator_id: "execution-reviewer", evaluator_version: "1", reward: 0.74, passed: true }),
        evaluatePlanning: () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.83, passed: true }),
      },
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.planner_evaluation.reward, 0.83, domain);
    assert.equal(result.planner_settlement.status, "settled", domain);
    assert.equal(result.settlement.episode.status, "settled", domain);
  }
  const state = agent.learner.snapshot();
  assert.equal(state.credited_outcomes.length, Object.keys(tasks).length * 2);
  assert.equal(health.health({ model: "cycle-model" })[0]?.quality_observations, Object.keys(tasks).length * 2);
});

test("cross-domain decision-cycle planning persists and rehydrates the accepted fan-out proposal", async () => {
  const { agent, calls } = providerPlanningCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence.";
  const reviewed = await runAutonomousCrossDomainDecisionCycle(agent, task, {
    allowCrossDomain: true,
    cycleId: "planned-cross-domain-cycle",
    decisionStateStore: stateStore,
    providerPlanning: { approveProviderCall: true },
    approveProviderCall: true,
    maxParallelChildren: 1,
  });
  assert.equal(reviewed.status, "plan_review_required");
  assert.equal(reviewed.plan_refinement.status, "completed");
  const pending = await stateStore.load("planned-cross-domain-cycle");
  assert.equal(pending.phase, "planning_pending");
  assert.equal(pending.plan_refinement_digest.length, 64);
  const resumed = await runAutonomousCrossDomainDecisionCycle(agent, task, {
    allowCrossDomain: true,
    cycleId: "planned-cross-domain-cycle",
    decisionStateStore: stateStore,
    rehydrateRoute: () => reviewed.route,
    acceptedCrossDomainPlanRefinement: reviewed.plan_refinement,
    approveProviderCall: true,
    maxParallelChildren: 1,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.run.plan_refinement_digest, pending.plan_refinement_digest);
  assert.equal(resumed.run.child_runs.length, 2);
  assert.equal(calls(), 4, "one planner plus two specialists and one synthesis call");
});

test("replan-cycle planning review is resumable through its outer metadata-only ledger", async () => {
  const { agent, calls } = providerPlanningCycleAgent();
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  const task = "Debug this coding repository and report verified tests.";
  const reviewed = await runAutonomousReplanCycle(agent, task, {
    domain: "coding",
    cycleId: "planned-replan-cycle",
    stateStore,
    maxReplans: 0,
    providerPlanning: { approveProviderCall: true },
    approveProviderCall: true,
    evaluate: () => ({ evaluator_id: "cycle-reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
  });
  assert.equal(reviewed.status, "plan_review_required");
  assert.equal(reviewed.attempts[0].plan_refinement_digest.length, 64);
  const pending = await stateStore.load("planned-replan-cycle");
  assert.equal(pending.phase, "execution_pending");
  assert.equal(pending.plan_refinement_digest, reviewed.attempts[0].plan_refinement_digest);
  const resumed = await runAutonomousReplanCycle(agent, task, {
    domain: "coding",
    cycleId: "planned-replan-cycle",
    stateStore,
    maxReplans: 0,
    rehydrateRoute: () => reviewed.final.route,
    acceptedSingleDomainPlanRefinement: reviewed.final.plan_refinement,
    approveProviderCall: true,
    evaluate: () => ({ evaluator_id: "cycle-reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.final.run.plan_refinement_digest, pending.plan_refinement_digest);
  assert.equal(calls(), 2, "replan restart resumes execution without replaying provider planning");
});

test("single-domain replanning settles each accepted planner proposal alongside attempt credit", async () => {
  const health = new InMemoryAutonomousModelHealthStore();
  const { agent } = providerPlanningCycleAgent({ learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousReplanCycle(agent, "Debug this coding repository and report verified tests.", {
    domain: "coding",
    providerPlanning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    maxReplans: 0,
    learning: { controller: learning, episodePrefix: "planner-replan" },
    evaluate: () => ({ evaluator_id: "execution-reviewer", evaluator_version: "1", reward: 0.78, passed: true, replan_requested: false }),
    evaluatePlanning: () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.86, passed: true }),
  });
  assert.equal(result.status, "completed");
  assert.equal(result.planner_evaluations.length, 1);
  assert.equal(result.planner_evaluations[0].reward, 0.86);
  assert.equal(result.planner_settlements.length, 1);
  assert.equal(result.planner_settlements[0].status, "settled");
  assert.equal(result.final.planner_settlement.status, "settled");
  assert.equal(health.health({ model: "cycle-model" })[0]?.quality_observations, 2);
});

test("ordinary decision cycles persist a metadata-only restart barrier across every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const first = cycleAgent();
  const store = new InMemoryAutonomousDecisionCycleStateStore();
  const privateResults = new Map();
  for (const domain of domains) {
    const task = `restart-safe ${domain} review`;
    const cycleId = `ordinary-${domain}`;
    const result = await runAutonomousDecisionCycle(first.agent, task, { domain, approveProviderCall: true, cycleId, decisionStateStore: store });
    assert.equal(result.status, "completed", domain);
    privateResults.set(cycleId, result);
  }
  assert.equal(first.calls(), domains.length);
  const persisted = new Map();
  const coordinator = new AutonomousDecisionCyclePersistenceCoordinator(store, {
    read: () => persisted.get("snapshot") ?? null,
    write: (snapshot) => { persisted.set("snapshot", structuredClone(snapshot)); },
  });
  const flushed = await coordinator.flush();
  assert.equal(flushed.states.length, domains.length);
  assert.doesNotMatch(JSON.stringify(flushed), /restart-safe|cycle answer/);
  const tampered = structuredClone(flushed);
  tampered.snapshot_digest = "0".repeat(64);
  await assert.rejects(() => validateAutonomousDecisionCycleSnapshot(tampered), /digest/);

  const restoredStore = new InMemoryAutonomousDecisionCycleStateStore();
  const restoredCoordinator = new AutonomousDecisionCyclePersistenceCoordinator(restoredStore, {
    read: () => persisted.get("snapshot") ?? null,
    write: () => {},
  });
  const restored = await restoredCoordinator.restore();
  assert.equal(restored?.snapshot_digest, flushed.snapshot_digest);

  let encodedCycle = null;
  const transactionalTextStore = {
    read: () => encodedCycle,
    write: (value) => { encodedCycle = value; },
    writeIfUnchanged: (expected, value) => {
      const observed = encodedCycle === null ? null : JSON.parse(encodedCycle).snapshot_digest;
      if (observed !== expected) return false;
      encodedCycle = value;
      return true;
    },
  };
  const transactionalPersistence = new TransactionalJsonAutonomousDecisionCycleSnapshotPersistence(transactionalTextStore);
  const transactionalCoordinator = new AutonomousDecisionCyclePersistenceCoordinator(store, transactionalPersistence);
  await transactionalCoordinator.flush();
  const staleCoordinator = new AutonomousDecisionCyclePersistenceCoordinator(new InMemoryAutonomousDecisionCycleStateStore(), transactionalPersistence);
  await assert.rejects(() => staleCoordinator.flush(), /compare-and-swap/);

  const restarted = cycleAgent();
  for (const domain of domains) {
    const cycleId = `ordinary-${domain}`;
    const replay = await runAutonomousDecisionCycle(restarted.agent, `restart-safe ${domain} review`, {
      domain,
      approveProviderCall: true,
      cycleId,
      decisionStateStore: restoredStore,
      rehydrateResult: () => privateResults.get(cycleId),
    });
    assert.equal(replay.status, "completed", domain);
    assert.equal(replay.run.response?.text, "cycle answer", domain);
  }
  assert.equal(restarted.calls(), 0);
});

test("ordinary decision cycles recover an execution boundary from caller-owned run state", async () => {
  const task = "Restart this coding review after the provider worker exits.";
  const source = cycleAgent();
  const baseline = await runAutonomousDecisionCycle(source.agent, task, { domain: "coding", approveProviderCall: true });
  const interrupted = failingCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  await assert.rejects(
    runAutonomousDecisionCycle(interrupted.agent, task, {
      domain: "coding",
      routeOverride: baseline.route,
      approveProviderCall: true,
      cycleId: "ordinary-execution-interruption",
      decisionStateStore: stateStore,
    }),
    /provider transport failed/,
  );
  const pending = await stateStore.load("ordinary-execution-interruption");
  assert.equal(pending.phase, "execution_pending");
  assert.equal(pending.outcome_digest, null);
  assert.doesNotMatch(JSON.stringify(pending), /Restart this coding review|cycle answer/);

  const resumed = await runAutonomousDecisionCycle(interrupted.agent, task, {
    domain: "coding",
    approveProviderCall: true,
    cycleId: "ordinary-execution-interruption",
    decisionStateStore: stateStore,
    rehydrateRoute: () => baseline.route,
    rehydrateRun: () => baseline.run,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.run.response.text, "cycle answer");
  assert.equal(interrupted.calls(), 1);
});

test("semantic routing recovery requires explicit route reuse or an explicit retry", async () => {
  const task = "Help with an unfamiliar coding migration after a worker restart.";
  const routeSource = cycleAgent();
  const route = await routeSource.agent.route(task, { domain: "coding" });
  const interrupted = interruptedSemanticCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const base = {
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false, maxDomains: 1 },
    approveProviderCall: true,
    cycleId: "ordinary-semantic-interruption",
    decisionStateStore: stateStore,
  };
  await assert.rejects(runAutonomousDecisionCycle(interrupted.agent, task, base), /provider transport failed/);
  const pending = await stateStore.load("ordinary-semantic-interruption");
  assert.equal(pending.phase, "route_pending");
  assert.equal(pending.route_digest, null);
  await assert.rejects(
    runAutonomousDecisionCycle(interrupted.agent, task, base),
    /rehydrateRoute or retrySemanticRoutingOnRestart/,
  );
  assert.equal(interrupted.calls(), 1, "an interrupted semantic route must not be implicitly replayed");

  const resumed = await runAutonomousDecisionCycle(interrupted.agent, task, {
    ...base,
    rehydrateRoute: () => route,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.run.response.text, "cycle answer");
  assert.equal(interrupted.calls(), 2, "route rehydration should leave only the execution provider call");
  assert.equal((await stateStore.load("ordinary-semantic-interruption")).phase, "terminal");
});

test("semantic routing restart retry is an explicit opt-in", async () => {
  const interrupted = retryingSemanticCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const task = "Help with an unfamiliar coding migration after a worker restart.";
  const base = {
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false, maxDomains: 1 },
    approveProviderCall: true,
    cycleId: "ordinary-semantic-retry",
    decisionStateStore: stateStore,
  };
  await assert.rejects(runAutonomousDecisionCycle(interrupted.agent, task, base), /provider transport failed/);
  const resumed = await runAutonomousDecisionCycle(interrupted.agent, task, {
    ...base,
    retrySemanticRoutingOnRestart: true,
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.route.primary_domain, "coding");
  assert.equal(interrupted.calls(), 3, "explicit retry permits exactly one new route call before execution");
  assert.equal((await stateStore.load("ordinary-semantic-retry")).phase, "terminal");
});

test("cross-domain semantic routing rehydrates once before bounded fan-out", async () => {
  const task = "Help with an unfamiliar biomedical neuroscience study after a worker restart.";
  const routeSource = cycleAgent();
  const route = await routeSource.agent.route(task, { allowCrossDomain: true });
  assert.equal(route.cross_domain, true);
  const interrupted = interruptedSemanticCycleAgent();
  const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
  const result = await (async () => {
    await assert.rejects(
      runAutonomousCrossDomainDecisionCycle(interrupted.agent, task, {
        semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true },
        approveProviderCall: true,
        cycleId: "cross-semantic-interruption",
        decisionStateStore: stateStore,
        subtasks: [
          { id: "bio", domain: "biomedical", task: "Review the biomedical evidence." },
          { id: "neuro", domain: "neuroscience", task: "Review the neuroscience evidence." },
        ],
      }),
      /provider transport failed/,
    );
    return runAutonomousCrossDomainDecisionCycle(interrupted.agent, task, {
      semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true },
      approveProviderCall: true,
      synthesize: false,
      cycleId: "cross-semantic-interruption",
      decisionStateStore: stateStore,
      rehydrateRoute: () => route,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review the biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review the neuroscience evidence." },
      ],
    });
  })();
  assert.equal(result.status, "children_completed");
  assert.equal(result.run.status, "children_completed");
  assert.equal(interrupted.calls(), 3, "one rehydrated route must lead to exactly two specialist calls");
});

test("decision cycle preserves structured output and caller selection policy", async () => {
  const cycle = cycleAgent([{ text: JSON.stringify({ answer: "cycle-structured" }) }]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await runAutonomousDecisionCycle(cycle.agent, "Return a structured coding result.", {
    domain: "coding",
    approveProviderCall: true,
    maxCostPerMillionTokens: 5,
    maxLatencyMs: 100,
    minQuality: 0.9,
    requireJson: true,
    responseSchema,
  });
  assert.equal(result.status, "completed");
  assert.deepEqual(result.run.response.structured, { answer: "cycle-structured" });
  assert.deepEqual(cycle.bodies[0].response_format, { type: "json_object" });
});

test("decision cycle preserves bounded tool-loop exhaustion without evaluator settlement", async () => {
  const { agent, calls } = toolLoopAgent();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousDecisionCycle(agent, "Review this coding repository", {
    domain: "coding",
    approveProviderCall: true,
    tools: loopTools,
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    learning: {
      controller: learning,
      episodeId: "cycle-tool-limit",
      evaluate: () => { throw new Error("tool-limit result must not be evaluated"); },
    },
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.run.status, "turn_limit_reached");
  assert.equal(result.run.tool_loop.status, "turn_limit_reached");
  assert.equal(result.learning_episode_id, null);
  assert.equal(result.evaluation, null);
  assert.equal(result.settlement, null);
  assert.equal(calls(), 4);
});

test("decision cycle fails the shared execution when post-run evaluation throws", async () => {
  const { agent, calls } = cycleAgent();
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-post-run-failure-1", domain: "coding", capability: "code_review", riskClass: "read_only", journal: new InMemoryAutonomousExecutionJournal() });
  const learning = new AutonomousLearningController(agent);
  await assert.rejects(
    runAutonomousDecisionCycle(agent, "Review this coding change", {
      domain: "coding",
      approveProviderCall: true,
      execution,
      learning: { controller: learning, episodeId: "cycle-post-run-failure-episode", evaluate: async () => { throw new Error("post-run evaluator unavailable"); } },
    }),
    /post-run evaluator unavailable/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("replan cycle feeds bounded evaluator guidance into the next attempt and settles each attempt", async () => {
  const { agent, bodies, calls } = cycleAgent([{ text: "first answer" }, { text: "verified answer" }]);
  const learning = new AutonomousLearningController(agent);
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-execution-1", domain: "coding", capability: "code_review", riskClass: "read_only", journal: new InMemoryAutonomousExecutionJournal() });
  let evaluations = 0;
  const result = await runAutonomousReplanCycle(agent, "Debug this coding repository and report the verified tests.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 1,
    execution,
    evaluate: () => {
      evaluations += 1;
      return evaluations === 1
        ? { evaluator_id: "coding-reviewer", evaluator_version: "2", reward: 0.25, passed: false, failed: true, replan_requested: true, replan_instruction: "Add explicit verification evidence before concluding.", evidence_digest: "a".repeat(64) }
        : { evaluator_id: "coding-reviewer", evaluator_version: "2", reward: 0.95, passed: true, failed: false, replan_requested: false, evidence_digest: "b".repeat(64) };
    },
    learning: { controller: learning, episodePrefix: "cycle-replan" },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.replan_count, 1);
  assert.equal(result.attempts.length, 2);
  assert.equal(result.attempts[0].evaluation.replan_requested, true);
  assert.equal(result.attempts[1].evaluation.replan_requested, false);
  assert.equal(result.learning_episode_ids.length, 2);
  assert.equal(result.settlements.length, 2);
  assert.equal(result.settlements.at(-1).next_state.generation, 2);
  assert.equal(execution.state.status, "completed");
  assert.equal(execution.state.provider_calls, 2);
  assert.equal(execution.state.replans, 1);
  assert.equal(calls(), 2);
  assert.match(JSON.stringify(bodies[1]), /autonomous-replan-2/);
  assert.match(JSON.stringify(bodies[1]), /Add explicit verification evidence/);
  assert.equal(JSON.stringify(result.attempts).includes("Add explicit verification evidence"), false);
});

test("replan persistence resumes an interrupted evaluator boundary without replaying the provider", async () => {
  const cycle = cycleAgent();
  const task = "Review this coding change and report the verified tests.";
  const route = await cycle.agent.route(task, { domain: "coding" });
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  let interruptedRun = null;
  await assert.rejects(
    runAutonomousReplanCycle(cycle.agent, task, {
      domain: "coding",
      routeOverride: route,
      approveProviderCall: true,
      cycleId: "restartable-single-cycle",
      stateStore,
      evaluate: (run) => {
        interruptedRun = run;
        throw new Error("simulated evaluator interruption");
      },
    }),
    /simulated evaluator interruption/,
  );
  const interrupted = await stateStore.load("restartable-single-cycle");
  assert.equal(interrupted.phase, "evaluation_pending");
  assert.equal(interrupted.attempts.length, 1);
  assert.equal(Object.hasOwn(interrupted, "task"), false);
  assert.equal(JSON.stringify(interrupted).includes("cycle answer"), false);

  const resumed = await runAutonomousReplanCycle(cycle.agent, task, {
    domain: "coding",
    approveProviderCall: true,
    cycleId: "restartable-single-cycle",
    stateStore,
    rehydrateRoute: () => route,
    rehydrateRun: () => interruptedRun,
    evaluate: () => ({ evaluator_id: "restart-reviewer", evaluator_version: "1", reward: 0.9, passed: true, replan_requested: false }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.attempts.length, 1);
  assert.equal(resumed.evaluations.length, 1);
  assert.equal(cycle.calls(), 1);
  assert.equal((await stateStore.load("restartable-single-cycle")).phase, "terminal");
  const snapshot = await stateStore.snapshot();
  await assert.rejects(validateAutonomousCycleReplanSnapshot({ ...snapshot, snapshot_digest: "0".repeat(64) }), /snapshot digest/);
});

test("replan persistence resumes a settlement boundary from the evaluator packet", async () => {
  const cycle = cycleAgent();
  const task = "Review this coding change and report the verified tests.";
  const route = await cycle.agent.route(task, { domain: "coding" });
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  const learning = new AutonomousLearningController(cycle.agent);
  const settle = learning.settleRun.bind(learning);
  let failSettlement = true;
  learning.settleRun = async (...args) => {
    if (failSettlement) {
      failSettlement = false;
      throw new Error("simulated settlement interruption");
    }
    return settle(...args);
  };
  let evaluatorPacket = null;
  let interruptedRun = null;
  await assert.rejects(
    runAutonomousReplanCycle(cycle.agent, task, {
      domain: "coding",
      routeOverride: route,
      approveProviderCall: true,
      cycleId: "restartable-settlement-cycle",
      stateStore,
      learning: { controller: learning, episodePrefix: "restartable-settlement" },
      evaluate: (run) => {
        interruptedRun = run;
        evaluatorPacket = { evaluator_id: "settlement-reviewer", evaluator_version: "1", reward: 0.7, passed: true, replan_requested: false };
        return evaluatorPacket;
      },
    }),
    /simulated settlement interruption/,
  );
  assert.equal((await stateStore.load("restartable-settlement-cycle")).phase, "settlement_pending");
  const resumed = await runAutonomousReplanCycle(cycle.agent, task, {
    domain: "coding",
    approveProviderCall: true,
    cycleId: "restartable-settlement-cycle",
    stateStore,
    rehydrateRoute: () => route,
    rehydrateRun: () => interruptedRun,
    rehydrateEvaluation: () => evaluatorPacket,
    evaluate: () => { throw new Error("evaluator must not replay after settlement checkpoint"); },
    learning: { controller: learning, episodePrefix: "restartable-settlement" },
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.settlements.length, 1);
  assert.equal(cycle.calls(), 1);
});

test("replan cycle preserves one-shot completion and enforces the replan ceiling", async () => {
  const oneShot = cycleAgent();
  const completed = await runAutonomousReplanCycle(oneShot.agent, "Review this coding change.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.replan_count, 0);
  assert.equal(oneShot.calls(), 1);

  const limited = cycleAgent();
  const result = await runAutonomousReplanCycle(limited.agent, "Review this coding change.", {
    domain: "coding",
    approveProviderCall: true,
    maxReplans: 0,
    evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "Collect an independent verification witness." }),
  });
  assert.equal(result.status, "replan_limit_reached");
  assert.equal(result.replan_count, 0);
  assert.equal(limited.calls(), 1);
  await assert.rejects(
    runAutonomousReplanCycle(limited.agent, "Review this coding change.", { domain: "coding", maxReplans: 4, evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: false }) }),
    /maxReplans/,
  );
});

test("replan cycle refuses credential-shaped evaluator instructions", async () => {
  const { agent } = cycleAgent();
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change.", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0, passed: false, replan_requested: true, replan_instruction: "Use the api_key from the task." }),
    }),
    /credential material/,
  );
});

test("execution policy stops a replanned provider call before dispatch", async () => {
  const { agent, calls } = cycleAgent([{ text: "first answer" }, { text: "must not dispatch" }]);
  const execution = await AutonomousExecutionController.create({ executionId: "cycle-execution-budget-1", domain: "coding", capability: "code_review", riskClass: "read_only", policy: { max_provider_calls: 1 } });
  let evaluations = 0;
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change.", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 1,
      execution,
      evaluate: () => {
        evaluations += 1;
        return { evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: evaluations === 1, replan_instruction: evaluations === 1 ? "Collect another independent witness." : null };
      },
    }),
    /max_provider_calls/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
});

test("replan policy failures after evaluation fail the shared execution", async () => {
  const { agent, calls } = cycleAgent([{ text: "first answer" }]);
  const execution = await AutonomousExecutionController.create({ executionId: "replan-policy-failure-1", domain: "coding", capability: "code_review", riskClass: "read_only", policy: { max_replans: 0 }, journal: new InMemoryAutonomousExecutionJournal() });
  await assert.rejects(
    runAutonomousReplanCycle(agent, "Review this coding change", {
      domain: "coding",
      approveProviderCall: true,
      maxReplans: 1,
      execution,
      evaluate: () => ({ evaluator_id: "reviewer", evaluator_version: "1", reward: 0.2, passed: false, replan_requested: true, replan_instruction: "Collect another independent witness." }),
    }),
    /max_replans/,
  );
  assert.equal(calls(), 1);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("replan cycle runs the same reviewed path for every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const { agent, calls } = cycleAgent();
  for (const domain of domains) {
    const execution = await AutonomousExecutionController.create({ executionId: `domain-execution-${domain}`, domain, capability: "domain_review", riskClass: "read_only" });
    const result = await runAutonomousReplanCycle(agent, `${domain} review`, {
      domain,
      approveProviderCall: true,
      maxReplans: 0,
      execution,
      evaluate: () => ({ evaluator_id: `${domain}-reviewer`, evaluator_version: "1", reward: 0.75, passed: true, replan_requested: false }),
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.final.run.blueprint.domain_profile.domain, domain);
    assert.equal(execution.state.status, "completed", domain);
    assert.equal(execution.state.provider_calls, 1, domain);
  }
  assert.equal(calls(), domains.length);
});

test("decision cycle keeps semantic routing, provider approval, and disagreement as separate gates", async () => {
  const semantic = cycleAgent([
    { route: { selected_domains: [{ domain: "coding", score: 0.94, rationale: "implementation" }], confidence: 0.94, abstain: false, abstain_reason: null } },
    { text: "semantic cycle answer" },
  ]);
  const result = await runAutonomousDecisionCycle(semantic.agent, "Help with an unfamiliar technical migration.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.semantic_route.status, "completed");
  assert.equal(result.route.source, "provider_semantic_hybrid");
  assert.equal(result.route.primary_domain, "coding");
  assert.equal(result.run.response.text, "semantic cycle answer");
  assert.equal(semantic.calls(), 2);

  const disagreement = cycleAgent([
    { route: { selected_domains: [{ domain: "biomedical", score: 0.95, rationale: "clinical wording" }], confidence: 0.95, abstain: false, abstain_reason: null } },
  ]);
  const refused = await runAutonomousDecisionCycle(disagreement.agent, "Debug this Rust repository and report the tests.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true },
  });
  assert.equal(refused.status, "provider_disagreement");
  assert.equal(refused.run, null);
  assert.equal(disagreement.calls(), 1);
});

test("decision cycle requires both semantic and execution approvals before any provider call", async () => {
  const gatedSemantic = cycleAgent([{ route: { selected_domains: [{ domain: "coding", score: 0.9, rationale: "code" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  const semanticGate = await runAutonomousDecisionCycle(gatedSemantic.agent, "an unfamiliar task", { semanticRouting: { enabled: true, approveProviderCall: false }, approveProviderCall: true });
  assert.equal(semanticGate.status, "approval_required");
  assert.equal(gatedSemantic.calls(), 0);

  const providerGate = cycleAgent();
  const providerResult = await runAutonomousDecisionCycle(providerGate.agent, "Debug this coding repository.", { domain: "coding", approveProviderCall: false });
  assert.equal(providerResult.status, "approval_required");
  assert.equal(providerGate.calls(), 0);
});

test("decision cycle forwards strict semantic policy admission before classifier dispatch", async () => {
  const gated = cycleAgent([{ route: { selected_domains: [{ domain: "coding", score: 0.9, rationale: "code" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  const result = await runAutonomousDecisionCycle(gated.agent, "an unfamiliar task", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, domainPolicyMode: "strict" },
  });
  assert.equal(result.status, "policy_review_required");
  assert.equal(result.semantic_route.status, "policy_review_required");
  assert.equal(result.semantic_route.domain_policy_admission.domain, "cross_domain");
  assert.ok(result.semantic_route.domain_policy_admission.reasons.includes("evidence_required_before_provider"));
  assert.equal(gated.calls(), 0);
});

test("decision cycle forwards selection gates into semantic routing", async () => {
  const gated = cycleAgent([{ route: { selected_domains: [{ domain: "coding", score: 0.9, rationale: "code" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  await assert.rejects(
    () => runAutonomousDecisionCycle(gated.agent, "Route this unfamiliar task", { semanticRouting: { enabled: true, approveProviderCall: true }, approveProviderCall: true, maxCostPerMillionTokens: 1, maxLatencyMs: 50, minQuality: 0.95 }),
    /abstain|eligible|cost|latency|quality/,
  );
  assert.equal(gated.calls(), 0, "decision-cycle semantic routing must honor caller gates before dispatch");
});

test("route handoff refuses a route from a different task", async () => {
  const { agent, calls } = cycleAgent();
  const route = await agent.route("Debug this coding repository.", { domain: "coding" });
  await assert.rejects(
    agent.run("Review this biomedical evidence.", { routeOverride: route, approveProviderCall: true }),
    /does not match the task digest/,
  );
  assert.equal(calls(), 0);
});

test("decision cycle executes every built-in domain through the same reviewed path", async () => {
  const { agent, calls } = cycleAgent();
  const examples = {
    coding: "debug this code repository",
    browser: "research and compare browser sources",
    data: "analyze this dataset lineage",
    science: "design a reproducible experiment",
    biomedical: "review biomedical evidence with safety boundaries",
    neuroscience: "analyze an EEG signal study",
    operations: "plan an incident rollback",
    enterprise: "review enterprise governance compliance",
    multi_agent: "delegate a bounded subtask to another agent",
    multimodal: "inspect an image and document together",
    cross_domain: "synthesize domain evidence",
    evaluation: "run a benchmark replay and failure analysis",
  };
  for (const [domain, task] of Object.entries(examples)) {
    const result = await runAutonomousDecisionCycle(agent, task, { domain, approveProviderCall: true });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.route.primary_domain, domain);
    assert.equal(result.run.blueprint.domain_profile.domain, domain);
  }
  assert.equal(calls(), 12);
});

test("cross-domain decision cycle settles specialist and synthesis credit as one trajectory", async () => {
  const { agent, calls } = cycleAgent();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const learning = new AutonomousLearningController(agent, { feedbackOutbox: outbox });
  const execution = await AutonomousExecutionController.create({ executionId: "cross-execution-1", domain: "cross_domain", capability: "cross_domain_synthesis", riskClass: "review_required", policy: { max_provider_calls: 4 }, journal: new InMemoryAutonomousExecutionJournal() });
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    execution,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence and safety boundaries." },
      { id: "neuro", domain: "neuroscience", task: "Analyze EEG signal design and interpretation limits." },
    ],
    learning: {
      controller: learning,
      trajectoryId: "cross-cycle-1",
      outbox: { workerId: "cross-cycle-worker" },
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "cross-reviewer", evaluator_version: "1", reward: 0.8, passed: true }])),
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.run.child_runs.length, 2);
  assert.equal(result.run.completed_children, 2);
  assert.equal(result.run.synthesis.response.text, "cycle answer");
  assert.equal(result.learning_episode_ids.length, 3);
  assert.equal(Object.keys(result.evaluation).length, 3);
  assert.equal(result.settlement.trajectory.trajectory.status, "settled");
  assert.equal(result.settlement.trajectory.settlements.length, 3);
  assert.equal(result.settlement.trajectory.settlements.at(-1).next_state.generation, 3);
  assert.equal(outbox.rows().filter((command) => command.status === "applied").length, 1);
  assert.equal(execution.state.status, "completed");
  assert.equal(execution.state.provider_calls, 3);
  assert.equal(calls(), 3);
});

test("provider-planned cross-domain cycles settle planner quality separately from the execution trajectory", async () => {
  const health = new InMemoryAutonomousModelHealthStore();
  const { agent } = providerPlanningCycleAgent({ learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    allowCrossDomain: true,
    providerPlanning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    maxParallelChildren: 1,
    learning: {
      controller: learning,
      trajectoryId: "planned-cross-cycle-trajectory",
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "execution-reviewer", evaluator_version: "1", reward: 0.76, passed: true }])),
      evaluatePlanning: () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.91, passed: true }),
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.planner_evaluation.reward, 0.91);
  assert.equal(result.planner_settlement.status, "settled");
  assert.equal(result.settlement.trajectory.settlements.length, 3);
  assert.equal(result.learning_episode_ids.length, 3);
  assert.equal(health.health({ model: "cycle-model" })[0]?.quality_observations, 4);
});

test("cross-domain replanning settles fan-out planner quality without merging it into child credit", async () => {
  const health = new InMemoryAutonomousModelHealthStore();
  const { agent } = providerPlanningCycleAgent({ learner: new AutonomousOnlineLearner(), modelHealthStore: health });
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainReplanCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    allowCrossDomain: true,
    providerPlanning: { approveProviderCall: true },
    acceptPlan: true,
    approveProviderCall: true,
    maxReplans: 0,
    maxParallelChildren: 1,
    learning: { controller: learning, episodePrefix: "planner-cross-replan", trajectoryIdPrefix: "planner-cross-replan-trajectory" },
    evaluate: (run) => ({
      evaluator_id: "execution-reviewer",
      evaluator_version: "1",
      reward: 0.77,
      passed: true,
      replan_requested: false,
      rewards: Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "execution-reviewer", evaluator_version: "1", reward: 0.77, passed: true }])),
    }),
    evaluatePlanning: () => ({ evaluator_id: "planner-reviewer", evaluator_version: "1", reward: 0.89, passed: true }),
  });
  assert.equal(result.status, "completed");
  assert.equal(result.planner_evaluations.length, 1);
  assert.equal(result.planner_settlements[0].status, "settled");
  assert.equal(result.final.planner_settlement.status, "settled");
  assert.equal(result.final.settlement.trajectory.settlements.length, 3);
  assert.equal(health.health({ model: "cycle-model" })[0]?.quality_observations, 4);
});

test("cross-domain replan cycle repeats bounded fan-out with unique trajectories and screened feedback", async () => {
  const cycle = cycleAgent();
  const learning = new AutonomousLearningController(cycle.agent);
  let evaluations = 0;
  const result = await runAutonomousCrossDomainReplanCycle(cycle.agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    maxReplans: 1,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
    learning: {
      controller: learning,
      episodePrefix: "cross-replan-test",
      trajectoryIdPrefix: "cross-replan-trajectory",
    },
    evaluate: (run) => {
      const shouldReplan = evaluations++ === 0;
      return {
        evaluator_id: "cross-reviewer",
        evaluator_version: "1",
        reward: shouldReplan ? 0.35 : 0.9,
        passed: !shouldReplan,
        replan_requested: shouldReplan,
        replan_instruction: shouldReplan ? "Resolve the specialist disagreement and make uncertainty explicit." : null,
        rewards: Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, {
          evaluator_id: "cross-reviewer",
          evaluator_version: "1",
          reward: shouldReplan ? 0.35 : 0.9,
          passed: !shouldReplan,
        }])),
      };
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.replan_count, 1);
  assert.equal(result.attempts.length, 2);
  assert.notEqual(result.attempts[0].trajectory_id, result.attempts[1].trajectory_id);
  assert.equal(result.settlements.length, 2);
  assert.equal(result.final.run.learning_episode_ids.length, 3);
  assert.equal(result.final.settlement.trajectory.settlements.length, 3);
  assert.equal(result.final.settlement.trajectory.settlements.at(-1).next_state.generation, 6);
  assert.equal(cycle.calls(), 6);
  assert.equal(JSON.stringify(cycle.bodies[3]).includes("autonomous-cross-domain-replan-2"), true);
  assert.equal(JSON.stringify({ attempts: result.attempts, evaluations: result.evaluations }).includes("Resolve the specialist disagreement"), false);
});

test("cross-domain replan persistence makes terminal completion idempotent across restart", async () => {
  const cycle = cycleAgent();
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  const options = {
    approveProviderCall: true,
    maxReplans: 0,
    cycleId: "restartable-cross-cycle",
    stateStore,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
    evaluate: () => ({ evaluator_id: "cross-restart-reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false, rewards: {} }),
  };
  const first = await runAutonomousCrossDomainReplanCycle(cycle.agent, "Research a biomedical neuroscience experiment with EEG patient evidence", options);
  assert.equal(first.status, "completed");
  assert.equal((await stateStore.load("restartable-cross-cycle")).phase, "terminal");
  const providerCalls = cycle.calls();
  const replay = await runAutonomousCrossDomainReplanCycle(cycle.agent, "Research a biomedical neuroscience experiment with EEG patient evidence", options);
  assert.equal(replay.status, "completed");
  assert.equal(replay.final, null);
  assert.equal(cycle.calls(), providerCalls);
});

test("cross-domain persistence rehydrates a completed fan-out before evaluator settlement", async () => {
  const cycle = cycleAgent();
  const task = "Research a biomedical neuroscience experiment with EEG patient evidence";
  const route = await cycle.agent.route(task, { allowCrossDomain: true });
  const stateStore = new InMemoryAutonomousCycleReplanStateStore();
  let interruptedRun = null;
  const base = {
    approveProviderCall: true,
    routeOverride: route,
    cycleId: "restartable-cross-evaluator-cycle",
    stateStore,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  };
  await assert.rejects(
    runAutonomousCrossDomainReplanCycle(cycle.agent, task, {
      ...base,
      evaluate: (run) => {
        interruptedRun = run;
        throw new Error("simulated cross-domain evaluator interruption");
      },
    }),
    /simulated cross-domain evaluator interruption/,
  );
  assert.equal((await stateStore.load("restartable-cross-evaluator-cycle")).phase, "evaluation_pending");
  const resumed = await runAutonomousCrossDomainReplanCycle(cycle.agent, task, {
    ...base,
    rehydrateRoute: () => route,
    rehydrateRun: () => interruptedRun,
    evaluate: () => ({ evaluator_id: "cross-restart-reviewer", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false, rewards: {} }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.attempts.length, 1);
  assert.equal(cycle.calls(), 3);
});

test("cross-domain replan cycle refuses incomplete per-episode evaluator coverage", async () => {
  const { agent, calls } = cycleAgent();
  const learning = new AutonomousLearningController(agent);
  await assert.rejects(
    runAutonomousCrossDomainReplanCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
      approveProviderCall: true,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
      ],
      learning: { controller: learning },
      evaluate: () => ({
        evaluator_id: "cross-reviewer",
        evaluator_version: "1",
        reward: 0,
        passed: false,
        replan_requested: false,
        rewards: {},
      }),
    }),
    /cover exactly every pending learning episode/,
  );
  assert.equal(calls(), 3);
});

test("cross-domain decision cycle propagates structured output through fan-out and synthesis", async () => {
  const cycle = cycleAgent([{ text: JSON.stringify({ answer: "specialist-1" }) }, { text: JSON.stringify({ answer: "specialist-2" }) }, { text: JSON.stringify({ answer: "synthesis" }) }]);
  const responseSchema = { type: "object", additionalProperties: false, properties: { answer: { type: "string" } }, required: ["answer"] };
  const result = await runAutonomousCrossDomainDecisionCycle(cycle.agent, "Return a structured biomedical neuroscience synthesis.", {
    approveProviderCall: true,
    maxParallelChildren: 1,
    requireJson: true,
    responseSchema,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.deepEqual(result.run.child_runs.map((child) => child.result.response.structured), [{ answer: "specialist-1" }, { answer: "specialist-2" }]);
  assert.deepEqual(result.run.synthesis.response.structured, { answer: "synthesis" });
  assert.deepEqual(cycle.bodies.map((body) => body.response_format), [{ type: "json_object" }, { type: "json_object" }, { type: "json_object" }]);
});

test("cross-domain decision cycle preserves synthesis tool-loop exhaustion", async () => {
  const { agent, calls } = toolLoopAgent(2);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
    approveProviderCall: true,
    tools: loopTools,
    authorizeAndExecute: async (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "turn_limit_reached");
  assert.equal(result.run.status, "turn_limit_reached");
  assert.equal(result.run.synthesis.status, "turn_limit_reached");
  assert.equal(result.run.completed_children, 2);
  assert.equal(calls(), 6);
});

test("cross-domain decision cycle fails the shared execution when settlement throws", async () => {
  const { agent, calls } = cycleAgent();
  const execution = await AutonomousExecutionController.create({ executionId: "cross-post-run-failure-1", domain: "cross_domain", capability: "cross_domain_synthesis", riskClass: "review_required", policy: { max_provider_calls: 4 }, journal: new InMemoryAutonomousExecutionJournal() });
  const learning = new AutonomousLearningController(agent);
  await assert.rejects(
    runAutonomousCrossDomainDecisionCycle(agent, "Research a biomedical neuroscience experiment with EEG patient evidence", {
      approveProviderCall: true,
      execution,
      subtasks: [
        { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
        { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
      ],
      learning: {
        controller: learning,
        trajectoryId: "cross-post-run-failure",
        evaluate: async () => { throw new Error("cross-domain evaluator unavailable"); },
      },
    }),
    /cross-domain evaluator unavailable/,
  );
  assert.equal(calls(), 3);
  assert.equal(execution.state.status, "failed");
  assert.equal(execution.state.last_event_kind, "failed");
});

test("cross-domain decision cycle applies semantic routing before fan-out and preserves both gates", async () => {
  const semantic = cycleAgent([
    { route: { selected_domains: [{ domain: "biomedical", score: 0.93, rationale: "biomedical evidence" }, { domain: "neuroscience", score: 0.91, rationale: "EEG study" }], confidence: 0.92, abstain: false, abstain_reason: null } },
    { text: "biomedical specialist" },
    { text: "neuroscience specialist" },
    { text: "integrated synthesis" },
  ]);
  const result = await runAutonomousCrossDomainDecisionCycle(semantic.agent, "Help with an unfamiliar biomedical neuroscience study.", {
    approveProviderCall: true,
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: true },
    subtasks: [
      { id: "bio", domain: "biomedical", task: "Review biomedical evidence." },
      { id: "neuro", domain: "neuroscience", task: "Review neuroscience evidence." },
    ],
  });
  assert.equal(result.status, "completed");
  assert.equal(result.semantic_route.status, "completed");
  assert.equal(result.route.source, "provider_semantic_hybrid");
  assert.deepEqual(result.run.child_runs.map((child) => child.domain), ["biomedical", "neuroscience"]);
  assert.equal(result.run.synthesis.response.text, "integrated synthesis");
  assert.equal(semantic.calls(), 4);

  const gatedSemantic = cycleAgent([{ route: { selected_domains: [{ domain: "biomedical", score: 0.9, rationale: "bio" }, { domain: "neuroscience", score: 0.9, rationale: "neuro" }], confidence: 0.9, abstain: false, abstain_reason: null } }]);
  const semanticGate = await runAutonomousCrossDomainDecisionCycle(gatedSemantic.agent, "an unfamiliar biomedical neuroscience study", { approveProviderCall: true, semanticRouting: { enabled: true, approveProviderCall: false } });
  assert.equal(semanticGate.status, "approval_required");
  assert.equal(gatedSemantic.calls(), 0);

  const gatedExecution = cycleAgent();
  const executionGate = await runAutonomousCrossDomainDecisionCycle(gatedExecution.agent, "biomedical neuroscience", { approveProviderCall: false, synthesize: false, subtasks: [{ id: "bio", domain: "biomedical", task: "bio" }, { id: "neuro", domain: "neuroscience", task: "neuro" }] });
  assert.equal(executionGate.status, "approval_required");
  assert.equal(gatedExecution.calls(), 0);
});

test("cross-domain decision cycle settles partial specialist trajectories without inventing synthesis", async () => {
  const { agent, calls } = cycleAgent();
  const learning = new AutonomousLearningController(agent);
  const result = await runAutonomousCrossDomainDecisionCycle(agent, "biomedical neuroscience", {
    approveProviderCall: true,
    synthesize: false,
    subtasks: [
      { id: "bio", domain: "biomedical", task: "bio" },
      { id: "neuro", domain: "neuroscience", task: "neuro" },
    ],
    learning: {
      controller: learning,
      trajectoryId: "cross-cycle-specialists-only",
      evaluate: (run) => Object.fromEntries(run.learning_episode_ids.map((episodeId) => [episodeId, { evaluator_id: "specialist-reviewer", evaluator_version: "1", reward: 0.7, passed: true }])),
    },
  });
  assert.equal(result.status, "children_completed");
  assert.equal(result.run.synthesis, null);
  assert.equal(result.learning_episode_ids.length, 2);
  assert.equal(result.settlement.trajectory.settlements.length, 2);
  assert.equal(calls(), 2);
});

test("cross-domain fan-out accepts a representative pair for every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const { agent, calls } = cycleAgent();
  for (let index = 0; index < domains.length; index += 1) {
    const left = domains[index];
    const right = domains[(index + 1) % domains.length];
    const task = `${left} ${right}`;
    const route = await agent.route(task, { maxDomains: 2, minMargin: 0.2, allowCrossDomain: true });
    assert.equal(route.cross_domain, true, `${left}/${right} should fan out`);
    const result = await runAutonomousCrossDomainDecisionCycle(agent, task, {
      routeOverride: route,
      approveProviderCall: true,
      synthesize: false,
      subtasks: [
        { id: "left", domain: left, task: `${left} specialist review` },
        { id: "right", domain: right, task: `${right} specialist review` },
      ],
    });
    assert.equal(result.status, "children_completed", `${left}/${right} should complete`);
    assert.deepEqual(result.run.child_runs.map((child) => child.domain), [left, right]);
    assert.equal(result.run.synthesis, null);
  }
  assert.equal(calls(), domains.length * 2);
});

test("cross-domain replan keeps the same reviewed contract for every built-in domain", async () => {
  const domains = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"];
  const { agent, calls } = cycleAgent();
  for (let index = 0; index < domains.length; index += 1) {
    const left = domains[index];
    const right = domains[(index + 1) % domains.length];
    const task = `${left} ${right}`;
    const route = await agent.route(task, { maxDomains: 2, minMargin: 0.2, allowCrossDomain: true });
    const result = await runAutonomousCrossDomainReplanCycle(agent, task, {
      routeOverride: route,
      approveProviderCall: true,
      maxReplans: 0,
      synthesize: false,
      subtasks: [
        { id: "left", domain: left, task: `${left} specialist review` },
        { id: "right", domain: right, task: `${right} specialist review` },
      ],
      evaluate: () => ({ evaluator_id: "domain-reviewer", evaluator_version: "1", reward: 1, passed: true, replan_requested: false, rewards: {} }),
    });
    assert.equal(result.status, "completed", `${left}/${right} should complete`);
    assert.equal(result.final.run.status, "children_completed", `${left}/${right} should retain child completion`);
  }
  assert.equal(calls(), domains.length * 2);
});
