import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_PROVISIONED_RUN_SCHEMA,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousBrainPlan,
  CredentialError,
  CredentialStore,
  InMemoryAutonomousRunTraceStore,
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

async function approvedLaunchAdmission(brain) {
  const profiles = await builtinAutonomousDomainProfiles();
  const availableToolNames = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const availableEvidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const ready = { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true };
  const preflight = await brain.launchPreflight({
    availableToolNames,
    availableEvidence,
    deploymentCapabilities: {
      persistence: ready,
      queue: ready,
      approval_authority: ready,
      external_auth: ready,
      telemetry: ready,
    },
  });
  return brain.admitLaunchPreflight(preflight, { decision: "approve", authorizationDigest: "b".repeat(64) });
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

test("provisioned brain tracing covers every domain and keeps launch admission before session opening", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", capabilities));
  const brain = new AutonomousBrainFacade({ agent });
  const store = new InMemoryAutonomousRunTraceStore();

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const run = await brain.executeWithProvisionedCredentialsWithTrace({ task: `trace a bounded ${domain} review`, domain }, {
      credentialProviders: ["offline"],
      approveProviderCall: true,
      traceStore: store,
      runId: `provisioned-trace-${domain}`,
    });
    assert.equal(run.status, "completed", domain);
    assert.equal(run.result.execution.status, "completed", domain);
    assert.equal(run.result.trace.status, "completed", domain);
    assert.equal(JSON.stringify(run.toJSON()).includes(`trace a bounded ${domain} review`), false, domain);
  }

  const admission = await approvedLaunchAdmission(brain);
  const direct = await brain.executeWithProvisionedCredentialsWithLaunchAdmissionAndTrace({ task: "trace an admitted coding review", domain: "coding" }, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-trace-admitted-direct",
  });
  assert.equal(direct.result.trace.status, "completed");

  const automatic = await brain.executeAutoWithProvisionedCredentialsWithLaunchAdmissionAndTrace({ task: "trace an admitted browser review", domain: "browser" }, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-trace-admitted-automatic",
  });
  assert.equal(automatic.result.trace.status, "completed");

  const cycle = await brain.executeCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace({ task: "trace an admitted science cycle", domain: "science" }, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-trace-admitted-cycle",
  });
  assert.equal(cycle.result.trace.status, "completed");

  const adaptive = await brain.executeAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace({ task: "trace an admitted evaluation cycle", domain: "evaluation" }, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-trace-admitted-adaptive",
    adaptive: {
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "provisioned-trace-evaluator", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
    },
  });
  assert.equal(adaptive.result.trace.status, "completed");
  assert.equal(store.verifyIntegrity().verified, true);
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
});

test("provisioned approved model selection closes sessions across every domain and traces admitted arms", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", capabilities));
  const brain = new AutonomousBrainFacade({ agent });
  const store = new InMemoryAutonomousRunTraceStore();

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const request = { task: `select an exact bounded ${domain} model arm`, domain };
    const preview = await brain.modelSelectionPreview(request);
    const run = await brain.executeApprovedSelectionWithProvisionedCredentials(request, preview, {
      credentialProviders: ["offline"],
    });
    assert.equal(run.status, "completed", domain);
    assert.equal(run.result.status, "completed", domain);
    assert.equal(run.result.run?.selection.selected_model?.provider, "offline", domain);
    assert.equal(run.result.run?.response.provider, "offline", domain);
  }

  const request = { task: "trace an admitted exact coding model arm", domain: "coding" };
  const preview = await brain.modelSelectionPreview(request);
  const admission = await approvedLaunchAdmission(brain);
  const traced = await brain.executeApprovedSelectionWithProvisionedCredentialsWithLaunchAdmissionAndTrace(request, preview, admission, {
    credentialProviders: ["offline"],
    traceStore: store,
    runId: "provisioned-approved-selection-admitted",
  });
  assert.equal(traced.status, "completed");
  assert.equal(traced.result.execution.status, "completed");
  assert.equal(traced.result.trace.status, "completed");
  assert.equal(traced.result.trace.provider_invocations, 1);
  assert.ok(store.events({ run_id: "provisioned-approved-selection-admitted" }).some((event) => event.phase === "model_selection_finished"));
  assert.doesNotMatch(JSON.stringify(traced.toJSON()), /trace an admitted exact coding model arm/);

  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold", reason: "approved model arm pending" });
  const attempts = runtime.providerStatus("offline").attempts;
  await assert.rejects(
    () => brain.executeApprovedSelectionWithProvisionedCredentialsWithLaunchAdmission(request, preview, held, {
      credentialProviders: ["offline"],
    }),
    /not approved/,
  );
  assert.equal(runtime.providerStatus("offline").attempts, attempts);
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
  assert.equal(store.verifyIntegrity().verified, true);
});

