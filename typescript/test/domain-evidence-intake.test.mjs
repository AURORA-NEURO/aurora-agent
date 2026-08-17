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
});
