import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AutonomousAgent,
  AutonomousLearningController,
  AutonomousOnlineLearner,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousLearningEpisodeStore,
  InMemoryAutonomousLearningFeedbackOutboxStore,
  InMemoryAutonomousLearningTrajectoryStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  InMemoryAutonomousWorkflowCycleStateStore,
  LLMRuntime,
  autonomousWorkflowEvaluatorForDomain,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
  runAutonomousWorkflowCycle,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), { status, headers: { "content-type": "application/json" } });
}

function model() {
  return {
    provider: "cycle-provider",
    model: "cycle-model",
    capabilities: [
      "reasoning", "code", "web", "data", "science", "biomedical", "neuroscience",
      "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output",
    ],
    context_window_tokens: 64_000,
    max_output_tokens: 4_000,
    quality: 0.95,
    latency_ms: 40,
    cost_per_million_tokens: 5,
    reliability: 0.99,
  };
}

function stagePayload(init) {
  let body = {};
  try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
  const prompt = JSON.stringify(body.messages ?? []);
  const stageId = prompt.match(/Execute workflow stage ([A-Za-z0-9_.:-]+)/)?.[1] ?? "stage";
  return {
    stage_id: stageId,
    status: "completed",
    evidence: [`evidence-${stageId}`],
    uncertainty: [],
    notes: `verified ${stageId}`,
    next_actions: [],
  };
}

async function makeAgent(withLearning = false) {
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(stagePayload(init)) }, finish_reason: "stop" }] }),
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm, withLearning ? { learner: new AutonomousOnlineLearner() } : {});
  agent.registerModel(model());
  return agent;
}

function perfectEvidence(execution) {
  return {
    stages: execution.blueprint.workflow.stages.map((stage) => ({
      stage_id: stage.id,
      signals: Object.fromEntries(stage.evaluator_signals.map((signal) => [signal, 1])),
    })),
  };
}

test("workflow cycle supervises every built-in domain with explicit evidence", async () => {
  const agent = await makeAgent();
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
    const cycle = await runAutonomousWorkflowCycle(`Run a verified ${profile.domain} workflow.`, executor, {
      domain: profile.domain,
      candidates: agent.models(),
      approveProviderCall: true,
      jobId: `cycle-${profile.domain}`,
      evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
    });
    assert.equal(cycle.status, "completed", profile.domain);
    assert.equal(cycle.attempts.length, 1, profile.domain);
    assert.equal(cycle.evaluations[0].status, "passed", profile.domain);
    assert.equal(cycle.evaluations[0].reward, 1, profile.domain);
  }
});

test("automatic evaluator replanning recovers every built-in domain without granting new authority", async () => {
  const agent = await makeAgent();
  const profiles = await builtinAutonomousDomainProfiles();
  for (const profile of profiles) {
    const evaluator = await autonomousWorkflowEvaluatorForDomain(profile.domain);
    const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
    let evaluations = 0;
    const cycle = await runAutonomousWorkflowCycle(`Recover a failed ${profile.domain} workflow.`, executor, {
      domain: profile.domain,
      candidates: agent.models(),
      approveProviderCall: true,
      jobId: `automatic-replan-${profile.domain}`,
      maxReplans: 1,
      evaluator,
      automaticReplan: true,
      evaluate: async (execution) => {
        const firstAttempt = evaluations === 0;
        evaluations += 1;
        return {
          evidence: {
            stages: execution.blueprint.workflow.stages.map((stage) => ({
              stage_id: stage.id,
              signals: Object.fromEntries(stage.evaluator_signals.map((signal) => [signal, firstAttempt ? 0 : 1])),
            })),
          },
        };
      },
    });
    assert.equal(cycle.status, "completed", profile.domain);
    assert.equal(cycle.replan_count, 1, profile.domain);
    assert.equal(cycle.attempts.length, 2, profile.domain);
    assert.equal(cycle.evaluations[0].replan_requested, true, profile.domain);
    assert.equal(cycle.evaluations[0].failure_class, "evaluator_gate_failed", profile.domain);
    assert.match(cycle.evaluations[0].replan_instruction_digest, /^[0-9a-f]{64}$/, profile.domain);
    assert.equal(cycle.evaluations[1].passed, true, profile.domain);
  }
});

test("workflow cycle refuses evaluator drift between decision and delayed-credit settlement", async () => {
  const agent = await makeAgent(true);
  const learning = new AutonomousLearningController(agent);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore(), { learning });
  const differentEvaluator = await autonomousWorkflowEvaluatorForDomain("coding", { evaluatorVersion: "different-rubric" });
  await assert.rejects(
    () => runAutonomousWorkflowCycle("Reject evaluator drift.", executor, {
      domain: "coding",
      candidates: agent.models(),
      approveProviderCall: true,
      evaluator: differentEvaluator,
      learning: { controller: learning },
      evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
    }),
    /match the learning controller evaluator/,
  );
});

