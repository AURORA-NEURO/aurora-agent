import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousBrainFacade,
  AutonomousClaimIntegrityClaim,
  AutonomousClaimIntegrityEvidence,
  AutonomousEvidenceAdapterRegistry,
  AutonomousEvidenceReadinessPolicy,
  AutonomousInformationAcquisitionCandidate,
  AutonomousInformationAcquisitionObservation,
  InMemoryAutonomousEvidenceExecutionCheckpointStore,
  LLMRuntime,
  builtinAutonomousDomainProfiles,
  digestJsonSync,
} from "../dist/index.js";

const REFERENCE_TIME = "2026-08-26T12:00:00Z";

function claimFor(domain) {
  const claimId = `facade-claim-${domain}`;
  return new AutonomousClaimIntegrityClaim({
    claimId,
    domain,
    claimDigest: digestJsonSync({ claim_id: claimId, domain }),
    requiredSupport: 0.5,
  });
}

function evidenceFor(claim, domain) {
  const evidenceId = `facade-evidence-${domain}`;
  return new AutonomousClaimIntegrityEvidence({
    evidenceId,
    domain,
    claimIds: [claim.claimId],
    sourceId: `facade-source-${domain}`,
    sourceDigest: digestJsonSync({ source: domain }),
    evidenceDigest: digestJsonSync({ evidence: evidenceId }),
    observedAt: "2026-08-25T12:00:00Z",
    reliability: 0.95,
    support: 0.95,
    stance: "support",
    modality: "primary",
    reproducibility: "reproduced",
    status: "accepted",
  });
}

function candidateFor(domain, claimId) {
  return new AutonomousInformationAcquisitionCandidate({
    candidateId: `facade-acquisition-${domain}`,
    domain,
    capability: "evidence_acquisition",
    sourceId: `facade-source-${domain}`,
    informationGain: 0.8,
    uncertaintyReduction: 0.8,
    reliability: 0.95,
    freshness: 0.95,
    coverage: 0.9,
    cost: 0.1,
    latencyMs: 100,
    risk: 0.02,
    conflictRisk: 0.02,
    priority: 0.8,
    metadata: { claim_ids: [claimId] },
  });
}

async function approvedLaunchAdmission(brain) {
  const profiles = await builtinAutonomousDomainProfiles();
  const ready = { configured: true, operational: true, restart_safe: true, integrity_fenced: true, caller_owned: true };
  const preflight = await brain.launchPreflight({
    availableToolNames: profiles.flatMap((profile) => profile.tool_profile.bindings.map((binding) => binding.name)),
    availableEvidence: profiles.flatMap((profile) => profile.workflow.stages.flatMap((stage) => stage.evidence_outputs.map((label) => `${profile.domain}:${stage.id}:${label}`))),
    deploymentCapabilities: {
      persistence: ready,
      queue: ready,
      approval_authority: ready,
      external_auth: ready,
      telemetry: ready,
    },
  });
  return brain.admitLaunchPreflight(preflight, { decision: "approve", authorizationDigest: "a".repeat(64) });
}

function registerEvidenceAdapter(registry, calls) {
  registry.register({
    adapterId: "facade-integrity-all-domains",
    version: "1.0.0",
    domains: AUTONOMOUS_DOMAIN_NAMES,
    capabilities: ["evidence_acquisition"],
    sourceKinds: ["caller_fixture"],
    acquire: async (context) => {
      calls.count += 1;
      return { transient_source_value: `value-${calls.count}`, domain: context.requirement.domain };
    },
  });
}

function executionOptions() {
  return {
    projector: {
      project: (_value, context) => [{ label: context.requirement.label, kind: "fact", status: "observed" }],
    },
    evaluator: {
      evaluator_id: "facade-integrity-evaluator",
      evaluator_version: "1.0.0",
      evaluate: () => ({
        evaluator_id: "facade-integrity-evaluator",
        evaluator_version: "1.0.0",
        verdict: "accepted",
        score: 1,
        evidence_digest: "d".repeat(64),
      }),
    },
    sleep: async () => {},
  };
}

