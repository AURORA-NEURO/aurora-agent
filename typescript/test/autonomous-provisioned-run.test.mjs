import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_PROVISIONED_RUN_SCHEMA,
  AutonomousAgent,
  AutonomousBrainFacade,
  CredentialError,
  CredentialStore,
  LLMRuntime,
  ProviderSetup,
  builtinAutonomousDomainProfiles,
} from "../dist/index.js";

async function broadCapabilities() {
  const profiles = await builtinAutonomousDomainProfiles();
  return [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))].sort();
}

function localRuntime(provider, onRequest = () => {}, discoverModels) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider(provider, (request) => {
    onRequest(request);
    return { output_text: `offline:${request.model}` };
  }, discoverModels ? { discoverModels } : {});
  return runtime;
}

function candidate(provider, capabilities, model = "offline-model") {
  return {
    provider,
    model,
    capabilities,
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 10,
    cost_per_million_tokens: 0,
    reliability: 0.99,
    requires_credential: provider !== "offline",
  };
}

test("provisioned TypeScript execution closes sessions and serializes metadata only", async () => {
  const calls = [];
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  const setup = new ProviderSetup(runtime);
  setup.registerProvider("groq", {
    transport: {
      invoke: (request) => {
        calls.push(request.model);
        return { output_text: "credentialed transient answer" };
      },
    },
  });
  await setup.provisioner.registerResolver("groq", "test-only-secret-reference", async () => "test-only-secret");
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("groq", ["reasoning", "code"]));

  const run = await setup.runWithProvisionedCredentials(agent, "review a bounded implementation", {
    domain: "coding",
    credentialProviders: ["groq"],
    approveProviderCall: true,
  });

  assert.equal(run.schema, AUTONOMOUS_PROVISIONED_RUN_SCHEMA);
  assert.equal(run.status, "completed");
  assert.equal(run.result.response.text, "credentialed transient answer");
  assert.deepEqual(calls, ["offline-model"]);
  assert.equal(runtime.credentials.status("groq").active_handles, 0, "request session must be revoked");
  const projected = run.toJSON();
  assert.equal(projected.secret_material, "never_returned");
  assert.equal(projected.result_metadata.serialized, false);
  assert.equal("result" in projected, false);
  assert.doesNotMatch(JSON.stringify(run), /credentialed transient answer|test-only-secret/);
});

test("automatic provisioned execution preserves routing and supports strict inventory refresh", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline", undefined, async () => ({
    data: [{ id: "discovered-model", active: true, context_window: 32_000, max_output_tokens: 2_000, capabilities }],
  }));
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  const run = await setup.runAutoWithProvisionedCredentials(agent, "produce a bounded coding implementation review", {
    credentialProviders: ["offline"],
    refreshInventory: true,
    inventorySpecs: [{
      provider: "offline",
      defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 },
    }],
    approveProviderCall: true,
    allowCrossDomain: false,
  });

  assert.equal(run.status, "completed");
  assert.equal(run.result.route.primary_domain, "coding");
  assert.equal(run.result.response.text, "offline:discovered-model");
  assert.equal(run.inventory.status, "completed");
  assert.equal(run.inventory.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(run.toJSON().inventory.domain_count, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("strict provisioned inventory refuses partial discovery before provider execution", async () => {
  let invoked = 0;
  const runtime = localRuntime("offline", () => { invoked += 1; }, async () => ({
    data: [{ id: "offline-model", context_window: 32_000, max_output_tokens: 2_000, capabilities: ["reasoning", "code"] }],
  }));
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  const brain = new AutonomousBrainFacade({ agent });
  await assert.rejects(
    () => setup.runWithProvisionedCredentials(agent, "this must not execute", {
      domain: "coding",
      credentialProviders: ["offline"],
      refreshInventory: true,
      inventorySpecs: [
        { provider: "offline", defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 } },
        { provider: "missing-provider", defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 } },
      ],
      approveProviderCall: true,
    }),
    (error) => error instanceof CredentialError && /inventory refresh did not complete/.test(error.message),
  );
  await assert.rejects(
    () => setup.runBrainWithProvisionedCredentials(brain, { task: "this must also not execute", domain: "coding" }, {
      credentialProviders: ["offline"],
      refreshInventory: true,
      inventorySpecs: [
        { provider: "offline", defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 } },
        { provider: "missing-provider", defaults: { context_window_tokens: 16_000, max_output_tokens: 1_000, quality: 0.8, latency_ms: 20, cost_per_million_tokens: 0, reliability: 0.95 } },
      ],
      approveProviderCall: true,
    }),
    (error) => error instanceof CredentialError && /inventory refresh did not complete/.test(error.message),
  );
  assert.equal(invoked, 0);
});

test("the request-scoped facade executes every configured domain with one credentialless local arm", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", capabilities));

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const run = await setup.runWithProvisionedCredentials(agent, `produce a bounded ${domain} review`, {
      domain,
      credentialProviders: ["offline"],
      approveProviderCall: true,
    });
    assert.equal(run.status, "completed", domain);
    assert.equal(run.result.response.provider, "offline", domain);
  }
});

test("provisioned brain facade executes direct, closed-loop, and adaptive paths across every domain", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", capabilities));
  const brain = new AutonomousBrainFacade({ agent });

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const run = await setup.runBrainWithProvisionedCredentials(brain, { task: `produce a bounded ${domain} review`, domain }, {
      credentialProviders: ["offline"],
      approveProviderCall: true,
    });
    assert.equal(run.status, "completed", domain);
    assert.equal(run.result.run?.status, "completed", domain);
    assert.equal(run.result.run?.response.provider, "offline", domain);
  }

  const cycle = await setup.runBrainCycleWithProvisionedCredentials(brain, { task: "close a bounded science review", domain: "science" }, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
  });
  assert.ok(["completed", "children_completed"].includes(cycle.status));
  assert.ok(cycle.result.cycle);

  const adaptive = await setup.runBrainAdaptiveCycleWithProvisionedCredentials(brain, { task: "close a bounded evaluation review", domain: "evaluation" }, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    adaptive: {
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "provisioned-facade-evaluator", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
    },
  });
  assert.equal(adaptive.status, "completed");
  assert.equal(adaptive.result.adaptive.attempts.length, 1);
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
});

