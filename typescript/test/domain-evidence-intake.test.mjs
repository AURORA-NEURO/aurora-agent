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
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.domainEvidenceIntake(args)).outcome, "observed");
  assert.equal((await client.domainEvidenceIntakeTool(args)).mcp.result.structuredContent.intake_digest, "c".repeat(64));
  assert.equal((await client.domainEvidenceCoverage({ include_intake_digests: true })).coverage_digest, "f".repeat(64));
  assert.equal((await client.domainEvidenceCoverageTool({ group_id: "biological_domains" })).mcp.result.structuredContent.complete, true);
  assert.equal(seen[0].url.pathname, "/v1/domain-evidence/intake");
  assert.equal(seen[2].url.searchParams.get("include_intake_digests"), "true");
  await assert.rejects(
    client.domainEvidenceIntake({ ...args, outcome: "success" }),
    ArgumentError,
  );
});
