import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceExecutionResumableController,
  AutonomousEvidenceReadinessPolicy,
  createAutonomousEvidenceExecutionReconciliationReceipt,
  CredentialStore,
  digestJsonSync,
  InMemoryAutonomousEvidenceExecutionCheckpointStore,
  InMemoryAutonomousEvidenceRuntimeJournal,
  JsonAutonomousEvidenceExecutionCheckpointStore,
  LLMRuntime,
  TransactionalJsonAutonomousEvidenceExecutionCheckpointStore,
  WebStorageAutonomousEvidenceExecutionCheckpointTextStore,
  validateAutonomousEvidenceExecutionCheckpoint,
  validateAutonomousEvidenceExecutionReconciliationReceipt,
} from "../dist/index.js";

function planAgent() {
  return new AutonomousAgent(new LLMRuntime({ credentials: new CredentialStore() }));
}

function registerAllDomains(registry, calls) {
  registry.register({
    adapterId: "resumable.all-domains",
    version: "1.0.0",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: async () => {
      calls.count += 1;
      const value = { transient_source_value: calls.count };
      calls.values.set(digestJsonSync(value), value);
      return value;
    },
  });
}

function executionOptions() {
  return {
    sleep: async () => {},
    projector: {
      project: (_value, context) => [{
        label: context.requirement.label,
        kind: "fact",
        status: "observed",
        value_digest: null,
        source_digest: context.request.source_digest ?? null,
      }],
    },
    evaluator: {
      evaluator_id: "resumable-evaluator",
      evaluator_version: "1.0.0",
      evaluate: () => ({
        evaluator_id: "resumable-evaluator",
        evaluator_version: "1.0.0",
        verdict: "accepted",
        score: 1,
        evidence_digest: "d".repeat(64),
      }),
    },
    executionPolicyIdentity: {
      projector: { id: "resumable-projector", version: "1.0.0" },
      evaluator: { id: "resumable-evaluator", version: "1.0.0" },
      journal: { id: "resumable-runtime-journal", version: "1.0.0" },
      value_rehydrator: { id: "resumable-value-rehydrator", version: "1.0.0" },
      sleeper: { id: "resumable-no-delay-sleeper", version: "1.0.0" },
    },
  };
}

const RECONCILIATION_CONTROLLER_OPTIONS = {
  reconciliationAuthority: { id: "source-audit", version: "1", config_digest: "6".repeat(64) },
};

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
      throw new Error("source acknowledgement lost after durable journal append");
    }
    return persisted;
  }
}

async function uncertainDispatchFixture(jobId, controllerOptions = RECONCILIATION_CONTROLLER_OPTIONS) {
  const calls = { count: 0, values: new Map() };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(registry, calls);
  const evidencePlan = await planAgent().evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `reconciliation-source-${index}`,
    source_digest: "9".repeat(64),
    request_id: `reconciliation-request-${index}`,
    metadata: { operation: "observe" },
  }));
  requests.push({
    ...requests[0],
    source_id: "resumable-source-secondary",
    request_id: "resumable-request-secondary",
  });
  const controller = new AutonomousEvidenceExecutionController(registry);
  const executionPlan = await controller.prepare(evidencePlan, {
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
  });
  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const journal = new AcknowledgementLostJournal();
  const worker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, jobId, controllerOptions);
  await assert.rejects(
    worker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true }),
    /acknowledgement lost/,
  );
  const checkpoint = checkpointStore.read();
  assert.equal(checkpoint?.status, "reconciliation_required");
  assert.equal(calls.count, 1);
  const [succeeded] = await journal.records();
  assert.ok(succeeded);
  return { calls, controller, evidencePlan, executionPlan, requests, checkpointStore, journal, checkpoint, succeeded, jobId };
}

function reconciliationDecisions(fixture, outcomeForIndex) {
  return fixture.requests.map((_request, index) => {
    const outcome = outcomeForIndex(index);
    return {
      outcome,
      evidenceDigest: digestJsonSync({ authority: "caller-owned-reconciliation-probe", index, outcome }),
      evidenceKind: "source_dispatch_audit",
      effectAbsent: outcome === "not_executed",
      ...(outcome === "succeeded" ? { succeededReceiptDigest: fixture.succeeded.receipt.receipt_digest } : {}),
    };
  });
}

