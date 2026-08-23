import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceAdapterSelector,
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceProviderContractRegistry,
  AutonomousEvidenceReadinessPolicy,
  AutonomousEvidenceSourceLedger,
  AutonomousEvidenceSourcePolicy,
  AutonomousEvidenceSourceLedgerWebStorage,
  JsonAutonomousEvidenceSourceLedgerPersistence,
  InMemoryAutonomousEvidenceSourceLedgerPersistence,
  TransactionalJsonAutonomousEvidenceSourceLedgerPersistence,
  LLMRuntime,
  createAutonomousEvidenceSourceAcquirer,
  createAutonomousEvidenceAdapterFailoverAcquirer,
  digestJsonSync,
} from "../dist/index.js";

const ALL_DOMAIN_CAPABILITIES = [
  "implementation", "debugging", "testing", "review", "web_research", "source_comparison", "navigation",
  "data_analysis", "schema_validation", "lineage", "quality_control", "literature", "hypothesis", "experiment",
  "statistics", "reproducibility", "biomedical_review", "provenance", "safety_boundary", "human_review",
  "neuroscience_analysis", "signal_interpretation", "study_design", "runbook", "incident_response", "observability",
  "risk_review", "rollback", "approval", "workflow", "governance", "compliance", "analytics", "coordination",
  "delegation", "consensus", "handoff", "conflict_resolution", "image", "audio", "video", "document",
  "cross_modal_alignment", "routing", "synthesis", "evidence_alignment", "workflow_composition", "benchmarking",
  "rubric", "replay", "failure_analysis", "evidence_acquisition_discovery", "evidence_source_execution",
];

function runtime() {
  return new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
}

function contextFor(plan, requirement, sourceId, sourceDigest, metadata = { operation: "observe" }) {
  return {
    plan_digest: plan.plan_digest,
    requirement,
    request: { requirement_id: requirement.requirement_id, source_id: sourceId, source_digest: sourceDigest, metadata },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };
}

async function populatePortableSourceLedger(persistence, now = 1_700_000_000_000) {
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({
    adapterId: "portable-source-fixture",
    version: "1",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ALL_DOMAIN_CAPABILITIES,
    sourceKinds: ["json"],
    acquire: async (context) => ({ domain: context.requirement.domain, transient_payload: `portable-${context.request.source_id}` }),
  });
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "portable.source",
    version: "1",
    provider: "offline-fixture",
    protocol: "http_json",
    operations: ["observe"],
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ALL_DOMAIN_CAPABILITIES,
    sourceKinds: ["json"],
    authMode: "none",
    freshness: "realtime",
    pagination: "none",
    requiredMetadata: ["operation"],
    operationMetadataKey: "operation",
    adapterId: "portable-source-fixture",
  });
  const agent = new AutonomousAgent(runtime());
  const ledger = new AutonomousEvidenceSourceLedger(persistence);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    const requirement = plan.requirements[0];
    const sourceId = `portable-source-${domain}`;
    const sourceDigest = digestJsonSync({ sourceId, revision: "1" });
    const acquirer = createAutonomousEvidenceSourceAcquirer({
      providerContracts: contracts,
      adapterId: "portable-source-fixture",
      domain,
      policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: 60_000 }),
      ledger,
      describeSource: ({ now_ms }) => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now_ms }),
    });
    await acquirer.acquire(contextFor(plan, requirement, sourceId, sourceDigest));
  }
  return { agent, contracts, ledger };
}