test("workflow cycle composes semantic routing with durable stage supervision", async () => {
  let calls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls += 1;
      let body = {};
      try { body = JSON.parse(String(init?.body ?? "{}")); } catch { /* bounded fixture fallback */ }
      const isRouter = JSON.stringify(body.messages ?? []).includes("bounded autonomous task router");
      const content = isRouter
        ? JSON.stringify({ selected_domains: [{ domain: "coding", score: 0.99, rationale: "The request is a coding workflow." }], confidence: 0.99, abstain: false, abstain_reason: null })
        : JSON.stringify(stagePayload(init));
      return jsonResponse({ choices: [{ message: { role: "assistant", content }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("semantic-cycle-provider", "https://semantic-cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  const candidate = { ...model(), provider: "semantic-cycle-provider", model: "semantic-cycle-model" };
  agent.registerModel(candidate);
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  const cycle = await runAutonomousWorkflowCycle("Help with an unfamiliar coding migration.", executor, {
    candidates: [candidate],
    semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false, maxDomains: 1 },
    approveProviderCall: true,
    jobId: "semantic-cycle-1",
    evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
  });
  assert.equal(cycle.status, "completed");
  assert.equal(cycle.final.route.primary_domain, "coding");
  assert.equal(cycle.final.semantic_route_status, "completed");
  assert.equal(cycle.final.checkpoint.route_digest, cycle.final.route.route_digest);
  assert.equal(calls, 6, "the cycle should route once and execute the five coding stages");
});

test("workflow cycle gives the evaluator a bounded replan path and settles stage trajectories", async () => {
  const agent = await makeAgent(true);
  const episodes = new InMemoryAutonomousLearningEpisodeStore();
  const trajectories = new InMemoryAutonomousLearningTrajectoryStore();
  const outbox = new InMemoryAutonomousLearningFeedbackOutboxStore();
  const learning = new AutonomousLearningController(agent, { episodes, trajectories, feedbackOutbox: outbox });
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore(), { learning });
  let evaluations = 0;
  const cycle = await runAutonomousWorkflowCycle("Replan this verified coding workflow once.", executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    jobId: "cycle-replan-coding",
    maxReplans: 1,
    learning: { controller: learning, trajectoryIdPrefix: "cycle-replan-trajectory", outbox: { workerId: "workflow-cycle-worker" } },
    evaluate: async (execution) => {
      evaluations += 1;
      return {
        evidence: perfectEvidence(execution),
        replan_requested: evaluations === 1,
        replan_instruction: evaluations === 1 ? "Add one independent verification pass." : null,
        feedback_digest: evaluations === 1 ? "a".repeat(64) : null,
      };
    },
  });
  assert.equal(cycle.status, "completed");
  assert.equal(cycle.replan_count, 1);
  assert.equal(cycle.attempts.length, 2);
  assert.equal(cycle.evaluations.length, 2);
  assert.equal(cycle.evaluations[0].replan_requested, true);
  assert.equal(cycle.evaluations[1].passed, true);
  assert.equal(cycle.settlements.length, 2);
  assert.equal(episodes.pending().length, 0);
  assert.equal(agent.learner.snapshot().generation, 10);
  assert.equal(outbox.rows().filter((command) => command.status === "applied").length, 2);
  assert.match(cycle.attempts[1].job_id, /:attempt-2$/);
});

test("workflow cycle refuses credential-shaped evaluator guidance before another provider attempt", async () => {
  const agent = await makeAgent();
  let calls = 0;
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  await assert.rejects(
    () => runAutonomousWorkflowCycle("Reject unsafe workflow feedback.", executor, {
      domain: "coding",
      candidates: agent.models(),
      approveProviderCall: true,
      jobId: "cycle-unsafe-feedback",
      evaluate: async (execution) => {
        calls += 1;
        return { evidence: perfectEvidence(execution), replan_requested: true, replan_instruction: "Use api_key=never." };
      },
    }),
    /credential-shaped material/,
  );
  assert.equal(calls, 1);
});

test("workflow cycle persists the evaluator boundary and rehydrates without replaying provider work", async () => {
  let providerCalls = 0;
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      providerCalls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: JSON.stringify(stagePayload(init)) }, finish_reason: "stop" }] });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("cycle-provider", "https://cycle.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const checkpointStore = new InMemoryAutonomousWorkflowCheckpointStore();
  const cycleStore = new InMemoryAutonomousWorkflowCycleStateStore();
  const executor = new AutonomousWorkflowExecutor(agent, checkpointStore);
  const task = "Persist this coding workflow evaluator boundary.";
  let capturedExecution;
  await assert.rejects(
    () => runAutonomousWorkflowCycle(task, executor, {
      domain: "coding",
      candidates: agent.models(),
      approveProviderCall: true,
      cycleId: "persistent-cycle-1",
      jobId: "persistent-workflow-1",
      stateStore: cycleStore,
      evaluate: async (execution) => {
        capturedExecution = execution;
        throw new Error("simulated evaluator interruption");
      },
    }),
    /simulated evaluator interruption/,
  );
  assert.equal(providerCalls, 5);
  const pending = await cycleStore.load("persistent-cycle-1");
  assert.equal(pending.phase, "evaluation_pending");
  assert.equal(JSON.stringify(pending).includes(task), false);
  assert.equal(JSON.stringify(pending).includes("evidence-scope"), false);

  const resumed = await runAutonomousWorkflowCycle(task, executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    cycleId: "persistent-cycle-1",
    jobId: "persistent-workflow-1",
    stateStore: cycleStore,
    rehydrateExecution: async (context) => {
      assert.equal(context.phase, "evaluation_pending");
      return capturedExecution;
    },
    evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.attempts.length, 1);
  assert.equal(providerCalls, 5, "rehydrating the evaluator boundary must not dispatch another provider call");

  const terminal = await cycleStore.load("persistent-cycle-1");
  assert.equal(terminal.phase, "terminal");
  assert.equal(terminal.evaluations.length, 1);
  const replayed = await runAutonomousWorkflowCycle(task, executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    cycleId: "persistent-cycle-1",
    jobId: "persistent-workflow-1",
    stateStore: cycleStore,
    evaluate: async () => { throw new Error("terminal replay must not evaluate"); },
  });
  assert.equal(replayed.final, null);
  assert.equal(replayed.status, "completed");
  assert.equal(providerCalls, 5);
});