test("restart-safe evidence execution fences approval, readiness, settlement, and replay across all domains", async () => {
  const calls = { count: 0, values: new Map() };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(registry, calls);
  const agent = planAgent();
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `resumable-source-${index}`,
    source_digest: "c".repeat(64),
    request_id: `resumable-request-${index}`,
    metadata: { operation: "observe" },
  }));
  const controller = new AutonomousEvidenceExecutionController(registry);
  const executionPlan = await controller.prepare(evidencePlan, {
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
  });
  assert.equal(executionPlan.status, "ready_for_review");

  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "duplicate-request-job",
    ).run(executionPlan, evidencePlan, [requests[0], structuredClone(requests[0])], { ...executionOptions(), journal }),
    /requests contain duplicates/,
  );
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "credential-metadata-job",
    ).run(executionPlan, evidencePlan, [{ ...requests[0], metadata: { apiKey: "low-entropy-secret" } }], { ...executionOptions(), journal }),
    /credential-shaped metadata/,
  );
  const unidentifiedProjector = executionOptions();
  delete unidentifiedProjector.executionPolicyIdentity.projector;
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "unidentified-projector-job",
    ).run(executionPlan, evidencePlan, requests, { ...unidentifiedProjector, journal, approveSourceDispatch: true }),
    /requires projector identity/,
  );
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "missing-runtime-journal-job",
      RECONCILIATION_CONTROLLER_OPTIONS,
    ).run(executionPlan, evidencePlan, requests, { ...executionOptions(), approveSourceDispatch: true }),
    /requires a caller-owned runtime journal/,
  );
  const noReservedRehydrator = executionOptions();
  delete noReservedRehydrator.executionPolicyIdentity.value_rehydrator;
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "missing-rehydrator-identity-job",
      RECONCILIATION_CONTROLLER_OPTIONS,
    ).run(executionPlan, evidencePlan, requests, { ...noReservedRehydrator, journal, approveSourceDispatch: true }),
    /actual or reserved value_rehydrator identity/,
  );
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      controller,
      new InMemoryAutonomousEvidenceExecutionCheckpointStore(),
      "invalid-parent-evidence-job",
      RECONCILIATION_CONTROLLER_OPTIONS,
    ).run(executionPlan, evidencePlan, requests, {
      ...executionOptions(),
      journal,
      parentEvidenceDigests: ["not-a-sha256-digest"],
      approveSourceDispatch: true,
    }),
    /parentEvidenceDigests\[0\] must be a lowercase SHA-256 digest/,
  );
  assert.equal(calls.count, 0);
  const firstWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains", RECONCILIATION_CONTROLLER_OPTIONS);
  const gated = await firstWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal });
  assert.equal(gated.status, "approval_required");
  assert.equal(gated.checkpoint.completed_request_count, 0);
  assert.equal(gated.checkpoint.required_requirement_count, evidencePlan.requirements.length);
  assert.equal(gated.checkpoint.checkpoint_generation, 1);
  assert.equal(gated.checkpoint.previous_checkpoint_digest, null);
  assert.equal(calls.count, 0);

  const driftedPolicy = executionOptions();
  driftedPolicy.stopOnFailure = true;
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains", RECONCILIATION_CONTROLLER_OPTIONS)
      .run(executionPlan, evidencePlan, requests, { ...driftedPolicy, journal, approveSourceDispatch: true }),
    /execution policy/,
  );
  assert.equal(calls.count, 0);

  let plainCheckpoint = null;
  const plainStore = {
    read: () => plainCheckpoint,
    write: (checkpoint) => { plainCheckpoint = structuredClone(checkpoint); },
  };
  const plainGated = await new AutonomousEvidenceExecutionResumableController(controller, plainStore, "plain-store-job", RECONCILIATION_CONTROLLER_OPTIONS)
    .run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal });
  assert.equal(plainGated.status, "approval_required");
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(controller, plainStore, "plain-store-job", RECONCILIATION_CONTROLLER_OPTIONS)
      .run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true }),
    /transactional compare-and-swap checkpoint store/,
  );
  assert.equal(calls.count, 0);

  const restartedWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains", RECONCILIATION_CONTROLLER_OPTIONS);
  assert.equal((await restartedWorker.restore()).status, "restored");
  const completed = await restartedWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true });
  assert.equal(completed.status, "completed");
  assert.equal(completed.checkpoint.completed_request_count, evidencePlan.requirements.length);
  assert.equal(completed.checkpoint.accepted_request_count, requests.length);
  assert.equal(completed.checkpoint.runtime_status, "completed");
  assert.equal(completed.checkpoint.required_requirement_count, evidencePlan.requirements.length);
  assert.equal(completed.checkpoint.checkpoint_generation, 3);
  assert.notEqual(completed.checkpoint.previous_checkpoint_digest, null);
  assert.equal(completed.replayed, false);
  assert.equal(calls.count, requests.length);
  assert.doesNotMatch(JSON.stringify(completed.toJSON()), /transient_source_value|operation/);

  const replayWorker = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "resumable-all-domains", RECONCILIATION_CONTROLLER_OPTIONS);
  const replayed = await replayWorker.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, rehydrateValue: (receipt) => calls.values.get(receipt.value_digest) ?? null, approveSourceDispatch: true });
  assert.equal(replayed.status, "completed");
  assert.equal(replayed.replayed, true);
  assert.equal(replayed.result?.runtime.json.receipts.every((receipt) => receipt.replay === "replayed"), true);
  assert.equal(calls.count, requests.length);
  assert.equal(replayed.toJSON().checkpoint_digest, replayed.checkpoint.checkpoint_digest);
  assert.equal(replayed.checkpoint.checkpoint_generation, completed.checkpoint.checkpoint_generation + 2);
  assert.notEqual(replayed.checkpoint.checkpoint_digest, completed.checkpoint.checkpoint_digest);
});

