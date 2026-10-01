import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_GDC_TRANSPORT_ID,
  REVIEWED_GDC_PROJECTS,
  AutonomousEvidenceAdapterRegistry,
  ReviewedGdcRetrievalAdapter,
  ReviewedGdcRetrievalConfig,
  ReviewedGdcRetrievalError,
  ReviewedGdcRetrievalPlan,
  createReviewedGdcAutonomousEvidenceRegistration,
  createReviewedGdcExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const TRANSPORT = {
  transportId: "fixture.nci-gdc",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";

function project(projectId = "TCGA-GBM", { missingCounts = false } = {}) {
  const summary = {
    data_categories: [
      { data_category: "Transcriptome Profiling", case_count: 610, file_count: 1200 },
      { data_category: "Clinical", case_count: 615, file_count: 5 },
    ],
  };
  if (!missingCounts) Object.assign(summary, { case_count: 615, file_count: 2405 });
  return {
    data: {
      project_id: projectId,
      name: `${projectId} aggregate metadata fixture`,
      disease_type: ["Gliomas", "Central Nervous System Tumors"],
      primary_site: ["Brain"],
      state: "open",
      released: true,
      summary,
    },
    warnings: {},
  };
}

function fixtureAdapter(responses = [project()], options = {}) {
  const config = new ReviewedGdcRetrievalConfig({ projectIds: options.projectIds ?? ["TCGA-GBM"], ...TRANSPORT });
  const calls = [];
  const adapter = new ReviewedGdcRetrievalAdapter(config, { fetch: (url) => { calls.push(url); return JSON.stringify(responses[calls.length - 1]); } });
  return { config, adapter, calls };
}

test("reviewed GDC config, plan, and normalized receipt digests match Python", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  assert.equal(Object.isFrozen(REVIEWED_GDC_PROJECTS), true);
  assert.throws(() => { REVIEWED_GDC_PROJECTS.other = "TCGA-OTHER"; }, TypeError);
  assert.equal(fixture.config.configDigest, "a03ec2cd569e56c7e710db40d5566d5102ba552f5a76468831bbc1e59e6c6f33");
  assert.equal(plan.planDigest, "3da893a4df5e7bb7a076e1228f3447e95d48b31edbb8d6d07aff6b04afff1c91");
  assert.deepEqual(ReviewedGdcRetrievalConfig.fromJSON(fixture.config.toJSON()).toJSON(), fixture.config.toJSON());
  assert.deepEqual(ReviewedGdcRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.bundle_digest, "b74bb9523cc445efc01d603da00ef4b8c83e8d3cdf4019ee9bc87f9d5e3c8d81");
  assert.equal(result.receipt.receipt_digest, "1ece144e9d9fcb8c0144480df1cf86d183d100694cbf2dccbccd884a2b4ebf7b");
  assert.equal(result.receipt.response_bytes, 415);
  const detachedReceipt = result.receipt; detachedReceipt.bundle_digest = "tampered";
  assert.equal(result.receipt.bundle_digest, "b74bb9523cc445efc01d603da00ef4b8c83e8d3cdf4019ee9bc87f9d5e3c8d81");
});

test("dispatch needs literal approval and sends only the fixed aggregate project query", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls.length, 0);
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls.length, 1);
  const url = new URL(fixture.calls[0]);
  assert.equal(`${url.protocol}//${url.host}`, "https://api.gdc.cancer.gov");
  assert.equal(url.pathname, "/projects/TCGA-GBM");
  assert.equal(url.searchParams.get("format"), "json");
  assert.equal(url.searchParams.get("expand"), "summary,summary.data_categories");
  assert.equal(url.searchParams.get("fields"), "project_id,name,disease_type,primary_site,state,released,summary.case_count,summary.file_count,summary.data_categories");
  const normalized = result.toTransientJSON().bundle.projects[0];
  assert.equal(normalized.case_count, 615);
  assert.equal(normalized.file_count, 2405);
  assert.deepEqual(normalized.disease_types, ["Central Nervous System Tumors", "Gliomas"]);
  assert.deepEqual(normalized.data_categories.map((row) => row.data_category), ["Clinical", "Transcriptome Profiling"]);
  const serialized = JSON.stringify(result.toTransientJSON().bundle).toLowerCase();
  assert.equal(serialized.includes("case_id"), false);
  assert.equal(serialized.includes("sample_id"), false);
  assert.equal(serialized.includes("file_id"), false);
});

