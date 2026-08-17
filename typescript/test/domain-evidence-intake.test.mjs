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

test("domain evidence intake REST and tool clients preserve exact envelope metadata", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      seen.push({ url, init });
      if (url.pathname === "/v1/domain-evidence/intake") return new Response(JSON.stringify(intake), { status: 200, headers: { "content-type": "application/json" } });
      if (url.pathname === "/v1/tools/domain_evidence_intake") return new Response(JSON.stringify({ ok: true, tool: "domain_evidence_intake", mcp: { result: { structuredContent: intake } } }), { status: 200, headers: { "content-type": "application/json" } });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.domainEvidenceIntake(args)).outcome, "observed");
  assert.equal((await client.domainEvidenceIntakeTool(args)).mcp.result.structuredContent.intake_digest, "c".repeat(64));
  assert.equal(seen[0].url.pathname, "/v1/domain-evidence/intake");
  await assert.rejects(
    client.domainEvidenceIntake({ ...args, outcome: "success" }),
    ArgumentError,
  );
});
