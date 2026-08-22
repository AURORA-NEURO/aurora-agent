import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceAdapterHealthController,
  JsonAutonomousEvidenceAdapterHealthPersistence,
  AutonomousEvidenceAdapterHealthPersistenceCoordinator,
  AutonomousEvidenceAdapterSelectionPlan,
  AutonomousEvidenceAdapterSelector,
  AutonomousEvidenceRuntime,
  AutonomousHttpConnectorPolicy,
  AutonomousHttpConnectorRequest,
  AutonomousWorkflowPortfolioEvidenceController,
  AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker,
  AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator,
  AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator,
  AutonomousWorkflowPortfolioEvidenceWorkWorker,
  InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue,
  InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
  JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
  InMemoryAutonomousEvidenceRuntimeJournal,
  InMemoryAutonomousEvidenceAdapterHealthStore,
  TransactionalJsonAutonomousEvidenceAdapterHealthPersistence,
  WebStorageAutonomousEvidenceAdapterHealthSnapshotTextStore,
  InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore,
  JsonAutonomousWorkflowPortfolioEvidenceCheckpointStore,
  LLMRuntime,
  TransactionalJsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence,
  TransactionalJsonAutonomousWorkflowPortfolioEvidenceCheckpointStore,
  WebStorageAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore,
  admitAutonomousWorkflowPortfolioEvidenceWorkItems,
  digestJson,
  registerAutonomousEvidenceAdaptersForAllDomains,
  registerAutonomousHttpEvidenceAdapter,
  validateAutonomousWorkflowPortfolioEvidenceCheckpoint,
  validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot,
} from "../dist/index.js";

const model = {
  provider: "offline",
  model: "offline-model",
  capabilities: [
    "reasoning", "structured_output", "code", "web", "data", "science", "biomedical",
    "operations", "enterprise", "coordination", "multimodal", "evaluation",
  ],
  context_window_tokens: 32_000,
  max_output_tokens: 2_000,
  quality: 0.9,
  latency_ms: 10,
  cost_per_million_tokens: 0,
  reliability: 0.99,
};

function agentFor(onRequest = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("HTTP must not be reached"); } });
  runtime.registerInMemoryProvider("offline", (request) => {
    onRequest(request);
    return { output_text: `offline result for ${request.model}` };
  });
  const agent = new AutonomousAgent(runtime);
  agent.registerModel(model);
  return agent;
}

function portfolioRequests(domains = AUTONOMOUS_DOMAIN_NAMES) {
  return domains.map((domain, index) => ({
    id: `portfolio-${domain}`,
    task: `private provider task payload for ${domain}`,
    domain,
    ...(index === 0 ? {} : { dependsOn: [`portfolio-${domains[index - 1]}`] }),
    hints: [`private provider hint for ${domain}`],
  }));
}

function evidenceRequests(evidencePlan, domains = AUTONOMOUS_DOMAIN_NAMES) {
  return domains.map((domain) => ({
    item_id: `portfolio-${domain}`,
    requests: evidencePlan.requirements
      .filter((requirement) => requirement.domain === domain)
      .map((requirement, index) => ({
        requirement_id: requirement.requirement_id,
        source_id: `evidence-source-${domain}-${index}`,
        request_id: `evidence-request-${domain}-${index}`,
        metadata: { purpose: "bounded-portfolio-evidence" },
      })),
  }));
}

function evidenceRuntime({ acquire, parentDigests = [] } = {}) {
  return {
    acquirer: {
      async acquire(context) {
        acquire?.(context);
        parentDigests.push(...context.parent_evidence_digests);
        return {
          private_raw_evidence: "must remain caller-owned",
          item_id: context.request.metadata.portfolio_item_id,
          requirement_id: context.requirement.requirement_id,
        };
      },
    },
    projector: {
      project(_value, context) {
        return [{ label: context.requirement.label, kind: "fact", status: "observed" }];
      },
    },
    evaluator: {
      evaluator_id: "portfolio-evidence-evaluator",
      evaluator_version: "1",
      evaluate() {
        return {
          evaluator_id: "portfolio-evidence-evaluator",
          evaluator_version: "1",
          verdict: "accepted",
          score: 1,
          evidence_digest: "4".repeat(64),
        };
      },
    },
  };
}

test("portfolio evidence supervisor evaluates every domain in dependency waves and keeps values transient", async () => {
  const providerCalls = [];
  const agent = agentFor((request) => providerCalls.push(request));
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    maxParallelism: 3,
  });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const parentDigests = [];
  const result = await agent.executeWorkflowPortfolioEvidence(providerExecution, {
    evidencePlan,
    items: evidenceRequests(evidencePlan),
    runtime: evidenceRuntime({ parentDigests }),
    maxParallelism: 3,
  });

  assert.equal(providerCalls.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.status, "completed");
  assert.equal(result.toJSON().completed_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(result.toJSON().failed_count, 0);
  assert.ok(result.items.every((item) => item.status === "completed"));
  assert.ok(parentDigests.length > 0, "dependent evidence receives predecessor result digests");
  assert.equal(result.toJSON().retention, "metadata_only;raw_evidence_values_caller_owned");
  assert.doesNotMatch(JSON.stringify(result), /private_raw_evidence|private provider task payload|offline result/);
  assert.ok(result.runtimeFor("portfolio-coding")?.values);
});