test("provisioned persisted-plan replay revalidates route identity before credential provisioning", async () => {
  const capabilities = await broadCapabilities();
  const runtime = localRuntime("offline");
  const setup = new ProviderSetup(runtime);
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(candidate("offline", capabilities));
  const brain = new AutonomousBrainFacade({ agent });
  const admission = await approvedLaunchAdmission(brain);
  const store = new InMemoryAutonomousRunTraceStore();
  const request = { task: "replay an admitted coding plan", domain: "coding" };
  const plan = await brain.plan(request);

  const direct = await brain.executePlannedWithProvisionedCredentialsWithLaunchAdmissionAndTrace(plan, request, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-planned-direct",
  });
  assert.equal(direct.result.execution.status, "completed");
  assert.equal(direct.result.trace.status, "completed");

  const plainDirect = await brain.executePlannedWithProvisionedCredentialsWithLaunchAdmission(plan, request, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
  });
  assert.equal(plainDirect.result.status, "completed");

  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const domainRequest = { task: `replay a bounded ${domain} plan`, domain };
    const domainPlan = await brain.plan(domainRequest);
    const domainRun = await brain.executePlannedWithProvisionedCredentialsWithLaunchAdmission(domainPlan, domainRequest, admission, {
      credentialProviders: ["offline"],
      approveProviderCall: true,
    });
    assert.equal(domainRun.result.status, "completed", domain);
  }

  const cycleRequest = { task: "replay an admitted science cycle", domain: "science" };
  const cyclePlan = await brain.plan(cycleRequest);
  const cycle = await brain.executePlannedCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(cyclePlan, cycleRequest, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-planned-cycle",
  });
  assert.equal(cycle.result.execution.status, "completed");
  assert.equal(cycle.result.trace.status, "completed");

  const adaptiveRequest = { task: "replay an admitted evaluation cycle", domain: "evaluation" };
  const adaptivePlan = await brain.plan(adaptiveRequest);
  const adaptive = await brain.executePlannedAdaptiveCycleWithProvisionedCredentialsWithLaunchAdmissionAndTrace(adaptivePlan, adaptiveRequest, admission, {
    credentialProviders: ["offline"],
    approveProviderCall: true,
    traceStore: store,
    runId: "provisioned-planned-adaptive",
    adaptive: {
      maxReplans: 0,
      evaluate: () => ({ evaluator_id: "provisioned-planned-evaluator", evaluator_version: "1", reward: 0.8, passed: true, replan_requested: false }),
    },
  });
  assert.equal(adaptive.result.execution.status, "completed");
  assert.equal(adaptive.result.trace.status, "completed");

  const tampered = AutonomousBrainPlan.fromJSON(plan.toJSON());
  tampered.route.selected_domains = ["browser"];
  await assert.rejects(
    () => brain.executePlannedWithProvisionedCredentialsWithLaunchAdmissionAndTrace(tampered, request, admission, {
      credentialProviders: ["offline"],
      approveProviderCall: true,
      traceStore: store,
      runId: "provisioned-planned-tampered",
    }),
    /plan digest is invalid|does not match/,
  );
  const held = brain.admitLaunchPreflight(await brain.launchPreflight({
    availableToolNames: (await builtinAutonomousDomainProfiles()).flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name)),
    availableEvidence: (await builtinAutonomousDomainProfiles()).flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`))),
    deploymentCapabilities: {
      persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    },
  }), { decision: "hold", reason: "planned replay review pending" });
  await assert.rejects(
    () => brain.executePlannedWithProvisionedCredentialsWithLaunchAdmissionAndTrace(plan, request, held, {
      credentialProviders: ["offline"],
      approveProviderCall: true,
      traceStore: store,
      runId: "provisioned-planned-held",
    }),
    /not approved/,
  );
  assert.equal(runtime.credentials.status("offline").active_handles, 0);
  assert.equal(store.events({ run_id: "provisioned-planned-held" }).length, 0);
  assert.equal(store.verifyIntegrity().verified, true);
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
