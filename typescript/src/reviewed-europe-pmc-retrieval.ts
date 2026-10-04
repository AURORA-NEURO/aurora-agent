/** Reviewed, bounded Europe PMC publication-metadata retrieval for autonomous evidence. */

import { ArgumentError } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

export const REVIEWED_EUROPE_PMC_CONFIG_SCHEMA = "bioprism-reviewed-europe-pmc-config/0.1" as const;
export const REVIEWED_EUROPE_PMC_PLAN_SCHEMA = "bioprism-reviewed-europe-pmc-plan/0.1" as const;
export const REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-europe-pmc-source-receipt/0.1" as const;
export const REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA = "bioprism-reviewed-europe-pmc-bundle/0.1" as const;
export const REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA = "bioprism-reviewed-europe-pmc-receipt/0.1" as const;
export const REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA = "bioprism-reviewed-europe-pmc-transient/0.1" as const;
export const REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-europe-pmc-execution-metadata/0.1" as const;
export const REVIEWED_EUROPE_PMC_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_EUROPE_PMC_HOST = "www.ebi.ac.uk" as const;
export const REVIEWED_EUROPE_PMC_PATH = "/europepmc/webservices/rest/search" as const;
export const REVIEWED_EUROPE_PMC_ENDPOINT = "https://www.ebi.ac.uk/europepmc/webservices/rest/search" as const;
export const REVIEWED_EUROPE_PMC_AUTHORITY = "Europe PMC / EMBL-EBI" as const;
export const REVIEWED_EUROPE_PMC_LANES = Object.freeze({
  glioma: '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma")',
  cranial_base: '(("skull base" OR "cranial base" OR petroclival OR "cavernous sinus") AND neurosurgery)',
  craniosynostosis: '(craniosynostosis OR scaphocephaly OR plagiocephaly OR "Apert syndrome" OR "Crouzon syndrome")',
  encephalocele: '(encephalocele OR meningoencephalocele OR "basal encephalocele" OR "occipital encephalocele")',
  spina_bifida: '("spina bifida" OR "spinal dysraphism" OR myelomeningocele OR lipomeningocele OR "tethered cord")',
  chiari_malformation: '("Chiari malformation" OR syringomyelia OR "craniocervical junction" OR "CSF flow")',
} as const);
export type ReviewedEuropePmcLane = keyof typeof REVIEWED_EUROPE_PMC_LANES;
export const MAX_REVIEWED_EUROPE_PMC_PAGE_SIZE = 100;
export const MAX_REVIEWED_EUROPE_PMC_PAGES = 4;
export const MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES = 1_000_000;
export const MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES = 24_000_000;
export const MAX_REVIEWED_EUROPE_PMC_BUNDLE_BYTES = 12_000_000;
export const MAX_REVIEWED_EUROPE_PMC_TREE_DEPTH = 40;
export const MAX_REVIEWED_EUROPE_PMC_TREE_NODES = 100_000;
export const BUILTIN_EUROPE_PMC_TRANSPORT_ID = "builtin.europepmc.fetch" as const;
export const BUILTIN_EUROPE_PMC_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_EUROPE_PMC_HOST,
  path: REVIEWED_EUROPE_PMC_PATH, method: "GET", format: "json", result_type: "lite",
  synonym_expansion: false, redirects: "refused", credentials: "not_accepted",
});

