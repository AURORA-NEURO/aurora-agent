import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
  AutonomousAgent,
  AutonomousEffectBoundary,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceReadinessPolicy,
  AutonomousPromptTemplate,
  createAutonomousEvidenceExecutionReconciliationReceipt,
  digestJsonSync,
  InMemoryAutonomousEvidenceRuntimeJournal,
  AutonomousEvidenceBackedController,
  CredentialStore,
  InMemoryAutonomousEvidenceExecutionCheckpointStore,
  InMemoryAutonomousEvidenceBackedCheckpointStore,
  JsonAutonomousEvidenceBackedCheckpointStore,
  LLMRuntime,
  openaiCompatibleProvider,
  registerAutonomousEvidenceAdaptersForAllDomains,
  runAutonomousEvidenceBackedResumable,
  builtinAutonomousDomainProfiles,
  TransactionalJsonAutonomousEvidenceBackedCheckpointStore,
  validateAutonomousEvidenceBackedCheckpoint,
} from "../dist/index.js";

const PROVIDER_POLICY_CONFIG_DIGEST = digestJsonSync({
  models: "fixture-model-v1",
  prompts: "builtin-prompts-v1",
  tools: "fixture-tool-runtime-v1",
  provider: "resumable-provider-v1",
});

class AcknowledgementLostJournal {
  constructor() {
    this.inner = new InMemoryAutonomousEvidenceRuntimeJournal();
    this.failNextAppend = true;
  }

  records() {
    return this.inner.records();
  }

  async append(entry) {
    const persisted = await this.inner.append(entry);
    if (this.failNextAppend) {
      this.failNextAppend = false;
      throw new Error("evidence-backed source acknowledgement lost after durable append");
    }
    return persisted;
  }
}

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

async function setup({ onProviderRequest, onEvidenceAcquire, providerResponse, providerOptions, RuntimeClass = LLMRuntime, effectBoundary, credentialStore } = {}) {
  const profiles = await builtinAutonomousDomainProfiles();
  const calls = { evidence: 0, provider: 0, providerIdempotencyKeys: [], providerUrls: [] };
  const llm = new RuntimeClass({
    credentials: credentialStore ?? new CredentialStore(),
    effectBoundary,
    fetch: async (_url, init) => {
      calls.provider += 1;
      calls.providerIdempotencyKeys.push(new Headers(init.headers).get("Idempotency-Key"));
      calls.providerUrls.push(String(_url));
      await onProviderRequest?.(init, _url);
      const body = JSON.parse(String(init.body));
      const supplied = await providerResponse?.(body, calls.provider, String(_url));
      if (supplied instanceof Response) return supplied;
      return jsonResponse(supplied ?? {
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
  llm.registerProvider(openaiCompatibleProvider("resumable-provider", "https://resumable-provider.test", { requiresCredential: false, ...providerOptions }));
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
        await onEvidenceAcquire?.(context);
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
    requests: overrides.requests ?? requests,
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
    allowIncompleteEvidence: overrides.allowIncompleteEvidence,
    crossDomain: overrides.crossDomain,
    promptBuilder: overrides.promptBuilder ?? (({ values }) => [{
      id: "resumable-transient-value",
      content: JSON.stringify({ claim: Object.values(values)[0]?.claim }),
      required: true,
      priority: 970,
    }]),
    resumablePolicyIdentity: overrides.resumablePolicyIdentity ?? {
      projector: { id: "resumable-projector", version: "1" },
      value_rehydrator: { id: "resumable-value-rehydrator", version: "1" },
      prompt_builder: { id: "resumable-prompt-builder", version: "1" },
      provider_policy: { id: "resumable-provider-policy", version: "1", config_digest: PROVIDER_POLICY_CONFIG_DIGEST },
    },
    rehydrateProviderRun: overrides.rehydrateProviderRun,
    rehydrateAutomaticRun: overrides.rehydrateAutomaticRun,
    rehydrateCrossDomainRun: overrides.rehydrateCrossDomainRun,
    resumeProvider: overrides.resumeProvider,
    evidenceCheckpointStore: overrides.evidenceCheckpointStore,
    evidenceJobId: overrides.evidenceJobId,
    evidenceReconciliationAuthority: overrides.evidenceReconciliationAuthority
      ?? (overrides.evidenceCheckpointStore !== undefined ? { id: "source-audit", version: "1" } : undefined),
    evidenceExecutionPolicyIdentity: overrides.evidenceExecutionPolicyIdentity
      ?? (overrides.evidenceCheckpointStore !== undefined ? {
        projector: { id: "resumable-projector", version: "1" },
        evaluator: { id: "resumable-evaluator", version: "1" },
        journal: { id: "resumable-journal", version: "1" },
        value_rehydrator: { id: "resumable-value-rehydrator", version: "1" },
      } : undefined),
    evidenceReconciliationReceipt: overrides.evidenceReconciliationReceipt,
    evidenceResumeAfterReconciliation: overrides.evidenceResumeAfterReconciliation,
  };
}

function resealCheckpoint(checkpoint, changes = {}) {
  const merged = { ...structuredClone(checkpoint), ...changes };
  const {
    checkpoint_digest: _checkpointDigest,
    retention,
    secret_material: secretMaterial,
    ...payload
  } = merged;
  return {
    ...payload,
    checkpoint_digest: digestJsonSync(payload),
    retention,
    secret_material: secretMaterial,
  };
}

function recordingCheckpointStore() {
  const state = { current: null, transitions: [], dispatchReceipts: [], privateDispatchReceipts: [] };
  return {
    state,
    store: {
      read: () => state.current === null ? null : structuredClone(state.current),
      write: (checkpoint) => {
        state.current = structuredClone(checkpoint);
        state.transitions.push({ kind: "write", expected: null, checkpoint: structuredClone(checkpoint) });
      },
      writeIfUnchanged: (expected, checkpoint) => {
        if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
        state.current = structuredClone(checkpoint);
        state.transitions.push({ kind: "cas", expected, checkpoint: structuredClone(checkpoint) });
        return true;
      },
      writeDispatchIfUnchanged: (expected, checkpoint, receipt) => {
        if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
        state.current = structuredClone(checkpoint);
        state.dispatchReceipts.push({
          projection: receipt.toJSON(),
          providerIdempotencyKey: receipt.providerIdempotencyKey(),
        });
        state.privateDispatchReceipts.push(receipt);
        state.transitions.push({ kind: "dispatch", expected, checkpoint: structuredClone(checkpoint) });
        return true;
      },
    },
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
    resumablePolicyIdentity: {
      projector: { id: "cross-resumable-projector", version: "1" },
      value_rehydrator: { id: "cross-resumable-value-rehydrator", version: "1" },
      provider_policy: { id: "cross-resumable-provider-policy", version: "1", config_digest: PROVIDER_POLICY_CONFIG_DIGEST },
    },
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

test("restored source approval advances only to provider_pending before any provider authority", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "source-approval-transition-job", store);
  const task = "Approve evidence and provider work in separate durable transitions.";
  const held = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { approveSourceDispatch: false },
  }));
  assert.equal(held.run.status, "evidence_review_required");
  assert.equal(held.run.checkpoint.generation, 1);
  assert.equal(held.run.checkpoint.provider_dispatch_count, 0);
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);

  const evidenceApproved = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
    resumeProvider: true,
  }));
  assert.equal(evidenceApproved.run.status, "provider_pending");
  assert.equal(evidenceApproved.run.checkpoint.status, "provider_pending");
  assert.equal(evidenceApproved.run.checkpoint.generation, 2);
  assert.equal(evidenceApproved.run.checkpoint.previous_checkpoint_digest, held.run.checkpoint.checkpoint_digest);
  assert.equal(evidenceApproved.run.checkpoint.provider_dispatch_count, 0);
  assert.ok(calls.evidence > 0);
  assert.equal(calls.provider, 0);

  const providerApproved = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => evidenceApproved.run.result.evidence.runtime.values[receipt.request_digest] ?? null },
    resumeProvider: true,
  }));
  assert.equal(providerApproved.run.status, "completed");
  assert.equal(providerApproved.run.checkpoint.provider_dispatch_count, 1);
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

test("checkpointed evidence-backed runs forward typed policy identity and reconciliation receipts", async () => {
  const { agent, registry, calls } = await setup();
  const domain = "coding";
  const journal = new AcknowledgementLostJournal();
  const sourceCheckpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const firstOptions = await optionsFor(agent, registry, domain, journal, {
    run: { approveProviderCall: false },
    evidenceCheckpointStore: sourceCheckpointStore,
    evidenceJobId: "evidence-backed-reconciliation-source-job",
  });
  await assert.rejects(
    agent.runWithReviewedEvidence("Recover one uncertain source boundary before provider review.", firstOptions),
    /source acknowledgement lost/,
  );
  assert.equal(calls.evidence, 1);
  assert.equal(calls.provider, 0);
  const uncertainCheckpoint = sourceCheckpointStore.read();
  assert.equal(uncertainCheckpoint.status, "reconciliation_required");

  const evidencePlan = await agent.evidencePlan([domain]);
  const evidenceController = await agent.createEvidenceExecutionController(registry);
  const executionPlan = await evidenceController.prepare(evidencePlan, firstOptions.prepare);
  const [succeeded] = await journal.records();
  assert.ok(succeeded);
  const reconciliationReceipt = createAutonomousEvidenceExecutionReconciliationReceipt({
    jobId: "evidence-backed-reconciliation-source-job",
    checkpoint: uncertainCheckpoint,
    evidencePlan,
    requests: firstOptions.requests,
    authorityId: "source-audit",
    authorityVersion: "1",
    outcomes: firstOptions.requests.map((_request, index) => ({
      outcome: index === 0 ? "succeeded" : "not_executed",
      evidenceDigest: digestJsonSync({ source_audit: "evidence-backed", index }),
      evidenceKind: "source_dispatch_audit",
      effectAbsent: index !== 0,
      ...(index === 0 ? { succeededReceiptDigest: succeeded.receipt.receipt_digest } : {}),
    })),
  });
  const recovered = await agent.runWithReviewedEvidence(
    "Recover one uncertain source boundary before provider review.",
    await optionsFor(agent, registry, domain, journal, {
      run: { approveProviderCall: false },
      execute: {
        rehydrateValue: (receipt) => ({
          domain,
          requirement: receipt.requirement_id,
          claim: "resumable-transient-claim",
        }),
      },
      evidenceCheckpointStore: sourceCheckpointStore,
      evidenceJobId: "evidence-backed-reconciliation-source-job",
      evidenceReconciliationReceipt: reconciliationReceipt,
    }),
  );
  assert.equal(recovered.status, "approval_required");
  assert.equal(recovered.evidence.status, "completed");
  assert.equal(calls.evidence, firstOptions.requests.length);
  assert.equal(calls.provider, 0);
});

test("completed provider rehydration rejects prompt projection drift before the rehydrator or provider", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "prompt-drift-job", checkpointStore);
  const task = "Reject a changed evidence prompt across restart.";
  const first = await controller.run(task, await optionsFor(agent, registry, "coding", journal));
  const values = first.run.result.evidence.runtime.values;
  const providerResult = first.run.result.run;
  const evidenceCalls = calls.evidence;
  const providerCalls = calls.provider;
  let rehydratorCalls = 0;

  await assert.rejects(
    async () => controller.run(task, await optionsFor(agent, registry, "coding", journal, {
      execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
      promptBuilder: () => [{ id: "changed-prompt", content: "marker-b", required: true, priority: 970 }],
      // Holding the declared identity constant cannot conceal changed projected bytes.
      resumablePolicyIdentity: {
        projector: { id: "resumable-projector", version: "1" },
        value_rehydrator: { id: "resumable-value-rehydrator", version: "1" },
        prompt_builder: { id: "resumable-prompt-builder", version: "1" },
        provider_policy: { id: "resumable-provider-policy", version: "1", config_digest: PROVIDER_POLICY_CONFIG_DIGEST },
      },
      rehydrateProviderRun: () => {
        rehydratorCalls += 1;
        return providerResult;
      },
    })),
    /prompt projection/,
  );
  assert.equal(rehydratorCalls, 0);
  assert.equal(calls.evidence, evidenceCalls);
  assert.equal(calls.provider, providerCalls);
});

