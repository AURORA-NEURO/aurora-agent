import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const args = {
  subject_id: "subject-control-plane",
  policy: { require_route_review: true, require_release_ready: true },
  readiness_audit: { workflow: "domain_decision_readiness_audit" },
  route_review: { workflow: "capability_route_review" },
};

const result = {
  ok: true,
  schema: "bioprism-control-plane-readiness/0.1",
  workflow: "control_plane_readiness_audit",
  readiness_claimed: false,
  execution: "not_started",
  audit: {
    subject_id: "subject-control-plane",
    control_plane_state: "ready_for_human_review",
    policy_satisfied: true,
    components: { domain_decision_readiness: { satisfied: true } },
    component_states: { domain_decision_readiness: { state: "ready_for_human_review" } },
    component_count: 5,
    blockers: [],
    digest: "a".repeat(64),
  },
  artifact_registry: { indexed: true, content_digest: "b".repeat(64) },
};

const query = {
  ok: true,
  schema: "bioprism-devplat-artifact-control-plane-readiness-query/0.1",
  workflow: "artifact_registry_control_plane_readiness_query",
  rows: [{ content_digest: "b".repeat(64) }],
  next_after: null,
  has_more: false,
  registry_generation: 2,
  registry_size: 1,
};

const comparison = {
  ok: true,
  schema: "bioprism-control-plane-readiness-compare/0.1",
  workflow: "control_plane_readiness_compare",
  comparison: {
    subject_id: "subject-control-plane",
    state_direction: "unchanged",
    evidence_direction: "unchanged",
    component_changes: [],
    blockers_added: [],
    blockers_removed: [],
    improvements: [],
    regressions: [],
    comparison_digest: "c".repeat(64),
  },
  readiness_claimed: false,
  execution: "not_started",
};

const retainedComparison = {
  ok: true,
  schema: "bioprism-control-plane-readiness-compare-retained/0.1",
  workflow: "control_plane_readiness_compare_retained",
  subject_id: "subject-control-plane",
  before_content_digest: "b".repeat(64),
  after_content_digest: "d".repeat(64),
  source: "content_addressed_artifact_registry",
  comparison: comparison.comparison,
  readiness_claimed: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

function response(value) {
  return new Response(JSON.stringify(value), { status: 200, headers: { "content-type": "application/json" } });
}

test("control-plane readiness preserves explicit packets through MCP and REST", async () => {
  const seen = [];
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const url = new URL(String(input));
      seen.push({ path: url.pathname, body: init?.body ? JSON.parse(init.body) : undefined });
      if (url.pathname === "/v1/tools/control_plane_readiness_audit") return response({ ok: true, tool: "control_plane_readiness_audit", mcp: { result: { structuredContent: result } } });
      if (url.pathname === "/v1/control-plane-readiness" && init.method === "POST") return response(result);
      if (url.pathname === "/v1/tools/control_plane_readiness_compare") return response({ ok: true, tool: "control_plane_readiness_compare", mcp: { result: { structuredContent: comparison } } });
      if (url.pathname === "/v1/control-plane-readiness/compare") return response(comparison);
      if (url.pathname === "/v1/tools/control_plane_readiness_compare_retained") return response({ ok: true, tool: "control_plane_readiness_compare_retained", mcp: { result: { structuredContent: retainedComparison } } });
      if (url.pathname === "/v1/control-plane-readiness/compare-retained") return response(retainedComparison);
      if (url.pathname === "/v1/control-plane-readiness" && init.method === "GET") return response(query);
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  const mcp = await client.controlPlaneReadinessAudit(args);
  assert.equal(mcp.mcp.result.structuredContent.audit.control_plane_state, "ready_for_human_review");
  const rest = await client.controlPlaneReadinessAuditRest(args);
  assert.equal(rest.artifact_registry.content_digest, "b".repeat(64));
  const compared = await client.controlPlaneReadinessCompare({ before: result, after: result });
  assert.equal(compared.mcp.result.structuredContent.comparison.evidence_direction, "unchanged");
  const comparedRest = await client.controlPlaneReadinessCompareRest({ before: result, after: result });
  assert.equal(comparedRest.comparison.comparison_digest, "c".repeat(64));
  const retained = await client.controlPlaneReadinessCompareRetained({
    before_content_digest: "b".repeat(64),
    after_content_digest: "d".repeat(64),
  });
  assert.equal(retained.mcp.result.structuredContent.workflow, "control_plane_readiness_compare_retained");
  const retainedRest = await client.controlPlaneReadinessCompareRetainedRest({
    before_content_digest: "b".repeat(64),
    after_content_digest: "d".repeat(64),
    subject_id: "subject-control-plane",
  });
  assert.equal(retainedRest.after_content_digest, "d".repeat(64));
  await assert.rejects(client.controlPlaneReadinessCompareRetained({
    before_content_digest: "not-a-digest",
    after_content_digest: "d".repeat(64),
  }), ArgumentError);
  assert.equal(seen[0].body.route_review.workflow, "capability_route_review");
  await assert.rejects(client.controlPlaneReadinessAudit({ ...args, subject_id: "" }), ArgumentError);
});

test("control-plane readiness query validates structural state and cursor filters", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      const url = new URL(String(input));
      assert.equal(url.pathname, "/v1/control-plane-readiness");
      assert.equal(url.searchParams.get("control_plane_state"), "ready_for_human_review");
      return response(query);
    },
  });
  const page = await client.controlPlaneReadinessQuery({ control_plane_state: "ready_for_human_review", policy_satisfied: true });
  assert.equal(page.rows[0].content_digest, "b".repeat(64));
  await assert.rejects(client.controlPlaneReadinessQuery({ control_plane_state: "unknown" }), ArgumentError);
});
