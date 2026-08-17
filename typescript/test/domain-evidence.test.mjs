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
    coverage: { traceability_state: "complete", all_reports_linked: true },
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

test("domain evidence REST and tool clients preserve traceability posture", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      if (url.pathname === "/v1/domain-evidence/harmonize") return new Response(JSON.stringify(harmonization), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_harmonize") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_harmonize", mcp: { result: { structuredContent: harmonization } } }), { status: 200, headers: { "content-type": "application/json" } });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.domainEvidenceHarmonize(args)).harmonization.coverage.traceability_state, "complete");
  assert.equal((await client.domainEvidenceHarmonizeTool(args)).mcp.result.structuredContent.workflow, "domain_evidence_harmonize");
  assert.equal(seen[0].url.pathname, "/v1/domain-evidence/harmonize");
  await assert.rejects(
    client.domainEvidenceHarmonize({ ...args, links: [{ report_index: 0, role: "qualifies" }] }),
    ArgumentError,
  );
});
