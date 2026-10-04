/** Reviewed, bounded retrieval of fixed direct-trait GWAS Catalog v2 associations. */

import { ArgumentError } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonValue } from "./types.js";

export const REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA = "bioprism-reviewed-gwas-catalog-config/0.1" as const;
export const REVIEWED_GWAS_CATALOG_PLAN_SCHEMA = "bioprism-reviewed-gwas-catalog-plan/0.1" as const;
export const REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-source-receipt/0.1" as const;
export const REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA = "bioprism-reviewed-gwas-catalog-bundle/0.1" as const;
export const REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-receipt/0.1" as const;
export const REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA = "bioprism-reviewed-gwas-catalog-transient/0.1" as const;
export const REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-gwas-catalog-execution-metadata/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_CONFIG_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-config/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_PLAN_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-plan/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-source-receipt/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_BUNDLE_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-bundle/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_RECEIPT_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-receipt/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_TRANSIENT_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-transient/0.1" as const;
export const REVIEWED_GWAS_CATALOG_ANCESTRY_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-gwas-catalog-ancestry-execution-metadata/0.1" as const;
export const REVIEWED_GWAS_CATALOG_HOST = "www.ebi.ac.uk" as const;
export const REVIEWED_GWAS_CATALOG_ENDPOINT = `https://${REVIEWED_GWAS_CATALOG_HOST}/gwas/rest/api/v2/associations` as const;
export const REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT = `https://${REVIEWED_GWAS_CATALOG_HOST}/gwas/rest/api/v2/studies` as const;
export const REVIEWED_GWAS_CATALOG_AUTHORITY = "NHGRI-EBI GWAS Catalog" as const;
export const REVIEWED_GWAS_CATALOG_LANES = Object.freeze({ gbm: "MONDO_0018177", glioma: "MONDO_0021042" } as const);
export type ReviewedGwasCatalogLane = keyof typeof REVIEWED_GWAS_CATALOG_LANES;
export const MAX_REVIEWED_GWAS_CATALOG_LANES = 2;
export const MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE = 50;
export const MAX_REVIEWED_GWAS_CATALOG_PAGES = 5;
export const MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES = 512_000;
export const MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES = 2_000_000;
export const MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES = 1_000_000;
export const MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH = 20;
export const MAX_REVIEWED_GWAS_CATALOG_TREE_NODES = 80_000;
export const REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS = 100;
export const MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES = 10;
export const MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY = 50;
export const MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY = 500;
export const BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_ID = "builtin.gwas-catalog.ancestry.fetch" as const;
export const BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_GWAS_CATALOG_TRANSPORT_ID = "builtin.gwas-catalog.fetch" as const;
export const BUILTIN_GWAS_CATALOG_TRANSPORT_VERSION = "1" as const;

const USER_AGENT = "AURORA-Prism-SDK/0.1";
const RETENTION = "bounded_transient_association_metadata;autonomous_evidence_digest_only" as const;
const LIMITATIONS = Object.freeze([
  "GWAS Catalog REST API v2 supplies literature-curated top associations, not complete genome-wide summary statistics",
  "the adapter queries only direct matches to its fixed ontology traits and does not expand child traits",
  "only the first bounded page prefix is retrieved; returned order is source pagination order, not a ranking",
  "p-values and reported traits are copied metadata and do not establish causality, clinical relevance, or treatment benefit",
  "a numeric p-value of zero is retained as source output and flagged as possibly precision-limited, not interpreted as certainty",
  "mapped genes and variant locations are Catalog annotations and are not causal gene or mechanism assignments",
  "the fixed glioblastoma and glioma catalogue is not an exhaustive disease or population search",
  "source records and totals can change as the Catalog is updated",
  "independent review is required for source quality, ancestry, applicability, omissions, and freshness",
  "the adapter does not retrieve patient-level data, full summary statistics, or individual-level genotypes",
  "caller-injected transports own redirect, network, and credential policy under their declared identity",
]);
const LANE_DETAILS: Readonly<Record<ReviewedGwasCatalogLane, { diseaseId: string; diseaseName: string }>> = Object.freeze({
  gbm: { diseaseId: "MONDO_0018177", diseaseName: "glioblastoma" },
  glioma: { diseaseId: "MONDO_0021042", diseaseName: "glioma" },
});
const IDENTIFIER_RE = /^[A-Za-z0-9_.:-]{1,128}$/;
const ACCESSION_RE = /^GCST[0-9]{5,14}$/;
const PUBMED_RE = /^[0-9]{1,12}$/;
const DIGEST_RE = /^[0-9a-f]{64}$/;
const NativeObjectKeys = Object.keys.bind(globalThis.Object);
const NativeObjectValues = Object.values.bind(globalThis.Object);
const NativeOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty) as (value: object, key: PropertyKey) => boolean;
const NativeFreeze = globalThis.Object.freeze.bind(globalThis.Object);
const NativeSet = globalThis.Set;
const NativeWeakSet = globalThis.WeakSet;
const NativeDate = globalThis.Date;
const NativeIsFinite = globalThis.Number.isFinite.bind(globalThis.Number);
const NativeIsInteger = globalThis.Number.isInteger.bind(globalThis.Number);
const NativeIsSafeInteger = globalThis.Number.isSafeInteger.bind(globalThis.Number);
const NativeJsonParse = globalThis.JSON.parse.bind(globalThis.JSON);
const NativeJsonStringify = globalThis.JSON.stringify.bind(globalThis.JSON);
const NativeTextEncoder = globalThis.TextEncoder;
const NativeTextDecoder = globalThis.TextDecoder;
const NativeURL = globalThis.URL;
const NativeFetch = typeof globalThis.fetch === "function" ? globalThis.fetch.bind(globalThis) : undefined;
const NativeAbortController = globalThis.AbortController;
const NativePromiseRace = globalThis.Promise.race.bind(globalThis.Promise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;
const NativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const NativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const Encoder = new NativeTextEncoder();
let nextRequestAt = 0;

export const BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_GWAS_CATALOG_HOST,
  path: "/gwas/rest/api/v2/associations", method: "GET", accept: "application/json", user_agent: USER_AGENT,
  query: ["efo_id", "show_child_traits=false", "page", "size"], pagination: "validate_and_follow_same_origin_next_link",
  minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS,
  redirects: "refused", credentials: "not_accepted",
});
export const BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "bounded_https_get", scheme: "https", host: REVIEWED_GWAS_CATALOG_HOST,
  path: "/gwas/rest/api/v2/studies/{accession_id}/ancestries", method: "GET", accept: "application/json", user_agent: USER_AGENT,
  pagination: "none_one_complete_collection_response_per_explicit_study", minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS,
  redirects: "refused", credentials: "not_accepted",
});