test("portfolio evidence supervisor rehydrates item journals without reacquiring provider-owned evidence", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    maxParallelism: 4,
  });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const journals = new Map();
  const firstCalls = [];
  const first = await agent.executeWorkflowPortfolioEvidence(providerExecution, {
    evidencePlan,
    items: evidenceRequests(evidencePlan),
    journalFor: ({ itemId }) => {
      const journal = journals.get(itemId) ?? new InMemoryAutonomousEvidenceRuntimeJournal();
      journals.set(itemId, journal);
      return journal;
    },
    runtime: evidenceRuntime({ acquire: (context) => firstCalls.push(context.request.request_id) }),
  });
  const valuesByRequest = new Map();
  for (const item of first.items) for (const [requestDigest, value] of Object.entries(item.runtime?.values ?? {})) valuesByRequest.set(requestDigest, value);
  const secondCalls = [];
  const resumed = await agent.executeWorkflowPortfolioEvidence(providerExecution, {
    evidencePlan,
    items: evidenceRequests(evidencePlan),
    journalFor: ({ itemId }) => journals.get(itemId),
    runtime: {
      ...evidenceRuntime({ acquire: (context) => secondCalls.push(context.request.request_id) }),
      rehydrateValue: (receipt) => valuesByRequest.get(receipt.request_digest) ?? null,
    },
  });

  assert.equal(first.status, "completed");
  assert.equal(resumed.status, "completed");
  assert.equal(firstCalls.length, valuesByRequest.size);
  assert.equal(secondCalls.length, 0);
  assert.ok(resumed.items.every((item) => item.runtime?.json.receipts.every((receipt) => receipt.replay === "replayed")));
});

test("portfolio evidence supervisor refuses unapproved provider executions and rejects cross-domain evidence", async () => {
  let providerCalls = 0;
  const agent = agentFor(() => { providerCalls += 1; });
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), { planOptions: { requireAllDomains: true } });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const calls = [];
  const result = await agent.executeWorkflowPortfolioEvidence(providerExecution, {
    evidencePlan,
    items: evidenceRequests(evidencePlan),
    runtime: evidenceRuntime({ acquire: () => calls.push("acquire") }),
  });

  assert.equal(providerCalls, 0);
  assert.equal(calls.length, 0);
  assert.equal(result.status, "partial");
  assert.equal(result.toJSON().omitted_count, AUTONOMOUS_DOMAIN_NAMES.length);

  const approved = await agent.executeWorkflowPortfolio([{ id: "coding", task: "approved task", domain: "coding" }], { approveProviderCall: true });
  const scienceRequirement = evidencePlan.requirements.find((requirement) => requirement.domain === "science");
  await assert.rejects(
    () => agent.executeWorkflowPortfolioEvidence(approved, {
      evidencePlan,
      items: [{ item_id: "coding", requests: [{ requirement_id: scienceRequirement.requirement_id, source_id: "wrong-domain" }] }],
      runtime: evidenceRuntime(),
    }),
    /crosses item domain/,
  );
});

test("portfolio evidence supervisor stops later dependency waves after a failed acquisition", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio([
    { id: "coding", task: "coding provider task", domain: "coding" },
    { id: "data", task: "dependent data provider task", domain: "data", dependsOn: ["coding"] },
  ], { approveProviderCall: true });
  const evidencePlan = await agent.evidencePlan(["coding", "data"]);
  const items = evidenceRequests(evidencePlan, ["coding", "data"])
    .map((entry, index) => ({ ...entry, item_id: ["coding", "data"][index] }));
  const result = await agent.executeWorkflowPortfolioEvidence(providerExecution, {
    evidencePlan,
    items,
    stopOnFailure: true,
    runtime: {
      ...evidenceRuntime(),
      stopOnFailure: true,
      acquirer: {
        async acquire(context) {
          if (context.request.metadata.portfolio_item_id === "coding") throw new Error("synthetic acquisition failure");
          return { item_id: context.request.metadata.portfolio_item_id };
        },
      },
    },
  });

  const byId = new Map(result.items.map((item) => [item.itemId, item]));
  assert.equal(result.status, "failed");
  assert.equal(byId.get("coding")?.status, "failed");
  assert.equal(byId.get("data")?.status, "omitted");
  assert.equal(byId.get("data")?.errorClass, "portfolio_evidence_stopped_after_failure");
});

