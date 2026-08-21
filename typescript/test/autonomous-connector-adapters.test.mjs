import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousConnectorObservation,
  AutonomousConnectorRegistration,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousMissionExecutor,
  AutonomousWorkflowExecutor,
  CredentialStore,
  InMemoryAutonomousConnectorReceiptJournal,
  InMemoryAutonomousMissionCheckpointStore,
  InMemoryAutonomousWorkflowCheckpointStore,
  LLMRuntime,
  ToolCatalogue,
  autonomousConnectorMissionStepExecutor,
  autonomousConnectorWorkflowStageExecutor,
  builtinAutonomousDomainProfiles,
} from "../dist/index.js";

function connectorManifest(capabilities) {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "local.domain-evidence",
    version: "1.0.0",
    provider: "local-test-connector",
    connector_kind: "provider_api",
    domains: [...AUTONOMOUS_DOMAIN_NAMES],
    capabilities: [...new Set(capabilities)],
    transport: "caller_managed",
    auth_posture: {
      status: "delegated",
      secret_refs: ["opaque-session-ref"],
      does_not_claim: ["credential validity", "provider availability"],
    },
  };
}

async function connectorFixture() {
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = ["evidence_read"];
  for (const profile of profiles) for (const stage of profile.workflow.stages) capabilities.push(...stage.required_capabilities);
  let calls = 0;
  const registration = new AutonomousConnectorRegistration(
    connectorManifest(capabilities),
    async (_manifest, request) => {
      calls += 1;
      return new AutonomousConnectorObservation({
        source: "local-deterministic-connector",
        stage_id: request.stage_id ?? request.step_id ?? "unknown",
        observation_digest: "a".repeat(64),
      }, "observed");
    },
  );
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const registry = new AutonomousConnectorRegistry([registration]);
  const runtime = new AutonomousConnectorRuntime(registry, { receiptStore: journal });
  return { profiles, calls: () => calls, journal, registry, runtime };
}

test("connector-backed workflow stages execute every autonomous domain through durable checkpoints", async () => {
  const fixture = await connectorFixture();
  const agent = new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
  const adapter = autonomousConnectorWorkflowStageExecutor({ runtime: fixture.runtime, approved: true });
  let totalStages = 0;
  for (const profile of fixture.profiles) {
    const result = await new AutonomousWorkflowExecutor(
      agent,
      new InMemoryAutonomousWorkflowCheckpointStore(),
      { stageExecutor: adapter },
    ).start(`Run a local connector-backed ${profile.domain} workflow`, {
      domain: profile.domain,
      jobId: `connector-workflow-${profile.domain}`,
      approveProviderCall: true,
      maxStages: 32,
    });
    totalStages += profile.workflow.stages.length;
    assert.equal(result.status, "completed", profile.domain);
    assert.equal(result.completed_stage_count, profile.workflow.stages.length, profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.declared_status === "completed" && stage.validation_errors.length === 0), profile.domain);
    assert.ok(result.stage_results.every((stage) => stage.run?.selection?.selected_model?.provider === "local-test-connector"), profile.domain);
    assert.equal(JSON.stringify(result.checkpoint).includes("local connector-backed"), false, "checkpoint retains metadata, not task text");
  }
  assert.equal(fixture.calls(), totalStages);
  assert.equal((await fixture.journal.verifyIntegrity()).entries, totalStages);
});

test("connector-backed mission steps bind capability, approval, and caller-owned replay rehydration", async () => {
  const fixture = await connectorFixture();
  const step = { id: "replay-step", domain: "science", capability: "evidence_read", objective: "read bounded evidence", tool: "connector_probe", arguments: { query_digest: "b".repeat(64) } };
  const context = {
    mission_id: "connector-mission-replay",
    goal: "retrieve a bounded observation",
    wave: 0,
    step,
    arguments: step.arguments,
    dependency_outputs: {},
    execution_attempt: 1,
    resumed: false,
  };
  const firstAdapter = autonomousConnectorMissionStepExecutor({ runtime: fixture.runtime, approved: true });
  const first = await firstAdapter(context);
  assert.equal(first.status, "succeeded");
  assert.equal(first.run_status, "connector_observed");
  assert.equal(first.decision.plan_digest.length, 64);
  assert.equal(fixture.calls(), 1);

  const rehydrated = autonomousConnectorMissionStepExecutor({
    runtime: fixture.runtime,
    approved: true,
    rehydratePayload: () => ({ source: "local-deterministic-connector", stage_id: "replay-step", observation_digest: "a".repeat(64) }),
  });
  const replay = await rehydrated(context);
  assert.equal(replay.status, "succeeded");
  assert.equal(replay.run_status, "connector_observed");
  assert.deepEqual(replay.value, { source: "local-deterministic-connector", stage_id: "replay-step", observation_digest: "a".repeat(64) });
  assert.equal(fixture.calls(), 1, "replay must not invoke the connector again");

  const refused = await autonomousConnectorMissionStepExecutor({ runtime: fixture.runtime, approved: false })({
    ...context,
    execution_attempt: 2,
  });
  assert.equal(refused.status, "approval_required");
  assert.equal(refused.error_class, "ConnectorApprovalRequired");
});

test("connector-backed missions execute all domains and leave raw observations outside checkpoints", async () => {
  const fixture = await connectorFixture();
  const steps = fixture.profiles.map((profile) => ({
    id: `step-${profile.domain}`,
    domain: profile.domain,
    capability: "evidence_read",
    objective: `observe ${profile.domain}`,
    tool: "connector_probe",
    arguments: { subject_digest: "c".repeat(64) },
  }));
  const catalogue = await ToolCatalogue.fromDefinitions([{
    name: "connector_probe",
    description: "bounded connector probe",
    inputSchema: { type: "object", additionalProperties: true },
  }]);
  const adapter = autonomousConnectorMissionStepExecutor({ runtime: fixture.runtime, approved: true });
  const result = await new AutonomousMissionExecutor({
    catalogue,
    checkpointStore: new InMemoryAutonomousMissionCheckpointStore(),
    executeStep: adapter,
  }).start({
    mission_id: "connector-mission-all-domains",
    goal: "execute every bounded domain connector step",
    steps,
    policy: {
      execute: true,
      stop_on_error: true,
      allow_side_effects: false,
      max_steps: 32,
      max_step_output_bytes: 100_000,
      max_total_output_bytes: 2_000_000,
      execution_mode: "parallel_waves",
      max_parallelism: 4,
      allowed_tools: ["connector_probe"],
    },
  }, { approveProviderCall: true });
  assert.equal(result.status, "succeeded");
  assert.equal(result.results.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(result.results.every((row) => row.status === "succeeded"));
  assert.equal(JSON.stringify(result.checkpoint).includes("local-deterministic-connector"), false);
});
