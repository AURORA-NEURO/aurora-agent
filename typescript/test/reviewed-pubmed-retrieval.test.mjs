import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_PUBMED_TRANSPORT_ID,
  MAX_REVIEWED_PUBMED_ABSTRACT_BYTES,
  PUBMED_SPECIALTY_LANES,
  REVIEWED_PUBMED_ENDPOINTS,
  REVIEWED_PUBMED_HOST,
  REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA,
  AutonomousEvidenceAdapterRegistry,
  ReviewedPubMedRetrievalAdapter,
  ReviewedPubMedRetrievalConfig,
  ReviewedPubMedRetrievalError,
  ReviewedPubMedRetrievalPlan,
  ReviewedPubMedRetrievalReceipt,
  createReviewedPubMedAutonomousEvidenceRegistration,
  createReviewedPubMedExecutionMetadata,
  digestJsonSync,
  reviewedPubMedBundleDigest,
} from "../dist/index.js";

const CUSTOM_TRANSPORT = {
  transportId: "fixture.ncbi_eutils",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";
const STANDARD_DTD = '<!DOCTYPE PubmedArticleSet PUBLIC "-//NLM//DTD PubMedArticle, 1st January 2025//EN" "https://dtd.nlm.nih.gov/ncbi/pubmed/out/pubmed_250101.dtd">';

function xmlFor(id = "123", options = {}) {
  const declaration = options.declaration ?? STANDARD_DTD;
  const abstract = options.abstract ?? '<AbstractText Label="RESULTS">Useful <i>bounded</i> abstract.</AbstractText>';
  return `<?xml version="1.0" encoding="utf-8"?>${declaration}<PubmedArticleSet><PubmedArticle><MedlineCitation><PMID>${id}</PMID><Article><Abstract>${abstract}</Abstract><PublicationTypeList><PublicationType>Journal Article</PublicationType></PublicationTypeList></Article><MeshHeadingList><MeshHeading><DescriptorName>Glioma</DescriptorName></MeshHeading></MeshHeadingList></MedlineCitation></PubmedArticle></PubmedArticleSet>`;
}

function fixtureResponses(id = "123", overrides = {}) {
  return [
    overrides.search ?? JSON.stringify({ esearchresult: { idlist: [id] } }),
    overrides.summary ?? JSON.stringify({ result: { [id]: { title: "Reviewed study", fulljournalname: "Journal", pubdate: "2025 Jan 02", articleids: [{ idtype: "doi", value: "10.1000/example" }] } } }),
    overrides.xml ?? xmlFor(id),
  ];
}

function fixtureAdapter(options = {}) {
  const config = new ReviewedPubMedRetrievalConfig({ specialtyLanes: options.lanes ?? ["glioma"], ...CUSTOM_TRANSPORT, ...(options.config ?? {}) });
  const responses = options.responses ?? fixtureResponses();
  const urls = [];
  let calls = 0;
  const fetch = async (url) => {
    urls.push(url);
    const index = calls++;
    if (options.onFetch) await options.onFetch({ index, url, config });
    if (options.throwAt === index) throw new Error("sensitive upstream detail");
    return responses[index];
  };
  return { config, adapter: new ReviewedPubMedRetrievalAdapter(config, { fetch }), urls, calls: () => calls };
}

test("prepare is pure and emits a deterministic metadata-only reviewed plan", () => {
  let calls = 0;
  const config = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["chiari_malformation", "glioma"], ...CUSTOM_TRANSPORT });
  const adapter = new ReviewedPubMedRetrievalAdapter(config, { fetch: () => { calls += 1; return ""; } });
  const first = adapter.prepare();
  const second = adapter.prepare();
  assert.equal(calls, 0);
  assert.deepEqual(first.toJSON(), second.toJSON());
  assert.deepEqual(first.specialty_lanes, ["glioma", "chiari_malformation"]);
  assert.equal(first.request_limit, 6);
  assert.equal(first.record_limit, 20);
  assert.equal(first.status, "ready_for_review");
  assert.doesNotMatch(JSON.stringify(first), /glioblastoma|cine MRI/);
});

