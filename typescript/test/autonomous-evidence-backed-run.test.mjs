import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceReadinessPolicy,
  CredentialStore,
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