test("two restored workers cannot both cross the source-dispatch CAS fence", async () => {
  let releaseFirst;
  let markFirstStarted;
  const firstStarted = new Promise((resolve) => { markFirstStarted = resolve; });
  const holdFirst = new Promise((resolve) => { releaseFirst = resolve; });
  const calls = { count: 0 };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({
    adapterId: "resumable.cas-fence",
    version: "1.0.0",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: async () => {
      calls.count += 1;
      if (calls.count === 1) {
        markFirstStarted();
        await holdFirst;
      }
      return { fenced_source_call: calls.count };
    },
  });
  const evidencePlan = await planAgent().evidencePlan(["coding"]);
  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `cas-source-${index}`,
    source_digest: "7".repeat(64),
    request_id: `cas-request-${index}`,
  }));
  const controller = new AutonomousEvidenceExecutionController(registry);
  const executionPlan = await controller.prepare(evidencePlan, {
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
  });
  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  await new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "two-worker-job", RECONCILIATION_CONTROLLER_OPTIONS)
    .run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal });
  const workerA = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "two-worker-job", RECONCILIATION_CONTROLLER_OPTIONS);
  const workerB = new AutonomousEvidenceExecutionResumableController(controller, checkpointStore, "two-worker-job", RECONCILIATION_CONTROLLER_OPTIONS);
  await Promise.all([workerA.restore(), workerB.restore()]);
  const winner = workerA.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true });
  await firstStarted;
  await assert.rejects(
    workerB.run(executionPlan, evidencePlan, requests, { ...executionOptions(), journal, approveSourceDispatch: true }),
    /another worker committed after restore/,
  );
  assert.equal(calls.count, 1);
  releaseFirst();
  const completed = await winner;
  assert.equal(completed.status, "completed");
  assert.equal(calls.count, requests.length);
});

