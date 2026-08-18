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

test("fiberCompileRateDistortion exposes the exhaustive observed-context projection", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/fiber_compile");
      return new Response(JSON.stringify({
        ok: true,
        tool: "fiber_compile",
        request_id: "fiber-2",
        mcp: { result: { structuredContent: {
          layer: "l0",
          rate_distortion: {
            schema: "bioprism-mcp/epistemic-context-audit/0.2",
            criterion: "bayes_regret",
            tolerance: 0.25,
            compatibility_floor: 0.05,
            evidence_count: 2,
            full_rate: 3,
            identification: { status: "point_identified" },
            sufficiency: { outcome: "sufficient" },
            frontier: { evaluated: 4, points: [] },
            certificate_binding: { query_sha256: "a".repeat(64), certificate_sha256: "b".repeat(64) },
            guarantees: ["exhaustive"],
            limitations: ["caller-declared"],
          },
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const response = await client.fiberCompileRateDistortion({ world: "fixtures/world.json", query: "fixtures/query-v04.json" });
  const projection = response.mcp.result.structuredContent.rate_distortion;
  assert.equal(projection.schema, "bioprism-mcp/epistemic-context-audit/0.2");
  assert.equal(projection.frontier.evaluated, 4);
  assert.equal(projection.certificate_binding.query_sha256.length, 64);
});

test("fiberCompileAdaptiveAcquisition exposes certificate-bound planning posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/fiber_compile");
      return new Response(JSON.stringify({
        ok: true,
        tool: "fiber_compile",
        request_id: "fiber-3",
        mcp: { result: { structuredContent: {
          layer: "l0",
          adaptive_acquisition: {
            schema: "bioprism-mcp/fiber-adaptive-acquisition/0.1",
            budget: 1,
            max_steps: 2,
            prior: [0.5, 0.5],
            problem: { actions: ["accept"], models: ["m0", "m1"], action_count: 1, model_count: 2 },
            acquisitions: [],
            policy: { expected_total: 0, expected_terminal_risk: 0, expected_acquisition_cost: 0, nodes_evaluated: 1, selected_depth: 0, root: { kind: "stop", action_index: 0, action: "accept", risk: 0 } },
            certificate_binding: { query_sha256: "a".repeat(64), certificate_sha256: "b".repeat(64) },
            execution: "not_started",
            authorization: "not_granted",
            provenance: { planner: "bioprism-epistemic::adaptive_policy" },
            guarantees: ["exact"],
            limitations: ["not execution"],
          },
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const response = await client.fiberCompileAdaptiveAcquisition({ world: "fixtures/world.json", query: "fixtures/query-v05.json" });
  const projection = response.mcp.result.structuredContent.adaptive_acquisition;
  assert.equal(projection.schema, "bioprism-mcp/fiber-adaptive-acquisition/0.1");
  assert.equal(projection.execution, "not_started");
  assert.equal(projection.authorization, "not_granted");
});
