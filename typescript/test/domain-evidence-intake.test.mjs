import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const intake = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-intake/0.1",
  workflow: "domain_evidence_intake",
  group_id: "biological_domains",
  domains: ["modalities"],
  subject_id: "subject-ts",
  source_tool: "modality_catalog",
  request_supplied: true,
  request_digest: "a".repeat(64),
  response_digest: "b".repeat(64),
  intake_digest: "c".repeat(64),
  outcome: "observed",
  report: { schema: "bioprism-devplat-domain-report/0.1" },
  intake: { response: { status: "bounded" } },
  artifact_registry: { indexed: true, kind: "domain_evidence_intake", content_digest: "d".repeat(64) },
  catalogue_digest: "e".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

const args = {
  group_id: "biological_domains",
  domains: ["modalities"],
  subject_id: "subject-ts",
  source_tool: "modality_catalog",
  request: { modality: "single_cell" },
  response: { status: "bounded" },
  outcome: "observed",
  claim_posture: { status: "observed", does_not_claim: ["truth"] },
};

const coverage = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-intake-coverage/0.1",
  workflow: "domain_evidence_intake_coverage",
  catalogue_digest: "e".repeat(64),
  coverage_digest: "f".repeat(64),
  filters: { max_groups: 64, include_intake_digests: true },
  group_count: 1,
  reported_group_count: 1,
  missing_group_count: 0,
  missing_group_ids: [],
  complete: true,
  tool_coverage_complete: false,
  missing_tool_group_ids: ["biological_domains"],
  domain_coverage_complete: true,
  missing_domain_group_ids: [],
  groups: [{
    id: "biological_domains",
    domains: ["modalities"],
    status: "active",
    declared_tool_count: 1,
    declared_tools: ["modality_catalog"],
    intake_count: 1,
    subject_ids: ["subject-ts"],
    source_tools: ["modality_catalog"],
    outcomes: ["observed"],
    reported_domains: ["modalities"],
    missing_source_tools: [],
    source_tool_coverage: [{ tool: "modality_catalog", intake_count: 1, outcomes: ["observed"], coverage_state: "reported" }],
    missing_domains: [],
    tool_coverage_state: "complete",
    domain_coverage_state: "complete",
    intake_digests: ["c".repeat(64)],
    coverage_state: "reported",
  }],
  domain_summary: { modalities: { group_count: 1, reported_group_count: 1, missing_group_count: 0, intake_count: 1 } },
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

const sourcePlan = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-source-plan/0.1",
  workflow: "domain_evidence_source_plan",
  plan_digest: "g".repeat(64),
  group_id: "biological_domains",
  domains: ["modalities"],
  subject_id: "subject-ts",
  source_tool: "modality_catalog",
  connector_kind: "literature",
  locator_kind: "uri",
  locator: "https://example.org/article/1",
  retrieval_mode: "metadata_only",
  expected_content_digest: "a".repeat(64),
  parent_digests: [],
  retrieval_policy: { network: "caller_managed", max_bytes: 4096, cache: "content_addressed", credentials: "caller_managed_not_supplied" },
  plan: { retrieval_status: "not_started" },
  artifact_registry: { indexed: true, kind: "domain_evidence_source_plan", content_digest: "h".repeat(64) },
  catalogue_digest: "i".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  retrieval_status: "not_started",
  guarantees: [],
  does_not_claim: ["retrieval occurred"],
};

const sourceArgs = {
  group_id: "biological_domains",
  domains: ["modalities"],
  subject_id: "subject-ts",
  source_tool: "modality_catalog",
  connector_kind: "literature",
  locator_kind: "uri",
  locator: "https://example.org/article/1",
  retrieval_mode: "metadata_only",
  expected_content_digest: "a".repeat(64),
  retrieval_policy: { network: "caller_managed", max_bytes: 4096, cache: "content_addressed" },
  does_not_claim: ["retrieval occurred"],
};

const sourceExecution = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-source-execution/0.1",
  workflow: "domain_evidence_source_execute",
  source_plan_digest: "a".repeat(64),
  group_id: "biological_domains",
  domains: ["modalities"],
  subject_id: "subject-ts",
  source_tool: "modality_catalog",
  outcome: "observed",
  retrieval_status: "observed",
  execution: "completed",
  raw_content_digest: "f".repeat(64),
  response_digest: "a".repeat(64),
  byte_length: 24,
  content_type: "application/json",
  execution_result: { response: { retrieval: { body_encoding: "json" } } },
  intake: { workflow: "domain_evidence_intake" },
  artifact_registry: { indexed: true, kind: "domain_evidence_intake", content_digest: "d".repeat(64) },
  catalogue_digest: "i".repeat(64),
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: [],
};

