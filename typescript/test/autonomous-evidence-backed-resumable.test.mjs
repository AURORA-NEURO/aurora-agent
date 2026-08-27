import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceReadinessPolicy,
  InMemoryAutonomousEvidenceRuntimeJournal,
  AutonomousEvidenceBackedController,
  CredentialStore,
  InMemoryAutonomousEvidenceExecutionCheckpointStore,
  InMemoryAutonomousEvidenceBackedCheckpointStore,
  JsonAutonomousEvidenceBackedCheckpointStore,
  LLMRuntime,
  openaiCompatibleProvider,
  registerAutonomousEvidenceAdaptersForAllDomains,
  builtinAutonomousDomainProfiles,
  TransactionalJsonAutonomousEvidenceBackedCheckpointStore,
  validateAutonomousEvidenceBackedCheckpoint,
} from "../dist/index.js";

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function model() {
  return {
    provider: "resumable-provider",
    model: "resumable-model",
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
            content: `provider response ${JSON.stringify(body.messages).includes("resumable-transient-claim") ? "with transient evidence" : "with reviewed metadata"}`,
          },
          finish_reason: "stop",
        }],
      });
    },
  });
  llm.registerProvider(openaiCompatibleProvider("resumable-provider", "https://resumable-provider.test", { requiresCredential: false }));
  const agent = new AutonomousAgent(llm);
  agent.registerModel(model());
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAutonomousEvidenceAdaptersForAllDomains(registry, (domain) => {
    const profile = profiles.find((candidate) => candidate.domain === domain);
    return {
      adapterId: `resumable-${domain}`,
      version: "1",
      capabilities: profile.capabilities,
      sourceKinds: ["fixture"],
      acquire: async (context) => {
        calls.evidence += 1;
        return { domain, requirement: context.requirement.requirement_id, claim: "resumable-transient-claim" };
      },
    };
  });
  return { agent, registry, calls };
}

async function optionsFor(agent, registry, domain, journal, overrides = {}) {
  const plan = await agent.evidencePlan([domain]);
  const requests = plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `resumable-source-${index}`,
    request_id: `resumable-request-${index}`,
    metadata: {},
  }));
  return {
    registry,
    domains: [domain],
    requests,
    prepare: { readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }), allowDegradedDispatch: true },
    execute: {
      approveSourceDispatch: true,
      journal,
      projector: { project: (_value, context) => [{ label: context.requirement.requirement_id, kind: "fact", status: "observed" }] },
      evaluator: {
        evaluator_id: "resumable-evaluator",
        evaluator_version: "1",
        evaluate: ({ requirement }) => ({ evaluator_id: "resumable-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
      },
      ...overrides.execute,
    },
    run: { domain, candidates: [model()], approveProviderCall: true, ...overrides.run },
    runMode: overrides.runMode,
    crossDomain: overrides.crossDomain,
    promptBuilder: ({ values }) => [{
      id: "resumable-transient-value",
      content: JSON.stringify({ claim: Object.values(values)[0]?.claim }),
      required: true,
      priority: 970,
    }],
    rehydrateProviderRun: overrides.rehydrateProviderRun,
    rehydrateAutomaticRun: overrides.rehydrateAutomaticRun,
    rehydrateCrossDomainRun: overrides.rehydrateCrossDomainRun,
    automaticRunOverride: overrides.automaticRunOverride,
    crossDomainRunOverride: overrides.crossDomainRunOverride,
    resumeProvider: overrides.resumeProvider,
    evidenceCheckpointStore: overrides.evidenceCheckpointStore,
    evidenceJobId: overrides.evidenceJobId,
    evidenceResumeAfterReconciliation: overrides.evidenceResumeAfterReconciliation,
  };
}