test("provisioned brain facade rejects nested credential injection before opening a session", async () => {
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", ["reasoning", "code"]));
  const brain = new AutonomousBrainFacade({ agent });

  await assert.rejects(
    () => setup.runBrainWithProvisionedCredentials(brain, { task: "must not dispatch", domain: "coding" }, {
      credentialProviders: ["offline"],
      run: { credentialFor: () => undefined },
    }),
    (error) => error instanceof CredentialError && /owns credentials/.test(error.message),
  );
  await assert.rejects(
    () => setup.runBrainCycleWithProvisionedCredentials(brain, { task: "must not dispatch either", domain: "coding" }, {
      credentialProviders: ["offline"],
      cycle: { providerPlanning: { credential: { provider: "offline" } } },
    }),
    (error) => error instanceof CredentialError && /owns credentials/.test(error.message),
  );
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
  assert.equal(runtime.providerStatus("offline").attempts, 0);
});

test("launch-admitted provisioned execution refuses before credential provisioning across every brain entrypoint", async () => {
  const fixture = await (async () => {
    const runtime = new LLMRuntime({ credentials: new CredentialStore(), fetch: async () => { throw new Error("launch admission must not dispatch"); } });
    const setup = new ProviderSetup(runtime);
    setup.registerProvider("openai", { baseUrl: "https://launch-admission-provisioned.invalid" });
    const session = setup.startSession({ ttlMs: 60_000, sessionId: "launch-admitted-provisioned-preflight" });
    setup.collectUserCredential(session, "openai", "unit-test-only-not-a-provider-key");
    const profiles = await builtinAutonomousDomainProfiles();
    const modelCapabilities = [...new Set(profiles.flatMap((profile) => profile.required_model_capabilities))];
    const agent = new AutonomousAgent(runtime);
    agent.registerModel({ provider: "openai", model: "admission-provisioned-model", capabilities: modelCapabilities, context_window_tokens: 32_000, max_output_tokens: 2_000, quality: 0.9, latency_ms: 100, cost_per_million_tokens: 10, reliability: 0.95 });
    const brain = new AutonomousBrainFacade({ agent });
    const tools = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
    const evidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
    const capabilities = {
      persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    };
    const preflight = await brain.launchPreflight({ availableToolNames: tools, availableEvidence: evidence, deploymentCapabilities: capabilities });
    const admission = brain.admitLaunchPreflight(preflight, { decision: "hold", reason: "operator review is pending" });
    session.close();
    return { runtime, setup: new ProviderSetup(runtime), agent, brain, admission };
  })();

  let resolverCalls = 0;
  await fixture.setup.provisioner.registerResolver("openai", "launch-admitted-provisioned-secret", async () => {
    resolverCalls += 1;
    return "unit-test-only-not-a-provider-key";
  });

  await assert.rejects(
    () => fixture.setup.runWithProvisionedCredentialsWithLaunchAdmission(fixture.agent, "write a small function", fixture.admission, { domain: "coding", credentialProviders: ["openai"], approveProviderCall: false }),
    /not approved/,
  );
  await assert.rejects(
    () => fixture.setup.runAutoWithProvisionedCredentialsWithLaunchAdmission(fixture.agent, "write a small function", fixture.admission, { credentialProviders: ["openai"], approveProviderCall: false }),
    /not approved/,
  );
  await assert.rejects(
    () => fixture.setup.runBrainWithProvisionedCredentialsWithLaunchAdmission(fixture.brain, { task: "write a small function", domain: "coding" }, fixture.admission, { credentialProviders: ["openai"], approveProviderCall: false }),
    /not approved/,
  );
  await assert.rejects(
    () => fixture.setup.runBrainCycleWithProvisionedCredentialsWithLaunchAdmission(fixture.brain, { task: "write a small function", domain: "coding" }, fixture.admission, { credentialProviders: ["openai"], approveProviderCall: false }),
    /not approved/,
  );
  await assert.rejects(
    () => fixture.setup.runBrainAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmission(fixture.brain, { task: "write a small function", domain: "coding" }, fixture.admission, { credentialProviders: ["openai"], approveProviderCall: false, adaptive: { maxReplans: 0, evaluate: () => ({ evaluator_id: "unused", evaluator_version: "1", reward: 0, passed: false, replan_requested: false }) } }),
    /not approved/,
  );

  assert.equal(resolverCalls, 0, "held launch admission must be checked before resolving a credential");
  assert.equal(fixture.runtime.credentials.status("openai").active_handles, 0, "held launch admission must not open a session");
  assert.equal(fixture.runtime.providerStatus("openai").attempts, 0, "held launch admission must not reach a provider");
});

test("launch-admitted automatic provisioning rejects provider-assisted routing before credential resolution", async () => {
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", ["reasoning", "code"]));
  const brain = new AutonomousBrainFacade({ agent });
  const preflight = await brain.launchPreflight();
  const admission = brain.admitLaunchPreflight(preflight, { decision: "hold", reason: "not used" });
  await assert.rejects(
    () => setup.runAutoWithProvisionedCredentialsWithLaunchAdmission(agent, "write a small function", admission, { semanticRouting: true }),
    /requires provider-free routing/,
  );
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
  assert.equal(runtime.providerStatus("offline").attempts, 0);
});
