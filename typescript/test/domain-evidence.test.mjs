import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const harmonization = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-harmonization/0.1",
  workflow: "domain_evidence_harmonize",
  catalogue_digest: "a".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  harmonization: {
    coverage: {
      traceability_state: "complete",
      all_reports_linked: true,
      bridge_summary: {
        report_classes: { provider_normalization_external_payload: 1 },
        modes: { external_payload: 1 },
        lineage: { parent_digest_count: 2, reports_with_lineage_parents: 1, reports_without_lineage_parents: 0 },
      },
    },
    posture: { explicit_contradiction_declared: false },
    harmonization_digest: "b".repeat(64),
  },
  artifact_registry: { indexed: true, kind: "domain_evidence_harmonization", content_digest: "c".repeat(64) },
  guarantees: [],
  does_not_claim: [],
};

const args = {
  subject_id: "subject-ts",
  claim: { id: "claim-1", statement: "opaque" },
  reports: [{ schema: "canonical-report" }],
  links: [{ report_index: 0, role: "supports" }],
  required_group_ids: ["biological_domains"],
};

const harmonizationCoverage = {
  ok: true,
  schema: "bioprism-devplat-domain-evidence-harmonization-coverage/0.1",
  workflow: "domain_evidence_harmonization_coverage",
  filters: { subject_id: "subject-ts", domain: "modalities", max_items: 7, include_report_digests: true },
  registry_size: 3,
  matching_count: 1,
  returned_count: 1,
  has_more: false,
  next_after: null,
  rows: [{
    content_digest: "d".repeat(64), subject_id: "subject-ts", domains: ["modalities"], claim_id: "claim-1",
    report_count: 1, link_count: 1, traceability_state: "complete", requirements_complete: true,
    all_reports_linked: true, contradiction_declared: false, qualification_declared: false,
    report_classes: { ordinary: 1 }, bridge_modes: { inline: 1 }, lineage: { harmonization_parent_digest_count: 1 },
    missing_group_ids: [], missing_domains: [], report_digests: ["e".repeat(64)],
  }],
  summary: { subject_count: 1, domain_summary: { modalities: { report_count: 1 } } },
  coverage_digest: "f".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

test("domain evidence REST and tool clients preserve traceability posture", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      if (url.pathname === "/v1/domain-evidence/harmonize") return new Response(JSON.stringify(harmonization), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_harmonize") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_harmonize", mcp: { result: { structuredContent: harmonization } } }), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/domain-evidence/harmonization/coverage") return new Response(JSON.stringify(harmonizationCoverage), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_harmonization_coverage") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_harmonization_coverage", mcp: { result: { structuredContent: harmonizationCoverage } } }), { status: 200, headers: { "content-type": "application/json" } });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.domainEvidenceHarmonize(args)).harmonization.coverage.traceability_state, "complete");
  assert.equal((await client.domainEvidenceHarmonize(args)).harmonization.coverage.bridge_summary.modes.external_payload, 1);
  assert.equal((await client.domainEvidenceHarmonizeTool(args)).mcp.result.structuredContent.workflow, "domain_evidence_harmonize");
  const coverage = await client.domainEvidenceHarmonizationCoverage({ subject_id: "subject-ts", domain: "modalities", traceability_state: "complete", max_items: 7, include_report_digests: true });
  assert.equal(coverage.rows[0].report_count, 1);
  assert.equal(coverage.filters.max_items, 7);
  const coverageTool = await client.domainEvidenceHarmonizationCoverageTool({ subject_id: "subject-ts" });
  assert.equal(coverageTool.mcp.result.structuredContent.workflow, "domain_evidence_harmonization_coverage");
  assert.equal(seen[0].url.pathname, "/v1/domain-evidence/harmonize");
  assert.equal(seen[3].url.pathname, "/v1/domain-evidence/harmonization/coverage");
  assert.equal(seen[3].url.searchParams.get("traceability_state"), "complete");
  await assert.rejects(
    client.domainEvidenceHarmonizationCoverage({ after: "bad" }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainEvidenceHarmonize({ ...args, links: [{ report_index: 0, role: "qualifies" }] }),
    ArgumentError,
  );
});
