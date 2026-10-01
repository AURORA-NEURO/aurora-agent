import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_CBIOPORTAL_TRANSPORT_ID,
  REVIEWED_CBIOPORTAL_STUDIES,
  AutonomousEvidenceAdapterRegistry,
  ReviewedCBioPortalRetrievalAdapter,
  ReviewedCBioPortalRetrievalConfig,
  ReviewedCBioPortalRetrievalPlan,
  createReviewedCBioPortalAutonomousEvidenceRegistration,
  createReviewedCBioPortalExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const TRANSPORT = { transportId: "fixture.cbioportal", transportVersion: "1", transportConfigDigest: "a".repeat(64) };
const RETRIEVED_AT = "2025-01-02T03:04:05Z";

function study(studyId = "gbm_tcga", sampleCount = 619) {
  return {
    studyId, cancerTypeId: studyId === "gbm_tcga" ? "gbm" : "lgg", name: `${studyId} public fixture`,
    description: "Aggregate catalogue metadata only.", publicStudy: true, pmid: "22824167",
    allSampleCount: sampleCount, referenceGenome: "hg19", importDate: "2023-01-02", status: 0,
    clinicalData: [{ unrequested: "must not be retained" }],
  };
}
function profiles(studyId = "gbm_tcga") {
  return [
    { molecularProfileId: `${studyId}_mutations`, studyId, molecularAlterationType: "MUTATION_EXTENDED", genericAssayType: null, datatype: "MAF", name: "Mutations", description: "Mutation profile catalogue entry.", showProfileInAnalysisTab: true, patientLevel: false, pivotThreshold: 0.5, molecularData: [{ unrequested: "must not be retained" }] },
    { molecularProfileId: `${studyId}_mrna`, studyId, molecularAlterationType: "MRNA_EXPRESSION", genericAssayType: null, datatype: "CONTINUOUS", name: "mRNA expression", description: null, showProfileInAnalysisTab: true, patientLevel: false },
  ];
}
function adapter(responses = [study(), profiles()], studyIds = ["gbm_tcga"]) {
  const calls = [];
  const config = new ReviewedCBioPortalRetrievalConfig({ studyIds, ...TRANSPORT });
  const instance = new ReviewedCBioPortalRetrievalAdapter(config, { fetch: (url) => { calls.push(url); return responses[calls.length - 1]; } });
  return { config, adapter: instance, calls };
}

test("cBioPortal config, plan, source catalogue and normalized digests match Python", async () => {
  const fixture = adapter();
  const plan = fixture.adapter.prepare();
  assert.equal(Object.isFrozen(REVIEWED_CBIOPORTAL_STUDIES), true);
  assert.throws(() => { REVIEWED_CBIOPORTAL_STUDIES.other = "other_tcga"; }, TypeError);
  assert.equal(fixture.config.requestLimit, 2);
  assert.deepEqual(ReviewedCBioPortalRetrievalConfig.fromJSON(fixture.config.toJSON()).toJSON(), fixture.config.toJSON());
  assert.deepEqual(ReviewedCBioPortalRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.bundle_digest, "9e1c17fc1df33e21f164d5c2428eb94a05c4b222762563fb649d41eb45db9a1e");
  assert.equal(result.receipt.receipt_digest, "94df44e1220da21603f06365837f9da146f637bc0b75ca554b4d78b11d81a769");
  assert.equal(result.receipt.response_bytes, 905);
  assert.equal(fixture.config.transportId, "fixture.cbioportal");
  assert.equal(BUILTIN_CBIOPORTAL_TRANSPORT_ID, "builtin.nci-cbioportal.fetch");
  assert.match(BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST, /^[0-9a-f]{64}$/);
});

test("literal approval dispatches only fixed study and SUMMARY profile metadata routes", async () => {
  const fixture = adapter();
  const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls.length, 0);
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls.length, 2);
  const studyUrl = new URL(fixture.calls[0]); const profileUrl = new URL(fixture.calls[1]);
  assert.equal(studyUrl.origin, "https://www.cbioportal.org");
  assert.equal(studyUrl.pathname, "/api/studies/gbm_tcga");
  assert.equal(profileUrl.pathname, "/api/studies/gbm_tcga/molecular-profiles");
  assert.deepEqual(Object.fromEntries(profileUrl.searchParams), { projection: "SUMMARY", pageSize: "128", pageNumber: "0", sortBy: "molecularProfileId", direction: "ASC" });
  const bundle = result.toTransientJSON().bundle;
  assert.equal(bundle.studies[0].sample_count, 619);
  assert.deepEqual(bundle.molecular_profiles.map((profile) => profile.profile_id), ["gbm_tcga_mrna", "gbm_tcga_mutations"]);
  assert.equal(result.receipt.request_count, 2);
  const serialized = JSON.stringify(bundle).toLowerCase();
  assert.equal(serialized.includes("sample_id"), false);
  assert.equal(serialized.includes("patient_id"), false);
  assert.equal(serialized.includes("pivotthreshold"), false);
  assert.equal(serialized.includes("clinicaldata"), false);
  assert.equal(serialized.includes("moleculardata"), false);
});

