import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const queryResult = {
  ok: true,
  schema: "bioprism-devplat-adapter-execution-evidence-query/0.1",
  workflow: "adapter_execution_evidence_query",
  filters: { adapter_id: "bioprism.python.vcf_text", max_items: 1 },
  registry_generation: 4,
  registry_size: 3,
  page_summary: {
    page_row_count: 1,
    execution_status_counts: { succeeded: 1 },
    conformance_status_counts: { verified: 1 },
    semantic_loss_status_counts: { lossless: 1 },
    join_status_counts: { source_bound: 1 },
    source_bound_rows: 1,
    workflow_bound_rows: 0,
    rows_with_missing_parents: 0,
    output_digest_present_rows: 1,
    total_loss_entries: 0,
  },
  rows: [{
    row_digest: "a".repeat(64),
    content_digest: "b".repeat(64),
    evidence_digest: "c".repeat(64),
    subject_id: "subject-1",
    group_id: "biological_domains",
    domains: ["genomics"],
    adapter_id: "bioprism.python.vcf_text",
    adapter_version: "0.1.0",
    source_id: "vcf-1",
    input_digest: "d".repeat(64),
    output_digest: "e".repeat(64),
    execution_status: "succeeded",
    conformance_status: "verified",
    semantic_loss_status: "lossless",
    loss_count: 0,
    parent_digests: ["f".repeat(64)],
    attestation_posture: "caller_asserted",
    join_status: "source_bound",
    joins: {
      source_plan_digests: ["f".repeat(64)],
      intake_digests: [],
      external_payload_digests: [],
      workflow_reconciliation_digests: [],
      missing_parent_digests: [],
      unclassified_parent_digests: [],
      source_bound: true,
      workflow_bound: false,
    },
  }],
  next_after: null,
  has_more: false,
  query_digest: "1".repeat(64),
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  limitations: [],
};

test("client exposes bounded adapter evidence queries and explicit joins", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      const path = new URL(String(input)).pathname;
      assert.equal(path, "/v1/tools/adapter_execution_evidence_query");
      return new Response(JSON.stringify({ ok: true, tool: "adapter_execution_evidence_query", mcp: { result: { structuredContent: queryResult } } }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  const report = (await client.adapterExecutionEvidenceQuery({ adapter_id: "bioprism.python.vcf_text", max_items: 1 })).mcp.result.structuredContent;
  assert.equal(report.rows[0].join_status, "source_bound");
  assert.equal(report.rows[0].joins.source_bound, true);
  assert.equal(report.page_summary.output_digest_present_rows, 1);
  const alias = (await client.adapterExecutionEvidenceQueryTool({})).mcp.result.structuredContent;
  assert.equal(alias.has_more, false);
  await assert.rejects(client.adapterExecutionEvidenceQuery({ after: "not-a-digest" }), ArgumentError);
  await assert.rejects(client.adapterExecutionEvidenceQuery({ max_items: 129 }), ArgumentError);
});