test("portfolio evidence controller checkpoints every wave and replays completed evidence after restart", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
    maxParallelism: 4,
  });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const items = evidenceRequests(evidencePlan);
  const journals = new Map();
  const checkpointStore = new InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore();
  const firstCalls = [];
  const progressSnapshots = [];
  const firstController = new AutonomousWorkflowPortfolioEvidenceController(agent, "portfolio-evidence-restart", checkpointStore);
  const first = await firstController.run(providerExecution, {
    evidencePlan,
    items,
    runtimePolicyDigest: "5".repeat(64),
    progressSink: (progress) => progressSnapshots.push(progress),
    journalFor: ({ itemId }) => {
      const journal = journals.get(itemId) ?? new InMemoryAutonomousEvidenceRuntimeJournal();
      journals.set(itemId, journal);
      return journal;
    },
    runtime: evidenceRuntime({ acquire: (context) => firstCalls.push(context.request.request_id) }),
  });
  const valuesByRequest = new Map();
  for (const item of first.evidence.items) for (const [requestDigest, value] of Object.entries(item.runtime?.values ?? {})) valuesByRequest.set(requestDigest, value);
  const checkpoint = await checkpointStore.read();
  assert.ok(checkpoint);
  assert.equal(checkpoint.status, "completed");
  assert.equal(checkpoint.settled_item_ids.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(progressSnapshots.length >= AUTONOMOUS_DOMAIN_NAMES.length, "checkpoint progress is flushed after dependency waves");
  assert.equal(checkpoint.retention, "request_and_result_digests_only;raw_evidence_values_and_sources_never_persisted");

  const casStore = new InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore();
  assert.equal(await casStore.writeIfUnchanged(null, checkpoint), true);
  assert.equal(await casStore.writeIfUnchanged(null, checkpoint), false, "a second writer cannot reuse the empty-store fence");
  let checkpointText = null;
  const textStore = {
    read: () => checkpointText,
    write: (value) => { checkpointText = value; },
    writeIfUnchanged: (expected, value) => {
      const current = checkpointText === null ? null : JSON.parse(checkpointText).checkpoint_digest;
      if (current !== expected) return false;
      checkpointText = value;
      return true;
    },
  };
  const jsonStore = new JsonAutonomousWorkflowPortfolioEvidenceCheckpointStore(textStore);
  await jsonStore.write(checkpoint);
  assert.deepEqual((await jsonStore.read()).checkpoint_digest, checkpoint.checkpoint_digest);
  const transactionalJsonStore = new TransactionalJsonAutonomousWorkflowPortfolioEvidenceCheckpointStore(textStore);
  assert.equal(await transactionalJsonStore.writeIfUnchanged("0".repeat(64), checkpoint), false);
  assert.equal(await transactionalJsonStore.writeIfUnchanged(checkpoint.checkpoint_digest, checkpoint), true);

  const staleController = new AutonomousWorkflowPortfolioEvidenceController(agent, "portfolio-evidence-restart", casStore);
  assert.equal((await staleController.restore()).status, "restored");
  const { checkpoint_digest: _checkpointDigest, retention: _retention, secret_material: _secretMaterial, ...checkpointPayload } = checkpoint;
  const externallyAdvancedPayload = { ...checkpointPayload, status: "partial" };
  const externallyAdvanced = { ...externallyAdvancedPayload, checkpoint_digest: await digestJson(externallyAdvancedPayload), retention: _retention, secret_material: _secretMaterial };
  assert.equal(await casStore.writeIfUnchanged(checkpoint.checkpoint_digest, externallyAdvanced), true);
  await assert.rejects(
    () => staleController.run(providerExecution, {
      evidencePlan,
      items,
      runtimePolicyDigest: "5".repeat(64),
      journalFor: ({ itemId }) => journals.get(itemId),
      runtime: {
        ...evidenceRuntime(),
        rehydrateValue: (receipt) => valuesByRequest.get(receipt.request_digest) ?? null,
      },
    }),
    /compare-and-swap conflict/,
  );

  const secondCalls = [];
  const secondController = new AutonomousWorkflowPortfolioEvidenceController(agent, "portfolio-evidence-restart", checkpointStore);
  assert.equal((await secondController.restore()).status, "restored");
  const resumed = await secondController.run(providerExecution, {
    evidencePlan,
    items,
    runtimePolicyDigest: "5".repeat(64),
    journalFor: ({ itemId }) => journals.get(itemId),
    runtime: {
      ...evidenceRuntime({ acquire: (context) => secondCalls.push(context.request.request_id) }),
      rehydrateValue: (receipt) => valuesByRequest.get(receipt.request_digest) ?? null,
    },
  });

  assert.equal(first.evidence.status, "completed");
  assert.equal(resumed.evidence.status, "completed");
  assert.equal(firstCalls.length, valuesByRequest.size);
  assert.equal(secondCalls.length, 0);
  assert.equal(resumed.controller.status, "completed");
  assert.equal(resumed.controller.settled_items, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(resumed.evidence), /private_raw_evidence|offline result/);
});

test("portfolio evidence checkpoints reject tampering, request drift, and evaluator-policy drift before replay", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio([{ id: "coding", task: "checkpoint task", domain: "coding" }], { approveProviderCall: true });
  const evidencePlan = await agent.evidencePlan(["coding"]);
  const items = evidenceRequests(evidencePlan, ["coding"]).map((entry) => ({ ...entry, item_id: "coding" }));
  const journals = new Map([["coding", new InMemoryAutonomousEvidenceRuntimeJournal()]]);
  const store = new InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore();
  const controller = new AutonomousWorkflowPortfolioEvidenceController(agent, "checkpoint-drift", store);
  await controller.run(providerExecution, {
    evidencePlan,
    items,
    runtimePolicyDigest: "6".repeat(64),
    journalFor: ({ itemId }) => journals.get(itemId),
    runtime: evidenceRuntime(),
  });
  const checkpoint = await store.read();
  await assert.rejects(
    () => validateAutonomousWorkflowPortfolioEvidenceCheckpoint({ ...checkpoint, status: "partial" }),
    /checkpoint digest is invalid/,
  );
  const restarted = new AutonomousWorkflowPortfolioEvidenceController(agent, "checkpoint-drift", store);
  const changedItems = items.map((entry) => ({ ...entry, requests: entry.requests.map((request) => ({ ...request, source_id: "changed-source" })) }));
  await assert.rejects(
    () => restarted.run(providerExecution, {
      evidencePlan,
      items: changedItems,
      runtimePolicyDigest: "6".repeat(64),
      journalFor: ({ itemId }) => journals.get(itemId),
      runtime: evidenceRuntime(),
    }),
    /does not match the current reviewed execution or evidence input/,
  );
  await assert.rejects(
    () => restarted.run(providerExecution, {
      evidencePlan,
      items,
      runtimePolicyDigest: "7".repeat(64),
      journalFor: ({ itemId }) => journals.get(itemId),
      runtime: evidenceRuntime(),
    }),
    /checkpoint controls do not match/,
  );
});

