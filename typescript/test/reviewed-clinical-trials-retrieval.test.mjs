import assert from "node:assert/strict";
import test from "node:test";

import {
  AutonomousEvidenceAdapterRegistry,
  BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID,
  MAX_REVIEWED_CLINICAL_TRIALS_RECORDS,
  REVIEWED_CLINICAL_TRIALS_FIELDS,
  REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA,
  ReviewedClinicalTrialsRetrievalAdapter,
  ReviewedClinicalTrialsRetrievalConfig,
  ReviewedClinicalTrialsRetrievalError,
  createReviewedClinicalTrialsAutonomousEvidenceRegistration,
  createReviewedClinicalTrialsExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const CUSTOM_TRANSPORT = {
  transportId: "fixture.clinicaltrials",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";

function study(nctId = "NCT01234567", title = "Reviewed registry study") {
  return {
    protocolSection: {
      identificationModule: { nctId, briefTitle: title },
      statusModule: { overallStatus: "RECRUITING", lastUpdatePostDateStruct: { date: "2025-01" } },
      designModule: { phases: ["PHASE2"], studyType: "INTERVENTIONAL", enrollmentInfo: { count: 42 } },
      armsInterventionsModule: { interventions: [{ name: "Study drug" }, { name: null }] },
    },
  };
}

function response(studies, totalCount, nextPageToken) {
  return { totalCount, studies, ...(nextPageToken === undefined ? {} : { nextPageToken }) };
}

function fixture(responses, options = {}) {
  const calls = [];
  const config = new ReviewedClinicalTrialsRetrievalConfig({
    conditionLanes: options.lanes ?? ["glioblastoma"],
    pageSize: options.pageSize ?? 2,
    maxPages: options.maxPages ?? 2,
    ...CUSTOM_TRANSPORT,
  });
  const adapter = new ReviewedClinicalTrialsRetrievalAdapter(config, {
    fetch: async (url) => { calls.push(url); return responses[calls.length - 1]; },
  });
  return { config, adapter, calls };
}

test("preflight is pure, deterministic, and restricted to fixed source lanes", () => {
  let calls = 0;
  const config = new ReviewedClinicalTrialsRetrievalConfig(CUSTOM_TRANSPORT);
  const adapter = new ReviewedClinicalTrialsRetrievalAdapter(config, { fetch: () => { calls += 1; return {}; } });
  const plan = adapter.prepare();
  assert.deepEqual(adapter.prepare().toJSON(), plan.toJSON());
  assert.equal(calls, 0);
  assert.equal(MAX_REVIEWED_CLINICAL_TRIALS_RECORDS, 1000);
  assert.throws(() => REVIEWED_CLINICAL_TRIALS_FIELDS.push("EligibilityCriteria"), TypeError);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: [] }), /fixed, unique/);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["cardiology"] }), /fixed, unique/);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["__proto__"] }), /fixed, unique/);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["glioma", "glioma"] }), /fixed, unique/);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["glioma"], query: "arbitrary" }), /unsupported field/);
});

test("built-in and injected transport identities cannot be confused", () => {
  const builtin = new ReviewedClinicalTrialsRetrievalConfig();
  assert.equal(builtin.toJSON().transport_id, BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID);
  assert.equal(builtin.toJSON().transport_config_digest, BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST);
  assert.equal(BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST, "88be25f8d87736595bfef98b41fd73ed7a2c869f043e0c7b903da68c55d62328");
  assert.equal(builtin.config_digest, "985018e7076de7e76bef6e4a68e78ba9a699a0f979c234db4ff6da6a149135a1");
  assert.throws(() => new ReviewedClinicalTrialsRetrievalAdapter(builtin, { fetch: () => ({}) }), /distinct reviewed identity/);
  assert.throws(() => new ReviewedClinicalTrialsRetrievalAdapter(new ReviewedClinicalTrialsRetrievalConfig(CUSTOM_TRANSPORT)), /identity is not exact/);
});

test("execution requires literal approval and returns transient rows behind a metadata-only JSON projection", async () => {
  const state = fixture([response([study()], 1)]);
  const plan = state.adapter.prepare();
  await assert.rejects(state.adapter.execute(plan, { approveSourceDispatch: false }), /literal source-dispatch approval/);
  assert.equal(state.calls.length, 0);
  const result = await state.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.schema, REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA);
  assert.equal(result.bundle.trials[0].nct_id, "NCT01234567");
  assert.deepEqual(result.bundle.trials[0].intervention_names, ["Study drug"]);
  assert.doesNotMatch(JSON.stringify(result.toJSON()), /Reviewed registry study|Study drug/);
  const transient = result.bundle;
  transient.trials[0].title = "mutated";
  assert.equal(result.bundle.trials[0].title, "Reviewed registry study");
  assert.equal(result.toTransientJSON().bundle.trials.length, 1);
});

