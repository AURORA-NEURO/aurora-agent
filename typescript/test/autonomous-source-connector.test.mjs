import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousConnectorObservation,
  createAutonomousApiSourceConnectorExecutor,
} from "../dist/index.js";

const planDigest = "a".repeat(64);
const toolPlanDigest = "b".repeat(64);
const parentDigest = "c".repeat(64);

function manifest(domains = AUTONOMOUS_DOMAIN_NAMES) {
  return {
    schema: "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1",
    connector_id: "source.api.test",
    version: "1.0.0",
    provider: "test-source",
    connector_kind: "provider_api",
    domains: [...domains],
    capabilities: ["source_plan", "source_execute"],
    transport: "caller_managed",
    auth_posture: { status: "delegated", does_not_claim: ["the bridge does not own credentials"] },
  };
}

function planRequest(domains = AUTONOMOUS_DOMAIN_NAMES) {
  return {
    group_id: "evidence-group",
    domains: [...domains],
    subject_id: "subject-digest",
    source_tool: "test-source-search",
    connector_kind: "provider_api",
    locator_kind: "uri",
    locator: "https://source.test/query",
    retrieval_mode: "metadata_only",
    parent_digests: [parentDigest],
    does_not_claim: ["retrieval is not domain truth"],
  };
}

function directClient(seen) {
  return {
    async domainEvidenceSourcePlan(args) {
      seen.plan = args;
      return { ok: true, plan_digest: planDigest, workflow: "domain_evidence_source_plan" };
    },
    async domainEvidenceSourceExecute(args) {
      seen.execute = args;
      return {
        ok: true,
        outcome: "observed",
        source_plan_digest: args.source_plan_digest,
        response_digest: "d".repeat(64),
        execution_result: { retrieval: { body_encoding: "omitted" } },
      };
    },
  };
}

test("API source connector binds planning output into execution across every autonomous domain", async () => {
  const seen = {};
  const executor = createAutonomousApiSourceConnectorExecutor(directClient(seen));
  const observation = await executor(manifest(), {
    plan: planRequest(),
    execution: {
      request: { query: "transient source query" },
      claim_posture: { status: "review_required", does_not_claim: ["source output is not truth"] },
      parent_digests: [parentDigest],
    },
  });

  assert.ok(observation instanceof AutonomousConnectorObservation);
  assert.equal(observation.status, "observed");
  assert.equal(seen.plan.domains.length, AUTONOMOUS_DOMAIN_NAMES.length);
  assert.equal(seen.execute.source_plan_digest, planDigest);
  assert.deepEqual(seen.execute.parent_digests, [parentDigest]);
  assert.equal(observation.value.source_plan_digest, planDigest);
  assert.doesNotMatch(JSON.stringify(observation), /api[_ -]?key|authorization\s*:/i);
});

test("API source connector supports the MCP tool route without accepting a caller plan digest", async () => {
  const seen = {};
  const client = {
    async domainEvidenceSourcePlanTool(args) {
      seen.plan = args;
      return { ok: true, mcp: { result: { structuredContent: { ok: true, plan_digest: toolPlanDigest } } } };
    },
    async domainEvidenceSourceExecuteTool(args) {
      seen.execute = args;
      return { ok: true, mcp: { result: { structuredContent: { ok: true, outcome: "partial", source_plan_digest: args.source_plan_digest, response_digest: "e".repeat(64) } } } };
    },
    async domainEvidenceSourcePlan() { throw new Error("direct route must not be called"); },
    async domainEvidenceSourceExecute() { throw new Error("direct route must not be called"); },
  };
  const executor = createAutonomousApiSourceConnectorExecutor(client, { useToolRoute: true });
  const observation = await executor(manifest(), { plan: planRequest(), execution: { source_plan_digest: "f".repeat(64) } });

  assert.equal(observation.status, "partial");
  assert.equal(seen.execute.source_plan_digest, toolPlanDigest);
  assert.notEqual(seen.execute.source_plan_digest, "f".repeat(64));
});

test("API source connector fails closed on scope, plan identity, and response status errors", async () => {
  const executor = createAutonomousApiSourceConnectorExecutor({
    async domainEvidenceSourcePlan() { return { ok: true }; },
    async domainEvidenceSourceExecute() { return { ok: true, outcome: "not-a-status" }; },
  });
  await assert.rejects(() => executor(manifest(["coding"]), { plan: planRequest(["data"]), execution: {} }), /scope/);
  await assert.rejects(() => executor(manifest(), { plan: planRequest(), execution: {} }), /omitted its digest/);

  const invalidStatus = createAutonomousApiSourceConnectorExecutor({
    async domainEvidenceSourcePlan() { return { ok: true, plan_digest: planDigest }; },
    async domainEvidenceSourceExecute() { return { ok: true, outcome: "not-a-status" }; },
  });
  await assert.rejects(() => invalidStatus(manifest(), { plan: planRequest(), execution: {} }), /response is malformed/);
  assert.throws(() => createAutonomousApiSourceConnectorExecutor({}), /requires a configured ApiClient/);
});
