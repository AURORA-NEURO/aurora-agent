import assert from "node:assert/strict";
import { test } from "node:test";
import {
  REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT,
  ReviewedGwasCatalogAncestryRetrievalAdapter,
  ReviewedGwasCatalogAncestryRetrievalConfig,
  ReviewedGwasCatalogAncestryRetrievalPlan,
  ReviewedGwasCatalogRetrievalError,
  createReviewedGwasCatalogAncestryAutonomousEvidenceRegistration,
  createReviewedGwasCatalogAncestryExecutionMetadata,
} from "../dist/index.js";

const TRANSPORT = { transportId: "fixture.gwas-catalog-ancestry", transportVersion: "1", transportConfigDigest: "1".repeat(64) };
const ACCESSIONS = ["GCST90296481", "GCST000854"];
function ancestry(accession, itemId, stage, individuals, group, countries) {
  return {
    type: stage,
    number_of_individuals: individuals,
    ancestral_groups: [{ ancestral_group: group }],
    country_of_origin: [],
    country_of_recruitment: countries,
    _links: { self: { href: `${REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/${accession}/ancestries/${itemId}` } },
  };
}
function fixture(accession) {
  if (accession === "GCST000854") return { _embedded: { ancestries: [
    ancestry(accession, 4595, "initial", 4390, "European", [{ major_area: "Europe", region: "Northern Europe", country_name: "U.K." }]),
    ancestry(accession, 7528, "replication", 2698, "European", [{ major_area: "Europe", region: "Western Europe", country_name: "Germany" }]),
    ancestry(accession, 7529, "replication", 1649, "NR", []),
  ] } };
  return { _embedded: { ancestries: [ancestry(accession, 165312141, "initial", 7273, "European", [
    { major_area: "Europe", region: "Northern Europe", country_name: "Sweden" },
    { major_area: "Northern America", country_name: "U.S." },
    { major_area: "Europe", region: "Northern Europe", country_name: "Denmark" },
  ])] } };
}
function config(studyAccessions = ACCESSIONS, options = {}) {
  return new ReviewedGwasCatalogAncestryRetrievalConfig({ studyAccessions, timeoutMs: 1500, ...TRANSPORT, ...options });
}
class Transport {
  constructor(corrupt) { this.urls = []; this.corrupt = corrupt; }
  fetch = (url) => {
    this.urls.push(url);
    const parsed = new URL(url);
    assert.equal(parsed.protocol, "https:");
    assert.equal(parsed.hostname, "www.ebi.ac.uk");
    const prefix = "/gwas/rest/api/v2/studies/";
    const accession = parsed.pathname.slice(prefix.length, -"/ancestries".length);
    assert.equal(parsed.pathname, `${prefix}${accession}/ancestries`);
    assert.ok(ACCESSIONS.includes(accession));
    const result = fixture(accession);
    const rows = result._embedded.ancestries;
    if (this.corrupt === "foreign_link") rows[0]._links.self.href = "https://example.invalid/studies/elsewhere/ancestries/1";
    else if (this.corrupt === "duplicate_id") rows.push(structuredClone(rows[0]));
    else if (this.corrupt === "bad_count") rows[0].number_of_individuals = -1;
    else if (this.corrupt === "duplicate_json") return JSON.stringify(result).replace('"type":"initial"', '"type":"initial","type":"initial"');
    else if (this.corrupt === "unknown_shape") result.unexpected = true;
    return result;
  };
}

test("config plan and explicit study collections are canonical and bounded", async () => {
  const transport = new Transport();
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config(), { fetch: transport.fetch });
  const plan = adapter.prepare();
  const result = await adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: "2026-09-28T12:00:00Z" });
  assert.deepEqual(plan.config.studyAccessions, ["GCST000854", "GCST90296481"]);
  assert.deepEqual(transport.urls, plan.config.studyAccessions.map((study) => `${REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/${study}/ancestries`));
  assert.equal(result.receipt.request_count, 2);
  assert.equal(result.bundle.study_count, 2);
  assert.equal(result.bundle.ancestry_count, 4);
  assert.equal(result.bundle.coverage, "all_source_rows");
  assert.deepEqual(result.bundle.studies[0].ancestries[2].country_of_recruitment, []);
  assert.deepEqual(ReviewedGwasCatalogAncestryRetrievalConfig.fromJSON(adapter.config.toJSON()).toJSON(), adapter.config.toJSON());
  assert.deepEqual(ReviewedGwasCatalogAncestryRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  assert.equal(plan.toJSON().maximum_source_ancestry_records_per_study, 500);
  assert.equal(result.receipt.plan_digest, plan.planDigest);
  assert.equal(result.bundle.bundle_digest.length, 64);
  assert.equal(plan.configDigest, "7204e82b206e09a120d53e87f64f2ca785974400d9f1bddb145849144d70ff88");
  assert.equal(plan.planDigest, "7f0bce4085cf1476b555c9121431b003d1fc70cbe66782f54078312dd26746b3");
  assert.equal(result.bundle.bundle_digest, "a89f598453cd2e4f375ba6f1af93f0be5477d12eb5ba1adc41546543f008e7b1");
  assert.equal(result.receipt.receipt_digest, "053dbf749a3778684be67558b12d280d5594e35ed1f21303c5e886520598331d");
});

