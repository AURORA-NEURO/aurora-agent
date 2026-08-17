import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const args = {
  group_id: "biological_domains",
  domains: ["oncology"],
  subject_id: "adapter-subject-1",
  adapter_id: "bioprism.python.vcf_text",
  adapter_version: "0.1.0",
  source_id: "vcf-source-1",
  input_digest: "a".repeat(64),
  output_digest: "b".repeat(64),
  execution_status: "succeeded",
  conformance_status: "verified",
  semantic_loss_status: "lossless",
  item_count: 4,
  byte_length: 128,
  parent_digests: ["c".repeat(64)],
};

const evidence = {
  ...args,
  schema: "bioprism-devplat-adapter-execution-evidence/0.1",
  workflow: "adapter_execution_evidence",
  losses: [],
  error_code: null,
  attempt_id: null,
  attestation_posture: "caller_asserted",
  evidence_digest: "d".repeat(64),
};

const result = {
  ok: true,
  schema: "bioprism-devplat-adapter-execution-evidence/0.1",
  workflow: "adapter_execution_evidence",
  evidence,
  adapter: {
    id: "bioprism.python.vcf_text",
    version: "0.1.0",
    execution: "python_delegated",
    conformance_level: "normalize",
    optional_dependency: null,
    declared_loss_kinds: [],
    scope_dimensions: ["subject", "sample", "variant", "genome"],
  },
  evidence_digest: "d".repeat(64),
  attestation_posture: "caller_asserted",
  artifact_registry: { indexed: true, created: true, kind: "adapter_execution_evidence" },
  execution: "not_started",
  readiness_claimed: false,
  guarantees: [],
  does_not_claim: ["adapter execution by the MCP core"],
};

test("client preserves adapter execution, conformance, semantic-loss, and artifact posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      const path = new URL(String(input)).pathname;
      assert.equal(path, "/v1/tools/adapter_execution_evidence");
      return new Response(JSON.stringify({ ok: true, tool: "adapter_execution_evidence", mcp: { result: { structuredContent: result } } }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  assert.equal((await client.adapterExecutionEvidence(args)).mcp.result.structuredContent.evidence.execution_status, "succeeded");
  assert.equal((await client.adapterExecutionEvidenceTool(args)).mcp.result.structuredContent.adapter.id, "bioprism.python.vcf_text");
  await assert.rejects(client.adapterExecutionEvidence({ ...args, semantic_loss_status: "lossy" }), ArgumentError);
  await assert.rejects(client.adapterExecutionEvidence({ ...args, credential_material: "never" }), ArgumentError);
  await assert.rejects(client.adapterExecutionEvidence({ ...args, execution_status: "failed", error_code: null }), ArgumentError);
});