test("two-project plans are bounded and missing aggregate counts remain unknown", async () => {
  const fixture = fixtureAdapter([project("TCGA-GBM", { missingCounts: true }), project("TCGA-LGG")], { projectIds: ["TCGA-LGG", "TCGA-GBM"] });
  assert.deepEqual(fixture.config.projectIds, ["TCGA-GBM", "TCGA-LGG"]);
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(fixture.calls.map((raw) => new URL(raw).pathname), ["/projects/TCGA-GBM", "/projects/TCGA-LGG"]);
  assert.equal(result.toTransientJSON().receipt.completeness, "unknown");
  assert.equal(result.toTransientJSON().bundle.projects[0].case_count, null);
  assert.equal(result.toTransientJSON().bundle.projects[0].file_count, null);
});

test("GDC duplicate fields, foreign project identity, malformed counts, and unreviewed transports fail closed", async () => {
  const config = new ReviewedGdcRetrievalConfig(TRANSPORT);
  const duplicate = new ReviewedGdcRetrievalAdapter(config, { fetch: () => '{"data":{"project_id":"TCGA-GBM","project_id":"TCGA-LGG"}}' });
  await assert.rejects(duplicate.execute(duplicate.prepare(), { approveSourceDispatch: true }), /duplicate JSON fields/);
  const foreign = new ReviewedGdcRetrievalAdapter(config, { fetch: () => JSON.stringify(project("TCGA-LGG")) });
  await assert.rejects(foreign.execute(foreign.prepare(), { approveSourceDispatch: true }), /identity does not match/);
  const malformed = project(); malformed.data.summary.case_count = true;
  const badCount = new ReviewedGdcRetrievalAdapter(config, { fetch: () => JSON.stringify(malformed) });
  await assert.rejects(badCount.execute(badCount.prepare(), { approveSourceDispatch: true }), /integer range/);
  assert.equal(new ReviewedGdcRetrievalConfig().transportId, BUILTIN_GDC_TRANSPORT_ID);
  assert.match(BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST, /^[0-9a-f]{64}$/);
  assert.throws(() => new ReviewedGdcRetrievalAdapter(new ReviewedGdcRetrievalConfig(), { fetch: () => "{}" }), /distinct reviewed identity/);
  assert.throws(() => new ReviewedGdcRetrievalConfig({ projectIds: ["TCGA-OTHER"], ...TRANSPORT }), /unsupported or duplicate/);
  const invalidUnicode = new ReviewedGdcRetrievalAdapter(config, { fetch: () => JSON.stringify({ data: { ...project().data, bad: "\ud800" } }) });
  await assert.rejects(invalidUnicode.execute(invalidUnicode.prepare(), { approveSourceDispatch: true }), /invalid Unicode/);
});

test("evidence registration requires the reviewed plan and retains digest-only observations", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const registration = createReviewedGdcAutonomousEvidenceRegistration(fixture.adapter, plan, "TCGA-GBM");
  const manifest = new AutonomousEvidenceAdapterRegistry().register(registration);
  assert.equal(manifest.adapter_id, "reviewed.gdc.gbm");
  const metadata = createReviewedGdcExecutionMetadata(plan, true, RETRIEVED_AT);
  const context = { request: { source_id: "gdc_gbm", source_digest: plan.planDigest, metadata }, requirement: { label: "GDC cohort inventory" } };
  const transient = await registration.acquire(context);
  const observations = await registration.project(transient, context);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, transient.receipt.bundle_digest);
  assert.equal(JSON.stringify(observations).includes("aggregate metadata fixture"), false);
  const forgedMetadata = { ...metadata, approve_source_dispatch: false };
  forgedMetadata.metadata_digest = digestJsonSync(Object.fromEntries(Object.entries(forgedMetadata).filter(([key]) => key !== "metadata_digest")));
  await assert.rejects(registration.acquire({ request: { ...context.request, metadata: forgedMetadata } }), /failed review binding/);
  const tampered = JSON.parse(JSON.stringify(transient));
  tampered.bundle.projects[0].case_count = 0;
  const bundleUnsigned = Object.fromEntries(Object.entries(tampered.bundle).filter(([key]) => key !== "bundle_digest"));
  tampered.bundle.bundle_digest = digestJsonSync(bundleUnsigned);
  tampered.receipt.bundle_digest = tampered.bundle.bundle_digest;
  const receiptUnsigned = Object.fromEntries(Object.entries(tampered.receipt).filter(([key]) => key !== "receipt_digest"));
  tampered.receipt.receipt_digest = digestJsonSync(receiptUnsigned);
  await assert.rejects(registration.project(tampered, context), /source metadata/);
});