test("completed provider rehydration rejects evaluator identity drift before callbacks", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "evaluator-drift-job", checkpointStore);
  const task = "Reject a changed evidence evaluator across restart.";
  const first = await controller.run(task, await optionsFor(agent, registry, "coding", journal));
  const evidenceCalls = calls.evidence;
  const providerCalls = calls.provider;
  let rehydratorCalls = 0;

  await assert.rejects(
    async () => controller.run(task, await optionsFor(agent, registry, "coding", journal, {
      execute: {
        evaluator: {
          evaluator_id: "resumable-evaluator",
          evaluator_version: "2",
          evaluate: ({ requirement }) => ({ evaluator_id: "resumable-evaluator", evaluator_version: "2", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
        },
      },
      rehydrateProviderRun: () => {
        rehydratorCalls += 1;
        return first.run.result.run;
      },
    })),
    /run policy/,
  );
  assert.equal(rehydratorCalls, 0);
  assert.equal(calls.evidence, evidenceCalls);
  assert.equal(calls.provider, providerCalls);
});

test("incomplete-evidence provider attempts are operation-bound reconciliation and never replayed", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "incomplete-evidence-job", checkpointStore);
  const task = "Keep an exploratory synthesis incomplete across restart.";
  const completeOptions = await optionsFor(agent, registry, "science", journal);
  const firstOptions = await optionsFor(agent, registry, "science", journal, {
    requests: completeOptions.requests.slice(0, 1),
    allowIncompleteEvidence: true,
  });
  const first = await controller.run(task, firstOptions);
  assert.equal(first.run.status, "provider_reconciliation_required");
  assert.equal(first.run.result.run?.status, "completed");
  assert.equal(first.run.checkpoint.status, "provider_reconciliation_required");
  assert.match(first.run.checkpoint.provider_operation_digest, /^[0-9a-f]{64}$/);
  assert.match(first.run.checkpoint.provider_result_digest, /^[0-9a-f]{64}$/);
  assert.equal(first.run.checkpoint.provider_status, "completed");
  const rawValues = first.run.result.evidence.runtime.values;
  const evidenceCalls = calls.evidence;
  const providerCalls = calls.provider;

  const second = await controller.run(task, await optionsFor(agent, registry, "science", journal, {
    requests: completeOptions.requests.slice(0, 1),
    allowIncompleteEvidence: true,
    execute: { rehydrateValue: (receipt) => rawValues[receipt.request_digest] ?? null },
  }));
  assert.equal(second.run.status, "provider_reconciliation_required");
  assert.equal(second.run.provider_rehydrated, false);
  assert.equal(calls.evidence, evidenceCalls);
  assert.equal(calls.provider, providerCalls);
});

test("custom resumable callbacks require explicit stable identities before evidence dispatch", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "missing-callback-identity-job", checkpointStore);
  const options = await optionsFor(agent, registry, "coding", journal);
  delete options.resumablePolicyIdentity.prompt_builder;
  await assert.rejects(
    () => controller.run("Reject an unidentified prompt projector.", options),
    /requires prompt_builder identity/,
  );
  const unidentifiedRehydrator = await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: () => null },
  });
  delete unidentifiedRehydrator.resumablePolicyIdentity.value_rehydrator;
  await assert.rejects(
    () => controller.run("Reject an unidentified value rehydrator.", unidentifiedRehydrator),
    /requires value_rehydrator identity/,
  );
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);
});

test("resumable provider policy refuses missing candidates, opaque state identity, and unknown option fields before evidence", async () => {
  const { agent, registry, calls } = await setup();

  const missingCandidates = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  delete missingCandidates.run.candidates;
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "missing-provider-candidates-job", new InMemoryAutonomousEvidenceBackedCheckpointStore()).run("Reject an implicit model catalogue.", missingCandidates),
    /explicit non-empty run\.candidates/,
  );

  const missingPlanningCandidates = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    runMode: "auto",
    run: { planningMode: "provider", planning: { approveProviderCall: false } },
  });
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "missing-planning-candidates-job", new InMemoryAutonomousEvidenceBackedCheckpointStore()).run("Reject an implicit planning model catalogue.", missingPlanningCandidates),
    /explicit non-empty planning\.candidates/,
  );

  const missingOpaqueIdentity = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  delete missingOpaqueIdentity.resumablePolicyIdentity.provider_policy;
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "missing-provider-policy-job", new InMemoryAutonomousEvidenceBackedCheckpointStore()).run("Reject unidentified provider state.", missingOpaqueIdentity),
    /requires provider_policy identity with config_digest/,
  );

  const unknownField = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  unknownField.run.futureMutableProviderControl = { enabled: true };
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "unknown-provider-policy-job", new InMemoryAutonomousEvidenceBackedCheckpointStore()).run("Reject an unknown provider control.", unknownField),
    /run options contains unsupported fields/,
  );

  const unknownOuterField = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  unknownOuterField.futureMutableEvidenceControl = { enabled: true };
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "unknown-resumable-option-job", new InMemoryAutonomousEvidenceBackedCheckpointStore()).run("Reject an unknown outer control.", unknownOuterField),
    /(?:resumable|controller run) options contains unsupported fields/,
  );

  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);
});

test("pending restore rejects route, full tool schema, planning, and prompt-manifest drift before callbacks", async () => {
  const { agent, registry, calls } = await setup();
  const task = "Bind every provider-shaping control before a resumable run.";
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-policy-drift-job", store);
  const codingRoute = await agent.route(task, { domain: "coding" });
  const dataRoute = await agent.route(task, { domain: "data" });
  const promptA = new AutonomousPromptTemplate({
    promptId: "resumable-policy-prompt",
    version: "1",
    domain: "coding",
    capabilities: ["code"],
    stages: ["answer", "planning"],
    templateDigest: digestJsonSync({ prompt: "a" }),
    render: () => [{ role: "user", content: "stable prompt a" }],
  });
  const promptB = new AutonomousPromptTemplate({
    promptId: "resumable-policy-prompt",
    version: "1",
    domain: "coding",
    capabilities: ["code"],
    stages: ["answer", "planning"],
    templateDigest: digestJsonSync({ prompt: "b" }),
    render: () => [{ role: "user", content: "stable prompt b" }],
  });
  const tools = [{
    name: "lookup",
    description: "Look up one reviewed value.",
    parameters: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
  }];
  const baseRun = {
    approveProviderCall: false,
    routeOverride: codingRoute,
    tools,
    promptTemplate: promptA,
    maxOutputTokens: 384,
  };
  const first = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
    run: baseRun,
  }));
  assert.equal(first.run.status, "provider_pending");
  assert.equal(calls.provider, 0);
  const evidenceCalls = calls.evidence;

  const resumedBase = {
    ...baseRun,
    approveProviderCall: true,
  };
  const driftCases = [
    { label: "route", change: { routeOverride: dataRoute } },
    {
      label: "tool schema",
      change: {
        tools: [{
          ...tools[0],
          parameters: { type: "object", properties: { query: { type: "number" } }, required: ["query"] },
        }],
      },
    },
    { label: "prompt manifest", change: { promptTemplate: promptB } },
  ];
  for (const drift of driftCases) {
    await assert.rejects(
      async () => controller.run(task, await optionsFor(agent, registry, "coding", journal, {
        resumeProvider: true,
        run: { ...resumedBase, ...drift.change },
      })),
      /run policy/,
      drift.label,
    );
    assert.equal(calls.evidence, evidenceCalls, drift.label);
    assert.equal(calls.provider, 0, drift.label);
  }

  const planning = { approveProviderCall: false, candidates: [model()], maxOutputTokens: 192, promptStage: "planning" };
  const planningJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const planningStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const planningController = new AutonomousEvidenceBackedController(agent, "provider-planning-policy-drift-job", planningStore);
  const planningTask = "Bind nested provider planning controls before automatic execution.";
  const planningFirst = await planningController.run(planningTask, await optionsFor(agent, registry, "coding", planningJournal, {
    runMode: "auto",
    run: { approveProviderCall: false, planningMode: "provider", planning, acceptPlan: false },
  }));
  assert.equal(planningFirst.run.status, "provider_pending");
  const planningEvidenceCalls = calls.evidence;
  await assert.rejects(
    async () => planningController.run(planningTask, await optionsFor(agent, registry, "coding", planningJournal, {
      runMode: "auto",
      resumeProvider: true,
      run: {
        approveProviderCall: true,
        planningMode: "provider",
        planning: { ...planning, approveProviderCall: true, maxOutputTokens: 193 },
        acceptPlan: false,
      },
    })),
    /run policy/,
  );
  assert.equal(calls.evidence, planningEvidenceCalls);
  assert.equal(calls.provider, 0);
});

test("provider dispatch uses the pre-evidence run snapshot despite caller mutation during acquisition", async () => {
  let mutableOptions;
  let providerBody = null;
  let mutated = false;
  const { agent, registry, calls } = await setup({
    onEvidenceAcquire: () => {
      if (mutated) return;
      mutated = true;
      mutableOptions.run.candidates[0].model = "mutated-after-snapshot";
      mutableOptions.run.maxOutputTokens = 999;
      mutableOptions.run.tools[0].description = "mutated tool";
      mutableOptions.run.tools[0].parameters.properties.query.type = "number";
      mutableOptions.requests[0].source_id = "mutated-source";
      mutableOptions.domains[0] = "data";
      mutableOptions.availableEvidence.push("mutated-evidence");
      mutableOptions.completedStages.coding = ["mutated-stage"];
    },
    onProviderRequest: (init) => {
      providerBody = JSON.parse(String(init.body));
    },
  });
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    run: {
      candidates: [{ ...model(), model: "snapshotted-model" }],
      maxOutputTokens: 321,
      tools: [{
        name: "lookup",
        description: "snapshotted tool",
        parameters: { type: "object", properties: { query: { type: "string" } }, required: ["query"] },
      }],
    },
  });
  options.availableEvidence = [];
  options.completedStages = {};
  const expectedRequestDigest = digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA,
    requests: structuredClone(options.requests),
  });
  mutableOptions = options;
  const result = await new AutonomousEvidenceBackedController(
    agent,
    "provider-policy-toctou-job",
    new InMemoryAutonomousEvidenceBackedCheckpointStore(),
  ).run("Use the snapshotted provider request policy.", options);

  assert.equal(result.run.status, "completed");
  assert.equal(calls.provider, 1);
  assert.equal(result.run.checkpoint.request_digest, expectedRequestDigest);
  assert.deepEqual(result.run.result.execution_plan.domains, ["coding"]);
  assert.equal(providerBody.model, "snapshotted-model");
  assert.equal(providerBody.max_tokens, 321);
  assert.equal(providerBody.tools[0].function.description, "snapshotted tool");
  assert.equal(providerBody.tools[0].function.parameters.properties.query.type, "string");
});

