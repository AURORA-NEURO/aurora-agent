import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAcquisitionError,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceProviderContractRegistry,
  AutonomousEvidenceReadinessPolicy,
  LLMRuntime,
  createAutonomousLLMEvidenceAdapterRegistration,
  registerAutonomousLLMEvidenceAdapter,
} from "../dist/index.js";

function offlineRuntime(onRequest, onCredential = () => {}) {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("offline-evidence", (request) => {
    onRequest(request);
    return {
      structured: {
        answer: "offline provider answer must remain transient",
        requirement_id: request.messages.at(-1)?.content ?? "unknown",
      },
    };
  });
  return { runtime, onCredential };
}

function adapterOptions(runtime, domain, onCredential = () => {}, extra = {}) {
  return {
    adapterId: `llm-evidence-${domain}`,
    version: "1.0.0",
    domain,
    provider: "offline-evidence",
    runtime,
    model: "offline-model",
    capabilities: [
      "bounded_evidence", "review", "debugging", "implementation", "testing", "analysis",
      "web_research", "navigation", "source_comparison", "schema_validation", "lineage", "quality_control", "data_analysis",
      "hypothesis", "literature", "statistics", "experiment", "reproducibility", "biomedical_review", "safety_boundary", "provenance", "human_review",
      "neuroscience_analysis", "signal_interpretation", "study_design", "observability", "incident_response", "risk_review", "rollback", "approval", "runbook",
      "workflow", "coordination", "governance", "compliance", "analytics", "delegation", "consensus", "conflict_resolution", "handoff",
      "document", "cross_modal_alignment", "image", "audio", "video", "routing", "synthesis", "evidence_alignment", "workflow_composition",
      "rubric", "benchmarking", "replay", "failure_analysis",
    ],
    sourceKinds: ["llm_structured"],
    credentialFor: (provider, context) => {
      onCredential(provider, context);
      return undefined;
    },
    promptForContext: (context) => [{ role: "user", content: `evidence requirement ${context.requirement.requirement_id}` }],
    requireJson: true,
    responseSchema: { type: "object", required: ["answer", "requirement_id"], properties: { answer: { type: "string" }, requirement_id: { type: "string" } }, additionalProperties: false },
    project: (_value, context) => [{ label: context.requirement.requirement_id, kind: "fact", status: "observed" }],
    ...extra,
  };
}

function requestsFor(plan) {
  return plan.requirements.map((requirement, index) => ({
    requirement_id: requirement.requirement_id,
    source_id: `llm-source-${index}`,
    request_id: `llm-request-${index}`,
    metadata: { operation: "analyze" },
  }));
}

