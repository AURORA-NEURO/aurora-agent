import assert from "node:assert/strict";
import { test } from "node:test";
import {
  REVIEWED_GWAS_CATALOG_ENDPOINT,
  ReviewedGwasCatalogRetrievalAdapter,
  ReviewedGwasCatalogRetrievalConfig,
  ReviewedGwasCatalogRetrievalError,
  ReviewedGwasCatalogRetrievalPlan,
  createReviewedGwasCatalogAutonomousEvidenceRegistration,
  createReviewedGwasCatalogExecutionMetadata,
} from "../dist/index.js";

const transport = {
  transportId: "fixture.gwas-catalog",
  transportVersion: "1",
  transportConfigDigest: "1".repeat(64),
};
const traits = { gbm: "MONDO_0018177", glioma: "MONDO_0021042" };

function config(options = {}) {
  return new ReviewedGwasCatalogRetrievalConfig({ lanes: ["gbm", "glioma"], pageSize: 2, maxPages: 2, timeoutMs: 1500, ...transport, ...options });
}

function association(lane, rowId) {
  const trait = lane === "gbm" ? "glioblastoma" : "glioma";
  const title = lane === "gbm" ? "Glioblastoma" : "Glioma";
  return {
    association_id: rowId,
    accession_id: "GCST00001",
    pubmed_id: "12345678",
    first_author: "Example A",
    p_value: 0.125,
    efo_traits: [{ efo_id: traits[lane], efo_trait: trait }],
    reported_trait: [title],
    mapped_genes: ["EGFR", "TP53"],
    locations: ["7:55012345"],
  };
}

function page(lane, pageNumber, { total = 5, foreignNext = false } = {}) {
  const diseaseId = traits[lane];
  const pageSize = 2;
  const totalPages = Math.ceil(total / pageSize);
  const start = pageNumber * pageSize;
  const end = Math.min(total, start + pageSize);
  const offset = lane === "gbm" ? 100 : 200;
  const associations = Array.from({ length: Math.max(0, end - start) }, (_item, index) => association(lane, offset + start + index));
  const url = (number) => {
    const next = new URL(REVIEWED_GWAS_CATALOG_ENDPOINT);
    next.searchParams.set("efo_id", diseaseId);
    next.searchParams.set("show_child_traits", "false");
    next.searchParams.set("page", String(number));
    next.searchParams.set("size", String(pageSize));
    return next.toString();
  };
  const links = { self: { href: url(pageNumber) } };
  if (totalPages) {
    links.first = { href: url(0) };
    links.last = { href: url(totalPages - 1) };
    if (pageNumber + 1 < totalPages) links.next = { href: foreignNext ? "https://example.invalid/escape" : url(pageNumber + 1) };
  }
  const result = { _links: links, page: { size: pageSize, totalElements: total, totalPages, number: pageNumber } };
  if (associations.length) result._embedded = { associations };
  return result;
}

class Transport {
  constructor({ drift = false, foreignNext = false, corrupt } = {}) {
    this.urls = [];
    this.drift = drift;
    this.foreignNext = foreignNext;
    this.corrupt = corrupt;
  }
  fetch = (url, _signal) => {
    this.urls.push(url);
    const parsed = new URL(url);
    assert.equal(parsed.protocol, "https:");
    assert.equal(parsed.hostname, "www.ebi.ac.uk");
    assert.equal(parsed.pathname, "/gwas/rest/api/v2/associations");
    assert.deepEqual([...parsed.searchParams.keys()].sort(), ["efo_id", "page", "show_child_traits", "size"]);
    assert.equal(parsed.searchParams.get("show_child_traits"), "false");
    const lane = Object.keys(traits).find((key) => traits[key] === parsed.searchParams.get("efo_id"));
    const pageNumber = Number(parsed.searchParams.get("page"));
    const total = !this.drift || pageNumber === 0 ? 5 : 7;
    const value = page(lane, pageNumber, { total, foreignNext: this.foreignNext });
    if (this.corrupt === "foreign_trait" && value._embedded) value._embedded.associations[0].efo_traits[0].efo_id = "MONDO_0000001";
    if (this.corrupt === "bad_pvalue" && value._embedded) value._embedded.associations[0].p_value = 1.5;
    if (this.corrupt === "zero_pvalue" && value._embedded) value._embedded.associations[0].p_value = 0;
    if (this.corrupt === "duplicate_json" && value._embedded) return JSON.stringify(value).replace('"association_id":100,', '"association_id":100,"association_id":100,');
    return value;
  };
}

