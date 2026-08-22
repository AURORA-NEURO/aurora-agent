import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousWorkflowPortfolioEvidenceController,
  InMemoryAutonomousEvidenceRuntimeJournal,
  InMemoryAutonomousWorkflowPortfolioEvidenceCheckpointStore,
  LLMRuntime,
  validateAutonomousWorkflowPortfolioEvidenceCheckpoint,
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
