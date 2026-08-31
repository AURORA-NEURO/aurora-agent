import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  InMemoryAutonomousRunTraceStore,
  builtinAutonomousDomainProfiles,
  LLMRuntime,
} from "../dist/index.js";

const taskFor = (domain) => `review a bounded ${domain} task and report uncertainty`;

const model = {
  provider: "brain-trace-offline",
  model: "brain-trace-model",
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

function brain() {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("brain-trace-offline", () => ({ output_text: "bounded brain result" }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return new AutonomousBrainFacade({ agent });
}

async function launchAdmission(facade, decision = "approve") {
  const profiles = await builtinAutonomousDomainProfiles();
  const availableToolNames = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const availableEvidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const deploymentCapabilities = {
    persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  };
  const preflight = await facade.launchPreflight({ availableToolNames, availableEvidence, deploymentCapabilities });
  return facade.admitLaunchPreflight(preflight, decision === "approve"
    ? { decision, authorizationDigest: "a".repeat(64) }
    : { decision });
}

test("brain facade trace spans plan compilation and provider execution for every domain", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const task = taskFor(domain);
    const traced = await facade.executeWithTrace({ task, domain }, {
      traceStore: store,
      runId: `facade-${domain}`,
      approveProviderCall: true,
      run: { candidates: [model] },
    });
    assert.equal(traced.execution.status, "completed", domain);
    assert.equal(traced.trace.status, "completed", domain);
    assert.equal(traced.trace.provider_invocations, 1, domain);
    assert.ok(traced.trace.route_digest);
    assert.equal(traced.trace.plan_digest, traced.execution.plan.plan_digest);
    const events = store.events({ run_id: `facade-${domain}` });
    assert.deepEqual(events.map((event) => event.phase), [
      "started", "plan_compiled", "model_selection_started", "model_selection_finished", "provider_invocation_started", "provider_invocation_finished", "completed",
    ], domain);
    assert.equal(JSON.stringify(traced.trace).includes(task), false);
  }
  assert.equal(store.verifyIntegrity().verified, true);
});

test("brain facade composes launch admission with metadata traces across core execution modes", async () => {
  const facade = brain();
  const admission = await launchAdmission(facade);
  assert.equal(admission.status, "approved");
  const store = new InMemoryAutonomousRunTraceStore();
  const directTask = taskFor("coding");
  const direct = await facade.executeWithLaunchAdmissionAndTrace({ task: directTask, domain: "coding" }, admission, {
    traceStore: store,
    runId: "launch-trace-direct",
    approveProviderCall: true,
    run: { candidates: [model] },
  });
  assert.equal(direct.execution.status, "completed");
  assert.equal(direct.trace.status, "completed");
  assert.equal(direct.trace.provider_invocations, 1);
  assert.equal(JSON.stringify(direct.trace).includes(directTask), false);

  const automatic = await facade.executeAutoWithLaunchAdmissionAndTrace({ task: taskFor("browser"), domain: "browser" }, admission, {
    traceStore: store,
    runId: "launch-trace-automatic",
    approveProviderCall: true,
    candidates: [model],
  });
  assert.equal(automatic.execution.status, "completed");
  assert.equal(automatic.trace.status, "completed");
  assert.equal(automatic.trace.provider_invocations, 1);

  const cycle = await facade.executeCycleWithLaunchAdmissionAndTrace({ task: taskFor("science"), domain: "science" }, admission, {
    traceStore: store,
    runId: "launch-trace-cycle",
    approveProviderCall: true,
  });
  assert.equal(cycle.execution.status, "completed");
  assert.equal(cycle.trace.status, "completed");
  assert.ok(cycle.execution.cycle);

  const adaptive = await facade.executeAdaptiveCycleWithLaunchAdmissionAndTrace({ task: taskFor("evaluation"), domain: "evaluation" }, admission, {
    traceStore: store,
    runId: "launch-trace-adaptive",
    approveProviderCall: true,
    adaptive: {
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "launch-trace-evaluator", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
    },
  });
  assert.equal(adaptive.execution.status, "completed");
  assert.equal(adaptive.trace.status, "completed");
  assert.equal(adaptive.execution.adaptive.attempts.length, 1);
  assert.equal(store.verifyIntegrity().verified, true);

  const held = await launchAdmission(facade, "hold");
  await assert.rejects(
    () => facade.executeWithLaunchAdmissionAndTrace({ task: "held launch trace", domain: "coding" }, held, {
      traceStore: store,
      runId: "launch-trace-held",
      approveProviderCall: true,
      run: { candidates: [model] },
    }),
    /not approved/,
  );
  assert.equal(store.events({ run_id: "launch-trace-held" }).length, 0);
});

test("brain facade trace preserves approval pauses and cross-domain fan-out without inventing success", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  const paused = await facade.executeWithTrace({ task: taskFor("coding"), domain: "coding" }, {
    traceStore: store,
    runId: "facade-paused",
    run: { candidates: [model] },
  });
  assert.equal(paused.execution.status, "approval_required");
  assert.equal(paused.trace.status, "paused");
  assert.equal(paused.trace.provider_invocations, 0);
  assert.equal(store.events({ run_id: "facade-paused", phase: "plan_compiled" }).length, 1);

  const cross = await facade.executeWithTrace({ task: "coordinate biomedical neuroscience evidence review" }, {
    traceStore: store,
    runId: "facade-cross",
    approveProviderCall: true,
    run: { candidates: [model] },
  });
  assert.equal(cross.execution.status, "completed");
  assert.equal(cross.trace.status, "completed");
  assert.ok(cross.trace.domains.includes("cross_domain"));
  assert.ok(cross.trace.provider_invocations >= 2);
  assert.ok(store.events({ run_id: "facade-cross", phase: "plan_compiled" }).length === 1);
  assert.equal(JSON.stringify(cross.trace).includes("coordinate biomedical neuroscience evidence review"), false);
});

test("brain facade traced plan replay revalidates identity before provider dispatch", async () => {
  const facade = brain();
  const store = new InMemoryAutonomousRunTraceStore();
  const task = taskFor("evaluation");
  const plan = await facade.plan({ task, domain: "evaluation" });
  const traced = await facade.executePlannedWithTrace(plan, { task, domain: "evaluation" }, {
    traceStore: store,
    runId: "facade-replay",
    approveProviderCall: true,
    run: { candidates: [model] },
  });
  assert.equal(traced.execution.status, "completed");
  assert.equal(traced.execution.plan.plan_digest, plan.plan_digest);
  assert.equal(traced.trace.plan_digest, plan.plan_digest);
  await assert.rejects(
    () => facade.executePlannedWithTrace(plan, { task: `${task} changed`, domain: "evaluation" }, {
      traceStore: store,
      runId: "facade-replay-tampered",
      approveProviderCall: true,
      run: { candidates: [model] },
    }),
    /does not match/,
  );
  assert.equal(store.events({ run_id: "facade-replay-tampered" }).length, 0);
});