test("source boundary admits fresh provider observations across every autonomous domain and persists only metadata", async () => {
  const now = 1_700_000_000_000;
  const registry = new AutonomousEvidenceAdapterRegistry();
  let dispatches = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    registry.register({
      adapterId: `fixture-${domain}`,
      version: "1",
      domains: [domain],
      capabilities: ALL_DOMAIN_CAPABILITIES,
      sourceKinds: ["json"],
      acquire: async (context) => {
        dispatches += 1;
        return { domain, transient_payload: `payload-${context.request.source_id}`, requirement: context.requirement.requirement_id };
      },
    });
  }
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    contracts.register({
      contractId: `fixture.${domain}`,
      version: "1",
      provider: "offline-fixture",
      protocol: "http_json",
      operations: ["observe"],
      domains: [domain],
      capabilities: ALL_DOMAIN_CAPABILITIES,
      sourceKinds: ["json"],
      authMode: "none",
      freshness: "realtime",
      pagination: "none",
      requiredMetadata: ["operation"],
      operationMetadataKey: "operation",
      adapterId: `fixture-${domain}`,
    });
  }

  const persistence = new InMemoryAutonomousEvidenceSourceLedgerPersistence();
  const ledger = new AutonomousEvidenceSourceLedger(persistence);
  const policy = new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: 60_000 });
  const agent = new AutonomousAgent(runtime());
  const sourceDigests = new Map();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const plan = await agent.evidencePlan([domain]);
    const requirement = plan.requirements[0];
    const sourceId = `fixture-source-${domain}`;
    const sourceDigest = digestJsonSync({ source: sourceId, revision: "1" });
    sourceDigests.set(domain, sourceDigest);
    const acquirer = createAutonomousEvidenceSourceAcquirer({
      providerContracts: contracts,
      adapterId: `fixture-${domain}`,
      domain,
      policy,
      ledger,
      describeSource: ({ now_ms }) => ({
        authority: "provider_observed",
        status: "observed",
        sourceDigest,
        observedAtMs: now_ms,
        citationDigest: digestJsonSync({ sourceId, domain }),
        limitations: ["offline fixture;not a production source"],
      }),
    });
    const value = await acquirer.acquire(contextFor(plan, requirement, sourceId, sourceDigest));
    assert.equal(value.domain, domain);
  }

  assert.equal(dispatches, AUTONOMOUS_DOMAIN_NAMES.length);
  const snapshot = ledger.toJSON();
  assert.equal(snapshot.entries.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(snapshot.entries.every((entry) => entry.receipt.decision === "accepted"), true);
  assert.equal(snapshot.entries.every((entry) => !JSON.stringify(entry).includes("transient_payload")), true);
  assert.equal(snapshot.entries.every((entry) => entry.receipt.source_digest === sourceDigests.get(entry.receipt.domain)), true);
  assert.equal(snapshot.entries.at(-1).entry_digest, snapshot.head_digest);
  assert.equal(snapshot.ledger_digest.length, 64);

  const restored = new AutonomousEvidenceSourceLedger(persistence);
  const restoreResult = await restored.restore();
  assert.equal(restoreResult.restored, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(restored.toJSON(), snapshot);
});