test("config accepts only fixed unique lanes and bounded values", () => {
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: [], ...CUSTOM_TRANSPORT }), /1\.\.6 lanes/);
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma", "glioma"], ...CUSTOM_TRANSPORT }), /duplicate/);
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["cardiology"], ...CUSTOM_TRANSPORT }), /non-allow-listed/);
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], perSpecialtyLimit: 51, ...CUSTOM_TRANSPORT }), /between 1 and 50/);
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], apiKey: "forbidden", ...CUSTOM_TRANSPORT }), /unsupported or credential-shaped/);
});

test("NCBI tool and email are paired, digest-bound, and absent from artifacts", () => {
  assert.throws(() => new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], ncbiTool: "aurora", ...CUSTOM_TRANSPORT }), /provided together/);
  const config = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], ncbiTool: "aurora_agent", ncbiEmail: "dev@example.org", ...CUSTOM_TRANSPORT });
  const serialized = JSON.stringify(config);
  assert.equal(config.ncbi_registration_configured, true);
  assert.doesNotMatch(serialized, /aurora_agent|dev@example\.org/);
  assert.throws(() => ReviewedPubMedRetrievalConfig.fromJSON(config.toJSON()), /digest|re-supplied/i);
  assert.throws(() => ReviewedPubMedRetrievalConfig.fromJSON(config.toJSON(), { ncbiTool: "other", ncbiEmail: "dev@example.org" }), /digest/i);
  assert.deepEqual(ReviewedPubMedRetrievalConfig.fromJSON(config.toJSON(), { ncbiTool: "aurora_agent", ncbiEmail: "dev@example.org" }).toJSON(), config.toJSON());
});

test("injected and built-in transport identities cannot be confused", () => {
  const builtin = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"] });
  assert.equal(builtin.transport_id, BUILTIN_PUBMED_TRANSPORT_ID);
  assert.equal(builtin.transport_config_digest, BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST);
  assert.throws(() => new ReviewedPubMedRetrievalAdapter(builtin, { fetch: () => "" }), /distinct reviewed transport identity/);
  const custom = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], ...CUSTOM_TRANSPORT });
  assert.throws(() => new ReviewedPubMedRetrievalAdapter(custom), /exact transport identity/);
});

test("execute requires a literal true approval before any dispatch", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: false }), /literal approval/);
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls(), 0);
});

test("reviewed execution performs exact ESearch, ESummary, EFetch and returns only a metadata receipt durably", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls(), 3);
  assert.deepEqual(fixture.urls.map((url) => new URL(url).pathname.split("/").at(-1)), REVIEWED_PUBMED_ENDPOINTS);
  for (const url of fixture.urls) {
    const parsed = new URL(url);
    assert.equal(parsed.protocol, "https:");
    assert.equal(parsed.host, REVIEWED_PUBMED_HOST);
    assert.equal(parsed.searchParams.has("api_key"), false);
  }
  assert.equal(result.receipt.schema, undefined);
  assert.equal(result.receipt.toJSON().schema, REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA);
  assert.equal(result.receipt.request_count, 3);
  assert.equal(result.receipt.record_count, 1);
  assert.equal(result.receipt.abstract_count, 1);
  assert.equal(result.bundle.records[0].abstract_text, "RESULTS: Useful bounded abstract.");
  assert.equal(JSON.stringify(result).includes("Useful bounded abstract"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(result.receipt.toJSON(), "bundle"), false);
});