test("adapter manifest mutation after the synchronous snapshot is rejected before source or provider callbacks", async () => {
  const { agent, registry, calls } = await setup();
  let checkpoint = null;
  const base = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  const run = runAutonomousEvidenceBackedResumable(agent, "Reject a mutable evidence adapter registry after policy binding.", {
    ...base,
    jobId: "adapter-registry-toctou-job",
    checkpointSink: (next) => { checkpoint = structuredClone(next); },
    checkpointCompareAndStore: (expected, next) => {
      if ((checkpoint?.checkpoint_digest ?? null) !== expected) return false;
      checkpoint = structuredClone(next);
      return true;
    },
    checkpointDispatchCompareAndStore: (expected, next) => {
      if ((checkpoint?.checkpoint_digest ?? null) !== expected) return false;
      checkpoint = structuredClone(next);
      return true;
    },
  });
  assert.equal(registry.unregister("resumable-data"), true);
  const result = await run;
  assert.equal(result.status, "evidence_failed");
  assert.equal(result.result.status, "evidence_failed");
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);
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

  const completedStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const completedRun = await new AutonomousEvidenceBackedController(agent, "checkpoint-transition-job", completedStore).run(
    "Persist one terminal provider checkpoint.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );
  const completed = completedRun.run.checkpoint;
  assert.equal(completed.status, "completed");
  const forgedPending = resealCheckpoint(completed, {
    status: "provider_pending",
    generation: completed.generation + 1,
    previous_checkpoint_digest: completed.checkpoint_digest,
    provider_operation_digest: null,
    provider_result_digest: null,
    provider_status: null,
    provider_dispatch_count: 0,
    provider_dispatch_head_digest: null,
  });
  await validateAutonomousEvidenceBackedCheckpoint(forgedPending);
  await assert.rejects(
    completedStore.writeIfUnchanged(completed.checkpoint_digest, forgedPending),
    /completed -> provider_pending is not permitted/,
  );
  assert.equal((await completedStore.read()).checkpoint_digest, completed.checkpoint_digest);

  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
  };
  const jsonStore = new JsonAutonomousEvidenceBackedCheckpointStore(textStore);
  await jsonStore.write(checkpoint);
  assert.deepEqual(await jsonStore.read(), checkpoint);
  await jsonStore.write(completed);
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
  await assert.rejects(
    transactional.writeIfUnchanged(completed.checkpoint_digest, forgedPending),
    /completed -> provider_pending is not permitted/,
  );
  assert.equal((await transactional.read()).checkpoint_digest, completed.checkpoint_digest);
});

test("transactional JSON dispatch persistence commits the private receipt and public in-flight head together", async () => {
  let encoded = null;
  let privateReceiptValue = null;
  let persistedAtProvider = null;
  const transitions = [];
  const { agent, registry, calls } = await setup({
    onProviderRequest: () => {
      persistedAtProvider = encoded === null ? null : JSON.parse(encoded);
    },
  });
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).checkpoint_digest;
      if (current !== expected) return false;
      encoded = value;
      transitions.push({ kind: "checkpoint", value: JSON.parse(value) });
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpointValue, receiptValue) => {
      const current = encoded === null ? null : JSON.parse(encoded).checkpoint_digest;
      if (current !== expected) return false;
      encoded = checkpointValue;
      privateReceiptValue = receiptValue;
      transitions.push({ kind: "dispatch", value: JSON.parse(checkpointValue) });
      return true;
    },
  };
  const store = new TransactionalJsonAutonomousEvidenceBackedCheckpointStore(textStore);
  const result = await new AutonomousEvidenceBackedController(agent, "transactional-json-dispatch-job", store).run(
    "Commit the private dispatch receipt with its public head.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );

  assert.equal(result.run.status, "completed");
  assert.equal(transitions.length, 2);
  assert.equal(transitions[0].kind, "dispatch");
  assert.equal(transitions[0].value.status, "provider_in_flight");
  assert.equal(transitions[1].kind, "checkpoint");
  assert.equal(transitions[1].value.status, "completed");
  assert.deepEqual(persistedAtProvider, transitions[0].value);
  const privateReceipt = JSON.parse(privateReceiptValue);
  assert.equal(privateReceipt.schema, "bioprism-typescript-autonomous-evidence-backed-provider-dispatch-private/0.1");
  assert.equal(privateReceipt.provider_idempotency_key, calls.providerIdempotencyKeys[0]);
  assert.equal(privateReceipt.projection.receipt_digest, transitions[0].value.provider_dispatch_head_digest);
  assert.doesNotMatch(JSON.stringify(privateReceipt.projection), new RegExp(privateReceipt.provider_idempotency_key));
  assert.doesNotMatch(encoded, new RegExp(privateReceipt.provider_idempotency_key));
});

test("fresh provider dispatch commits an exact in-flight-to-terminal CAS chain before HTTP", async () => {
  let persistedAtProvider = null;
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup({
    onProviderRequest: () => {
      persistedAtProvider = structuredClone(state.current);
    },
  });
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-cas-chain-job", store);
  const result = await controller.run(
    "Fence one provider call with an exact durable chain.",
    await optionsFor(agent, registry, "coding", journal),
  );

  assert.equal(result.run.status, "completed");
  assert.equal(state.transitions.length, 2);
  const [inFlightTransition, terminalTransition] = state.transitions;
  const inFlight = inFlightTransition.checkpoint;
  const terminal = terminalTransition.checkpoint;
  assert.equal(inFlight.schema, AUTONOMOUS_EVIDENCE_BACKED_CHECKPOINT_SCHEMA);
  assert.equal(inFlight.status, "provider_in_flight");
  assert.equal(inFlight.generation, 1);
  assert.equal(inFlight.previous_checkpoint_digest, null);
  assert.match(inFlight.provider_operation_digest, /^[0-9a-f]{64}$/);
  assert.equal(inFlight.provider_dispatch_count, 1);
  assert.match(inFlight.provider_dispatch_head_digest, /^[0-9a-f]{64}$/);
  assert.deepEqual(persistedAtProvider, inFlight);
  assert.equal(terminal.status, "completed");
  assert.equal(terminal.generation, 2);
  assert.equal(terminal.previous_checkpoint_digest, inFlight.checkpoint_digest);
  assert.equal(terminal.provider_dispatch_count, 1);
  assert.equal(terminal.provider_dispatch_head_digest, inFlight.provider_dispatch_head_digest);
  assert.equal(terminalTransition.expected, inFlight.checkpoint_digest);
  assert.equal(result.run.checkpoint.checkpoint_digest, terminal.checkpoint_digest);
  const rootKey = digestJsonSync({
    schema: "bioprism-typescript-autonomous-evidence-backed-provider-idempotency/0.1",
    provider_operation_digest: inFlight.provider_operation_digest,
  });
  assert.equal(calls.providerIdempotencyKeys.length, 1);
  assert.match(calls.providerIdempotencyKeys[0], /^[0-9a-f]{64}$/);
  assert.notEqual(calls.providerIdempotencyKeys[0], rootKey);
  assert.equal(state.dispatchReceipts.length, 1);
  assert.equal(state.dispatchReceipts[0].providerIdempotencyKey, calls.providerIdempotencyKeys[0]);
  assert.equal(state.dispatchReceipts[0].projection.receipt_digest, inFlight.provider_dispatch_head_digest);
  assert.doesNotMatch(JSON.stringify(result.run), new RegExp(calls.providerIdempotencyKeys[0]));
  assert.doesNotMatch(JSON.stringify(state.privateDispatchReceipts[0]), new RegExp(calls.providerIdempotencyKeys[0]));
});

test("each HTTP retry receives an ordered durable receipt for the exact provider key", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup({
    providerOptions: { maxAttempts: 2, retryBackoffMs: 1 },
    providerResponse: (_body, call) => call === 1
      ? jsonResponse({ error: "retry" }, 503)
      : { choices: [{ message: { role: "assistant", content: "retry completed" }, finish_reason: "stop" }] },
  });
  const result = await new AutonomousEvidenceBackedController(agent, "provider-retry-receipt-job", store).run(
    "Record every retry before transport.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );
  assert.equal(result.run.status, "completed");
  assert.equal(calls.provider, 2);
  assert.equal(state.dispatchReceipts.length, 2);
  assert.deepEqual(state.dispatchReceipts.map(({ projection }) => projection.sequence), [1, 2]);
  assert.equal(state.dispatchReceipts[0].projection.previous_receipt_digest, null);
  assert.equal(state.dispatchReceipts[1].projection.previous_receipt_digest, state.dispatchReceipts[0].projection.receipt_digest);
  assert.deepEqual(state.dispatchReceipts.map(({ projection }) => projection.transport_attempt), [1, 2]);
  assert.deepEqual(state.dispatchReceipts.map(({ providerIdempotencyKey }) => providerIdempotencyKey), calls.providerIdempotencyKeys);
  assert.equal(new Set(calls.providerIdempotencyKeys).size, 1);
  assert.equal(result.run.checkpoint.provider_dispatch_count, 2);
  assert.equal(result.run.checkpoint.provider_dispatch_head_digest, state.dispatchReceipts[1].projection.receipt_digest);
  assert.deepEqual(state.transitions.map(({ checkpoint }) => checkpoint.generation), [1, 2, 3]);
});

test("model failover receipts capture each distinct request-bound provider key", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup({
    providerOptions: { maxAttempts: 1 },
    providerResponse: (_body, _call, url) => url.startsWith("https://resumable-provider.test")
      ? jsonResponse({ error: "fail over" }, 503)
      : { choices: [{ message: { role: "assistant", content: "stable failover" }, finish_reason: "stop" }] },
  });
  const stableModel = { ...model(), provider: "resumable-stable", model: "resumable-stable-model", quality: 0.5 };
  agent.llm.registerProvider(openaiCompatibleProvider("resumable-stable", "https://resumable-stable.test", { requiresCredential: false, maxAttempts: 1 }));
  agent.registerModel(stableModel);
  const result = await new AutonomousEvidenceBackedController(agent, "provider-failover-receipt-job", store).run(
    "Record both failed and successful failover transports.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
      run: { candidates: [model(), stableModel], maxProviderFailovers: 1 },
    }),
  );
  assert.equal(result.run.status, "completed");
  assert.deepEqual(state.dispatchReceipts.map(({ projection }) => projection.provider), ["resumable-provider", "resumable-stable"]);
  assert.deepEqual(state.dispatchReceipts.map(({ projection }) => projection.transport_attempt), [1, 1]);
  assert.deepEqual(state.dispatchReceipts.map(({ providerIdempotencyKey }) => providerIdempotencyKey), calls.providerIdempotencyKeys);
  assert.notEqual(calls.providerIdempotencyKeys[0], calls.providerIdempotencyKeys[1]);
  assert.equal(result.run.checkpoint.provider_dispatch_count, 2);
});

test("nontransactional persistence and a losing CAS refuse provider dispatch", async () => {
  {
    const { agent, registry, calls } = await setup();
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    let current = null;
    const controller = new AutonomousEvidenceBackedController(agent, "nontransactional-provider-job", {
      read: () => current,
      write: (checkpoint) => { current = structuredClone(checkpoint); },
    });
    const options = await optionsFor(agent, registry, "coding", journal);
    await assert.rejects(
      () => controller.run("Refuse a provider without transactional persistence.", options),
      /atomic checkpointCompareAndStore/,
    );
    assert.equal(current, null);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }

  {
    const { agent, registry, calls } = await setup();
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    const controller = new AutonomousEvidenceBackedController(agent, "provider-cas-conflict-job", {
      read: () => null,
      write: () => { throw new Error("ordinary write must not be used for provider dispatch"); },
      writeIfUnchanged: () => false,
      writeDispatchIfUnchanged: () => false,
    });
    const options = await optionsFor(agent, registry, "coding", journal);
    await assert.rejects(
      () => controller.run("Lose the in-flight CAS without invoking a provider.", options),
      /dispatch transaction.*reload required/,
    );
    assert.ok(calls.evidence > 0);
    assert.equal(calls.provider, 0);
  }
});

