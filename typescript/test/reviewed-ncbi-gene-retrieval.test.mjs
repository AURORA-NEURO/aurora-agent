import assert from "node:assert/strict";
import test from "node:test";

import {
  BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST,
  BUILTIN_NCBI_GENE_TRANSPORT_ID,
  REVIEWED_NCBI_GENE_CATALOGUE,
  AutonomousEvidenceAdapterRegistry,
  ReviewedNcbiGeneRetrievalAdapter,
  ReviewedNcbiGeneRetrievalConfig,
  ReviewedNcbiGeneRetrievalPlan,
  createReviewedNcbiGeneAutonomousEvidenceRegistration,
  createReviewedNcbiGeneExecutionMetadata,
  digestJsonSync,
} from "../dist/index.js";

const TRANSPORT = {
  transportId: "fixture.ncbi-gene",
  transportVersion: "1",
  transportConfigDigest: "a".repeat(64),
};
const RETRIEVED_AT = "2025-01-02T03:04:05Z";
const SYMBOLS = ["IDH1", "MGMT"];

function summary() {
  return {
    header: { type: "esummary", version: "0.3" },
    result: {
      uids: ["3417", "4255"],
      "3417": {
        uid: "3417", name: "IDH1", description: "isocitrate dehydrogenase 1",
        chromosome: "2", maplocation: "2q34", otheraliases: "HEL-216, IDP",
        organism: { taxid: 9606, scientificname: "Homo sapiens" },
        summary: "This full functional summary must remain transient and unprojected.",
        genomicinfo: [{ chraccver: "NC_000002.12", chrstart: 1 }],
      },
      "4255": {
        uid: "4255", name: "MGMT", description: "O-6-methylguanine-DNA methyltransferase",
        chromosome: "10", maplocation: "10q26.3", otheraliases: "AGT, MGMT1",
        organism: { taxid: 9606, scientificname: "Homo sapiens" },
        summary: "Another non-projected functional summary.",
      },
    },
  };
}

function fixtureAdapter(response = summary(), options = {}) {
  const config = new ReviewedNcbiGeneRetrievalConfig({ geneSymbols: options.geneSymbols ?? SYMBOLS, ...TRANSPORT, ...options });
  const calls = [];
  const adapter = new ReviewedNcbiGeneRetrievalAdapter(config, { fetch: (url) => { calls.push(url); return typeof response === "string" ? response : JSON.stringify(response); } });
  return { config, adapter, calls };
}

test("NCBI Gene config, plan, fixed catalogue, and digest parity match Python", async () => {
  assert.equal(Object.isFrozen(REVIEWED_NCBI_GENE_CATALOGUE), true);
  assert.throws(() => { REVIEWED_NCBI_GENE_CATALOGUE.OTHER = "1"; }, TypeError);
  assert.equal(REVIEWED_NCBI_GENE_CATALOGUE.PDGFRA, "5156");
  const builtin = new ReviewedNcbiGeneRetrievalConfig();
  assert.equal(builtin.transportId, BUILTIN_NCBI_GENE_TRANSPORT_ID);
  assert.match(BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST, /^[0-9a-f]{64}$/);
  const fixture = fixtureAdapter(summary(), { geneSymbols: ["MGMT", "IDH1"] });
  assert.deepEqual(fixture.config.geneSymbols, SYMBOLS);
  const plan = fixture.adapter.prepare();
  assert.equal(fixture.config.requestLimit, 1);
  assert.deepEqual(ReviewedNcbiGeneRetrievalConfig.fromJSON(fixture.config.toJSON()).toJSON(), fixture.config.toJSON());
  assert.deepEqual(ReviewedNcbiGeneRetrievalPlan.fromJSON(plan.toJSON()).toJSON(), plan.toJSON());
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(result.receipt.bundle_digest, "9943c5606ff60d2612c3e9d040b3491d3e7affaeb56e6b1a6becdac042d9102a");
  assert.equal(result.receipt.receipt_digest, "980bafb3726e18411fb6625093b35e186bd062445a1450be6e459b556bbc9435");
  assert.equal(result.receipt.response_bytes, Buffer.byteLength(JSON.stringify(summary())));
  const detached = result.receipt; detached.bundle_digest = "tampered";
  assert.notEqual(result.receipt.bundle_digest, "tampered");
});