test("LLM evidence adapters invoke the provider-neutral runtime across every autonomous domain", async () => {
  const requests = [];
  let credentialLookups = 0;
  const { runtime } = offlineRuntime((request) => requests.push(request));
  const registry = new AutonomousEvidenceAdapterRegistry();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    registerAutonomousLLMEvidenceAdapter(registry, adapterOptions(runtime, domain, () => { credentialLookups += 1; }));
  }
  const contracts = new AutonomousEvidenceProviderContractRegistry(registry);
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    contracts.register({
      contractId: `offline.${domain}`,
      version: "1",
      provider: "offline-evidence",
      protocol: "openai_responses",
      operations: ["analyze"],
      domains: [domain],
      capabilities: [
        "bounded_evidence", "review", "debugging", "implementation", "testing", "analysis",
        "web_research", "navigation", "source_comparison", "schema_validation", "lineage", "quality_control", "data_analysis",
        "hypothesis", "literature", "statistics", "experiment", "reproducibility", "biomedical_review", "safety_boundary", "provenance", "human_review",
        "neuroscience_analysis", "signal_interpretation", "study_design", "observability", "incident_response", "risk_review", "rollback", "approval", "runbook",
        "workflow", "coordination", "governance", "compliance", "analytics", "delegation", "consensus", "conflict_resolution", "handoff",
        "document", "cross_modal_alignment", "image", "audio", "video", "routing", "synthesis", "evidence_alignment", "workflow_composition",
        "rubric", "benchmarking", "replay", "failure_analysis",
      ],
      sourceKinds: ["llm_structured"],
      authMode: "none",
      freshness: "caller_declared",
      pagination: "none",
      requiredMetadata: ["operation"],
      operationMetadataKey: "operation",
      adapterId: `llm-evidence-${domain}`,
    });
  }

  const agent = new AutonomousAgent(runtime);
  const plan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const result = await agent.executeReviewedEvidence(registry, AUTONOMOUS_DOMAIN_NAMES, requestsFor(plan), {
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
      providerContracts: contracts,
    },
    execute: {
      approveSourceDispatch: true,
      projector: registry.createProjector(),
      evaluator: {
        evaluator_id: "offline-evidence-evaluator",
        evaluator_version: "1",
        evaluate: () => ({ evaluator_id: "offline-evidence-evaluator", evaluator_version: "1", verdict: "accepted", score: 1 }),
      },
    },
  });

  assert.equal(result.status, "completed");
  assert.equal(requests.length, plan.requirements.length);
  assert.equal(credentialLookups, plan.requirements.length);
  assert.equal(new Set(requests.map((request) => request.model)).size, 1);
  assert.ok(requests.every((request) => request.requireJson === true && /^[0-9a-f]{64}$/.test(request.idempotencyKey)));
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /offline provider answer|evidence requirement/);
});

test("LLM evidence adapters preserve opaque credential lookup and convert provider failures into typed retry metadata", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  runtime.registerInMemoryProvider("offline-failure", () => {
    throw new Error("provider payload must not escape");
  });
  let credentialCalls = 0;
  const registration = createAutonomousLLMEvidenceAdapterRegistration(adapterOptions(runtime, "coding", (provider, context) => {
    credentialCalls += 1;
    assert.equal(provider, "offline-failure");
    assert.equal(context.requirement.domain, "coding");
  }, { provider: "offline-failure", parseResponse: () => { throw new Error("parser failure"); } }));
  const context = {
    plan_digest: "a".repeat(64),
    requirement: {
      schema: "bioprism-typescript-autonomous-evidence-requirement/0.1",
      requirement_id: "coding:scope:scope",
      domain: "coding",
      workflow_id: "coding_delivery",
      workflow_digest: "b".repeat(64),
      stage_id: "scope",
      label: "scope",
      objective: "bound",
      required_capabilities: ["review"],
      evaluator_signals: ["schema_valid"],
      depends_on: [],
    },
    request: { requirement_id: "coding:scope:scope", source_id: "source-1" },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };

  await assert.rejects(() => registration.acquire(context), AutonomousEvidenceAcquisitionError);
  assert.equal(credentialCalls, 1);
});

test("LLM evidence adapter registration rejects ambiguous model configuration and unsafe structured contracts", async () => {
  const runtime = new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } });
  assert.throws(() => createAutonomousLLMEvidenceAdapterRegistration(adapterOptions(runtime, "coding", undefined, { model: "one", modelForContext: () => "two" })), /model or modelForContext|both/);
  assert.throws(() => createAutonomousLLMEvidenceAdapterRegistration(adapterOptions(runtime, "coding", undefined, { requireJson: false, responseSchema: { type: "object" } })), /responseSchema requires requireJson=true/);
  assert.throws(() => createAutonomousLLMEvidenceAdapterRegistration(adapterOptions(runtime, "coding", undefined, { requireJson: undefined, responseSchema: { type: "object" } })), /responseSchema requires requireJson=true/);
  assert.throws(() => createAutonomousLLMEvidenceAdapterRegistration(adapterOptions(runtime, "coding", undefined, { maxOutputTokens: 0 })), /maxOutputTokens/);
});