test("two-study selection is canonical and missing aggregate counts stay unknown", async () => {
  const fixture = adapter([study("gbm_tcga", null), profiles(), study("lgg_tcga", 530), profiles("lgg_tcga")], ["lgg_tcga", "gbm_tcga"]);
  assert.deepEqual(fixture.config.studyIds, ["gbm_tcga", "lgg_tcga"]);
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(fixture.calls.map((raw) => new URL(raw).pathname), [
    "/api/studies/gbm_tcga", "/api/studies/gbm_tcga/molecular-profiles", "/api/studies/lgg_tcga", "/api/studies/lgg_tcga/molecular-profiles",
  ]);
  assert.equal(result.receipt.request_count, 4);
  assert.equal(result.receipt.completeness, "unknown");
  assert.equal(result.toTransientJSON().bundle.studies[0].sample_count, null);
});

test("private, foreign, unsafe and possibly truncated catalogues fail closed", async () => {
  const privateStudy = study(); privateStudy.publicStudy = false;
  let fixture = adapter([privateStudy, profiles()]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /public status/);
  assert.equal(fixture.calls.length, 1);
  fixture = adapter([study("lgg_tcga"), profiles()]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /identity or public status/);
  assert.equal(fixture.calls.length, 1);
  const foreignProfile = profiles(); foreignProfile[0].studyId = "lgg_tcga";
  fixture = adapter([study(), foreignProfile]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /mismatched study identity/);
  assert.equal(fixture.calls.length, 2);
  const duplicates = profiles(); duplicates[1].molecularProfileId = duplicates[0].molecularProfileId;
  fixture = adapter([study(), duplicates]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /duplicate IDs/);
  const unsupported = profiles(); unsupported[0].molecularAlterationType = "UNREVIEWED_TYPE";
  fixture = adapter([study(), unsupported]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /unsupported alteration type/);
  const invalidId = profiles(); invalidId[0].molecularProfileId = "gbm_tcga_😀";
  fixture = adapter([study(), invalidId]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /identifier bound/);
  fixture = adapter(['{"studyId":"gbm_tcga","studyId":"gbm_tcga","name":"duplicate field"}', profiles()]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /duplicate JSON fields/);
  fixture = adapter([study(), Array.from({ length: 128 }, (_, index) => ({ ...profiles()[0], molecularProfileId: `gbm_tcga_${index}` }))]);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /catalogue bound/);
  assert.throws(() => new ReviewedCBioPortalRetrievalConfig({ studyIds: ["unknown_tcga"], ...TRANSPORT }), /unsupported or duplicate/);
});

test("autonomous evidence registration validates the transient and exposes digests only", async () => {
  const fixture = adapter(); const plan = fixture.adapter.prepare();
  const registration = createReviewedCBioPortalAutonomousEvidenceRegistration(fixture.adapter, plan, "gbm_tcga");
  assert.equal(new AutonomousEvidenceAdapterRegistry().register(registration).adapter_id, "reviewed.cbioportal.gbm");
  const metadata = createReviewedCBioPortalExecutionMetadata(plan, true, RETRIEVED_AT);
  const context = { request: { source_id: "cbioportal_gbm", source_digest: plan.planDigest, metadata }, requirement: { label: "cBioPortal catalogue" } };
  const transient = await registration.acquire(context);
  const observations = await registration.project(transient, context);
  assert.equal(observations[0].value_digest, transient.receipt.bundle_digest);
  assert.equal(JSON.stringify(observations).includes("gbm_tcga public fixture"), false);
  const tampered = JSON.parse(JSON.stringify(transient));
  tampered.bundle.molecular_profiles[0].datatype = "CHANGED";
  tampered.bundle.bundle_digest = digestJsonSync(Object.fromEntries(Object.entries(tampered.bundle).filter(([key]) => key !== "bundle_digest")));
  tampered.receipt.bundle_digest = tampered.bundle.bundle_digest;
  tampered.receipt.receipt_digest = digestJsonSync(Object.fromEntries(Object.entries(tampered.receipt).filter(([key]) => key !== "receipt_digest")));
  await assert.rejects(registration.project(tampered, context), /profiles|source metadata/);
});