test("portfolio evidence work queue admits every domain, enforces dependency waves, and fences leases", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), {
    planOptions: { requireAllDomains: true },
    approveProviderCall: true,
  });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const queue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  const admitted = admitAutonomousWorkflowPortfolioEvidenceWorkItems(queue, {
    jobId: "evidence-work-job",
    execution: providerExecution,
    evidencePlanDigest: evidencePlan.plan_digest,
    itemRequestDigests: AUTONOMOUS_DOMAIN_NAMES.map((_, index) => `${String(index + 1).padStart(2, "0")}`.repeat(32)),
    checkpointDigest: "a".repeat(64),
    now: 1_000,
  });

  assert.equal(admitted.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(queue.pending().length, 1, "only the root wave is claimable initially");
  const first = queue.claim("evidence-work-job:portfolio-coding", "worker-a", 30_000, 1_000);
  assert.equal(first?.domain, "coding");
  assert.equal(queue.claim(first.work_id, "worker-b", 30_000, 1_001), null, "an active lease is not stealable");
  queue.complete(first.work_id, "worker-a", { status: "completed", resultDigest: "b".repeat(64) }, 1_002);

  for (const domain of AUTONOMOUS_DOMAIN_NAMES.slice(1)) {
    const next = queue.pending(1, 2_000)[0];
    assert.equal(next?.item_id, `portfolio-${domain}`);
    const claimed = queue.claim(next.work_id, "worker-a", 30_000, 2_000);
    assert.ok(claimed);
    queue.complete(claimed.work_id, "worker-a", { status: "completed", resultDigest: "c".repeat(64) }, 2_001);
  }

  assert.ok(queue.rows().every((item) => item.status === "completed"));
  assert.equal(queue.bindCheckpointDigest("evidence-work-job", "d".repeat(64), 3_000), AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(queue.snapshot()), /private provider task payload|offline result|source secret/);
});

test("portfolio evidence work worker retries bounded failures, reconciles expired leases, and restores through fenced persistence", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio([{ id: "coding", task: "worker task", domain: "coding" }], { approveProviderCall: true });
  const queue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  admitAutonomousWorkflowPortfolioEvidenceWorkItems(queue, {
    jobId: "worker-job",
    execution: providerExecution,
    evidencePlanDigest: "e".repeat(64),
    itemRequestDigests: ["f".repeat(64)],
    maxAttempts: 2,
    now: 1_000,
  });
  let attempts = 0;
  const worker = new AutonomousWorkflowPortfolioEvidenceWorkWorker(queue, () => {
    attempts += 1;
    return attempts === 1
      ? { status: "failed", result_digest: null, error_class: "transport_error", retryable: true }
      : { status: "completed", result_digest: "1".repeat(64) };
  });
  const firstRun = await worker.run({ workerId: "worker-a", now: 1_000 });
  assert.equal(firstRun.retried, 1);
  const secondRun = await worker.run({ workerId: "worker-a", now: 2_001 });
  assert.equal(secondRun.completed, 1);
  assert.equal(queue.get("worker-job:coding")?.status, "completed");

  const expiredQueue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  admitAutonomousWorkflowPortfolioEvidenceWorkItems(expiredQueue, {
    jobId: "expiry-job",
    execution: providerExecution,
    evidencePlanDigest: "2".repeat(64),
    itemRequestDigests: ["3".repeat(64)],
    now: 5_000,
  });
  const leased = expiredQueue.claim("expiry-job:coding", "worker-a", 10, 5_000);
  assert.ok(leased);
  let expiredExecutorCalls = 0;
  const expiredRun = await new AutonomousWorkflowPortfolioEvidenceWorkWorker(expiredQueue, () => {
    expiredExecutorCalls += 1;
    return { status: "completed", result_digest: "0".repeat(64) };
  }).run({ workerId: "worker-b", now: 5_011, limit: 1 });
  assert.equal(expiredRun.reconciled, 1, "a worker reaper reconciles expired leases without stealing them");
  assert.equal(expiredExecutorCalls, 0);
  assert.equal(expiredQueue.get(leased.work_id)?.status, "reconciliation_required");
  assert.throws(() => expiredQueue.complete(leased.work_id, "worker-a", { status: "completed", resultDigest: "4".repeat(64) }, 5_012), /fenced/);

  const persistence = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence();
  const coordinator = new AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator(queue, persistence);
  const snapshot = await coordinator.flush();
  const restoredQueue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  const restored = new AutonomousWorkflowPortfolioEvidenceWorkQueuePersistenceCoordinator(restoredQueue, persistence);
  assert.equal((await restored.restore()).status, "restored");
  assert.equal(restoredQueue.get("worker-job:coding")?.item_digest, queue.get("worker-job:coding")?.item_digest);
  restoredQueue.bindCheckpointDigest("worker-job", "5".repeat(64), 3_000);
  await restored.flush();
  await assert.rejects(() => coordinator.flush(), /compare-and-swap conflict/, "a stale coordinator cannot overwrite a newer queue snapshot");
  assert.equal(snapshot.schema, "bioprism-typescript-autonomous-workflow-portfolio-evidence-work-queue/0.1");
});

