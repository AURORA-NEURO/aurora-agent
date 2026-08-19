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
      if (url.pathname === "/v1/control-plane-readiness" && init.method === "GET") return response(query);
      throw new Error(`unexpected path ${url.pathname}`);
    },
  });
  const mcp = await client.controlPlaneReadinessAudit(args);
  assert.equal(mcp.mcp.result.structuredContent.audit.control_plane_state, "ready_for_human_review");
  const rest = await client.controlPlaneReadinessAuditRest(args);
  assert.equal(rest.artifact_registry.content_digest, "b".repeat(64));
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