test("source ledger JSON, CAS, and browser persistence restore all domains and fail closed on tampering", async () => {
  let encoded = null;
  const textStore = {
    read: () => encoded,
    write: (value) => { encoded = value; },
  };
  const jsonPersistence = new JsonAutonomousEvidenceSourceLedgerPersistence(textStore);
  const populated = await populatePortableSourceLedger(jsonPersistence);
  const snapshot = populated.ledger.toJSON();
  assert.equal(snapshot.entries.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal((await jsonPersistence.records()).length, AUTONOMOUS_DOMAIN_NAMES.length);

  const restored = new AutonomousEvidenceSourceLedger(jsonPersistence);
  assert.equal((await restored.restore()).restored, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(restored.toJSON(), snapshot);

  const tampered = JSON.parse(encoded);
  tampered.entries[0].receipt.source_id = "tampered-source";
  encoded = JSON.stringify(tampered);
  await assert.rejects(() => restored.restore(), /digest|chain|invalid/);

  let casEncoded = null;
  let rejectNext = false;
  const casStore = {
    read: () => casEncoded,
    write: () => { throw new Error("CAS store must not use unconditional writes"); },
    writeIfUnchanged: (expected, value) => {
      if (rejectNext) { rejectNext = false; return false; }
      const current = casEncoded === null ? null : JSON.parse(casEncoded).ledger_digest;
      if (current !== expected) return false;
      casEncoded = value;
      return true;
    },
  };
  const casPersistence = new TransactionalJsonAutonomousEvidenceSourceLedgerPersistence(casStore);
  const casPopulated = await populatePortableSourceLedger(casPersistence);
  const casSnapshot = casPopulated.ledger.toJSON();
  assert.equal(casSnapshot.entries.length, AUTONOMOUS_DOMAIN_NAMES.length);
  const last = casSnapshot.entries.at(-1);
  const nextReceiptBase = {
    ...last.receipt,
    request_digest: digestJsonSync({ request: "cas-extra" }),
    source_id: "cas-extra-source",
    source_digest: digestJsonSync({ source: "cas-extra-source" }),
    value_digest: digestJsonSync({ value: "cas-extra" }),
  };
  const { receipt_digest: _receiptDigest, ...nextReceiptDescriptor } = nextReceiptBase;
  const nextReceipt = { ...nextReceiptBase, receipt_digest: digestJsonSync(nextReceiptDescriptor) };
  const nextEntryBase = {
    schema: "bioprism-typescript-autonomous-evidence-source-ledger-entry/0.1",
    sequence: last.sequence + 1,
    previous_entry_digest: last.entry_digest,
    receipt: nextReceipt,
    retention: "metadata_only;raw_source_values_excluded",
    secret_material: "never_returned",
  };
  const nextEntry = { ...nextEntryBase, entry_digest: digestJsonSync(nextEntryBase) };
  rejectNext = true;
  await assert.rejects(() => casPersistence.append(nextEntry), /stale writer/);

  let storageValue = null;
  const storage = {
    getItem: () => storageValue,
    setItem: (_key, value) => { storageValue = value; },
  };
  const browserPersistence = new JsonAutonomousEvidenceSourceLedgerPersistence(new AutonomousEvidenceSourceLedgerWebStorage(storage, "aurora-source-ledger"));
  await browserPersistence.append(casSnapshot.entries[0]);
  assert.equal((await browserPersistence.records()).length, 1);
});

test("source boundary fails closed on stale, unverified, future, and mismatched source observations", async () => {
  const now = 2_000;
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({
    adapterId: "source-fixture",
    version: "1",
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json"],
    acquire: async () => ({ answer: "transient" }),
  });
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "source.fixture",
    version: "1",
    provider: "offline-fixture",
    protocol: "http_json",
    operations: ["observe"],
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json"],
    authMode: "none",
    freshness: "bounded_cache",
    pagination: "none",
    requiredMetadata: ["operation"],
    operationMetadataKey: "operation",
    adapterId: "source-fixture",
  });
  const agent = new AutonomousAgent(runtime());
  const plan = await agent.evidencePlan(["coding"]);
  const requirement = plan.requirements[0];
  const sourceDigest = digestJsonSync({ source: "fixture" });
  const base = { plan, requirement, sourceId: "bounded-source", sourceDigest };

  const staleLedger = new AutonomousEvidenceSourceLedger();
  const stale = createAutonomousEvidenceSourceAcquirer({
    providerContracts: contracts,
    adapterId: "source-fixture",
    domain: "coding",
    policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: null }),
    ledger: staleLedger,
    describeSource: () => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: 0, expiresAtMs: 1_000 }),
  });
  await assert.rejects(() => stale.acquire(contextFor(plan, requirement, base.sourceId, base.sourceDigest)), /source admission stale/);
  assert.equal(staleLedger.records()[0].receipt.decision, "stale");

  const unverified = createAutonomousEvidenceSourceAcquirer({
    providerContracts: contracts,
    adapterId: "source-fixture",
    domain: "coding",
    policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: null }),
    describeSource: () => ({ authority: "caller_declared", status: "observed", observedAtMs: now, expiresAtMs: now + 1_000 }),
  });
  await assert.rejects(() => unverified.acquire(contextFor(plan, requirement, base.sourceId, null)), /source admission unverified/);

  const permittedUnverified = createAutonomousEvidenceSourceAcquirer({
    providerContracts: contracts,
    adapterId: "source-fixture",
    domain: "coding",
    policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: null, allowUnverified: true }),
    describeSource: () => ({ authority: "caller_declared", status: "observed", observedAtMs: now, expiresAtMs: now + 1_000 }),
  });
  assert.deepEqual(await permittedUnverified.acquire(contextFor(plan, requirement, base.sourceId, null)), { answer: "transient" });

  const futureLedger = new AutonomousEvidenceSourceLedger();
  const future = createAutonomousEvidenceSourceAcquirer({
    providerContracts: contracts,
    adapterId: "source-fixture",
    domain: "coding",
    policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: null }),
    ledger: futureLedger,
    describeSource: () => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now + 100_000, expiresAtMs: now + 200_000 }),
  });
  await assert.rejects(() => future.acquire(contextFor(plan, requirement, base.sourceId, base.sourceDigest)), /source admission refused/);
  assert.deepEqual(futureLedger.records()[0].receipt.decision_reasons, ["observed_at_is_in_the_future"]);

  const mismatch = createAutonomousEvidenceSourceAcquirer({
    providerContracts: contracts,
    adapterId: "source-fixture",
    domain: "coding",
    policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: null }),
    describeSource: () => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now, expiresAtMs: now + 1_000 }),
  });
  await assert.rejects(() => mismatch.acquire(contextFor(plan, requirement, base.sourceId, digestJsonSync({ other: true }))), /source_digest does not match/);
});