test("resumable evidence-backed runs rehydrate completed evidence and provider results across every domain without replay", async () => {
  const { agent, registry, calls } = await setup();
  let expectedEvidenceCalls = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
    const firstController = new AutonomousEvidenceBackedController(agent, `resumable-${domain}`, checkpointStore);
    const firstOptions = await optionsFor(agent, registry, domain, journal);
    expectedEvidenceCalls += firstOptions.requests.length;
    const first = await firstController.run(`Resume a bounded ${domain} task after a restart.`, firstOptions);
    assert.equal(first.run.status, "completed", domain);
    assert.equal(first.run.result.status, "completed", domain);
    assert.equal(first.run.provider_rehydrated, false, domain);
    const rawValues = first.run.result.evidence.runtime.values;
    const rawProviderRun = first.run.result.run;
    assert.ok(rawProviderRun);

    const secondController = new AutonomousEvidenceBackedController(agent, `resumable-${domain}`, checkpointStore);
    const secondOptions = await optionsFor(agent, registry, domain, journal, {
      execute: { rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
      rehydrateProviderRun: () => rawProviderRun,
    });
    const second = await secondController.run(`Resume a bounded ${domain} task after a restart.`, secondOptions);
    assert.equal(second.run.status, "completed", domain);
    assert.equal(second.run.provider_rehydrated, true, domain);
    assert.equal(second.run.result.status, "completed", domain);
    assert.equal(calls.evidence, expectedEvidenceCalls, domain);
    assert.equal(calls.provider, AUTONOMOUS_DOMAIN_NAMES.indexOf(domain) + 1, domain);
    assert.doesNotMatch(JSON.stringify(second.run.toJSON()), /resumable-transient-claim/);
    assert.doesNotMatch(JSON.stringify(second.run.toJSON()), /provider response/);
  }
});

test("resumable automatic evidence runs rehydrate the complete envelope without replaying planning or provider work", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const firstController = new AutonomousEvidenceBackedController(agent, "automatic-rehydration-job", checkpointStore);
  const firstOptions = await optionsFor(agent, registry, "coding", journal, { runMode: "auto" });
  const first = await firstController.run("Rehydrate an automatic coding evidence run.", firstOptions);
  assert.equal(first.run.status, "completed");
  assert.equal(first.run.result.run_mode, "auto");
  assert.ok(first.run.result.automatic);
  const rawValues = first.run.result.evidence.runtime.values;
  const rawAutomaticRun = first.run.result.automatic;
  const sourceCalls = calls.evidence;
  const providerCalls = calls.provider;

  const secondController = new AutonomousEvidenceBackedController(agent, "automatic-rehydration-job", checkpointStore);
  const second = await secondController.run("Rehydrate an automatic coding evidence run.", await optionsFor(agent, registry, "coding", journal, {
    runMode: "auto",
    execute: { rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
    rehydrateAutomaticRun: () => rawAutomaticRun,
  }));
  assert.equal(second.run.status, "completed");
  assert.equal(second.run.provider_rehydrated, true);
  assert.equal(second.run.result.automatic?.status, "completed");
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, providerCalls);
  assert.doesNotMatch(JSON.stringify(second.run.toJSON()), /resumable-transient-claim/);
  assert.doesNotMatch(JSON.stringify(second.run.toJSON()), /provider response/);
});

test("resumable cross-domain evidence runs rehydrate fan-out metadata without replaying specialists or synthesis", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const plan = await agent.evidencePlan(["coding", "data"]);
  const requests = plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `cross-resumable-source-${index}`,
    request_id: `cross-resumable-request-${index}`,
    metadata: {},
  }));
  const common = {
    registry,
    domains: ["coding", "data"],
    requests,
    prepare: { readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }), allowDegradedDispatch: true },
    execute: {
      approveSourceDispatch: true,
      journal,
      projector: { project: (_value, context) => [{ label: context.requirement.requirement_id, kind: "fact", status: "observed" }] },
      evaluator: {
        evaluator_id: "cross-resumable-evaluator",
        evaluator_version: "1",
        evaluate: ({ requirement }) => ({ evaluator_id: "cross-resumable-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
      },
    },
    runMode: "cross_domain",
    crossDomain: {
      subtasks: [
        { id: "cross-coding", domain: "coding", task: "Review coding evidence." },
        { id: "cross-data", domain: "data", task: "Review data evidence." },
      ],
      maxParallelChildren: 2,
    },
    run: { domain: "coding", candidates: [model()], approveProviderCall: true },
  };
  const firstController = new AutonomousEvidenceBackedController(agent, "cross-rehydration-job", checkpointStore);
  const first = await firstController.run("Rehydrate a cross-domain evidence run.", common);
  assert.equal(first.run.status, "completed");
  assert.equal(first.run.result.run_mode, "cross_domain");
  assert.ok(first.run.result.cross_domain_run);
  const rawValues = first.run.result.evidence.runtime.values;
  const rawCrossRun = first.run.result.cross_domain_run;
  const providerCalls = calls.provider;
  const sourceCalls = calls.evidence;

  const secondController = new AutonomousEvidenceBackedController(agent, "cross-rehydration-job", checkpointStore);
  const second = await secondController.run("Rehydrate a cross-domain evidence run.", {
    ...common,
    execute: { ...common.execute, rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
    rehydrateCrossDomainRun: () => rawCrossRun,
  });
  assert.equal(second.run.status, "completed");
  assert.equal(second.run.provider_rehydrated, true);
  assert.equal(second.run.result.cross_domain_run?.status, "completed");
  assert.equal(second.run.result.cross_domain_run?.child_runs.length, 2);
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, providerCalls);
});