const RETENTION = "metadata_only;publication_values_and_cursor_marks_transient" as const;
const LIMITATIONS = Object.freeze([
  "Europe PMC lite results are bibliographic metadata, not abstracts or scientific conclusions",
  "fixed specialty keyword queries are bounded discovery aids and do not establish exhaustive coverage",
  "a live search index may change between pages; inconsistent hit counts are reported as unknown",
  "partial results require a later reviewed expansion before any completeness claim",
  "source records require independent review for study quality, omissions, freshness, and applicability",
  "caller-injected transports must enforce timeout, redirect, and network policy under their declared identity",
] as const);
const DIGEST_RE = /^[0-9a-f]{64}$/;
const IDENTIFIER_RE = /^[A-Za-z0-9_.:-]{1,128}$/;
const CONTROL_RE = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/;
const PYTHON_WHITESPACE_RE = /[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu;
const PMID_RE = /^[0-9]{1,12}$/;
const PMCID_RE = /^PMC[0-9]{1,12}$/i;
const DOI_RE = /^10\.[0-9]{4,9}\/[^\s]{1,256}$/i;
const DATE_RE = /^[0-9]{4}(?:-[0-9]{2}(?:-[0-9]{2})?)?$/;
const NativeFetch = globalThis.fetch;
const NativeTextEncoder = globalThis.TextEncoder;
const NativeTextDecoder = globalThis.TextDecoder;
const NativeURL = globalThis.URL;
const NativeAbortController = globalThis.AbortController;
const NativeDate = globalThis.Date;
const nativeEncode = new NativeTextEncoder().encode.bind(new NativeTextEncoder());
const nativeEncodeURIComponent = globalThis.encodeURIComponent.bind(globalThis);
const nativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const nativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const nativeJsonParse = globalThis.JSON.parse.bind(globalThis.JSON);
const nativeJsonStringify = globalThis.JSON.stringify.bind(globalThis.JSON);
const nativeObjectKeys = globalThis.Object.keys.bind(globalThis.Object);
const nativeObjectValues = globalThis.Object.values.bind(globalThis.Object);
const nativeOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty) as (value: object, key: PropertyKey) => boolean;
const nativeObjectFreeze = globalThis.Object.freeze.bind(globalThis.Object);
const nativePromiseRace = globalThis.Promise.race.bind(globalThis.Promise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;
const nativeIsFinite = globalThis.Number.isFinite.bind(globalThis.Number);
const nativeIsInteger = globalThis.Number.isInteger.bind(globalThis.Number);
const nativeIsSafeInteger = globalThis.Number.isSafeInteger.bind(globalThis.Number);
const nativeDateParse = globalThis.Date.parse.bind(globalThis.Date);

export class ReviewedEuropePmcRetrievalError extends ArgumentError {
  override readonly name = "ReviewedEuropePmcRetrievalError";
}

function fail(message: string): never { throw new ReviewedEuropePmcRetrievalError(message); }
function byteLength(value: string): number { return nativeEncode(value).byteLength; }
function validateUnicode(value: string): void {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) fail("Europe PMC response contains invalid Unicode");
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) fail("Europe PMC response contains invalid Unicode");
  }
}
function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST_RE.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value as string;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!nativeIsSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} must be an integer between ${minimum} and ${maximum}`);
  return value as number;
}
function text(name: string, value: unknown, maximum: number, optional = false): string | null {
  if (value === null || value === undefined) { if (optional) return null; fail(`${name} must be text`); }
  if (typeof value !== "string") fail(`${name} must be text`);
  validateUnicode(value as string);
  const normalized = (value as string).replace(PYTHON_WHITESPACE_RE, " ").replace(/^ | $/g, "");
  if (!normalized && optional) return null;
  if (!normalized || CONTROL_RE.test(normalized) || byteLength(normalized) > maximum) fail(`${name} is outside its text bound`);
  return normalized;
}
function timestamp(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)) fail(`${name} must be a UTC timestamp with second precision`);
  const date = new NativeDate(value);
  if (!nativeIsFinite(date.getTime()) || date.toISOString().replace(".000Z", "Z") !== value) fail(`${name} is not a valid UTC timestamp`);
  return value;
}
function now(): string { return new NativeDate().toISOString().replace(/\.\d{3}Z$/, "Z"); }
function exactObject(name: string, value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(`${name} must be an object`);
  const actual = nativeObjectKeys(value);
  if (actual.length !== keys.length || keys.some((key) => !nativeOwn(value, key))) fail(`${name} has an invalid shape`);
  return value as Record<string, unknown>;
}
function normalizeLanes(value: readonly string[]): ReviewedEuropePmcLane[] {
  const allowed = Object.keys(REVIEWED_EUROPE_PMC_LANES) as ReviewedEuropePmcLane[];
  if (!Array.isArray(value) || value.length < 1 || value.length > allowed.length || value.some((lane) => typeof lane !== "string" || !allowed.includes(lane as ReviewedEuropePmcLane)) || new Set(value).size !== value.length) fail("Europe PMC lanes contain an unsupported or duplicate entry");
  return value.map((lane) => lane as ReviewedEuropePmcLane);
}
function queryDigest(lanes: readonly ReviewedEuropePmcLane[]): string {
  return digestJsonSync({ schema: "bioprism-reviewed-europe-pmc-query-set/0.1", queries: lanes.map((lane) => ({ lane, query: REVIEWED_EUROPE_PMC_LANES[lane] })), format: "json", result_type: "lite", synonym_expansion: false });
}
function validJsonTree(root: unknown): void {
  const pending: { value: unknown; depth: number }[] = [{ value: root, depth: 0 }];
  let nodes = 0;
  while (pending.length > 0) {
    const item = pending.pop()!;
    nodes += 1;
    if (nodes > MAX_REVIEWED_EUROPE_PMC_TREE_NODES || item.depth > MAX_REVIEWED_EUROPE_PMC_TREE_DEPTH) fail("Europe PMC response exceeds its JSON tree bound");
    const current = item.value;
    if (typeof current === "string") validateUnicode(current);
    if (typeof current === "number" && (!nativeIsFinite(current) || (nativeIsInteger(current) && !nativeIsSafeInteger(current)))) fail("Europe PMC response contains an invalid number");
    if (Array.isArray(current)) for (const child of current) pending.push({ value: child, depth: item.depth + 1 });
    else if (typeof current === "object" && current !== null) {
      for (const key of nativeObjectKeys(current)) validateUnicode(key);
      for (const child of nativeObjectValues(current)) pending.push({ value: child, depth: item.depth + 1 });
    }
  }
}

/** Scan JSON syntax before JSON.parse so duplicate names cannot be silently overwritten. */
function parseStrictJson(source: string): unknown {
  let index = 0;
  let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index;
    if (source[index] !== "\"") fail("Europe PMC response is not valid JSON");
    index += 1;
    while (index < source.length) {
      const character = source[index]!;
      if (character === "\"") {
        index += 1;
        try { return nativeJsonParse(source.slice(start, index)) as string; } catch { return fail("Europe PMC response is not valid JSON"); }
      }
      if (character === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 0x20) fail("Europe PMC response is not valid JSON");
      index += 1;
    }
    return fail("Europe PMC response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1;
    if (nodes > MAX_REVIEWED_EUROPE_PMC_TREE_NODES || depth > MAX_REVIEWED_EUROPE_PMC_TREE_DEPTH) fail("Europe PMC response exceeds its JSON tree bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace();
      const keys = new Set<string>();
      if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string();
        if (keys.has(key)) fail("Europe PMC response contains a duplicate JSON field");
        keys.add(key); whitespace();
        if (source[index] !== ":") fail("Europe PMC response is not valid JSON");
        index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; }
        if (source[index] !== ",") fail("Europe PMC response is not valid JSON");
        index += 1;
      }
      return fail("Europe PMC response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace();
      if (source[index] === "]") { index += 1; return; }
      while (index < source.length) {
        scan(depth + 1); whitespace();
        if (source[index] === "]") { index += 1; return; }
        if (source[index] !== ",") fail("Europe PMC response is not valid JSON");
        index += 1;
      }
      return fail("Europe PMC response is not valid JSON");
    }
    const start = index;
    while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (index === start) fail("Europe PMC response is not valid JSON");
    try { nativeJsonParse(source.slice(start, index)); } catch { fail("Europe PMC response is not valid JSON"); }
  };
  scan(0); whitespace();
  if (index !== source.length) fail("Europe PMC response is not valid JSON");
  const parsed = nativeJsonParse(source) as unknown;
  validJsonTree(parsed);
  return parsed;
}

function parseResponse(value: unknown): { payload: Record<string, unknown>; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) {
    try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("Europe PMC response is not valid UTF-8"); }
  } else if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    try { body = nativeJsonStringify(value) as string; } catch { return fail("Europe PMC injected response is not JSON"); }
  } else return fail("Europe PMC transport returned an unsupported response type");
  const size = byteLength(body);
  if (size > MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES) fail("Europe PMC response exceeds its byte bound");
  const payload = parseStrictJson(body);
  if (typeof payload !== "object" || payload === null || Array.isArray(payload)) fail("Europe PMC response root must be an object");
  return { payload: payload as Record<string, unknown>, bytes: size };
}

function normalizePublication(raw: unknown, sourceId: string): Record<string, JsonValue> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("Europe PMC result entry is not an object");
  const value = raw as Record<string, unknown>;
  const recordId = text("Europe PMC record id", value.id, 32)!;
  const recordSource = text("Europe PMC record source", value.source, 16)!;
  if (!IDENTIFIER_RE.test(recordId) || !IDENTIFIER_RE.test(recordSource)) fail("Europe PMC record identity is invalid");
  let pmid = text("Europe PMC PMID", value.pmid, 12, true);
  if (pmid !== null && !PMID_RE.test(pmid)) fail("Europe PMC PMID is invalid");
  let pmcid = text("Europe PMC PMCID", value.pmcid, 16, true);
  if (pmcid !== null) { pmcid = pmcid.toUpperCase(); if (!PMCID_RE.test(pmcid)) fail("Europe PMC PMCID is invalid"); }
  let doi = text("Europe PMC DOI", value.doi, 300, true);
  if (doi !== null) { doi = doi.toLowerCase(); if (!DOI_RE.test(doi)) fail("Europe PMC DOI is invalid"); }
  if (pmid === null && pmcid === null && doi === null) fail("Europe PMC result has no portable publication identifier");
  const title = text("Europe PMC title", value.title, 8_000)!;
  const year = text("Europe PMC publication year", value.pubYear, 4, true);
  if (year !== null && !/^\d{4}$/.test(year)) fail("Europe PMC publication year is invalid");
  const firstDate = text("Europe PMC first publication date", value.firstPublicationDate, 10, true);
  if (firstDate !== null) {
    if (!DATE_RE.test(firstDate)) fail("Europe PMC first publication date is invalid");
    if (firstDate.length === 7 && !nativeIsFinite(nativeDateParse(`${firstDate}-01T00:00:00Z`))) fail("Europe PMC first publication date is invalid");
    if (firstDate.length === 10 && timestampDateInvalid(firstDate)) fail("Europe PMC first publication date is invalid");
  }
  const cited = value.citedByCount === null || value.citedByCount === undefined ? null : integer("Europe PMC citedByCount", value.citedByCount, 0, Number.MAX_SAFE_INTEGER);
  const access = value.isOpenAccess;
  const isOpenAccess = access === "Y" ? true : access === "N" ? false : null;
  return {
    source_id: sourceId, europe_pmc_id: recordId, record_source: recordSource,
    pmid, pmcid, doi, title, author_string: text("Europe PMC authorString", value.authorString, 2_000, true),
    journal_title: text("Europe PMC journalTitle", value.journalTitle, 1_000, true), publication_year: year,
    first_publication_date: firstDate, publication_type: text("Europe PMC pubType", value.pubType, 2_000, true),
    is_open_access: isOpenAccess, cited_by_count: cited,
  };
}

function timestampDateInvalid(value: string): boolean {
  const date = new NativeDate(`${value}T00:00:00Z`);
  return !nativeIsFinite(date.getTime()) || date.toISOString().slice(0, 10) !== value;
}

function buildUrl(lane: ReviewedEuropePmcLane, pageSize: number, cursor: string): string {
  const encode = (value: string): string => nativeEncodeURIComponent(value).replace(/[!'()*]/g, (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`);
  const query = [
    `query=${encode(REVIEWED_EUROPE_PMC_LANES[lane])}`,
    "format=json", "resultType=lite", `pageSize=${pageSize}`,
    `cursorMark=${encode(cursor)}`, "synonym=N",
  ].join("&");
  const url = `${REVIEWED_EUROPE_PMC_ENDPOINT}?${query}`;
  if (!url.startsWith(`${REVIEWED_EUROPE_PMC_ENDPOINT}?`) || byteLength(url) > 8_192) fail("Europe PMC request escaped its reviewed endpoint");
  return url;
}