test("uncertain dispatch requires exact per-request reconciliation and redispatches only proven not-executed requests", async () => {
  const fixture = await uncertainDispatchFixture("mixed-reconciliation-job");
  const legacyBooleanOnly = await new AutonomousEvidenceExecutionResumableController(
    fixture.controller,
    fixture.checkpointStore,
    fixture.jobId,
    RECONCILIATION_CONTROLLER_OPTIONS,
  ).run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
    ...executionOptions(),
    journal: fixture.journal,
    approveSourceDispatch: true,
    resumeAfterReconciliation: true,
    rehydrateValue: (receipt) => fixture.calls.values.get(receipt.value_digest) ?? null,
  });
  assert.equal(legacyBooleanOnly.status, "reconciliation_required");
  assert.equal(fixture.calls.count, 1);

  const reconciliationReceipt = createAutonomousEvidenceExecutionReconciliationReceipt({
    jobId: fixture.jobId,
    checkpoint: legacyBooleanOnly.checkpoint,
    evidencePlan: fixture.evidencePlan,
    requests: fixture.requests,
    authorityId: "source-audit",
    authorityVersion: "1",
    outcomes: reconciliationDecisions(fixture, (index) => index === 0 ? "succeeded" : "not_executed"),
  });
  assert.equal(validateAutonomousEvidenceExecutionReconciliationReceipt(reconciliationReceipt).receipt_digest, reconciliationReceipt.receipt_digest);
  assert.doesNotMatch(JSON.stringify(reconciliationReceipt), /transient_source_value|operation/);

  const completed = await new AutonomousEvidenceExecutionResumableController(
    fixture.controller,
    fixture.checkpointStore,
    fixture.jobId,
    RECONCILIATION_CONTROLLER_OPTIONS,
  ).run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
    ...executionOptions(),
    journal: fixture.journal,
    approveSourceDispatch: true,
    reconciliationReceipt,
    rehydrateValue: (receipt) => fixture.calls.values.get(receipt.value_digest) ?? null,
  });
  assert.equal(completed.status, "completed");
  assert.equal(completed.replayed, true);
  assert.equal(completed.checkpoint.reconciliation_receipt_digest, reconciliationReceipt.receipt_digest);
  assert.equal(fixture.calls.count, fixture.requests.length);
  assert.equal(completed.result?.runtime.json.receipts[0].replay, "replayed");
  assert.equal(completed.result?.runtime.json.receipts.slice(1).every((receipt) => receipt.replay === "fresh"), true);
});

