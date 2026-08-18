import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

function jsonResponse(value, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json" } });
}

const project = {
  ok: true,
  schema: "bioprism-devplat-domain-report-project/0.1",
  workflow: "domain_report_project",
  report: { schema: "bioprism-devplat-domain-report/0.1", group_id: "biological_domains" },
  artifact_registry: {
    indexed: true,
    kind: "domain_report",
    subject_id: "subject-ts",
    content_digest: "a".repeat(64),
  },
  coverage: { group_id: "biological_domains", declared_tool_count: 20 },
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

const coverage = {
  ok: true,
  schema: "bioprism-devplat-domain-report-coverage/0.1",
  workflow: "domain_report_coverage",
  catalogue_digest: "b".repeat(64),
  coverage_digest: "c".repeat(64),
  filters: {},
  group_count: 29,
  reported_group_count: 1,
  missing_group_count: 28,
  missing_group_ids: ["documentation_and_knowledge"],
  complete: false,
  groups: [],
  domain_summary: {},
  bridge_summary: {
    report_classes: { ordinary: 1 },
    lineage: { parent_digest_count: 0, reports_with_lineage_parents: 0, reports_without_lineage_parents: 1 },
  },
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

test("domain report REST and tool clients preserve bounded projection semantics", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      if (url.pathname === "/v1/domain-reports" && init.method === "POST") return jsonResponse(project);
      if (url.pathname === "/v1/domain-reports/coverage") return jsonResponse(coverage);
      if (url.pathname === "/v1/tools/domain_report_project") return jsonResponse({ ok: true, tool: "domain_report_project", request_id: "r1", mcp: { result: { structuredContent: project } }, guarantee: "shared" });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  const args = {
    group_id: "biological_domains",
    domains: ["modalities"],
    subject_id: "subject-ts",
    source_tool: "modality_catalog",
    report: { observations: [] },
    claim_posture: { status: "review_required", does_not_claim: ["truth"] },
  };
  assert.equal((await client.domainReportProject(args)).artifact_registry.content_digest, "a".repeat(64));
  const coverageResult = await client.domainReportCoverage({ include_report_digests: true, report_class: "ordinary", bridge_mode: "inline" });
  assert.equal(coverageResult.missing_group_count, 28);
  assert.equal(coverageResult.bridge_summary.report_classes.ordinary, 1);
  assert.equal((await client.domainReportProjectTool(args)).mcp.result.structuredContent.workflow, "domain_report_project");
  assert.equal(seen[0].url.pathname, "/v1/domain-reports");
  assert.equal(seen[1].url.searchParams.get("include_report_digests"), "true");
  assert.equal(seen[1].url.searchParams.get("report_class"), "ordinary");
  assert.equal(seen[1].url.searchParams.get("bridge_mode"), "inline");
  await assert.rejects(
    client.domainReportProject({ ...args, claim_posture: { status: "derived", does_not_claim: [] } }),
    ArgumentError,
  );
});

test("domain report client composes inline and external provider normalization", async () => {
  const seen = [];
  const providerResult = {
    ok: true,
    tool: "domain_report_project",
    request_id: "provider-report-1",
    mcp: {
      result: {
        structuredContent: {
          ok: true,
          schema: "bioprism-devplat-provider-domain-report/0.1",
          workflow: "provider_domain_report",
          mode: "inline",
          readiness_claimed: false,
          execution: "not_started",
        },
      },
    },
  };
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      return jsonResponse(providerResult);
    },
  });
  const inline = {
    group_id: "biological_domains",
    domains: ["oncology"],
    subject_id: "provider-ts",
    source_tool: "literature_bind_check",
    connector_kind: "literature",
    provider: "pubmed",
    payload: { records: [{ id: "pmid:1" }] },
    outcome: "observed",
  };
  await client.domainReportFromProviderNormalization(inline);
  const external = {
    ...inline,
    handoff_digest: "a".repeat(64),
    transfer_id: "provider-ts-transfer",
    payload_digest: "b".repeat(64),
    byte_length: 4096,
    storage_backend: "object_store",
    locator_kind: "opaque",
    locator: "store://caller/pubmed/provider-ts",
    availability: "available",
    retention: "durable",
  };
  await client.domainReportFromExternalProviderNormalization(external);
  assert.equal(seen.length, 2);
  const inlineBody = JSON.parse(seen[0].init.body);
  assert.equal(seen[0].url.pathname, "/v1/tools/domain_report_project");
  assert.equal(inlineBody.operation, "from_provider_normalization");
  assert.equal(inlineBody.normalization.outcome, "observed");
  const externalBody = JSON.parse(seen[1].init.body);
  assert.equal(externalBody.operation, "from_external_provider_normalization");
  assert.equal(externalBody.normalization.locator_opened, undefined);
  assert.equal(externalBody.normalization.availability, "available");
  await assert.rejects(
    client.domainReportFromExternalProviderNormalization({ ...external, credentials: "nope" }),
    ArgumentError,
  );
});