test("portfolio evidence work queue has bounded JSON, transactional, and browser-storage persistence", async () => {
  const queue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  const digest = "a".repeat(64);
  queue.admit({
    workId: "json-job:item",
    jobId: "json-job",
    itemId: "item",
    domain: "coding",
    waveIndex: 0,
    providerStatus: "succeeded",
    portfolioPlanDigest: digest,
    providerExecutionDigest: "b".repeat(64),
    evidencePlanDigest: "c".repeat(64),
    requestDigest: "d".repeat(64),
    checkpointDigest: "e".repeat(64),
    now: 7_000,
  });
  const snapshot = queue.snapshot();
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
    writeIfUnchanged: (expected, value) => {
      const current = encoded === null ? null : JSON.parse(encoded).snapshot_digest;
      if (current !== expected) return false;
      encoded = value;
      return true;
    },
  };
  const json = new JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence(textStore);
  await json.write(snapshot);
  assert.equal((await json.read()).snapshot_digest, snapshot.snapshot_digest);
  assert.deepEqual(validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot(JSON.parse(encoded)).items.map((item) => item.work_id), ["json-job:item"]);

  const transactional = new TransactionalJsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence(textStore);
  assert.equal(await transactional.writeIfUnchanged("0".repeat(64), snapshot), false);
  assert.equal(await transactional.writeIfUnchanged(snapshot.snapshot_digest, snapshot), true);
  const malformed = new JsonAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence({ read: () => "{", write: () => {} });
  await assert.rejects(() => malformed.read(), /invalid JSON/);

  const values = new Map();
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  };
  const browserStore = new WebStorageAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshotTextStore(storage, "queue-key");
  browserStore.write(encoded);
  assert.equal(browserStore.read(), encoded);
  assert.throws(() => validateAutonomousWorkflowPortfolioEvidenceWorkQueueSnapshot({ ...snapshot, snapshot_digest: "f".repeat(64) }), /snapshot digest is invalid/);
});

test("CAS portfolio evidence coordinators prevent duplicate claims and drive every domain through atomic worker transitions", async () => {
  const agent = agentFor();
  const providerExecution = await agent.executeWorkflowPortfolio(portfolioRequests(), { planOptions: { requireAllDomains: true }, approveProviderCall: true });
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const seedQueue = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue();
  admitAutonomousWorkflowPortfolioEvidenceWorkItems(seedQueue, {
    jobId: "atomic-job",
    execution: providerExecution,
    evidencePlanDigest: evidencePlan.plan_digest,
    itemRequestDigests: AUTONOMOUS_DOMAIN_NAMES.map((_, index) => `${String(index + 11).padStart(2, "0")}`.repeat(32)),
    now: 8_000,
  });
  const persistence = new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueuePersistence();
  await persistence.write(seedQueue.snapshot());
  const originalRead = persistence.read.bind(persistence);
  const waiters = [];
  let coordinatedReads = 0;
  let casConflicts = 0;
  const sharedPersistence = {
    read: () => {
      const snapshot = originalRead();
      if (coordinatedReads < 2) {
        coordinatedReads += 1;
        return new Promise((resolve) => {
          waiters.push(() => resolve(snapshot));
          if (waiters.length === 2) for (const release of waiters.splice(0)) release();
        });
      }
      return snapshot;
    },
    write: (snapshot) => persistence.write(snapshot),
    writeIfUnchanged: (expected, snapshot) => {
      const committed = persistence.writeIfUnchanged(expected, snapshot);
      if (!committed) casConflicts += 1;
      return committed;
    },
  };
  const coordinatorA = new AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator(new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue(), sharedPersistence);
  const coordinatorB = new AutonomousWorkflowPortfolioEvidenceWorkQueueAtomicCoordinator(new InMemoryAutonomousWorkflowPortfolioEvidenceWorkQueue(), sharedPersistence);
  const [claimA, claimB] = await Promise.all([
    coordinatorA.claim("atomic-job:portfolio-coding", "worker-a", 30_000, 8_000),
    coordinatorB.claim("atomic-job:portfolio-coding", "worker-b", 30_000, 8_000),
  ]);
  assert.equal([claimA, claimB].filter(Boolean).length, 1, "only one coordinator can commit the root lease");
  assert.ok(casConflicts >= 1, "the losing coordinator observes a compare-and-swap conflict");
  const ownerCoordinator = claimA ? coordinatorA : coordinatorB;
  await ownerCoordinator.complete("atomic-job:portfolio-coding", claimA ? "worker-a" : "worker-b", { status: "completed", resultDigest: "0".repeat(64) }, 8_001);

  let executions = 0;
  const worker = new AutonomousWorkflowPortfolioEvidenceAtomicWorkWorker(coordinatorA, () => {
    executions += 1;
    return { status: "completed", result_digest: "1".repeat(64) };
  });
  for (let index = 1; index < AUTONOMOUS_DOMAIN_NAMES.length; index += 1) {
    const run = await worker.run({ workerId: "atomic-worker", limit: 1, now: 8_002 + index });
    assert.equal(run.completed, 1, `atomic worker completes dependency wave ${index}`);
  }
  const finalSnapshot = await coordinatorB.snapshot();
  assert.equal(executions, AUTONOMOUS_DOMAIN_NAMES.length - 1);
  assert.ok(finalSnapshot.items.every((item) => item.status === "completed"));
  assert.doesNotMatch(JSON.stringify(finalSnapshot), /private provider task payload|offline result|source secret/);
});

