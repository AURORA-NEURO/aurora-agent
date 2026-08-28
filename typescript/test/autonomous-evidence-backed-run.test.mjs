import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousEvidenceAdapterRegistry,
  InMemoryAutonomousEvidenceBackedCheckpointStore,
  AutonomousEvidenceReadinessPolicy,
  CredentialStore,
  InMemoryAutonomousRunTraceStore,
  LLMRuntime,
  builtinAutonomousDomainEvidenceSourceProfiles,
  createBuiltinAutonomousDomainEvidenceSourceCatalogue,
  registerAutonomousEvidenceAdaptersForAllDomains,
  builtinAutonomousDomainProfiles,
  openaiCompatibleProvider,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

async function launchAdmissionFor(agent) {
  const profiles = await builtinAutonomousDomainProfiles();
  const brain = new AutonomousBrainFacade({ agent });
  const availableToolNames = profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name));
  const availableEvidence = profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`)));
  const deploymentCapabilities = {
    persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
  };
  const preflight = await brain.launchPreflight({ availableToolNames, availableEvidence, deploymentCapabilities });
  return brain.admitLaunchPreflight(preflight, { decision: "approve", authorizationDigest: "e".repeat(64) });
}

function model() {
  return {
    provider: "evidence-backed-provider",
    model: "evidence-backed-model",
    capabilities: ["reasoning", "code", "web", "data", "science", "biomedical", "neuroscience", "coordination", "operations", "enterprise", "multimodal", "evaluation", "structured_output"],
    context_window_tokens: 64_000,
    max_output_tokens: 2_000,
    quality: 0.95,
    latency_ms: 50,
    cost_per_million_tokens: 10,
    reliability: 0.99,
  };
}

async function setup() {
  const profiles = await builtinAutonomousDomainProfiles();
  const calls = { evidence: 0, provider: 0 };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls.provider += 1;
      const body = JSON.parse(String(init.body));
      return jsonResponse({
        choices: [{
          message: {
            role: "assistant",
            content: `provider response observed ${JSON.stringify(body.messages).includes("transient-evidence-claim") ? "reviewed evidence" : "metadata contract"}`,
          },
          finish_reason: "stop",
        }],
      });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("evidence-backed-provider", "https://evidence-backed-provider.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAutonomousEvidenceAdaptersForAllDomains(registry, (domain) => {
    const profile = profiles.find((candidate) => candidate.domain === domain);
    return {
      adapterId: `evidence-backed-${domain}`,
      version: "1",
      capabilities: profile.capabilities,
      sourceKinds: ["fixture"],
      acquire: async (context) => {
        calls.evidence += 1;
        return { domain, requirement: context.requirement.requirement_id, claim: "transient-evidence-claim" };
      },
    };
  });
  return { agent, registry, calls };
}

function evidenceOptions(plan, runOptions = {}) {
  return {
    domains: plan.domains,
    requests: plan.requirements.map((requirement, index) => ({ requirement_id: requirement.requirement_id, source_id: `fixture-source-${index}`, request_id: `fixture-request-${index}`, metadata: {} })),
    prepare: { readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }), allowDegradedDispatch: true },
    execute: {
      approveSourceDispatch: true,
      projector: { project: (_value, context) => [{ label: context.requirement.requirement_id, kind: "fact", status: "observed" }] },
      evaluator: {
        evaluator_id: "evidence-backed-evaluator",
        evaluator_version: "1",
        evaluate: ({ requirement }) => ({ evaluator_id: "evidence-backed-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
      },
    },
    run: {
      domain: plan.domains[0],
      candidates: [model()],
      approveProviderCall: true,
      ...runOptions,
    },
  };
}

test("evidence-backed execution composes approved acquisition, transient evidence prompting, and provider invocation across every domain", async () => {
  const { agent, registry, calls } = await setup();
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    expectedEvidenceCalls += plan.requirements.length;
    const result = await agent.runWithReviewedEvidence(`Review a bounded ${domain} task with source evidence.`, {
      registry,
      ...evidenceOptions(plan),
      promptBuilder: ({ values }) => [{
        id: "transient-evidence-value",
        content: JSON.stringify({ transient: Object.values(values)[0]?.claim }),
        required: true,
        priority: 970,
      }],
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.evidence.status, "completed", domain);
    assert.equal(result.run.status, "completed", domain);
    assert.equal(result.toJSON().secret_material, "never_returned");
    assert.doesNotMatch(JSON.stringify(result.toJSON()), /transient-evidence-claim/);
    assert.match(result.run.response.text, /reviewed evidence/);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade exposes reviewed evidence execution across every built-in domain", async () => {
  const { agent, registry, calls } = await setup();
  const brain = new AutonomousBrainFacade({ agent });
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    expectedEvidenceCalls += plan.requirements.length;
    const result = await brain.runWithReviewedEvidence(`Facade evidence review for ${domain}.`, {
      registry,
      ...evidenceOptions(plan),
      promptBuilder: ({ values }) => [{
        id: "facade-transient-evidence",
        content: JSON.stringify({ claim: Object.values(values)[0]?.claim }),
        required: true,
        priority: 970,
      }],
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.evidence?.status, "completed", domain);
    assert.equal(result.run?.status, "completed", domain);
    assert.doesNotMatch(JSON.stringify(result.toJSON()), /transient-evidence-claim/);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade traces reviewed evidence across every built-in domain without retaining values", async () => {
  const { agent, registry, calls } = await setup();
  const brain = new AutonomousBrainFacade({ agent });
  const traceStore = new InMemoryAutonomousRunTraceStore();
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    expectedEvidenceCalls += plan.requirements.length;
    const traced = await brain.runWithReviewedEvidenceWithTrace(`Traced facade evidence review for ${domain}.`, {
      registry,
      ...evidenceOptions(plan),
      traceStore,
      runId: `facade-evidence-trace-${domain}`,
      promptBuilder: ({ values }) => [{
        id: "trace-transient-evidence",
        content: JSON.stringify({ claim: Object.values(values)[0]?.claim }),
        required: true,
        priority: 970,
      }],
    });
    assert.equal(traced.result.status, "completed", domain);
    assert.equal(traced.trace.status, "completed", domain);
    assert.equal(traced.trace.domains.includes(domain), true, domain);
    assert.equal(traced.trace.provider_invocations, 1, domain);
    assert.ok(traced.trace.plan_digest, domain);
    assert.ok(traced.trace.selection_digests.length >= 1, domain);
    assert.doesNotMatch(JSON.stringify(traced), /transient-evidence-claim/);
    const phases = traceStore.events({ run_id: `facade-evidence-trace-${domain}` }).map((event) => event.phase);
    assert.ok(phases.includes("plan_compiled"), domain);
    assert.ok(phases.includes("model_selection_finished"), domain);
    assert.ok(phases.includes("provider_invocation_finished"), domain);
    assert.ok(phases.includes("evaluation_settled"), domain);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(traceStore.verifyIntegrity().verified, true);
});

test("evidence-backed execution keeps source approval and provider approval independent", async () => {
  const { agent, registry, calls } = await setup();
  const plan = await agent.evidencePlan(["coding"]);
  const reviewRequired = await agent.runWithReviewedEvidence("Review a coding task only after evidence approval.", {
    registry,
    ...evidenceOptions(plan),
    execute: { ...evidenceOptions(plan).execute, approveSourceDispatch: false },
  });
  assert.equal(reviewRequired.status, "evidence_review_required");
  assert.equal(reviewRequired.evidence, null);
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);

  const providerReviewRequired = await agent.runWithReviewedEvidence("Review a coding task after source approval.", {
    registry,
    ...evidenceOptions(plan),
    run: { ...evidenceOptions(plan).run, approveProviderCall: false },
  });
  assert.equal(providerReviewRequired.status, "approval_required");
  assert.equal(providerReviewRequired.evidence.status, "completed");
  assert.equal(providerReviewRequired.run.status, "approval_required");
  assert.equal(calls.evidence, plan.requirements.length);
  assert.equal(calls.provider, 0);
});

test("evidence-backed execution defaults to metadata-only prompting and blocks unsettled evidence", async () => {
  const { agent, registry, calls } = await setup();
  const plan = await agent.evidencePlan(["science"]);
  const metadataOnly = await agent.runWithReviewedEvidence("Review a science task with metadata-only evidence context.", {
    registry,
    ...evidenceOptions(plan),
  });
  assert.equal(metadataOnly.status, "completed");
  assert.equal(metadataOnly.evidence.status, "completed");
  assert.equal(metadataOnly.run.status, "completed");
  assert.doesNotMatch(metadataOnly.prompt_context[0].content, /transient-evidence-claim/);
  assert.match(metadataOnly.run.response.text, /metadata contract/);
  assert.doesNotMatch(JSON.stringify(metadataOnly.toJSON()), /transient-evidence-claim/);

  const unsettled = await agent.runWithReviewedEvidence("Review a science task only after evidence settles.", {
    registry,
    ...evidenceOptions(plan),
    execute: {
      ...evidenceOptions(plan).execute,
      evaluator: {
        evaluator_id: "evidence-backed-evaluator",
        evaluator_version: "1",
        evaluate: () => ({ evaluator_id: "evidence-backed-evaluator", evaluator_version: "1", verdict: "indeterminate", score: 0.5 }),
      },
    },
  });
  assert.equal(unsettled.status, "evidence_incomplete");
  assert.equal(unsettled.evidence.status, "awaiting_evaluation");
  assert.equal(unsettled.run, null);
  assert.equal(calls.provider, 1);
});

test("evidence-backed automatic execution preserves reviewed scope across every built-in domain", async () => {
  const { agent, registry, calls } = await setup();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    const result = await agent.runWithReviewedEvidence(`Run an automatic bounded ${domain} task after evidence review.`, {
      registry,
      ...evidenceOptions(plan),
      runMode: "auto",
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.run_mode, "auto", domain);
    assert.equal(result.automatic?.status, "completed", domain);
    assert.deepEqual(result.automatic?.route.selected_domains, [domain], domain);
    assert.equal(result.run?.status, "completed", domain);
    assert.equal(result.cross_domain_run, null, domain);
    assert.equal(result.toJSON().automatic_status, "completed", domain);
    assert.equal(result.toJSON().automatic_route_digest, result.automatic?.route.route_digest, domain);
    assert.doesNotMatch(JSON.stringify(result.toJSON()), /transient-evidence-claim/);
  }
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("evidence-backed cross-domain execution fans out only to the reviewed scope", async () => {
  const { agent, registry, calls } = await setup();
  const plan = await agent.evidencePlan(["coding", "data"]);
  const result = await agent.runWithReviewedEvidence("Synthesize a bounded coding and data task after evidence review.", {
    registry,
    ...evidenceOptions(plan),
    runMode: "cross_domain",
    crossDomain: {
      subtasks: [
        { id: "coding-review", domain: "coding", task: "Review the coding implementation implications." },
        { id: "data-review", domain: "data", task: "Review the data validation implications." },
      ],
      maxParallelChildren: 2,
    },
  });
  assert.equal(result.status, "completed");
  assert.equal(result.run_mode, "cross_domain");
  assert.equal(result.run, null);
  assert.equal(result.cross_domain_run?.status, "completed");
  assert.deepEqual(result.cross_domain_run?.blueprint?.child_blueprints.map((child) => child.domain_profile.domain), ["coding", "data"]);
  assert.equal(result.cross_domain_run?.child_runs.length, 2);
  assert.equal(result.cross_domain_run?.synthesis?.status, "completed");
  assert.equal(result.toJSON().cross_domain_run_status, "completed");
  assert.equal(result.toJSON().run_status, null);
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /transient-evidence-claim/);
  assert.equal(calls.provider, 3);

  await assert.rejects(
    agent.runWithReviewedEvidence("Reject semantic rerouting outside the evidence scope.", {
      registry,
      ...evidenceOptions(plan),
      runMode: "auto",
      run: { ...evidenceOptions(plan).run, semanticRouting: true },
    }),
    /cannot combine an exact evidence scope with provider-assisted semantic routing/,
  );
});

test("launch admission gates evidence acquisition before source dispatch across every domain", async () => {
  const { agent, registry, calls } = await setup();
  const admission = await launchAdmissionFor(agent);
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    expectedEvidenceCalls += plan.requirements.length;
    const result = await agent.runWithReviewedEvidenceWithLaunchAdmission(
      `Launch-admitted ${domain} evidence review.`,
      admission,
      { registry, ...evidenceOptions(plan, { approveProviderCall: false }) },
    );
    assert.equal(result.status, "approval_required", domain);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, 0);

  const codingBrain = new AutonomousBrainFacade({ agent });
  const profiles = await builtinAutonomousDomainProfiles();
  const preflight = await codingBrain.launchPreflight({
    availableToolNames: profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name)),
    availableEvidence: profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`))),
    deploymentCapabilities: {
      persistence: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      queue: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      approval_authority: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      external_auth: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
      telemetry: { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true },
    },
  });
  const subset = codingBrain.admitLaunchPreflight(preflight, { decision: "approve", approvedDomains: ["coding"], authorizationDigest: "d".repeat(64) });
  const sourceCalls = calls.evidence;
  await assert.rejects(
    agent.runWithReviewedEvidenceWithLaunchAdmission("Reject an unapproved biomedical source scope.", subset, {
      registry,
      ...evidenceOptions(await agent.evidencePlan(["biomedical"]), { approveProviderCall: false }),
    }),
    /does not approve requested domains/,
  );
  assert.equal(calls.evidence, sourceCalls);

  const catalogueFixture = await catalogueSetup();
  const catalogueAdmission = await launchAdmissionFor(catalogueFixture.agent);
  const codingProfile = builtinAutonomousDomainEvidenceSourceProfiles().find((profile) => profile.domain === "coding");
  const catalogueResult = await catalogueFixture.agent.runWithDomainEvidenceCatalogueWithLaunchAdmission(
    "Launch-admitted catalogue coding review.",
    catalogueAdmission,
    {
      catalogue: catalogueFixture.catalogue,
      domains: ["coding"],
      prepare: { profileId: codingProfile.profile_id, quorum: 1 },
      execute: { approveSourceDispatch: true },
      run: { domain: "coding", approveProviderCall: false },
    },
  );
  assert.equal(catalogueResult.status, "approval_required");
  assert.equal(catalogueFixture.calls.provider, 0);
});

