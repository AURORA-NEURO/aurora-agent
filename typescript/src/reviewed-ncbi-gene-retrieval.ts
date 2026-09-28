/** Reviewed fixed-catalogue NCBI Gene summary retrieval (blueprint modules 11.06 and 40.15, TypeScript SDK). */

import { ArgumentError } from "./errors.js";
import { acquireNcbiRequestSlot } from "./ncbi-rate-limit.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonValue } from "./types.js";

export const REVIEWED_NCBI_GENE_CONFIG_SCHEMA = "bioprism-reviewed-ncbi-gene-config/0.1" as const;
export const REVIEWED_NCBI_GENE_PLAN_SCHEMA = "bioprism-reviewed-ncbi-gene-plan/0.1" as const;
export const REVIEWED_NCBI_GENE_BUNDLE_SCHEMA = "bioprism-reviewed-ncbi-gene-bundle/0.1" as const;
export const REVIEWED_NCBI_GENE_RECEIPT_SCHEMA = "bioprism-reviewed-ncbi-gene-receipt/0.1" as const;
export const REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA = "bioprism-reviewed-ncbi-gene-transient/0.1" as const;
export const REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-ncbi-gene-execution-metadata/0.1" as const;
export const REVIEWED_NCBI_GENE_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_NCBI_GENE_HOST = "eutils.ncbi.nlm.nih.gov" as const;
export const REVIEWED_NCBI_GENE_PATH = "/entrez/eutils/esummary.fcgi" as const;
export const REVIEWED_NCBI_GENE_ENDPOINT = `https://${REVIEWED_NCBI_GENE_HOST}${REVIEWED_NCBI_GENE_PATH}` as const;
export const REVIEWED_NCBI_GENE_AUTHORITY = "NCBI Gene" as const;
export const REVIEWED_NCBI_GENE_CATALOGUE = Object.freeze({
  IDH1: "3417", IDH2: "3418", MGMT: "4255", EGFR: "1956", TERT: "7015", TP53: "7157",
  ATRX: "546", NF1: "4763", PTEN: "5728", CDKN2A: "1029", PDGFRA: "5156", BRAF: "673",
} as const);
export type ReviewedNcbiGeneSymbol = keyof typeof REVIEWED_NCBI_GENE_CATALOGUE;
export const MAX_REVIEWED_NCBI_GENE_SYMBOLS = 12;
export const MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES = 1_000_000;
export const MAX_REVIEWED_NCBI_GENE_BUNDLE_BYTES = 256_000;
export const MAX_REVIEWED_NCBI_GENE_TREE_DEPTH = 32;
export const MAX_REVIEWED_NCBI_GENE_TREE_NODES = 50_000;
export const BUILTIN_NCBI_GENE_TRANSPORT_ID = "builtin.ncbi-gene.fetch" as const;
export const BUILTIN_NCBI_GENE_TRANSPORT_VERSION = "1" as const;
const catalogueDigest = digestJsonSync(REVIEWED_NCBI_GENE_CATALOGUE);
export const BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_NCBI_GENE_HOST,
  path: REVIEWED_NCBI_GENE_PATH, method: "GET", database: "gene",
  parameters: ["db", "id", "retmode", "tool?", "email?"], catalogue_digest: catalogueDigest,
  redirects: "refused", credentials: "none",
});