test("domain evidence adapter registry routes all twelve domains through scoped transient acquisition", async () => {
  const agent = agentFor();
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const registry = new AutonomousEvidenceAdapterRegistry();
  const acquiredDomains = [];
  const manifests = registerAutonomousEvidenceAdaptersForAllDomains(registry, (domain) => ({
    adapterId: `source.${domain}`,
    version: "1.0.0",
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: (context) => {
      acquiredDomains.push(context.requirement.domain);
      return { transient_private_value: `${domain}-value`, domain, source_id: context.request.source_id };
    },
    project: (_value, context) => [{ label: context.requirement.label, kind: "fact", status: "observed", value_digest: null, source_digest: context.request.source_digest ?? null, limitations: ["caller_owned_transient_value"] }],
  }));
  assert.equal(manifests.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(registry.coverage().every((row) => row.state === "complete"));
  assert.equal(registry.toJSON().coverage_digest.length, 64);
  assert.doesNotMatch(JSON.stringify(registry.toJSON()), /transient_private_value|caller_fixture_secret|api_key|access_token/);

  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `adapter-source-${index}`,
    source_digest: "a".repeat(64),
    request_id: `adapter-request-${index}`,
    metadata: { adapter_test: true },
  }));
  const runtime = new AutonomousEvidenceRuntime({ plan: evidencePlan, journal: new InMemoryAutonomousEvidenceRuntimeJournal() });
  const execution = await runtime.execute(requests, {
    acquirer: registry.createAcquirer(),
    projector: registry.createProjector(),
    evaluator: {
      evaluator_id: "adapter-evaluator",
      evaluator_version: "1.0.0",
      evaluate: () => ({ evaluator_id: "adapter-evaluator", evaluator_version: "1.0.0", verdict: "accepted", score: 1, evidence_digest: "b".repeat(64) }),
    },
  });
  assert.equal(execution.json.status, "completed");
  assert.equal(execution.json.receipts.length, evidencePlan.requirements.length);
  assert.equal(new Set(acquiredDomains).size, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(execution.toJSON()), /transient_private_value|coding-value|science-value/);

  registry.register({
    adapterId: "source.coding.secondary",
    version: "1.0.0",
    domains: ["coding"],
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: () => ({ alternate: true }),
  });
  assert.throws(() => registry.resolve("coding"), /ambiguous/);
  assert.equal(registry.resolve("coding", "source.coding").adapter_id, "source.coding");
  assert.throws(() => registry.resolve("science", "source.coding"), /not registered for science/);
});

test("HTTP evidence adapters execute bounded source calls for every domain and preserve refusal states", async () => {
  const agent = agentFor();
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const registry = new AutonomousEvidenceAdapterRegistry();
  const policy = new AutonomousHttpConnectorPolicy({ allowedHosts: ["127.0.0.1"], requireHttps: false, allowLoopback: true, timeoutMs: 1_000, maxResponseBytes: 64_000 });
  let calls = 0;
  const fakeFetch = async (_url, init) => {
    calls += 1;
    assert.equal(init.redirect, "error");
    assert.equal(init.method, "POST");
    return new Response(JSON.stringify({ source_value: "transient-http-value", accepted: true }), { status: 200, headers: { "content-type": "application/json" } });
  };
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    registerAutonomousHttpEvidenceAdapter(registry, {
      adapterId: `http.${domain}`,
      version: "1.0.0",
      domain,
      capabilities: ["http_json_evidence"],
      policy,
      fetch: fakeFetch,
      endpointResolver: (_manifest, request) => new AutonomousHttpConnectorRequest({ method: "POST", url: "http://127.0.0.1/evidence", body: request }),
      requestForContext: (context) => ({ operation_id: `evidence.${domain}`, subject_digest: "a".repeat(64), requirement_id: context.requirement.requirement_id, source_id: context.request.source_id }),
      headerResolver: () => ({ "X-Caller-Context": "opaque-session" }),
      project: (_value, context) => [{ label: context.requirement.label, kind: "fact", status: "observed", source_digest: context.request.source_digest ?? null, value_digest: null, limitations: ["HTTP source interpretation remains caller-owned"] }],
    });
  }
  const requests = evidencePlan.requirements.map((requirement, index) => ({ requirement_id: requirement.requirement_id, source_id: `http-source-${index}`, source_digest: "b".repeat(64), request_id: `http-request-${index}` }));
  const runtime = new AutonomousEvidenceRuntime({ plan: evidencePlan });
  const result = await runtime.execute(requests, {
    acquirer: registry.createAcquirer(),
    projector: registry.createProjector(),
    evaluator: { evaluator_id: "http-evaluator", evaluator_version: "1.0.0", evaluate: () => ({ evaluator_id: "http-evaluator", evaluator_version: "1.0.0", verdict: "accepted", score: 1, evidence_digest: "c".repeat(64) }) },
  });
  assert.equal(result.json.status, "completed");
  assert.equal(calls, evidencePlan.requirements.length);
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /transient-http-value|opaque-session/);

  const failurePlan = await agent.evidencePlan(["coding"]);
  const failureRegistry = new AutonomousEvidenceAdapterRegistry();
  registerAutonomousHttpEvidenceAdapter(failureRegistry, {
    adapterId: "http.failure",
    version: "1.0.0",
    domain: "coding",
    capabilities: ["http_json_evidence"],
    policy,
    fetch: async () => new Response("forbidden", { status: 403 }),
    endpointResolver: () => new AutonomousHttpConnectorRequest({ method: "GET", url: "http://127.0.0.1/evidence" }),
    requestForContext: () => ({ operation_id: "evidence.coding", subject_digest: "d".repeat(64) }),
  });
  const failed = await new AutonomousEvidenceRuntime({ plan: failurePlan }).execute(failurePlan.requirements.map((requirement, index) => ({ requirement_id: requirement.requirement_id, source_id: `failure-source-${index}` })), { acquirer: failureRegistry.createAcquirer() });
  assert.equal(failed.json.status, "failed");
  assert.equal(failed.json.receipts.every((receipt) => receipt.status === "failed"), true);
});