test("ambiguous dispatch acknowledgements send nothing and force a controller reload", async () => {
  for (const acknowledgement of ["false", "throw"]) {
    const state = { current: null, reads: 0 };
    const persistence = {
      read: () => {
        state.reads += 1;
        return state.current === null ? null : structuredClone(state.current);
      },
      write: (checkpoint) => { state.current = structuredClone(checkpoint); },
      writeIfUnchanged: (expected, checkpoint) => {
        if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
        state.current = structuredClone(checkpoint);
        return true;
      },
      writeDispatchIfUnchanged: (expected, checkpoint) => {
        if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
        // Deliberately emulate a backend that commits but loses or falsifies its acknowledgement.
        state.current = structuredClone(checkpoint);
        if (acknowledgement === "throw") throw new Error("dispatch acknowledgement lost");
        return false;
      },
    };
    const { agent, registry, calls } = await setup();
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    const controller = new AutonomousEvidenceBackedController(agent, `ambiguous-dispatch-${acknowledgement}`, persistence);
    const task = "Never send without an exact durable dispatch acknowledgement.";
    const firstOptions = await optionsFor(agent, registry, "coding", journal);
    await assert.rejects(
      () => controller.run(task, firstOptions),
      /dispatch transaction.*reload required/,
    );
    assert.equal(calls.provider, 0);
    assert.equal(state.current.status, "provider_in_flight");
    const readsAfterFailure = state.reads;
    const restored = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
      execute: { rehydrateValue: (receipt) => ({ domain: "coding", requirement: receipt.requirement_id, claim: "resumable-transient-claim" }) },
    }));
    assert.ok(state.reads > readsAfterFailure);
    assert.equal(restored.run.status, "provider_reconciliation_required");
    assert.equal(calls.provider, 0);
  }
});

test("dispatch persistence cannot mutate its acknowledged checkpoint before transport", async () => {
  const state = { current: null };
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      checkpoint.status = "completed";
      return true;
    },
  };
  const { agent, registry, calls } = await setup();
  const controller = new AutonomousEvidenceBackedController(agent, "mutating-dispatch-commit-job", persistence);
  await assert.rejects(
    async () => controller.run(
      "Refuse a mutated dispatch commit value before provider transport.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
    ),
    /dispatch transaction mutated its commit value.*reload required/,
  );
  assert.equal(calls.provider, 0);
  assert.equal(state.current.status, "provider_in_flight");
});

test("provider-free policy refusal stays pending without an attempted-provider checkpoint", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-policy-pretransport-job", store);
  const result = await controller.run(
    "Keep strict-policy refusal on the pre-transport side of the fence.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
      run: {
        domainPolicyMode: "strict",
        domainPolicyEvidenceReady: true,
        domainPolicyEvaluatorConfigured: false,
      },
    }),
  );
  assert.equal(result.run.status, "provider_pending");
  assert.equal(result.run.checkpoint.status, "provider_pending");
  assert.equal(result.run.checkpoint.provider_operation_digest, null);
  assert.equal(state.transitions.length, 1);
  assert.equal(state.transitions.some(({ checkpoint }) => checkpoint.status === "provider_in_flight"), false);
  assert.equal(calls.provider, 0);
});

test("a caller dispatch observer refusal runs before the final transport fence", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-observer-order-job", store);
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    run: { observer: { dispatch: () => { throw new Error("caller dispatch observer refusal"); } } },
  });
  await assert.rejects(
    () => controller.run(
      "Keep observer refusal on the pre-attempt side of the transport fence.",
      options,
    ),
    /caller dispatch observer refusal|provider transport failed/,
  );
  assert.equal(state.transitions.length, 0);
  assert.equal(state.current, null);
  assert.equal(calls.provider, 0);
});

test("caller cancellation inside a dispatch observer stays before the durable provider fence", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup();
  const abort = new AbortController();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-observer-abort-job", store);
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    run: {
      signal: abort.signal,
      observer: { dispatch: () => { abort.abort(); } },
    },
  });
  await assert.rejects(
    () => controller.run(
      "Keep observer-triggered caller cancellation on the pre-attempt side of the fence.",
      options,
    ),
    /aborted/,
  );
  assert.equal(state.transitions.length, 0);
  assert.equal(state.current, null);
  assert.equal(calls.provider, 0);
});

test("in-memory checkpoint CAS admits only one concurrent controller to provider transport", async () => {
  const { agent, registry, calls } = await setup();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const task = "Allow exactly one concurrent provider controller.";
  const controllers = [
    new AutonomousEvidenceBackedController(agent, "provider-concurrent-cas-job", store),
    new AutonomousEvidenceBackedController(agent, "provider-concurrent-cas-job", store),
  ];
  const options = await Promise.all(controllers.map(() => optionsFor(
    agent,
    registry,
    "coding",
    new InMemoryAutonomousEvidenceRuntimeJournal(),
  )));
  const settled = await Promise.allSettled(controllers.map((controller, index) => controller.run(task, options[index])));
  assert.equal(settled.filter(({ status }) => status === "fulfilled").length, 1);
  assert.equal(settled.filter(({ status }) => status === "rejected").length, 1);
  assert.match(String(settled.find(({ status }) => status === "rejected").reason), /dispatch transaction.*reload required/);
  assert.equal(calls.provider, 1);
  assert.equal((await store.read()).status, "completed");
});

test("accessor-backed resumable control envelopes are rejected before callbacks", async () => {
  const { agent, registry, calls } = await setup();
  let sinkCalls = 0;
  let promptGetterCalls = 0;
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  Object.assign(options, {
    jobId: "provider-accessor-envelope-job",
    checkpointSink: () => { sinkCalls += 1; },
    checkpointCompareAndStore: () => true,
    checkpointDispatchCompareAndStore: () => true,
  });
  Object.defineProperty(options, "promptBuilder", {
    enumerable: true,
    configurable: true,
    get: () => {
      promptGetterCalls += 1;
      return promptGetterCalls === 1 ? (() => []) : undefined;
    },
  });
  await assert.rejects(
    () => runAutonomousEvidenceBackedResumable(agent, "Reject a time-varying getter.", options),
    /enumerable data properties only/,
  );
  assert.equal(promptGetterCalls, 0);
  assert.equal(sinkCalls, 0);
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);
});

test("resumable execution rejects effect-boundary subclasses before source or checkpoint work", async () => {
  class MaliciousEffectBoundary extends AutonomousEffectBoundary {}
  for (const constructorBound of [false, true]) {
    const maliciousBoundary = new MaliciousEffectBoundary();
    const { agent, registry, calls } = await setup(constructorBound ? { effectBoundary: maliciousBoundary } : {});
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), constructorBound ? {} : {
      run: { effectBoundary: maliciousBoundary },
    });
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject a subclassed effect boundary.", {
        ...options,
        jobId: `effect-boundary-subclass-${constructorBound}`,
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /exact built-in AutonomousEffectBoundary/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
  {
    const { agent, registry, calls } = await setup();
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
    const coreFetchOnce = Object.getPrototypeOf(agent.llm).fetchOnce;
    let getterCalls = 0;
    Object.defineProperty(agent.llm, "fetchOnce", {
      get: () => {
        getterCalls += 1;
        return coreFetchOnce;
      },
    });
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject a shadowed final provider transport primitive.", {
        ...options,
        jobId: "shadowed-llm-fetch-once-job",
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /instance-shadowed fetchOnce provider path/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(getterCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
  {
    const { agent, registry, calls } = await setup();
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
    agent.runtime.llm = new LLMRuntime();
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject a split autonomous/provider runtime graph.", {
        ...options,
        jobId: "mismatched-autonomous-llm-job",
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /share the exact validated LLMRuntime/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
  {
    const { agent, registry, calls } = await setup();
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
    let getterCalls = 0;
    Object.defineProperty(agent.runtime, "llm", {
      get: () => {
        getterCalls += 1;
        return agent.llm;
      },
    });
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject an accessor-backed autonomous/provider runtime graph.", {
        ...options,
        jobId: "accessor-autonomous-llm-job",
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /share the exact validated LLMRuntime/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(getterCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
});

test("stock AutonomousEffectBoundary remains supported with uncached provider effects", async () => {
  const boundary = new AutonomousEffectBoundary();
  const { agent, registry, calls } = await setup({ effectBoundary: boundary });
  const result = await new AutonomousEvidenceBackedController(
    agent,
    "stock-effect-boundary-job",
    new InMemoryAutonomousEvidenceBackedCheckpointStore(),
  ).run(
    "Use the stock crash-safe effect boundary.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );
  assert.equal(result.run.status, "completed");
  assert.equal(result.run.checkpoint.provider_dispatch_count, 1);
  assert.equal(calls.provider, 1);
  assert.equal((await boundary.pendingRecords()).length, 0);
});

test("resumable execution rejects subclassed and instance-shadowed core provider paths before I/O", async () => {
  class MaliciousLLMRuntime extends LLMRuntime {}
  {
    const { agent, registry, calls } = await setup({ RuntimeClass: MaliciousLLMRuntime });
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject a subclassed provider runtime.", {
        ...options,
        jobId: "subclassed-llm-runtime-job",
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /exact built-in LLMRuntime/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
  {
    const { agent, registry, calls } = await setup();
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
    const coreRunWithReviewedEvidence = Object.getPrototypeOf(agent).runWithReviewedEvidence;
    let getterCalls = 0;
    Object.defineProperty(agent, "runWithReviewedEvidence", {
      get: () => {
        getterCalls += 1;
        return coreRunWithReviewedEvidence;
      },
    });
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject a shadowed agent provider path.", {
        ...options,
        jobId: "shadowed-agent-runtime-job",
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /instance-shadowed runWithReviewedEvidence/,
    );
    assert.equal(checkpointCalls, 0);
    assert.equal(getterCalls, 0);
    assert.equal(calls.evidence, 0);
    assert.equal(calls.provider, 0);
  }
});

test("resumable execution rejects provider transport graph changes before the final fence", async () => {
  for (const mutation of ["fetch", "provider"]) {
    const { agent, registry, calls } = await setup();
    let checkpointCalls = 0;
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
      promptBuilder: ({ values }) => {
        if (mutation === "fetch") {
          agent.llm.fetchImplementation = async () => jsonResponse({
            choices: [{ message: { role: "assistant", content: "wrong transport" }, finish_reason: "stop" }],
          });
        } else {
          agent.llm.registerProvider(openaiCompatibleProvider("resumable-provider", "https://changed-provider.test", { requiresCredential: false }));
        }
        return [{ id: "transport-mutation", content: JSON.stringify(values), required: true, priority: 970 }];
      },
    });
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, "Reject transport mutation during prompt projection.", {
        ...options,
        jobId: `transport-graph-${mutation}-job`,
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /provider (transport graph|registry) changed after its policy snapshot/,
    );
    assert.equal(checkpointCalls, 0);
    assert.ok(calls.evidence > 0);
    assert.equal(calls.provider, 0);
  }
});

test("the final fence rejects selected transports restored before inspection", async () => {
  for (const mutation of ["config", "fetch", "local_transport"]) {
    const { agent, registry, calls } = await setup();
    const providers = agent.llm.providers;
    const originalConfig = providers.get("resumable-provider");
    const originalFetch = agent.llm.fetchImplementation;
    let maliciousCalls = 0;
    const maliciousFetch = async () => {
      maliciousCalls += 1;
      return jsonResponse({ choices: [{ message: { role: "assistant", content: "wrong transport" }, finish_reason: "stop" }] });
    };
    const maliciousLocalTransport = Object.freeze({
      invoke: async () => {
        maliciousCalls += 1;
        return "wrong local transport";
      },
    });
    const maliciousConfig = Object.freeze({
      ...originalConfig,
      ...(mutation === "local_transport"
        ? { transport: maliciousLocalTransport }
        : { baseUrl: "https://coordinated-swap.test" }),
    });
    let selectionCallbacks = 0;
    let dispatchCallbacks = 0;
    const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
      run: {
        selectionEventCallback: async () => {
          selectionCallbacks += 1;
          if (mutation !== "fetch") providers.set("resumable-provider", maliciousConfig);
          if (mutation === "fetch") agent.llm.fetchImplementation = maliciousFetch;
        },
        observer: {
          dispatch: async () => {
            dispatchCallbacks += 1;
            providers.set("resumable-provider", originalConfig);
            agent.llm.fetchImplementation = originalFetch;
          },
        },
      },
    });
    let checkpointCalls = 0;
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, `Reject a coordinated ${mutation} swap and restore.`, {
        ...options,
        jobId: `coordinated-provider-${mutation}-swap-job`,
        checkpointSink: () => { checkpointCalls += 1; },
        checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
        checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
      }),
      /selected provider transport binding does not match/,
    );
    assert.ok(selectionCallbacks > 0);
    assert.equal(dispatchCallbacks, 1);
    assert.equal(checkpointCalls, 0);
    assert.ok(calls.evidence > 0);
    assert.equal(calls.provider, 0);
    assert.equal(maliciousCalls, 0);
  }
});