const sourceExecutionArgs = {
  source_plan_digest: "a".repeat(64),
  request: { method: "read" },
  parent_digests: ["e".repeat(64)],
};

const providerNormalization = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-normalization/0.1",
  workflow: "domain_evidence_provider_normalize",
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "provider-ts",
  source_tool: "literature_bind_check",
  connector_kind: "literature",
  provider: "pubmed",
  outcome: "unknown",
  payload_digest: "j".repeat(64),
  request_digest: null,
  response: { provider: "pubmed", authenticated: false, payload_digest: "j".repeat(64) },
  shape_audit: {
    schema: "bioprism-devplat-domain-evidence-provider-shape-audit/0.1",
    status: "unclassified",
    connector_kind: "literature",
    root_kind: "object",
    recognized_container: "records",
    record_count: 0,
    valid_record_count: 0,
    invalid_record_count: 0,
    identifier_coverage: { candidate_fields: ["id", "pmid", "doi", "source_id"], present_record_count: 0, missing_record_count: 0 },
    content_digest_coverage: null,
    missing_fields: [],
    warnings: [],
    limitations: ["structural only"],
    shape_digest: "m".repeat(64),
  },
  record_index: {
    schema: "bioprism-devplat-domain-evidence-provider-record-index/0.1",
    connector_kind: "literature",
    recognized_container: "records",
    record_count: 0,
    indexed_record_count: 0,
    omitted_record_count: 0,
    row_digests: [],
    index_digest: "a".repeat(64),
    limitations: ["digest-only"],
  },
  normalization: { payload_digest: "j".repeat(64) },
  intake: { workflow: "domain_evidence_intake", outcome: "unknown" },
  artifact_registry: { indexed: true, kind: "domain_evidence_intake", content_digest: "k".repeat(64) },
  catalogue_digest: "l".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: ["provider authenticity"],
};

const providerNormalizationArgs = {
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "provider-ts",
  source_tool: "literature_bind_check",
  connector_kind: "literature",
  provider: "pubmed",
  payload: { records: [{ id: "pmid:1" }] },
};

const providerReplay = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-replay/0.1",
  workflow: "domain_evidence_provider_replay_verify",
  replay: {
    schema: "bioprism-devplat-domain-evidence-provider-replay/0.1",
    workflow: "domain_evidence_provider_replay_verify",
    replay_status: "matched",
    matched: true,
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    connector_kind: "literature",
    provider: "pubmed",
    expected_payload_digest: "a".repeat(64),
    observed_payload_digest: "a".repeat(64),
    expected_request_digest: null,
    observed_request_digest: null,
    expected_shape_digest: "b".repeat(64),
    observed_shape_digest: "b".repeat(64),
    expected_normalization_digest: "c".repeat(64),
    observed_normalization_digest: "c".repeat(64),
    expected_intake_digest: "d".repeat(64),
    observed_intake_digest: "d".repeat(64),
    matches: { payload_digest: true, request_digest: true, shape_digest: true, normalization_digest: true, intake_digest: true },
    differences: [],
    shape_audit: providerNormalization.shape_audit,
    record_index: providerNormalization.record_index,
    replay_digest: "e".repeat(64),
    guarantees: [],
    limitations: [],
  },
  matched: true,
  replay_status: "matched",
  replay_digest: "e".repeat(64),
  artifact_registry: { created: true, indexed: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["provider authenticity"],
};

const providerReplayArgs = {
  ...providerNormalizationArgs,
  expected_payload_digest: "a".repeat(64),
  expected_shape_digest: "b".repeat(64),
  expected_normalization_digest: "c".repeat(64),
  expected_intake_digest: "d".repeat(64),
};