test("evidence adapter selection ranks explicit signals, abstains conservatively, and rejects registry drift", async () => {
  const registry = new AutonomousEvidenceAdapterRegistry();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    for (const tier of ["fast", "slow"]) {
      registry.register({
        adapterId: `selection.${tier}.${domain}`,
        version: "1.0.0",
        domains: [domain],
        capabilities: ["bounded_evidence"],
        sourceKinds: ["caller_fixture"],
        acquire: () => ({ transient_selection_value: `${tier}-${domain}` }),
        project: (_value, context) => [{ label: context.requirement.label, kind: "fact", status: "observed", value_digest: null }],
      });
    }
  }
  const signals = Object.fromEntries(registry.manifests().map((manifest) => [manifest.adapter_id, manifest.adapter_id.startsWith("selection.fast.")
    ? { eligible: true, health: 0.98, success_rate: 0.96, evaluator_reward: 0.9, latency_ms: 10, cost_units: 1 }
    : { eligible: true, health: 0.42, success_rate: 0.4, evaluator_reward: 0.1, latency_ms: 2_000, cost_units: 200 }]));
  const selector = new AutonomousEvidenceAdapterSelector(registry);
  const plan = selector.selectAdaptiveForDomains(AUTONOMOUS_DOMAIN_NAMES, signals, { capability: "bounded_evidence", minScore: 0.7, minMargin: 0.05 });
  assert.equal(plan.complete, true);
  assert.ok(plan.rows.every((row) => row.adapter_id?.startsWith("selection.fast.")));
  assert.doesNotMatch(JSON.stringify(plan.toJSON()), /transient_selection_value|api_key|access_token/);

  const rehydrated = AutonomousEvidenceAdapterSelectionPlan.fromJSON(plan.toJSON());
  assert.doesNotThrow(() => rehydrated.verify(registry));
  const acquirer = selector.createAcquirerFromSelection(rehydrated);
  const evidencePlan = await agentFor().evidencePlan(["coding"]);
  const requirement = evidencePlan.requirements[0];
  const acquired = await acquirer.acquire({
    plan_digest: evidencePlan.plan_digest,
    requirement,
    request: { requirement_id: requirement.requirement_id, source_id: "selection-source", request_id: "selection-request" },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  });
  assert.equal(acquired.transient_selection_value, "fast-coding");

  const abstained = selector.selectAdaptiveForDomains(["coding"], signals, { capability: "bounded_evidence", minMargin: 1 });
  assert.equal(abstained.complete, false);
  assert.equal(abstained.rows[0].reason, "insufficient_selection_margin");
  assert.throws(() => selector.createAcquirerFromSelection(abstained), /incomplete/);

  registry.register({
    adapterId: "selection.drift.coding",
    version: "1.0.0",
    domains: ["coding"],
    capabilities: ["bounded_evidence"],
    sourceKinds: ["caller_fixture"],
    acquire: () => ({ drift: true }),
  });
  assert.throws(() => rehydrated.verify(registry), /stale or tampered/);
});

