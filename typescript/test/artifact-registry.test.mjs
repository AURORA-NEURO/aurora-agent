import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient } from "../dist/index.js";

function jsonResponse(value, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json" } });
}

const digest = "a".repeat(64);

test("artifact registry REST and MCP clients preserve lineage uncertainty", async () => {
  const registration = {
    ok: true,
    schema: "bioprism-devplat-artifact-register/0.1",
    workflow: "artifact_registry_register",
    content_digest: digest,
    kind: "domain_report",
    subject_id: "subject-1",
    declared_digest: null,
    verification: { content_digest_verified: true, semantic_verification: "not_run" },
    created: true,
    already_present: false,
    registry_generation: 1,
    registry_size: 1,
    execution: "not_started",
    guarantees: [],
    does_not_claim: ["scientific validity"],
  };
  const query = {
    ok: true,
    schema: "bioprism-devplat-artifact-query/0.1",
    workflow: "artifact_registry_query",
    filters: {},
    registry_generation: 1,
    registry_size: 1,
    rows: [{ content_digest: digest, kind: "domain_report", subject_id: "subject-1" }],
    next_after: null,
    has_more: false,
    execution: "not_started",
    guarantees: [],
    does_not_claim: [],
  };
  const fetched = {
    ok: true,
    schema: "bioprism-devplat-artifact-get/0.1",
    workflow: "artifact_registry_get",
    record: { content_digest: digest, kind: "domain_report", subject_id: "subject-1", artifact: {} },
    execution: "not_started",
    guarantees: [],
    does_not_claim: [],
  };
  const lineage = {
    ok: true,
    schema: "bioprism-devplat-artifact-lineage/0.1",
    workflow: "artifact_registry_lineage",
    root: digest,
    nodes: [],
    missing_parent_digests: ["b".repeat(64)],
    cycles: [],
    bounded: true,
    execution: "not_started",
    guarantees: [],
    does_not_claim: ["parent presence proves causal provenance or scientific validity"],
  };
  const crossStore = {
    ok: true,
    schema: "bioprism-devplat-cross-domain-artifact-audit/0.1",
    workflow: "artifact_registry_cross_store_audit",
    consistent: true,
    bounded: true,
    truncated: false,
    stores: {},
    coverage: {},
    artifact_kind_counts: { domain_report: 1 },
    findings: [],
    execution: "not_started",
    guarantees: [],
    does_not_claim: ["the three stores were read in one atomic transaction"],
  };
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init = {}) => {
      const url = new URL(String(input));
      if (url.pathname === "/v1/artifacts" && init.method === "POST") {
        assert.deepEqual(JSON.parse(init.body), { kind: "domain_report", subject_id: "subject-1", artifact: { status: "review_required" } });
        return jsonResponse(registration, 201);
      }
      if (url.pathname === "/v1/artifacts") return jsonResponse(query);
      if (url.pathname === `/v1/artifacts/${digest}/lineage`) return jsonResponse(lineage);
      if (url.pathname === `/v1/artifacts/${digest}`) return jsonResponse(fetched);
      if (url.pathname === "/v1/artifacts/cross-store") return jsonResponse(crossStore);
      if (url.pathname === "/v1/tools/artifact_registry_audit") return jsonResponse({ ok: true, tool: "artifact_registry_audit", mcp: { result: { structuredContent: lineage } } });
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  assert.equal((await client.artifactRegister({ kind: "domain_report", subject_id: "subject-1", artifact: { status: "review_required" } })).created, true);
  assert.equal((await client.artifactQuery({ domain: "oncology" })).rows.length, 1);
  assert.equal((await client.artifactGet(digest)).record.content_digest, digest);
  assert.deepEqual((await client.artifactLineage(digest)).missing_parent_digests, ["b".repeat(64)]);
  assert.equal((await client.artifactCrossStoreAudit()).consistent, true);
  assert.equal((await client.artifactRegistryAudit({ operation: "lineage", content_digest: digest })).mcp.result.structuredContent.workflow, "artifact_registry_lineage");
});