test("provider-pending checkpoints require explicit resume approval and never replay source work", async () => {
  const { agent, registry, calls } = await setup();
  const domain = "coding";
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-pending-job", checkpointStore);
  const firstOptions = await optionsFor(agent, registry, domain, journal, { run: { approveProviderCall: false } });
  const first = await controller.run("Pause before a provider call and resume explicitly.", firstOptions);
  assert.equal(first.run.status, "provider_pending");
  assert.equal(first.run.result.status, "approval_required");
  assert.equal(calls.provider, 0);
  const sourceCalls = calls.evidence;

  const values = first.run.result.evidence.runtime.values;
  const pending = await controller.run("Pause before a provider call and resume explicitly.", await optionsFor(agent, registry, domain, journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
  }));
  assert.equal(pending.run.status, "provider_pending");
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, 0);

  const resumed = await controller.run("Pause before a provider call and resume explicitly.", await optionsFor(agent, registry, domain, journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
    resumeProvider: true,
  }));
  assert.equal(resumed.run.status, "completed");
  assert.equal(resumed.run.result.status, "completed");
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, 1);
});

test("provider checkpoint composition reuses the source checkpoint and never redispatches settled evidence", async () => {
  const { agent, registry, calls } = await setup();
  const domain = "coding";
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const sourceCheckpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const providerCheckpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "composed-evidence-brain-job", providerCheckpointStore);

  const first = await controller.run("Pause after reviewed evidence before provider invocation.", await optionsFor(agent, registry, domain, journal, {
    run: { approveProviderCall: false },
    evidenceCheckpointStore: sourceCheckpointStore,
    evidenceJobId: "composed-evidence-source-job",
  }));
  assert.equal(first.run.status, "provider_pending");
  assert.equal(first.run.result.status, "approval_required");
  assert.equal(sourceCheckpointStore.read().status, "completed");
  const sourceCalls = calls.evidence;
  const rawValues = first.run.result.evidence.runtime.values;
  assert.equal(calls.provider, 0);

  const pending = await controller.run("Pause after reviewed evidence before provider invocation.", await optionsFor(agent, registry, domain, journal, {
    execute: { rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
    evidenceCheckpointStore: sourceCheckpointStore,
    evidenceJobId: "composed-evidence-source-job",
  }));
  assert.equal(pending.run.status, "provider_pending");
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, 0);

  const resumed = await controller.run("Pause after reviewed evidence before provider invocation.", await optionsFor(agent, registry, domain, journal, {
    execute: { rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
    evidenceCheckpointStore: sourceCheckpointStore,
    evidenceJobId: "composed-evidence-source-job",
    resumeProvider: true,
  }));
  assert.equal(resumed.run.status, "completed");
  assert.equal(resumed.run.result.status, "completed");
  assert.equal(calls.evidence, sourceCalls);
  assert.equal(calls.provider, 1);
});

test("resumable checkpoint stores are bounded, digest-validated, and CAS-fenced", async () => {
  const { agent, registry } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const memoryStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "checkpoint-contract-job", memoryStore);
  const first = await controller.run("Persist only a reviewed checkpoint.", await optionsFor(agent, registry, "coding", journal, { run: { approveProviderCall: false } }));
  const checkpoint = first.run.checkpoint;
  await assert.rejects(validateAutonomousEvidenceBackedCheckpoint({ ...checkpoint, task_digest: "0".repeat(64) }), /digest/);
  assert.equal(await memoryStore.writeIfUnchanged("wrong-checkpoint-digest", checkpoint), false);

  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
  };
  const jsonStore = new JsonAutonomousEvidenceBackedCheckpointStore(textStore);
  await jsonStore.write(checkpoint);
  assert.deepEqual(await jsonStore.read(), checkpoint);
  const transactional = new TransactionalJsonAutonomousEvidenceBackedCheckpointStore({
    ...textStore,
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).checkpoint_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  });
  assert.equal(await transactional.writeIfUnchanged("stale", checkpoint), false);
  assert.equal(await transactional.writeIfUnchanged(checkpoint.checkpoint_digest, checkpoint), true);
});