test("resumable execution rejects a shadowed downstream provider registry", async () => {
  const { agent, registry, calls } = await setup();
  agent.llm.providers.get = Map.prototype.get;
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  let checkpointCalls = 0;
  await assert.rejects(
    () => runAutonomousEvidenceBackedResumable(agent, "Reject a shadowed provider registry primitive.", {
      ...options,
      jobId: "shadowed-provider-registry-job",
      checkpointSink: () => { checkpointCalls += 1; },
      checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
      checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
    }),
    /unshadowed built-in provider registry/,
  );
  assert.equal(checkpointCalls, 0);
  assert.equal(calls.evidence, 0);
  assert.equal(calls.provider, 0);
});

test("dispatch persistence cannot swap the concrete fetch transport behind its receipt", async () => {
  const state = { current: null };
  const { agent, registry, calls } = await setup();
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      agent.llm.fetchImplementation = async () => jsonResponse({
        choices: [{ message: { role: "assistant", content: "wrong transport" }, finish_reason: "stop" }],
      });
      return true;
    },
  };
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, "post-dispatch-transport-swap-job", persistence).run(
      "Reject a transport swap by the dispatch transaction adapter.",
      options,
    ),
    /provider transport graph changed after its policy snapshot/,
  );
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
});

test("prompt callbacks cannot bind a new effect boundary after resumable preflight", async () => {
  let checkpointCalls = 0;
  const { agent, registry, calls } = await setup();
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    promptBuilder: ({ values }) => {
      agent.llm.bindEffectBoundary(new AutonomousEffectBoundary());
      return [{ id: "mutating-prompt", content: JSON.stringify(values), required: true, priority: 970 }];
    },
  });
  await assert.rejects(
    () => runAutonomousEvidenceBackedResumable(agent, "Reject a prompt-time boundary mutation.", {
      ...options,
      jobId: "prompt-boundary-mutation-job",
      checkpointSink: () => { checkpointCalls += 1; },
      checkpointCompareAndStore: () => { checkpointCalls += 1; return true; },
      checkpointDispatchCompareAndStore: () => { checkpointCalls += 1; return true; },
    }),
    /effectBoundary changed during resumable preparation/,
  );
  assert.equal(checkpointCalls, 0);
  assert.ok(calls.evidence > 0);
  assert.equal(calls.provider, 0);
});

test("terminal CAS rejects projection drift introduced after the in-flight fence", async () => {
  const { state, store } = recordingCheckpointStore();
  let capturedEvidence = null;
  const { agent, registry, calls } = await setup({
    onProviderRequest: () => { capturedEvidence.runtime.json.result_digest = "f".repeat(64); },
  });
  const controller = new AutonomousEvidenceBackedController(agent, "provider-terminal-projection-job", store);
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
    promptBuilder: ({ evidence, values }) => {
      capturedEvidence = evidence;
      return [{ id: "captured", content: JSON.stringify(values), required: true, priority: 970 }];
    },
  });
  await assert.rejects(
    () => controller.run(
      "Refuse terminal settlement if provider-adjacent state mutates reviewed evidence.",
      options,
    ),
    /current evidence result/,
  );
  assert.equal(calls.provider, 1);
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(state.transitions.length, 1);
});

test("a tool authorization refusal still binds the observed provider result", async () => {
  const { agent, registry, calls } = await setup({
    providerResponse: () => ({
      choices: [{
        message: {
          role: "assistant",
          content: "",
          tool_calls: [{ id: "reviewed-tool-call", type: "function", function: { name: "lookup", arguments: "{\"query\":\"reviewed\"}" } }],
        },
        finish_reason: "tool_calls",
      }],
    }),
  });
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const jobId = "provider-tool-refusal-result-job";
  const task = "Bind a provider response before tool authorization refusal.";
  const runOptions = {
    tools: [{ name: "lookup", description: "Look up reviewed data.", parameters: { type: "object", properties: { query: { type: "string" } } } }],
    authorizeAndExecute: (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: false, content: { status: "authorization_required" } })),
  };
  const first = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    task,
    await optionsFor(agent, registry, "coding", journal, { run: runOptions }),
  );
  assert.equal(first.run.result.run.status, "approval_required");
  assert.equal(first.run.checkpoint.status, "provider_reconciliation_required");
  assert.equal(first.run.checkpoint.provider_status, "approval_required");
  assert.match(first.run.checkpoint.provider_result_digest, /^[0-9a-f]{64}$/);
  const providerCalls = calls.provider;
  const rawRun = first.run.result.run;
  const values = first.run.result.evidence.runtime.values;
  const tampered = resealCheckpoint(first.run.checkpoint, { provider_result_digest: "a".repeat(64) });
  let rehydratorCalls = 0;
  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, jobId, new InMemoryAutonomousEvidenceBackedCheckpointStore(tampered)).run(
      task,
      await optionsFor(agent, registry, "coding", journal, {
        run: runOptions,
        execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
        rehydrateProviderRun: () => { rehydratorCalls += 1; return rawRun; },
      }),
    ),
    /does not match its checkpoint digest/,
  );
  assert.equal(rehydratorCalls, 1);
  assert.equal(calls.provider, providerCalls);
});

test("a duck-typed agent cannot bypass the built-in resumable transport fence", async () => {
  const { agent, registry, calls } = await setup();
  const task = "Do not infer transport from a completed return value.";
  const options = await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal());
  const completed = await agent.runWithReviewedEvidence(task, options);
  const providerCalls = calls.provider;
  let persisted = null;
  const replayingAgent = {
    runWithReviewedEvidence: async (_task, received) => {
      await received.beforeProviderRun?.({
        executionPlan: completed.execution_plan,
        evidence: completed.evidence,
        promptContext: completed.prompt_context,
      });
      return completed;
    },
  };
  await assert.rejects(
    () => runAutonomousEvidenceBackedResumable(replayingAgent, task, {
      ...options,
      jobId: "provider-no-dispatch-completed-envelope-job",
      checkpointSink: (checkpoint) => { persisted = structuredClone(checkpoint); },
      checkpointCompareAndStore: () => { throw new Error("core rejection must precede checkpoint CAS"); },
      checkpointDispatchCompareAndStore: () => { throw new Error("core rejection must precede dispatch CAS"); },
    }),
    /exact built-in AutonomousAgent/,
  );
  assert.equal(persisted, null);
  assert.equal(calls.provider, providerCalls);
});

test("a crash after provider response leaves in-flight state and restore never redispatches", async () => {
  const state = { current: null, terminalFailurePending: true, reads: 0 };
  const persistence = {
    read: () => {
      state.reads += 1;
      return state.current === null ? null : structuredClone(state.current);
    },
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      if (checkpoint.status === "completed" && state.terminalFailurePending) {
        state.terminalFailurePending = false;
        throw new Error("simulated crash after provider response before terminal commit");
      }
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
  };
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const task = "Recover an uncertain provider boundary without duplicate work.";
  const firstController = new AutonomousEvidenceBackedController(agent, "provider-crash-window-job", persistence);
  const firstOptions = await optionsFor(agent, registry, "coding", journal);
  await assert.rejects(
    () => firstController.run(task, firstOptions),
    /simulated crash/,
  );
  assert.equal(calls.provider, 1);
  assert.equal(state.current.status, "provider_in_flight");
  const inFlightDigest = state.current.checkpoint_digest;
  const readsAfterCrash = state.reads;

  const restored = await firstController.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: {
      rehydrateValue: (receipt) => ({
        domain: "coding",
        requirement: receipt.requirement_id,
        claim: "resumable-transient-claim",
      }),
    },
    resumeProvider: true,
  }));
  assert.ok(state.reads > readsAfterCrash);
  assert.equal(restored.run.status, "provider_reconciliation_required");
  assert.equal(restored.run.checkpoint.status, "provider_reconciliation_required");
  assert.equal(restored.run.checkpoint.previous_checkpoint_digest, inFlightDigest);
  assert.equal(restored.run.checkpoint.provider_result_digest, null);
  assert.equal(restored.run.checkpoint.provider_status, null);
  assert.equal(calls.provider, 1);
});

test("completed inspection is immutable and unknown reconciliation accepts an operation-bound rehydrator", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const task = "Reconcile a caller-owned provider result against the durable operation.";
  const firstStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const firstController = new AutonomousEvidenceBackedController(agent, "provider-rehydration-settlement-job", firstStore);
  const first = await firstController.run(task, await optionsFor(agent, registry, "coding", journal));
  const completed = first.run.checkpoint;
  const values = first.run.result.evidence.runtime.values;
  const rawProviderRun = first.run.result.run;
  const providerCalls = calls.provider;

  const inspectionController = new AutonomousEvidenceBackedController(agent, "provider-rehydration-settlement-job", firstStore);
  const inspected = await inspectionController.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
  }));
  assert.equal(inspected.run.status, "completed");
  assert.equal(inspected.run.provider_rehydrated, false);
  assert.equal(inspected.run.checkpoint.checkpoint_digest, completed.checkpoint_digest);
  assert.equal(inspected.run.checkpoint.generation, completed.generation);
  assert.equal(calls.provider, providerCalls);

  const exactlyRehydrated = await inspectionController.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
    rehydrateProviderRun: () => rawProviderRun,
  }));
  assert.equal(exactlyRehydrated.run.status, "completed");
  assert.equal(exactlyRehydrated.run.provider_rehydrated, true);
  assert.equal(exactlyRehydrated.run.checkpoint.checkpoint_digest, completed.checkpoint_digest);
  assert.equal(exactlyRehydrated.run.checkpoint.generation, completed.generation);
  assert.equal((await firstStore.read()).checkpoint_digest, completed.checkpoint_digest);

  const unknown = resealCheckpoint(completed, {
    status: "provider_reconciliation_required",
    provider_result_digest: null,
    provider_status: null,
  });
  const unknownStore = new InMemoryAutonomousEvidenceBackedCheckpointStore(unknown);
  const reconciliationController = new AutonomousEvidenceBackedController(agent, "provider-rehydration-settlement-job", unknownStore);
  const settled = await reconciliationController.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
    rehydrateProviderRun: (context) => {
      assert.equal(context.providerDispatchCount, unknown.provider_dispatch_count);
      assert.equal(context.providerDispatchHeadDigest, unknown.provider_dispatch_head_digest);
      return rawProviderRun;
    },
  }));
  assert.equal(settled.run.status, "completed");
  assert.equal(settled.run.provider_rehydrated, true);
  assert.equal(settled.run.checkpoint.generation, unknown.generation + 1);
  assert.equal(settled.run.checkpoint.previous_checkpoint_digest, unknown.checkpoint_digest);
  assert.match(settled.run.checkpoint.provider_result_digest, /^[0-9a-f]{64}$/);
  assert.equal(calls.provider, providerCalls);
});