test("reconciliation authority is checkpoint-bound and unavailable when no trust root was configured", async () => {
  const fixture = await uncertainDispatchFixture("authority-bound-reconciliation-job");
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId).restore(),
    /authority does not match this controller trust root/,
  );
  assert.equal(fixture.checkpoint.reconciliation_authority_config_digest, "6".repeat(64));
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(
      fixture.controller,
      fixture.checkpointStore,
      fixture.jobId,
      { reconciliationAuthority: { id: "source-audit", version: "1", config_digest: "5".repeat(64) } },
    ).restore(),
    /authority does not match this controller trust root/,
  );
  assert.throws(
    () => createAutonomousEvidenceExecutionReconciliationReceipt({
      jobId: fixture.jobId,
      checkpoint: fixture.checkpoint,
      evidencePlan: fixture.evidencePlan,
      requests: fixture.requests,
      authorityId: "different-source-audit",
      authorityVersion: "1",
      outcomes: reconciliationDecisions(fixture, (index) => index === 0 ? "succeeded" : "not_executed"),
    }),
    /authority does not match its checkpoint trust root/,
  );
  const validReceipt = createAutonomousEvidenceExecutionReconciliationReceipt({
    jobId: fixture.jobId,
    checkpoint: fixture.checkpoint,
    evidencePlan: fixture.evidencePlan,
    requests: fixture.requests,
    authorityId: "source-audit",
    authorityVersion: "1",
    outcomes: reconciliationDecisions(fixture, (index) => index === 0 ? "succeeded" : "not_executed"),
  });
  let reconciliationRehydratorCalls = 0;
  const unfencedCheckpointStore = {
    read: () => fixture.checkpoint,
    write: () => {},
  };
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, unfencedCheckpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: validReceipt,
        rehydrateValue: (journalReceipt) => {
          reconciliationRehydratorCalls += 1;
          return fixture.calls.values.get(journalReceipt.value_digest) ?? null;
        },
      }),
    /transactional compare-and-swap checkpoint store/,
  );
  assert.equal(reconciliationRehydratorCalls, 0);

  const unconfiguredCalls = { count: 0, values: new Map() };
  const unconfiguredRegistry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(unconfiguredRegistry, unconfiguredCalls);
  const unconfiguredEvidencePlan = await planAgent().evidencePlan(["coding"]);
  const unconfiguredRequests = unconfiguredEvidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `unconfigured-source-${index}`,
    source_digest: "8".repeat(64),
  }));
  const unconfiguredController = new AutonomousEvidenceExecutionController(unconfiguredRegistry);
  const unconfiguredExecutionPlan = await unconfiguredController.prepare(unconfiguredEvidencePlan, {
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
  });
  const unconfiguredStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const unconfiguredWorker = new AutonomousEvidenceExecutionResumableController(unconfiguredController, unconfiguredStore, "authority-unavailable-reconciliation-job");
  await assert.rejects(
    unconfiguredWorker.run(unconfiguredExecutionPlan, unconfiguredEvidencePlan, unconfiguredRequests, {
      ...executionOptions(),
      journal: new InMemoryAutonomousEvidenceRuntimeJournal(),
      approveSourceDispatch: true,
    }),
    /requires a configured reconciliationAuthority/,
  );
  assert.equal(unconfiguredCalls.count, 0);
  const unconfiguredGated = await unconfiguredWorker.run(unconfiguredExecutionPlan, unconfiguredEvidencePlan, unconfiguredRequests, { ...executionOptions() });
  const unconfiguredCheckpoint = { ...unconfiguredGated.checkpoint, status: "dispatch_pending", checkpoint_digest: "" };
  const { checkpoint_digest: _unconfiguredDigest, retention: _unconfiguredRetention, secret_material: _unconfiguredSecretMaterial, ...unconfiguredPayload } = unconfiguredCheckpoint;
  unconfiguredCheckpoint.checkpoint_digest = digestJsonSync(unconfiguredPayload);
  assert.equal(validateAutonomousEvidenceExecutionCheckpoint(unconfiguredCheckpoint).reconciliation_authority_id, null);
  assert.throws(
    () => createAutonomousEvidenceExecutionReconciliationReceipt({
      jobId: "authority-unavailable-reconciliation-job",
      checkpoint: unconfiguredCheckpoint,
      evidencePlan: unconfiguredEvidencePlan,
      requests: unconfiguredRequests,
      authorityId: "source-audit",
      authorityVersion: "1",
      outcomes: unconfiguredRequests.map((_request, index) => ({
        outcome: "not_executed",
        evidenceDigest: digestJsonSync({ index, authority: "unconfigured-probe" }),
        evidenceKind: "source_dispatch_audit",
        effectAbsent: true,
      })),
    }),
    /checkpoint has no configured authority/,
  );
  assert.equal(unconfiguredCalls.count, 0);
});

