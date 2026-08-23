import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES,
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousDeploymentReadinessAuditor,
  CredentialStore,
  LLMRuntime,
  ProviderSetup,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
  validateAutonomousDeploymentReadinessReport,
} from "../dist/index.js";

const candidate = (provider, capabilities) => ({
  provider,
  model: "deployment-audit-model",
  capabilities,
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 100,
  cost_per_million_tokens: 10,
  reliability: 0.95,
});

async function fixture({ credential = false } = {}) {
  const runtime = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async () => {
      throw new Error("deployment readiness must not contact a provider");
    },
  });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("openai", { baseUrl: "https://deployment-readiness.invalid" });
  const session = setup.startSession({ ttlMs: 60_000, sessionId: credential ? "deployment-ready" : "deployment-missing-credential" });
  if (credential) setup.collectUserCredential(session, "openai", "unit-test-only-not-a-provider-key");
  const profiles = await builtinAutonomousDomainProfiles();
  const capabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("openai", capabilities));
  return { setup, session, agent, profiles, providerPlan: setup.plan(["openai"]), readiness: await agent.readiness() };
}

const durableCapabilities = {
  persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
};

test("deployment readiness joins protected provider setup with every built-in domain", async () => {
  const fixtureValue = await fixture({ credential: true });
  const report = new AutonomousDeploymentReadinessAuditor().audit({
    agent: fixtureValue.readiness,
    provider_plan: fixtureValue.providerPlan,
    capabilities: durableCapabilities,
  });

  assert.equal(report.schema, "bioprism-typescript-autonomous-deployment-readiness/0.1");
  assert.deepEqual(report.domains.map((row) => row.domain), AUTONOMOUS_DOMAIN_NAMES);
  assert.equal(report.domains.length, 12);
  assert.equal(report.ready_domain_count, 12);
  assert.equal(report.partial_domain_count, 0);
  assert.equal(report.blocked_domain_count, 0);
  assert.equal(report.state, "ready_for_review");
  assert.equal(report.provider_gate.ready_provider_count, 1);
  assert.equal(report.global_blockers.length, 0);
  assert.equal(report.capabilities.length, AUTONOMOUS_DEPLOYMENT_CAPABILITY_NAMES.length);
  assert.equal(report.capabilities.find((row) => row.name === "persistence").satisfies_requirement, true);
  assert.equal(report.execution, "audit_only;no_provider_source_tool_queue_or_credential_dispatch");
  assert.equal(report.authority, "audit_does_not_grant_dispatch_authority");
  assert.match(report.readiness_digest, /^[0-9a-f]{64}$/);
  assert.deepEqual(validateAutonomousDeploymentReadinessReport(report), report);
  assert.doesNotMatch(JSON.stringify(report), /unit-test-only-not-a-provider-key|authorization|Bearer|api_key/i);
  fixtureValue.session.close();
});

test("deployment readiness refuses an uncredentialed provider without making a network call", async () => {
  const fixtureValue = await fixture();
  assert.equal(fixtureValue.providerPlan.ready, false);
  const report = new AutonomousDeploymentReadinessAuditor().audit({
    agent: fixtureValue.readiness,
    provider_plan: fixtureValue.providerPlan,
    capabilities: durableCapabilities,
  });

  assert.equal(report.state, "blocked");
  assert.ok(report.global_blockers.some((row) => row.code === "credential"));
  assert.ok(report.domains.every((row) => row.state === "blocked"));
  assert.ok(report.domains.every((row) => row.blockers.some((blocker) => blocker.code === "credential")));
  fixtureValue.session.close();
});

test("deployment policy turns optional evidence, queue, tools, and learning into explicit gates", async () => {
  const fixtureValue = await fixture({ credential: true });
  const report = new AutonomousDeploymentReadinessAuditor({
    requireToolCatalogue: true,
    requireEvidence: true,
    requireLearning: true,
    requireQueue: true,
    requireTelemetry: true,
  }).audit({
    agent: fixtureValue.readiness,
    provider_plan: fixtureValue.providerPlan,
    capabilities: durableCapabilities,
  });

  assert.equal(report.state, "blocked");
  assert.ok(report.global_blockers.some((row) => row.code === "queue"));
  assert.ok(report.global_blockers.some((row) => row.code === "telemetry"));
  assert.ok(report.domains.every((row) => row.blockers.some((blocker) => blocker.code === "tool_catalogue")));
  assert.ok(report.domains.every((row) => row.blockers.some((blocker) => blocker.code === "evidence_adapter")));
  assert.ok(report.domains.every((row) => row.blockers.some((blocker) => blocker.code === "learning")));
  fixtureValue.session.close();
});

test("deployment readiness is digest-bound and rejects tampering", async () => {
  const fixtureValue = await fixture({ credential: true });
  const report = new AutonomousDeploymentReadinessAuditor().audit({
    agent: fixtureValue.readiness,
    provider_plan: fixtureValue.providerPlan,
    capabilities: durableCapabilities,
  });
  const tampered = structuredClone(report);
  tampered.domains[0].next_actions.push("tampered");
  assert.throws(() => validateAutonomousDeploymentReadinessReport(tampered), /digest/);
  fixtureValue.session.close();
});