test("adapter health persists runtime outcomes and drives per-domain failover without retaining values", async () => {
  const agent = agentFor();
  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const registry = new AutonomousEvidenceAdapterRegistry();
  for (const tier of ["fast", "slow"]) {
    registerAutonomousEvidenceAdaptersForAllDomains(registry, (domain) => ({
      adapterId: `health.${tier}.${domain}`,
      version: "1.0.0",
      capabilities: ["bounded_evidence"],
      sourceKinds: ["caller_fixture"],
      acquire: () => ({ transient_health_value: `${tier}-${domain}` }),
      project: (_value, context) => [{ label: context.requirement.label, kind: "fact", status: "observed", value_digest: null }],
    }));
  }
  const route = Object.fromEntries(AUTONOMOUS_DOMAIN_NAMES.map((domain) => [domain, `health.fast.${domain}`]));
  const store = new InMemoryAutonomousEvidenceAdapterHealthStore({ clock: (() => { let tick = 1_000; return () => tick += 10; })() });
  const controller = new AutonomousEvidenceAdapterHealthController(store, registry);
  const staticPlan = controller.selector.selectForDomains(AUTONOMOUS_DOMAIN_NAMES, { capability: "bounded_evidence" });
  const observedAcquirer = controller.createObservedAcquirerFromSelection(staticPlan, { cost_units_by_adapter: Object.fromEntries(AUTONOMOUS_DOMAIN_NAMES.map((domain) => [`health.fast.${domain}`, 2])) });
  const observedEvaluator = controller.createObservedEvaluatorFromSelection(staticPlan, {
    evaluator_id: "health-evaluator",
    evaluator_version: "1.0.0",
    evaluate: () => ({ evaluator_id: "health-evaluator", evaluator_version: "1.0.0", verdict: "accepted", score: 0.85, evidence_digest: "e".repeat(64) }),
  });
  const requests = evidencePlan.requirements.map((requirement, index) => ({ requirement_id: requirement.requirement_id, source_id: `health-source-${index}`, source_digest: "a".repeat(64), request_id: `health-request-${index}` }));
  const runtime = new AutonomousEvidenceRuntime({ plan: evidencePlan });
  const execution = await runtime.execute(requests, {
    acquirer: observedAcquirer,
    projector: registry.createProjector({ adapterIdForDomain: route }),
    evaluator: observedEvaluator,
  });
  assert.equal(execution.json.status, "completed");
  assert.equal(store.health().length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.ok(store.health().every((row) => row.attempts >= 1 && row.success_rate === 1 && row.quality_observations >= 1));
  assert.doesNotMatch(JSON.stringify(await store.snapshot()), /transient_health_value|api_key|access_token/);

  const codingFast = registry.resolve("coding", route.coding);
  const codingSlow = registry.resolve("coding", "health.slow.coding");
  for (let index = 0; index < 3; index += 1) {
    await store.recordAcquisition({ adapter_id: codingFast.adapter_id, manifest_digest: codingFast.manifest_digest, domain: "coding", outcome: "failure", status: "timeout", latency_ms: 100 + index, failure_class: "timeout" });
  }
  await store.recordAcquisition({ adapter_id: codingSlow.adapter_id, manifest_digest: codingSlow.manifest_digest, domain: "coding", outcome: "success", status: "success", latency_ms: 12 });
  const adaptive = await controller.selectAdaptiveForDomains(AUTONOMOUS_DOMAIN_NAMES, { capability: "bounded_evidence", min_attempts: 3, failure_threshold: 0.75, minScore: 0.1, minMargin: 0 });
  assert.equal(adaptive.complete, true);
  assert.equal(adaptive.rows.find((row) => row.domain === "coding")?.adapter_id, "health.slow.coding");
  assert.ok(adaptive.rows.filter((row) => row.domain !== "coding").every((row) => row.adapter_id?.startsWith("health.fast.")));
  assert.doesNotMatch(JSON.stringify(adaptive.toJSON()), /transient_health_value|api_key|access_token/);

  const persisted = {};
  const coordinator = new AutonomousEvidenceAdapterHealthPersistenceCoordinator(store, {
    read: () => persisted.snapshot ?? null,
    write: (snapshot) => { persisted.snapshot = snapshot; },
  });
  const snapshot = await coordinator.flush();
  const restored = new InMemoryAutonomousEvidenceAdapterHealthStore();
  await new AutonomousEvidenceAdapterHealthPersistenceCoordinator(restored, { read: () => snapshot, write: () => {} }).restore();
  assert.deepEqual(restored.health(), store.health());
  const tampered = structuredClone(snapshot);
  tampered.events[0].observation.adapter_id = "health.tampered";
  await assert.rejects(() => restored.restore(tampered), /snapshot digest mismatch/);
});

test("adapter health JSON persistence is bounded, browser-portable, and CAS-fenced", async () => {
  const source = new InMemoryAutonomousEvidenceAdapterHealthStore({ clock: () => 2_000 });
  for (const [index, domain] of AUTONOMOUS_DOMAIN_NAMES.entries()) {
    await source.recordAcquisition({ adapter_id: `persist.${domain}`, manifest_digest: `${String(index + 1).padStart(2, "0")}${"a".repeat(62)}`, domain, outcome: "success", status: "success", latency_ms: index + 1 });
  }
  const original = await source.snapshot();
  let text = null;
  const textStore = { read: () => text, write: (value) => { text = value; } };
  const jsonPersistence = new JsonAutonomousEvidenceAdapterHealthPersistence(textStore, 32);
  await jsonPersistence.write(original);
  const roundTrip = await jsonPersistence.read();
  assert.equal(roundTrip?.snapshot_digest, original.snapshot_digest);
  assert.equal(roundTrip?.events.length, AUTONOMOUS_DOMAIN_NAMES.length);
  text = "{malformed";
  await assert.rejects(() => jsonPersistence.read(), /invalid JSON/);
  text = JSON.stringify({ ...original, snapshot_digest: "f".repeat(64) });
  await assert.rejects(() => jsonPersistence.read(), /digest/);

  let storageValue = null;
  const webStorage = new WebStorageAutonomousEvidenceAdapterHealthSnapshotTextStore({
    getItem: () => storageValue,
    setItem: (_key, value) => { storageValue = value; },
  });
  webStorage.write(JSON.stringify(original));
  assert.equal(webStorage.read(), JSON.stringify(original));

  let transactionalText = null;
  const transactionalStore = {
    read: () => transactionalText,
    write: (value) => { transactionalText = value; },
    writeIfUnchanged: (expectedDigest, value) => {
      const currentDigest = transactionalText === null ? null : JSON.parse(transactionalText).snapshot_digest;
      if (currentDigest !== expectedDigest) return false;
      transactionalText = value;
      return true;
    },
  };
  const transactionalPersistence = new TransactionalJsonAutonomousEvidenceAdapterHealthPersistence(transactionalStore, 32);
  const writerStore = new InMemoryAutonomousEvidenceAdapterHealthStore({ clock: () => 3_000 });
  await writerStore.restore(original);
  const writer = new AutonomousEvidenceAdapterHealthPersistenceCoordinator(writerStore, transactionalPersistence);
  await writer.flush();
  const staleStore = new InMemoryAutonomousEvidenceAdapterHealthStore({ clock: () => 4_000 });
  const stale = new AutonomousEvidenceAdapterHealthPersistenceCoordinator(staleStore, transactionalPersistence);
  await stale.restore();
  await writerStore.recordAcquisition({ adapter_id: "persist.coding", manifest_digest: `01${"a".repeat(62)}`, domain: "coding", outcome: "failure", status: "timeout", latency_ms: 9, failure_class: "timeout" });
  await writer.flush();
  await staleStore.recordAcquisition({ adapter_id: "persist.science", manifest_digest: `04${"a".repeat(62)}`, domain: "science", outcome: "failure", status: "transport_error", latency_ms: 9, failure_class: "transport_error" });
  await assert.rejects(() => stale.flush(), /stale writer/);
});