test("source ledger rejects tampered chains and source adapters require explicit multi-kind selection", async () => {
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({ adapterId: "multi-source", version: "1", domains: ["coding"], capabilities: ["review"], sourceKinds: ["json", "table"], acquire: async () => ({ ok: true }) });
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "multi.source",
    version: "1",
    provider: "offline-fixture",
    protocol: "caller_defined",
    operations: ["observe"],
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json", "table"],
    authMode: "none",
    freshness: "historical",
    pagination: "none",
    adapterId: "multi-source",
  });
  const descriptor = { describeSource: () => ({ authority: "human_verified", status: "observed", sourceDigest: digestJsonSync({ source: "historical" }), observedAtMs: 1 }) };
  assert.throws(() => createAutonomousEvidenceSourceAcquirer({ providerContracts: contracts, adapterId: "multi-source", domain: "coding", ...descriptor }), /sourceKind/);
  const persistence = new InMemoryAutonomousEvidenceSourceLedgerPersistence();
  const ledger = new AutonomousEvidenceSourceLedger(persistence);
  const acquirer = createAutonomousEvidenceSourceAcquirer({ providerContracts: contracts, adapterId: "multi-source", domain: "coding", sourceKind: "table", ledger, ...descriptor });
  const agent = new AutonomousAgent(runtime());
  const plan = await agent.evidencePlan(["coding"]);
  await acquirer.acquire(contextFor(plan, plan.requirements[0], "historical", digestJsonSync({ source: "historical" })));

  const tamperedEntries = persistence.records().map((entry) => ({ ...entry, entry_digest: "f".repeat(64) }));
  const tamperedPersistence = { append: (entry) => entry, records: () => tamperedEntries };
  const tamperedLedger = new AutonomousEvidenceSourceLedger(tamperedPersistence);
  await assert.rejects(() => tamperedLedger.restore(), /digest|malformed/);
});