test("workflow cycle state snapshots are digest-bound and metadata-only", async () => {
  const store = new InMemoryAutonomousWorkflowCycleStateStore();
  const persistence = {
    value: null,
    async read() { return this.value; },
    async write(snapshot) { this.value = snapshot; },
  };
  // A cycle state is produced by the supervisor; this adapter test uses a completed cycle to
  // verify the production snapshot bridge without retaining the private execution response.
  const agent = await makeAgent();
  const executor = new AutonomousWorkflowExecutor(agent, new InMemoryAutonomousWorkflowCheckpointStore());
  await runAutonomousWorkflowCycle("Snapshot this coding cycle.", executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    cycleId: "snapshot-cycle-1",
    jobId: "snapshot-workflow-1",
    stateStore: store,
    evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
  });
  const coordinator = new (await import("../dist/index.js")).AutonomousWorkflowCyclePersistenceCoordinator(store, persistence);
  const flushed = await coordinator.flush();
  assert.equal(flushed.retention, "metadata_only");
  assert.equal(JSON.stringify(persistence.value).includes("Snapshot this coding cycle"), false);
  const restoredStore = new InMemoryAutonomousWorkflowCycleStateStore();
  const restoredCoordinator = new (await import("../dist/index.js")).AutonomousWorkflowCyclePersistenceCoordinator(restoredStore, persistence);
  const restored = await restoredCoordinator.restore();
  assert.equal(restored.restored, true);
  assert.equal(restored.cycles, 1);
  const tampered = structuredClone(persistence.value);
  tampered.states[0].terminal_status = "failed";
  persistence.value = tampered;
  await assert.rejects(() => restoredCoordinator.restore(), /digest/);
});

test("workflow cycle rehydrates screened evaluator guidance from a restart handoff", async () => {
  const agent = await makeAgent();
  const checkpointStore = new InMemoryAutonomousWorkflowCheckpointStore();
  const cycleStore = new InMemoryAutonomousWorkflowCycleStateStore();
  const executor = new AutonomousWorkflowExecutor(agent, checkpointStore);
  const originalStart = executor.start.bind(executor);
  let starts = 0;
  executor.start = async (...args) => {
    starts += 1;
    if (starts === 2) throw new Error("simulated worker interruption after replan handoff");
    return originalStart(...args);
  };
  const task = "Restart this evaluator-guided coding workflow.";
  await assert.rejects(
    () => runAutonomousWorkflowCycle(task, executor, {
      domain: "coding",
      candidates: agent.models(),
      approveProviderCall: true,
      cycleId: "handoff-cycle-1",
      jobId: "handoff-workflow-1",
      maxReplans: 1,
      stateStore: cycleStore,
      evaluate: async (execution) => ({
        evidence: perfectEvidence(execution),
        replan_requested: true,
        replan_instruction: "Add an independent verification pass.",
      }),
    }),
    /simulated worker interruption/,
  );
  const handoff = await cycleStore.load("handoff-cycle-1");
  assert.equal(handoff.phase, "execution_pending");
  executor.start = originalStart;
  const resumed = await runAutonomousWorkflowCycle(task, executor, {
    domain: "coding",
    candidates: agent.models(),
    approveProviderCall: true,
    cycleId: "handoff-cycle-1",
    jobId: "handoff-workflow-1",
    maxReplans: 1,
    stateStore: cycleStore,
    rehydrateReplanInstruction: async (context) => {
      assert.equal(context.phase, "execution_pending");
      return "Add an independent verification pass.";
    },
    evaluate: async (execution) => ({ evidence: perfectEvidence(execution) }),
  });
  assert.equal(resumed.status, "completed");
  assert.equal(resumed.attempts.length, 2);
  assert.equal(resumed.evaluations.length, 2);
  assert.equal((await cycleStore.load("handoff-cycle-1")).phase, "terminal");
});
