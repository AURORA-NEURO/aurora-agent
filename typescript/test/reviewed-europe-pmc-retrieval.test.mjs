import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_EUROPE_PMC_TRANSPORT_ID,
  MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES,
  REVIEWED_EUROPE_PMC_ENDPOINT,
  REVIEWED_EUROPE_PMC_LANES,
  AutonomousEvidenceAdapterRegistry,
  ReviewedEuropePmcRetrievalAdapter,
  ReviewedEuropePmcRetrievalConfig,
  ReviewedEuropePmcRetrievalError,
  ReviewedEuropePmcRetrievalPlan,
  createReviewedEuropePmcAutonomousEvidenceRegistration,
  createReviewedEuropePmcExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const CUSTOM_TRANSPORT = {
  transportId: "fixture.europepmc",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";

function publication(id = "24999217", overrides = {}) {
  return {
    id, source: "MED", pmid: id, doi: "10.1016/j.aat.2014.02.001",
    title: "Europe PMC metadata fixture", authorString: "Ghorbani J, Dabir S, Givehchi G, Najafi M.",
    journalTitle: "Acta Anaesthesiol Taiwan", pubYear: "2014",
    pubType: "review; journal article; case reports", isOpenAccess: "N", citedByCount: 11,
    firstPublicationDate: "2014-03-01", ...overrides,
  };
}

function response(records = [publication()], hitCount = records.length, nextCursorMark) {
  const payload = { version: "6.9", hitCount, request: { queryString: "fixed", resultType: "lite" }, resultList: { result: records } };
  if (nextCursorMark !== undefined) payload.nextCursorMark = nextCursorMark;
  return JSON.stringify(payload);
}

function fixtureAdapter(responses = [response()], options = {}) {
  const config = new ReviewedEuropePmcRetrievalConfig({
    lanes: options.lanes ?? ["glioma"], pageSize: options.pageSize ?? 2, maxPages: options.maxPages ?? 2,
    ...CUSTOM_TRANSPORT,
  });
  const calls = [];
  const adapter = new ReviewedEuropePmcRetrievalAdapter(config, { fetch: (url) => { calls.push(url); return responses[calls.length - 1]; } });
  return { config, adapter, calls };
}

test("reviewed config, plan, and deterministic result digests match the Python SDK", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  assert.equal(fixture.config.configDigest, "d472bdf5f9e081434b56979899a4d23461ef00b366540be96efc58dcb65ad65e");
  assert.equal(plan.planDigest, "21ec7602916419e8a15b698ee9fb71c0878b15f7759b7240ea3feb0810cf9b38");
  assert.deepEqual(ReviewedEuropePmcRetrievalConfig.fromJSON(fixture.config.toJSON()).toJSON(), fixture.config.toJSON());
  assert.deepEqual(ReviewedEuropePmcRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.bundle_digest, "158f206bef32c5e8d9f96058f9be17b647121a7644d33e66ff2106a896818ab6");
  assert.equal(result.receipt.receipt_digest, "bb5c597e848a37c70cfab9e8ffe13d82c84c3cc72a4749c8ef27e7086d53308c");
  assert.equal(result.receipt.response_bytes, 475);
});

test("fixed-lane preflight and transport identity are immutable and bounded", () => {
  assert.equal(Object.isFrozen(REVIEWED_EUROPE_PMC_LANES), true);
  assert.throws(() => { REVIEWED_EUROPE_PMC_LANES.glioma = "caller query"; }, TypeError);
  assert.throws(() => new ReviewedEuropePmcRetrievalConfig({ lanes: ["cardiology"], ...CUSTOM_TRANSPORT }), /unsupported or duplicate/);
  assert.throws(() => new ReviewedEuropePmcRetrievalConfig({ lanes: ["glioma", "glioma"], ...CUSTOM_TRANSPORT }), /unsupported or duplicate/);
  const builtin = new ReviewedEuropePmcRetrievalConfig();
  assert.equal(builtin.transportId, BUILTIN_EUROPE_PMC_TRANSPORT_ID);
  assert.equal(builtin.transportConfigDigest, BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST);
  assert.throws(() => new ReviewedEuropePmcRetrievalAdapter(builtin, { fetch: () => "{}" }), /distinct reviewed identity/);
  assert.throws(() => new ReviewedEuropePmcRetrievalAdapter(new ReviewedEuropePmcRetrievalConfig(CUSTOM_TRANSPORT)), /identity is not exact/);
});

test("dispatch requires literal approval and pages only the pinned lite endpoint", async () => {
  const cursor = "cursor/+==";
  const fixture = fixtureAdapter([
    response([publication()], 3, cursor),
    response([publication("PMC4054321", { pmid: null, pmcid: "PMC4054321", doi: null, title: "Second result" })], 3),
  ]);
  const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls.length, 0);
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls.length, 2);
  for (const [index, rawUrl] of fixture.calls.entries()) {
    const url = new URL(rawUrl);
    assert.equal(`${url.protocol}//${url.host}${url.pathname}`, REVIEWED_EUROPE_PMC_ENDPOINT);
    assert.equal(url.searchParams.get("format"), "json");
    assert.equal(url.searchParams.get("resultType"), "lite");
    assert.equal(url.searchParams.get("synonym"), "N");
    assert.equal(url.searchParams.get("pageSize"), "2");
    assert.equal(url.searchParams.get("query"), REVIEWED_EUROPE_PMC_LANES.glioma);
    assert.equal(url.searchParams.get("cursorMark"), index === 0 ? "*" : cursor);
  }
  assert.equal(result.receipt.completeness, "partial");
  assert.equal(result.receipt.reported_hit_count, 3);
  assert.equal(result.receipt.omitted_record_count, 1);
  assert.doesNotMatch(JSON.stringify(result), /Europe PMC metadata fixture|glioblastoma OR|nextCursorMark/);
});