export interface ReviewedEuropePmcRetrievalConfigOptions {
  lanes?: readonly ReviewedEuropePmcLane[];
  pageSize?: number;
  maxPages?: number;
  timeoutMs?: number;
  transportId?: string;
  transportVersion?: string;
  transportConfigDigest?: string;
}

export interface ReviewedEuropePmcRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_EUROPE_PMC_CONFIG_SCHEMA;
  lanes: ReviewedEuropePmcLane[];
  page_size: number;
  max_pages: number;
  timeout_ms: number;
  request_limit: number;
  record_limit: number;
  transport_id: string;
  transport_version: string;
  transport_config_digest: string;
  query_set_digest: string;
  retention: typeof RETENTION;
  credentials: "not_accepted";
}

export interface ReviewedEuropePmcRetrievalConfigJSON extends ReviewedEuropePmcRetrievalConfigPayload {
  config_digest: string;
}

export class ReviewedEuropePmcRetrievalConfig {
  readonly lanes: readonly ReviewedEuropePmcLane[];
  readonly pageSize: number;
  readonly maxPages: number;
  readonly timeoutMs: number;
  readonly transportId: string;
  readonly transportVersion: string;
  readonly transportConfigDigest: string;
  readonly querySetDigest: string;
  readonly requestLimit: number;
  readonly recordLimit: number;
  readonly configDigest: string;