test("provider rehydrators cannot rewrite the checkpoint digest they are checked against", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const jobId = "provider-rehydrator-checkpoint-mutation-job";
  const task = "Keep provider rehydration bound to the durable result digest.";
  const first = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    task,
    await optionsFor(agent, registry, "coding", journal),
  );
  const durableCheckpoint = structuredClone(first.run.checkpoint);
  const values = first.run.result.evidence.runtime.values;
  const altered = structuredClone(first.run.result.run);
  altered.response.text = "different caller-owned provider result";
  const providerCalls = calls.provider;

  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, jobId, store).run(
      task,
      await optionsFor(agent, registry, "coding", journal, {
        execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
        rehydrateProviderRun: async (context) => {
          context.checkpoint.provider_result_digest = digestJsonSync(altered);
          return altered;
        },
      }),
    ),
    /rehydrated provider run does not match its checkpoint digest/,
  );
  assert.deepEqual(await store.read(), durableCheckpoint);
  assert.equal(calls.provider, providerCalls);
});

test("rehydrated provider results reject getters, hidden fields, and inherited response state", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const jobId = "provider-rehydration-json-graph-job";
  const task = "Reject provider result shapes whose digest differs from consumed state.";
  const first = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    task,
    await optionsFor(agent, registry, "coding", journal),
  );
  const completedDigest = first.run.checkpoint.checkpoint_digest;
  const values = first.run.result.evidence.runtime.values;
  const providerCalls = calls.provider;
  const common = {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
  };

  let schemaGetterCalls = 0;
  const getterResult = structuredClone(first.run.result.run);
  const schema = getterResult.schema;
  delete getterResult.schema;
  Object.defineProperty(getterResult, "schema", {
    enumerable: true,
    get: () => {
      schemaGetterCalls += 1;
      return schemaGetterCalls % 2 ? schema : "changed";
    },
  });
  const getterOptions = await optionsForResult(common, getterResult);
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, jobId, store).run(task, getterOptions),
    /accessor-backed provider data/,
  );
  assert.equal(schemaGetterCalls, 0);

  const hiddenResult = structuredClone(first.run.result.run);
  Object.defineProperty(hiddenResult.response, "hidden_transport_state", { value: "not-digested", enumerable: false });
  const hiddenOptions = await optionsForResult(common, hiddenResult);
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, jobId, store).run(task, hiddenOptions),
    /hidden or accessor-backed provider data/,
  );

  const inheritedResult = structuredClone(first.run.result.run);
  Object.setPrototypeOf(inheritedResult.response, { inherited_transport_state: "not-digested" });
  const inheritedOptions = await optionsForResult(common, inheritedResult);
  await assert.rejects(
    () => new AutonomousEvidenceBackedController(agent, jobId, store).run(task, inheritedOptions),
    /inherited or branded provider data/,
  );
  assert.equal((await store.read()).checkpoint_digest, completedDigest);
  assert.equal(calls.provider, providerCalls);

  function optionsForResult(overrides, result) {
    return optionsFor(agent, registry, "coding", journal, {
      ...overrides,
      rehydrateProviderRun: () => result,
    });
  }
});

test("restored attempted states require CAS before probing or invoking a supplied provider rehydrator", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const task = "Require an atomic successor before provider settlement rehydration.";
  const jobId = "provider-rehydration-cas-preflight-job";
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const first = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    task,
    await optionsFor(agent, registry, "coding", journal),
  );
  const completed = first.run.checkpoint;
  const unknown = resealCheckpoint(completed, {
    status: "provider_reconciliation_required",
    provider_result_digest: null,
    provider_status: null,
  });
  const values = first.run.result.evidence.runtime.values;
  const evidenceCalls = calls.evidence;
  const providerCalls = calls.provider;

  for (const checkpoint of [completed, unknown]) {
    let rehydratorCalls = 0;
    let sinkCalls = 0;
    const restore = await optionsFor(agent, registry, "coding", journal, {
      execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
      rehydrateProviderRun: () => {
        rehydratorCalls += 1;
        return first.run.result.run;
      },
    });
    await assert.rejects(
      () => runAutonomousEvidenceBackedResumable(agent, task, {
        ...restore,
        jobId,
        checkpoint,
        checkpointSink: () => { sinkCalls += 1; },
      }),
      /requires atomic checkpointCompareAndStore persistence before rehydration/,
    );
    assert.equal(rehydratorCalls, 0);
    assert.equal(sinkCalls, 0);
    assert.equal(calls.evidence, evidenceCalls);
    assert.equal(calls.provider, providerCalls);
  }
});

test("legacy, impossible, and self-consistently tampered checkpoints fail closed", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const task = "Reject a forged durable provider operation.";
  const checkpointStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "provider-tamper-job", checkpointStore);
  const first = await controller.run(task, await optionsFor(agent, registry, "coding", journal));
  const completed = first.run.checkpoint;
  const values = first.run.result.evidence.runtime.values;
  const providerCalls = calls.provider;

  const legacy = resealCheckpoint(completed, { schema: "bioprism-typescript-autonomous-evidence-backed-checkpoint/0.1" });
  await assert.rejects(validateAutonomousEvidenceBackedCheckpoint(legacy), /schema/);
  const legacyDispatchless = resealCheckpoint(completed, { schema: "bioprism-typescript-autonomous-evidence-backed-checkpoint/0.2" });
  await assert.rejects(validateAutonomousEvidenceBackedCheckpoint(legacyDispatchless), /schema/);

  const impossibleTerminal = resealCheckpoint(completed, {
    generation: 1,
    previous_checkpoint_digest: null,
  });
  await assert.rejects(validateAutonomousEvidenceBackedCheckpoint(impossibleTerminal), /must succeed an in-flight/);
  const impossibleReconciliation = resealCheckpoint(completed, {
    status: "provider_reconciliation_required",
    generation: 1,
    previous_checkpoint_digest: null,
  });
  await assert.rejects(validateAutonomousEvidenceBackedCheckpoint(impossibleReconciliation), /must succeed an in-flight/);

  const pendingStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const pendingController = new AutonomousEvidenceBackedController(agent, "provider-state-matrix-job", pendingStore);
  const pending = (await pendingController.run(
    "Create a safe pending checkpoint.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), { run: { approveProviderCall: false } }),
  )).run.checkpoint;
  const digest = "a".repeat(64);
  const invalidStates = [
    resealCheckpoint(pending, { provider_operation_digest: digest }),
    resealCheckpoint(pending, { status: "provider_in_flight", provider_operation_digest: digest, provider_result_digest: digest, provider_status: "completed" }),
    resealCheckpoint(pending, { status: "provider_in_flight", provider_operation_digest: digest, evidence_result_digest: null }),
    resealCheckpoint(pending, { generation: 2, previous_checkpoint_digest: null }),
  ];
  for (const invalid of invalidStates) await assert.rejects(validateAutonomousEvidenceBackedCheckpoint(invalid));

  const tampered = resealCheckpoint(completed, { provider_operation_digest: "b".repeat(64) });
  const tamperedStore = new InMemoryAutonomousEvidenceBackedCheckpointStore(tampered);
  const tamperedController = new AutonomousEvidenceBackedController(agent, "provider-tamper-job", tamperedStore);
  let rehydratorCalls = 0;
  const tamperedOptions = await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
    rehydrateProviderRun: () => {
      rehydratorCalls += 1;
      return first.run.result.run;
    },
  });
  await assert.rejects(
    () => tamperedController.run(task, tamperedOptions),
    /provider operation/,
  );
  assert.equal(rehydratorCalls, 0);
  assert.equal(calls.provider, providerCalls);
});

test("autonomous attempt keys bind the exact rendered request even under one stable root", async () => {
  const requests = [];
  const { agent } = await setup({
    onProviderRequest: (init) => {
      requests.push({
        key: new Headers(init.headers).get("Idempotency-Key"),
        body: JSON.parse(String(init.body)),
      });
    },
  });
  let variant = "alpha";
  const promptTemplate = new AutonomousPromptTemplate({
    promptId: "mutable-renderer-attempt-key",
    version: "1",
    domain: "coding",
    capabilities: ["code"],
    stages: ["answer"],
    templateDigest: digestJsonSync({ stable_manifest: true }),
    render: () => [{ role: "user", content: `renderer-${variant}` }],
  });
  const invoke = () => agent.run("Bind the transient rendered request.", {
    domain: "coding",
    candidates: [model()],
    promptTemplate,
    approveProviderCall: true,
    providerIdempotencyKey: "stable-operation-root",
  });
  await invoke();
  await invoke();
  variant = "beta";
  await invoke();
  assert.equal(requests.length, 3);
  assert.match(requests[0].key, /^[0-9a-f]{64}$/);
  assert.equal(requests[0].key, requests[1].key);
  assert.deepEqual(requests[0].body, requests[1].body);
  assert.notEqual(requests[2].key, requests[0].key);
  assert.notDeepEqual(requests[2].body, requests[0].body);
  assert.ok(requests.every(({ key }) => key !== "stable-operation-root"));
});

