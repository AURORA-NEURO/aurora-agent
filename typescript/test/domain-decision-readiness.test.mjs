import assert from "node:assert/strict";
import test from "node:test";
import { ApiClient, ArgumentError } from "../dist/index.js";

const args = {
  subject_id: "subject-ts",
  claim: { id: "claim-1", statement: "opaque" },
  reports: [{ schema: "canonical-report" }],
  links: [{ report_index: 0, role: "supports" }],
  policy: {
    required_group_ids: ["biological_domains"],
    required_domains: ["modalities"],
    minimum_supporting_reports: 1,
    minimum_qualifying_reports: 0,
  },
};

const readiness = {
  ok: true,
  schema: "bioprism-devplat-domain-decision-readiness/0.1",
  workflow: "domain_decision_readiness_audit",
  catalogue_digest: "a".repeat(64),
  readiness_claimed: false,
  execution: "not_started",
  audit: {
    decision_state: "ready_for_human_review",
    policy_satisfied: true,
    counts: { reports: 1, supporting_reports: 1, qualifying_reports: 0 },
    blockers: [],
    digest: "b".repeat(64),
  },
  artifact_registry: { indexed: true, kind: "domain_decision_readiness", content_digest: "c".repeat(64) },
};

const readinessQuery = {
  ok: true,
  schema: "bioprism-devplat-artifact-domain-decision-readiness-query/0.1",
  workflow: "artifact_registry_domain_decision_readiness_query",
  filters: { subject_id: "subject-ts", decision_state: "ready_for_human_review" },
  registry_generation: 2,
  registry_size: 1,
  rows: [{ content_digest: "c".repeat(64), audit_digest: "b".repeat(64), decision_state: "ready_for_human_review", policy_satisfied: true }],
  next_after: null,
  has_more: false,
  execution: "not_started",
  guarantees: [],
  does_not_claim: [],
};

function response(value) {
  return new Response(JSON.stringify(value), { status: 200, headers: { "content-type": "application/json" } });
}

test("domain decision readiness validates policy and preserves the structural audit", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input, init) => {
      const url = new URL(String(input));
      assert.equal(url.pathname, "/v1/tools/domain_decision_readiness_audit");
      assert.deepEqual(JSON.parse(init.body), args);
      return response({ ok: true, tool: "domain_decision_readiness_audit", mcp: { result: { structuredContent: readiness } } });
    },
  });
  const result = await client.domainDecisionReadinessAudit(args);
  assert.equal(result.mcp.result.structuredContent.audit.decision_state, "ready_for_human_review");
  await assert.rejects(
    client.domainDecisionReadinessAudit({ ...args, policy: { ...args.policy, minimum_supporting_reports: 0 } }),
    ArgumentError,
  );
  await assert.rejects(
    client.domainDecisionReadinessAudit({ ...args, links: [{ report_index: 4, role: "supports" }] }),
    ArgumentError,
  );
});

test("domain decision readiness query preserves bounded retained posture", async () => {
  const client = new ApiClient({
    baseUrl: "http://127.0.0.1:18788",
    fetch: async (input) => {
      const url = new URL(String(input));
      assert.equal(url.pathname, "/v1/domain-decision-readiness");
      assert.equal(url.searchParams.get("decision_state"), "ready_for_human_review");
      assert.equal(url.searchParams.get("policy_satisfied"), "true");
      return response(readinessQuery);
    },
  });
  const result = await client.domainDecisionReadinessQuery({
    subject_id: "subject-ts",
    decision_state: "ready_for_human_review",
    policy_satisfied: true,
    include_audits: true,
  });
  assert.equal(result.rows[0].audit_digest, "b".repeat(64));
  await assert.rejects(
    client.domainDecisionReadinessQuery({ decision_state: "not-a-readiness-state" }),
    ArgumentError,
  );
});