test("multi-lane execution preserves canonical lane-major request order and exact record bounds", async () => {
  const fixture = fixtureAdapter({
    lanes: ["chiari_malformation", "glioma"],
    responses: [...fixtureResponses("123"), ...fixtureResponses("456", {
      summary: JSON.stringify({ result: { "456": { title: "Chiari study", source: "Second Journal", epubdate: "2024-12-31", articleids: [] } } }),
      xml: xmlFor("456"),
    })],
  });
  const plan = fixture.adapter.prepare();
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(plan.specialty_lanes, ["glioma", "chiari_malformation"]);
  assert.deepEqual(fixture.urls.map((url) => new URL(url).pathname.split("/").at(-1)), [...REVIEWED_PUBMED_ENDPOINTS, ...REVIEWED_PUBMED_ENDPOINTS]);
  assert.deepEqual(result.bundle.records.map((record) => record.specialty), ["glioma", "chiari_malformation"]);
  assert.equal(result.receipt.request_count, 6);
  assert.equal(result.receipt.record_count, 2);
});

test("registered NCBI identification is present on every dispatch but stripped from the bundle and receipt", async () => {
  const fixture = fixtureAdapter({ config: { ncbiTool: "aurora_agent", ncbiEmail: "dev@example.org" } });
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  for (const url of fixture.urls) {
    const parsed = new URL(url);
    assert.equal(parsed.searchParams.get("tool"), "aurora_agent");
    assert.equal(parsed.searchParams.get("email"), "dev@example.org");
  }
  assert.equal(new URL(result.bundle.sources[0].uri).searchParams.has("tool"), false);
  assert.doesNotMatch(JSON.stringify(result.receipt), /aurora_agent|dev@example\.org/);
});

test("bundle getter returns detached transient copies", async () => {
  const fixture = fixtureAdapter();
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  const first = result.bundle;
  first.records[0].title = "mutated";
  assert.equal(result.bundle.records[0].title, "Reviewed study");
});

test("standard NLM DTD is accepted while entities and alternate DTDs are rejected", async (t) => {
  await t.test("standard", async () => {
    const fixture = fixtureAdapter();
    const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    assert.equal(result.receipt.record_count, 1);
  });
  await t.test("internal entity", async () => {
    const malicious = '<!DOCTYPE PubmedArticleSet [<!ENTITY x "boom">]><PubmedArticleSet></PubmedArticleSet>';
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: malicious }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /forbidden document declaration/);
  });
  await t.test("alternate host", async () => {
    const alternate = xmlFor("123", { declaration: '<!DOCTYPE PubmedArticleSet PUBLIC "-//NLM//DTD PubMedArticle, 1st January 2025//EN" "https://evil.example/pubmed_250101.dtd">' });
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: alternate }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /forbidden document declaration/);
  });
});

test("malformed and over-deep XML are rejected", async (t) => {
  await t.test("mismatched", async () => {
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: "<PubmedArticleSet><PubmedArticle></PubmedArticleSet>" }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /mismatched XML tags/);
  });
  await t.test("deep", async () => {
    const deep = `<PubmedArticleSet>${"<x>".repeat(34)}${"</x>".repeat(34)}</PubmedArticleSet>`;
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: deep }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /deeply nested/);
  });
});

test("long abstracts are truncated on a UTF-8 boundary after full safety validation", async (t) => {
  await t.test("ASCII beyond the general text cap", async () => {
    const text = "a".repeat(17_000);
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: xmlFor("123", { abstract: `<AbstractText>${text}</AbstractText>` }) }) });
    const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    const record = result.bundle.records[0];
    assert.equal(record.abstract_truncated, true);
    assert.equal(new TextEncoder().encode(record.abstract_text).byteLength, MAX_REVIEWED_PUBMED_ABSTRACT_BYTES);
    assert.equal(record.abstract_text, "a".repeat(MAX_REVIEWED_PUBMED_ABSTRACT_BYTES));
  });
  await t.test("multibyte code point crossing the byte ceiling", async () => {
    const prefix = "a".repeat(MAX_REVIEWED_PUBMED_ABSTRACT_BYTES - 1);
    const text = `${prefix}${"😀".repeat(1_000)}`;
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: xmlFor("123", { abstract: `<AbstractText>${text}</AbstractText>` }) }) });
    const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    const record = result.bundle.records[0];
    assert.equal(record.abstract_truncated, true);
    assert.equal(record.abstract_text, prefix);
    assert.equal(new TextEncoder().encode(record.abstract_text).byteLength, MAX_REVIEWED_PUBMED_ABSTRACT_BYTES - 1);
    assert.doesNotMatch(record.abstract_text, /\uFFFD/);
  });
  await t.test("unsafe marker beyond the retained prefix", async () => {
    const text = `${"a".repeat(17_000)} synthetic fixture`;
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { xml: xmlFor("123", { abstract: `<AbstractText>${text}</AbstractText>` }) }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /synthetic marker/);
  });
});