export class ReviewedGwasCatalogRetrievalError extends ArgumentError {
  override readonly name = "ReviewedGwasCatalogRetrievalError";
}
function fail(message: string): never { throw new ReviewedGwasCatalogRetrievalError(message); }
function byteLength(value: string): number { return Encoder.encode(value).byteLength; }
function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST_RE.test(value)) fail(`${name} is invalid`);
  return value;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!NativeIsSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its bounded integer range`);
  return value as number;
}
function text(name: string, value: unknown, maximum: number, allowEmpty = false): string {
  if (typeof value !== "string") fail(`${name} must be text`);
  const normalized = (value as string).replace(/[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu, " ").replace(/^ | $/g, "");
  if ((!normalized && !allowEmpty) || /[\u0000-\u001f\u007f]/.test(normalized) || byteLength(normalized) > maximum || /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(normalized)) fail(`${name} is empty or exceeds its text bound`);
  return normalized;
}
function timestamp(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)) fail(`${name} must be a UTC timestamp with second precision`);
  const date = new NativeDate(value);
  if (!NativeIsFinite(date.getTime()) || date.toISOString().replace(".000Z", "Z") !== value) fail(`${name} is not a valid UTC timestamp`);
  return value;
}
function now(): string { return new NativeDate().toISOString().replace(/\.\d{3}Z$/, "Z"); }
function exactObject(name: string, value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(`${name} must be an object`);
  const actual = NativeObjectKeys(value);
  if (actual.length !== keys.length || keys.some((key) => !NativeOwn(value, key))) fail(`${name} has an invalid shape`);
  return value as Record<string, unknown>;
}
function validateTree(root: unknown): void {
  const pending: Array<{ value: unknown; depth: number }> = [{ value: root, depth: 0 }];
  const seen = new NativeWeakSet<object>(); let nodes = 0;
  while (pending.length) {
    const next = pending.pop()!; nodes += 1;
    if (nodes > MAX_REVIEWED_GWAS_CATALOG_TREE_NODES || next.depth > MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH) fail("GWAS Catalog response exceeds its structural bound");
    const value = next.value;
    if (typeof value === "number" && (!NativeIsFinite(value) || (NativeIsInteger(value) && !NativeIsSafeInteger(value)))) fail("GWAS Catalog response contains an invalid number");
    if (value !== null && typeof value === "object") {
      if (seen.has(value)) fail("GWAS Catalog response is not a JSON tree"); seen.add(value);
      if (Array.isArray(value)) for (const child of value) pending.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(value) !== Object.prototype) fail("GWAS Catalog response contains a non-JSON object");
        for (const key of NativeObjectKeys(value)) if (/[\u0000-\u001f\u007f]/.test(key) || /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(key)) fail("GWAS Catalog response contains an invalid key");
        for (const child of NativeObjectValues(value as Record<string, unknown>)) pending.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof value === "string" && /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(value)) fail("GWAS Catalog response contains invalid Unicode");
    else if (value !== null && !["string", "number", "boolean"].includes(typeof value)) fail("GWAS Catalog response contains an unsupported value");
  }
}
function parseStrictJson(source: string): unknown {
  let index = 0; let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index; if (source[index] !== "\"") fail("GWAS Catalog response is not valid JSON"); index += 1;
    while (index < source.length) {
      if (source[index] === "\"") { index += 1; try { return NativeJsonParse(source.slice(start, index)) as string; } catch { return fail("GWAS Catalog response is not valid JSON"); } }
      if (source[index] === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 32) fail("GWAS Catalog response is not valid JSON"); index += 1;
    }
    return fail("GWAS Catalog response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1; if (nodes > MAX_REVIEWED_GWAS_CATALOG_TREE_NODES || depth > MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH) fail("GWAS Catalog response exceeds its structural bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace(); const keys = new NativeSet<string>(); if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string(); if (keys.has(key)) fail("GWAS Catalog response contains duplicate JSON fields"); keys.add(key); whitespace();
        if (source[index] !== ":") fail("GWAS Catalog response is not valid JSON"); index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; } if (source[index] !== ",") fail("GWAS Catalog response is not valid JSON"); index += 1;
      }
      return fail("GWAS Catalog response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace(); if (source[index] === "]") { index += 1; return; }
      while (index < source.length) { scan(depth + 1); whitespace(); if (source[index] === "]") { index += 1; return; } if (source[index] !== ",") fail("GWAS Catalog response is not valid JSON"); index += 1; }
      return fail("GWAS Catalog response is not valid JSON");
    }
    const start = index; while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (start === index) fail("GWAS Catalog response is not valid JSON"); try { NativeJsonParse(source.slice(start, index)); } catch { fail("GWAS Catalog response is not valid JSON"); }
  };
  scan(0); whitespace(); if (index !== source.length) fail("GWAS Catalog response is not valid JSON");
  const parsed = NativeJsonParse(source) as unknown; validateTree(parsed); return parsed;
}
function parseResponse(value: unknown): { payload: Record<string, unknown>; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) { try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("GWAS Catalog response is not valid UTF-8"); } }
  else if (typeof value === "object" && value !== null && !Array.isArray(value)) { validateTree(value); try { body = NativeJsonStringify(value) as string; } catch { return fail("GWAS Catalog injected response is not JSON"); } }
  else return fail("GWAS Catalog transport returned an unsupported response type");
  const bytes = byteLength(body); if (bytes > MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES) fail("GWAS Catalog response exceeds its byte bound");
  const parsed = parseStrictJson(body); if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) fail("GWAS Catalog response root is not an object");
  return { payload: parsed as Record<string, unknown>, bytes };
}
function queryUrl(diseaseId: string, page: number, pageSize: number): string {
  const url = new NativeURL(REVIEWED_GWAS_CATALOG_ENDPOINT);
  url.searchParams.set("efo_id", diseaseId); url.searchParams.set("show_child_traits", "false"); url.searchParams.set("page", String(page)); url.searchParams.set("size", String(pageSize));
  return url.toString();
}
function validatePageUrl(value: unknown, diseaseId: string, page: number, pageSize: number): void {
  if (typeof value !== "string" || byteLength(value) > 2_048) fail("GWAS Catalog pagination link is invalid");
  let url: URL; try { url = new NativeURL(value); } catch { return fail("GWAS Catalog pagination link is malformed"); }
  const expected: Record<string, string> = { efo_id: diseaseId, show_child_traits: "false", page: String(page), size: String(pageSize) };
  const entries: Array<[string, string]> = []; url.searchParams.forEach((entryValue, entryKey) => entries.push([entryKey, entryValue]));
  if (url.protocol !== "https:" || url.hostname !== REVIEWED_GWAS_CATALOG_HOST || url.port || url.username || url.password || url.pathname !== "/gwas/rest/api/v2/associations" || url.hash || entries.length !== 4 || new NativeSet(entries.map(([key]) => key)).size !== 4 || canonicalJson(Object.fromEntries(entries)) !== canonicalJson(expected)) fail("GWAS Catalog pagination link left its reviewed endpoint or query");
}
async function paceRequests(): Promise<void> {
  const current = Date.now(); const start = Math.max(current, nextRequestAt); nextRequestAt = start + REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS;
  if (start > current) await new Promise<void>((resolve) => NativeSetTimeout(resolve, start - current));
}
async function builtinFetch(url: string, timeoutMs: number): Promise<Uint8Array> {
  if (NativeFetch === undefined || NativeAbortController === undefined) fail("GWAS Catalog built-in fetch is unavailable");
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedGwasCatalogRetrievalError("GWAS Catalog request timed out")); }, timeoutMs); });
  try {
    const operation = NativeFetch(url, { method: "GET", headers: { Accept: "application/json", "User-Agent": USER_AGENT }, redirect: "error", signal: controller.signal }).then(async (response) => {
      if (!response.ok || response.url !== url || response.redirected || !response.body) fail("GWAS Catalog endpoint returned an unexpected response");
      const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let total = 0;
      while (true) { const { done, value } = await reader.read(); if (done) break; if (!(value instanceof Uint8Array) || total + value.byteLength > MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES) { try { await reader.cancel(); } catch { /* Keep the bounded refusal. */ } fail("GWAS Catalog response exceeds its byte bound"); } chunks.push(value); total += value.byteLength; }
      const result = new Uint8Array(total); let offset = 0; for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.byteLength; } return result;
    });
    return await NativePromiseRace([operation, timeout]);
  } catch (error) { if (error instanceof ReviewedGwasCatalogRetrievalError) throw error; throw new ReviewedGwasCatalogRetrievalError("GWAS Catalog request failed"); }
  finally { if (timer !== undefined) NativeClearTimeout(timer); }
}
function normalizeRow(raw: unknown, lane: ReviewedGwasCatalogLane): Record<string, JsonValue> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("GWAS Catalog association row is malformed");
  const row = raw as Record<string, unknown>;
  const associationId = integer("GWAS Catalog association_id", row.association_id, 1, Number.MAX_SAFE_INTEGER);
  const accession = text("GWAS Catalog accession_id", row.accession_id, 32); if (!ACCESSION_RE.test(accession)) fail("GWAS Catalog study accession is invalid");
  if (!Array.isArray(row.efo_traits) || row.efo_traits.length < 1 || row.efo_traits.length > 32) fail("GWAS Catalog EFO trait list is malformed");
  const seenTraits = new NativeSet<string>(); const traits: JsonValue[] = [];
  for (const item of row.efo_traits) {
    const trait = exactObject("GWAS Catalog EFO trait entry", item, ["efo_id", "efo_trait"]);
    const efoId = text("GWAS Catalog efo_id", trait.efo_id, 64); const efoTrait = text("GWAS Catalog efo_trait", trait.efo_trait, 256);
    if (seenTraits.has(efoId)) fail("GWAS Catalog EFO trait list contains duplicates"); seenTraits.add(efoId); traits.push({ efo_id: efoId, efo_trait: efoTrait });
  }
  if (!seenTraits.has(LANE_DETAILS[lane].diseaseId)) fail("GWAS Catalog association does not match the exact reviewed ontology trait");
  if (!Array.isArray(row.reported_trait) || row.reported_trait.length > 64) fail("GWAS Catalog reported_trait list is malformed");
  const reportedTraits = row.reported_trait.map((item) => text("GWAS Catalog reported trait", item, 512));
  if (!Array.isArray(row.mapped_genes) || row.mapped_genes.length > 256 || !Array.isArray(row.locations) || row.locations.length > 256) fail("GWAS Catalog mapped gene or location list is malformed");
  const genes = row.mapped_genes.map((item) => text("GWAS Catalog mapped gene", item, 128)); const locations = row.locations.map((item) => text("GWAS Catalog location", item, 128));
  if (new NativeSet(genes).size !== genes.length || new NativeSet(locations).size !== locations.length) fail("GWAS Catalog association contains duplicate gene or location rows");
  let pubmedId: string | null;
  if (typeof row.pubmed_id === "number") pubmedId = String(integer("GWAS Catalog pubmed_id", row.pubmed_id, 1, 999_999_999_999));
  else if (typeof row.pubmed_id === "string") pubmedId = text("GWAS Catalog pubmed_id", row.pubmed_id, 12);
  else if (row.pubmed_id === undefined || row.pubmed_id === null) pubmedId = null;
  else return fail("GWAS Catalog pubmed_id has an unsupported type");
  if (pubmedId !== null && !PUBMED_RE.test(pubmedId)) fail("GWAS Catalog pubmed_id is invalid");
  let pValue: number | null; let pValueState: "reported" | "reported_zero_or_underflow" | "not_reported";
  if (row.p_value === undefined || row.p_value === null) { pValue = null; pValueState = "not_reported"; }
  else if (typeof row.p_value === "number" && NativeIsFinite(row.p_value) && row.p_value >= 0 && row.p_value <= 1) { if (Object.is(row.p_value, -0)) fail("GWAS Catalog p_value uses a negative zero"); pValue = row.p_value; pValueState = row.p_value === 0 ? "reported_zero_or_underflow" : "reported"; }
  else return fail("GWAS Catalog p_value is invalid");
  const firstAuthor = row.first_author === undefined || row.first_author === null ? null : text("GWAS Catalog first_author", row.first_author, 256);
  return { association_id: associationId, study_accession: accession, pubmed_id: pubmedId, first_author: firstAuthor, p_value: pValue, p_value_state: pValueState, efo_traits: traits, reported_traits: reportedTraits, mapped_genes: genes, locations };
}
function validateProjectedRow(raw: unknown, lane: ReviewedGwasCatalogLane): Record<string, JsonValue> {
  const row = exactObject("GWAS Catalog projected association row", raw, ["association_id", "study_accession", "pubmed_id", "first_author", "p_value", "p_value_state", "efo_traits", "reported_traits", "mapped_genes", "locations"]);
  const normalized = normalizeRow({ association_id: row.association_id, accession_id: row.study_accession, pubmed_id: row.pubmed_id, first_author: row.first_author, p_value: row.p_value, efo_traits: row.efo_traits, reported_trait: row.reported_traits, mapped_genes: row.mapped_genes, locations: row.locations }, lane);
  if (row.p_value_state !== normalized.p_value_state || canonicalJson(row) !== canonicalJson(normalized)) fail("GWAS Catalog projected association row is not normalized");
  return normalized;
}
function pageRows(raw: Record<string, unknown>, lane: ReviewedGwasCatalogLane, pageNumber: number, pageSize: number): { rows: Record<string, unknown>[]; total: number; totalPages: number } {
  const rootKeys = NativeObjectKeys(raw).sort().join(","); if (rootKeys !== "_embedded,_links,page" && rootKeys !== "_links,page") fail("GWAS Catalog page has an unexpected object shape");
  const page = exactObject("GWAS Catalog page metadata", raw.page, ["size", "totalElements", "totalPages", "number"]);
  const size = integer("GWAS Catalog page size", page.size, 1, MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE); const total = integer("GWAS Catalog total elements", page.totalElements, 0, 50_000_000);
  const totalPages = integer("GWAS Catalog total pages", page.totalPages, 0, 50_000_000); const number = integer("GWAS Catalog page number", page.number, 0, 50_000_000);
  const expectedPages = total ? Math.ceil(total / size) : 0;
  if (size !== pageSize || totalPages !== expectedPages || number !== pageNumber || (!total && pageNumber !== 0) || pageNumber >= Math.max(totalPages, 1)) fail("GWAS Catalog page metadata differs from the requested page");
  let rows: Record<string, unknown>[];
  if (!total) {
    if ((raw._embedded !== undefined && canonicalJson(raw._embedded) !== "{}") || pageNumber !== 0) fail("empty GWAS Catalog result has unexpected association rows"); rows = [];
  } else {
    const embedded = exactObject("GWAS Catalog embedded result", raw._embedded, ["associations"]);
    if (!Array.isArray(embedded.associations) || embedded.associations.some((item) => typeof item !== "object" || item === null || Array.isArray(item))) fail("GWAS Catalog association collection is malformed");
    rows = embedded.associations as Record<string, unknown>[];
    if (rows.length !== Math.min(size, total - pageNumber * size)) fail("GWAS Catalog association page is incomplete");
  }
  const links = raw._links; if (typeof links !== "object" || links === null || Array.isArray(links)) fail("GWAS Catalog pagination links are malformed");
  const diseaseId = LANE_DETAILS[lane].diseaseId; const expectedKeys = ["self"];
  if (totalPages) { expectedKeys.push("first", "last"); if (pageNumber + 1 < totalPages) expectedKeys.push("next"); }
  if (canonicalJson(NativeObjectKeys(links).sort()) !== canonicalJson(expectedKeys.sort())) fail("GWAS Catalog pagination links do not match page coverage");
  for (const [name, targetPage] of [["self", pageNumber], ["first", 0], ["last", Math.max(totalPages - 1, 0)], ["next", pageNumber + 1]] as const) {
    if (expectedKeys.includes(name)) validatePageUrl(exactObject("GWAS Catalog pagination link", (links as Record<string, unknown>)[name], ["href"]).href, diseaseId, targetPage, pageSize);
  }
  return { rows, total, totalPages };
}

export interface ReviewedGwasCatalogRetrievalConfigOptions {
  lanes?: readonly ReviewedGwasCatalogLane[]; pageSize?: number; maxPages?: number; timeoutMs?: number;
  transportId?: string; transportVersion?: string; transportConfigDigest?: string;
}
export interface ReviewedGwasCatalogRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA; lanes: ReviewedGwasCatalogLane[]; page_size: number; max_pages: number; timeout_ms: number; request_limit: number;
  transport_id: string; transport_version: string; transport_config_digest: string; lane_set_digest: string; retention: typeof RETENTION; credentials: "not_accepted";
}
export interface ReviewedGwasCatalogRetrievalConfigJSON extends ReviewedGwasCatalogRetrievalConfigPayload { config_digest: string; }
export class ReviewedGwasCatalogRetrievalConfig {
  readonly lanes: readonly ReviewedGwasCatalogLane[]; readonly pageSize: number; readonly maxPages: number; readonly timeoutMs: number;
  readonly transportId: string; readonly transportVersion: string; readonly transportConfigDigest: string; readonly laneSetDigest: string;
  constructor(options: ReviewedGwasCatalogRetrievalConfigOptions = {}) {
    const lanes = options.lanes ?? ["gbm", "glioma"];
    if (!Array.isArray(lanes) || lanes.length < 1 || lanes.length > MAX_REVIEWED_GWAS_CATALOG_LANES || lanes.some((lane) => typeof lane !== "string" || !NativeOwn(LANE_DETAILS, lane)) || new NativeSet(lanes).size !== lanes.length) fail("GWAS Catalog lanes contain an unsupported or duplicate lane");
    this.lanes = NativeFreeze((["gbm", "glioma"] as const).filter((lane) => lanes.includes(lane)));
    this.pageSize = integer("GWAS Catalog pageSize", options.pageSize ?? 20, 1, MAX_REVIEWED_GWAS_CATALOG_PAGE_SIZE);
    this.maxPages = integer("GWAS Catalog maxPages", options.maxPages ?? 2, 1, MAX_REVIEWED_GWAS_CATALOG_PAGES);
    this.timeoutMs = integer("GWAS Catalog timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_GWAS_CATALOG_TRANSPORT_ID; this.transportVersion = options.transportVersion ?? BUILTIN_GWAS_CATALOG_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("GWAS Catalog transport identity is invalid");
    this.transportConfigDigest = digest("GWAS Catalog transportConfigDigest", options.transportConfigDigest ?? BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST);
    this.laneSetDigest = digestJsonSync({ diseases: this.lanes.map((lane) => ({ lane, disease_id: LANE_DETAILS[lane].diseaseId, disease_name: LANE_DETAILS[lane].diseaseName })) }); NativeFreeze(this);
  }
  get requestLimit(): number { return this.lanes.length * this.maxPages; }
  private payload(): ReviewedGwasCatalogRetrievalConfigPayload {
    return { schema: REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA, lanes: [...this.lanes], page_size: this.pageSize, max_pages: this.maxPages, timeout_ms: this.timeoutMs, request_limit: this.requestLimit, transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest, lane_set_digest: this.laneSetDigest, retention: RETENTION, credentials: "not_accepted" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedGwasCatalogRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown): ReviewedGwasCatalogRetrievalConfig {
    const raw = exactObject("GWAS Catalog config", value, ["schema", "lanes", "page_size", "max_pages", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "lane_set_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_GWAS_CATALOG_CONFIG_SCHEMA || !Array.isArray(raw.lanes)) fail("GWAS Catalog config has an invalid shape");
    const config = new ReviewedGwasCatalogRetrievalConfig({ lanes: raw.lanes as ReviewedGwasCatalogLane[], pageSize: raw.page_size as number, maxPages: raw.max_pages as number, timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("GWAS Catalog config is not normalized or its digest is invalid"); return config;
  }
}
export interface ReviewedGwasCatalogRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_PLAN_SCHEMA; config: ReviewedGwasCatalogRetrievalConfigJSON; config_digest: string; lane_set_digest: string; request_limit: number;
  maximum_returned_associations: number; minimum_request_interval_ms: number;
  scope: "fixed_glioma_direct_ontology_trait_association_pages"; execution: "bounded_https_get_and_validated_same_origin_pagination_after_literal_approval";
  retention: typeof RETENTION; credentials: "not_accepted"; plan_digest: string;
}
export class ReviewedGwasCatalogRetrievalPlan {
  readonly config: ReviewedGwasCatalogRetrievalConfig; readonly configDigest: string; readonly laneSetDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedGwasCatalogRetrievalConfig, configDigest: string, laneSetDigest: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.laneSetDigest = laneSetDigest; this.planDigest = planDigest; NativeFreeze(this); }
  static create(config: ReviewedGwasCatalogRetrievalConfig): ReviewedGwasCatalogRetrievalPlan {
    if (Object.getPrototypeOf(config) !== ReviewedGwasCatalogRetrievalConfig.prototype) fail("GWAS Catalog plan requires an exact config");
    const fields = { schema: REVIEWED_GWAS_CATALOG_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, lane_set_digest: config.laneSetDigest, request_limit: config.requestLimit, maximum_returned_associations: config.lanes.length * config.pageSize * config.maxPages, minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS, scope: "fixed_glioma_direct_ontology_trait_association_pages", execution: "bounded_https_get_and_validated_same_origin_pagination_after_literal_approval", retention: RETENTION, credentials: "not_accepted" };
    return new ReviewedGwasCatalogRetrievalPlan(config, config.configDigest, config.laneSetDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedGwasCatalogRetrievalPlanJSON { return { schema: REVIEWED_GWAS_CATALOG_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, lane_set_digest: this.laneSetDigest, request_limit: this.config.requestLimit, maximum_returned_associations: this.config.lanes.length * this.config.pageSize * this.config.maxPages, minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS, scope: "fixed_glioma_direct_ontology_trait_association_pages", execution: "bounded_https_get_and_validated_same_origin_pagination_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.planDigest }; }
  validate(): void { const expected = ReviewedGwasCatalogRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.laneSetDigest !== expected.laneSetDigest || this.planDigest !== expected.planDigest) fail("GWAS Catalog plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown): ReviewedGwasCatalogRetrievalPlan {
    const raw = exactObject("GWAS Catalog plan", value, ["schema", "config", "config_digest", "lane_set_digest", "request_limit", "maximum_returned_associations", "minimum_request_interval_ms", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_GWAS_CATALOG_PLAN_SCHEMA) fail("GWAS Catalog plan has an invalid shape");
    const plan = ReviewedGwasCatalogRetrievalPlan.create(ReviewedGwasCatalogRetrievalConfig.fromJSON(raw.config)); if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("GWAS Catalog plan is not normalized or its digest is invalid"); return plan;
  }
}
export type ReviewedGwasCatalogBundle = Record<string, JsonValue>;
export type ReviewedGwasCatalogReceipt = Record<string, JsonValue>;
export type ReviewedGwasCatalogTransient = Record<string, JsonValue>;
export type ReviewedGwasCatalogFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
export interface ReviewedGwasCatalogRetrievalAdapterOptions { fetch?: ReviewedGwasCatalogFetch; }
export class ReviewedGwasCatalogRetrievalResult {
  private readonly bundleValue: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundleValue = NativeJsonParse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = NativeJsonParse(canonicalJson(receipt)) as Record<string, JsonValue>; NativeFreeze(this); }
  get bundle(): ReviewedGwasCatalogBundle { return NativeJsonParse(canonicalJson(this.bundleValue)) as ReviewedGwasCatalogBundle; }
  get receipt(): ReviewedGwasCatalogReceipt { return NativeJsonParse(canonicalJson(this.receiptValue)) as ReviewedGwasCatalogReceipt; }
  toJSON(): Record<string, JsonValue> { return { receipt: this.receipt, retention: "digest_metadata_only" }; }
  toTransientJSON(): ReviewedGwasCatalogTransient { return { schema: REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA, bundle: this.bundle, receipt: this.receipt, retention: "caller_owned_transient_association_metadata" }; }
}
export class ReviewedGwasCatalogRetrievalAdapter {
  readonly config: ReviewedGwasCatalogRetrievalConfig; private readonly fetcher?: ReviewedGwasCatalogFetch;
  constructor(config: ReviewedGwasCatalogRetrievalConfig, options: ReviewedGwasCatalogRetrievalAdapterOptions = {}) {
    if (Object.getPrototypeOf(config) !== ReviewedGwasCatalogRetrievalConfig.prototype || Object.keys(config).length !== 8) fail("GWAS Catalog adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("GWAS Catalog injected transport is malformed");
    if (options.fetch !== undefined && config.transportId === BUILTIN_GWAS_CATALOG_TRANSPORT_ID) fail("GWAS Catalog injected transport requires a distinct reviewed identity");
    if (options.fetch === undefined && (config.transportId !== BUILTIN_GWAS_CATALOG_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_GWAS_CATALOG_TRANSPORT_CONFIG_DIGEST)) fail("GWAS Catalog built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; NativeFreeze(this);
  }
  prepare(): ReviewedGwasCatalogRetrievalPlan { return ReviewedGwasCatalogRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedGwasCatalogRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedGwasCatalogRetrievalResult> {
    if (Object.getPrototypeOf(plan) !== ReviewedGwasCatalogRetrievalPlan.prototype || Object.keys(plan).length !== 4) fail("GWAS Catalog execution requires an exact reviewed plan"); plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("GWAS Catalog execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("GWAS Catalog dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("GWAS Catalog retrievedAt", options.retrievedAt);
    const lanes: Record<string, JsonValue>[] = []; const sources: Record<string, JsonValue>[] = []; const sourceReceipts: Record<string, JsonValue>[] = []; let responseBytes = 0; let requestCount = 0;
    for (const lane of this.config.lanes) {
      const details = LANE_DETAILS[lane]; const associations: Record<string, JsonValue>[] = []; const seen = new NativeSet<number>(); let total: number | undefined; let totalPages: number | undefined; let pagesRequested = 0;
      for (let pageNumber = 0; pageNumber < this.config.maxPages; pageNumber += 1) {
        if (totalPages !== undefined && pageNumber >= totalPages) break;
        const url = queryUrl(details.diseaseId, pageNumber, this.config.pageSize); await paceRequests(); let raw: unknown;
        try { raw = this.fetcher ? await this.fetcher(url, new NativeAbortController().signal) : await builtinFetch(url, this.config.timeoutMs); }
        catch (error) { if (error instanceof ReviewedGwasCatalogRetrievalError) throw error; throw new ReviewedGwasCatalogRetrievalError("GWAS Catalog association request failed"); }
        const parsed = parseResponse(raw); responseBytes += parsed.bytes; requestCount += 1; pagesRequested += 1;
        if (responseBytes > MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES) fail("GWAS Catalog aggregate response bytes exceed the plan bound");
        const page = pageRows(parsed.payload, lane, pageNumber, this.config.pageSize);
        if (total !== undefined && (page.total !== total || page.totalPages !== totalPages)) fail("GWAS Catalog totals changed during paginated retrieval");
        total = page.total; totalPages = page.totalPages;
        for (const rawRow of page.rows) { const row = normalizeRow(rawRow, lane); const associationId = row.association_id as number; if (seen.has(associationId)) fail("GWAS Catalog page prefix contains duplicate association identifiers"); seen.add(associationId); associations.push(row); }
      }
      if (total === undefined || totalPages === undefined) fail("GWAS Catalog retrieval produced no validated first page");
      const expectedReturned = Math.min(total, this.config.pageSize * Math.min(totalPages, this.config.maxPages)); if (associations.length !== expectedReturned) fail("GWAS Catalog retrieved page prefix is incomplete");
      const coverage = totalPages <= this.config.maxPages ? "all_source_rows" : "bounded_page_prefix";
      const laneRecord: Record<string, JsonValue> = { source_id: `gwas_catalog_${lane}`, lane, disease_id: details.diseaseId, disease_name: details.diseaseName, total_associations: total, returned_associations: associations.length, omitted_associations: total - associations.length, page_size: this.config.pageSize, max_pages: this.config.maxPages, pages_requested: pagesRequested, coverage, query_semantics: "exact_ontology_trait_direct_matches_only", associations };
      const laneDigest = digestJsonSync(laneRecord); const queryDigest = digestJsonSync({ endpoint: REVIEWED_GWAS_CATALOG_ENDPOINT, disease_id: details.diseaseId, show_child_traits: false, page_size: this.config.pageSize, max_pages: this.config.maxPages });
      sources.push({ source_id: laneRecord.source_id as string, authority: REVIEWED_GWAS_CATALOG_AUTHORITY, uri: REVIEWED_GWAS_CATALOG_ENDPOINT, query_digest: queryDigest, retrieved_at: retrievedAt, content_sha256: laneDigest, record_count: associations.length, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA, lane, source_id: laneRecord.source_id as string, disease_id: details.diseaseId, content_digest: laneDigest, total_associations: total, returned_associations: associations.length, omitted_associations: total - associations.length, pages_requested: pagesRequested, total_pages: totalPages, coverage }); lanes.push(laneRecord);
    }
    const coverage = lanes.every((row) => row.coverage === "all_source_rows") ? "all_source_rows" : "bounded_page_prefix"; const associationCount = lanes.reduce((sum, row) => sum + (row.returned_associations as number), 0); const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned: Record<string, JsonValue> = { schema: REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, lanes, source_set_digest: sourceSetDigest, lane_count: lanes.length, association_count: associationCount, coverage, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES) fail("GWAS Catalog bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned: Record<string, JsonValue> = { schema: REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, lane_set_digest: plan.laneSetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, lane_count: lanes.length, association_count: associationCount, request_count: requestCount, response_bytes: responseBytes, coverage, retrieved_at: retrievedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    return new ReviewedGwasCatalogRetrievalResult(bundle, { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) });
  }
}
export interface ReviewedGwasCatalogExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true; retrieved_at: string | null; retention: "metadata_only"; credentials: "not_accepted"; metadata_digest: string;
}
export function createReviewedGwasCatalogExecutionMetadata(plan: ReviewedGwasCatalogRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedGwasCatalogExecutionMetadata {
  if (Object.getPrototypeOf(plan) !== ReviewedGwasCatalogRetrievalPlan.prototype) fail("GWAS Catalog execution metadata requires an exact plan"); plan.validate();
  if (approveSourceDispatch !== true) fail("GWAS Catalog execution metadata requires literal approval");
  const payload = { schema: REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("GWAS Catalog retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}
function validateTransient(value: unknown, plan: ReviewedGwasCatalogRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  if (byteLength(canonicalJson(value)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES + 100_000) fail("GWAS Catalog transient value exceeds its byte bound");
  const transient = exactObject("GWAS Catalog transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_GWAS_CATALOG_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_association_metadata") fail("GWAS Catalog transient value is malformed");
  const bundle = exactObject("GWAS Catalog transient bundle", transient.bundle, ["schema", "generated_at", "sources", "lanes", "source_set_digest", "lane_count", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("GWAS Catalog transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "lane_set_digest", "bundle_digest", "source_set_digest", "source_count", "lane_count", "association_count", "request_count", "response_bytes", "coverage", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  if (byteLength(canonicalJson(bundle)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES) fail("GWAS Catalog transient bundle exceeds its byte bound");
  const unsignedBundle = Object.fromEntries(NativeObjectKeys(bundle).filter((key) => key !== "bundle_digest").map((key) => [key, bundle[key]])); const unsignedReceipt = Object.fromEntries(NativeObjectKeys(receipt).filter((key) => key !== "receipt_digest").map((key) => [key, receipt[key]]));
  if (digestJsonSync(unsignedBundle) !== digest("GWAS Catalog bundle digest", bundle.bundle_digest) || digestJsonSync(unsignedReceipt) !== digest("GWAS Catalog receipt digest", receipt.receipt_digest)) fail("GWAS Catalog transient digests are invalid");
  if (bundle.schema !== REVIEWED_GWAS_CATALOG_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_GWAS_CATALOG_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.lane_set_digest !== plan.laneSetDigest) fail("GWAS Catalog transient identity differs from the reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "not_accepted" || receipt.retention !== RETENTION || bundle.provider !== "none" || bundle.credentials !== "not_accepted") fail("GWAS Catalog transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("GWAS Catalog transient limitations are invalid");
  const retrievedAt = timestamp("GWAS Catalog retrievedAt", receipt.retrieved_at); if (timestamp("GWAS Catalog generatedAt", bundle.generated_at) !== retrievedAt) fail("GWAS Catalog bundle and receipt timestamps do not match");
  const lanes = bundle.lanes; const sources = bundle.sources; const sourceReceipts = receipt.source_receipts;
  if (!Array.isArray(lanes) || !Array.isArray(sources) || !Array.isArray(sourceReceipts) || lanes.length !== plan.config.lanes.length || sources.length !== lanes.length || sourceReceipts.length !== lanes.length) fail("GWAS Catalog transient lane coverage is incomplete");
  const sourceSetDigest = digest("GWAS Catalog source_set_digest", bundle.source_set_digest);
  if (digestJsonSync(sources) !== sourceSetDigest || receipt.source_set_digest !== sourceSetDigest || receipt.bundle_digest !== bundle.bundle_digest) fail("GWAS Catalog transient source-set binding is invalid");
  let associationCount = 0; let requestCount = 0; const laneCoverage: string[] = [];
  for (const [index, lane] of plan.config.lanes.entries()) {
    const record = exactObject("GWAS Catalog lane record", lanes[index], ["source_id", "lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "page_size", "max_pages", "pages_requested", "coverage", "query_semantics", "associations"]);
    const details = LANE_DETAILS[lane]; const total = integer("GWAS Catalog total associations", record.total_associations, 0, 50_000_000); const totalPages = total ? Math.ceil(total / plan.config.pageSize) : 0;
    const pagesRequested = totalPages ? Math.min(totalPages, plan.config.maxPages) : 1; const returned = Math.min(total, plan.config.pageSize * plan.config.maxPages); const omitted = total - returned;
    const coverage = totalPages <= plan.config.maxPages ? "all_source_rows" : "bounded_page_prefix"; const rows = record.associations;
    if (!Array.isArray(rows) || rows.length !== returned || record.source_id !== `gwas_catalog_${lane}` || record.lane !== lane || record.disease_id !== details.diseaseId || record.disease_name !== details.diseaseName || record.returned_associations !== returned || record.omitted_associations !== omitted || record.page_size !== plan.config.pageSize || record.max_pages !== plan.config.maxPages || record.pages_requested !== pagesRequested || record.coverage !== coverage || record.query_semantics !== "exact_ontology_trait_direct_matches_only") fail("GWAS Catalog lane projection differs from its reviewed query");
    const seen = new NativeSet<number>();
    for (const rawRow of rows) { const row = validateProjectedRow(rawRow, lane); const id = row.association_id as number; if (seen.has(id)) fail("GWAS Catalog projected lane repeats an association identifier"); seen.add(id); }
    const laneDigest = digestJsonSync(record); const queryDigest = digestJsonSync({ endpoint: REVIEWED_GWAS_CATALOG_ENDPOINT, disease_id: details.diseaseId, show_child_traits: false, page_size: plan.config.pageSize, max_pages: plan.config.maxPages });
    const expectedSource = { source_id: `gwas_catalog_${lane}`, authority: REVIEWED_GWAS_CATALOG_AUTHORITY, uri: REVIEWED_GWAS_CATALOG_ENDPOINT, query_digest: queryDigest, retrieved_at: retrievedAt, content_sha256: laneDigest, record_count: returned, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (canonicalJson(sources[index]) !== canonicalJson(expectedSource)) fail("GWAS Catalog source metadata does not match its lane record");
    const expectedReceipt = { schema: REVIEWED_GWAS_CATALOG_SOURCE_RECEIPT_SCHEMA, lane, source_id: `gwas_catalog_${lane}`, disease_id: details.diseaseId, content_digest: laneDigest, total_associations: total, returned_associations: returned, omitted_associations: omitted, pages_requested: pagesRequested, total_pages: totalPages, coverage };
    if (canonicalJson(sourceReceipts[index]) !== canonicalJson(expectedReceipt)) fail("GWAS Catalog source receipt does not match its lane record");
    associationCount += returned; requestCount += pagesRequested; laneCoverage.push(coverage);
  }
  const coverage = laneCoverage.every((item) => item === "all_source_rows") ? "all_source_rows" : "bounded_page_prefix";
  if (bundle.coverage !== coverage || receipt.coverage !== coverage || integer("GWAS Catalog bundle lane_count", bundle.lane_count, 1, MAX_REVIEWED_GWAS_CATALOG_LANES) !== lanes.length || integer("GWAS Catalog receipt lane_count", receipt.lane_count, 1, MAX_REVIEWED_GWAS_CATALOG_LANES) !== lanes.length || integer("GWAS Catalog source_count", receipt.source_count, 1, MAX_REVIEWED_GWAS_CATALOG_LANES) !== lanes.length || integer("GWAS Catalog bundle association_count", bundle.association_count, 0, plan.config.lanes.length * plan.config.pageSize * plan.config.maxPages) !== associationCount || integer("GWAS Catalog receipt association_count", receipt.association_count, 0, plan.config.lanes.length * plan.config.pageSize * plan.config.maxPages) !== associationCount || integer("GWAS Catalog request_count", receipt.request_count, 1, plan.config.requestLimit) !== requestCount) fail("GWAS Catalog transient counts or coverage are inconsistent");
  integer("GWAS Catalog response_bytes", receipt.response_bytes, requestCount, MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES);
  return { bundle, receipt };
}
export function createReviewedGwasCatalogAutonomousEvidenceRegistration(adapter: ReviewedGwasCatalogRetrievalAdapter, plan: ReviewedGwasCatalogRetrievalPlan, lane: ReviewedGwasCatalogLane): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (Object.getPrototypeOf(adapter) !== ReviewedGwasCatalogRetrievalAdapter.prototype || Object.keys(adapter).length !== 2 || Object.getPrototypeOf(plan) !== ReviewedGwasCatalogRetrievalPlan.prototype) fail("GWAS Catalog registration requires exact adapter and plan values"); plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || !plan.config.lanes.includes(lane) || plan.config.lanes.length !== 1) fail("GWAS Catalog registration requires the exact single-lane plan");
  const frozenPlan = ReviewedGwasCatalogRetrievalPlan.create(plan.config); const sourceId = `gwas_catalog_${lane}`; const expectedNetwork = adapter.config.transportId === BUILTIN_GWAS_CATALOG_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.gwas_catalog.${lane}`, version: REVIEWED_GWAS_CATALOG_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["human_gwas_association_metadata", "source_provenance"], sourceKinds: ["gwas_catalog_direct_trait_association_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request; if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("GWAS Catalog acquisition request does not match its reviewed source");
      const metadata = exactObject("GWAS Catalog execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("GWAS Catalog metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_GWAS_CATALOG_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("GWAS Catalog execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("GWAS Catalog retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("GWAS Catalog evidence projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}

const ANCESTRY_RETENTION = "bounded_transient_study_ancestry_metadata;autonomous_evidence_digest_only" as const;
const ANCESTRY_LIMITATIONS = Object.freeze([
  "the v2 ancestry endpoint returns study-level curated sample descriptors, not participant-level data",
  "number_of_individuals is the source-reported count and does not provide case/control counts or an association-specific sample size",
  "ancestral-group labels and recruitment geography are source descriptors, not a genetic ancestry inference or population-representativeness claim",
  "missing metadata is retained as null and an empty source list is retained as an empty list",
  "only the first bounded ancestry-record prefix is retained in source order, which is not a ranking",
  "the endpoint does not provide pagination; a single complete collection response is requested per explicitly selected study",
  "source records can change and require independent review for applicability, freshness, and study design",
  "ancestry metadata does not establish causality, clinical relevance, or treatment benefit",
  "the adapter does not retrieve patient-level data, full summary statistics, or individual-level genotypes",
  "caller-injected transports own timeout, redirect, network, and credential policy under their declared identity",
]);
export type ReviewedGwasCatalogAncestryFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
function ancestryUrl(accession: string): string { return `${REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/${accession}/ancestries`; }
function ancestryCollection(raw: Record<string, unknown>): Record<string, unknown>[] {
  exactObject("GWAS Catalog ancestry response", raw, ["_embedded"]);
  const embedded = exactObject("GWAS Catalog ancestry collection", raw._embedded, ["ancestries"]);
  if (!Array.isArray(embedded.ancestries) || embedded.ancestries.length > MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY) fail("GWAS Catalog ancestry source rows exceed the structural bound");
  return embedded.ancestries.map((row) => {
    if (typeof row !== "object" || row === null || Array.isArray(row)) fail("GWAS Catalog ancestry item is malformed");
    return row as Record<string, unknown>;
  });
}
function ancestryId(raw: Record<string, unknown>, accession: string): number {
  const links = exactObject("GWAS Catalog ancestry item links", raw._links, ["self"]);
  const self = exactObject("GWAS Catalog ancestry item self link", links.self, ["href"]);
  const href = self.href;
  if (typeof href !== "string" || byteLength(href) > 2_048) fail("GWAS Catalog ancestry item link is invalid");
  let url: URL;
  try { url = new NativeURL(href); } catch { return fail("GWAS Catalog ancestry item link is malformed"); }
  const prefix = `/gwas/rest/api/v2/studies/${accession}/ancestries/`;
  const suffix = url.pathname.startsWith(prefix) ? url.pathname.slice(prefix.length) : "";
  if (url.protocol !== "https:" || url.hostname !== REVIEWED_GWAS_CATALOG_HOST || url.port || url.username || url.password || url.pathname !== `${prefix}${suffix}` || url.search || url.hash || !/^[1-9][0-9]{0,15}$/.test(suffix)) fail("GWAS Catalog ancestry item link left its reviewed study");
  return integer("GWAS Catalog ancestry item identifier", Number(suffix), 1, Number.MAX_SAFE_INTEGER);
}
function ancestryGroups(value: unknown): Array<{ ancestral_group: string | null }> | null {
  if (value === undefined || value === null) return null;
  if (!Array.isArray(value) || value.length > 128) fail("GWAS Catalog ancestral groups are malformed");
  const result = value.map((item) => {
    const row = exactObject("GWAS Catalog ancestral group", item, ["ancestral_group"]);
    return { ancestral_group: row.ancestral_group === null ? null : text("GWAS Catalog ancestral group", row.ancestral_group, 256) };
  });
  if (new NativeSet(result.map((item) => item.ancestral_group)).size !== result.length) fail("GWAS Catalog ancestral groups contain duplicates");
  return result;
}
type AncestryCountry = { major_area: string | null; region: string | null; country_name: string | null };
function ancestryCountries(value: unknown, name: string): AncestryCountry[] | null {
  if (value === undefined || value === null) return null;
  if (!Array.isArray(value) || value.length > 128) fail(`${name} is malformed`);
  const fields = ["major_area", "region", "country_name"] as const;
  const result = value.map((item) => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) fail(`${name} entry is malformed`);
    const row = item as Record<string, unknown>;
    if (NativeObjectKeys(row).some((key) => !fields.includes(key as (typeof fields)[number])) || NativeObjectKeys(row).length === 0) fail(`${name} entry is malformed`);
    const normalized = Object.fromEntries(fields.map((field) => [field, row[field] === undefined || row[field] === null ? null : text(`${name} ${field}`, row[field], 256)])) as AncestryCountry;
    if (!Object.values(normalized).some(Boolean)) fail(`${name} entry has no reported geography`);
    return normalized;
  });
  if (new NativeSet(result.map((item) => canonicalJson(item))).size !== result.length) fail(`${name} contains duplicate rows`);
  return result;
}
function projectAncestry(raw: Record<string, unknown>, accession: string): Record<string, JsonValue> {
  const allowed = ["type", "number_of_individuals", "ancestral_groups", "country_of_origin", "country_of_recruitment", "_links"];
  if (NativeObjectKeys(raw).some((key) => !allowed.includes(key)) || !NativeOwn(raw, "_links")) fail("GWAS Catalog ancestry item has an unexpected shape");
  const stage = raw.type === undefined || raw.type === null ? null : text("GWAS Catalog ancestry stage", raw.type, 64);
  const individuals = raw.number_of_individuals === undefined || raw.number_of_individuals === null ? null : integer("GWAS Catalog number_of_individuals", raw.number_of_individuals, 0, 2_147_483_647);
  return {
    ancestry_id: ancestryId(raw, accession), type: stage, number_of_individuals: individuals,
    ancestral_groups: ancestryGroups(raw.ancestral_groups) as unknown as JsonValue,
    country_of_origin: ancestryCountries(raw.country_of_origin, "GWAS Catalog country_of_origin") as unknown as JsonValue,
    country_of_recruitment: ancestryCountries(raw.country_of_recruitment, "GWAS Catalog country_of_recruitment") as unknown as JsonValue,
  };
}
function validateProjectedAncestry(value: unknown, accession: string): Record<string, JsonValue> {
  const raw = exactObject("GWAS Catalog projected ancestry", value, ["ancestry_id", "type", "number_of_individuals", "ancestral_groups", "country_of_origin", "country_of_recruitment"]);
  const ancestryIdValue = integer("GWAS Catalog projected ancestry_id", raw.ancestry_id, 1, Number.MAX_SAFE_INTEGER);
  const projected = projectAncestry({
    type: raw.type, number_of_individuals: raw.number_of_individuals, ancestral_groups: raw.ancestral_groups,
    country_of_origin: raw.country_of_origin, country_of_recruitment: raw.country_of_recruitment,
    _links: { self: { href: `${REVIEWED_GWAS_CATALOG_STUDIES_ENDPOINT}/${accession}/ancestries/${ancestryIdValue}` } },
  }, accession);
  if (canonicalJson(projected) !== canonicalJson(raw)) fail("GWAS Catalog projected ancestry is not normalized");
  return projected;
}

export interface ReviewedGwasCatalogAncestryRetrievalConfigOptions {
  studyAccessions: readonly string[]; timeoutMs?: number; transportId?: string; transportVersion?: string; transportConfigDigest?: string;
}
export interface ReviewedGwasCatalogAncestryRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_ANCESTRY_CONFIG_SCHEMA; study_accessions: string[]; timeout_ms: number; request_limit: number;
  maximum_studies: number; maximum_returned_ancestry_records_per_study: number; maximum_source_ancestry_records_per_study: number;
  transport_id: string; transport_version: string; transport_config_digest: string;
  study_set_digest: string; retention: typeof ANCESTRY_RETENTION; credentials: "not_accepted";
}
export interface ReviewedGwasCatalogAncestryRetrievalConfigJSON extends ReviewedGwasCatalogAncestryRetrievalConfigPayload { config_digest: string; }
export class ReviewedGwasCatalogAncestryRetrievalConfig {
  readonly studyAccessions: readonly string[]; readonly timeoutMs: number; readonly transportId: string; readonly transportVersion: string;
  readonly transportConfigDigest: string; readonly studySetDigest: string;
  constructor(options: ReviewedGwasCatalogAncestryRetrievalConfigOptions) {
    if (!Array.isArray(options.studyAccessions) || options.studyAccessions.length < 1 || options.studyAccessions.length > MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES) fail("GWAS Catalog study accessions must be a bounded non-empty list");
    const accessions = options.studyAccessions.map((item) => text("GWAS Catalog study accession", item, 32));
    if (accessions.some((item) => !ACCESSION_RE.test(item)) || new NativeSet(accessions).size !== accessions.length) fail("GWAS Catalog study accessions contain an unsupported or duplicate accession");
    this.studyAccessions = NativeFreeze([...accessions].sort());
    this.timeoutMs = integer("GWAS Catalog ancestry timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    const transportId = options.transportId ?? BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_ID;
    const transportVersion = options.transportVersion ?? BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_VERSION;
    if (typeof transportId !== "string" || !IDENTIFIER_RE.test(transportId)) fail("GWAS Catalog ancestry transportId is invalid");
    if (typeof transportVersion !== "string" || !IDENTIFIER_RE.test(transportVersion)) fail("GWAS Catalog ancestry transportVersion is invalid");
    this.transportId = transportId;
    this.transportVersion = transportVersion;
    this.transportConfigDigest = digest("GWAS Catalog ancestry transportConfigDigest", options.transportConfigDigest ?? BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_CONFIG_DIGEST);
    this.studySetDigest = digestJsonSync({ study_accessions: [...this.studyAccessions] }); NativeFreeze(this);
  }
  get requestLimit(): number { return this.studyAccessions.length; }
  private payload(): ReviewedGwasCatalogAncestryRetrievalConfigPayload {
    return { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_CONFIG_SCHEMA, study_accessions: [...this.studyAccessions], timeout_ms: this.timeoutMs,
      request_limit: this.requestLimit, maximum_studies: MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES,
      maximum_returned_ancestry_records_per_study: MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY,
      maximum_source_ancestry_records_per_study: MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY,
      transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest,
      study_set_digest: this.studySetDigest, retention: ANCESTRY_RETENTION, credentials: "not_accepted" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedGwasCatalogAncestryRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown): ReviewedGwasCatalogAncestryRetrievalConfig {
    const raw = exactObject("GWAS Catalog ancestry config", value, ["schema", "study_accessions", "timeout_ms", "request_limit", "maximum_studies", "maximum_returned_ancestry_records_per_study", "maximum_source_ancestry_records_per_study", "transport_id", "transport_version", "transport_config_digest", "study_set_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_CONFIG_SCHEMA || !Array.isArray(raw.study_accessions)) fail("GWAS Catalog ancestry config has an invalid shape");
    const config = new ReviewedGwasCatalogAncestryRetrievalConfig({ studyAccessions: raw.study_accessions as string[], timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("GWAS Catalog ancestry config is not normalized or its digest is invalid"); return config;
  }
}
export interface ReviewedGwasCatalogAncestryRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_ANCESTRY_PLAN_SCHEMA; config: ReviewedGwasCatalogAncestryRetrievalConfigJSON; config_digest: string; study_set_digest: string;
  request_limit: number; maximum_returned_ancestry_records: number; maximum_response_bytes_per_request: number; maximum_aggregate_response_bytes: number;
  maximum_bundle_bytes: number; maximum_response_tree_depth: number; maximum_response_tree_nodes: number; maximum_source_ancestry_records_per_study: number; minimum_request_interval_ms: number;
  scope: "explicit_gwas_catalog_study_ancestries"; execution: "one_bounded_https_collection_request_per_explicit_study_after_literal_approval";
  retention: typeof ANCESTRY_RETENTION; credentials: "not_accepted"; plan_digest: string;
}
export class ReviewedGwasCatalogAncestryRetrievalPlan {
  readonly config: ReviewedGwasCatalogAncestryRetrievalConfig; readonly configDigest: string; readonly studySetDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedGwasCatalogAncestryRetrievalConfig, configDigest: string, studySetDigest: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.studySetDigest = studySetDigest; this.planDigest = planDigest; NativeFreeze(this); }
  static create(config: ReviewedGwasCatalogAncestryRetrievalConfig): ReviewedGwasCatalogAncestryRetrievalPlan {
    if (Object.getPrototypeOf(config) !== ReviewedGwasCatalogAncestryRetrievalConfig.prototype) fail("GWAS Catalog ancestry plan requires an exact config");
    const fields = { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, study_set_digest: config.studySetDigest,
      request_limit: config.requestLimit, maximum_returned_ancestry_records: config.requestLimit * MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY,
      maximum_response_bytes_per_request: MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES, maximum_aggregate_response_bytes: MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES,
      maximum_bundle_bytes: MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES, maximum_response_tree_depth: MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH,
      maximum_response_tree_nodes: MAX_REVIEWED_GWAS_CATALOG_TREE_NODES, maximum_source_ancestry_records_per_study: MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY,
      minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS, scope: "explicit_gwas_catalog_study_ancestries" as const,
      execution: "one_bounded_https_collection_request_per_explicit_study_after_literal_approval" as const, retention: ANCESTRY_RETENTION, credentials: "not_accepted" as const };
    return new ReviewedGwasCatalogAncestryRetrievalPlan(config, config.configDigest, config.studySetDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedGwasCatalogAncestryRetrievalPlanJSON {
    return { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, study_set_digest: this.studySetDigest,
      request_limit: this.config.requestLimit, maximum_returned_ancestry_records: this.config.requestLimit * MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY,
      maximum_response_bytes_per_request: MAX_REVIEWED_GWAS_CATALOG_RESPONSE_BYTES, maximum_aggregate_response_bytes: MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES,
      maximum_bundle_bytes: MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES, maximum_response_tree_depth: MAX_REVIEWED_GWAS_CATALOG_TREE_DEPTH,
      maximum_response_tree_nodes: MAX_REVIEWED_GWAS_CATALOG_TREE_NODES, maximum_source_ancestry_records_per_study: MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY,
      minimum_request_interval_ms: REVIEWED_GWAS_CATALOG_MIN_REQUEST_INTERVAL_MS, scope: "explicit_gwas_catalog_study_ancestries",
      execution: "one_bounded_https_collection_request_per_explicit_study_after_literal_approval", retention: ANCESTRY_RETENTION, credentials: "not_accepted", plan_digest: this.planDigest };
  }
  validate(): void { const expected = ReviewedGwasCatalogAncestryRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.studySetDigest !== expected.studySetDigest || this.planDigest !== expected.planDigest) fail("GWAS Catalog ancestry plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown): ReviewedGwasCatalogAncestryRetrievalPlan {
    const raw = exactObject("GWAS Catalog ancestry plan", value, ["schema", "config", "config_digest", "study_set_digest", "request_limit", "maximum_returned_ancestry_records", "maximum_response_bytes_per_request", "maximum_aggregate_response_bytes", "maximum_bundle_bytes", "maximum_response_tree_depth", "maximum_response_tree_nodes", "maximum_source_ancestry_records_per_study", "minimum_request_interval_ms", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_PLAN_SCHEMA) fail("GWAS Catalog ancestry plan has an invalid shape");
    const plan = ReviewedGwasCatalogAncestryRetrievalPlan.create(ReviewedGwasCatalogAncestryRetrievalConfig.fromJSON(raw.config)); if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("GWAS Catalog ancestry plan is not normalized or its digest is invalid"); return plan;
  }
}
export type ReviewedGwasCatalogAncestryBundle = Record<string, JsonValue>;
export type ReviewedGwasCatalogAncestryReceipt = Record<string, JsonValue>;
export type ReviewedGwasCatalogAncestryTransient = Record<string, JsonValue>;
export interface ReviewedGwasCatalogAncestryRetrievalAdapterOptions { fetch?: ReviewedGwasCatalogAncestryFetch; }
async function ancestryInjectedFetch(fetcher: ReviewedGwasCatalogAncestryFetch, url: string, timeoutMs: number): Promise<unknown> {
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedGwasCatalogRetrievalError("GWAS Catalog ancestry request timed out")); }, timeoutMs); });
  try { return await NativePromiseRace([Promise.resolve().then(() => fetcher(url, controller.signal)), timeout]); }
  catch (error) { if (error instanceof ReviewedGwasCatalogRetrievalError) throw error; throw new ReviewedGwasCatalogRetrievalError("GWAS Catalog ancestry request failed"); }
  finally { if (timer !== undefined) NativeClearTimeout(timer); }
}
export class ReviewedGwasCatalogAncestryRetrievalResult {
  private readonly bundleValue: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundleValue = NativeJsonParse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = NativeJsonParse(canonicalJson(receipt)) as Record<string, JsonValue>; NativeFreeze(this); }
  get bundle(): ReviewedGwasCatalogAncestryBundle { return NativeJsonParse(canonicalJson(this.bundleValue)) as ReviewedGwasCatalogAncestryBundle; }
  get receipt(): ReviewedGwasCatalogAncestryReceipt { return NativeJsonParse(canonicalJson(this.receiptValue)) as ReviewedGwasCatalogAncestryReceipt; }
  toJSON(): Record<string, JsonValue> { return { receipt: this.receipt, retention: "digest_metadata_only" }; }
  toTransientJSON(): ReviewedGwasCatalogAncestryTransient { return { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_TRANSIENT_SCHEMA, bundle: this.bundle, receipt: this.receipt, retention: "caller_owned_transient_study_ancestry_metadata" }; }
}
export class ReviewedGwasCatalogAncestryRetrievalAdapter {
  readonly config: ReviewedGwasCatalogAncestryRetrievalConfig; private readonly fetcher?: ReviewedGwasCatalogAncestryFetch;
  constructor(config: ReviewedGwasCatalogAncestryRetrievalConfig, options: ReviewedGwasCatalogAncestryRetrievalAdapterOptions = {}) {
    if (Object.getPrototypeOf(config) !== ReviewedGwasCatalogAncestryRetrievalConfig.prototype || Object.keys(config).length !== 6) fail("GWAS Catalog ancestry adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("GWAS Catalog ancestry injected transport is malformed");
    if (options.fetch !== undefined && config.transportId === BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_ID) fail("GWAS Catalog ancestry injected transport requires a distinct reviewed identity");
    if (options.fetch === undefined && (config.transportId !== BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_CONFIG_DIGEST)) fail("GWAS Catalog ancestry built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; NativeFreeze(this);
  }
  prepare(): ReviewedGwasCatalogAncestryRetrievalPlan { return ReviewedGwasCatalogAncestryRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedGwasCatalogAncestryRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedGwasCatalogAncestryRetrievalResult> {
    if (Object.getPrototypeOf(plan) !== ReviewedGwasCatalogAncestryRetrievalPlan.prototype || Object.keys(plan).length !== 4) fail("GWAS Catalog ancestry execution requires an exact reviewed plan"); plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("GWAS Catalog ancestry execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("GWAS Catalog ancestry dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("GWAS Catalog ancestry retrievedAt", options.retrievedAt);
    const studies: Record<string, JsonValue>[] = []; const sources: Record<string, JsonValue>[] = []; const sourceReceipts: Record<string, JsonValue>[] = []; let responseBytes = 0;
    for (const accession of this.config.studyAccessions) {
      const url = ancestryUrl(accession); await paceRequests(); let raw: unknown;
      try { raw = this.fetcher ? await ancestryInjectedFetch(this.fetcher, url, this.config.timeoutMs) : await builtinFetch(url, this.config.timeoutMs); }
      catch (error) { if (error instanceof ReviewedGwasCatalogRetrievalError) throw error; throw new ReviewedGwasCatalogRetrievalError("GWAS Catalog ancestry request failed"); }
      const parsed = parseResponse(raw); responseBytes += parsed.bytes; if (responseBytes > MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES) fail("GWAS Catalog ancestry aggregate response bytes exceed the plan bound");
      const rawRows = ancestryCollection(parsed.payload); const seen = new NativeSet<number>(); const ancestries: Record<string, JsonValue>[] = [];
      for (const [index, rawRow] of rawRows.entries()) { const row = projectAncestry(rawRow, accession); const id = row.ancestry_id as number; if (seen.has(id)) fail("GWAS Catalog ancestry response contains duplicate item identifiers"); seen.add(id); if (index < MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY) ancestries.push(row); }
      const total = rawRows.length; const omitted = total - ancestries.length; const coverage = omitted === 0 ? "all_source_rows" : "bounded_source_order_prefix"; const sourceId = `gwas_catalog_ancestry_${accession}`;
      const studyRecord: Record<string, JsonValue> = { source_id: sourceId, study_accession: accession, total_ancestry_records: total, returned_ancestry_records: ancestries.length, omitted_ancestry_records: omitted, coverage, query_semantics: "complete_study_ancestry_collection_without_sort_order", ancestries };
      const recordDigest = digestJsonSync(studyRecord); const queryDigest = digestJsonSync({ endpoint: url, method: "GET", pagination: "none_one_complete_collection_response" });
      sources.push({ source_id: sourceId, authority: REVIEWED_GWAS_CATALOG_AUTHORITY, uri: url, query_digest: queryDigest, retrieved_at: retrievedAt, content_sha256: recordDigest, record_count: ancestries.length, provider: "none", credentials: "not_accepted", limitations: [...ANCESTRY_LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECEIPT_SCHEMA, study_accession: accession, source_id: sourceId, content_digest: recordDigest, total_ancestry_records: total, returned_ancestry_records: ancestries.length, omitted_ancestry_records: omitted, coverage }); studies.push(studyRecord);
    }
    const coverage = studies.every((row) => row.coverage === "all_source_rows") ? "all_source_rows" : "bounded_source_order_prefix"; const ancestryCount = studies.reduce((sum, row) => sum + (row.returned_ancestry_records as number), 0); const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned: Record<string, JsonValue> = { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, studies, source_set_digest: sourceSetDigest, study_count: studies.length, ancestry_count: ancestryCount, coverage, provider: "none", credentials: "not_accepted", limitations: [...ANCESTRY_LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES) fail("GWAS Catalog ancestry bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned: Record<string, JsonValue> = { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, study_set_digest: plan.studySetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, study_count: studies.length, ancestry_count: ancestryCount, request_count: studies.length, response_bytes: responseBytes, coverage, retrieved_at: retrievedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: ANCESTRY_RETENTION, credentials: "not_accepted", limitations: [...ANCESTRY_LIMITATIONS] };
    return new ReviewedGwasCatalogAncestryRetrievalResult(bundle, { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) });
  }
}
export interface ReviewedGwasCatalogAncestryExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GWAS_CATALOG_ANCESTRY_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true;
  retrieved_at: string | null; retention: "metadata_only"; credentials: "not_accepted"; metadata_digest: string;
}
export function createReviewedGwasCatalogAncestryExecutionMetadata(plan: ReviewedGwasCatalogAncestryRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedGwasCatalogAncestryExecutionMetadata {
  if (Object.getPrototypeOf(plan) !== ReviewedGwasCatalogAncestryRetrievalPlan.prototype) fail("GWAS Catalog ancestry execution metadata requires an exact plan"); plan.validate();
  if (approveSourceDispatch !== true) fail("GWAS Catalog ancestry execution metadata requires literal approval");
  const payload = { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const,
    retrieved_at: retrievedAt === undefined ? null : timestamp("GWAS Catalog ancestry retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateAncestryTransient(value: unknown, plan: ReviewedGwasCatalogAncestryRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  if (byteLength(canonicalJson(value)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES + 100_000) fail("GWAS Catalog ancestry transient value exceeds its byte bound");
  const transient = exactObject("GWAS Catalog ancestry transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_study_ancestry_metadata") fail("GWAS Catalog ancestry transient value is malformed");
  const bundle = exactObject("GWAS Catalog ancestry transient bundle", transient.bundle, ["schema", "generated_at", "sources", "studies", "source_set_digest", "study_count", "ancestry_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("GWAS Catalog ancestry transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "study_set_digest", "bundle_digest", "source_set_digest", "source_count", "study_count", "ancestry_count", "request_count", "response_bytes", "coverage", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  if (byteLength(canonicalJson(bundle)) > MAX_REVIEWED_GWAS_CATALOG_BUNDLE_BYTES) fail("GWAS Catalog ancestry transient bundle exceeds its byte bound");
  const unsignedBundle = Object.fromEntries(NativeObjectKeys(bundle).filter((key) => key !== "bundle_digest").map((key) => [key, bundle[key]])); const unsignedReceipt = Object.fromEntries(NativeObjectKeys(receipt).filter((key) => key !== "receipt_digest").map((key) => [key, receipt[key]]));
  if (digestJsonSync(unsignedBundle) !== digest("GWAS Catalog ancestry bundle digest", bundle.bundle_digest) || digestJsonSync(unsignedReceipt) !== digest("GWAS Catalog ancestry receipt digest", receipt.receipt_digest)) fail("GWAS Catalog ancestry transient digests are invalid");
  if (bundle.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.study_set_digest !== plan.studySetDigest) fail("GWAS Catalog ancestry transient identity differs from the reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "not_accepted" || receipt.retention !== ANCESTRY_RETENTION || bundle.provider !== "none" || bundle.credentials !== "not_accepted") fail("GWAS Catalog ancestry transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(ANCESTRY_LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(ANCESTRY_LIMITATIONS)) fail("GWAS Catalog ancestry transient limitations are invalid");
  const retrievedAt = timestamp("GWAS Catalog ancestry retrievedAt", receipt.retrieved_at); if (timestamp("GWAS Catalog ancestry generatedAt", bundle.generated_at) !== retrievedAt) fail("GWAS Catalog ancestry timestamps do not match");
  const studies = bundle.studies; const sources = bundle.sources; const sourceReceipts = receipt.source_receipts;
  if (!Array.isArray(studies) || !Array.isArray(sources) || !Array.isArray(sourceReceipts) || studies.length !== plan.config.studyAccessions.length || sources.length !== studies.length || sourceReceipts.length !== studies.length) fail("GWAS Catalog ancestry transient study coverage is incomplete");
  const sourceSetDigest = digest("GWAS Catalog ancestry source_set_digest", bundle.source_set_digest);
  if (digestJsonSync(sources) !== sourceSetDigest || receipt.source_set_digest !== sourceSetDigest || receipt.bundle_digest !== bundle.bundle_digest) fail("GWAS Catalog ancestry transient source-set binding is invalid");
  let ancestryCount = 0; const coverageRows: string[] = [];
  for (const [index, accession] of plan.config.studyAccessions.entries()) {
    const record = exactObject("GWAS Catalog ancestry study record", studies[index], ["source_id", "study_accession", "total_ancestry_records", "returned_ancestry_records", "omitted_ancestry_records", "coverage", "query_semantics", "ancestries"]);
    const total = integer("GWAS Catalog total ancestry records", record.total_ancestry_records, 0, MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECORDS_PER_STUDY); const returned = Math.min(total, MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY); const omitted = total - returned;
    const coverage = omitted === 0 ? "all_source_rows" : "bounded_source_order_prefix"; const sourceId = `gwas_catalog_ancestry_${accession}`; const rows = record.ancestries;
    if (!Array.isArray(rows) || rows.length !== returned || record.source_id !== sourceId || record.study_accession !== accession || record.returned_ancestry_records !== returned || record.omitted_ancestry_records !== omitted || record.coverage !== coverage || record.query_semantics !== "complete_study_ancestry_collection_without_sort_order") fail("GWAS Catalog ancestry projection differs from its reviewed study");
    const seen = new NativeSet<number>(); for (const rawRow of rows) { const row = validateProjectedAncestry(rawRow, accession); const id = row.ancestry_id as number; if (seen.has(id)) fail("GWAS Catalog projected ancestry repeats an item identifier"); seen.add(id); }
    const endpoint = ancestryUrl(accession); const recordDigest = digestJsonSync(record); const queryDigest = digestJsonSync({ endpoint, method: "GET", pagination: "none_one_complete_collection_response" });
    const expectedSource = { source_id: sourceId, authority: REVIEWED_GWAS_CATALOG_AUTHORITY, uri: endpoint, query_digest: queryDigest, retrieved_at: retrievedAt, content_sha256: recordDigest, record_count: returned, provider: "none", credentials: "not_accepted", limitations: [...ANCESTRY_LIMITATIONS] };
    if (canonicalJson(sources[index]) !== canonicalJson(expectedSource)) fail("GWAS Catalog ancestry source metadata does not match its study record");
    const expectedReceipt = { schema: REVIEWED_GWAS_CATALOG_ANCESTRY_SOURCE_RECEIPT_SCHEMA, study_accession: accession, source_id: sourceId, content_digest: recordDigest, total_ancestry_records: total, returned_ancestry_records: returned, omitted_ancestry_records: omitted, coverage };
    if (canonicalJson(sourceReceipts[index]) !== canonicalJson(expectedReceipt)) fail("GWAS Catalog ancestry source receipt does not match its study record");
    ancestryCount += returned; coverageRows.push(coverage);
  }
  const coverage = coverageRows.every((item) => item === "all_source_rows") ? "all_source_rows" : "bounded_source_order_prefix"; const studyCount = plan.config.studyAccessions.length;
  if (bundle.coverage !== coverage || receipt.coverage !== coverage || integer("GWAS Catalog ancestry bundle study_count", bundle.study_count, 1, MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES) !== studyCount || integer("GWAS Catalog ancestry receipt study_count", receipt.study_count, 1, MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES) !== studyCount || integer("GWAS Catalog ancestry source_count", receipt.source_count, 1, MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_STUDIES) !== studyCount || integer("GWAS Catalog ancestry count", bundle.ancestry_count, 0, studyCount * MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY) !== ancestryCount || integer("GWAS Catalog ancestry receipt count", receipt.ancestry_count, 0, studyCount * MAX_REVIEWED_GWAS_CATALOG_ANCESTRY_RECORDS_PER_STUDY) !== ancestryCount || integer("GWAS Catalog ancestry request_count", receipt.request_count, studyCount, studyCount) !== studyCount) fail("GWAS Catalog ancestry transient counts or coverage are inconsistent");
  integer("GWAS Catalog ancestry response_bytes", receipt.response_bytes, studyCount, MAX_REVIEWED_GWAS_CATALOG_TOTAL_RESPONSE_BYTES); return { bundle, receipt };
}
export function createReviewedGwasCatalogAncestryAutonomousEvidenceRegistration(adapter: ReviewedGwasCatalogAncestryRetrievalAdapter, plan: ReviewedGwasCatalogAncestryRetrievalPlan, studyAccession: string): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (Object.getPrototypeOf(adapter) !== ReviewedGwasCatalogAncestryRetrievalAdapter.prototype || Object.keys(adapter).length !== 2 || Object.getPrototypeOf(plan) !== ReviewedGwasCatalogAncestryRetrievalPlan.prototype) fail("GWAS Catalog ancestry registration requires exact adapter and plan values"); plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || plan.config.studyAccessions.length !== 1 || plan.config.studyAccessions[0] !== studyAccession) fail("GWAS Catalog ancestry registration requires the exact single-study plan");
  const frozenPlan = ReviewedGwasCatalogAncestryRetrievalPlan.create(plan.config); const sourceId = `gwas_catalog_ancestry_${studyAccession}`; const expectedNetwork = adapter.config.transportId === BUILTIN_GWAS_CATALOG_ANCESTRY_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.gwas_catalog.ancestry.${studyAccession}`, version: REVIEWED_GWAS_CATALOG_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["human_gwas_study_ancestry_metadata", "source_provenance"], sourceKinds: ["gwas_catalog_study_ancestry_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request; if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("GWAS Catalog ancestry acquisition request does not match its reviewed source");
      const metadata = exactObject("GWAS Catalog ancestry execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("GWAS Catalog ancestry metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_GWAS_CATALOG_ANCESTRY_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("GWAS Catalog ancestry execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("GWAS Catalog ancestry retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateAncestryTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("GWAS Catalog ancestry evidence projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...ANCESTRY_LIMITATIONS] }];
    },
  };
}