test("brain facade launch admission gates evidence before source work and preserves resumable checkpoints", async () => {
  const { agent, registry, calls } = await setup();
  const brain = new AutonomousBrainFacade({ agent });
  const admission = await launchAdmissionFor(agent);
  const plan = await agent.evidencePlan(["coding"]);
  const held = await brain.runWithReviewedEvidenceWithLaunchAdmission(
    "Launch-admitted facade evidence review.",
    admission,
    { registry, ...evidenceOptions(plan, { approveProviderCall: false }) },
  );
  assert.equal(held.status, "approval_required");
  assert.equal(held.evidence?.status, "completed");
  assert.equal(held.run?.status, "approval_required");
  assert.equal(calls.provider, 0);

  const sourceCallsBeforeRefusal = calls.evidence;
  await assert.rejects(
    brain.runWithReviewedEvidenceWithLaunchAdmission(
      "Reject provider-assisted routing after launch admission.",
      admission,
      {
        registry,
        ...evidenceOptions(plan),
        run: { ...evidenceOptions(plan).run, semanticRouting: true },
      },
    ),
    /provider-free routing/,
  );
  assert.equal(calls.evidence, sourceCallsBeforeRefusal);

  const checkpoints = [];
  const resumable = await brain.runWithReviewedEvidenceResumable(
    "Restart-safe facade evidence review.",
    {
      registry,
      ...evidenceOptions(plan),
      jobId: "facade-evidence-resume",
      checkpointSink: (checkpoint) => checkpoints.push(checkpoint),
    },
  );
  assert.equal(resumable.status, "completed");
  assert.ok(checkpoints.length >= 2);
  assert.equal(checkpoints.at(-1).status, "completed");
  assert.doesNotMatch(JSON.stringify(resumable.toJSON()), /transient-evidence-claim/);

  const controller = brain.createEvidenceBackedController(
    "facade-evidence-controller",
    new InMemoryAutonomousEvidenceBackedCheckpointStore(),
  );
  assert.equal(controller.projection().status, "empty");
  const controlled = await controller.run("Controller-owned facade evidence review.", { registry, ...evidenceOptions(plan) });
  assert.equal(controlled.run.status, "completed");
  assert.equal(controlled.controller.status, "completed");
  assert.equal(controlled.controller.secret_material, "never_returned");

  const heldResumable = await assert.rejects(
    brain.runWithReviewedEvidenceResumableWithLaunchAdmission(
      "Reject resumable provider-assisted routing before dispatch.",
      admission,
      {
        registry,
        ...evidenceOptions(plan),
        jobId: "facade-evidence-held-resume",
        checkpointSink: () => undefined,
        run: { ...evidenceOptions(plan).run, semanticRouting: true },
      },
    ),
    /provider-free routing/,
  );
  assert.equal(heldResumable, undefined);
});