test("raw response byte caps apply before JSON and XML parsing", async (t) => {
  await t.test("JSON", async () => {
    const fixture = fixtureAdapter({ config: { responseByteLimit: 256, totalResponseByteLimit: 768 }, responses: ["x".repeat(257), "{}", "<PubmedArticleSet/>"] });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /byte limit/);
    assert.equal(fixture.calls(), 1);
  });
  await t.test("XML", async () => {
    const oversized = `<PubmedArticleSet>${" ".repeat(1_100)}</PubmedArticleSet>`;
    const fixture = fixtureAdapter({ config: { responseByteLimit: 1_024, totalResponseByteLimit: 3_072 }, responses: fixtureResponses("123", { xml: oversized }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /byte limit/);
  });
  await t.test("total", async () => {
    const summary = JSON.stringify({ result: { "123": { title: "Reviewed study", fulljournalname: "Journal", pubdate: "2025 Jan 02", articleids: [], ignored: "x".repeat(1_500) } } });
    const fixture = fixtureAdapter({ config: { responseByteLimit: 2_048, totalResponseByteLimit: 2_048 }, responses: fixtureResponses("123", { summary }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /byte limit|total response/);
  });
});

test("JSON depth and PMID cardinality are bounded", async (t) => {
  await t.test("depth", async () => {
    let value = { idlist: ["123"] };
    for (let index = 0; index < 34; index += 1) value = { nested: value };
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { search: JSON.stringify({ esearchresult: value }) }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /deeply nested/);
  });
  await t.test("duplicate IDs", async () => {
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { search: JSON.stringify({ esearchresult: { idlist: ["123", "123"] } }) }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /duplicate PMIDs/);
  });
  await t.test("node count", async () => {
    const fixture = fixtureAdapter({ responses: fixtureResponses("123", { search: JSON.stringify({ esearchresult: { idlist: ["123"], noise: Array(100_001).fill(null) } }) }) });
    await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }), /too many nodes|oversized array/);
  });
});

test("reviewed config and plan stay immutable when observed by a transport", async (t) => {
  await t.test("config", async () => {
    const fixture = fixtureAdapter({ onFetch: ({ index, config }) => { if (index === 0) assert.equal(Reflect.set(config, "per_specialty_limit", 2), false); } });
    const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    assert.equal(result.receipt.record_count, 1);
    assert.equal(fixture.config.per_specialty_limit, 10);
  });
  await t.test("plan", async () => {
    let plan;
    const fixture = fixtureAdapter({ onFetch: ({ index }) => { if (index === 0) assert.equal(Reflect.set(plan, "plan_digest", "f".repeat(64)), false); } });
    plan = fixture.adapter.prepare();
    const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    assert.equal(result.receipt.plan_digest, plan.plan_digest);
  });
});