test("config, plan, and bounded output are deterministic for a shared transport identity", async () => {
  const transport = new Transport();
  const adapter = new ReviewedGwasCatalogRetrievalAdapter(config(), { fetch: transport.fetch });
  const plan = adapter.prepare();
  const result = await adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: "2026-09-28T12:00:00Z" });
  assert.equal(transport.urls.length, 4);
  assert.equal(result.bundle.coverage, "bounded_page_prefix");
  assert.equal(result.bundle.association_count, 8);
  assert.deepEqual(result.bundle.lanes.map((row) => row.returned_associations), [4, 4]);
  assert.deepEqual(result.bundle.lanes.map((row) => row.omitted_associations), [1, 1]);
  assert.equal(result.receipt.request_count, 4);
  assert.equal(result.receipt.coverage, "bounded_page_prefix");
  assert.equal(plan.configDigest, "4488a7090aab257524fb91dce8ac7540a4da6b1b2b4d6cc8467a3f794600527d");
  assert.equal(plan.planDigest, "c0a2eada1b2aaeb543d0f9b649d2d8c6a52aa8eeb7dc9afc5a0e6e323fade511");
  assert.equal(result.bundle.bundle_digest, "5cfdda6822b5c67eb962b9e2f93771814a260b07ba3e998903892e1101eac5fd");
  assert.equal(result.receipt.receipt_digest, "144dfce90e2780072b4b9376799d5d9c0913f0db417148955909cb05ebeaefeb");
  assert.deepEqual(ReviewedGwasCatalogRetrievalConfig.fromJSON(adapter.config.toJSON()).toJSON(), adapter.config.toJSON());
  assert.deepEqual(ReviewedGwasCatalogRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
});

test("dispatch requires literal approval and pagination stays on the fixed source", async () => {
  const transport = new Transport();
  const adapter = new ReviewedGwasCatalogRetrievalAdapter(config(), { fetch: transport.fetch });
  const plan = adapter.prepare();
  await assert.rejects(adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(transport.urls.length, 0);
  const result = await adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: "2026-09-28T12:00:00Z" });
  assert.ok(transport.urls.every((url) => url.startsWith(`${REVIEWED_GWAS_CATALOG_ENDPOINT}?`)));
  assert.deepEqual(transport.urls.map((url) => new URL(url).searchParams.get("page")), ["0", "1", "0", "1"]);
  assert.equal(result.bundle.lanes[0].query_semantics, "exact_ontology_trait_direct_matches_only");
});

for (const corrupt of ["foreign_trait", "bad_pvalue", "duplicate_json"]) {
  test(`foreign traits, invalid p-values, and duplicate JSON fail closed: ${corrupt}`, async () => {
    const adapter = new ReviewedGwasCatalogRetrievalAdapter(config(), { fetch: new Transport({ corrupt }).fetch });
    await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: true }), ReviewedGwasCatalogRetrievalError);
  });
}

test("same-origin pagination and stable totals are required", async () => {
  for (const transport of [new Transport({ foreignNext: true }), new Transport({ drift: true })]) {
    const adapter = new ReviewedGwasCatalogRetrievalAdapter(config(), { fetch: transport.fetch });
    await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: true }), ReviewedGwasCatalogRetrievalError);
  }
});

test("zero p-values stay distinct from positive reported values", async () => {
  const adapter = new ReviewedGwasCatalogRetrievalAdapter(config(), { fetch: new Transport({ corrupt: "zero_pvalue" }).fetch });
  const result = await adapter.execute(adapter.prepare(), { approveSourceDispatch: true });
  const row = result.bundle.lanes[0].associations[0];
  assert.equal(row.p_value, 0);
  assert.equal(row.p_value_state, "reported_zero_or_underflow");
});

test("autonomous evidence registration keeps association values transient and projects digests", async () => {
  const singleLaneConfig = new ReviewedGwasCatalogRetrievalConfig({ lanes: ["gbm"], pageSize: 2, maxPages: 2, timeoutMs: 1500, ...transport });
  const transportFixture = new Transport();
  const adapter = new ReviewedGwasCatalogRetrievalAdapter(singleLaneConfig, { fetch: transportFixture.fetch });
  const plan = adapter.prepare();
  const registration = createReviewedGwasCatalogAutonomousEvidenceRegistration(adapter, plan, "gbm");
  const metadata = createReviewedGwasCatalogExecutionMetadata(plan, true, "2026-09-28T12:00:00Z");
  const transient = await registration.acquire({ request: { source_id: "gwas_catalog_gbm", source_digest: plan.planDigest, metadata } });
  assert.equal(transient.retention, "caller_owned_transient_association_metadata");
  const projected = await registration.project(transient, { requirement: { label: "GBM genetic association metadata" } });
  assert.equal(projected.length, 1);
  assert.equal(projected[0].confidence, null);
  assert.ok(projected[0].value_digest && projected[0].source_digest);
  assert.equal("associations" in projected[0], false);
  const forged = structuredClone(transient);
  forged.bundle.lanes[0].associations[0].association_id += 1;
  await assert.rejects(registration.project(forged, { requirement: { label: "GBM genetic association metadata" } }), /digests/);
});
