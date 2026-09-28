/** Reviewed, bounded Open Targets disease-to-gene association metadata retrieval. */

import { ArgumentError } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonValue } from "./types.js";

export const REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA = "bioprism-reviewed-open-targets-config/0.1" as const;
export const REVIEWED_OPEN_TARGETS_PLAN_SCHEMA = "bioprism-reviewed-open-targets-plan/0.1" as const;
export const REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-open-targets-source-receipt/0.1" as const;
export const REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA = "bioprism-reviewed-open-targets-bundle/0.1" as const;
export const REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA = "bioprism-reviewed-open-targets-receipt/0.1" as const;
export const REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA = "bioprism-reviewed-open-targets-transient/0.1" as const;
export const REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-open-targets-execution-metadata/0.1" as const;
export const REVIEWED_OPEN_TARGETS_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_OPEN_TARGETS_HOST = "api.platform.opentargets.org" as const;
export const REVIEWED_OPEN_TARGETS_ENDPOINT = `https://${REVIEWED_OPEN_TARGETS_HOST}/api/v4/graphql` as const;
export const REVIEWED_OPEN_TARGETS_AUTHORITY = "Open Targets Platform" as const;
export const REVIEWED_OPEN_TARGETS_LANES = Object.freeze({ gbm: "MONDO_0018177", lgg: "MONDO_0021637" } as const);
export type ReviewedOpenTargetsLane = keyof typeof REVIEWED_OPEN_TARGETS_LANES;
export const MAX_REVIEWED_OPEN_TARGETS_LANES = 2;
export const MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE = 50;
export const MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES = 512_000;
export const MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES = 1_024_000;
export const MAX_REVIEWED_OPEN_TARGETS_BUNDLE_BYTES = 1_000_000;
export const MAX_REVIEWED_OPEN_TARGETS_TREE_DEPTH = 20;
export const MAX_REVIEWED_OPEN_TARGETS_TREE_NODES = 12_000;
export const BUILTIN_OPEN_TARGETS_TRANSPORT_ID = "builtin.open-targets.fetch" as const;
export const BUILTIN_OPEN_TARGETS_TRANSPORT_VERSION = "1" as const;

const QUERY = "query ReviewedDiseaseAssociations($efoId: String!, $pageIndex: Int!, $pageSize: Int!) { disease(efoId: $efoId) { id name associatedTargets(page: { index: $pageIndex, size: $pageSize }) { count rows { target { id approvedSymbol } score } } } }";
export const BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_OPEN_TARGETS_HOST,
  path: "/api/v4/graphql", method: "POST", content_type: "application/json", user_agent: "AURORA-Prism-SDK/0.1",
  query_digest: digestJsonSync(QUERY), variables: ["efoId", "pageIndex", "pageSize"], page_index: 0,
  redirects: "refused", credentials: "not_accepted",
});

