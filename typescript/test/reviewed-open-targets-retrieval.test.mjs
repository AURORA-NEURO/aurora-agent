import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_OPEN_TARGETS_TRANSPORT_ID,
  REVIEWED_OPEN_TARGETS_LANES,
  AutonomousEvidenceAdapterRegistry,
  ReviewedOpenTargetsRetrievalAdapter,
  ReviewedOpenTargetsRetrievalConfig,
  ReviewedOpenTargetsRetrievalError,
  ReviewedOpenTargetsRetrievalPlan,
  createReviewedOpenTargetsAutonomousEvidenceRegistration,
  createReviewedOpenTargetsExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const TRANSPORT = {
  transportId: "fixture.open-targets",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";

function response(lane = "gbm", { total = 3, scores = [0.75, 0.5] } = {}) {
  const [id, name] = lane === "gbm" ? ["MONDO_0018177", "glioblastoma"] : ["MONDO_0021637", "low grade glioma"];
  return {
    data: {
      disease: {
        id,
        name,
        associatedTargets: {
          count: total,
          rows: scores.map((score, index) => ({
            target: { id: `ENSG${String(index + 1).padStart(11, "0")}`, approvedSymbol: ["TP53", "EGFR", "TERT"][index] },
            score,
          })),
        },
      },
    },
  };
}

function fixtureAdapter(responses = [response()], options = {}) {
  const config = new ReviewedOpenTargetsRetrievalConfig({ lanes: options.lanes ?? ["gbm"], pageSize: options.pageSize ?? 2, ...TRANSPORT });
  const calls = [];
  const adapter = new ReviewedOpenTargetsRetrievalAdapter(config, { fetch: (url, body) => { calls.push({ url, body: JSON.parse(body) }); return JSON.stringify(responses[calls.length - 1]); } });
  return { config, adapter, calls };
}

test("reviewed Open Targets fixture config, plan, and output digests match Python", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  assert.equal(Object.isFrozen(REVIEWED_OPEN_TARGETS_LANES), true);
  assert.throws(() => { REVIEWED_OPEN_TARGETS_LANES.other = "MONDO_0000000"; }, TypeError);
  assert.match(BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST, /^[0-9a-f]{64}$/);
  assert.equal(fixture.config.configDigest, "7a7f94256ebba8cec8bfce6310f01ddfff0b9d5496b4b33d06f5cb8d377de08a");
  assert.equal(plan.planDigest, "3cba77e6ddc03b360e7c075a55501f9f675371a0236251b9a463205cf0b987ce");
  assert.deepEqual(ReviewedOpenTargetsRetrievalConfig.fromJSON(fixture.config.toJSON()).toJSON(), fixture.config.toJSON());
  assert.deepEqual(ReviewedOpenTargetsRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.bundle_digest, "10a3a1a9f3121644a0b283b834afa11d129e02532fa49fa0f5c4d4abd476499b");
  assert.equal(result.receipt.receipt_digest, "1472957e7165c725cd59c321a5b176632f4d23fc13fb0b1fd3a483740b9b51d2");
  assert.equal(result.receipt.response_bytes, 251);
  const detached = result.receipt; detached.bundle_digest = "tampered";
  assert.notEqual(result.receipt.bundle_digest, "tampered");
});

test("dispatch requires literal approval and sends only a bounded fixed GraphQL request", async () => {
  const fixture = fixtureAdapter(); const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls.length, 0);
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls.length, 1);
  assert.equal(fixture.calls[0].url, "https://api.platform.opentargets.org/api/v4/graphql");
  assert.deepEqual(fixture.calls[0].body.variables, { efoId: "MONDO_0018177", pageIndex: 0, pageSize: 2 });
  const row = result.bundle.associations[0];
  assert.equal(row.total_associations, 3);
  assert.equal(row.returned_associations, 2);
  assert.equal(row.omitted_associations, 1);
  assert.equal(row.coverage, "top_ranked_page_only");
  assert.equal(row.targets[0].approved_symbol, "TP53");
  assert.equal(row.score_interpretation, "ranking_only_not_confidence");
  assert.equal(result.receipt.coverage, "top_ranked_pages_only");
  assert.equal(result.receipt.request_count, 1);
  assert.equal(JSON.stringify(result.bundle).includes('"confidence"'), false);
});