test("unknown, tampered, and replayed reconciliation receipts remain quarantined without another source call", async () => {
  const fixture = await uncertainDispatchFixture("quarantined-reconciliation-job");
  const decisions = reconciliationDecisions(fixture, (index) => index === 0 ? "succeeded" : index === 1 ? "unknown" : "not_executed");
  const receipt = createAutonomousEvidenceExecutionReconciliationReceipt({
    jobId: fixture.jobId,
    checkpoint: fixture.checkpoint,
    evidencePlan: fixture.evidencePlan,
    requests: fixture.requests,
    authorityId: "source-audit",
    authorityVersion: "1",
    outcomes: decisions,
  });
  const tampered = structuredClone(receipt);
  tampered.outcomes[1].outcome = "not_executed";
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: tampered,
        rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
      }),
    /contradicts effect_absent/,
  );
  assert.equal(fixture.calls.count, 1);

  const forgedAuthority = structuredClone(receipt);
  forgedAuthority.authority_id = "forged-source-audit";
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: forgedAuthority,
        rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
      }),
    /receipt digest is invalid/,
  );
  const restampedForgedAuthority = { ...receipt, authority_id: "forged-source-audit", receipt_digest: "" };
  const { receipt_digest: _restampedDigest, ...restampedForgedAuthorityPayload } = restampedForgedAuthority;
  restampedForgedAuthority.receipt_digest = digestJsonSync(restampedForgedAuthorityPayload);
  assert.equal(validateAutonomousEvidenceExecutionReconciliationReceipt(restampedForgedAuthority).authority_id, "forged-source-audit");
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: restampedForgedAuthority,
        rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
      }),
    /authority does not match its checkpoint trust root/,
  );
  assert.throws(
    () => createAutonomousEvidenceExecutionReconciliationReceipt({
      jobId: fixture.jobId,
      checkpoint: fixture.checkpoint,
      evidencePlan: fixture.evidencePlan,
      requests: fixture.requests,
      authorityId: "source-audit",
      authorityVersion: "1",
      outcomes: decisions.map((decision, index) => index === 2 ? { ...decision, effectAbsent: false } : decision),
    }),
    /contradicts effectAbsent/,
  );
  assert.equal(fixture.calls.count, 1);

  const falseSuccess = createAutonomousEvidenceExecutionReconciliationReceipt({
    jobId: fixture.jobId,
    checkpoint: fixture.checkpoint,
    evidencePlan: fixture.evidencePlan,
    requests: fixture.requests,
    authorityId: "source-audit",
    authorityVersion: "1",
    outcomes: decisions.map((decision, index) => index === 0 ? { ...decision, succeededReceiptDigest: "f".repeat(64) } : decision),
  });
  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: falseSuccess,
        rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
      }),
    /does not match a journal-backed source success/,
  );
  assert.equal(fixture.calls.count, 1);

  const held = await new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
    .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
      ...executionOptions(),
      journal: fixture.journal,
      approveSourceDispatch: true,
      reconciliationReceipt: receipt,
      rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
    });
  assert.equal(held.status, "reconciliation_required");
  assert.equal(held.checkpoint.reconciliation_receipt_digest, receipt.receipt_digest);
  assert.notEqual(held.checkpoint.checkpoint_digest, receipt.checkpoint_digest);
  assert.equal(fixture.calls.count, 1);

  await assert.rejects(
    new AutonomousEvidenceExecutionResumableController(fixture.controller, fixture.checkpointStore, fixture.jobId, RECONCILIATION_CONTROLLER_OPTIONS)
      .run(fixture.executionPlan, fixture.evidencePlan, fixture.requests, {
        ...executionOptions(),
        journal: fixture.journal,
        approveSourceDispatch: true,
        reconciliationReceipt: receipt,
        rehydrateValue: (journalReceipt) => fixture.calls.values.get(journalReceipt.value_digest) ?? null,
      }),
    /stale or bound to a different checkpoint/,
  );
  assert.equal(fixture.calls.count, 1);
});

