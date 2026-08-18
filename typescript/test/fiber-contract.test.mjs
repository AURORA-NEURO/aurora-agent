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

test("epistemicAdaptiveExecute preserves explicit authorization and provenance counts", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/epistemic_adaptive_execute");
      const body = JSON.parse(String(init?.body));
      assert.equal(body.authorization.provider, "mcp-simulated");
      assert.equal(body.observations[0].outcome_label, "negative");
      return new Response(JSON.stringify({
        ok: true,
        tool: "epistemic_adaptive_execute",
        mcp: { result: { structuredContent: {
          ok: true,
          schema: "bioprism-epistemic/adaptive-execution/0.1",
          mode: "simulate",
          plan_digest: "a".repeat(64),
          completed: false,
          receipt: {
            schema: "bioprism-epistemic/adaptive-execution/0.1",
            plan_digest: "a".repeat(64),
            provider: "mcp-simulated",
            status: "refused",
            authorization: { granted: false, grant_id: null, provider: null },
            observations: [],
            actual_acquisition_cost: 0,
            terminal_action: null,
            terminal_risk: null,
            refusal: "authorization_required",
            refusal_detail: "no grant",
          },
          provenance_counts: { observed: 0, simulated: 0, replayed: 0 },
          guarantees: ["no-call"],
          limitations: ["simulation only"],
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });

  const response = await client.epistemicAdaptiveExecute({
    problem: { actions: ["m0"], models: ["m0"], loss: [0] },
    belief: { mass: [1] },
    acquisitions: [{ id: "screen", cost: 0.1, outcomes: [{ label: "negative", likelihood: [1] }] }],
    budget: 0.1,
    max_steps: 1,
    authorization: { grant_id: "grant-1", provider: "mcp-simulated" },
    observations: [{ acquisition_id: "screen", outcome_label: "negative" }],
  });
  assert.equal(response.mcp.result.structuredContent.receipt.refusal, "authorization_required");
});

test("epistemicAdaptiveCosted sends canonical vector dimensions", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/epistemic_adaptive_costed");
      const body = JSON.parse(String(init?.body));
      assert.equal(body.budget.latency_ms, 10);
      assert.equal(body.weights.latency_ms, 1);
      return new Response(JSON.stringify({
        ok: true,
        tool: "epistemic_adaptive_costed",
        mcp: { result: { structuredContent: {
          ok: true,
          schema: "bioprism-mcp/epistemic-adaptive-costed/0.1",
          cost_dimensions: ["tokens", "compute_ms", "latency_ms", "money_usd", "privacy_loss", "specimen_units", "expert_minutes"],
          policy: { expected_scalarized_cost: 0, root: { Stop: { action: 0, risk: 0.5 } } },
          guarantees: ["component-wise"],
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  const vector = { tokens: 100, compute_ms: 100, latency_ms: 10, money_usd: 1, privacy_loss: 1, specimen_units: 1, expert_minutes: 10 };
  const response = await client.epistemicAdaptiveCosted({
    problem: { actions: ["m0"], models: ["m0"], loss: [0] },
    belief: { mass: [1] },
    acquisitions: [{ acquisition: { id: "screen", cost: 0.1, outcomes: [{ label: "negative", likelihood: [1] }] }, cost: { ...vector, latency_ms: 100 } }],
    budget: vector,
    weights: { tokens: 0, compute_ms: 0, latency_ms: 1, money_usd: 0, privacy_loss: 0, specimen_units: 0, expert_minutes: 0 },
    max_steps: 1,
  });
  assert.equal(response.mcp.result.structuredContent.schema, "bioprism-mcp/epistemic-adaptive-costed/0.1");
});

test("interweaveWorkflowExecute preserves workflow identity and receipt-only posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/interweave_workflow_execute");
      const body = JSON.parse(String(init?.body));
      assert.equal(body.workflow, "incident_response");
      assert.equal(body.capabilities[0], "receipt-only");
      return new Response(JSON.stringify({
        ok: true,
        tool: "interweave_workflow_execute",
        mcp: { result: { structuredContent: {
          ok: true,
          schema: "bioprism-interweave/workflow-execution/0.1",
          mode: "simulate",
          workflow: "incident_response",
          plan_digest: "a".repeat(64),
          binding_digest: "b".repeat(64),
          binding: { workflow: "incident_response", binding_digest: "b".repeat(64) },
          completed: false,
          release_posture: "workflow_receipt_only_external_release_not_authorized",
          receipt: { schema: "bioprism-interweave/workflow-execution/0.1" },
          provenance_counts: { observed: 0, simulated: 0, replayed: 0 },
          guarantees: ["receipt"],
          limitations: ["simulator"],
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  const response = await client.interweaveWorkflowExecute({
    workflow: "incident_response",
    problem: { actions: ["m0"], models: ["m0"], loss: [0] },
    belief: { mass: [1] },
    acquisitions: [{ id: "screen", cost: 0.1, outcomes: [{ label: "negative", likelihood: [1] }] }],
    budget: 0.1,
    max_steps: 1,
    capabilities: ["receipt-only"],
  });
  assert.equal(response.mcp.result.structuredContent.workflow, "incident_response");
  assert.equal(response.mcp.result.structuredContent.release_posture, "workflow_receipt_only_external_release_not_authorized");
});

test("interweaveWorkflowExecutionEvidence validates bounded labels and exposes digest records", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      assert.equal(new URL(String(input)).pathname, "/v1/tools/interweave_workflow_execution_evidence");
      const body = JSON.parse(String(init?.body));
      assert.equal(body.subject_id, "case-1");
      assert.deepEqual(body.domains, ["incident_response"]);
      return new Response(JSON.stringify({
        ok: true,
        tool: "interweave_workflow_execution_evidence",
        mcp: { result: { structuredContent: {
          ok: true,
          schema: "bioprism-devplat-workflow-execution-evidence/0.1",
          workflow: "interweave_workflow_execution_evidence",
          evidence_digest: "c".repeat(64),
          evidence: { evidence_digest: "c".repeat(64), readiness_claimed: false, execution: "not_started" },
          registry: { created: true },
        } } },
      }), { status: 200, headers: { "content-type": "application/json" } });
    },
  });
  const response = await client.interweaveWorkflowExecutionEvidence({
    binding: { binding_digest: "b".repeat(64) },
    receipt: { schema: "bioprism-interweave/workflow-execution/0.1" },
    subject_id: "case-1",
    domains: ["incident_response"],
    parent_digests: ["a".repeat(64)],
  });
  assert.equal(response.mcp.result.structuredContent.evidence_digest, "c".repeat(64));
});