const providerHandoff = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1",
  workflow: "domain_evidence_provider_connector_handoff",
  handoff: {
    schema: "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1",
    workflow: "domain_evidence_provider_connector_handoff",
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    provider: "pubmed",
    connector_kind: "literature",
    status: "prepared",
    manifest: {
      schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
      connector_id: "caller.pubmed",
      version: "1.2.0",
      provider: "pubmed",
      connector_kind: "literature",
      domains: ["oncology"],
      capabilities: ["query", "retain"],
      transport: "caller_managed",
      auth_posture: { status: "caller_asserted", secret_refs: ["secret://caller/pubmed"], does_not_claim: ["provider authentication"] },
    },
    manifest_digest: "f".repeat(64),
    request_digest: "a".repeat(64),
    payload_digest: "b".repeat(64),
    source_plan_digest: null,
    parent_digests: [],
    attempt_id: null,
    handoff_digest: "e".repeat(64),
    execution: "not_started",
    readiness_claimed: false,
    guarantees: [],
    limitations: [],
  },
  manifest_digest: "f".repeat(64),
  handoff_digest: "e".repeat(64),
  artifact_registry: { created: true, indexed: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["provider authentication"],
};

const providerHandoffArgs = {
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "provider-ts",
  source_tool: "literature_bind_check",
  provider: "pubmed",
  connector_kind: "literature",
  manifest: providerHandoff.handoff.manifest,
  status: "prepared",
  request_digest: "a".repeat(64),
  payload_digest: "b".repeat(64),
};

const providerExternalReceipt = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1",
  workflow: "domain_evidence_provider_external_payload_receipt",
  receipt: {
    schema: "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1",
    workflow: "domain_evidence_provider_external_payload_receipt",
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    provider: "pubmed",
    connector_kind: "literature",
    handoff_digest: "a".repeat(64),
    transfer_id: "export-1",
    payload_digest: "b".repeat(64),
    byte_length: 4096,
    storage_backend: "object_store",
    locator_kind: "opaque",
    locator: "store://caller/pubmed/objects/1",
    content_type: "application/json",
    content_encoding: "gzip",
    request_digest: null,
    parent_digests: [],
    availability: "available",
    retention: "durable",
    attempt_id: null,
    receipt_digest: "e".repeat(64),
    execution: "not_started",
    readiness_claimed: false,
    guarantees: [],
    limitations: [],
  },
  handoff_digest: "a".repeat(64),
  payload_digest: "b".repeat(64),
  receipt_digest: "e".repeat(64),
  artifact_registry: { created: true, indexed: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["store accessibility"],
};

const providerExternalReceiptArgs = {
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "provider-ts",
  source_tool: "literature_bind_check",
  provider: "pubmed",
  connector_kind: "literature",
  handoff_digest: "a".repeat(64),
  transfer_id: "export-1",
  payload_digest: "b".repeat(64),
  byte_length: 4096,
  storage_backend: "object_store",
  locator_kind: "opaque",
  locator: "store://caller/pubmed/objects/1",
  availability: "available",
  retention: "durable",
};

const providerExternalReplayArgs = {
  ...providerExternalReceiptArgs,
  expected_receipt_digest: "e".repeat(64),
  expected_handoff_digest: "a".repeat(64),
  expected_payload_digest: "b".repeat(64),
  expected_byte_length: 4096,
};

const providerExternalReplay = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1",
  workflow: "domain_evidence_provider_external_payload_replay_verify",
  replay: {
    schema: "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1",
    workflow: "domain_evidence_provider_external_payload_replay_verify",
    replay_status: "matched",
    matched: true,
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    provider: "pubmed",
    connector_kind: "literature",
    expected_receipt_digest: "e".repeat(64),
    observed_receipt_digest: "e".repeat(64),
    expected_handoff_digest: "a".repeat(64),
    observed_handoff_digest: "a".repeat(64),
    expected_payload_digest: "b".repeat(64),
    observed_payload_digest: "b".repeat(64),
    expected_byte_length: 4096,
    observed_byte_length: 4096,
    matches: { byte_length: true, handoff_digest: true, payload_digest: true, receipt_digest: true },
    differences: [],
    receipt: providerExternalReceipt.receipt,
    replay_digest: "f".repeat(64),
    guarantees: [],
    limitations: [],
  },
  matched: true,
  replay_status: "matched",
  replay_digest: "f".repeat(64),
  artifact_registry: { created: true, indexed: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["store accessibility"],
};

