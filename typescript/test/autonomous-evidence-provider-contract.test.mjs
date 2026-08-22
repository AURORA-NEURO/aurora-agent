import assert from "node:assert/strict";
import test from "node:test";

import {
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceProviderContractRegistry,
  AutonomousEvidenceReadinessPolicy,
  LLMRuntime,
} from "../dist/index.js";

function registryFor(onAcquire = () => {}) {
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register({
    adapterId: "offline-source",
    version: "1",
    domains: ["coding"],
    capabilities: ["review", "debugging", "implementation", "testing"],
    sourceKinds: ["json"],
    acquire: async (context) => {
      onAcquire(context);
      return { source: "offline", requirement: context.requirement.requirement_id };
    },
  });
  return registry;
}

function contractFor(registry) {
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  contracts.register({
    contractId: "offline.search",
    version: "1",
    provider: "offline-provider",
    protocol: "http_json",
    operations: ["search"],
    domains: ["coding"],
    capabilities: ["review", "debugging", "implementation", "testing"],
    sourceKinds: ["json"],
    authMode: "none",
    freshness: "caller_declared",
    pagination: "none",
    requiredMetadata: ["operation", "query"],
    operationMetadataKey: "operation",
    adapterId: "offline-source",
  });
  return contracts;
}

function agentFor() {
  return new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } }));
}

test("provider contracts bind adapter identity and enforce request semantics before dispatch", async () => {
  let dispatches = 0;
  const registry = registryFor(() => { dispatches += 1; });
  const contracts = contractFor(registry);
  const projection = contracts.toJSON();
  assert.equal(projection.contracts.length, 1);
  assert.equal(projection.coverage.find((row) => row.domain === "coding").state, "complete");

  const acquirer = contracts.createAcquirerForAdapter("offline-source", "coding");
  const agent = agentFor();
  const plan = await agent.evidencePlan(["coding"]);
  const requirement = plan.requirements[0];
  const baseContext = {
    plan_digest: plan.plan_digest,
    requirement,
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };

  await assert.rejects(
    () => acquirer.acquire({ ...baseContext, request: { requirement_id: requirement.requirement_id, source_id: "missing-metadata", metadata: { operation: "search" } } }),
    /missing required metadata: query/,
  );
  await assert.rejects(
    () => acquirer.acquire({ ...baseContext, request: { requirement_id: requirement.requirement_id, source_id: "bad-operation", metadata: { operation: "retrieve", query: "offline" } } }),
    /operation is not declared/,
  );
  await acquirer.acquire({ ...baseContext, request: { requirement_id: requirement.requirement_id, source_id: "valid", metadata: { operation: "search", query: "offline" } } });
  assert.equal(dispatches, 1);
});

test("AutonomousAgent exposes provider-contract binding through reviewed preparation and execution", async () => {
  let dispatches = 0;
  const registry = registryFor(() => { dispatches += 1; });
  const contracts = contractFor(registry);
  const agent = agentFor();
  const readinessPolicy = new AutonomousEvidenceReadinessPolicy({ requireHealth: false });
  const prepared = await agent.prepareReviewedEvidence(registry, ["coding"], { providerContracts: contracts, readinessPolicy, allowDegradedDispatch: true });
  assert.equal(prepared.status, "ready_for_review");
  assert.equal(typeof prepared.provider_contract_registry_digest, "string");

  const evidencePlan = await agent.evidencePlan(["coding"]);
  const requests = evidencePlan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `offline-${index}`,
    metadata: { operation: "search", query: "offline" },
  }));
  const result = await agent.executeReviewedEvidence(registry, ["coding"], requests, {
    prepare: { providerContracts: contracts, readinessPolicy, allowDegradedDispatch: true },
    execute: {
      approveSourceDispatch: true,
      projector: { project: (_value, context) => [{ label: context.requirement.requirement_id }] },
    },
  });
  assert.equal(result.plan.provider_contract_registry_digest, prepared.provider_contract_registry_digest);
  assert.equal(dispatches, requests.length);
  assert.equal(result.runtime.toJSON().receipts.length, requests.length);
});

test("provider contract execution rejects stale adapter catalogues and capability gaps", async () => {
  const registry = registryFor();
  const contracts = contractFor(registry);
  registry.register({
    adapterId: "offline-source",
    version: "2",
    domains: ["coding"],
    capabilities: ["review"],
    sourceKinds: ["json"],
    acquire: async () => ({ source: "replacement" }),
  }, { replace: true });
  assert.throws(() => contracts.verify(), /adapter registry is stale or tampered/);
});
