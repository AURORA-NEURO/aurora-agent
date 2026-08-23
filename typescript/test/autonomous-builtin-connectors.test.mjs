import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousConnectorOperationRegistry,
  AutonomousConnectorRuntime,
  InMemoryAutonomousConnectorReceiptJournal,
  InMemoryAutonomousMissionCheckpointStore,
  ToolCatalogue,
  autonomousConnectorMissionExecutor,
  builtinAutonomousConnectorRegistration,
  createBuiltinAutonomousConnectorRuntime,
} from "../dist/index.js";

const RECOMMENDED_FIELDS = {
  coding: { repository_digest: "b".repeat(64), changed_files: ["file-digest"], test_results: { passed: 4 } },
  browser: { source_digests: ["b".repeat(64)], retrieved_at: "2026-08-21T00:00:00Z", citation_metadata: { source: "fixture" } },
  data: { schema: { columns: ["id"] }, row_count: 4, column_count: 1, lineage: { source: "fixture" } },
  science: { hypothesis: "bounded hypothesis", evidence_digests: ["b".repeat(64)], analysis_digest: "b".repeat(64) },
  biomedical: { provenance: { source: "fixture" }, cohort_digest: "b".repeat(64), review_questions: ["scope"] },
  neuroscience: { signal_digest: "b".repeat(64), sampling_rate: 1000, study_design: { modality: "fixture" } },
  operations: { incident_digest: "b".repeat(64), telemetry_digest: "b".repeat(64), runbook_digest: "b".repeat(64) },
  enterprise: { workflow_digest: "b".repeat(64), record_type: "fixture", policy_digest: "b".repeat(64) },
  multi_agent: { delegation_digest: "b".repeat(64), agent_digests: ["b".repeat(64)], conflicts: [] },
  multimodal: { modalities: ["document"], asset_digests: ["b".repeat(64)], alignment_digest: "b".repeat(64) },
  cross_domain: { domain_digests: { science: "b".repeat(64) }, evidence_digests: ["b".repeat(64)], route_digest: "b".repeat(64) },
  evaluation: { benchmark_digest: "b".repeat(64), case_count: 4, replay_digest: "b".repeat(64) },
};

test("credentialless built-in connector projects bounded metadata for every operation domain", () => {
  const operationRegistry = new AutonomousConnectorOperationRegistry();
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const registration = builtinAutonomousConnectorRegistration({ operationRegistry, domain, approvalRequired: false });
    const operation = operationRegistry.forDomain(domain)[0];
    const request = {
      operation_id: operation.operation_id,
      subject_digest: "a".repeat(64),
      ...RECOMMENDED_FIELDS[domain],
    };
    const observation = registration.executor(registration.manifest, request);
    assert.equal(observation.status, "observed", domain);
    assert.equal(observation.value.domain, domain);
    assert.equal(observation.value.operation_id, operation.operation_id);
    assert.equal(observation.value.evidence_posture, "caller_supplied_metadata;offline_fixture;not_external_validation");
    assert.equal(JSON.stringify(observation.value).includes("bounded hypothesis"), false, domain);
    assert.equal(JSON.stringify(observation.value).includes("a".repeat(64)), true, "subject identity is intentionally digest-bound");
  }
});

test("built-in connector reports partial fixtures and rejects credential-shaped fields", () => {
  const registration = builtinAutonomousConnectorRegistration({ domain: "science", approvalRequired: false });
  const partial = registration.executor(registration.manifest, {
    operation_id: "science.reproducible_evidence_acquisition",
    subject_digest: "a".repeat(64),
    hypothesis: "only one recommended field",
  });
  assert.equal(partial.status, "partial");
  assert.deepEqual(partial.value.missing_fields, ["evidence_digests", "analysis_digest"]);
  assert.throws(() => registration.executor(registration.manifest, {
    operation_id: "science.reproducible_evidence_acquisition",
    subject_digest: "a".repeat(64),
    api_key: "never-accepted",
  }), /credential-shaped fields/);
});

test("domain-scoped built-ins execute through the durable mission executor without retaining payloads", async () => {
  const operationRegistry = new AutonomousConnectorOperationRegistry();
  const fixture = createBuiltinAutonomousConnectorRuntime({ operationRegistry, domainScoped: true, approvalRequired: false });
  const registry = fixture.registry;
  const registrations = fixture.registrations;
  assert.equal(registrations.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.deepEqual(registry.registrations().map((row) => row.manifest.domains[0]), [...AUTONOMOUS_DOMAIN_NAMES].sort());
  const journal = new InMemoryAutonomousConnectorReceiptJournal();
  const runtime = new AutonomousConnectorRuntime(registry, { receiptStore: journal });
  const catalogue = await ToolCatalogue.fromDefinitions([{
    name: "offline_connector_probe",
    description: "offline connector probe",
    inputSchema: { type: "object", additionalProperties: true },
  }]);
  const executor = autonomousConnectorMissionExecutor({
    catalogue,
    checkpointStore: new InMemoryAutonomousMissionCheckpointStore(),
    connector: { runtime, operationRegistry, approved: true },
  });
  const result = await executor.start({
    mission_id: "builtin-all-domain-mission",
    goal: "exercise every offline autonomous domain",
    steps: AUTONOMOUS_DOMAIN_NAMES.map((domain) => ({
      id: `step-${domain}`,
      domain,
      capability: operationRegistry.forDomain(domain)[0].capabilities[0],
      objective: `observe ${domain}`,
      tool: "offline_connector_probe",
      arguments: { subject_digest: "a".repeat(64), ...RECOMMENDED_FIELDS[domain] },
    })),
    policy: {
      execute: true,
      stop_on_error: true,
      allow_side_effects: false,
      max_steps: 32,
      max_step_output_bytes: 200_000,
      max_total_output_bytes: 3_000_000,
      execution_mode: "parallel_waves",
      max_parallelism: 4,
      allowed_tools: ["offline_connector_probe"],
    },
  }, { approveProviderCall: true });
  assert.equal(result.status, "succeeded", JSON.stringify(result.preflight));
  assert.equal(result.succeeded_steps, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal((await journal.verifyIntegrity()).entries, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(JSON.stringify(result.checkpoint).includes("bounded hypothesis"), false);
});