test("pagination URLs are exact, bounded, and report source completeness", async () => {
  const token = "cursor/+==";
  const state = fixture([response([study()], 2, token), response([study("NCT76543210", "Second study")], 2)]);
  const result = await state.adapter.execute(state.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(state.calls.length, 2);
  for (const [index, value] of state.calls.entries()) {
    const url = new URL(value);
    assert.equal(url.protocol, "https:");
    assert.equal(url.hostname, "clinicaltrials.gov");
    assert.equal(url.pathname, "/api/v2/studies");
    assert.equal(url.searchParams.get("format"), "json");
    assert.equal(url.searchParams.get("pageSize"), "2");
    assert.equal(url.searchParams.get("query.cond"), "Glioblastoma");
    if (index === 0) assert.equal(url.searchParams.has("pageToken"), false);
    else assert.equal(url.searchParams.get("pageToken"), token);
  }
  assert.equal(result.receipt.request_count, 2);
  assert.equal(result.receipt.reported_total_count, 2);
  assert.equal(result.receipt.truncated, false);
  assert.deepEqual(result.bundle.trials.map((row) => row.nct_id), ["NCT01234567", "NCT76543210"]);
  const incomplete = fixture([response([study()], 3)]);
  const truncated = await incomplete.adapter.execute(incomplete.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(truncated.receipt.truncated, true);
  assert.equal(truncated.receipt.omitted_record_count, 2);
});

test("duplicate JSON keys, invalid studies, cycles, and contradictory totals fail closed", async () => {
  const cases = [
    ['{"totalCount":1,"totalCount":1,"studies":[]}'],
    ['{"totalCount":0,"studies":[],"ignored":1e400}'],
    [response([study("NCT123")], 1)],
    [response([study(), study()], 1)],
    [response([study()], 0)],
    [response([], 0, "same"), response([], 0, "same")],
  ];
  for (const replies of cases) {
    const state = fixture(replies);
    await assert.rejects(state.adapter.execute(state.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), ReviewedClinicalTrialsRetrievalError);
  }
  let nested = "null";
  for (let depth = 0; depth < 66; depth += 1) nested = `[${nested}]`;
  const deepState = fixture([`{\"totalCount\":0,\"studies\":[],\"ignored\":${nested}}`]);
  await assert.rejects(deepState.adapter.execute(deepState.adapter.prepare(), { approveSourceDispatch: true }), /tree bound/);
  const invalid = study();
  invalid.protocolSection.statusModule.lastUpdatePostDateStruct.date = "2025-19-44";
  const dateState = fixture([response([invalid], 1)]);
  await assert.rejects(dateState.adapter.execute(dateState.adapter.prepare(), { approveSourceDispatch: true }), /last update/);
  const sensitive = new ReviewedClinicalTrialsRetrievalAdapter(new ReviewedClinicalTrialsRetrievalConfig(CUSTOM_TRANSPORT), {
    fetch: async () => { throw new Error("sensitive upstream response detail"); },
  });
  await assert.rejects(
    sensitive.execute(sensitive.prepare(), { approveSourceDispatch: true }),
    (error) => error instanceof ReviewedClinicalTrialsRetrievalError && !error.message.includes("sensitive upstream"),
  );
});

test("single-lane evidence registration binds approval and validates the entire transient bundle", async () => {
  const state = fixture([response([study()], 1)]);
  const plan = state.adapter.prepare();
  const registration = createReviewedClinicalTrialsAutonomousEvidenceRegistration(state.adapter, plan, "glioblastoma");
  const registry = new AutonomousEvidenceAdapterRegistry();
  registry.register(registration);
  const metadata = createReviewedClinicalTrialsExecutionMetadata(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  const context = {
    plan_digest: "b".repeat(64),
    requirement: { requirement_id: "req-1", domain: "biomedical", label: "trial_registry" },
    request: { requirement_id: "req-1", source_id: "clinicaltrials_glioblastoma", source_digest: plan.plan_digest, metadata },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };
  const value = await registration.acquire(context);
  const observations = await registration.project(value, context);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, value.receipt.bundle_digest);
  assert.equal(observations[0].source_digest, value.receipt.source_set_digest);
  const tampered = structuredClone(value);
  tampered.bundle.trials[0].title = "tampered";
  await assert.rejects(registration.project(tampered, context), /digest|source content/);
  assert.throws(
    () => createReviewedClinicalTrialsAutonomousEvidenceRegistration(
      fixture([], { lanes: ["glioblastoma", "glioma"] }).adapter,
      new ReviewedClinicalTrialsRetrievalAdapter(new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["glioblastoma", "glioma"], ...CUSTOM_TRANSPORT }), { fetch: () => ({}) }).prepare(),
      "glioblastoma",
    ),
    /single reviewed lane/,
  );
});

test("shared fixture config and plan digests use canonical JSON", () => {
  const config = new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: ["glioblastoma"], pageSize: 1, maxPages: 2, ...CUSTOM_TRANSPORT });
  const plan = new ReviewedClinicalTrialsRetrievalAdapter(config, { fetch: () => ({}) }).prepare();
  assert.equal(config.config_digest, digestJsonSync(config.toJSON()));
  const unsigned = plan.toJSON();
  delete unsigned.plan_digest;
  assert.equal(plan.plan_digest, digestJsonSync(unsigned));
  assert.equal(config.config_digest, "459bd696c0262033b456db584620ac2760b3ca3f499e0239d4838dbf32a683d8");
  assert.equal(plan.query_set_digest, "6bf9c9f2be2d25041b1aada907c99bab86cef2158524bc8d0ce9d249d085289b");
  assert.equal(plan.plan_digest, "27c7b38a6b8ffca018bbd29dd7c6b39ba63e6ebbf158f7df9387808c55561bff");
});

test("shared source, bundle, and receipt fixture digests match the Python contract", async () => {
  const state = fixture([response([study("NCT01234567", "Reviewed\tregistry\u00a0study")], 1)], { pageSize: 1, maxPages: 2 });
  const plan = state.adapter.prepare();
  const result = await state.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.bundle.trials[0].title, "Reviewed registry study");
  assert.equal(result.bundle.sources[0].content_sha256, "652627652cb3163c302eb670603cc622b0358d814eb1ad0ca7e019d64b00bc58");
  assert.equal(result.bundle.bundle_digest, "74d26b1b71989d24e04056ab20fb1a9ba7e9d72c1083dd2d4638ece368016b8e");
  assert.equal(result.receipt.receipt_digest, "164344a7c80bc3d75c8c0741ba6b5fa61ebf8313622b858860f410ba4b1ece63");
});