test("literal approval precedes network and accession set is bounded", async () => {
  const transport = new Transport();
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config(), { fetch: transport.fetch });
  await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: 1 }), /literal approval/);
  assert.deepEqual(transport.urls, []);
  for (const studies of [[], ["GCST000854", "GCST000854"], ["GCST1"], Array.from({ length: 11 }, (_item, index) => `GCST${String(index).padStart(5, "0")}`)]) {
    assert.throws(() => config(studies), ReviewedGwasCatalogRetrievalError);
  }
  assert.throws(() => config(["GCST000854"], { transportId: 42 }), ReviewedGwasCatalogRetrievalError);
  assert.throws(() => createReviewedGwasCatalogAncestryAutonomousEvidenceRegistration(adapter, adapter.prepare(), "GCST90296481"), /single-study/);
});

test("injected transport is aborted at its reviewed timeout", async () => {
  const accession = "GCST90296481";
  let observedSignal;
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession], { timeoutMs: 100 }), {
    fetch: (_url, signal) => { observedSignal = signal; return new Promise(() => {}); },
  });
  await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: true }), /timed out/);
  assert.equal(observedSignal.aborted, true);
});

for (const corrupt of ["foreign_link", "duplicate_id", "bad_count", "duplicate_json", "unknown_shape"]) {
  test(`foreign or malformed ancestry responses fail closed: ${corrupt}`, async () => {
    const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config(["GCST000854"]), { fetch: new Transport(corrupt).fetch });
    await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: true }), ReviewedGwasCatalogRetrievalError);
  });
}

test("missing metadata and reported empty lists remain distinct", async () => {
  const accession = "GCST90296481";
  const row = ancestry(accession, 900, "initial", 100, "European", []);
  delete row.number_of_individuals;
  delete row.country_of_origin;
  row.ancestral_groups = null;
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession]), { fetch: () => ({ _embedded: { ancestries: [row] } }) });
  const projected = (await adapter.execute(adapter.prepare(), { approveSourceDispatch: true })).bundle.studies[0].ancestries[0];
  assert.equal(projected.number_of_individuals, null);
  assert.equal(projected.country_of_origin, null);
  assert.equal(projected.ancestral_groups, null);
  assert.deepEqual(projected.country_of_recruitment, []);
});

test("empty collection is a measured empty result", async () => {
  const accession = "GCST90296481";
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession]), { fetch: () => ({ _embedded: { ancestries: [] } }) });
  const study = (await adapter.execute(adapter.prepare(), { approveSourceDispatch: true })).bundle.studies[0];
  assert.equal(study.total_ancestry_records, 0);
  assert.equal(study.coverage, "all_source_rows");
  assert.deepEqual(study.ancestries, []);
});

test("output cap reports an explicit source-order prefix", async () => {
  const accession = "GCST90296481";
  const rows = Array.from({ length: 52 }, (_item, index) => ancestry(accession, index + 1, "initial", index + 1, "European", []));
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession]), { fetch: () => ({ _embedded: { ancestries: rows } }) });
  const study = (await adapter.execute(adapter.prepare(), { approveSourceDispatch: true })).bundle.studies[0];
  assert.equal(study.total_ancestry_records, 52);
  assert.equal(study.returned_ancestry_records, 50);
  assert.equal(study.omitted_ancestry_records, 2);
  assert.equal(study.coverage, "bounded_source_order_prefix");
  assert.equal(study.ancestries.at(-1).ancestry_id, 50);
});

test("oversized source collection fails before projection", async () => {
  const accession = "GCST90296481";
  const rows = Array.from({ length: 501 }, (_item, index) => ancestry(accession, index + 1, "initial", index + 1, "European", []));
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession]), { fetch: () => ({ _embedded: { ancestries: rows } }) });
  await assert.rejects(adapter.execute(adapter.prepare(), { approveSourceDispatch: true }), /source rows/);
});

test("autonomous projection retains only digests and rejects changed transient values", async () => {
  const accession = "GCST90296481";
  const adapter = new ReviewedGwasCatalogAncestryRetrievalAdapter(config([accession]), { fetch: () => fixture(accession) });
  const plan = adapter.prepare();
  const registration = createReviewedGwasCatalogAncestryAutonomousEvidenceRegistration(adapter, plan, accession);
  const metadata = createReviewedGwasCatalogAncestryExecutionMetadata(plan, true, "2026-09-28T12:00:00Z");
  const transient = await registration.acquire({ request: { source_id: `gwas_catalog_ancestry_${accession}`, source_digest: plan.planDigest, metadata } });
  const projected = await registration.project(transient, { requirement: { label: "GWAS study ancestry context" } });
  assert.equal(transient.retention, "caller_owned_transient_study_ancestry_metadata");
  assert.equal(projected[0].confidence, null);
  assert.ok(projected[0].value_digest && projected[0].source_digest);
  assert.equal("ancestries" in projected[0], false);
  const forged = structuredClone(transient);
  forged.bundle.studies[0].ancestries[0].ancestry_id += 1;
  await assert.rejects(registration.project(forged, { requirement: { label: "GWAS study ancestry context" } }), /digests/);
});
