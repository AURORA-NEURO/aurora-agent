import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousConnectorObservation,
  AutonomousConnectorRegistration,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  CredentialStore,
  InMemoryAutonomousConnectorReceiptJournal,
  LLMRuntime,
  ToolCatalogue,
  applyAutonomousOrderedStepPlan,
  connectorMissionPlannerSteps,
  connectorMissionProtectedContractDigest,
  digestJsonSync,
  runAutonomousConnectorMission,
} from "../dist/index.js";

function manifest() {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "local.mission-connector",
    version: "1.0.0",
    provider: "local-test-connector",
    connector_kind: "provider_api",
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
    capabilities: ["evidence_read"],
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: ["opaque-session-ref"],
      does_not_claim: ["credential validity", "provider availability"],
    },
  };
}

async function fixture() {
  let calls = 0;
  const registration = new AutonomousConnectorRegistration(manifest(), async (_manifest, request) => {
    calls += 1;
    return new AutonomousConnectorObservation({
      source: "local-deterministic-connector",
      stage_id: request.stage_id ?? request.step_id ?? "unknown",
      observation_digest: "b".repeat(64),
    }, "observed");
  });
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const registry = new AutonomousConnectorRegistry([registration]);
  const runtime = new AutonomousConnectorRuntime(registry, { receiptStore: journal });
  const catalogue = await ToolCatalogue.fromDefinitions([{
    name: "connector_probe",
    description: "bounded connector mission probe",
    inputSchema: { type: "object", additionalProperties: true },
  }]);
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }), { toolCatalogue: catalogue });
  return { agent, catalogue, calls: () => calls, journal, registry, runtime };
}

function mission(steps, missionId = "connector-mission-test") {
  return {
    mission_id: missionId,
    goal: "coordinate a bounded connector mission across reviewed domains",
    steps,
    policy: {
      execute: true,
      stop_on_error: true,
      allow_side_effects: false,
      max_steps: 128,
      max_step_output_bytes: 100_000,
      max_total_output_bytes: 1_000_000,
      execution_mode: "serial",
      max_parallelism: 1,
      allowed_tools: ["connector_probe"],
    },
  };
}

function step(id, domain, dependsOn = [], value = id) {
  return {
    id,
    domain,
    capability: "evidence_read",
    objective: `perform the reviewed ${domain} connector observation`,
    tool: "connector_probe",
    arguments: { value },
    ...(dependsOn.length ? { depends_on: dependsOn } : {}),
  };
}

function refinementFor(value) {
  const plannerSteps = connectorMissionPlannerSteps(value);
  return {
    schema: "bioprism-typescript-autonomous-ordered-step-plan-refinement/0.1",
    status: "completed",
    task_digest: digestJsonSync({ task: value.goal }),
    base_plan_digest: digestJsonSync({ steps: plannerSteps }),
    protected_contract_digest: connectorMissionProtectedContractDigest(value),
    priority_step_ids: plannerSteps.map(({ id }) => id),
    focus_step_ids: plannerSteps.map(({ id }) => id),
    review_required: false,
    confidence: 1,
    selected_model: null,
    selection_digest: null,
    planner_prompt_digest: null,
    planner_plan_digest: null,
    outcome_digest: null,
    retention: "step_ids_and_digests_only; planner_transcript_not_retained",
    authorization: "plan_proposal_only; no_tools_arguments_or_effects_authorized",
  };
}