test("brain facade traced launch admission preserves evidence review and blocks provider dispatch", async () => {
  const { agent, registry, calls } = await setup();
  const brain = new AutonomousBrainFacade({ agent });
  const admission = await launchAdmissionFor(agent);
  const plan = await agent.evidencePlan(["coding"]);
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.runWithReviewedEvidenceWithLaunchAdmissionAndTrace(
    "Launch-admitted traced evidence review.",
    admission,
    {
      registry,
      ...evidenceOptions(plan, { approveProviderCall: false }),
      traceStore,
      runId: "facade-evidence-launch-trace",
    },
  );
  assert.equal(traced.result.status, "approval_required");
  assert.equal(traced.trace.status, "paused");
  assert.equal(traced.trace.provider_invocations, 0);
  assert.equal(calls.provider, 0);
  assert.doesNotMatch(JSON.stringify(traced), /transient-evidence-claim/);
  assert.ok(traceStore.events({ run_id: "facade-evidence-launch-trace" }).some((event) => event.phase === "plan_compiled"));
  assert.equal(traceStore.verifyIntegrity().verified, true);
});

async function catalogueSetup() {
  const profiles = await builtinAutonomousDomainProfiles();
  const sourceProfiles = builtinAutonomousDomainEvidenceSourceProfiles();
  const calls = { evidence: 0, provider: 0 };
  const llm = new LLMRuntime({
    credentials: new CredentialStore(),
    fetch: async (_url, init) => {
      calls.provider += 1;
      const body = JSON.parse(String(init.body));
      return jsonResponse({
        choices: [{
          message: {
            role: "assistant",
            content: JSON.stringify(body.messages).includes("catalogue-transient-evidence") ? "catalogue evidence reached provider" : "catalogue metadata reached provider",
          },
          finish_reason: "stop",
        }],
      });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("catalogue-provider", "https://catalogue-provider.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel({ ...model(), provider: "catalogue-provider", model: "catalogue-model" });
  const catalogue = createBuiltinAutonomousDomainEvidenceSourceCatalogue();
  for (const profile of sourceProfiles) {
    catalogue.registerRoute({
      sourceId: `catalogue-${profile.domain}`,
      profileId: profile.profile_id,
      provider: `fixture-${profile.domain}`,
      sourceDigest: "a".repeat(64),
      requestId: `request-${profile.domain}`,
      metadata: { operation: profile.operations[0] },
      acquirer: {
        acquire: async () => {
          calls.evidence += 1;
          return { claim: `catalogue-transient-evidence-${profile.domain}`, domain: profile.domain };
        },
      },
    });
  }
  return { agent, catalogue, calls };
}

test("brain facade exposes catalogue evidence execution across every built-in domain", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  const brain = new AutonomousBrainFacade({ agent });
  const sourceProfiles = builtinAutonomousDomainEvidenceSourceProfiles();
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const profile = sourceProfiles.find((candidate) => candidate.domain === domain);
    expectedEvidenceCalls += (await agent.evidencePlan([domain])).requirements.length;
    const result = await brain.runWithDomainEvidenceCatalogue(`Facade catalogue review for ${domain}.`, {
      catalogue,
      domains: [domain],
      prepare: { profileId: profile.profile_id, quorum: 1 },
      execute: { approveSourceDispatch: true },
      run: {
        domain,
        candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }],
        approveProviderCall: true,
      },
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.prepared.every((item) => item.result?.toJSON().status === "consensus"), true, domain);
    assert.equal(result.run?.status, "completed", domain);
    assert.doesNotMatch(JSON.stringify(result.toJSON()), /catalogue-transient-evidence/);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("brain facade traces catalogue evidence with a metadata-only reconciliation lifecycle", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  const brain = new AutonomousBrainFacade({ agent });
  const profile = builtinAutonomousDomainEvidenceSourceProfiles().find((candidate) => candidate.domain === "science");
  const traceStore = new InMemoryAutonomousRunTraceStore();
  const traced = await brain.runWithDomainEvidenceCatalogueWithTrace("Traced facade catalogue science review.", {
    catalogue,
    domains: ["science"],
    prepare: { profileId: profile.profile_id, quorum: 1 },
    execute: { approveSourceDispatch: true },
    traceStore,
    runId: "facade-catalogue-trace-science",
    run: {
      domain: "science",
      candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }],
      approveProviderCall: true,
    },
  });
  assert.equal(traced.result.status, "completed");
  assert.equal(traced.trace.status, "completed");
  assert.equal(traced.trace.domains.includes("science"), true);
  assert.equal(traced.trace.provider_invocations, 1);
  assert.doesNotMatch(JSON.stringify(traced), /catalogue-transient-evidence/);
  assert.ok(traceStore.events({ run_id: "facade-catalogue-trace-science" }).some((event) => event.phase === "evaluation_settled"));
  assert.equal(traceStore.verifyIntegrity().verified, true);
  assert.equal(calls.provider, 1);
});

test("catalogue-backed brain composes normalizers, reconciliation, model selection, and prompting for every domain", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const profile = builtinAutonomousDomainEvidenceSourceProfiles().find((candidate) => candidate.domain === domain);
    expectedEvidenceCalls += (await agent.evidencePlan([domain])).requirements.length;
    const result = await agent.runWithDomainEvidenceCatalogue(`Review a bounded ${domain} task with catalogue evidence.`, {
      catalogue,
      domains: [domain],
      prepare: { profileId: profile.profile_id, quorum: 1 },
      execute: { approveSourceDispatch: true },
      promptBuilder: ({ values }) => [{
        id: "catalogue-transient-value",
        content: JSON.stringify({ transient: Object.values(values)[0][`catalogue-${domain}`].claim }),
        required: true,
        priority: 970,
      }],
      run: {
        domain,
        candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }],
        approveProviderCall: true,
      },
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.prepared.every((item) => item.result?.toJSON().status === "consensus"), true, domain);
    assert.equal(result.run?.status, "completed", domain);
    assert.match(result.run?.response?.text ?? "", /catalogue evidence reached provider/);
    assert.doesNotMatch(JSON.stringify(result.toJSON()), /catalogue-transient-evidence/);
  }
  assert.equal(calls.evidence, expectedEvidenceCalls);
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length);
});