  constructor(options: ReviewedEuropePmcRetrievalConfigOptions = {}) {
    this.lanes = nativeObjectFreeze(normalizeLanes(options.lanes ?? ["glioma"]));
    this.pageSize = integer("Europe PMC pageSize", options.pageSize ?? 10, 1, MAX_REVIEWED_EUROPE_PMC_PAGE_SIZE);
    this.maxPages = integer("Europe PMC maxPages", options.maxPages ?? 2, 1, MAX_REVIEWED_EUROPE_PMC_PAGES);
    this.timeoutMs = integer("Europe PMC timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_EUROPE_PMC_TRANSPORT_ID;
    this.transportVersion = options.transportVersion ?? BUILTIN_EUROPE_PMC_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("Europe PMC transport identity is invalid");
    this.transportConfigDigest = digest("Europe PMC transportConfigDigest", options.transportConfigDigest ?? BUILTIN_EUROPE_PMC_TRANSPORT_CONFIG_DIGEST);
    this.querySetDigest = queryDigest(this.lanes);
    this.requestLimit = this.lanes.length * this.maxPages;
    this.recordLimit = this.requestLimit * this.pageSize;
    this.configDigest = digestJsonSync(this.payload());
    nativeObjectFreeze(this);
  }

  private payload(): ReviewedEuropePmcRetrievalConfigPayload {
    return {
      schema: REVIEWED_EUROPE_PMC_CONFIG_SCHEMA, lanes: [...this.lanes], page_size: this.pageSize,
      max_pages: this.maxPages, timeout_ms: this.timeoutMs, request_limit: this.requestLimit,
      record_limit: this.recordLimit, transport_id: this.transportId, transport_version: this.transportVersion,
      transport_config_digest: this.transportConfigDigest, query_set_digest: this.querySetDigest,
      retention: RETENTION, credentials: "not_accepted",
    };
  }

  toJSON(): ReviewedEuropePmcRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }

  static fromJSON(raw: unknown): ReviewedEuropePmcRetrievalConfig {
    const value = exactObject("Europe PMC config", raw, ["schema", "lanes", "page_size", "max_pages", "timeout_ms", "request_limit", "record_limit", "transport_id", "transport_version", "transport_config_digest", "query_set_digest", "retention", "credentials", "config_digest"]);
    if (value.schema !== REVIEWED_EUROPE_PMC_CONFIG_SCHEMA || !Array.isArray(value.lanes)) fail("Europe PMC config has an invalid schema");
    const config = new ReviewedEuropePmcRetrievalConfig({ lanes: value.lanes as ReviewedEuropePmcLane[], pageSize: value.page_size as number, maxPages: value.max_pages as number, timeoutMs: value.timeout_ms as number, transportId: value.transport_id as string, transportVersion: value.transport_version as string, transportConfigDigest: value.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(value)) fail("Europe PMC config is not normalized or its digest is invalid");
    return config;
  }
}

export interface ReviewedEuropePmcRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_EUROPE_PMC_PLAN_SCHEMA;
  config: ReviewedEuropePmcRetrievalConfigJSON;
  config_digest: string;
  query_set_digest: string;
  request_limit: number;
  record_limit: number;
  scope: "fixed_public_europe_pmc_lite_metadata";
  execution: "bounded_https_get_after_literal_approval";
  retention: typeof RETENTION;
  credentials: "not_accepted";
  plan_digest: string;
}