test("two disease lanes use deterministic order and carry per-lane truncation", async () => {
  const fixture = fixtureAdapter([response(), response("lgg", { total: 1, scores: [0.25] })], { lanes: ["lgg", "gbm"] });
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(fixture.calls.map((call) => call.body.variables.efoId), ["MONDO_0018177", "MONDO_0021637"]);
  assert.equal(result.receipt.request_count, 2);
  assert.equal(result.bundle.coverage, "top_ranked_pages_only");
  assert.equal(result.bundle.associations[1].coverage, "all_source_rows");
});

test("duplicate JSON fields, foreign disease identity, invalid scores, and transport drift fail closed", async () => {
  const config = new ReviewedOpenTargetsRetrievalConfig({ pageSize: 2, ...TRANSPORT });
  const duplicate = new ReviewedOpenTargetsRetrievalAdapter(config, { fetch: () => '{"data":{"disease":{"id":"MONDO_0018177","id":"MONDO_0021637"}}}' });
  await assert.rejects(duplicate.execute(duplicate.prepare(), { approveSourceDispatch: true }), /duplicate JSON fields/);
  const foreign = new ReviewedOpenTargetsRetrievalAdapter(config, { fetch: () => JSON.stringify(response("lgg")) });
  await assert.rejects(foreign.execute(foreign.prepare(), { approveSourceDispatch: true }), /identity differs/);
  const invalid = response(); invalid.data.disease.associatedTargets.rows[0].score = 1.5;
  const badScore = new ReviewedOpenTargetsRetrievalAdapter(config, { fetch: () => JSON.stringify(invalid) });
  await assert.rejects(badScore.execute(badScore.prepare(), { approveSourceDispatch: true }), /score is outside/);
  const outOfOrder = response("gbm", { scores: [0.5, 0.75] });
  const badOrder = new ReviewedOpenTargetsRetrievalAdapter(config, { fetch: () => JSON.stringify(outOfOrder) });
  await assert.rejects(badOrder.execute(badOrder.prepare(), { approveSourceDispatch: true }), /descending bounded range/);
  assert.equal(new ReviewedOpenTargetsRetrievalConfig().transportId, BUILTIN_OPEN_TARGETS_TRANSPORT_ID);
  assert.equal(new ReviewedOpenTargetsRetrievalConfig().transportConfigDigest, BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST);
  assert.throws(() => new ReviewedOpenTargetsRetrievalAdapter(new ReviewedOpenTargetsRetrievalConfig(), { fetch: () => "{}" }), /distinct reviewed identity/);
});

test("autonomous evidence registration retains only provenance digests", async () => {
  const fixture = fixtureAdapter(); const plan = fixture.adapter.prepare();
  const registration = createReviewedOpenTargetsAutonomousEvidenceRegistration(fixture.adapter, plan, "gbm");
  const manifest = new AutonomousEvidenceAdapterRegistry().register(registration);
  assert.equal(manifest.adapter_id, "reviewed.open_targets.gbm");
  const metadata = createReviewedOpenTargetsExecutionMetadata(plan, true, RETRIEVED_AT);
  const context = { request: { source_id: "open_targets_gbm", source_digest: plan.planDigest, metadata }, requirement: { label: "Open Targets association review" } };
  const transient = await registration.acquire(context);
  const observations = await registration.project(transient, context);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, transient.receipt.bundle_digest);
  assert.equal(observations[0].confidence, null);
  assert.equal(JSON.stringify(observations).includes("TP53"), false);
  assert.equal(JSON.stringify(observations).includes("association_score"), false);
  const tampered = JSON.parse(JSON.stringify(transient));
  tampered.bundle.associations[0].targets[0].association_score = 0.1;
  await assert.rejects(registration.project(tampered, context), /transient digests are invalid/);
  const forged = { ...metadata, approve_source_dispatch: false };
  forged.metadata_digest = digestJsonSync(Object.fromEntries(Object.entries(forged).filter(([key]) => key !== "metadata_digest")));
  await assert.rejects(registration.acquire({ request: { ...context.request, metadata: forged } }), /failed review binding/);
});