test("unstable source totals remain unknown and malformed source data fails closed", async () => {
  const unstable = fixtureAdapter([response([publication()], 4, "next"), response([publication("PMC4", { pmid: null, pmcid: "PMC4", doi: null })], 5)]);
  const result = await unstable.adapter.execute(unstable.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.completeness, "unknown");
  assert.equal(result.receipt.reported_hit_count, null);
  assert.equal(result.receipt.source_receipts[0].omitted_record_count, null);

  const malformed = [
    '{"hitCount":0,"hitCount":0,"resultList":{"result":[]}}',
    '{"hitCount":0,"resultList":{"result":[]},"extra":1e400}',
    response([publication("invalid", { pmid: "invalid", doi: null })], 1),
    response([publication("24999217"), publication("24999217")], 2),
  ];
  for (const raw of malformed) {
    const fixture = fixtureAdapter([raw]);
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), ReviewedEuropePmcRetrievalError);
  }
  const repeated = fixtureAdapter([response([], 3, "a"), response([], 3, "b"), response([], 3, "a")], { maxPages: 4 });
  await assert.rejects(repeated.adapter.execute(repeated.adapter.prepare(), { approveSourceDispatch: true }), /cursor repeated/);
});

test("Unicode normalization matches Python and unpaired surrogates fail closed", async () => {
  const fixture = fixtureAdapter([response([publication("1", { pmid: "1", doi: null, title: " \u001c\u0085Europe\u2003PMC  " })], 1)]);
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.bundle.publications[0].title, "Europe PMC");
  const invalid = fixtureAdapter(['{"hitCount":1,"resultList":{"result":[{"id":"1","source":"MED","pmid":"1","title":"\\ud800"}]}}']);
  await assert.rejects(invalid.adapter.execute(invalid.adapter.prepare(), { approveSourceDispatch: true }), /Unicode/);
});

test("response bounds, evidence registration, and transient projection are enforced", async () => {
  const oversized = fixtureAdapter([" ".repeat(MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES + 1)]);
  await assert.rejects(oversized.adapter.execute(oversized.adapter.prepare(), { approveSourceDispatch: true }), /byte bound/);

  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const registration = createReviewedEuropePmcAutonomousEvidenceRegistration(fixture.adapter, plan, "glioma");
  const manifest = new AutonomousEvidenceAdapterRegistry().register(registration);
  assert.deepEqual(manifest.domains, ["biomedical", "neuroscience"]);
  const metadata = createReviewedEuropePmcExecutionMetadata(plan, true, RETRIEVED_AT);
  const context = {
    plan_digest: "b".repeat(64),
    requirement: { requirement_id: "req-1", domain: "biomedical", label: "Europe PMC provenance" },
    request: { requirement_id: "req-1", source_id: "europepmc_glioma", source_digest: plan.planDigest, metadata },
    attempt: 1, parent_evidence_digests: [], execution: "caller_owned_adapter;raw_value_transient",
  };
  const transient = await registration.acquire(context);
  const observations = await registration.project(transient, context);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, transient.receipt.bundle_digest);
  assert.doesNotMatch(JSON.stringify(observations), /Europe PMC metadata fixture/);
  const forged = structuredClone(transient);
  forged.bundle.publications[0].title = "fabricated";
  const unsignedBundle = { ...forged.bundle };
  delete unsignedBundle.bundle_digest;
  forged.bundle.bundle_digest = digestJsonSync(unsignedBundle);
  forged.receipt.bundle_digest = forged.bundle.bundle_digest;
  const unsignedReceipt = { ...forged.receipt };
  delete unsignedReceipt.receipt_digest;
  forged.receipt.receipt_digest = digestJsonSync(unsignedReceipt);
  await assert.rejects(registration.project(forged, context), /source metadata|publication bundle/);
  assert.throws(() => createReviewedEuropePmcAutonomousEvidenceRegistration(fixture.adapter, plan, "chiari_malformation"), /single-lane/);
});

test("injected transports cannot replace the captured byte, URL, or timeout primitives", async () => {
  const originalTextDecoder = globalThis.TextDecoder;
  const originalURL = globalThis.URL;
  const originalEncode = globalThis.encodeURIComponent;
  const originalClearTimeout = globalThis.clearTimeout;
  const config = new ReviewedEuropePmcRetrievalConfig({ lanes: ["glioma"], ...CUSTOM_TRANSPORT });
  const adapter = new ReviewedEuropePmcRetrievalAdapter(config, {
    fetch: async () => {
      globalThis.TextDecoder = class { decode() { throw new Error("replaced decoder called"); } };
      globalThis.URL = class { constructor() { throw new Error("replaced URL called"); } };
      globalThis.encodeURIComponent = () => "&redirect=https://evil.example";
      globalThis.clearTimeout = () => { throw new Error("replaced timer called"); };
      return new TextEncoder().encode(response());
    },
  });
  try {
    const result = await adapter.execute(adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    assert.equal(result.receipt.record_count, 1);
    assert.equal(result.receipt.network, "caller_transport");
  } finally {
    globalThis.TextDecoder = originalTextDecoder;
    globalThis.URL = originalURL;
    globalThis.encodeURIComponent = originalEncode;
    globalThis.clearTimeout = originalClearTimeout;
  }
});