const RETENTION = "bounded_transient_target_ranking_metadata;autonomous_evidence_digest_only" as const;
const LIMITATIONS = Object.freeze([
  "Open Targets association scores are ranking aids and are not confidence values",
  "disease association pages may include indirect ontology-propagated evidence",
  "only the first bounded source-ranked page is retrieved; omitted association rows remain explicit",
  "the fixed glioblastoma and low-grade glioma catalogue is not an exhaustive disease search",
  "source rankings and counts can change with Open Targets data releases",
  "independent review is required for evidence quality, freshness, omissions, and applicability",
  "the adapter does not retrieve patient-level, clinical outcome, or treatment-response data",
  "caller-injected transports own timeout, redirect, and network policy under their declared identity",
]);
const LANE_TO_QUERY: Readonly<Record<ReviewedOpenTargetsLane, { diseaseId: string; nameFragment: string }>> = Object.freeze({
  gbm: { diseaseId: "MONDO_0018177", nameFragment: "glioblastoma" },
  lgg: { diseaseId: "MONDO_0021637", nameFragment: "glioma" },
});
const IDENTIFIER_RE = /^[A-Za-z0-9_.:-]{1,128}$/;
const ENSG_RE = /^ENSG[0-9]{11}$/;
const SYMBOL_RE = /^[A-Z0-9][A-Z0-9._-]{0,63}$/;
const DIGEST_RE = /^[0-9a-f]{64}$/;
const NativeObjectKeys = Object.keys.bind(Object);
const NativeObjectValues = Object.values.bind(Object);
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
const NativeFetch = typeof globalThis.fetch === "function" ? globalThis.fetch.bind(globalThis) : undefined;
const NativeAbortController = globalThis.AbortController;
const NativePromiseRace = globalThis.Promise.race.bind(globalThis.Promise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;
const NativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const NativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const Encoder = new NativeTextEncoder();

export class ReviewedOpenTargetsRetrievalError extends ArgumentError {
  override readonly name = "ReviewedOpenTargetsRetrievalError";
}
function fail(message: string): never { throw new ReviewedOpenTargetsRetrievalError(message); }
function byteLength(value: string): number { return Encoder.encode(value).byteLength; }
function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST_RE.test(value)) fail(`${name} is invalid`);
  return value;
}
function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!NativeIsSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} is outside its bounded integer range`);
  return value as number;
}
function text(name: string, value: unknown, maximum: number): string {
  if (typeof value !== "string") fail(`${name} must be text`);
  const normalized = (value as string).replace(/[\u0009-\u000d\u001c-\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/gu, " ").replace(/^ | $/g, "");
  if (!normalized || /[\u0000-\u001f\u007f]/.test(normalized) || byteLength(normalized) > maximum || /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(normalized)) fail(`${name} is empty or exceeds its text bound`);
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
    if (nodes > MAX_REVIEWED_OPEN_TARGETS_TREE_NODES || next.depth > MAX_REVIEWED_OPEN_TARGETS_TREE_DEPTH) fail("Open Targets response exceeds its structural bound");
    const value = next.value;
    if (typeof value === "number" && (!NativeIsFinite(value) || (NativeIsInteger(value) && !NativeIsSafeInteger(value)))) fail("Open Targets response contains an invalid number");
    if (value !== null && typeof value === "object") {
      if (seen.has(value)) fail("Open Targets response is not a JSON tree"); seen.add(value);
      if (Array.isArray(value)) for (const child of value) pending.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(value) !== Object.prototype) fail("Open Targets response contains a non-JSON object");
        for (const key of NativeObjectKeys(value)) if (/[\u0000-\u001f\u007f]/.test(key) || /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(key)) fail("Open Targets response contains an invalid key");
        for (const child of NativeObjectValues(value as Record<string, unknown>)) pending.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof value === "string" && /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(value)) fail("Open Targets response contains invalid Unicode");
    else if (value !== null && !["string", "number", "boolean"].includes(typeof value)) fail("Open Targets response contains an unsupported value");
  }
}
/** Scan JSON before parsing so duplicate field names are rejected in every runtime. */
function parseStrictJson(source: string): unknown {
  let index = 0; let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index; if (source[index] !== "\"") fail("Open Targets response is not valid JSON"); index += 1;
    while (index < source.length) {
      if (source[index] === "\"") { index += 1; try { return NativeJsonParse(source.slice(start, index)) as string; } catch { return fail("Open Targets response is not valid JSON"); } }
      if (source[index] === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 32) fail("Open Targets response is not valid JSON"); index += 1;
    }
    return fail("Open Targets response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1; if (nodes > MAX_REVIEWED_OPEN_TARGETS_TREE_NODES || depth > MAX_REVIEWED_OPEN_TARGETS_TREE_DEPTH) fail("Open Targets response exceeds its structural bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace(); const keys = new NativeSet<string>(); if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string(); if (keys.has(key)) fail("Open Targets response contains duplicate JSON fields"); keys.add(key); whitespace();
        if (source[index] !== ":") fail("Open Targets response is not valid JSON"); index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; } if (source[index] !== ",") fail("Open Targets response is not valid JSON"); index += 1;
      }
      return fail("Open Targets response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace(); if (source[index] === "]") { index += 1; return; }
      while (index < source.length) { scan(depth + 1); whitespace(); if (source[index] === "]") { index += 1; return; } if (source[index] !== ",") fail("Open Targets response is not valid JSON"); index += 1; }
      return fail("Open Targets response is not valid JSON");
    }
    const start = index; while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (start === index) fail("Open Targets response is not valid JSON"); try { NativeJsonParse(source.slice(start, index)); } catch { fail("Open Targets response is not valid JSON"); }
  };
  scan(0); whitespace(); if (index !== source.length) fail("Open Targets response is not valid JSON");
  const parsed = NativeJsonParse(source) as unknown; validateTree(parsed); return parsed;
}
function parseResponse(value: unknown): { payload: Record<string, unknown>; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) { try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("Open Targets response is not valid UTF-8"); } }
  else if (typeof value === "object" && value !== null && !Array.isArray(value)) { try { body = NativeJsonStringify(value) as string; } catch { return fail("Open Targets injected response is not JSON"); } }
  else return fail("Open Targets transport returned an unsupported response type");
  const bytes = byteLength(body); if (bytes > MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES) fail("Open Targets response exceeds its byte bound");
  const parsed = parseStrictJson(body); if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) fail("Open Targets response root is not an object");
  return { payload: parsed as Record<string, unknown>, bytes };
}
function requestBody(diseaseId: string, pageSize: number): string {
  const body = canonicalJson({ query: QUERY, variables: { efoId: diseaseId, pageIndex: 0, pageSize } });
  if (byteLength(body) > 8_192) fail("Open Targets request exceeds its byte bound"); return body;
}
async function builtinFetch(body: string, timeoutMs: number): Promise<Uint8Array> {
  if (NativeFetch === undefined || NativeAbortController === undefined) fail("Open Targets built-in fetch is unavailable");
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedOpenTargetsRetrievalError("Open Targets association request timed out")); }, timeoutMs); });
  try {
    const operation = NativeFetch(REVIEWED_OPEN_TARGETS_ENDPOINT, { method: "POST", headers: { Accept: "application/json", "Content-Type": "application/json", "User-Agent": "AURORA-Prism-SDK/0.1" }, body, redirect: "error", signal: controller.signal }).then(async (response) => {
      if (!response.ok || response.url !== REVIEWED_OPEN_TARGETS_ENDPOINT || response.redirected || !response.body) fail("Open Targets endpoint returned an unexpected response");
      const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let total = 0;
      while (true) { const { done, value } = await reader.read(); if (done) break; if (!(value instanceof Uint8Array) || total + value.byteLength > MAX_REVIEWED_OPEN_TARGETS_RESPONSE_BYTES) { try { await reader.cancel(); } catch { /* Keep the bounded refusal. */ } fail("Open Targets response exceeds its byte bound"); } chunks.push(value); total += value.byteLength; }
      const result = new Uint8Array(total); let offset = 0; for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.byteLength; } return result;
    });
    return await NativePromiseRace([operation, timeout]);
  } catch (error) { if (error instanceof ReviewedOpenTargetsRetrievalError) throw error; throw new ReviewedOpenTargetsRetrievalError("Open Targets association request failed"); }
  finally { if (timer !== undefined) NativeClearTimeout(timer); }
}
function normalizeAssociation(raw: unknown, lane: ReviewedOpenTargetsLane, pageSize: number): Record<string, JsonValue> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("Open Targets GraphQL response is malformed or contains errors");
  const root = raw as Record<string, unknown>; if (NativeObjectKeys(root).length !== 1 || !NativeOwn(root, "data")) fail("Open Targets GraphQL response is malformed or contains errors");
  if (typeof root.data !== "object" || root.data === null || Array.isArray(root.data)) fail("Open Targets response omitted the disease entity");
  const data = root.data as Record<string, unknown>; if (NativeObjectKeys(data).length !== 1 || !NativeOwn(data, "disease")) fail("Open Targets response omitted the disease entity");
  const disease = data.disease; const expected = LANE_TO_QUERY[lane];
  if (typeof disease !== "object" || disease === null || Array.isArray(disease)) fail("Open Targets disease identity is malformed");
  const entity = disease as Record<string, unknown>; if (NativeObjectKeys(entity).length !== 3 || !NativeOwn(entity, "id") || !NativeOwn(entity, "name") || !NativeOwn(entity, "associatedTargets") || entity.id !== expected.diseaseId) fail("Open Targets disease identity differs from the reviewed lane");
  const diseaseName = text("Open Targets disease name", entity.name, 256); if (!diseaseName.toLowerCase().includes(expected.nameFragment)) fail("Open Targets disease name does not match the reviewed lane");
  if (typeof entity.associatedTargets !== "object" || entity.associatedTargets === null || Array.isArray(entity.associatedTargets)) fail("Open Targets response omitted association page metadata");
  const page = entity.associatedTargets as Record<string, unknown>; if (NativeObjectKeys(page).length !== 2 || !NativeOwn(page, "count") || !NativeOwn(page, "rows")) fail("Open Targets association page has an unexpected shape");
  const total = integer("Open Targets association count", page.count, 0, 10_000_000);
  if (!Array.isArray(page.rows) || page.rows.length > pageSize || page.rows.length !== Math.min(total, pageSize)) fail("Open Targets association page is incomplete or exceeds its reviewed bound");
  const seen = new NativeSet<string>(); const targets: JsonValue[] = []; let previousScore = 1;
  for (const [index, rawRow] of page.rows.entries()) {
    if (typeof rawRow !== "object" || rawRow === null || Array.isArray(rawRow)) fail("Open Targets association row is malformed");
    const row = rawRow as Record<string, unknown>; if (NativeObjectKeys(row).length !== 2 || !NativeOwn(row, "target") || !NativeOwn(row, "score")) fail("Open Targets association row has an unexpected shape");
    if (typeof row.target !== "object" || row.target === null || Array.isArray(row.target)) fail("Open Targets target identity is malformed");
    const target = row.target as Record<string, unknown>; if (NativeObjectKeys(target).length !== 2 || !NativeOwn(target, "id") || !NativeOwn(target, "approvedSymbol")) fail("Open Targets target identity has an unexpected shape");
    if (typeof target.id !== "string" || !ENSG_RE.test(target.id) || typeof target.approvedSymbol !== "string" || !SYMBOL_RE.test(target.approvedSymbol) || seen.has(target.id)) fail("Open Targets target identity is invalid or duplicated");
    if (typeof row.score !== "number" || !NativeIsFinite(row.score) || row.score < 0 || row.score > previousScore) fail("Open Targets association score is outside its descending bounded range");
    seen.add(target.id); previousScore = row.score;
    targets.push({ rank: index + 1, target_id: target.id, approved_symbol: target.approvedSymbol, association_score: row.score });
  }
  return {
    source_id: `open_targets_${lane}`, lane, disease_id: expected.diseaseId, disease_name: diseaseName,
    total_associations: total, returned_associations: targets.length, omitted_associations: total - targets.length,
    page_size: pageSize, coverage: total === targets.length ? "all_source_rows" : "top_ranked_page_only",
    score_interpretation: "ranking_only_not_confidence", targets,
  };
}

export interface ReviewedOpenTargetsRetrievalConfigOptions {
  lanes?: readonly ReviewedOpenTargetsLane[]; pageSize?: number; timeoutMs?: number;
  transportId?: string; transportVersion?: string; transportConfigDigest?: string;
}
export interface ReviewedOpenTargetsRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA; lanes: ReviewedOpenTargetsLane[]; page_size: number; timeout_ms: number; request_limit: number;
  transport_id: string; transport_version: string; transport_config_digest: string; lane_set_digest: string;
  retention: typeof RETENTION; credentials: "not_accepted";
}
export interface ReviewedOpenTargetsRetrievalConfigJSON extends ReviewedOpenTargetsRetrievalConfigPayload {
  config_digest: string;
}
export class ReviewedOpenTargetsRetrievalConfig {
  readonly lanes: readonly ReviewedOpenTargetsLane[]; readonly pageSize: number; readonly timeoutMs: number;
  readonly transportId: string; readonly transportVersion: string; readonly transportConfigDigest: string; readonly laneSetDigest: string;
  constructor(options: ReviewedOpenTargetsRetrievalConfigOptions = {}) {
    const lanes = options.lanes ?? ["gbm"];
    if (!Array.isArray(lanes) || lanes.length < 1 || lanes.length > MAX_REVIEWED_OPEN_TARGETS_LANES || lanes.some((lane) => typeof lane !== "string" || !NativeOwn(LANE_TO_QUERY, lane)) || new NativeSet(lanes).size !== lanes.length) fail("Open Targets lanes contain an unsupported or duplicate lane");
    this.lanes = NativeFreeze((["gbm", "lgg"] as const).filter((lane) => lanes.includes(lane)));
    this.pageSize = integer("Open Targets pageSize", options.pageSize ?? MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE, 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE);
    this.timeoutMs = integer("Open Targets timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_OPEN_TARGETS_TRANSPORT_ID; this.transportVersion = options.transportVersion ?? BUILTIN_OPEN_TARGETS_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("Open Targets transport identity is invalid");
    this.transportConfigDigest = digest("Open Targets transportConfigDigest", options.transportConfigDigest ?? BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST);
    this.laneSetDigest = digestJsonSync({ diseases: this.lanes.map((lane) => ({ lane, disease_id: LANE_TO_QUERY[lane].diseaseId })) }); NativeFreeze(this);
  }
  get requestLimit(): number { return this.lanes.length; }
  private payload(): ReviewedOpenTargetsRetrievalConfigPayload {
    return { schema: REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA, lanes: [...this.lanes], page_size: this.pageSize, timeout_ms: this.timeoutMs, request_limit: this.requestLimit, transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest, lane_set_digest: this.laneSetDigest, retention: RETENTION, credentials: "not_accepted" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedOpenTargetsRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown): ReviewedOpenTargetsRetrievalConfig {
    const raw = exactObject("Open Targets config", value, ["schema", "lanes", "page_size", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "lane_set_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_OPEN_TARGETS_CONFIG_SCHEMA || !Array.isArray(raw.lanes)) fail("Open Targets config has an invalid shape");
    const config = new ReviewedOpenTargetsRetrievalConfig({ lanes: raw.lanes as ReviewedOpenTargetsLane[], pageSize: raw.page_size as number, timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("Open Targets config is not normalized or its digest is invalid"); return config;
  }
}
export interface ReviewedOpenTargetsRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_OPEN_TARGETS_PLAN_SCHEMA; config: ReviewedOpenTargetsRetrievalConfigJSON; config_digest: string; lane_set_digest: string; request_limit: number;
  scope: "fixed_glioma_disease_top_ranked_association_pages"; execution: "bounded_https_graphql_post_per_lane_after_literal_approval";
  retention: typeof RETENTION; credentials: "not_accepted"; plan_digest: string;
}
export class ReviewedOpenTargetsRetrievalPlan {
  readonly config: ReviewedOpenTargetsRetrievalConfig; readonly configDigest: string; readonly laneSetDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedOpenTargetsRetrievalConfig, configDigest: string, laneSetDigest: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.laneSetDigest = laneSetDigest; this.planDigest = planDigest; NativeFreeze(this); }
  static create(config: ReviewedOpenTargetsRetrievalConfig): ReviewedOpenTargetsRetrievalPlan {
    if (!(config instanceof ReviewedOpenTargetsRetrievalConfig)) fail("Open Targets plan requires an exact config");
    const fields = { schema: REVIEWED_OPEN_TARGETS_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, lane_set_digest: config.laneSetDigest, request_limit: config.requestLimit, scope: "fixed_glioma_disease_top_ranked_association_pages", execution: "bounded_https_graphql_post_per_lane_after_literal_approval", retention: RETENTION, credentials: "not_accepted" };
    return new ReviewedOpenTargetsRetrievalPlan(config, config.configDigest, config.laneSetDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedOpenTargetsRetrievalPlanJSON { return { schema: REVIEWED_OPEN_TARGETS_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, lane_set_digest: this.laneSetDigest, request_limit: this.config.requestLimit, scope: "fixed_glioma_disease_top_ranked_association_pages", execution: "bounded_https_graphql_post_per_lane_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.planDigest }; }
  validate(): void { const expected = ReviewedOpenTargetsRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.laneSetDigest !== expected.laneSetDigest || this.planDigest !== expected.planDigest) fail("Open Targets plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown): ReviewedOpenTargetsRetrievalPlan {
    const raw = exactObject("Open Targets plan", value, ["schema", "config", "config_digest", "lane_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_OPEN_TARGETS_PLAN_SCHEMA) fail("Open Targets plan has an invalid shape");
    const plan = ReviewedOpenTargetsRetrievalPlan.create(ReviewedOpenTargetsRetrievalConfig.fromJSON(raw.config)); if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("Open Targets plan is not normalized or its digest is invalid"); return plan;
  }
}
export type ReviewedOpenTargetsBundle = Record<string, JsonValue>;
export type ReviewedOpenTargetsReceipt = Record<string, JsonValue>;
export type ReviewedOpenTargetsTransient = Record<string, JsonValue>;
export type ReviewedOpenTargetsFetch = (url: string, body: string, signal: AbortSignal) => Promise<unknown> | unknown;
export class ReviewedOpenTargetsRetrievalResult {
  private readonly bundleValue: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundleValue = NativeJsonParse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = NativeJsonParse(canonicalJson(receipt)) as Record<string, JsonValue>; NativeFreeze(this); }
  get bundle(): ReviewedOpenTargetsBundle { return NativeJsonParse(canonicalJson(this.bundleValue)) as ReviewedOpenTargetsBundle; }
  get receipt(): ReviewedOpenTargetsReceipt { return NativeJsonParse(canonicalJson(this.receiptValue)) as ReviewedOpenTargetsReceipt; }
  toJSON(): Record<string, JsonValue> { return { receipt: this.receipt, retention: "digest_metadata_only" }; }
  toTransientJSON(): ReviewedOpenTargetsTransient { return { schema: REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA, bundle: this.bundle, receipt: this.receipt, retention: "caller_owned_transient_association_metadata" }; }
}
export interface ReviewedOpenTargetsRetrievalAdapterOptions { fetch?: ReviewedOpenTargetsFetch }
export class ReviewedOpenTargetsRetrievalAdapter {
  readonly config: ReviewedOpenTargetsRetrievalConfig; private readonly fetcher?: ReviewedOpenTargetsFetch;
  constructor(config: ReviewedOpenTargetsRetrievalConfig, options: ReviewedOpenTargetsRetrievalAdapterOptions = {}) {
    if (!(config instanceof ReviewedOpenTargetsRetrievalConfig)) fail("Open Targets adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("Open Targets injected transport is malformed");
    if (options.fetch !== undefined && config.transportId === BUILTIN_OPEN_TARGETS_TRANSPORT_ID) fail("Open Targets injected transport requires a distinct reviewed identity");
    if (options.fetch === undefined && (config.transportId !== BUILTIN_OPEN_TARGETS_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_OPEN_TARGETS_TRANSPORT_CONFIG_DIGEST)) fail("Open Targets built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; NativeFreeze(this);
  }
  prepare(): ReviewedOpenTargetsRetrievalPlan { return ReviewedOpenTargetsRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedOpenTargetsRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedOpenTargetsRetrievalResult> {
    if (!(plan instanceof ReviewedOpenTargetsRetrievalPlan)) fail("Open Targets execution requires an exact reviewed plan"); plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("Open Targets execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("Open Targets dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("Open Targets retrievedAt", options.retrievedAt);
    const associations: Record<string, JsonValue>[] = []; const sources: Record<string, JsonValue>[] = []; const sourceReceipts: Record<string, JsonValue>[] = []; let responseBytes = 0;
    for (const lane of this.config.lanes) {
      const body = requestBody(LANE_TO_QUERY[lane].diseaseId, this.config.pageSize); let raw: unknown;
      try { raw = this.fetcher ? await this.fetcher(REVIEWED_OPEN_TARGETS_ENDPOINT, body, new NativeAbortController().signal) : await builtinFetch(body, this.config.timeoutMs); }
      catch (error) { if (error instanceof ReviewedOpenTargetsRetrievalError) throw error; throw new ReviewedOpenTargetsRetrievalError("Open Targets association request failed"); }
      const parsed = parseResponse(raw); responseBytes += parsed.bytes; if (responseBytes > MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES) fail("Open Targets aggregate response bytes exceed the plan bound");
      const association = normalizeAssociation(parsed.payload, lane, this.config.pageSize); const associationDigest = digestJsonSync(association); const sourceId = association.source_id as string;
      sources.push({ source_id: sourceId, authority: REVIEWED_OPEN_TARGETS_AUTHORITY, uri: REVIEWED_OPEN_TARGETS_ENDPOINT, retrieved_at: retrievedAt, content_sha256: associationDigest, record_count: association.returned_associations as number, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA, lane, source_id: sourceId, disease_id: LANE_TO_QUERY[lane].diseaseId, content_digest: associationDigest, total_associations: association.total_associations as number, returned_associations: association.returned_associations as number, omitted_associations: association.omitted_associations as number, coverage: association.coverage as string }); associations.push(association);
    }
    const coverage = associations.every((row) => row.coverage === "all_source_rows") ? "all_source_rows" : "top_ranked_pages_only"; const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned = { schema: REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, associations, source_set_digest: sourceSetDigest, association_count: associations.length, coverage, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_OPEN_TARGETS_BUNDLE_BYTES) fail("Open Targets bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned = { schema: REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, lane_set_digest: plan.laneSetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, association_count: associations.length, request_count: associations.length, response_bytes: responseBytes, coverage, retrieved_at: retrievedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    const receipt = { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) };
    return new ReviewedOpenTargetsRetrievalResult(bundle as unknown as Record<string, JsonValue>, receipt as unknown as Record<string, JsonValue>);
  }
}
export interface ReviewedOpenTargetsExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true; retrieved_at: string | null;
  retention: "metadata_only"; credentials: "not_accepted"; metadata_digest: string;
}
export function createReviewedOpenTargetsExecutionMetadata(plan: ReviewedOpenTargetsRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedOpenTargetsExecutionMetadata {
  if (!(plan instanceof ReviewedOpenTargetsRetrievalPlan)) fail("Open Targets execution metadata requires an exact plan"); plan.validate();
  if (approveSourceDispatch !== true) fail("Open Targets execution metadata requires literal approval");
  const payload = { schema: REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("Open Targets retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateTransient(value: unknown, plan: ReviewedOpenTargetsRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("Open Targets transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_OPEN_TARGETS_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_association_metadata") fail("Open Targets transient value is malformed");
  const bundle = exactObject("Open Targets transient bundle", transient.bundle, ["schema", "generated_at", "sources", "associations", "source_set_digest", "association_count", "coverage", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("Open Targets transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "lane_set_digest", "bundle_digest", "source_set_digest", "source_count", "association_count", "request_count", "response_bytes", "coverage", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleUnsigned = Object.fromEntries(Object.entries(bundle).filter(([key]) => key !== "bundle_digest")); const receiptUnsigned = Object.fromEntries(Object.entries(receipt).filter(([key]) => key !== "receipt_digest"));
  if (digestJsonSync(bundleUnsigned) !== digest("Open Targets bundle digest", bundle.bundle_digest) || digestJsonSync(receiptUnsigned) !== digest("Open Targets receipt digest", receipt.receipt_digest)) fail("Open Targets transient digests are invalid");
  if (bundle.schema !== REVIEWED_OPEN_TARGETS_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_OPEN_TARGETS_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.lane_set_digest !== plan.laneSetDigest) fail("Open Targets transient identity differs from the reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "not_accepted" || receipt.retention !== RETENTION || bundle.provider !== "none" || bundle.credentials !== "not_accepted") fail("Open Targets transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("Open Targets transient limitations are invalid");
  const retrievedAt = timestamp("Open Targets retrievedAt", receipt.retrieved_at); if (timestamp("Open Targets generatedAt", bundle.generated_at) !== retrievedAt) fail("Open Targets bundle and receipt timestamps do not match");
  const associations = bundle.associations; const sources = bundle.sources; const sourceReceipts = receipt.source_receipts;
  if (!Array.isArray(associations) || !Array.isArray(sources) || !Array.isArray(sourceReceipts) || associations.length !== plan.config.lanes.length || sources.length !== associations.length || sourceReceipts.length !== associations.length) fail("Open Targets transient lane coverage is incomplete");
  if (digestJsonSync(sources) !== bundle.source_set_digest || receipt.source_set_digest !== bundle.source_set_digest || receipt.bundle_digest !== bundle.bundle_digest) fail("Open Targets transient source binding is invalid");
  for (const [index, lane] of plan.config.lanes.entries()) {
    const association = exactObject("Open Targets association", associations[index], ["source_id", "lane", "disease_id", "disease_name", "total_associations", "returned_associations", "omitted_associations", "page_size", "coverage", "score_interpretation", "targets"]);
    const total = integer("Open Targets association count", association.total_associations, 0, 10_000_000); const returned = integer("Open Targets returned count", association.returned_associations, 0, plan.config.pageSize); const omitted = integer("Open Targets omitted count", association.omitted_associations, 0, 10_000_000);
    if (!Array.isArray(association.targets) || association.targets.length !== returned || returned !== Math.min(total, plan.config.pageSize) || omitted !== total - returned) fail("Open Targets transient association counts are inconsistent");
    const expected = LANE_TO_QUERY[lane]; const diseaseName = text("Open Targets disease name", association.disease_name, 256);
    if (association.lane !== lane || association.disease_id !== expected.diseaseId || !diseaseName.toLowerCase().includes(expected.nameFragment) || association.source_id !== `open_targets_${lane}` || integer("Open Targets page size", association.page_size, 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE) !== plan.config.pageSize || association.score_interpretation !== "ranking_only_not_confidence" || association.coverage !== (total === returned ? "all_source_rows" : "top_ranked_page_only")) fail("Open Targets association differs from its reviewed lane");
    let previousScore = 1; const seen = new NativeSet<string>();
    for (const [rankIndex, rawTarget] of association.targets.entries()) {
      const target = exactObject("Open Targets target", rawTarget, ["rank", "target_id", "approved_symbol", "association_score"]);
      if (integer("Open Targets target rank", target.rank, 1, MAX_REVIEWED_OPEN_TARGETS_PAGE_SIZE) !== rankIndex + 1 || typeof target.target_id !== "string" || !ENSG_RE.test(target.target_id) || seen.has(target.target_id) || typeof target.approved_symbol !== "string" || !SYMBOL_RE.test(target.approved_symbol) || typeof target.association_score !== "number" || !NativeIsFinite(target.association_score) || target.association_score < 0 || target.association_score > previousScore) fail("Open Targets target row identity or score ordering is invalid");
      previousScore = target.association_score; seen.add(target.target_id);
    }
    const contentDigest = digestJsonSync(association);
    const source = exactObject("Open Targets source metadata", sources[index], ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"]);
    const sourceReceipt = exactObject("Open Targets source receipt", sourceReceipts[index], ["schema", "lane", "source_id", "disease_id", "content_digest", "total_associations", "returned_associations", "omitted_associations", "coverage"]);
    const expectedSource = { source_id: `open_targets_${lane}`, authority: REVIEWED_OPEN_TARGETS_AUTHORITY, uri: REVIEWED_OPEN_TARGETS_ENDPOINT, retrieved_at: retrievedAt, content_sha256: contentDigest, record_count: returned, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    const expectedReceipt = { schema: REVIEWED_OPEN_TARGETS_SOURCE_RECEIPT_SCHEMA, lane, source_id: `open_targets_${lane}`, disease_id: expected.diseaseId, content_digest: contentDigest, total_associations: total, returned_associations: returned, omitted_associations: omitted, coverage: association.coverage };
    if (canonicalJson(source) !== canonicalJson(expectedSource) || canonicalJson(sourceReceipt) !== canonicalJson(expectedReceipt)) fail("Open Targets source metadata does not match its association record");
  }
  const coverage = associations.every((row) => (row as Record<string, unknown>).coverage === "all_source_rows") ? "all_source_rows" : "top_ranked_pages_only";
  if (integer("Open Targets bundle association count", bundle.association_count, 1, MAX_REVIEWED_OPEN_TARGETS_LANES) !== associations.length || integer("Open Targets receipt association count", receipt.association_count, 1, MAX_REVIEWED_OPEN_TARGETS_LANES) !== associations.length || integer("Open Targets source count", receipt.source_count, 1, MAX_REVIEWED_OPEN_TARGETS_LANES) !== associations.length || integer("Open Targets request count", receipt.request_count, 1, MAX_REVIEWED_OPEN_TARGETS_LANES) !== associations.length || bundle.coverage !== coverage || receipt.coverage !== coverage) fail("Open Targets transient counts or coverage are inconsistent");
  integer("Open Targets response_bytes", receipt.response_bytes, associations.length, MAX_REVIEWED_OPEN_TARGETS_TOTAL_RESPONSE_BYTES);
  return { bundle, receipt };
}
export function createReviewedOpenTargetsAutonomousEvidenceRegistration(adapter: ReviewedOpenTargetsRetrievalAdapter, plan: ReviewedOpenTargetsRetrievalPlan, lane: ReviewedOpenTargetsLane): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (!(adapter instanceof ReviewedOpenTargetsRetrievalAdapter) || !(plan instanceof ReviewedOpenTargetsRetrievalPlan)) fail("Open Targets registration requires exact adapter and plan values"); plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || !plan.config.lanes.includes(lane) || plan.config.lanes.length !== 1) fail("Open Targets registration requires the exact single-lane plan");
  const frozenPlan = ReviewedOpenTargetsRetrievalPlan.create(plan.config); const sourceId = `open_targets_${lane}`; const expectedNetwork = adapter.config.transportId === BUILTIN_OPEN_TARGETS_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.open_targets.${lane}`, version: REVIEWED_OPEN_TARGETS_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["target_disease_association_metadata", "source_provenance"], sourceKinds: ["open_targets_target_disease_association_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request; if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("Open Targets acquisition request does not match its reviewed source");
      const metadata = exactObject("Open Targets execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("Open Targets metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_OPEN_TARGETS_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("Open Targets execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("Open Targets retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("Open Targets projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}