test("direct connector missions execute all twelve domains through one durable kernel", async () => {
  const fixtureValue = await fixture();
  const missionValue = mission(AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => step(`domain-${index}`, domain, [], `private-${domain}`)));
  const result = await runAutonomousConnectorMission(missionValue, {
    catalogue: fixtureValue.catalogue,
    connector: { runtime: fixtureValue.runtime, approved: true },
    agent: fixtureValue.agent,
    execute: { approveProviderCall: true },
  });

  assert.equal(result.status, "succeeded");
  assert.equal(result.completed_steps, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(fixtureValue.calls(), AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal((await fixtureValue.journal.verifyIntegrity()).entries, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(JSON.stringify(result.checkpoint).includes("private-coding"), false);
  assert.equal(JSON.stringify(result.preflight).includes("connector_probe"), true);
});

test("provider planning sees metadata only and accepted ordering is contract-bound", async () => {
  const fixtureValue = await fixture();
  const missionValue = mission([
    step("finish", "coding", ["seed"], "secret-finish"),
    step("seed", "coding", [], "secret-seed"),
    step("parallel", "data", [], "secret-parallel"),
  ], "provider-ordering-mission");
  let plannerCalls = 0;
  const proposed = {
    ...refinementFor(missionValue),
    priority_step_ids: ["seed", "parallel", "finish"],
    focus_step_ids: ["seed", "parallel", "finish"],
    // Adversarial extras must remain outside the metadata-only JSON projection.
    planner_context: { prompt: "provider-transcript-secret" },
    adaptive_selection: { secret: "adaptive-transcript-secret" },
  };
  fixtureValue.agent.planOrderedStepsWithProvider = async (request) => {
    plannerCalls += 1;
    assert.equal(request.task, missionValue.goal);
    assert.deepEqual(request.steps, connectorMissionPlannerSteps(missionValue));
    assert.equal(JSON.stringify(request).includes("connector_probe"), false);
    assert.equal(JSON.stringify(request).includes("secret-seed"), false);
    assert.equal(request.protectedContractDigest, connectorMissionProtectedContractDigest(missionValue));
    return proposed;
  };

  const held = await fixtureValue.agent.runConnectorMissionWithProviderPlanning(missionValue, {
    execution: { catalogue: fixtureValue.catalogue, connector: { runtime: fixtureValue.runtime, approved: true }, execute: { approveProviderCall: true } },
    providerPlanning: { approveProviderCall: true },
    acceptPlan: false,
  });
  assert.equal(held.status, "planning_acceptance_required");
  assert.equal(fixtureValue.calls(), 0);
  assert.equal(plannerCalls, 1);
  assert.equal(JSON.stringify(held), JSON.stringify(held.toJSON()));
  assert.equal(JSON.stringify(held).includes("secret-seed"), false);
  assert.equal(JSON.stringify(held).includes("connector_probe"), false);
  assert.equal(JSON.stringify(held).includes("provider-transcript-secret"), false);
  assert.equal(JSON.stringify(held).includes("adaptive-transcript-secret"), false);

  const executed = await fixtureValue.agent.runConnectorMissionWithProviderPlanning(missionValue, {
    execution: { catalogue: fixtureValue.catalogue, connector: { runtime: fixtureValue.runtime, approved: true }, execute: { approveProviderCall: true } },
    acceptedPlanRefinement: proposed,
    acceptPlan: true,
  });
  assert.equal(executed.status, "succeeded");
  assert.equal(plannerCalls, 1, "accepted plan replay must not invoke the planner again");
  assert.equal(fixtureValue.calls(), 3);
  assert.deepEqual(executed.execution.results.map(({ step }) => step.id), ["seed", "parallel", "finish"]);
});

test("connector approval remains independent from provider plan acceptance", async () => {
  const fixtureValue = await fixture();
  const missionValue = mission([step("approval", "operations", [], "approval-secret")], "connector-approval-mission");
  const proposed = refinementFor(missionValue);
  const result = await fixtureValue.agent.runConnectorMissionWithProviderPlanning(missionValue, {
    execution: { catalogue: fixtureValue.catalogue, connector: { runtime: fixtureValue.runtime, approved: false }, execute: { approveProviderCall: true } },
    acceptedPlanRefinement: proposed,
    acceptPlan: true,
  });
  assert.equal(result.status, "approval_required");
  assert.equal(fixtureValue.calls(), 0);
});

test("launch admission is checked before provider planning or connector dispatch", async () => {
  const fixtureValue = await fixture();
  const missionValue = mission([step("held", "coding", [], "held-secret")], "held-launch-mission");
  const brain = new AutonomousBrainFacade({ agent: fixtureValue.agent });
  const heldAdmission = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  let plannerCalls = 0;
  fixtureValue.agent.planOrderedStepsWithProvider = async () => {
    plannerCalls += 1;
    return refinementFor(missionValue);
  };
  await assert.rejects(
    () => fixtureValue.agent.runConnectorMissionWithProviderPlanningAndLaunchAdmission(missionValue, heldAdmission, {
      execution: { catalogue: fixtureValue.catalogue, connector: { runtime: fixtureValue.runtime, approved: true }, execute: { approveProviderCall: true } },
      providerPlanning: { approveProviderCall: true },
      acceptPlan: true,
    }),
    /not approved/,
  );
  assert.equal(plannerCalls, 0);
  assert.equal(fixtureValue.calls(), 0);
});

test("brain facade exposes the complete all-domain connector mission lifecycle", async () => {
  const fixtureValue = await fixture();
  const brain = new AutonomousBrainFacade({ agent: fixtureValue.agent });
  const allDomainMission = mission(
    AUTONOMOUS_DOMAIN_NAMES.map((domain, index) => step(`facade-domain-${index}`, domain, [], `facade-private-${domain}`)),
    "facade-all-domain-mission",
  );
  const direct = await brain.runConnectorMission(allDomainMission, {
    catalogue: fixtureValue.catalogue,
    connector: { runtime: fixtureValue.runtime, approved: true },
    execute: { approveProviderCall: true },
  });
  assert.equal(direct.status, "succeeded");
  assert.equal(direct.completed_steps, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(fixtureValue.calls(), AUTONOMOUS_DOMAIN_NAMES.length);

  const plannedMission = mission([
    step("finish", "coding", ["seed"], "facade-secret-finish"),
    step("seed", "coding", [], "facade-secret-seed"),
    step("parallel", "data", [], "facade-secret-parallel"),
  ], "facade-provider-planned-mission");
  let plannerCalls = 0;
  const proposed = {
    ...refinementFor(plannedMission),
    priority_step_ids: ["seed", "parallel", "finish"],
    focus_step_ids: ["seed", "parallel", "finish"],
    planner_context: { prompt: "facade-provider-transcript-secret" },
  };
  fixtureValue.agent.planOrderedStepsWithProvider = async (request) => {
    plannerCalls += 1;
    assert.equal(request.task, plannedMission.goal);
    assert.deepEqual(request.steps, connectorMissionPlannerSteps(plannedMission));
    assert.equal(JSON.stringify(request).includes("connector_probe"), false);
    assert.equal(JSON.stringify(request).includes("facade-secret-seed"), false);
    return proposed;
  };
  const held = await brain.runConnectorMissionWithProviderPlanning(plannedMission, {
    execution: {
      catalogue: fixtureValue.catalogue,
      connector: { runtime: fixtureValue.runtime, approved: true },
      execute: { approveProviderCall: true },
    },
    providerPlanning: { approveProviderCall: true },
    acceptPlan: false,
  });
  assert.equal(held.status, "planning_acceptance_required");
  assert.equal(plannerCalls, 1);
  assert.equal(fixtureValue.calls(), AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(JSON.stringify(held), JSON.stringify(held.toJSON()));
  assert.doesNotMatch(JSON.stringify(held), /facade-secret-seed|facade-provider-transcript-secret/);

  const replay = await brain.runConnectorMissionWithProviderPlanning(plannedMission, {
    execution: {
      catalogue: fixtureValue.catalogue,
      connector: { runtime: fixtureValue.runtime, approved: true },
      execute: { approveProviderCall: true },
    },
    acceptedPlanRefinement: held.plan_refinement,
    acceptPlan: true,
  });
  assert.equal(replay.status, "succeeded");
  assert.equal(plannerCalls, 1, "facade replay must not invoke the planner again");
  assert.deepEqual(replay.execution.results.map(({ step: missionStep }) => missionStep.id), ["seed", "parallel", "finish"]);
  assert.doesNotMatch(JSON.stringify(replay), /facade-secret-seed|facade-provider-transcript-secret/);

  const heldAdmission = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold" });
  const plannerCallsBeforeHold = plannerCalls;
  await assert.rejects(
    () => brain.runConnectorMissionWithProviderPlanningAndLaunchAdmission(plannedMission, heldAdmission, {
      execution: {
        catalogue: fixtureValue.catalogue,
        connector: { runtime: fixtureValue.runtime, approved: true },
        execute: { approveProviderCall: true },
      },
      providerPlanning: { approveProviderCall: true },
      acceptPlan: true,
    }),
    /not approved/,
  );
  assert.equal(plannerCalls, plannerCallsBeforeHold, "launch admission must precede facade planner invocation");
  assert.equal(fixtureValue.calls(), AUTONOMOUS_DOMAIN_NAMES.length + 3);
  await assert.rejects(
    () => brain.runConnectorMissionWithLaunchAdmission(
      mission([step("semantic", "coding", [], "semantic-secret")], "facade-semantic-launch"),
      heldAdmission,
      {
        catalogue: fixtureValue.catalogue,
        connector: { runtime: fixtureValue.runtime, approved: true },
        execute: { approveProviderCall: true, semanticRouting: { enabled: true } },
      },
    ),
    /provider-free routing/,
  );
});

test("tampered ordered plans cannot change protected mission fields or dependency order", async () => {
  const missionValue = mission([step("first", "coding"), step("second", "data", ["first"])], "tamper-mission");
  const refinement = refinementFor(missionValue);
  const reordered = { ...refinement, priority_step_ids: ["first", "second"], focus_step_ids: ["first", "second"] };
  assert.deepEqual(applyAutonomousOrderedStepPlan(missionValue, reordered).steps.map(({ id }) => id), ["first", "second"]);
  assert.throws(() => applyAutonomousOrderedStepPlan(missionValue, { ...reordered, protected_contract_digest: "a".repeat(64) }), /protected contract digest/);
  assert.throws(() => applyAutonomousOrderedStepPlan(missionValue, { ...reordered, priority_step_ids: ["second", "first"] }), /dependency/);
});