const providerExternalNormalizationArgs = {
  ...providerExternalReceiptArgs,
  payload: { records: [{ id: "pmid:1", title: "opaque" }] },
  outcome: "observed",
};

const providerExternalNormalization = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-normalization/0.1",
  workflow: "domain_evidence_provider_external_payload_normalize",
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "provider-ts",
  source_tool: "literature_bind_check",
  connector_kind: "literature",
  provider: "pubmed",
  outcome: "observed",
  payload_digest: "b".repeat(64),
  request_digest: null,
  response: { provider: "pubmed", payload_digest: "b".repeat(64) },
  shape_audit: {},
  record_index: {},
  normalization: {},
  receipt: providerExternalReceipt.receipt,
  receipt_digest: "e".repeat(64),
  materialization: { mode: "canonical_json", matched: true, materialized_payload_digest: "b".repeat(64), locator_opened: false },
  intake: {},
  artifact_registry: { created: true, indexed: true },
  receipt_artifact_registry: { created: true, indexed: true },
  catalogue_digest: "f".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: ["provider authenticity"],
};

const providerExternalLineage = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1",
  workflow: "domain_evidence_provider_external_payload_lineage_audit",
  audit: {
    schema: "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1",
    workflow: "domain_evidence_provider_external_payload_lineage_audit",
    lineage_status: "matched",
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    provider: "pubmed",
    connector_kind: "literature",
    receipt: providerExternalReceipt.receipt,
    handoff: { handoff_digest: "a".repeat(64), status: "prepared" },
    matches: { handoff_present: true, handoff_digest: true, payload_digest: true },
    differences: [],
    payload_binding_status: "matched",
    lineage_digest: "l".repeat(64),
    guarantees: [],
    limitations: [],
  },
  lineage_status: "matched",
  payload_binding_status: "matched",
  lineage_digest: "l".repeat(64),
  receipt_registry: { ok: true, created: true },
  artifact_registry: { ok: true, created: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["provider authenticity"],
};

const providerExternalExecutionArgs = {
  ...providerExternalReceiptArgs,
  expected_receipt_digest: "e".repeat(64),
  execution_status: "transferred",
  executor_id: "caller-transfer-worker",
  observed_payload_digest: "b".repeat(64),
  observed_byte_length: 4096,
  locator_opened: true,
  observation_digest: "c".repeat(64),
};

const providerExternalExecution = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-execution-evidence/0.1",
  workflow: "domain_evidence_provider_external_payload_execution_evidence",
  evidence: {
    schema: "bioprism-devplat-domain-evidence-provider-external-payload-execution-evidence/0.1",
    workflow: "domain_evidence_provider_external_payload_execution_evidence",
    evidence_status: "matched",
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    provider: "pubmed",
    connector_kind: "literature",
    expected_receipt_digest: "e".repeat(64),
    retained_receipt_digest: "e".repeat(64),
    observed_receipt_digest: "e".repeat(64),
    execution_status: "transferred",
    executor_id: "caller-transfer-worker",
    observed_payload_digest: "b".repeat(64),
    observed_byte_length: 4096,
    locator_opened: true,
    observation_digest: "c".repeat(64),
    receipt: providerExternalReceipt.receipt,
    matches: { receipt_present: true, observed_payload_digest: true, observed_byte_length: true },
    differences: [],
    evidence_digest: "1".repeat(64),
    guarantees: [],
    limitations: [],
  },
  evidence_status: "matched",
  evidence_digest: "1".repeat(64),
  receipt_registry: { ok: true, already_present: true },
  artifact_registry: { ok: true, created: true },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["provider authenticity"],
};

const providerExternalQueryArgs = {
  subject_id: "provider-ts",
  max_items: 1,
  include_artifacts: true,
};

const providerExternalQuery = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-provider-external-payload-query/0.1",
  workflow: "domain_evidence_provider_external_payload_evidence_query",
  filters: { subject_id: "provider-ts", max_items: 1, include_artifacts: true },
  registry_generation: 4,
  registry_size: 3,
  rows: [{
    row_digest: "2".repeat(64),
    receipt_digest: "e".repeat(64),
    subject_id: "provider-ts",
    group_id: "biological_domains",
    domains: ["oncology"],
    receipt_present: true,
    lineage_status: "matched",
    lineage_digest: "l".repeat(64),
    execution_evidence_status: "matched",
    execution_status: "transferred",
    evidence_digest: "1".repeat(64),
    join_status: "complete",
    parent_digests: [],
    receipt_artifact: providerExternalReceipt.receipt,
    lineage_artifact: providerExternalLineage.audit,
    execution_artifact: providerExternalExecution.evidence,
  }],
  next_after: null,
  has_more: false,
  query_digest: "3".repeat(64),
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  limitations: ["registry snapshot only"],
};