test("transport replacement of ambient byte, JSON, URL, and freeze primitives cannot alter reviewed execution", async () => {
  const originalTextEncoder = globalThis.TextEncoder;
  const originalTextDecoder = globalThis.TextDecoder;
  const originalJsonParse = JSON.parse;
  const originalJsonStringify = JSON.stringify;
  const originalEncodeURIComponent = globalThis.encodeURIComponent;
  const originalFreeze = Object.freeze;
  const originalForEach = URLSearchParams.prototype.forEach;
  let result;
  const fixture = fixtureAdapter({
    onFetch: ({ index }) => {
      if (index !== 0) return;
      globalThis.TextEncoder = class { encode() { return new Uint8Array([0]); } };
      globalThis.TextDecoder = class { decode() { throw new Error("ambient decoder used"); } };
      JSON.parse = () => ({ forged: true });
      JSON.stringify = () => "\"forged\"";
      globalThis.encodeURIComponent = (value) => `${value}&api_key=smuggled`;
      Object.freeze = (value) => value;
      URLSearchParams.prototype.forEach = () => undefined;
    },
  });
  try {
    result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  } finally {
    globalThis.TextEncoder = originalTextEncoder;
    globalThis.TextDecoder = originalTextDecoder;
    JSON.parse = originalJsonParse;
    JSON.stringify = originalJsonStringify;
    globalThis.encodeURIComponent = originalEncodeURIComponent;
    Object.freeze = originalFreeze;
    URLSearchParams.prototype.forEach = originalForEach;
  }
  assert.equal(result.receipt.record_count, 1);
  assert.ok(result.receipt.response_bytes > 100);
  assert.equal(reviewedPubMedBundleDigest(result.bundle), result.receipt.bundle_digest);
  assert.equal(Object.isFrozen(result), true);
  for (const url of fixture.urls) assert.equal(new URL(url).searchParams.has("api_key"), false);
});

test("injected transport is bounded by timeoutMs", async () => {
  const config = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"], timeoutMs: 1_000, ...CUSTOM_TRANSPORT });
  const adapter = new ReviewedPubMedRetrievalAdapter(config, { fetch: () => new Promise(() => undefined) });
  const started = performance.now();
  await assert.rejects(
    adapter.execute(adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }),
    (error) => error instanceof ReviewedPubMedRetrievalError && error.message === "reviewed PubMed request timed out",
  );
  const elapsed = performance.now() - started;
  assert.ok(elapsed >= 900 && elapsed < 3_000, `timeout elapsed ${elapsed}ms`);
});

test("receipt rehydration rejects accessors without invoking them", async () => {
  const fixture = fixtureAdapter();
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  const raw = result.receipt.toJSON();
  let reads = 0;
  Object.defineProperty(raw, "limitations", {
    enumerable: true,
    configurable: true,
    get() {
      reads += 1;
      return ["raw abstract secret"];
    },
  });
  assert.throws(() => ReviewedPubMedRetrievalReceipt.fromJSON(raw), /enumerable data properties/);
  assert.equal(reads, 0);
});

test("reviewed PubMed public artifacts and nested receipt sources are runtime immutable", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  const receiptBefore = result.receipt.toJSON();
  for (const artifact of [fixture.config, fixture.adapter, plan, result, result.receipt, result.receipt.specialty_lanes, result.receipt.sources, result.receipt.sources[0]]) {
    assert.equal(Object.isFrozen(artifact), true);
  }
  assert.throws(() => { result.receipt.bundle_digest = "f".repeat(64); }, TypeError);
  assert.throws(() => { result.receipt.sources[0].content_digest = "f".repeat(64); }, TypeError);
  assert.throws(() => { result.receipt = null; }, TypeError);
  assert.deepEqual(result.receipt.toJSON(), receiptBefore);
});

test("transport failures are not retried and raw upstream details are not exposed", async () => {
  const fixture = fixtureAdapter({ throwAt: 0 });
  await assert.rejects(
    fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }),
    (error) => error instanceof ReviewedPubMedRetrievalError && error.message === "reviewed PubMed request failed" && !error.message.includes("sensitive"),
  );
  assert.equal(fixture.calls(), 1);
});

test("receipt and plan rehydration reject restamped tampering", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  assert.deepEqual(ReviewedPubMedRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(ReviewedPubMedRetrievalReceipt.fromJSON(result.receipt.toJSON()).toJSON(), result.receipt.toJSON());
  assert.throws(() => ReviewedPubMedRetrievalReceipt.fromJSON({ ...result.receipt.toJSON(), record_count: 2 }), /count|digest/);
});