const RETENTION = "bounded_gene_identity_alias_location_metadata_only" as const;
const LIMITATIONS = Object.freeze([
  "the fixed human-gene catalogue is curated and is not an exhaustive gene search",
  "NCBI Gene metadata can change and does not establish disease relevance or evidence quality",
  "the adapter excludes Gene summaries, sequences, variants, expression, samples, and patient data",
  "retrieved symbols must match the reviewed catalogue exactly; changed records fail closed",
  "the adapter limits one request per execution and paces requests per process; deployments must coordinate rate limits across processes and hosts",
  "caller-injected transports own timeout, redirect, and network policy under their declared identity",
] as const);
const SYMBOLS = Object.keys(REVIEWED_NCBI_GENE_CATALOGUE) as ReviewedNcbiGeneSymbol[];
const DIGEST_RE = /^[0-9a-f]{64}$/;
const IDENTIFIER_RE = /^[A-Za-z0-9_.:-]{1,128}$/;
const NativeFetch = globalThis.fetch;
const NativeAbortController = globalThis.AbortController;
const NativeTextEncoder = globalThis.TextEncoder;
const NativeTextDecoder = globalThis.TextDecoder;
const NativeDate = globalThis.Date;
const NativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const NativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const NativeJsonParse = globalThis.JSON.parse.bind(globalThis.JSON);
const NativeJsonStringify = globalThis.JSON.stringify.bind(globalThis.JSON);
const NativeObjectKeys = globalThis.Object.keys.bind(globalThis.Object);
const NativeObjectValues = globalThis.Object.values.bind(globalThis.Object);
const NativeOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty) as (value: object, key: PropertyKey) => boolean;
const NativePromiseRace = globalThis.Promise.race.bind(globalThis.Promise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;
const NativeIsFinite = globalThis.Number.isFinite.bind(globalThis.Number);
const NativeIsSafeInteger = globalThis.Number.isSafeInteger.bind(globalThis.Number);
const Encoder = new NativeTextEncoder();
const CONFIG_REGISTRATION = new WeakMap<object, { tool: string | null; email: string | null }>();

export class ReviewedNcbiGeneRetrievalError extends ArgumentError {
  override readonly name = "ReviewedNcbiGeneRetrievalError";
}
function fail(message: string): never { throw new ReviewedNcbiGeneRetrievalError(message); }
function byteLength(value: string): number { return Encoder.encode(value).byteLength; }
function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST_RE.test(value)) fail(`${name} is invalid`);
  return value;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!NativeIsSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its bounded integer range`);
  return value as number;
}
function text(name: string, value: unknown, maximum: number, optional = false): string | null {
  if (value === null || value === undefined) { if (optional) return null; return fail(`${name} must be text`); }
  if (typeof value !== "string") fail(`${name} must be text`);
  const normalized = (value as string).replace(/[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu, " ").replace(/^ | $/g, "");
  if ((!normalized && !optional) || /[\u0000-\u001f\u007f]/.test(normalized) || byteLength(normalized) > maximum) fail(`${name} is empty or exceeds its text bound`);
  return normalized || null;
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
  let nodes = 0;
  const seen = new WeakSet<object>();
  while (pending.length) {
    const next = pending.pop()!;
    nodes += 1;
    if (nodes > MAX_REVIEWED_NCBI_GENE_TREE_NODES || next.depth > MAX_REVIEWED_NCBI_GENE_TREE_DEPTH) fail("NCBI Gene response exceeds its structural bound");
    const current = next.value;
    if (typeof current === "number" && (!NativeIsFinite(current) || (Number.isInteger(current) && !NativeIsSafeInteger(current)))) fail("NCBI Gene response contains an invalid number");
    if (current !== null && typeof current === "object") {
      if (seen.has(current)) fail("NCBI Gene response is not a JSON tree");
      seen.add(current);
      if (Array.isArray(current)) for (const child of current) pending.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(current) !== Object.prototype) fail("NCBI Gene response contains a non-JSON object");
        for (const key of NativeObjectKeys(current)) {
          if (/[\u0000-\u001f\u007f]/.test(key)) fail("NCBI Gene response contains an invalid key");
          if (/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(key)) fail("NCBI Gene response contains invalid Unicode");
        }
        for (const child of NativeObjectValues(current as Record<string, unknown>)) pending.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof current === "string" && /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(current)) fail("NCBI Gene response contains invalid Unicode");
    else if (current !== null && !["string", "number", "boolean"].includes(typeof current)) fail("NCBI Gene response contains an unsupported value");
  }
}

/** Reject duplicate JSON fields before native parsing can overwrite source values. */
function parseStrictJson(source: string): unknown {
  let index = 0; let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index;
    if (source[index] !== "\"") fail("NCBI Gene response is not valid JSON");
    index += 1;
    while (index < source.length) {
      if (source[index] === "\"") { index += 1; try { return NativeJsonParse(source.slice(start, index)) as string; } catch { return fail("NCBI Gene response is not valid JSON"); } }
      if (source[index] === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 32) fail("NCBI Gene response is not valid JSON");
      index += 1;
    }
    return fail("NCBI Gene response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1;
    if (nodes > MAX_REVIEWED_NCBI_GENE_TREE_NODES || depth > MAX_REVIEWED_NCBI_GENE_TREE_DEPTH) fail("NCBI Gene response exceeds its structural bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace(); const keys = new Set<string>();
      if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string();
        if (keys.has(key)) fail("NCBI Gene response contains duplicate JSON fields");
        keys.add(key); whitespace(); if (source[index] !== ":") fail("NCBI Gene response is not valid JSON");
        index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; }
        if (source[index] !== ",") fail("NCBI Gene response is not valid JSON"); index += 1;
      }
      return fail("NCBI Gene response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace(); if (source[index] === "]") { index += 1; return; }
      while (index < source.length) {
        scan(depth + 1); whitespace();
        if (source[index] === "]") { index += 1; return; }
        if (source[index] !== ",") fail("NCBI Gene response is not valid JSON"); index += 1;
      }
      return fail("NCBI Gene response is not valid JSON");
    }
    const start = index;
    while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (index === start) fail("NCBI Gene response is not valid JSON");
    try { NativeJsonParse(source.slice(start, index)); } catch { fail("NCBI Gene response is not valid JSON"); }
  };
  scan(0); whitespace();
  if (index !== source.length) fail("NCBI Gene response is not valid JSON");
  const parsed = NativeJsonParse(source) as unknown; validateTree(parsed); return parsed;
}
function parseResponse(value: unknown): { payload: Record<string, unknown>; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) {
    try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("NCBI Gene response is not valid UTF-8"); }
  } else if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    try { body = NativeJsonStringify(value) as string; } catch { return fail("NCBI Gene injected response is not JSON"); }
  } else return fail("NCBI Gene transport returned an unsupported response type");
  const size = byteLength(body);
  if (size > MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES) fail("NCBI Gene response exceeds its byte bound");
  const parsed = parseStrictJson(body);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) fail("NCBI Gene response root is not an object");
  return { payload: parsed as Record<string, unknown>, bytes: size };
}
function registration(tool: unknown, email: unknown): { tool: string | null; email: string | null; digest: string } {
  if ((tool === undefined || tool === null) !== (email === undefined || email === null)) fail("ncbiTool and ncbiEmail must be provided together");
  if (tool === undefined || tool === null) return { tool: null, email: null, digest: "none" };
  if (typeof tool !== "string" || !/^[A-Za-z][A-Za-z0-9_.-]{0,63}$/.test(tool)) fail("ncbiTool must be a bounded application name without spaces");
  if (typeof email !== "string" || !/^[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]+\.[A-Za-z]{2,63}$/.test(email) || email.length > 254) fail("ncbiEmail must be a bounded developer email address");
  return { tool, email, digest: digestJsonSync({ tool, email }) };
}
function queryUrl(geneIds: readonly string[], tool: string | null, email: string | null): string {
  const parameters = new URLSearchParams({ db: "gene", id: geneIds.join(","), retmode: "json" });
  if (tool !== null && email !== null) { parameters.set("tool", tool); parameters.set("email", email); }
  const url = `${REVIEWED_NCBI_GENE_ENDPOINT}?${parameters.toString()}`;
  if (!url.startsWith(`${REVIEWED_NCBI_GENE_ENDPOINT}?`) || byteLength(url) > 8_192) fail("NCBI Gene request escaped its reviewed endpoint");
  return url;
}
async function builtinFetch(url: string, timeoutMs: number): Promise<Uint8Array> {
  if (NativeFetch === undefined || NativeAbortController === undefined) fail("NCBI Gene built-in fetch is unavailable");
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedNcbiGeneRetrievalError("NCBI Gene request timed out")); }, timeoutMs); });
  try {
    const request = NativeFetch(url, { method: "GET", headers: { Accept: "application/json" }, redirect: "error", signal: controller.signal });
    const operation = request.then(async (response) => {
      if (!response.ok || response.url !== url || response.redirected || !response.body) fail("NCBI Gene endpoint returned an unexpected response");
      const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let total = 0;
      while (true) {
        const { done, value } = await reader.read(); if (done) break;
        if (!(value instanceof Uint8Array) || total + value.byteLength > MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES) { try { await reader.cancel(); } catch { /* Keep the bounded refusal. */ } fail("NCBI Gene response exceeds its byte bound"); }
        chunks.push(value); total += value.byteLength;
      }
      const body = new Uint8Array(total); let offset = 0;
      for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
      return body;
    });
    return await NativePromiseRace([operation, timeout]);
  } catch (error) {
    if (error instanceof ReviewedNcbiGeneRetrievalError) throw error;
    throw new ReviewedNcbiGeneRetrievalError("NCBI Gene request failed");
  } finally { if (timer !== undefined) NativeClearTimeout(timer); }
}
function normalizeGene(payload: Record<string, unknown>, symbol: ReviewedNcbiGeneSymbol): Record<string, JsonValue> {
  const geneId = REVIEWED_NCBI_GENE_CATALOGUE[symbol];
  const result = payload.result;
  if (typeof result !== "object" || result === null || Array.isArray(result)) fail("NCBI Gene response omitted its result object");
  const resultRow = result as Record<string, unknown>; const raw = resultRow[geneId];
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("NCBI Gene response omitted a reviewed record");
  const gene = raw as Record<string, unknown>;
  if (gene.uid !== geneId) fail("NCBI Gene record identity does not match the reviewed GeneID");
  if (typeof gene.organism !== "object" || gene.organism === null || Array.isArray(gene.organism) || (gene.organism as Record<string, unknown>).taxid !== 9606) fail("NCBI Gene record is not the reviewed human organism");
  if (text("NCBI Gene symbol", gene.name, 32) !== symbol) fail("NCBI Gene symbol differs from the reviewed catalogue");
  const aliasesRaw = text("NCBI Gene aliases", gene.otheraliases, 16_000, true);
  const aliases = aliasesRaw ? [...new Set(aliasesRaw.split(",").map((alias) => text("NCBI Gene alias", alias, 256, true)).filter((alias): alias is string => alias !== null))].sort() : [];
  if (aliases.length > 128) fail("NCBI Gene aliases exceed their list bound");
  return {
    source_id: "ncbi_gene", gene_id: geneId, symbol,
    description: text("NCBI Gene description", gene.description, 2_000)!,
    chromosome: text("NCBI Gene chromosome", gene.chromosome, 64, true),
    map_location: text("NCBI Gene map location", gene.maplocation, 128, true),
    aliases, organism_taxid: 9606,
  };
}
function validateResponseCoverage(payload: Record<string, unknown>, symbols: readonly ReviewedNcbiGeneSymbol[]): void {
  if (typeof payload.header !== "object" || payload.header === null || Array.isArray(payload.header) || (payload.header as Record<string, unknown>).type !== "esummary") fail("NCBI Gene response is not an ESummary result");
  if (typeof payload.result !== "object" || payload.result === null || Array.isArray(payload.result)) fail("NCBI Gene response omitted its result object");
  const result = payload.result as Record<string, unknown>; const ids = symbols.map((symbol) => REVIEWED_NCBI_GENE_CATALOGUE[symbol]);
  if (!Array.isArray(result.uids) || canonicalJson(result.uids) !== canonicalJson(ids)) fail("NCBI Gene response is incomplete or reordered against the reviewed catalogue");
  const expected = ["uids", ...ids].sort();
  if (canonicalJson(NativeObjectKeys(result).sort()) !== canonicalJson(expected)) fail("NCBI Gene response contains unrequested records");
  for (const symbol of symbols) normalizeGene(payload, symbol);
}

export interface ReviewedNcbiGeneRetrievalConfigOptions {
  geneSymbols?: readonly ReviewedNcbiGeneSymbol[]; timeoutMs?: number; transportId?: string; transportVersion?: string;
  transportConfigDigest?: string; ncbiTool?: string; ncbiEmail?: string;
}
export interface ReviewedNcbiGeneRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_NCBI_GENE_CONFIG_SCHEMA; gene_symbols: ReviewedNcbiGeneSymbol[]; gene_ids: string[];
  timeout_ms: number; request_limit: 1; transport_id: string; transport_version: string; transport_config_digest: string;
  catalogue_digest: string; ncbi_registration_digest: string; retention: typeof RETENTION; credentials: "none";
}
export interface ReviewedNcbiGeneRetrievalConfigJSON extends ReviewedNcbiGeneRetrievalConfigPayload { config_digest: string }
export class ReviewedNcbiGeneRetrievalConfig {
  readonly geneSymbols: readonly ReviewedNcbiGeneSymbol[]; readonly timeoutMs: number; readonly transportId: string;
  readonly transportVersion: string; readonly transportConfigDigest: string; readonly ncbiRegistrationDigest: string;
  constructor(options: ReviewedNcbiGeneRetrievalConfigOptions = {}) {
    const requested = options.geneSymbols ?? SYMBOLS;
    if (!Array.isArray(requested) || requested.length < 1 || requested.length > MAX_REVIEWED_NCBI_GENE_SYMBOLS || requested.some((symbol) => typeof symbol !== "string" || !NativeOwn(REVIEWED_NCBI_GENE_CATALOGUE, symbol)) || new Set(requested).size !== requested.length) fail("NCBI Gene symbols contain an unsupported or duplicate catalogue entry");
    this.geneSymbols = Object.freeze(SYMBOLS.filter((symbol) => requested.includes(symbol)));
    this.timeoutMs = integer("NCBI Gene timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_NCBI_GENE_TRANSPORT_ID; this.transportVersion = options.transportVersion ?? BUILTIN_NCBI_GENE_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("NCBI Gene transport identity is invalid");
    this.transportConfigDigest = digest("NCBI Gene transportConfigDigest", options.transportConfigDigest ?? BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST);
    const registered = registration(options.ncbiTool, options.ncbiEmail); this.ncbiRegistrationDigest = registered.digest;
    CONFIG_REGISTRATION.set(this, { tool: registered.tool, email: registered.email });
    Object.freeze(this);
  }
  get requestLimit(): number { return 1; }
  private payload(): ReviewedNcbiGeneRetrievalConfigPayload {
    return { schema: REVIEWED_NCBI_GENE_CONFIG_SCHEMA, gene_symbols: [...this.geneSymbols], gene_ids: this.geneSymbols.map((symbol) => REVIEWED_NCBI_GENE_CATALOGUE[symbol]), timeout_ms: this.timeoutMs, request_limit: 1, transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest, catalogue_digest: catalogueDigest, ncbi_registration_digest: this.ncbiRegistrationDigest, retention: RETENTION, credentials: "none" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedNcbiGeneRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown, registrationValues: Pick<ReviewedNcbiGeneRetrievalConfigOptions, "ncbiTool" | "ncbiEmail"> = {}): ReviewedNcbiGeneRetrievalConfig {
    const raw = exactObject("NCBI Gene config", value, ["schema", "gene_symbols", "gene_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "catalogue_digest", "ncbi_registration_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_NCBI_GENE_CONFIG_SCHEMA || !Array.isArray(raw.gene_symbols)) fail("NCBI Gene config has an invalid shape");
    const config = new ReviewedNcbiGeneRetrievalConfig({ geneSymbols: raw.gene_symbols as ReviewedNcbiGeneSymbol[], timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string, ...registrationValues });
    if (config.ncbiRegistrationDigest !== raw.ncbi_registration_digest) fail("NCBI Gene registration identity changed");
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("NCBI Gene config is not normalized or its digest is invalid");
    return config;
  }
}
function requestUrl(config: ReviewedNcbiGeneRetrievalConfig): string {
  const registered = CONFIG_REGISTRATION.get(config);
  if (!registered) fail("NCBI Gene request configuration lost its transient registration");
  return queryUrl(config.geneSymbols.map((symbol) => REVIEWED_NCBI_GENE_CATALOGUE[symbol]), registered.tool, registered.email);
}
export interface ReviewedNcbiGeneRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_NCBI_GENE_PLAN_SCHEMA; config: ReviewedNcbiGeneRetrievalConfigJSON; config_digest: string;
  catalogue_digest: string; request_limit: 1; scope: "fixed_human_ncbi_gene_summary_metadata";
  execution: "one_bounded_https_get_after_literal_approval"; retention: typeof RETENTION; credentials: "none"; plan_digest: string;
}
export class ReviewedNcbiGeneRetrievalPlan {
  readonly config: ReviewedNcbiGeneRetrievalConfig; readonly configDigest: string; readonly catalogueDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedNcbiGeneRetrievalConfig, configDigest: string, catalogDigestValue: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.catalogueDigest = catalogDigestValue; this.planDigest = planDigest; Object.freeze(this); }
  static create(config: ReviewedNcbiGeneRetrievalConfig): ReviewedNcbiGeneRetrievalPlan {
    if (!(config instanceof ReviewedNcbiGeneRetrievalConfig)) fail("NCBI Gene plan requires an exact config");
    const fields = { schema: REVIEWED_NCBI_GENE_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, catalogue_digest: catalogueDigest, request_limit: 1 as const, scope: "fixed_human_ncbi_gene_summary_metadata" as const, execution: "one_bounded_https_get_after_literal_approval" as const, retention: RETENTION, credentials: "none" as const };
    return new ReviewedNcbiGeneRetrievalPlan(config, config.configDigest, catalogueDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedNcbiGeneRetrievalPlanJSON { return { schema: REVIEWED_NCBI_GENE_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, catalogue_digest: this.catalogueDigest, request_limit: 1, scope: "fixed_human_ncbi_gene_summary_metadata", execution: "one_bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "none", plan_digest: this.planDigest }; }
  validate(): void { const expected = ReviewedNcbiGeneRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.catalogueDigest !== expected.catalogueDigest || this.planDigest !== expected.planDigest) fail("NCBI Gene plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown, registrationValues: Pick<ReviewedNcbiGeneRetrievalConfigOptions, "ncbiTool" | "ncbiEmail"> = {}): ReviewedNcbiGeneRetrievalPlan {
    const raw = exactObject("NCBI Gene plan", value, ["schema", "config", "config_digest", "catalogue_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_NCBI_GENE_PLAN_SCHEMA) fail("NCBI Gene plan has an invalid shape");
    const plan = ReviewedNcbiGeneRetrievalPlan.create(ReviewedNcbiGeneRetrievalConfig.fromJSON(raw.config, registrationValues));
    if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("NCBI Gene plan is not normalized or its digest is invalid");
    return plan;
  }
}
export type ReviewedNcbiGeneMetadata = Record<string, JsonValue>;
export type ReviewedNcbiGeneRetrievalReceiptJSON = Record<string, JsonValue>;
export type ReviewedNcbiGeneTransientJSON = Record<string, JsonValue>;
export class ReviewedNcbiGeneRetrievalResult {
  private readonly bundle: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundle = JSON.parse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = JSON.parse(canonicalJson(receipt)) as Record<string, JsonValue>; Object.freeze(this); }
  get receipt(): Record<string, JsonValue> { return JSON.parse(canonicalJson(this.receiptValue)) as Record<string, JsonValue>; }
  toJSON(): { receipt: Record<string, JsonValue>; retention: string } { return { receipt: this.receipt, retention: "metadata_only" }; }
  toTransientJSON(): ReviewedNcbiGeneTransientJSON { return { schema: REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA, bundle: JSON.parse(canonicalJson(this.bundle)) as JsonValue, receipt: this.receipt, retention: "caller_owned_transient_gene_metadata" }; }
}
export type ReviewedNcbiGeneFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
export interface ReviewedNcbiGeneRetrievalAdapterOptions { fetch?: ReviewedNcbiGeneFetch }
export class ReviewedNcbiGeneRetrievalAdapter {
  readonly config: ReviewedNcbiGeneRetrievalConfig; private readonly fetcher?: ReviewedNcbiGeneFetch;
  constructor(config: ReviewedNcbiGeneRetrievalConfig, options: ReviewedNcbiGeneRetrievalAdapterOptions = {}) {
    if (!(config instanceof ReviewedNcbiGeneRetrievalConfig)) fail("NCBI Gene adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("NCBI Gene injected transport is malformed");
    if (options.fetch && config.transportId === BUILTIN_NCBI_GENE_TRANSPORT_ID) fail("NCBI Gene injected transport requires a distinct reviewed identity");
    if (!options.fetch && (config.transportId !== BUILTIN_NCBI_GENE_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_NCBI_GENE_TRANSPORT_CONFIG_DIGEST)) fail("NCBI Gene built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; Object.freeze(this);
  }
  prepare(): ReviewedNcbiGeneRetrievalPlan { return ReviewedNcbiGeneRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedNcbiGeneRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedNcbiGeneRetrievalResult> {
    if (!(plan instanceof ReviewedNcbiGeneRetrievalPlan)) fail("NCBI Gene execution requires an exact reviewed plan");
    plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("NCBI Gene execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("NCBI Gene dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("NCBI Gene retrievedAt", options.retrievedAt);
    const url = requestUrl(this.config); let raw: unknown;
    try {
      await acquireNcbiRequestSlot();
      raw = this.fetcher ? await this.fetcher(url, new NativeAbortController().signal) : await builtinFetch(url, this.config.timeoutMs);
    } catch (error) { if (error instanceof ReviewedNcbiGeneRetrievalError) throw error; throw new ReviewedNcbiGeneRetrievalError("NCBI Gene request failed"); }
    const parsed = parseResponse(raw); validateResponseCoverage(parsed.payload, this.config.geneSymbols);
    const genes = this.config.geneSymbols.map((symbol) => normalizeGene(parsed.payload, symbol));
    const contentSha = digestJsonSync(genes);
    const ids = this.config.geneSymbols.map((symbol) => REVIEWED_NCBI_GENE_CATALOGUE[symbol]);
    const source = { source_id: "ncbi_gene", authority: REVIEWED_NCBI_GENE_AUTHORITY, uri: `https://www.ncbi.nlm.nih.gov/gene/?term=${ids.join(",")}`, retrieved_at: retrievedAt, content_sha256: contentSha, record_count: genes.length, provider: "none", credentials: "none", limitations: [...LIMITATIONS] };
    const sources = [source]; const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned = { schema: REVIEWED_NCBI_GENE_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, genes, source_set_digest: sourceSetDigest, gene_count: genes.length, catalogue_coverage: "complete", provider: "none", credentials: "none", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_NCBI_GENE_BUNDLE_BYTES) fail("NCBI Gene bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned = { schema: REVIEWED_NCBI_GENE_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, catalogue_digest: plan.catalogueDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: 1, gene_count: genes.length, request_count: 1, response_bytes: parsed.bytes, catalogue_coverage: "complete", retrieved_at: retrievedAt, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "none", limitations: [...LIMITATIONS] };
    return new ReviewedNcbiGeneRetrievalResult(bundle as unknown as Record<string, JsonValue>, { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) } as unknown as Record<string, JsonValue>);
  }
}
export interface ReviewedNcbiGeneExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true;
  retrieved_at: string | null; retention: "metadata_only"; credentials: "none"; metadata_digest: string;
}
export function createReviewedNcbiGeneExecutionMetadata(plan: ReviewedNcbiGeneRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedNcbiGeneExecutionMetadata {
  if (!(plan instanceof ReviewedNcbiGeneRetrievalPlan)) fail("NCBI Gene execution metadata requires an exact plan");
  plan.validate(); if (approveSourceDispatch !== true) fail("NCBI Gene execution metadata requires literal approval");
  const payload = { schema: REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("NCBI Gene retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "none" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}
function validateTransient(value: unknown, plan: ReviewedNcbiGeneRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("NCBI Gene transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_NCBI_GENE_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_gene_metadata") fail("NCBI Gene transient value is malformed");
  const bundle = exactObject("NCBI Gene transient bundle", transient.bundle, ["schema", "generated_at", "sources", "genes", "source_set_digest", "gene_count", "catalogue_coverage", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("NCBI Gene transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "catalogue_digest", "bundle_digest", "source_set_digest", "source_count", "gene_count", "request_count", "response_bytes", "catalogue_coverage", "retrieved_at", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleUnsigned = Object.fromEntries(Object.entries(bundle).filter(([key]) => key !== "bundle_digest"));
  const receiptUnsigned = Object.fromEntries(Object.entries(receipt).filter(([key]) => key !== "receipt_digest"));
  if (digestJsonSync(bundleUnsigned) !== digest("NCBI Gene bundle digest", bundle.bundle_digest) || digestJsonSync(receiptUnsigned) !== digest("NCBI Gene receipt digest", receipt.receipt_digest)) fail("NCBI Gene transient digest is invalid");
  if (bundle.schema !== REVIEWED_NCBI_GENE_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_NCBI_GENE_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.catalogue_digest !== plan.catalogueDigest) fail("NCBI Gene transient identity differs from its reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "none" || receipt.retention !== RETENTION || bundle.provider !== "none" || bundle.credentials !== "none") fail("NCBI Gene transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("NCBI Gene transient limitations are invalid");
  const retrievedAt = timestamp("NCBI Gene retrievedAt", receipt.retrieved_at);
  if (timestamp("NCBI Gene generatedAt", bundle.generated_at) !== retrievedAt) fail("NCBI Gene bundle and receipt timestamps do not match");
  if (!Array.isArray(bundle.genes) || !Array.isArray(bundle.sources) || bundle.genes.length !== plan.config.geneSymbols.length || bundle.sources.length !== 1) fail("NCBI Gene transient catalogue coverage is incomplete");
  const genes = bundle.genes as Record<string, unknown>[];
  for (const [index, symbol] of plan.config.geneSymbols.entries()) {
    const gene = exactObject("NCBI Gene transient record", genes[index], ["source_id", "gene_id", "symbol", "description", "chromosome", "map_location", "aliases", "organism_taxid"]);
    if (gene.source_id !== "ncbi_gene" || gene.gene_id !== REVIEWED_NCBI_GENE_CATALOGUE[symbol] || gene.symbol !== symbol || gene.organism_taxid !== 9606 || text("NCBI Gene description", gene.description, 2_000) !== gene.description || text("NCBI Gene chromosome", gene.chromosome, 64, true) !== gene.chromosome || text("NCBI Gene map location", gene.map_location, 128, true) !== gene.map_location) fail("NCBI Gene transient record differs from its reviewed catalogue");
    if (!Array.isArray(gene.aliases) || gene.aliases.length > 128 || canonicalJson(gene.aliases) !== canonicalJson([...new Set(gene.aliases)].sort()) || gene.aliases.some((alias) => text("NCBI Gene alias", alias, 256) !== alias)) fail("NCBI Gene transient aliases are malformed");
  }
  const ids = plan.config.geneSymbols.map((symbol) => REVIEWED_NCBI_GENE_CATALOGUE[symbol]);
  const expectedSource = { source_id: "ncbi_gene", authority: REVIEWED_NCBI_GENE_AUTHORITY, uri: `https://www.ncbi.nlm.nih.gov/gene/?term=${ids.join(",")}`, retrieved_at: retrievedAt, content_sha256: digestJsonSync(genes), record_count: genes.length, provider: "none", credentials: "none", limitations: [...LIMITATIONS] };
  const source = exactObject("NCBI Gene transient source", bundle.sources[0], ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"]);
  const sourceSetDigest = digestJsonSync([expectedSource]);
  if (canonicalJson(source) !== canonicalJson(expectedSource) || bundle.source_set_digest !== sourceSetDigest || receipt.source_set_digest !== sourceSetDigest || receipt.bundle_digest !== bundle.bundle_digest) fail("NCBI Gene transient source binding is invalid");
  if (receipt.source_count !== 1 || receipt.gene_count !== genes.length || bundle.gene_count !== genes.length || receipt.request_count !== 1 || receipt.catalogue_coverage !== "complete" || bundle.catalogue_coverage !== "complete") fail("NCBI Gene transient counts or coverage are inconsistent");
  integer("NCBI Gene response_bytes", receipt.response_bytes, 1, MAX_REVIEWED_NCBI_GENE_RESPONSE_BYTES);
  return { bundle, receipt };
}
export function createReviewedNcbiGeneAutonomousEvidenceRegistration(adapter: ReviewedNcbiGeneRetrievalAdapter, plan: ReviewedNcbiGeneRetrievalPlan): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (!(adapter instanceof ReviewedNcbiGeneRetrievalAdapter) || !(plan instanceof ReviewedNcbiGeneRetrievalPlan)) fail("NCBI Gene registration requires exact adapter and plan values");
  plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON())) fail("NCBI Gene registration config differs from its reviewed plan");
  const frozenPlan = ReviewedNcbiGeneRetrievalPlan.create(plan.config); const expectedNetwork = adapter.config.transportId === BUILTIN_NCBI_GENE_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: "reviewed.ncbi_gene", version: REVIEWED_NCBI_GENE_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["gene_catalogue_metadata", "source_provenance"], sourceKinds: ["ncbi_gene_summary_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request;
      if (!request || request.source_id !== "ncbi_gene" || request.source_digest !== frozenPlan.planDigest) fail("NCBI Gene acquisition request does not match its reviewed source");
      const metadata = exactObject("NCBI Gene execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("NCBI Gene metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_NCBI_GENE_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "none") fail("NCBI Gene execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("NCBI Gene retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("NCBI Gene projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}