test("domain evidence intake REST and tool clients preserve exact envelope metadata", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      if (url.pathname === "/v1/domain-evidence/intake") return new Response(JSON.stringify(intake), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_intake") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_intake", mcp: { result: { structuredContent: intake } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/domain-evidence/coverage") return new Response(JSON.stringify(coverage), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_coverage") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_coverage", mcp: { result: { structuredContent: coverage } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/domain-evidence/sources") return new Response(JSON.stringify(sourcePlan), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_source_plan") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_source_plan", mcp: { result: { structuredContent: sourcePlan } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/domain-evidence/sources/execute") return new Response(JSON.stringify(sourceExecution), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_source_execute") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_source_execute", mcp: { result: { structuredContent: sourceExecution } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_normalize") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_normalize", mcp: { result: { structuredContent: providerNormalization } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_replay_verify") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_replay_verify", mcp: { result: { structuredContent: providerReplay } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_connector_handoff") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_connector_handoff", mcp: { result: { structuredContent: providerHandoff } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_receipt") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_receipt", mcp: { result: { structuredContent: providerExternalReceipt } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_replay_verify") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_replay_verify", mcp: { result: { structuredContent: providerExternalReplay } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_normalize") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_normalize", mcp: { result: { structuredContent: providerExternalNormalization } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_lineage_audit") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_lineage_audit", mcp: { result: { structuredContent: providerExternalLineage } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_execution_evidence") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_execution_evidence", mcp: { result: { structuredContent: providerExternalExecution } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_provider_external_payload_evidence_query") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_provider_external_payload_evidence_query", mcp: { result: { structuredContent: providerExternalQuery } } }), { status: 200, headers: { "content-type": "application/json" } });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.domainEvidenceIntake(args)).outcome, "observed");
  assert.equal((await client.domainEvidenceIntakeTool(args)).mcp.result.structuredContent.intake_digest, "c".repeat(64));
  assert.equal((await client.domainEvidenceCoverage({ include_intake_digests: true })).coverage_digest, "f".repeat(64));
  assert.equal((await client.domainEvidenceCoverageTool({ group_id: "biological_domains" })).mcp.result.structuredContent.complete, true);
  assert.equal((await client.domainEvidenceSourcePlan(sourceArgs)).retrieval_status, "not_started");
  assert.equal((await client.domainEvidenceSourcePlanTool(sourceArgs)).mcp.result.structuredContent.plan_digest, "g".repeat(64));
  assert.equal((await client.domainEvidenceSourceExecute(sourceExecutionArgs)).outcome, "observed");
  assert.equal((await client.domainEvidenceSourceExecuteTool(sourceExecutionArgs)).mcp.result.structuredContent.raw_content_digest, "f".repeat(64));
  assert.equal((await client.domainEvidenceProviderNormalize(providerNormalizationArgs)).mcp.result.structuredContent.provider, "pubmed");
  assert.equal((await client.domainEvidenceProviderNormalizeTool(providerNormalizationArgs)).mcp.result.structuredContent.outcome, "unknown");
  assert.equal((await client.domainEvidenceProviderNormalize(providerNormalizationArgs)).mcp.result.structuredContent.shape_audit.status, "unclassified");
  assert.equal((await client.domainEvidenceProviderNormalize(providerNormalizationArgs)).mcp.result.structuredContent.record_index.omitted_record_count, 0);
  assert.equal((await client.domainEvidenceProviderReplayVerify(providerReplayArgs)).mcp.result.structuredContent.replay_status, "matched");
  assert.equal((await client.domainEvidenceProviderReplayVerifyTool(providerReplayArgs)).mcp.result.structuredContent.replay.matched, true);
  assert.equal((await client.domainEvidenceProviderConnectorHandoff(providerHandoffArgs)).mcp.result.structuredContent.handoff.status, "prepared");
  assert.equal((await client.domainEvidenceProviderConnectorHandoffTool(providerHandoffArgs)).mcp.result.structuredContent.handoff.manifest.auth_posture.secret_refs[0], "secret://caller/pubmed");
  assert.equal((await client.domainEvidenceProviderExternalPayloadReceipt(providerExternalReceiptArgs)).mcp.result.structuredContent.receipt.retention, "durable");
  assert.equal((await client.domainEvidenceProviderExternalPayloadReceiptTool(providerExternalReceiptArgs)).mcp.result.structuredContent.receipt.byte_length, 4096);
  assert.equal((await client.domainEvidenceProviderExternalPayloadReplayVerify(providerExternalReplayArgs)).mcp.result.structuredContent.replay_status, "matched");
  assert.equal((await client.domainEvidenceProviderExternalPayloadReplayVerifyTool(providerExternalReplayArgs)).mcp.result.structuredContent.replay.matches.receipt_digest, true);
  assert.equal((await client.domainEvidenceProviderExternalPayloadNormalize(providerExternalNormalizationArgs)).mcp.result.structuredContent.materialization.locator_opened, false);
  assert.equal((await client.domainEvidenceProviderExternalPayloadNormalizeTool(providerExternalNormalizationArgs)).mcp.result.structuredContent.outcome, "observed");
  assert.equal((await client.domainEvidenceProviderExternalPayloadLineageAudit(providerExternalReceiptArgs)).mcp.result.structuredContent.lineage_status, "matched");
  assert.equal((await client.domainEvidenceProviderExternalPayloadLineageAuditTool(providerExternalReceiptArgs)).mcp.result.structuredContent.audit.matches.payload_digest, true);
  assert.equal((await client.domainEvidenceProviderExternalPayloadExecutionEvidence(providerExternalExecutionArgs)).mcp.result.structuredContent.evidence_status, "matched");
  assert.equal((await client.domainEvidenceProviderExternalPayloadExecutionEvidenceTool(providerExternalExecutionArgs)).mcp.result.structuredContent.evidence.matches.observed_payload_digest, true);
  assert.equal((await client.domainEvidenceProviderExternalPayloadEvidenceQuery(providerExternalQueryArgs)).mcp.result.structuredContent.rows[0].join_status, "complete");
  assert.equal((await client.domainEvidenceProviderExternalPayloadEvidenceQueryTool(providerExternalQueryArgs)).mcp.result.structuredContent.rows[0].execution_status, "transferred");
  assert.equal(seen[0].url.pathname, "/v1/domain-evidence/intake");
  assert.equal(seen[2].url.searchParams.get("include_intake_digests"), "true");
  assert.equal(seen[4].url.pathname, "/v1/domain-evidence/sources");
  assert.equal(seen[6].url.pathname, "/v1/domain-evidence/sources/execute");
  await assert.rejects(
    client.domainEvidenceIntake({ ...args, outcome: "success" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceSourceExecute({ source_plan_digest: "not-a-digest" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderNormalize({ ...providerNormalizationArgs, connector_kind: "file" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderReplayVerify({ ...providerReplayArgs, expected_shape_digest: "not-a-digest" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderConnectorHandoff({ ...providerHandoffArgs, credential_material: "never" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderConnectorHandoff({ ...providerHandoffArgs, manifest: { ...providerHandoffArgs.manifest, transport: "http" } }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadReceipt({ ...providerExternalReceiptArgs, payload: { records: [] } }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadReceipt({ ...providerExternalReceiptArgs, locator: "https://user:pass@example.org/object" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadReplayVerify({ ...providerExternalReplayArgs, expected_byte_length: 0 }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadReplayVerify({ ...providerExternalReplayArgs, expected_payload_digest: "not-a-digest" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadReplayVerify({ ...providerExternalReplayArgs, payload: { records: [] } }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadNormalize({ ...providerExternalNormalizationArgs, credential_material: "never" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadLineageAudit({ ...providerExternalReceiptArgs, credential_material: "never" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadExecutionEvidence({ ...providerExternalExecutionArgs, execution_status: "bad" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadExecutionEvidence({ ...providerExternalExecutionArgs, credential_material: "never" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadEvidenceQuery({ ...providerExternalQueryArgs, max_items: 0 }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceProviderExternalPayloadEvidenceQuery({ ...providerExternalQueryArgs, credential_material: "never" }),
    ArgumentError,
  );
});