test("reviewed failover applies the source boundary inside the selected candidate route", async () => {
  const now = 10_000;
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({ adapterId: "failover-source", version: "1", domains: ["coding"], capabilities: ["review"], sourceKinds: ["json"], acquire: async () => ({ answer: "failover" }) });
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "failover.source",
    version: "1",
    provider: "offline-fixture",
    protocol: "http_json",
    operations: ["observe"],
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json"],
    authMode: "none",
    freshness: "realtime",
    pagination: "none",
    requiredMetadata: ["operation"],
    operationMetadataKey: "operation",
    adapterId: "failover-source",
  });
  const agent = new AutonomousAgent(runtime());
  const plan = await agent.evidencePlan(["coding"]);
  const selector = new AutonomousEvidenceAdapterSelector(registry);
  const selection = selector.selectForDomains(["coding"]);
  const ledger = new AutonomousEvidenceSourceLedger();
  const sourceDigest = digestJsonSync({ source: "failover" });
  const acquirer = createAutonomousEvidenceAdapterFailoverAcquirer(registry, selection, {
    providerContracts: contracts,
    sourceBoundary: {
      policy: new AutonomousEvidenceSourcePolicy({ now: () => now }),
      ledger,
      describeSource: ({ now_ms }) => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now_ms }),
    },
  });
  const requirement = plan.requirements[0];
  const value = await acquirer.acquire(contextFor(plan, requirement, "failover-source-id", sourceDigest));
  assert.deepEqual(value, { answer: "failover" });
  assert.equal(ledger.records()[0].receipt.adapter_id, "failover-source");
  assert.equal(ledger.records()[0].receipt.decision, "accepted");
});

test("reviewed execution binds the source policy digest before dispatch", async () => {
  const now = 20_000;
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({ adapterId: "reviewed-source", version: "1", domains: ["coding"], capabilities: ["review"], sourceKinds: ["json"], acquire: async () => ({ answer: "reviewed" }) });
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "reviewed.source",
    version: "1",
    provider: "offline-fixture",
    protocol: "http_json",
    operations: ["observe"],
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json"],
    authMode: "none",
    freshness: "realtime",
    pagination: "none",
    requiredMetadata: ["operation"],
    operationMetadataKey: "operation",
    adapterId: "reviewed-source",
  });
  const agent = new AutonomousAgent(runtime());
  const plan = await agent.evidencePlan(["coding"]);
  const sourcePolicy = new AutonomousEvidenceSourcePolicy({ now: () => now });
  const controller = new AutonomousEvidenceExecutionController(registry);
  const executionPlan = await controller.prepare(plan, {
    providerContracts: contracts,
    readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
    allowDegradedDispatch: true,
    sourceBoundary: { policy: sourcePolicy, sourceKind: "json" },
  });
  assert.equal(executionPlan.toJSON().source_policy_digest, sourcePolicy.policy_digest);
  assert.equal(executionPlan.toJSON().source_kind, "json");
  const sourceDigest = digestJsonSync({ source: "reviewed" });
  const ledger = new AutonomousEvidenceSourceLedger();
  const result = await controller.execute(executionPlan, plan, [{ requirement_id: plan.requirements[0].requirement_id, source_id: "reviewed-source-id", source_digest: sourceDigest, metadata: { operation: "observe" } }], {
    approveSourceDispatch: true,
    providerContracts: contracts,
    sourceBoundary: {
      policy: sourcePolicy,
      sourceKind: "json",
      ledger,
      describeSource: ({ now_ms }) => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now_ms }),
    },
    projector: { project: (_value, context) => [{ label: context.requirement.requirement_id }] },
  });
  assert.equal(result.plan.source_policy_digest, sourcePolicy.policy_digest);
  assert.equal(ledger.records()[0].receipt.policy_digest, sourcePolicy.policy_digest);
  await assert.rejects(() => controller.execute(executionPlan, plan, [{ requirement_id: plan.requirements[0].requirement_id, source_id: "reviewed-source-id", source_digest: sourceDigest, metadata: { operation: "observe" } }], {
    approveSourceDispatch: true,
    providerContracts: contracts,
    sourceBoundary: {
      policy: new AutonomousEvidenceSourcePolicy({ now: () => now, maxAgeMs: 1_000 }),
      sourceKind: "json",
      describeSource: ({ now_ms }) => ({ authority: "provider_observed", status: "observed", sourceDigest, observedAtMs: now_ms }),
    },
  }), /source boundary policy changed after planning/);
});