test("operation-derived idempotency reaches automatic planning and cross-domain fan-out", async () => {
  {
    const { state, store } = recordingCheckpointStore();
    const { agent, registry, calls } = await setup();
    const controller = new AutonomousEvidenceBackedController(agent, "automatic-idempotency-job", store);
    await controller.run(
      "Run one deterministic automatic provider operation.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), { runMode: "auto" }),
    );
    const operationDigest = state.transitions[0].checkpoint.provider_operation_digest;
    const rootKey = digestJsonSync({ schema: "bioprism-typescript-autonomous-evidence-backed-provider-idempotency/0.1", provider_operation_digest: operationDigest });
    assert.equal(calls.providerIdempotencyKeys.length, 1);
    assert.match(calls.providerIdempotencyKeys[0], /^[0-9a-f]{64}$/);
    assert.notEqual(calls.providerIdempotencyKeys[0], rootKey);
  }

  {
    const { state, store } = recordingCheckpointStore();
    const { agent, registry, calls } = await setup();
    const controller = new AutonomousEvidenceBackedController(agent, "automatic-unapproved-planning-job", store);
    const result = await controller.run(
      "Keep unapproved provider-assisted planning safely pending.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        runMode: "auto",
        run: {
          planningMode: "provider",
          planning: { approveProviderCall: false, candidates: [model()] },
        },
      }),
    );
    assert.equal(result.run.status, "provider_pending");
    assert.equal(result.run.checkpoint.status, "provider_pending");
    assert.equal(state.transitions.length, 1);
    assert.equal(state.transitions[0].checkpoint.status, "provider_pending");
    assert.equal(state.transitions.some((transition) => transition.checkpoint.status === "provider_in_flight"), false);
    assert.equal(calls.provider, 0);
  }

  {
    const { state, store } = recordingCheckpointStore();
    const { agent, registry, calls } = await setup();
    const controller = new AutonomousEvidenceBackedController(agent, "automatic-planning-idempotency-job", store);
    const result = await controller.run(
      "Fence provider-assisted automatic planning.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        runMode: "auto",
        run: {
          planningMode: "provider",
          planning: { approveProviderCall: true, candidates: [model()] },
          acceptPlan: false,
        },
      }),
    );
    assert.equal(result.run.status, "provider_reconciliation_required");
    const operationDigest = state.transitions[0].checkpoint.provider_operation_digest;
    const rootKey = digestJsonSync({ schema: "bioprism-typescript-autonomous-evidence-backed-provider-idempotency/0.1", provider_operation_digest: operationDigest });
    const planningKey = digestJsonSync({
      schema: "bioprism-typescript-autonomous-provider-idempotency-scope/0.1",
      provider_idempotency_key: rootKey,
      scope: { phase: "planning" },
    });
    assert.equal(calls.providerIdempotencyKeys.length, 1);
    assert.match(calls.providerIdempotencyKeys[0], /^[0-9a-f]{64}$/);
    assert.notEqual(calls.providerIdempotencyKeys[0], planningKey);
  }

  {
    const { state, store } = recordingCheckpointStore();
    const { agent, registry, calls } = await setup();
    const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
    const plan = await agent.evidencePlan(["coding", "data"]);
    const requests = plan.requirements.map((requirement, index) => ({
      requirement_id: requirement.requirement_id,
      source_id: `cross-idempotency-source-${index}`,
      request_id: `cross-idempotency-request-${index}`,
      metadata: {},
    }));
    const controller = new AutonomousEvidenceBackedController(agent, "cross-idempotency-job", store);
    await controller.run("Fence every cross-domain provider request.", {
      registry,
      domains: ["coding", "data"],
      requests,
      prepare: { readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }), allowDegradedDispatch: true },
      execute: {
        approveSourceDispatch: true,
        journal,
        projector: { project: (_value, context) => [{ label: context.requirement.requirement_id, kind: "fact", status: "observed" }] },
        evaluator: {
          evaluator_id: "cross-idempotency-evaluator",
          evaluator_version: "1",
          evaluate: ({ requirement }) => ({ evaluator_id: "cross-idempotency-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
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
      run: { candidates: [model()], approveProviderCall: true },
      resumablePolicyIdentity: {
        projector: { id: "cross-idempotency-projector", version: "1" },
        provider_policy: { id: "cross-idempotency-provider-policy", version: "1", config_digest: PROVIDER_POLICY_CONFIG_DIGEST },
      },
    });
    const operationDigest = state.transitions[0].checkpoint.provider_operation_digest;
    const rootKey = digestJsonSync({ schema: "bioprism-typescript-autonomous-evidence-backed-provider-idempotency/0.1", provider_operation_digest: operationDigest });
    const expected = [
      ["cross-coding", 0],
      ["cross-data", 1],
    ].map(([childId, childIndex]) => digestJsonSync({
      schema: "bioprism-typescript-autonomous-provider-idempotency-scope/0.1",
      provider_idempotency_key: rootKey,
      scope: { phase: "cross_domain_child", child_id: childId, child_index: childIndex },
    }));
    expected.push(digestJsonSync({
      schema: "bioprism-typescript-autonomous-provider-idempotency-scope/0.1",
      provider_idempotency_key: rootKey,
      scope: { phase: "cross_domain_synthesis" },
    }));
    assert.ok(calls.providerIdempotencyKeys.every((key) => typeof key === "string" && /^[0-9a-f]{64}$/.test(key)));
    assert.ok(calls.providerIdempotencyKeys.every((key) => !expected.includes(key)));
    assert.equal(new Set(calls.providerIdempotencyKeys).size, 3);
    assert.deepEqual(state.dispatchReceipts.map(({ projection }) => projection.sequence), [1, 2, 3]);
    assert.deepEqual(
      state.dispatchReceipts.slice(1).map(({ projection }) => projection.previous_receipt_digest),
      state.dispatchReceipts.slice(0, -1).map(({ projection }) => projection.receipt_digest),
    );
    assert.deepEqual(
      [...state.dispatchReceipts.map(({ providerIdempotencyKey }) => providerIdempotencyKey)].sort(),
      [...calls.providerIdempotencyKeys].sort(),
    );
  }
});

test("cross-domain children_completed is a normalized resumable terminal without synthesis", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const plan = await agent.evidencePlan(["coding", "data"]);
  const requests = plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `children-only-source-${index}`,
    request_id: `children-only-request-${index}`,
    metadata: {},
  }));
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const jobId = "cross-domain-children-only-job";
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
        evaluator_id: "children-only-evaluator",
        evaluator_version: "1",
        evaluate: ({ requirement }) => ({ evaluator_id: "children-only-evaluator", evaluator_version: "1", verdict: "accepted", score: 1, evidence_digest: requirement.workflow_digest }),
      },
    },
    runMode: "cross_domain",
    crossDomain: {
      subtasks: [
        { id: "children-coding", domain: "coding", task: "Review coding evidence." },
        { id: "children-data", domain: "data", task: "Review data evidence." },
      ],
      synthesize: false,
      maxParallelChildren: 2,
    },
    run: { candidates: [model()], approveProviderCall: true },
    resumablePolicyIdentity: {
      projector: { id: "children-only-projector", version: "1" },
      value_rehydrator: { id: "children-only-value-rehydrator", version: "1" },
      provider_policy: { id: "children-only-provider-policy", version: "1", config_digest: PROVIDER_POLICY_CONFIG_DIGEST },
    },
  };
  const first = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    "Complete reviewed children without synthesis.",
    common,
  );
  assert.equal(first.run.result.cross_domain_run.status, "children_completed");
  assert.equal(first.run.checkpoint.status, "completed");
  assert.equal(first.run.checkpoint.provider_status, "completed");
  const providerCalls = calls.provider;
  const values = first.run.result.evidence.runtime.values;
  const rawCrossDomainRun = first.run.result.cross_domain_run;
  const restored = await new AutonomousEvidenceBackedController(agent, jobId, store).run(
    "Complete reviewed children without synthesis.",
    {
      ...common,
      execute: {
        ...common.execute,
        rehydrateValue: (receipt) => values[receipt.request_digest] ?? null,
      },
      rehydrateCrossDomainRun: () => rawCrossDomainRun,
    },
  );
  assert.equal(restored.run.status, "completed");
  assert.equal(restored.run.result.cross_domain_run.status, "children_completed");
  assert.equal(restored.run.checkpoint.status, "completed");
  assert.equal(restored.run.checkpoint.provider_status, "completed");
  assert.equal(restored.run.provider_rehydrated, true);
  assert.equal(calls.provider, providerCalls);
});

test("provider tool-loop turns derive distinct retry-stable keys from one operation root", async () => {
  const invoke = async () => {
    const keys = [];
    let calls = 0;
    const runtime = new LLMRuntime({
      credentials: new CredentialStore(),
      fetch: async (_url, init) => {
        keys.push(new Headers(init.headers).get("Idempotency-Key"));
        calls += 1;
        if (calls === 1) {
          return jsonResponse({
            choices: [{
              message: {
                role: "assistant",
                content: "",
                tool_calls: [{ id: "call-1", type: "function", function: { name: "lookup", arguments: "{\"query\":\"safe\"}" } }],
              },
              finish_reason: "tool_calls",
            }],
          });
        }
        return jsonResponse({ choices: [{ message: { role: "assistant", content: "done" }, finish_reason: "stop" }] });
      },
    });
    runtime.registerProvider(openaiCompatibleProvider("turn-key-provider", "https://turn-key-provider.test", { requiresCredential: false }));
    const result = await runtime.invokeToolLoop("turn-key-provider", {
      model: "turn-key-model",
      messages: [{ role: "user", content: "Use the lookup once." }],
      maxOutputTokens: 128,
      idempotencyKey: "operation-root-key",
      tools: [{ name: "lookup", description: "Look up a safe value.", parameters: { type: "object" } }],
      toolChoice: "auto",
    }, {
      authorizeAndExecute: (toolCalls) => toolCalls.map((call) => ({ callId: call.id, approved: true, content: { ok: true } })),
      maxTurns: 3,
    });
    assert.equal(result.status, "completed");
    return keys;
  };

  const first = await invoke();
  const second = await invoke();
  assert.equal(first.length, 2);
  assert.ok(first.every((key) => typeof key === "string" && /^[0-9a-f]{64}$/.test(key)));
  assert.notEqual(first[0], first[1]);
  assert.deepEqual(first, second);
});

test("dispatch acknowledgement cannot revoke the selected credential behind its private receipt", async () => {
  const state = { current: null };
  const { agent, registry, calls } = await setup({
    providerOptions: { requiresCredential: true },
  });
  const handle = agent.llm.credentials.register("resumable-provider", "credential-bound-to-dispatch");
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      agent.llm.credentials.revoke(handle);
      return true;
    },
  };
  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, "post-dispatch-credential-revoke-job", persistence).run(
      "Refuse transport after dispatch persistence revokes its selected credential.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: { credential: handle },
      }),
    ),
    /selected provider transport binding does not match/,
  );
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
});

test("provider outcome observers cannot mutate the transport graph before terminal settlement", async () => {
  const { state, store } = recordingCheckpointStore();
  const { agent, registry, calls } = await setup();
  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, "post-response-provider-graph-job", store).run(
      "Refuse terminal settlement after an outcome observer changes provider state.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: {
          observer: {
            after: () => {
              agent.llm.registerProvider(openaiCompatibleProvider(
                "resumable-provider",
                "https://post-response-swap.test",
                { requiresCredential: false },
              ));
            },
          },
        },
      }),
    ),
    /provider registry changed after its policy snapshot/,
  );
  assert.equal(calls.provider, 1);
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(state.transitions.length, 1);
});

test("terminal persistence and controller state ignore a replaced global structuredClone", async () => {
  const nativeClone = globalThis.structuredClone;
  const state = { current: null };
  let retainedTerminalArgument = null;
  const { agent, registry, calls } = await setup();
  const persistence = {
    read: () => state.current === null ? null : nativeClone(state.current),
    write: (checkpoint) => { state.current = nativeClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      if (checkpoint.status === "completed") retainedTerminalArgument = checkpoint;
      state.current = nativeClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = nativeClone(checkpoint);
      return true;
    },
  };
  const controller = new AutonomousEvidenceBackedController(agent, "captured-structured-clone-job", persistence);
  let completed;
  try {
    completed = await controller.run(
      "Keep authoritative settlement detached from caller-retained persistence arguments.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: {
          observer: {
            after: () => { globalThis.structuredClone = (value) => value; },
          },
        },
      }),
    );
  } finally {
    globalThis.structuredClone = nativeClone;
  }

  assert.equal(completed.run.status, "completed");
  assert.equal(calls.provider, 1);
  assert.ok(retainedTerminalArgument);
  const settledDigest = completed.run.checkpoint.checkpoint_digest;
  retainedTerminalArgument.status = "evidence_incomplete";
  retainedTerminalArgument.generation = 999;
  retainedTerminalArgument.checkpoint_digest = "0".repeat(64);
  assert.equal(completed.run.checkpoint.status, "completed");
  assert.notEqual(completed.run.checkpoint.generation, 999);
  assert.equal(completed.run.checkpoint.checkpoint_digest, settledDigest);
  assert.equal(controller.projection().checkpoint_digest, settledDigest);
  assert.equal(state.current.status, "completed");
  assert.equal(state.current.checkpoint_digest, settledDigest);
});