export class ReviewedEuropePmcRetrievalPlan {
  readonly configDigest: string;
  readonly querySetDigest: string;
  readonly planDigest: string;
  private constructor(readonly config: ReviewedEuropePmcRetrievalConfig, configDigest: string, querySetDigest: string, planDigest: string) {
    this.configDigest = configDigest; this.querySetDigest = querySetDigest; this.planDigest = planDigest; nativeObjectFreeze(this);
  }
  static create(config: ReviewedEuropePmcRetrievalConfig): ReviewedEuropePmcRetrievalPlan {
    if (!(config instanceof ReviewedEuropePmcRetrievalConfig)) fail("Europe PMC plan requires an exact config");
    const unsigned = { schema: REVIEWED_EUROPE_PMC_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, query_set_digest: config.querySetDigest, request_limit: config.requestLimit, record_limit: config.recordLimit, scope: "fixed_public_europe_pmc_lite_metadata" as const, execution: "bounded_https_get_after_literal_approval" as const, retention: RETENTION, credentials: "not_accepted" as const };
    return new ReviewedEuropePmcRetrievalPlan(config, config.configDigest, config.querySetDigest, digestJsonSync(unsigned));
  }
  toJSON(): ReviewedEuropePmcRetrievalPlanJSON {
    return { schema: REVIEWED_EUROPE_PMC_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, query_set_digest: this.querySetDigest, request_limit: this.config.requestLimit, record_limit: this.config.recordLimit, scope: "fixed_public_europe_pmc_lite_metadata", execution: "bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.planDigest };
  }
  validate(): void { const expected = ReviewedEuropePmcRetrievalPlan.create(this.config); if (expected.planDigest !== this.planDigest || expected.configDigest !== this.configDigest || expected.querySetDigest !== this.querySetDigest) fail("Europe PMC plan has drifted from its reviewed identity"); }
  static fromJSON(raw: unknown): ReviewedEuropePmcRetrievalPlan {
    const value = exactObject("Europe PMC plan", raw, ["schema", "config", "config_digest", "query_set_digest", "request_limit", "record_limit", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (value.schema !== REVIEWED_EUROPE_PMC_PLAN_SCHEMA) fail("Europe PMC plan has an invalid schema");
    const plan = ReviewedEuropePmcRetrievalPlan.create(ReviewedEuropePmcRetrievalConfig.fromJSON(value.config));
    if (canonicalJson(plan.toJSON()) !== canonicalJson(value)) fail("Europe PMC plan is not normalized or its digest is invalid");
    return plan;
  }
}

export type ReviewedEuropePmcFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
export interface ReviewedEuropePmcRetrievalAdapterOptions { fetch?: ReviewedEuropePmcFetch }

export class ReviewedEuropePmcRetrievalResult {
  private readonly bundleValue: Record<string, JsonValue>;
  private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundleValue = structuredClone(bundle); this.receiptValue = structuredClone(receipt); nativeObjectFreeze(this); }
  get bundle(): Record<string, JsonValue> { return structuredClone(this.bundleValue); }
  get receipt(): Record<string, JsonValue> { return structuredClone(this.receiptValue); }
  toJSON(): Record<string, JsonValue> { return { receipt: this.receipt, retention: "metadata_only;publication_values_excluded" }; }
  toTransientJSON(): Record<string, JsonValue> { return { schema: REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA, bundle: this.bundle, receipt: this.receipt, retention: "caller_owned_transient_publication_metadata" }; }
}

async function invokeTransport(url: string, timeoutMs: number, fetcher?: ReviewedEuropePmcFetch): Promise<unknown> {
  if (!fetcher && typeof NativeFetch !== "function") fail("Europe PMC built-in fetch is unavailable");
  const controller = new NativeAbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = nativeSetTimeout(() => { controller.abort(); reject(new ReviewedEuropePmcRetrievalError("Europe PMC request timed out")); }, timeoutMs); });
  try {
    const operation = fetcher
      ? Promise.resolve().then(() => fetcher(url, controller.signal))
      : Promise.resolve().then(async () => {
          const response = await NativeFetch(url, { method: "GET", redirect: "manual", signal: controller.signal, headers: { Accept: "application/json" } });
          if (response.status !== 200 || response.redirected || (response.status >= 300 && response.status < 400) || (response.url && (() => { const finalUrl = new NativeURL(response.url); return finalUrl.origin !== "https://www.ebi.ac.uk" || finalUrl.pathname !== REVIEWED_EUROPE_PMC_PATH; })())) fail("Europe PMC returned an unsuccessful or redirected response");
          const reader = response.body?.getReader();
          if (!reader) fail("Europe PMC response body is unavailable");
          const chunks: Uint8Array[] = [];
          let total = 0;
          while (true) {
            const part = await reader.read();
            if (part.done) break;
            const chunk = part.value;
            if (!(chunk instanceof Uint8Array) || total + chunk.byteLength > MAX_REVIEWED_EUROPE_PMC_RESPONSE_BYTES) {
              try { await reader.cancel(); } catch { /* Keep the bounded refusal as the reported error. */ }
              fail("Europe PMC response exceeds its byte bound");
            }
            chunks.push(chunk);
            total += chunk.byteLength;
          }
          const body = new Uint8Array(total);
          let offset = 0;
          for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
          return body;
        });
    return await nativePromiseRace([operation, timeout]);
  } catch (error) {
    if (error instanceof ReviewedEuropePmcRetrievalError) throw error;
    throw new ReviewedEuropePmcRetrievalError("Europe PMC request failed");
  } finally { if (timer !== undefined) nativeClearTimeout(timer); }
}

