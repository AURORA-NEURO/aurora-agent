import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient } from "../dist/index.js";

test("fiberCompile exposes the versioned decision quotient projection with certificate binding", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/fiber_compile");
      return new Response(JSON.stringify({
        ok: true,
        tool: "fiber_compile",
        request_id: "fiber-1",
        mcp: {
          result: {
            structuredContent: {
              layer: "l0",
              decision_quotient: {
                schema: "bioprism-mcp/epistemic-decision-quotient/0.1",
                basis: "permitted_loss_difference_profile",
                permitted_actions: ["accept", "defer", "reject"],
                original_model_count: 3,
                quotient_model_count: 2,
                merged_model_count: 1,
                compressed: true,
                compression_fraction: 2 / 3,
                certificate_binding: { query_sha256: "a".repeat(64), certificate_sha256: "b".repeat(64) },
                limitations: ["not rate-distortion"],
              },
            },
          },
        },
        guarantee: "bounded",
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const response = await client.fiberCompile({ world: "fixtures/world.json", query: "fixtures/query.json" });
  const projection = response.mcp.result.structuredContent.decision_quotient;
  assert.equal(projection.schema, "bioprism-mcp/epistemic-decision-quotient/0.1");
  assert.deepEqual(projection.permitted_actions, ["accept", "defer", "reject"]);
  assert.equal(projection.certificate_binding.query_sha256.length, 64);
  assert.equal(projection.quotient_model_count, 2);
});