test("brain facade composes information planning, claim integrity, and resumable acquisition across every domain", async () => {
  const runtime = new LLMRuntime();
  runtime.registerInMemoryProvider("offline", () => ({ output_text: "unused" }));
  const agent = new AutonomousAgent(runtime);
  agent.registerModel({
    provider: "offline",
    model: "offline-model",
    capabilities: ["reasoning", "structured_output", "code", "web", "data", "science", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation"],
    context_window_tokens: 32_000,
    max_output_tokens: 2_000,
    quality: 0.9,
    latency_ms: 10,
    cost_per_million_tokens: 0,
    reliability: 0.99,
  });
  const brain = new AutonomousBrainFacade({ agent });
  const claims = AUTONOMOUS_DOMAIN_NAMES.map(claimFor);
  const candidates = claims.map((claim) => candidateFor(claim.domain, claim.claimId));

  const plan = await brain.planInformationAcquisition("private facade acquisition task must remain transient", {
    domains: AUTONOMOUS_DOMAIN_NAMES,
    candidates,
    policy: { maxCost: 2, maxItems: AUTONOMOUS_DOMAIN_NAMES.length, requireDomainCoverage: true, exploration: 0 },
  });
  assert.equal(plan.status, "ready");
  assert.deepEqual(plan.selectedDomains, AUTONOMOUS_DOMAIN_NAMES);
  assert.equal(plan.selected.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(plan.toJSON()), /private facade acquisition task/);
  assert.equal(brain.validateInformationAcquisitionPlan(plan).planDigest, plan.planDigest);

  const replanned = brain.replanInformationAcquisition({
    previousPlan: plan,
    candidates,
    observations: [new AutonomousInformationAcquisitionObservation({
      candidateId: candidates[0].candidateId,
      status: "accepted",
      valueDigest: "b".repeat(64),
      evaluatorDigest: "c".repeat(64),
    })],
  });
  assert.equal(replanned.generation, 2);
  assert.equal(replanned.priorPlanDigest, plan.planDigest);
  assert.ok(replanned.observationsDigest);

  const blocked = brain.assessClaimIntegrity("private claim context must remain transient", { claims, evidence: [], referenceTime: REFERENCE_TIME });
  assert.equal(blocked.status, "blocked");
  assert.equal(blocked.actions.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(blocked.toJSON()), /private claim context/);
  assert.equal(brain.validateClaimIntegrity(blocked).assessmentDigest, blocked.assessmentDigest);

  const recovered = brain.reassessClaimIntegrity(blocked, {
    claims,
    evidence: claims.map((claim) => evidenceFor(claim, claim.domain)),
    referenceTime: REFERENCE_TIME,
  });
  assert.equal(recovered.status, "ready");
  assert.equal(recovered.generation, 2);
  assert.equal(recovered.priorAssessmentDigest, blocked.assessmentDigest);

  const bridge = brain.planClaimIntegrityAcquisition(blocked, {
    candidates,
    policy: { maxCost: 2, maxItems: AUTONOMOUS_DOMAIN_NAMES.length, requireDomainCoverage: true, exploration: 0 },
  });
  assert.equal(bridge.status, "planned");
  assert.deepEqual(bridge.acquisitionPlan.selectedDomains, AUTONOMOUS_DOMAIN_NAMES);
  assert.deepEqual(bridge.targetedCandidateIds.sort(), candidates.map((candidate) => candidate.candidateId).sort());
  assert.equal(brain.validateClaimIntegrityAcquisitionBridge(bridge).bridgeDigest, bridge.bridgeDigest);

  const evidencePlan = await agent.evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const requirementByDomain = new Map(evidencePlan.requirements.map((requirement) => [requirement.domain, requirement]));
  const requests = bridge.acquisitionPlan.selected.map((selection, index) => {
    const requirement = requirementByDomain.get(selection.domain);
    return {
      candidate_id: selection.candidate_id,
      requirement_id: requirement.requirement_id,
      source_id: selection.source_id,
      request_id: `facade-integrity-request-${index}`,
      metadata: { purpose: "facade-integrity-acquisition" },
    };
  });
  const requestedRequirementIds = new Set(requests.map((request) => request.requirement_id));
  const availableEvidence = evidencePlan.requirements
    .filter((requirement) => !requestedRequirementIds.has(requirement.requirement_id))
    .map((requirement) => requirement.requirement_id);
  const binding = brain.bindClaimIntegrityAcquisition(bridge, requests);
  assert.equal(binding.candidateIds.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(brain.validateClaimIntegrityAcquisitionBinding(binding).bindingDigest, binding.bindingDigest);
  assert.doesNotMatch(JSON.stringify(binding.toJSON()), /facade-integrity-acquisition/);

  const calls = { count: 0 };
  const registry = new AutonomousEvidenceAdapterRegistry();
  registerEvidenceAdapter(registry, calls);
  const admission = await approvedLaunchAdmission(brain);
  const executed = await brain.executeClaimIntegrityAcquisitionWithLaunchAdmission(bridge, registry, requests, admission, {
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    availableEvidence,
    execute: { ...executionOptions(), approveSourceDispatch: true },
  });
  assert.equal(executed.status, "awaiting_evaluation");
  assert.equal(executed.runtime.json.status, "awaiting_evaluation");
  assert.equal(executed.runtime.json.pending_evaluation_requirement_ids.length, 0);
  assert.equal(executed.runtime.json.missing_requirement_ids.length, 0);
  assert.equal(calls.count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.doesNotMatch(JSON.stringify(executed.toJSON()), /transient_source_value|facade-integrity-acquisition/);

  const checkpointStore = new InMemoryAutonomousEvidenceExecutionCheckpointStore();
  const first = await brain.executeClaimIntegrityAcquisitionResumableWithLaunchAdmission(bridge, registry, requests, admission, {
    jobId: "facade-integrity-resume",
    checkpointStore,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    availableEvidence,
    execute: executionOptions(),
  });
  assert.equal(first.status, "approval_required");
  assert.equal(calls.count, AUTONOMOUS_DOMAIN_NAMES.length);
  const resumed = await brain.executeClaimIntegrityAcquisitionResumableWithLaunchAdmission(bridge, registry, requests, admission, {
    jobId: "facade-integrity-resume",
    checkpointStore,
    prepare: {
      readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }),
      allowDegradedDispatch: true,
    },
    availableEvidence,
    execute: { ...executionOptions(), approveSourceDispatch: true },
  });
  assert.equal(resumed.status, "awaiting_evaluation");
  assert.equal(resumed.checkpoint.completed_request_count, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(resumed.checkpoint.runtime_status, "awaiting_evaluation");
  assert.equal(calls.count, AUTONOMOUS_DOMAIN_NAMES.length * 2);

  const held = brain.admitLaunchPreflight(await brain.launchPreflight(), { decision: "hold", authorizationDigest: "e".repeat(64) });
  await assert.rejects(
    () => brain.executeClaimIntegrityAcquisitionWithLaunchAdmission(bridge, registry, requests, held, {
      prepare: { readinessPolicy: new AutonomousEvidenceReadinessPolicy({ requireHealth: false }), allowDegradedDispatch: true },
      availableEvidence,
      execute: { ...executionOptions(), approveSourceDispatch: true },
    }),
    /launch admission is not approved/,
  );
  assert.equal(calls.count, AUTONOMOUS_DOMAIN_NAMES.length * 2, "held admission must prevent source dispatch");
});
