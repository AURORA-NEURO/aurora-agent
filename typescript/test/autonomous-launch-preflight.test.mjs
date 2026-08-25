import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  CredentialStore,
  LLMRuntime,
  ProviderSetup,
  auditAutonomousBrainLaunchPreflight,
  builtinAutonomousDomainProfiles,
  validateAutonomousLaunchPreflightReport,
} from "../dist/index.js";

const completeCapabilities = {
  persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
};

const candidate = (capabilities) => ({
  provider: "openai",
  model: "launch-preflight-model",
  capabilities,
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
});

async function fixture() {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => { throw new Error("launch preflight must not contact a provider"); },
  });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("openai", { baseUrl: "https://launch-preflight.invalid" });
  const session = setup.startSession({ ttlMs: 60_000, sessionId: "launch-preflight-test" });
  setup.collectUserCredential(session, "openai", "unit-test-only-not-a-provider-key");
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate(capabilities));
  return { brain: new AutonomousBrainFacade({ agent }), profiles, session };
}

test("launch preflight composes all twelve domains without dispatch", async () => {
  let calls = 0;
  const runtime = new LLMRuntime({ fetch: async () => { calls += 1; throw new Error("must not dispatch"); } });
  const brain = new AutonomousBrainFacade({ agent: new AutonomousAgent(runtime) });
  const report = await brain.launchPreflight();

  assert.equal(report.summary.state, "blocked");
  assert.equal(report.summary.domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.domains.every((row) => row.state === "blocked"), true);
  assert.deepEqual(validateAutonomousLaunchPreflightReport(report), report);
  assert.equal(report.dispatch.provider_calls, 0);
  assert.equal(report.dispatch.source_calls, 0);
  assert.equal(report.dispatch.tool_calls, 0);
  assert.equal(report.dispatch.learner_mutations, 0);
  assert.equal(report.dispatch.credential_resolution, 0);
  assert.equal(calls, 0);
});

test("launch preflight admits complete reviewed gates while retaining caller-owned runtime gaps", async () => {
  const fixtureValue = await fixture();
  const report = await fixtureValue.brain.launchPreflight({ deploymentCapabilities: completeCapabilities });

  assert.equal(report.deployment_readiness.state, "ready_for_review");
  assert.equal(report.summary.state, "partial");
  assert.equal(report.domains.every((row) => row.state === "partial"), true);
  assert.equal(report.domains.every((row) => row.deployment_state === "ready_for_review"), true);
  assert.equal(report.domains.every((row) => row.contract_runtime_status === "unassessed"), true);
  assert.doesNotMatch(JSON.stringify(report), /unit-test-only-not-a-provider-key|api_key|Bearer/i);
  assert.deepEqual(validateAutonomousLaunchPreflightReport(report), report);
  fixtureValue.session.close();
});

test("launch preflight reaches review-ready for every domain with complete caller inventories", async () => {
  const fixtureValue = await fixture();
  const tools = fixtureValue.profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const evidence = fixtureValue.profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const report = await fixtureValue.brain.launchPreflight({
    availableToolNames: tools,
    availableEvidence: evidence,
    deploymentCapabilities: completeCapabilities,
  });

  assert.equal(report.summary.state, "ready_for_review");
  assert.equal(report.summary.ready_domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(report.summary.partial_domain_count, 0);
  assert.equal(report.summary.blocked_domain_count, 0);
  assert.equal(report.domains.every((row) => row.state === "ready_for_review"), true);
  assert.equal(report.contract_audit.runtime_status, "ready_for_review");
  assert.deepEqual(validateAutonomousLaunchPreflightReport(report), report);
  fixtureValue.session.close();
});

test("launch preflight rejects tampering and secret-shaped capability metadata", async () => {
  const fixtureValue = await fixture();
  const report = await auditAutonomousBrainLaunchPreflight(fixtureValue.brain, { deploymentCapabilities: completeCapabilities });
  const tampered = structuredClone(report);
  tampered.domains[0].next_actions.push("tampered");
  assert.throws(() => validateAutonomousLaunchPreflightReport(tampered), /report_digest/);

  await assert.rejects(
    () => fixtureValue.brain.launchPreflight({ deploymentCapabilities: { persistence: { api_key: "secret" } } }),
    /secret-shaped/,
  );
  fixtureValue.session.close();
});