export class ReviewedEuropePmcRetrievalAdapter {
  readonly config: ReviewedEuropePmcRetrievalConfig;
  private readonly fetcher?: ReviewedEuropePmcFetch;
  constructor(config: ReviewedEuropePmcRetrievalConfig, options: ReviewedEuropePmcRetrievalAdapterOptions = {}) {
    if (!(config instanceof ReviewedEuropePmcRetrievalConfig)) fail("Europe PMC adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("Europe PMC injected transport is malformed");
    if (options.fetch && config.transportId === BUILTIN_EUROPE_PMC_TRANSPORT_ID) fail("Europe PMC injected transport requires a distinct reviewed identity");
    if (!options.fetch && config.transportId !== BUILTIN_EUROPE_PMC_TRANSPORT_ID) fail("Europe PMC built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; nativeObjectFreeze(this);
  }
  prepare(): ReviewedEuropePmcRetrievalPlan { return ReviewedEuropePmcRetrievalPlan.create(this.config); }

  async execute(plan: ReviewedEuropePmcRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string } ): Promise<ReviewedEuropePmcRetrievalResult> {
    if (!(plan instanceof ReviewedEuropePmcRetrievalPlan)) fail("Europe PMC execution requires an exact reviewed plan");
    plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("Europe PMC execution config differs from the reviewed plan");
    if (options.approveSourceDispatch !== true) fail("Europe PMC dispatch requires literal approval");
    const generatedAt = options.retrievedAt === undefined ? now() : timestamp("Europe PMC retrievedAt", options.retrievedAt);
    const publications: Record<string, JsonValue>[] = [];
    const sources: Record<string, JsonValue>[] = [];
    const sourceReceipts: Record<string, JsonValue>[] = [];
    let requestCount = 0; let responseBytes = 0;

    for (const lane of this.config.lanes) {
      const sourceId = `europepmc_${lane}`;
      const laneRows: Record<string, JsonValue>[] = [];
      const seenIds = new Set<string>();
      const seenCursors = new Set<string>(["*"]);
      const pageHitCounts: number[] = [];
      let cursor = "*"; let hasMore = false;
      for (let page = 0; page < this.config.maxPages; page += 1) {
        const url = buildUrl(lane, this.config.pageSize, cursor);
        const fetched = await invokeTransport(url, this.config.timeoutMs, this.fetcher);
        requestCount += 1;
        const { payload, bytes } = parseResponse(fetched);
        responseBytes += bytes;
        if (responseBytes > MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES) fail("Europe PMC aggregate response bytes exceed the plan bound");
        const hitCount = integer("Europe PMC hitCount", payload.hitCount, 0, Number.MAX_SAFE_INTEGER);
        pageHitCounts.push(hitCount);
        const resultList = payload.resultList;
        if (typeof resultList !== "object" || resultList === null || Array.isArray(resultList) || !Array.isArray((resultList as Record<string, unknown>).result)) fail("Europe PMC response omitted resultList.result");
        const pageRows = (resultList as { result: unknown[] }).result;
        if (pageRows.length > this.config.pageSize) fail("Europe PMC response exceeded the reviewed page size");
        for (const raw of pageRows) {
          const publication = normalizePublication(raw, sourceId);
          const id = `${publication.record_source}:${publication.europe_pmc_id}`;
          if (seenIds.has(id)) fail("Europe PMC pagination returned a duplicate publication");
          seenIds.add(id); laneRows.push(publication);
        }
        const nextRaw = payload.nextCursorMark;
        if (nextRaw === undefined || nextRaw === null) { hasMore = nativeOwn(payload, "nextPageUrl"); break; }
        const nextCursor = text("Europe PMC nextCursorMark", nextRaw, 512)!;
        if (nextCursor === cursor) { hasMore = false; break; }
        if (seenCursors.has(nextCursor)) fail("Europe PMC pagination cursor repeated");
        if (payload.nextPageUrl !== undefined && (typeof payload.nextPageUrl !== "string" || byteLength(payload.nextPageUrl) > 2_048)) fail("Europe PMC nextPageUrl is malformed");
        hasMore = true;
        if (page + 1 >= this.config.maxPages) break;
        seenCursors.add(nextCursor); cursor = nextCursor;
      }
      if (publications.length + laneRows.length > this.config.recordLimit) fail("Europe PMC record count exceeds the reviewed plan");
      const stableHits = pageHitCounts.length > 0 && pageHitCounts.every((count) => count === pageHitCounts[0]);
      let reportedHits: number | null = stableHits ? pageHitCounts[0]! : null;
      let omitted: number | null = reportedHits === null || reportedHits < laneRows.length ? null : reportedHits - laneRows.length;
      if (reportedHits !== null && reportedHits < laneRows.length) reportedHits = null;
      if (reportedHits === null) omitted = null;
      const completeness = hasMore || (omitted !== null && omitted > 0) ? "partial" : reportedHits === laneRows.length ? "complete" : "unknown";
      const laneDigest = digestJsonSync(laneRows);
      sources.push({ source_id: sourceId, authority: REVIEWED_EUROPE_PMC_AUTHORITY, uri: REVIEWED_EUROPE_PMC_ENDPOINT, retrieved_at: generatedAt, content_sha256: laneDigest, record_count: laneRows.length, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA, lane, source_id: sourceId, content_digest: laneDigest, record_count: laneRows.length, reported_hit_count: reportedHits, omitted_record_count: omitted, completeness });
      publications.push(...laneRows);
    }

    const statusRank: Record<string, number> = { complete: 0, unknown: 1, partial: 2 };
    const completeness = sourceReceipts.map((row) => row.completeness as string).reduce((left, right) => statusRank[left]! >= statusRank[right]! ? left : right) as "complete" | "unknown" | "partial";
    const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned = { schema: REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA, generated_at: generatedAt, sources, publications, source_set_digest: sourceSetDigest, record_count: publications.length, completeness, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_EUROPE_PMC_BUNDLE_BYTES) fail("Europe PMC bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const knownTotals = sourceReceipts.every((row) => row.reported_hit_count !== null);
    const knownOmitted = sourceReceipts.every((row) => row.omitted_record_count !== null);
    const receiptUnsigned = { schema: REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, query_set_digest: plan.querySetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, record_count: publications.length, request_count: requestCount, response_bytes: responseBytes, reported_hit_count: knownTotals ? sourceReceipts.reduce((sum, row) => sum + (row.reported_hit_count as number), 0) : null, omitted_record_count: knownOmitted ? sourceReceipts.reduce((sum, row) => sum + (row.omitted_record_count as number), 0) : null, completeness, retrieved_at: generatedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (typeof receiptUnsigned.reported_hit_count === "number" && receiptUnsigned.reported_hit_count > Number.MAX_SAFE_INTEGER) receiptUnsigned.reported_hit_count = null;
    if (typeof receiptUnsigned.omitted_record_count === "number" && receiptUnsigned.omitted_record_count > Number.MAX_SAFE_INTEGER) receiptUnsigned.omitted_record_count = null;
    const receipt = { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) };
    return new ReviewedEuropePmcRetrievalResult(bundle as unknown as Record<string, JsonValue>, receipt as unknown as Record<string, JsonValue>);
  }
}

export interface ReviewedEuropePmcExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA;
  reviewed_plan_digest: string;
  approve_source_dispatch: true;
  retrieved_at: string | null;
  retention: "metadata_only";
  credentials: "not_accepted";
  metadata_digest: string;
}
export function createReviewedEuropePmcExecutionMetadata(plan: ReviewedEuropePmcRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedEuropePmcExecutionMetadata {
  if (!(plan instanceof ReviewedEuropePmcRetrievalPlan)) fail("Europe PMC execution metadata requires an exact plan");
  plan.validate();
  if (approveSourceDispatch !== true) fail("Europe PMC execution metadata requires literal approval");
  const payload = { schema: REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("Europe PMC retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateTransient(value: unknown, plan: ReviewedEuropePmcRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("Europe PMC transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_EUROPE_PMC_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_publication_metadata") fail("Europe PMC transient identity is invalid");
  const bundle = exactObject("Europe PMC transient bundle", transient.bundle, ["schema", "generated_at", "sources", "publications", "source_set_digest", "record_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("Europe PMC transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "query_set_digest", "bundle_digest", "source_set_digest", "source_count", "record_count", "request_count", "response_bytes", "reported_hit_count", "omitted_record_count", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleUnsigned = Object.fromEntries(Object.entries(bundle).filter(([key]) => key !== "bundle_digest"));
  const receiptUnsigned = Object.fromEntries(Object.entries(receipt).filter(([key]) => key !== "receipt_digest"));
  if (digestJsonSync(bundleUnsigned) !== digest("Europe PMC bundle_digest", bundle.bundle_digest) || digestJsonSync(receiptUnsigned) !== digest("Europe PMC receipt_digest", receipt.receipt_digest)) fail("Europe PMC transient digest is invalid");
  if (bundle.schema !== REVIEWED_EUROPE_PMC_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_EUROPE_PMC_RECEIPT_SCHEMA) fail("Europe PMC transient schema is invalid");
  if (receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.query_set_digest !== plan.querySetDigest || receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.credentials !== "not_accepted" || receipt.provider !== "none" || receipt.retention !== RETENTION) fail("Europe PMC transient receipt is outside the reviewed plan");
  if (bundle.provider !== "none" || bundle.credentials !== "not_accepted" || canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("Europe PMC transient boundary metadata is invalid");
  const generatedAt = timestamp("Europe PMC bundle generated_at", bundle.generated_at);
  const retrievedAt = timestamp("Europe PMC receipt retrieved_at", receipt.retrieved_at);
  if (generatedAt !== retrievedAt || bundle.generated_at !== receipt.retrieved_at) fail("Europe PMC bundle and receipt timestamps do not match");
  if (!Array.isArray(bundle.sources) || !Array.isArray(bundle.publications) || digestJsonSync(bundle.sources) !== bundle.source_set_digest || receipt.source_set_digest !== bundle.source_set_digest || receipt.bundle_digest !== bundle.bundle_digest) fail("Europe PMC transient source binding is invalid");
  const publications = bundle.publications;
  const recordCount = integer("Europe PMC bundle record_count", bundle.record_count, 0, plan.config.recordLimit);
  if (publications.length > plan.config.recordLimit || recordCount !== publications.length || integer("Europe PMC receipt record_count", receipt.record_count, 0, plan.config.recordLimit) !== publications.length) fail("Europe PMC transient publication count is invalid");
  const [lane] = plan.config.lanes;
  const sourceId = `europepmc_${lane}`;
  if (bundle.sources.length !== 1 || !Array.isArray(receipt.source_receipts) || receipt.source_receipts.length !== 1) fail("Europe PMC transient source cardinality is invalid");
  const source = exactObject("Europe PMC source metadata", bundle.sources[0], ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"]);
  const sourceReceipt = exactObject("Europe PMC source receipt", receipt.source_receipts[0], ["schema", "lane", "source_id", "content_digest", "record_count", "reported_hit_count", "omitted_record_count", "completeness"]);
  const identities = new Set<string>();
  for (const raw of publications) {
    const publication = exactObject("Europe PMC publication metadata", raw, ["source_id", "europe_pmc_id", "record_source", "pmid", "pmcid", "doi", "title", "author_string", "journal_title", "publication_year", "first_publication_date", "publication_type", "is_open_access", "cited_by_count"]);
    if (publication.source_id !== sourceId) fail("Europe PMC publication is outside its reviewed lane");
    const recordId = text("Europe PMC record id", publication.europe_pmc_id, 32)!;
    const recordSource = text("Europe PMC record source", publication.record_source, 16)!;
    if (!IDENTIFIER_RE.test(recordId) || !IDENTIFIER_RE.test(recordSource)) fail("Europe PMC publication identity is invalid");
    const identity = `${recordSource}:${recordId}`;
    if (identities.has(identity)) fail("Europe PMC transient publication identity is duplicated");
    identities.add(identity);
    const pmid = publication.pmid; const pmcid = publication.pmcid; const doi = publication.doi;
    if (pmid !== null && (typeof pmid !== "string" || !PMID_RE.test(pmid))) fail("Europe PMC transient PMID is invalid");
    if (pmcid !== null && (typeof pmcid !== "string" || !PMCID_RE.test(pmcid) || pmcid !== pmcid.toUpperCase())) fail("Europe PMC transient PMCID is invalid");
    if (doi !== null && (typeof doi !== "string" || !DOI_RE.test(doi) || doi !== doi.toLowerCase())) fail("Europe PMC transient DOI is invalid");
    if (pmid === null && pmcid === null && doi === null) fail("Europe PMC transient publication has no portable identifier");
    for (const [name, maximum, optional] of [["title", 8_000, false], ["author_string", 2_000, true], ["journal_title", 1_000, true], ["publication_year", 4, true], ["first_publication_date", 10, true], ["publication_type", 2_000, true]] as const) {
      const item = publication[name];
      if (optional && item === null) continue;
      if (text(`Europe PMC ${name}`, item, maximum, optional) !== item) fail("Europe PMC transient publication text is not normalized");
    }
    const year = publication.publication_year;
    if (year !== null && (typeof year !== "string" || !/^\d{4}$/.test(year))) fail("Europe PMC transient publication year is invalid");
    const published = publication.first_publication_date;
    if (published !== null) {
      if (typeof published !== "string" || !DATE_RE.test(published) || (published.length === 7 && !nativeIsFinite(nativeDateParse(`${published}-01T00:00:00Z`))) || (published.length === 10 && timestampDateInvalid(published))) fail("Europe PMC transient publication date is invalid");
    }
    if (publication.is_open_access !== null && typeof publication.is_open_access !== "boolean") fail("Europe PMC transient open-access value is invalid");
    if (publication.cited_by_count !== null) integer("Europe PMC transient cited_by_count", publication.cited_by_count, 0, Number.MAX_SAFE_INTEGER);
  }
  const publicationDigest = digestJsonSync(publications);
  if (source.source_id !== sourceId || source.authority !== REVIEWED_EUROPE_PMC_AUTHORITY || source.uri !== REVIEWED_EUROPE_PMC_ENDPOINT || source.retrieved_at !== retrievedAt || source.content_sha256 !== publicationDigest || integer("Europe PMC source record_count", source.record_count, 0, plan.config.recordLimit) !== publications.length || source.provider !== "none" || source.credentials !== "not_accepted" || canonicalJson(source.limitations) !== canonicalJson(LIMITATIONS)) fail("Europe PMC source metadata does not match its publication bundle");
  const reportedHits = sourceReceipt.reported_hit_count === null ? null : integer("Europe PMC reported_hit_count", sourceReceipt.reported_hit_count, publications.length, Number.MAX_SAFE_INTEGER);
  const omitted = sourceReceipt.omitted_record_count === null ? null : integer("Europe PMC omitted_record_count", sourceReceipt.omitted_record_count, 0, Number.MAX_SAFE_INTEGER);
  if (omitted !== null && (reportedHits === null || omitted !== reportedHits - publications.length)) fail("Europe PMC omitted-record count does not match its source total");
  const sourceStatus = sourceReceipt.completeness;
  if (sourceStatus !== "complete" && sourceStatus !== "unknown" && sourceStatus !== "partial") fail("Europe PMC source completeness is invalid");
  const expectedStatus = omitted !== null && omitted > 0 ? "partial" : reportedHits === publications.length ? "complete" : "unknown";
  if (sourceStatus !== expectedStatus && sourceStatus !== "partial") fail("Europe PMC source completeness does not match its coverage totals");
  if (sourceReceipt.schema !== REVIEWED_EUROPE_PMC_SOURCE_RECEIPT_SCHEMA || sourceReceipt.lane !== lane || sourceReceipt.source_id !== sourceId || sourceReceipt.content_digest !== publicationDigest || integer("Europe PMC source receipt record_count", sourceReceipt.record_count, 0, plan.config.recordLimit) !== publications.length) fail("Europe PMC source receipt does not match its publication bundle");
  if (bundle.completeness !== sourceStatus || receipt.completeness !== sourceStatus || integer("Europe PMC receipt source_count", receipt.source_count, 1, 1) !== 1 || canonicalJson(receipt.source_receipts) !== canonicalJson([sourceReceipt]) || receipt.reported_hit_count !== reportedHits || receipt.omitted_record_count !== omitted) fail("Europe PMC receipt totals do not match its source receipt");
  const requestCount = integer("Europe PMC request_count", receipt.request_count, 1, plan.config.maxPages);
  const responseBytes = integer("Europe PMC response_bytes", receipt.response_bytes, 1, MAX_REVIEWED_EUROPE_PMC_TOTAL_RESPONSE_BYTES);
  if (requestCount > responseBytes) fail("Europe PMC response byte count is inconsistent with its request count");
  return { bundle, receipt };
}

export function createReviewedEuropePmcAutonomousEvidenceRegistration(adapter: ReviewedEuropePmcRetrievalAdapter, plan: ReviewedEuropePmcRetrievalPlan, lane: ReviewedEuropePmcLane): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (!(adapter instanceof ReviewedEuropePmcRetrievalAdapter) || !(plan instanceof ReviewedEuropePmcRetrievalPlan)) fail("Europe PMC registration requires exact adapter and plan values");
  plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || !plan.config.lanes.includes(lane) || plan.config.lanes.length !== 1) fail("Europe PMC registration requires the exact single-lane plan");
  const frozenPlan = ReviewedEuropePmcRetrievalPlan.create(plan.config);
  const sourceId = `europepmc_${lane}`;
  const expectedNetwork = adapter.config.transportId === BUILTIN_EUROPE_PMC_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.europepmc.${lane}`, version: REVIEWED_EUROPE_PMC_ADAPTER_VERSION,
    domains: ["biomedical", "neuroscience"], capabilities: ["scientific_literature_retrieval", "source_provenance"], sourceKinds: ["europepmc_lite_publication_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request;
      if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("Europe PMC acquisition request does not match its reviewed source");
      const metadata = exactObject("Europe PMC execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("Europe PMC metadata_digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_EUROPE_PMC_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("Europe PMC execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("Europe PMC retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork);
      const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("Europe PMC projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}

export type ReviewedEuropePmcRetrievalReceiptJSON = Record<string, JsonValue>;
export type ReviewedEuropePmcTransientJSON = Record<string, JsonValue>;