test("catalogue-backed brain supports evidence-scoped automatic and cross-domain execution", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const profile = builtinAutonomousDomainEvidenceSourceProfiles().find((candidate) => candidate.domain === domain);
    const result = await agent.runWithDomainEvidenceCatalogue(`Automatically review a bounded ${domain} catalogue task.`, {
      catalogue,
      domains: [domain],
      runMode: "auto",
      prepare: { profileId: profile.profile_id, quorum: 1 },
      execute: { approveSourceDispatch: true },
      run: {
        domain,
        candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }],
        approveProviderCall: true,
      },
    });
    assert.equal(result.status, "completed", domain);
    assert.equal(result.run_mode, "auto", domain);
    assert.deepEqual(result.automatic?.route.selected_domains, [domain], domain);
    assert.equal(result.automatic?.status, "completed", domain);
    assert.equal(result.run?.status, "completed", domain);
    assert.equal(result.cross_domain_run, null, domain);
  }

  const cross = await agent.runWithDomainEvidenceCatalogue("Automatically synthesize a bounded coding and data catalogue task.", {
    catalogue,
    domains: ["coding", "data"],
    runMode: "cross_domain",
    prepare: { profileId: "builtin.coding.evidence", quorum: 1 },
    prepareForRequirement: (requirement) => ({ profileId: `builtin.${requirement.domain}.evidence`, quorum: 1 }),
    execute: { approveSourceDispatch: true },
    crossDomain: {
      subtasks: [
        { id: "coding-catalogue-review", domain: "coding", task: "Review the implementation evidence." },
        { id: "data-catalogue-review", domain: "data", task: "Review the data evidence." },
      ],
      maxParallelChildren: 2,
    },
    run: {
      domain: "coding",
      candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }],
      approveProviderCall: true,
    },
  });
  assert.equal(cross.status, "completed");
  assert.equal(cross.run_mode, "cross_domain");
  assert.equal(cross.run, null);
  assert.equal(cross.cross_domain_run?.status, "completed");
  assert.deepEqual(cross.cross_domain_run?.blueprint?.child_blueprints.map((child) => child.domain_profile.domain), ["coding", "data"]);
  assert.equal(cross.cross_domain_run?.child_runs.length, 2);
  assert.equal(cross.cross_domain_run?.synthesis?.status, "completed");
  assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.length + 3);
});