test("evidence execution checkpoint persistence is JSON-portable, browser-portable, tamper-evident, and CAS-fenced", async () => {
  let encoded = null;
  const storage = {
    getItem: () => encoded,
    setItem: (_key, value) => { encoded = value; },
  };
  const textStore = new WebStorageAutonomousEvidenceExecutionCheckpointTextStore(storage, "evidence-checkpoint");
  const jsonStore = new JsonAutonomousEvidenceExecutionCheckpointStore(textStore);
  const transactionalTextStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).checkpoint_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const transactionalStore = new TransactionalJsonAutonomousEvidenceExecutionCheckpointStore(transactionalTextStore);
  const checkpoint = {
    schema: "bioprism-typescript-autonomous-evidence-execution-checkpoint/0.2",
    job_id: "checkpoint-job",
    evidence_plan_digest: "a".repeat(64),
    execution_plan_digest: "b".repeat(64),
    request_digest: "c".repeat(64),
    readiness_report_digest: "d".repeat(64),
    execution_policy_digest: "f".repeat(64),
    required_requirement_count: 17,
    checkpoint_generation: 1,
    previous_checkpoint_digest: null,
    reconciliation_authority_id: null,
    reconciliation_authority_version: null,
    reconciliation_authority_config_digest: null,
    status: "approval_required",
    runtime_status: null,
    runtime_result_digest: null,
    completed_request_count: 0,
    pending_request_count: 0,
    accepted_request_count: 0,
    reconciliation_receipt_digest: null,
    checkpoint_digest: "",
    retention: "metadata_only;requests_readiness_and_source_values_caller_owned",
    secret_material: "never_returned",
  };
  const { checkpoint_digest: _checkpointDigest, retention: _retention, secret_material: _secretMaterial, ...payload } = checkpoint;
  checkpoint.checkpoint_digest = digestJsonSync(payload);
  await jsonStore.write(checkpoint);
  const restored = await jsonStore.read();
  assert.equal(restored?.checkpoint_digest, checkpoint.checkpoint_digest);
  assert.equal((await transactionalStore.writeIfUnchanged(null, checkpoint)), false);
  const successor = {
    ...checkpoint,
    checkpoint_generation: 2,
    previous_checkpoint_digest: checkpoint.checkpoint_digest,
    checkpoint_digest: "",
  };
  const { checkpoint_digest: _successorDigest, retention: _successorRetention, secret_material: _successorSecretMaterial, ...successorPayload } = successor;
  successor.checkpoint_digest = digestJsonSync(successorPayload);
  assert.equal((await transactionalStore.writeIfUnchanged(checkpoint.checkpoint_digest, successor)), true);
  assert.equal(validateAutonomousEvidenceExecutionCheckpoint(restored).job_id, "checkpoint-job");
  assert.throws(() => validateAutonomousEvidenceExecutionCheckpoint({ ...restored, checkpoint_digest: "e".repeat(64) }), /digest is invalid/);
  const brokenLineage = { ...checkpoint, checkpoint_generation: 1, previous_checkpoint_digest: "0".repeat(64), checkpoint_digest: "" };
  const { checkpoint_digest: _brokenLineageDigest, retention: _brokenLineageRetention, secret_material: _brokenLineageSecretMaterial, ...brokenLineagePayload } = brokenLineage;
  brokenLineage.checkpoint_digest = digestJsonSync(brokenLineagePayload);
  assert.throws(() => validateAutonomousEvidenceExecutionCheckpoint(brokenLineage), /lineage is inconsistent/);
  const incompleteAuthority = { ...checkpoint, reconciliation_authority_id: "source-audit", checkpoint_digest: "" };
  const { checkpoint_digest: _incompleteAuthorityDigest, retention: _incompleteAuthorityRetention, secret_material: _incompleteAuthoritySecretMaterial, ...incompleteAuthorityPayload } = incompleteAuthority;
  incompleteAuthority.checkpoint_digest = digestJsonSync(incompleteAuthorityPayload);
  assert.throws(() => validateAutonomousEvidenceExecutionCheckpoint(incompleteAuthority), /authority is incomplete/);
  const forged = {
    ...checkpoint,
    status: "completed",
    runtime_status: "completed",
    runtime_result_digest: "e".repeat(64),
    completed_request_count: 16,
    pending_request_count: 0,
    accepted_request_count: 16,
  };
  const { checkpoint_digest: _forgedDigest, retention: _forgedRetention, secret_material: _forgedSecretMaterial, ...forgedPayload } = forged;
  forged.checkpoint_digest = digestJsonSync(forgedPayload);
  assert.throws(() => validateAutonomousEvidenceExecutionCheckpoint(forged), /does not cover every required plan requirement/);

  const stale = { ...successor };
  assert.equal(await transactionalStore.writeIfUnchanged("0".repeat(64), stale), false);
});

test("AutonomousAgent exposes the restart-safe evidence lifecycle at the high-level facade", async () => {
  const calls = { count: 0, values: new Map() };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerAllDomains(registry, calls);
  const agent = planAgent();
  const plan = await agent.evidencePlan(["coding"]);
  const requests = plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `facade-source-${index}`,
    source_digest: "e".repeat(64),
  }));
  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const first = await agent.executeReviewedEvidenceResumable(registry, ["coding"], requests, {
    jobId: "facade-resumable-job",
    checkpointStore,
    reconciliationAuthority: RECONCILIATION_CONTROLLER_OPTIONS.reconciliationAuthority,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    execute: executionOptions(),
  });
  assert.equal(first.status, "approval_required");
  assert.equal(calls.count, 0);

  const journal = new InMemoryAutonomousEvidenceRuntimeJournal();
  const second = await agent.executeReviewedEvidenceResumable(registry, ["coding"], requests, {
    jobId: "facade-resumable-job",
    checkpointStore,
    reconciliationAuthority: RECONCILIATION_CONTROLLER_OPTIONS.reconciliationAuthority,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    execute: { ...executionOptions(), journal, approveSourceDispatch: true },
  });
  assert.equal(second.status, "completed");
  assert.equal(second.checkpoint.completed_request_count, requests.length);
  assert.equal(calls.count, requests.length);
});