test("provider rehydrators receive detached value projections even when they return no result", async () => {
  const { agent, registry, calls } = await setup();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const controller = new AutonomousEvidenceBackedController(agent, "detached-rehydrator-context-job", store);
  const task = "Keep restored probe state private from a caller-owned rehydrator.";
  const first = await controller.run(task, await optionsFor(agent, registry, "coding", journal));
  const durableCheckpoint = structuredClone(first.run.checkpoint);
  const values = first.run.result.evidence.runtime.values;
  const providerCalls = calls.provider;
  let rehydratorCalls = 0;

  const restored = await controller.run(task, await optionsFor(agent, registry, "coding", journal, {
    execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
    rehydrateProviderRun: (context) => {
      rehydratorCalls += 1;
      context.executionPlan.domains[0] = "data";
      context.evidence.status = "blocked";
      context.promptContext[0].content = "caller mutation must remain detached";
      return null;
    },
  }));

  assert.equal(rehydratorCalls, 1);
  assert.equal(restored.run.status, "completed");
  assert.equal(restored.run.provider_rehydrated, false);
  assert.deepEqual(restored.run.result.execution_plan.domains, ["coding"]);
  assert.equal(restored.run.result.evidence.status, "completed");
  assert.notEqual(restored.run.result.prompt_context[0].content, "caller mutation must remain detached");
  assert.deepEqual(await store.read(), durableCheckpoint);
  assert.equal(calls.provider, providerCalls);
});

test("dispatch callbacks cannot poison the private receipt method surface", async () => {
  const state = { current: null, prototypeWrites: [] };
  let retainedReceipt = null;
  const { agent, registry, calls } = await setup();
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint, receipt) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      retainedReceipt = receipt;
      const prototype = Object.getPrototypeOf(receipt);
      state.prototypeWrites.push(Reflect.set(prototype, "providerIdempotencyKey", () => "forged-key"));
      state.prototypeWrites.push(Reflect.set(prototype, "toJSON", () => ({ forged: true })));
      return true;
    },
  };
  const result = await new AutonomousEvidenceBackedController(
    agent,
    "private-receipt-prototype-job",
    persistence,
  ).run(
    "Keep the private dispatch receipt behavior immutable across storage callbacks.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );

  assert.equal(result.run.status, "completed");
  assert.deepEqual(state.prototypeWrites, [false, false]);
  assert.equal(retainedReceipt.providerIdempotencyKey(), calls.providerIdempotencyKeys[0]);
  assert.match(retainedReceipt.toJSON().receipt_digest, /^[0-9a-f]{64}$/);
  assert.equal(calls.provider, 1);
});

test("in-memory receipt projections follow only the selected job chain", async () => {
  const { agent, registry } = await setup();
  const sharedStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const first = await new AutonomousEvidenceBackedController(agent, "receipt-chain-first-job", sharedStore).run(
    "Complete the first independently retained provider operation.",
    await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal()),
  );
  const firstHead = first.run.checkpoint.provider_dispatch_head_digest;
  assert.ok(firstHead);

  const secondJournal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const pendingStore = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const pending = await new AutonomousEvidenceBackedController(agent, "receipt-chain-second-job", pendingStore).run(
    "Complete the second independently retained provider operation.",
    await optionsFor(agent, registry, "coding", secondJournal, { run: { approveProviderCall: false } }),
  );
  const values = pending.run.result.evidence.runtime.values;
  await sharedStore.write(pending.run.checkpoint);
  const second = await new AutonomousEvidenceBackedController(agent, "receipt-chain-second-job", sharedStore).run(
    "Complete the second independently retained provider operation.",
    await optionsFor(agent, registry, "coding", secondJournal, {
      execute: { rehydrateValue: (receipt) => values[receipt.request_digest] ?? null },
      resumeProvider: true,
    }),
  );
  const secondHead = second.run.checkpoint.provider_dispatch_head_digest;
  assert.ok(secondHead);
  assert.notEqual(secondHead, firstHead);

  const selected = sharedStore.providerDispatchReceiptProjections();
  const historical = sharedStore.providerDispatchReceiptProjections(firstHead);
  assert.equal(selected.length, 1);
  assert.equal(selected[0].job_id, "receipt-chain-second-job");
  assert.equal(selected[0].receipt_digest, secondHead);
  assert.equal(historical.length, 1);
  assert.equal(historical[0].job_id, "receipt-chain-first-job");
  assert.equal(historical[0].receipt_digest, firstHead);
});

test("credential probes use captured map intrinsics after dispatch acknowledgement", async () => {
  const state = { current: null };
  const { agent, registry, calls } = await setup({ providerOptions: { requiresCredential: true } });
  const store = agent.llm.credentials;
  const entries = store.entries;
  const handle = store.register("resumable-provider", "map-intrinsic-bound-secret");
  const nativeGet = Map.prototype.get;
  const nativeDelete = Map.prototype.delete;
  const savedEntry = nativeGet.call(entries, handle);
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      nativeDelete.call(entries, handle);
      Map.prototype.get = function (key) {
        if (this === entries && key === handle) return savedEntry;
        return nativeGet.call(this, key);
      };
      return true;
    },
  };
  try {
    await assert.rejects(
      async () => new AutonomousEvidenceBackedController(agent, "credential-map-intrinsic-job", persistence).run(
        "Reject a revoked credential even when Map.prototype.get lies about it.",
        await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
          run: { credential: handle },
        }),
      ),
      /selected provider transport binding does not match/,
    );
  } finally {
    Map.prototype.get = nativeGet;
  }
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
  assert.throws(() => store.resolve(handle, "resumable-provider"), /revoked or unknown/);
});

test("credential probes recheck state after a caller-owned expiry clock runs", async () => {
  let armed = false;
  let selectedHandle = null;
  let entries = null;
  const nativeDelete = Map.prototype.delete;
  const credentialStore = new CredentialStore({
    clock: () => {
      if (armed && entries && selectedHandle) nativeDelete.call(entries, selectedHandle);
      return 1_000;
    },
  });
  const handle = credentialStore.register("resumable-provider", "clock-bound-secret", { ttlMs: 10_000 });
  selectedHandle = handle;
  entries = credentialStore.entries;
  const state = { current: null };
  const { agent, registry, calls } = await setup({
    credentialStore,
    providerOptions: { requiresCredential: true },
  });
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      armed = true;
      return true;
    },
  };
  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, "credential-clock-side-effect-job", persistence).run(
      "Reject credential state changed from inside its expiry clock.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: { credential: handle },
      }),
    ),
    /selected provider transport binding does not match/,
  );
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
});

test("credential probes re-read store bindings after a caller-owned expiry clock runs", async () => {
  let armed = false;
  let credentialStore = null;
  const clock = () => {
    if (armed) {
      Object.defineProperty(credentialStore, "entries", {
        configurable: true,
        value: new Map(),
        writable: true,
      });
    }
    return 1_000;
  };
  credentialStore = new CredentialStore({ clock });
  const handle = credentialStore.register("resumable-provider", "store-binding-secret", { ttlMs: 10_000 });
  const state = { current: null };
  const { agent, registry, calls } = await setup({
    credentialStore,
    providerOptions: { requiresCredential: true },
  });
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      armed = true;
      return true;
    },
  };
  await assert.rejects(
    async () => new AutonomousEvidenceBackedController(agent, "credential-store-rebind-job", persistence).run(
      "Reject an expiry clock that swaps the selected credential store state.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: { credential: handle },
      }),
    ),
    /selected provider transport binding does not match/,
  );
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
});

test("post-ack credential clocks cannot hide caller cancellation through AbortController prototypes", async () => {
  const signalDescriptor = Object.getOwnPropertyDescriptor(AbortController.prototype, "signal");
  assert.equal(typeof signalDescriptor?.get, "function");
  const callerController = new AbortController();
  const callerSignal = callerController.signal;
  const decoySignal = new AbortController().signal;
  let armed = false;
  const credentialStore = new CredentialStore({
    clock: () => {
      if (armed) {
        callerController.abort();
        Object.defineProperty(AbortController.prototype, "signal", {
          ...signalDescriptor,
          get: () => decoySignal,
        });
      }
      return 1_000;
    },
  });
  const handle = credentialStore.register("resumable-provider", "abort-fence-secret", { ttlMs: 10_000 });
  const state = { current: null };
  const { agent, registry, calls } = await setup({
    credentialStore,
    providerOptions: { requiresCredential: true },
  });
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      armed = true;
      return true;
    },
  };
  try {
    await assert.rejects(
      async () => new AutonomousEvidenceBackedController(agent, "post-ack-abort-fence-job", persistence).run(
        "Honor caller cancellation even when an expiry clock changes abort prototypes.",
        await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
          run: { credential: handle, signal: callerSignal },
        }),
      ),
      /aborted/,
    );
  } finally {
    Object.defineProperty(AbortController.prototype, "signal", signalDescriptor);
  }
  assert.equal(callerSignal.aborted, true);
  assert.equal(state.current.status, "provider_in_flight");
  assert.equal(calls.provider, 0);
});

test("ordinary observers cannot skip the private dispatch transaction through Promise.prototype", async () => {
  const nativeThen = Promise.prototype.then;
  const state = { current: null, dispatchCommits: 0 };
  const { agent, registry, calls } = await setup();
  const persistence = {
    read: () => state.current === null ? null : structuredClone(state.current),
    write: (checkpoint) => { state.current = structuredClone(checkpoint); },
    writeIfUnchanged: (expected, checkpoint) => {
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
    writeDispatchIfUnchanged: (expected, checkpoint) => {
      Promise.prototype.then = nativeThen;
      state.dispatchCommits += 1;
      if ((state.current?.checkpoint_digest ?? null) !== expected) return false;
      state.current = structuredClone(checkpoint);
      return true;
    },
  };
  try {
    const result = await new AutonomousEvidenceBackedController(
      agent,
      "promise-intrinsic-dispatch-job",
      persistence,
    ).run(
      "Run the private transaction even if an observer replaces Promise.prototype.then.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: {
          observer: {
            dispatch: () => {
              Promise.prototype.then = function (onFulfilled, onRejected) {
                if (typeof onFulfilled === "function"
                    && Function.prototype.toString.call(onFulfilled).includes("providerDispatchFailure")) {
                  Promise.prototype.then = nativeThen;
                  return Promise.resolve();
                }
                return nativeThen.call(this, onFulfilled, onRejected);
              };
            },
          },
        },
      }),
    );
    assert.equal(result.run.status, "completed");
  } finally {
    Promise.prototype.then = nativeThen;
  }
  assert.equal(state.dispatchCommits, 1);
  assert.equal(calls.provider, 1);
  assert.equal(state.current.status, "completed");
});

test("in-memory dispatch retention ignores a poisoned Map.prototype.set", async () => {
  const nativeSet = Map.prototype.set;
  let poisonedCalls = 0;
  const store = new InMemoryAutonomousEvidenceBackedCheckpointStore();
  const { agent, registry, calls } = await setup({
    onProviderRequest: () => { Map.prototype.set = nativeSet; },
  });
  try {
    const result = await new AutonomousEvidenceBackedController(
      agent,
      "receipt-map-intrinsic-job",
      store,
    ).run(
      "Retain the dispatch receipt with a captured collection intrinsic.",
      await optionsFor(agent, registry, "coding", new InMemoryAutonomousEvidenceRuntimeJournal(), {
        run: {
          observer: {
            dispatch: () => {
              Map.prototype.set = function (key, value) {
                if (typeof key === "string" && value?.toJSON?.().schema === "bioprism-typescript-autonomous-evidence-backed-provider-dispatch-receipt/0.1") {
                  poisonedCalls += 1;
                  throw new Error("poisoned receipt map set");
                }
                return nativeSet.call(this, key, value);
              };
            },
          },
        },
      }),
    );
    const head = result.run.checkpoint.provider_dispatch_head_digest;
    assert.ok(head);
    assert.equal(store.providerDispatchReceipt(head)?.toJSON().receipt_digest, head);
  } finally {
    Map.prototype.set = nativeSet;
  }
  assert.equal(poisonedCalls, 0);
  assert.equal(calls.provider, 1);
});