test("catalogue-backed brain keeps source approval, provider approval, and evidence settlement independent", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  const codingPlan = await agent.evidencePlan(["coding"]);
  const reviewRequired = await agent.runWithDomainEvidenceCatalogue("Review a coding catalogue task.", {
    catalogue,
    domains: ["coding"],
    prepare: { profileId: "builtin.coding.evidence", quorum: 1 },
    execute: { approveSourceDispatch: false },
    run: { domain: "coding", approveProviderCall: true },
  });
  assert.equal(reviewRequired.status, "evidence_review_required");
  assert.equal(reviewRequired.prepared[0].result, null);
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);

  const providerReviewRequired = await agent.runWithDomainEvidenceCatalogue("Review a coding catalogue task after source approval.", {
    catalogue,
    domains: ["coding"],
    prepare: { profileId: "builtin.coding.evidence", quorum: 1 },
    execute: { approveSourceDispatch: true },
    run: { domain: "coding", approveProviderCall: false },
  });
  assert.equal(providerReviewRequired.status, "approval_required");
  assert.equal(providerReviewRequired.prepared[0].result?.toJSON().status, "consensus");
  assert.equal(providerReviewRequired.run?.status, "approval_required");
  assert.equal(calls.evidence, codingPlan.requirements.length);
  assert.equal(calls.provider, 0);
});

