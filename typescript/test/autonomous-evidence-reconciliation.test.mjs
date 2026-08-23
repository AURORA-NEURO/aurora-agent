import assert from "node:assert/strict";
import test from "node:test";

import {
  AUTONOMOUS_DOMAIN_NAMES,
  AutonomousAgent,
  AutonomousEvidenceAcquisitionError,
  AutonomousEvidenceSourceReconciler,
  LLMRuntime,
  digestJsonSync,
} from "../dist/index.js";

function offlineAgent() {
  return new AutonomousAgent(new LLMRuntime({ fetch: async () => { throw new Error("network must not be reached"); } }));
}

function route(sourceId, domain, answer, options = {}) {
  const sourceDigest = digestJsonSync({ sourceId, revision: "offline" });
  return {
    source_id: sourceId,
    source_digest: sourceDigest,
    request_id: options.requestId ?? null,
    metadata: { operation: "retrieve", domain, query: options.query ?? `bounded-${domain}` },
    acquirer: {
      acquire: async () => {
        if (options.error) throw options.error;
        return { answer, domain, provider_only_marker: options.marker ?? sourceId };
      },
    },
  };
}

test("reviewed reconciliation reaches normalized consensus across all autonomous domains with bounded concurrency", async () => {
  const plan = await offlineAgent().evidencePlan(AUTONOMOUS_DOMAIN_NAMES);
  const reconciler = new AutonomousEvidenceSourceReconciler(plan);
  let inFlight = 0;
  let maxInFlight = 0;
  for (const domain of AUTONOMOUS_DOMAIN_NAMES) {
    const requirement = plan.requirements.find((candidate) => candidate.domain === domain);
    const makeRoute = (sourceId, marker) => ({
      ...route(sourceId, domain, `answer-${domain}`, { marker }),
      acquirer: {
        acquire: async () => {
          inFlight += 1;
          maxInFlight = Math.max(maxInFlight, inFlight);
          await Promise.resolve();
          inFlight -= 1;
          return { answer: `answer-${domain}`, domain, provider_only_marker: marker };
        },
      },
    });
    const routes = [makeRoute(`${domain}-source-a`, "a"), makeRoute(`${domain}-source-b`, "b")];
    const reconciliationPlan = reconciler.prepare(requirement.requirement_id, routes, {
      quorum: 2,
      maxConcurrency: 2,
      normalizerId: "answer-only",
      normalizerVersion: "1",
    });
    assert.equal(reconciliationPlan.toJSON().routes.every((row) => !JSON.stringify(row).includes(`answer-${domain}`)), true);
    const result = await reconciler.execute(reconciliationPlan, routes, {
      approveSourceDispatch: true,
      normalizerId: "answer-only",
      normalizerVersion: "1",
      normalizer: (value) => ({ answer: value.answer, domain: value.domain }),
    });
    assert.equal(result.json.status, "consensus");
    assert.equal(result.json.observed_count, 2);
    assert.equal(result.json.unique_normalized_count, 1);
    assert.equal(result.json.failed_count, 0);
    assert.equal(result.json.consensus_normalized_digest.length, 64);
    assert.equal(JSON.stringify(result.json).includes("provider_only_marker"), false);
    assert.equal(result.values[`${domain}-source-a`].provider_only_marker, "a");
  }
  assert.ok(maxInFlight <= 2);
});

test("reconciliation preserves dissent and typed source failures instead of inventing consensus", async () => {
  const plan = await offlineAgent().evidencePlan(["science"]);
  const reconciler = new AutonomousEvidenceSourceReconciler(plan);
  const requirement = plan.requirements.find((candidate) => candidate.domain === "science");
  const routes = [
    route("science-a", "science", "claim-a"),
    route("science-b", "science", "claim-b"),
    route("science-failed", "science", "unused", { error: new AutonomousEvidenceAcquisitionError("transport_error", true) }),
  ];
  const reconciliationPlan = reconciler.prepare(requirement.requirement_id, routes, { quorum: 2, requireAll: false });
  const result = await reconciler.execute(reconciliationPlan, routes, { approveSourceDispatch: true });
  assert.equal(result.json.status, "disagreement");
  assert.equal(result.json.observed_count, 2);
  assert.equal(result.json.failed_count, 1);
  assert.equal(result.json.consensus_normalized_digest, null);
  assert.equal(typeof result.json.disagreement_digest, "string");
  assert.equal(result.json.source_results.find((row) => row.source_id === "science-failed").failure_class, "transport_error");
  assert.equal(result.json.source_results.find((row) => row.source_id === "science-failed").retryable, true);
});

test("reconciliation fails closed on approval, route metadata, normalizer, and secret-boundary drift", async () => {
  const plan = await offlineAgent().evidencePlan(["coding"]);
  const reconciler = new AutonomousEvidenceSourceReconciler(plan);
  const requirement = plan.requirements.find((candidate) => candidate.domain === "coding");
  const routes = [route("coding-a", "coding", "claim", { query: "private task text" }), route("coding-b", "coding", "claim")];
  const reconciliationPlan = reconciler.prepare(requirement.requirement_id, routes, { normalizerId: "answer-only", normalizerVersion: "1" });
  assert.equal(JSON.stringify(reconciliationPlan.toJSON()).includes("private task text"), false);
  await assert.rejects(() => reconciler.execute(reconciliationPlan, routes), /explicit approval/);
  const changedRoute = routes.map((item) => item.source_id === "coding-a" ? { ...item, metadata: { ...item.metadata, query: "changed" } } : item);
  await assert.rejects(() => reconciler.execute(reconciliationPlan, changedRoute, { approveSourceDispatch: true, normalizerId: "answer-only", normalizerVersion: "1", normalizer: (value) => ({ answer: value.answer }) }), /route changed/);
  await assert.rejects(() => reconciler.execute(reconciliationPlan, routes, { approveSourceDispatch: true, normalizerId: "other", normalizerVersion: "1", normalizer: (value) => value }), /normalizer contract changed/);
  const secretRoute = route("coding-secret", "coding", "claim");
  secretRoute.metadata = { operation: "retrieve", authorization: "must reject" };
  assert.throws(() => reconciler.prepare(requirement.requirement_id, [secretRoute]), /credential-shaped/);
});