test("single-lane registration bridges acquire and project without persisting the raw bundle", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const registration = createReviewedPubMedAutonomousEvidenceRegistration(fixture.adapter, plan, "glioma");
  const registry = new AutonomousEvidenceAdapterRegistry();
  const manifest = registry.register(registration);
  assert.deepEqual(manifest.domains, ["biomedical", "neuroscience"]);
  const metadata = createReviewedPubMedExecutionMetadata(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  const context = {
    plan_digest: "b".repeat(64),
    requirement: { requirement_id: "req-1", domain: "biomedical", label: "Reviewed PubMed provenance" },
    request: { requirement_id: "req-1", source_id: "pubmed_glioma", source_digest: plan.plan_digest, metadata },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };
  const value = await registration.acquire(context);
  const observations = await registration.project(value, context);
  assert.equal(value.bundle.records.length, 1);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, value.receipt.bundle_digest);
  assert.throws(() => createReviewedPubMedAutonomousEvidenceRegistration(fixture.adapter, plan, "chiari_malformation"), /single-lane/);
});

test("promotion cross-binds receipt source, timestamp, record, and abstract metadata to its bundle", async () => {
  const fixture = fixtureAdapter();
  const plan = fixture.adapter.prepare();
  const registration = createReviewedPubMedAutonomousEvidenceRegistration(fixture.adapter, plan, "glioma");
  const context = {
    plan_digest: "b".repeat(64),
    requirement: { requirement_id: "req-1", domain: "biomedical", label: "Reviewed PubMed provenance" },
    request: {
      requirement_id: "req-1",
      source_id: "pubmed_glioma",
      source_digest: plan.plan_digest,
      metadata: createReviewedPubMedExecutionMetadata(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT }),
    },
    attempt: 1,
    parent_evidence_digests: [],
    execution: "caller_owned_adapter;raw_value_transient",
  };
  const authentic = await registration.acquire(context);
  const restamp = (receipt) => {
    const payload = { ...receipt };
    delete payload.receipt_digest;
    receipt.receipt_digest = digestJsonSync(payload);
  };
  const cases = [
    (receipt) => { receipt.generated_at = "2025-01-03T03:04:05Z"; },
    (receipt) => {
      receipt.sources[0].content_digest = "f".repeat(64);
      receipt.source_set_digest = digestJsonSync(receipt.sources);
    },
    (receipt) => {
      receipt.sources[0].record_count = 2;
      receipt.record_count = 2;
      receipt.source_set_digest = digestJsonSync(receipt.sources);
    },
    (receipt) => { receipt.abstract_count = 0; },
  ];
  for (const mutate of cases) {
    const forged = structuredClone(authentic);
    mutate(forged.receipt);
    restamp(forged.receipt);
    await assert.rejects(registration.project(forged, context), /receipt metadata does not match its bundle/);
  }
});

test("built-in transport refuses redirects and spaces requests at no more than three per second", async () => {
  const originalFetch = globalThis.fetch;
  const started = [];
  const responses = fixtureResponses();
  let index = 0;
  globalThis.fetch = async (_url, init) => {
    started.push(performance.now());
    assert.equal(init.method, "GET");
    assert.equal(init.redirect, "error");
    return new Response(responses[index++], { status: 200 });
  };
  try {
    const config = new ReviewedPubMedRetrievalConfig({ specialtyLanes: ["glioma"] });
    const adapter = new ReviewedPubMedRetrievalAdapter(config);
    const result = await adapter.execute(adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
    assert.equal(result.receipt.request_count, 3);
    assert.ok(started[1] - started[0] >= 325);
    assert.ok(started[2] - started[1] >= 325);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("fixed specialty catalogue and endpoint arrays cannot be mutated", () => {
  assert.equal(Object.isFrozen(PUBMED_SPECIALTY_LANES), true);
  assert.equal(Object.isFrozen(REVIEWED_PUBMED_ENDPOINTS), true);
  assert.throws(() => { PUBMED_SPECIALTY_LANES.glioma = "malicious"; }, TypeError);
  assert.throws(() => { REVIEWED_PUBMED_ENDPOINTS.push("evil.fcgi"); }, TypeError);
});