test("catalogue-backed brain blocks provider invocation on unresolved source disagreement", async () => {
  const { agent, catalogue, calls } = await catalogueSetup();
  const coding = catalogue.profile("builtin.coding.evidence");
  catalogue.registerRoute({
    sourceId: "catalogue-coding-dissent",
    profileId: coding.profile_id,
    provider: "fixture-coding-dissent",
    sourceDigest: "b".repeat(64),
    requestId: "request-coding-dissent",
    metadata: { operation: coding.operations[0] },
    acquirer: { acquire: async () => ({ claim: "catalogue-conflicting-evidence", domain: "coding" }) },
  });
  const result = await agent.runWithDomainEvidenceCatalogue("Review a coding task with conflicting catalogue evidence.", {
    catalogue,
    domains: ["coding"],
    prepare: {
      profileId: coding.profile_id,
      sourceIds: ["catalogue-coding", "catalogue-coding-dissent"],
      quorum: 2,
      maxConcurrency: 2,
    },
    execute: { approveSourceDispatch: true },
    run: { domain: "coding", candidates: [{ ...model(), provider: "catalogue-provider", model: "catalogue-model" }], approveProviderCall: true },
  });
  assert.equal(result.status, "evidence_incomplete");
  assert.equal(result.prepared.every((item) => item.result?.toJSON().status === "disagreement"), true);
  assert.equal(result.run, null);
  assert.equal(calls.provider, 0);
});