test("literal approval dispatches one fixed ESummary request and retains only human gene metadata", async () => {
  const fixture = fixtureAdapter(); const plan = fixture.adapter.prepare();
  await assert.rejects(fixture.adapter.execute(plan, { approveSourceDispatch: 1 }), /literal approval/);
  assert.equal(fixture.calls.length, 0);
  const result = await fixture.adapter.execute(plan, { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.equal(fixture.calls.length, 1);
  const url = new URL(fixture.calls[0]);
  assert.equal(url.origin, "https://eutils.ncbi.nlm.nih.gov");
  assert.equal(url.pathname, "/entrez/eutils/esummary.fcgi");
  assert.deepEqual(Object.fromEntries(url.searchParams), { db: "gene", id: "3417,4255", retmode: "json" });
  const bundle = result.toTransientJSON().bundle;
  assert.equal(result.receipt.request_count, 1);
  assert.equal(result.receipt.catalogue_coverage, "complete");
  assert.deepEqual(bundle.genes[0], {
    source_id: "ncbi_gene", gene_id: "3417", symbol: "IDH1", description: "isocitrate dehydrogenase 1",
    chromosome: "2", map_location: "2q34", aliases: ["HEL-216", "IDP"], organism_taxid: 9606,
  });
  const serialized = JSON.stringify(bundle).toLowerCase();
  assert.equal(serialized.includes("full functional summary"), false);
  assert.equal(serialized.includes("genomicinfo"), false);
  assert.equal(serialized.includes("chrstart"), false);
  assert.equal(result.toJSON().retention, "metadata_only");
});

test("registered NCBI tool and email values are paired, bound by digest, and excluded from durable metadata", async () => {
  const email = "researcher@example.org";
  const fixture = fixtureAdapter(summary(), { ncbiTool: "aurora_agent", ncbiEmail: email });
  const serializedConfig = JSON.stringify(fixture.config.toJSON());
  assert.equal(serializedConfig.includes(email), false);
  assert.equal(serializedConfig.includes("aurora_agent"), false);
  const plan = fixture.adapter.prepare();
  assert.equal(JSON.stringify(plan.toJSON()).includes(email), false);
  assert.equal(JSON.stringify(plan.toJSON()).includes("aurora_agent"), false);
  const metadata = createReviewedNcbiGeneExecutionMetadata(plan, true, RETRIEVED_AT);
  assert.equal(JSON.stringify(metadata).includes(email), false);
  assert.equal(JSON.stringify(metadata).includes("aurora_agent"), false);
  assert.deepEqual(ReviewedNcbiGeneRetrievalConfig.fromJSON(fixture.config.toJSON(), { ncbiTool: "aurora_agent", ncbiEmail: email }).toJSON(), fixture.config.toJSON());
  assert.throws(() => ReviewedNcbiGeneRetrievalConfig.fromJSON(fixture.config.toJSON(), { ncbiTool: "different_tool", ncbiEmail: email }), /registration identity changed/);
  assert.throws(() => new ReviewedNcbiGeneRetrievalConfig({ ncbiTool: "aurora_agent", ...TRANSPORT }), /provided together/);
  const result = await fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true, retrievedAt: RETRIEVED_AT });
  assert.deepEqual(Object.fromEntries(new URL(fixture.calls[0]).searchParams), { db: "gene", id: "3417,4255", retmode: "json", tool: "aurora_agent", email });
  assert.equal(JSON.stringify(result.toTransientJSON()).includes(email), false);
  assert.equal(JSON.stringify(result.toTransientJSON()).includes("aurora_agent"), false);
  assert.equal(result.toTransientJSON().bundle.sources[0].uri.includes(email), false);
});

test("wrong organism, symbol, UID coverage, and duplicate fields fail closed", async () => {
  let payload = summary(); payload.result["3417"].organism.taxid = 10090;
  let fixture = fixtureAdapter(payload);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /human organism/);
  payload = summary(); payload.result["3417"].name = "IDH2";
  fixture = fixtureAdapter(payload);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /symbol differs/);
  payload = summary(); payload.result.uids = ["3417"];
  fixture = fixtureAdapter(payload);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /incomplete or reordered/);
  fixture = fixtureAdapter('{"result":{"uids":["3417","4255"],"3417":{"uid":"3417","uid":"4255"}}}');
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /duplicate JSON fields/);
  assert.throws(() => new ReviewedNcbiGeneRetrievalAdapter(new ReviewedNcbiGeneRetrievalConfig(), { fetch: () => "{}" }), /distinct reviewed identity/);
  assert.throws(() => new ReviewedNcbiGeneRetrievalConfig({ geneSymbols: ["UNKNOWN"], ...TRANSPORT }), /unsupported or duplicate/);
});

test("oversized and structurally deep NCBI responses fail closed", async () => {
  let fixture = fixtureAdapter(" ".repeat(1_000_001));
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /byte bound/);
  const payload = summary(); let cursor = payload;
  for (let index = 0; index < 40; index += 1) { cursor.nested = {}; cursor = cursor.nested; }
  fixture = fixtureAdapter(payload);
  await assert.rejects(fixture.adapter.execute(fixture.adapter.prepare(), { approveSourceDispatch: true }), /structural bound/);
});

test("autonomous evidence requires approved metadata and projects digest-only provenance", async () => {
  const fixture = fixtureAdapter(); const plan = fixture.adapter.prepare();
  const registration = createReviewedNcbiGeneAutonomousEvidenceRegistration(fixture.adapter, plan);
  assert.equal(new AutonomousEvidenceAdapterRegistry().register(registration).adapter_id, "reviewed.ncbi_gene");
  const metadata = createReviewedNcbiGeneExecutionMetadata(plan, true, RETRIEVED_AT);
  const context = { request: { source_id: "ncbi_gene", source_digest: plan.planDigest, metadata }, requirement: { label: "NCBI Gene catalogue" } };
  const transient = await registration.acquire(context);
  const observations = await registration.project(transient, context);
  assert.equal(observations[0].kind, "provenance");
  assert.equal(observations[0].value_digest, transient.receipt.bundle_digest);
  assert.equal(JSON.stringify(observations).includes("isocitrate dehydrogenase"), false);
  const forged = { ...metadata, approve_source_dispatch: false };
  forged.metadata_digest = digestJsonSync(Object.fromEntries(Object.entries(forged).filter(([key]) => key !== "metadata_digest")));
  await assert.rejects(registration.acquire({ ...context, request: { ...context.request, metadata: forged } }), /failed review binding/);
  const tampered = JSON.parse(JSON.stringify(transient));
  tampered.bundle.genes[0].gene_id = "4255";
  tampered.bundle.bundle_digest = digestJsonSync(Object.fromEntries(Object.entries(tampered.bundle).filter(([key]) => key !== "bundle_digest")));
  tampered.receipt.bundle_digest = tampered.bundle.bundle_digest;
  tampered.receipt.receipt_digest = digestJsonSync(Object.fromEntries(Object.entries(tampered.receipt).filter(([key]) => key !== "receipt_digest")));
  await assert.rejects(registration.project(tampered, context), /reviewed catalogue/);
});
