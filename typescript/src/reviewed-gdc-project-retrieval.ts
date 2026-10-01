/** Reviewed, aggregate-only retrieval of fixed NCI GDC project metadata. */

import { ArgumentError } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonValue } from "./types.js";

export const REVIEWED_GDC_CONFIG_SCHEMA = "bioprism-reviewed-gdc-project-config/0.1" as const;
export const REVIEWED_GDC_PLAN_SCHEMA = "bioprism-reviewed-gdc-project-plan/0.1" as const;
export const REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-gdc-project-source-receipt/0.1" as const;
export const REVIEWED_GDC_BUNDLE_SCHEMA = "bioprism-reviewed-gdc-project-bundle/0.1" as const;
export const REVIEWED_GDC_RECEIPT_SCHEMA = "bioprism-reviewed-gdc-project-receipt/0.1" as const;
export const REVIEWED_GDC_TRANSIENT_SCHEMA = "bioprism-reviewed-gdc-project-transient/0.1" as const;
export const REVIEWED_GDC_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-gdc-project-execution-metadata/0.1" as const;
export const REVIEWED_GDC_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_GDC_HOST = "api.gdc.cancer.gov" as const;
export const REVIEWED_GDC_PATH = "/projects" as const;
export const REVIEWED_GDC_ENDPOINT = `https://${REVIEWED_GDC_HOST}${REVIEWED_GDC_PATH}` as const;
export const REVIEWED_GDC_AUTHORITY = "NCI Genomic Data Commons" as const;
export const REVIEWED_GDC_PROJECTS = Object.freeze({ gbm: "TCGA-GBM", lgg: "TCGA-LGG" } as const);
export type ReviewedGdcProjectId = (typeof REVIEWED_GDC_PROJECTS)[keyof typeof REVIEWED_GDC_PROJECTS];
export const MAX_REVIEWED_GDC_PROJECTS = 2;
export const MAX_REVIEWED_GDC_RESPONSE_BYTES = 1_000_000;
export const MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES = 2_000_000;
export const MAX_REVIEWED_GDC_BUNDLE_BYTES = 1_000_000;
export const MAX_REVIEWED_GDC_TREE_DEPTH = 32;
export const MAX_REVIEWED_GDC_TREE_NODES = 50_000;
export const BUILTIN_GDC_TRANSPORT_ID = "builtin.nci-gdc.fetch" as const;
export const BUILTIN_GDC_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_GDC_HOST,
  path: REVIEWED_GDC_PATH, method: "GET", expand: ["summary", "summary.data_categories"],
  fields: ["project_id", "name", "disease_type", "primary_site", "state", "released", "summary.case_count", "summary.file_count", "summary.data_categories"],
  redirects: "refused", credentials: "not_accepted",
});

const RETENTION = "aggregate_project_metadata_only;no_case_sample_or_file_rows" as const;
const LIMITATIONS = Object.freeze([
  "GDC project summaries are aggregate source metadata, not patient-level evidence or outcome",
  "project and category counts are source-reported and may change between retrievals",
  "the fixed TCGA-GBM and TCGA-LGG catalogue is not an exhaustive glioma cohort search",
  "independent review is required for freshness, omissions, study quality, and applicability",
  "the adapter does not request or retain case, sample, file, sequence, assay, or controlled-access data",
  "caller-injected transports own timeout, redirect, and network policy under their declared identity",
] as const);
const PROJECT_TO_LANE: Readonly<Record<ReviewedGdcProjectId, "gbm" | "lgg">> = Object.freeze({ "TCGA-GBM": "gbm", "TCGA-LGG": "lgg" });
const DIGEST_RE = /^[0-9a-f]{64}$/;
const IDENTIFIER_RE = /^[A-Za-z0-9_.:-]{1,128}$/;
const NativeFetch = globalThis.fetch;
const NativeTextEncoder = globalThis.TextEncoder;
const NativeTextDecoder = globalThis.TextDecoder;
const NativeAbortController = globalThis.AbortController;
const NativeDate = globalThis.Date;
const NativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const NativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const NativeJsonParse = globalThis.JSON.parse.bind(globalThis.JSON);
const NativeJsonStringify = globalThis.JSON.stringify.bind(globalThis.JSON);
const NativeObjectKeys = globalThis.Object.keys.bind(globalThis.Object);
const NativeObjectValues = globalThis.Object.values.bind(globalThis.Object);
const NativeOwn = Function.prototype.call.bind(Object.prototype.hasOwnProperty) as (value: object, key: PropertyKey) => boolean;
const NativeObjectFreeze = globalThis.Object.freeze.bind(globalThis.Object);
const NativePromiseRace = globalThis.Promise.race.bind(globalThis.Promise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;
const NativeIsFinite = globalThis.Number.isFinite.bind(globalThis.Number);
const NativeIsInteger = globalThis.Number.isInteger.bind(globalThis.Number);
const NativeIsSafeInteger = globalThis.Number.isSafeInteger.bind(globalThis.Number);
const Encoder = new NativeTextEncoder();

export class ReviewedGdcRetrievalError extends ArgumentError {
  override readonly name = "ReviewedGdcRetrievalError";
}
function fail(message: string): never { throw new ReviewedGdcRetrievalError(message); }
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
    if (nodes > MAX_REVIEWED_GDC_TREE_NODES || next.depth > MAX_REVIEWED_GDC_TREE_DEPTH) fail("GDC response exceeds its structural bound");
    const current = next.value;
    if (typeof current === "number" && (!NativeIsFinite(current) || (NativeIsInteger(current) && !NativeIsSafeInteger(current)))) fail("GDC response contains an invalid number");
    if (current !== null && typeof current === "object") {
      if (seen.has(current)) fail("GDC response is not a JSON tree");
      seen.add(current);
      if (Array.isArray(current)) for (const child of current) pending.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(current) !== Object.prototype) fail("GDC response contains a non-JSON object");
        for (const key of NativeObjectKeys(current)) {
          if (/[\u0000-\u001f\u007f]/.test(key)) fail("GDC response contains an invalid key");
          if (/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(key)) fail("GDC response contains invalid Unicode");
        }
        for (const child of NativeObjectValues(current as Record<string, unknown>)) pending.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof current === "string" && /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(current)) fail("GDC response contains invalid Unicode");
    else if (current !== null && !["string", "number", "boolean"].includes(typeof current)) fail("GDC response contains an unsupported value");
  }
}

/** Scan JSON syntax before JSON.parse so duplicate names cannot be silently overwritten. */
function parseStrictJson(source: string): unknown {
  let index = 0; let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index;
    if (source[index] !== "\"") fail("GDC response is not valid JSON");
    index += 1;
    while (index < source.length) {
      if (source[index] === "\"") { index += 1; try { return NativeJsonParse(source.slice(start, index)) as string; } catch { return fail("GDC response is not valid JSON"); } }
      if (source[index] === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 32) fail("GDC response is not valid JSON");
      index += 1;
    }
    return fail("GDC response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1;
    if (nodes > MAX_REVIEWED_GDC_TREE_NODES || depth > MAX_REVIEWED_GDC_TREE_DEPTH) fail("GDC response exceeds its structural bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace(); const keys = new Set<string>();
      if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string();
        if (keys.has(key)) fail("GDC response contains duplicate JSON fields");
        keys.add(key); whitespace(); if (source[index] !== ":") fail("GDC response is not valid JSON");
        index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; }
        if (source[index] !== ",") fail("GDC response is not valid JSON"); index += 1;
      }
      return fail("GDC response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace(); if (source[index] === "]") { index += 1; return; }
      while (index < source.length) {
        scan(depth + 1); whitespace();
        if (source[index] === "]") { index += 1; return; }
        if (source[index] !== ",") fail("GDC response is not valid JSON"); index += 1;
      }
      return fail("GDC response is not valid JSON");
    }
    const start = index;
    while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (index === start) fail("GDC response is not valid JSON");
    try { NativeJsonParse(source.slice(start, index)); } catch { fail("GDC response is not valid JSON"); }
  };
  scan(0); whitespace();
  if (index !== source.length) fail("GDC response is not valid JSON");
  const parsed = NativeJsonParse(source) as unknown; validateTree(parsed); return parsed;
}
function parseResponse(value: unknown): { payload: Record<string, unknown>; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) {
    try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("GDC response is not valid UTF-8"); }
  } else if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    try { body = NativeJsonStringify(value) as string; } catch { return fail("GDC injected response is not JSON"); }
  } else return fail("GDC transport returned an unsupported response type");
  const size = byteLength(body);
  if (size > MAX_REVIEWED_GDC_RESPONSE_BYTES) fail("GDC response exceeds its byte bound");
  const parsed = parseStrictJson(body);
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) fail("GDC response root is not an object");
  return { payload: parsed as Record<string, unknown>, bytes: size };
}
function queryUrl(projectId: string): string {
  const parameters = new URLSearchParams({ format: "json", expand: "summary,summary.data_categories", fields: "project_id,name,disease_type,primary_site,state,released,summary.case_count,summary.file_count,summary.data_categories" });
  const url = `${REVIEWED_GDC_ENDPOINT}/${projectId}?${parameters.toString()}`;
  if (!url.startsWith(`${REVIEWED_GDC_ENDPOINT}/`) || byteLength(url) > 8_192) fail("GDC request escaped its reviewed endpoint");
  return url;
}
async function builtinFetch(url: string, timeoutMs: number): Promise<Uint8Array> {
  if (NativeFetch === undefined || NativeAbortController === undefined) fail("GDC built-in fetch is unavailable");
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedGdcRetrievalError("GDC project request timed out")); }, timeoutMs); });
  try {
    const request = NativeFetch(url, { method: "GET", headers: { Accept: "application/json" }, redirect: "error", signal: controller.signal });
    const operation = request.then(async (response) => {
      if (!response.ok || response.url !== url || response.redirected || !response.body) fail("GDC endpoint returned an unexpected response");
      const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let total = 0;
      while (true) {
        const { done, value } = await reader.read(); if (done) break;
        if (!(value instanceof Uint8Array) || total + value.byteLength > MAX_REVIEWED_GDC_RESPONSE_BYTES) { try { await reader.cancel(); } catch { /* Preserve bounded refusal. */ } fail("GDC response exceeds its byte bound"); }
        chunks.push(value); total += value.byteLength;
      }
      const body = new Uint8Array(total); let offset = 0;
      for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
      return body;
    });
    return await NativePromiseRace([operation, timeout]);
  } catch (error) {
    if (error instanceof ReviewedGdcRetrievalError) throw error;
    throw new ReviewedGdcRetrievalError("GDC project request failed");
  } finally { if (timer !== undefined) NativeClearTimeout(timer); }
}
function count(name: string, value: unknown, optional = false): number | null {
  if ((value === null || value === undefined) && optional) return null;
  return integer(name, value, 0, 100_000_000);
}
function textList(name: string, value: unknown): string[] {
  if (value === null || value === undefined) return [];
  if (!Array.isArray(value) || value.length > 64) fail(`${name} exceeds its list bound`);
  return [...new Set(value.map((item) => text(name, item, 512)!))].sort();
}
function normalizeProject(raw: unknown, projectId: ReviewedGdcProjectId): Record<string, JsonValue> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("GDC response is malformed");
  const data = (raw as Record<string, unknown>).data;
  if (typeof data !== "object" || data === null || Array.isArray(data) || (data as Record<string, unknown>).project_id !== projectId) fail("GDC project identity does not match the reviewed project");
  const project = data as Record<string, unknown>; const summary = project.summary;
  if (typeof summary !== "object" || summary === null || Array.isArray(summary)) fail("GDC project response omitted summary metadata");
  const summaryRow = summary as Record<string, unknown>;
  const caseCount = count("GDC case_count", summaryRow.case_count, true); const fileCount = count("GDC file_count", summaryRow.file_count, true);
  const rawCategories = summaryRow.data_categories ?? [];
  if (!Array.isArray(rawCategories) || rawCategories.length > 256) fail("GDC data categories exceed their list bound");
  const categories = new Map<string, Record<string, JsonValue>>();
  for (const item of rawCategories) {
    if (typeof item !== "object" || item === null || Array.isArray(item)) fail("GDC data category is malformed");
    const row = item as Record<string, unknown>; const category = text("GDC data category", row.data_category, 512)!;
    if (categories.has(category)) fail("GDC project contains duplicate data categories");
    categories.set(category, { data_category: category, case_count: count("GDC category case_count", row.case_count, true), file_count: count("GDC category file_count", row.file_count, true) });
  }
  const released = project.released ?? null;
  if (released !== null && typeof released !== "boolean") fail("GDC released must be a boolean");
  const lane = PROJECT_TO_LANE[projectId];
  return {
    source_id: `gdc_${lane}`, lane, project_id: projectId,
    name: text("GDC project name", project.name, 2_000)!, disease_types: textList("GDC disease_type", project.disease_type),
    primary_sites: textList("GDC primary_site", project.primary_site), state: text("GDC project state", project.state, 128, true),
    released, case_count: caseCount, file_count: fileCount,
    data_categories: [...categories.keys()].sort().map((key) => categories.get(key)!),
    metadata_completeness: caseCount !== null && fileCount !== null && released !== null ? "complete" : "unknown",
  };
}

export interface ReviewedGdcRetrievalConfigOptions {
  projectIds?: readonly ReviewedGdcProjectId[];
  timeoutMs?: number;
  transportId?: string;
  transportVersion?: string;
  transportConfigDigest?: string;
}
export interface ReviewedGdcRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GDC_CONFIG_SCHEMA; project_ids: ReviewedGdcProjectId[]; timeout_ms: number; request_limit: number;
  transport_id: string; transport_version: string; transport_config_digest: string; project_set_digest: string;
  retention: typeof RETENTION; credentials: "not_accepted";
}
export interface ReviewedGdcRetrievalConfigJSON extends ReviewedGdcRetrievalConfigPayload {
  config_digest: string;
}
export class ReviewedGdcRetrievalConfig {
  readonly projectIds: readonly ReviewedGdcProjectId[]; readonly timeoutMs: number; readonly transportId: string; readonly transportVersion: string; readonly transportConfigDigest: string; readonly projectSetDigest: string;
  constructor(options: ReviewedGdcRetrievalConfigOptions = {}) {
    const projects = options.projectIds ?? ["TCGA-GBM"];
    if (!Array.isArray(projects) || projects.length < 1 || projects.length > MAX_REVIEWED_GDC_PROJECTS || projects.some((project) => typeof project !== "string" || !Object.values(REVIEWED_GDC_PROJECTS).includes(project as ReviewedGdcProjectId)) || new Set(projects).size !== projects.length) fail("GDC projectIds contain an unsupported or duplicate project");
    this.projectIds = Object.freeze((Object.values(REVIEWED_GDC_PROJECTS) as ReviewedGdcProjectId[]).filter((project) => projects.includes(project)));
    this.timeoutMs = integer("GDC timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_GDC_TRANSPORT_ID; this.transportVersion = options.transportVersion ?? BUILTIN_GDC_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("GDC transport identity is invalid");
    this.transportConfigDigest = digest("GDC transportConfigDigest", options.transportConfigDigest ?? BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST);
    this.projectSetDigest = digestJsonSync({ projects: this.projectIds.map((projectId) => ({ lane: PROJECT_TO_LANE[projectId], project_id: projectId })) });
    NativeObjectFreeze(this);
  }
  get requestLimit(): number { return this.projectIds.length; }
  private payload(): ReviewedGdcRetrievalConfigPayload {
    return { schema: REVIEWED_GDC_CONFIG_SCHEMA, project_ids: [...this.projectIds], timeout_ms: this.timeoutMs, request_limit: this.requestLimit, transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest, project_set_digest: this.projectSetDigest, retention: RETENTION, credentials: "not_accepted" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedGdcRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown): ReviewedGdcRetrievalConfig {
    const raw = exactObject("GDC config", value, ["schema", "project_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "project_set_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_GDC_CONFIG_SCHEMA || !Array.isArray(raw.project_ids)) fail("GDC config has an invalid shape");
    const config = new ReviewedGdcRetrievalConfig({ projectIds: raw.project_ids as ReviewedGdcProjectId[], timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("GDC config is not normalized or its digest is invalid");
    return config;
  }
}

export interface ReviewedGdcRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GDC_PLAN_SCHEMA; config: ReviewedGdcRetrievalConfigJSON; config_digest: string; project_set_digest: string; request_limit: number;
  scope: "fixed_public_gdc_project_aggregate_metadata"; execution: "bounded_https_get_after_literal_approval"; retention: typeof RETENTION; credentials: "not_accepted"; plan_digest: string;
}
export class ReviewedGdcRetrievalPlan {
  readonly config: ReviewedGdcRetrievalConfig; readonly configDigest: string; readonly projectSetDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedGdcRetrievalConfig, configDigest: string, projectSetDigest: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.projectSetDigest = projectSetDigest; this.planDigest = planDigest; NativeObjectFreeze(this); }
  static create(config: ReviewedGdcRetrievalConfig): ReviewedGdcRetrievalPlan {
    if (!(config instanceof ReviewedGdcRetrievalConfig)) fail("GDC plan requires an exact config");
    const fields = { schema: REVIEWED_GDC_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, project_set_digest: config.projectSetDigest, request_limit: config.requestLimit, scope: "fixed_public_gdc_project_aggregate_metadata", execution: "bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "not_accepted" };
    return new ReviewedGdcRetrievalPlan(config, config.configDigest, config.projectSetDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedGdcRetrievalPlanJSON { return { schema: REVIEWED_GDC_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, project_set_digest: this.projectSetDigest, request_limit: this.config.requestLimit, scope: "fixed_public_gdc_project_aggregate_metadata", execution: "bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.planDigest }; }
  validate(): void { const expected = ReviewedGdcRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.projectSetDigest !== expected.projectSetDigest || this.planDigest !== expected.planDigest) fail("GDC plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown): ReviewedGdcRetrievalPlan {
    const raw = exactObject("GDC plan", value, ["schema", "config", "config_digest", "project_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_GDC_PLAN_SCHEMA) fail("GDC plan has an invalid shape");
    const plan = ReviewedGdcRetrievalPlan.create(ReviewedGdcRetrievalConfig.fromJSON(raw.config));
    if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("GDC plan is not normalized or its digest is invalid");
    return plan;
  }
}

export type GdcProjectMetadata = Record<string, JsonValue>;
export type ReviewedGdcRetrievalReceiptJSON = Record<string, JsonValue>;
export type ReviewedGdcTransientJSON = Record<string, JsonValue>;
export class ReviewedGdcRetrievalResult {
  private readonly bundle: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundle = JSON.parse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = JSON.parse(canonicalJson(receipt)) as Record<string, JsonValue>; NativeObjectFreeze(this); }
  get receipt(): Record<string, JsonValue> { return JSON.parse(canonicalJson(this.receiptValue)) as Record<string, JsonValue>; }
  toJSON(): { receipt: Record<string, JsonValue>; retention: string } { return { receipt: JSON.parse(canonicalJson(this.receiptValue)) as Record<string, JsonValue>, retention: "aggregate_metadata_only" }; }
  toTransientJSON(): ReviewedGdcTransientJSON { return { schema: REVIEWED_GDC_TRANSIENT_SCHEMA, bundle: JSON.parse(canonicalJson(this.bundle)) as JsonValue, receipt: JSON.parse(canonicalJson(this.receiptValue)) as JsonValue, retention: "caller_owned_transient_project_metadata" }; }
}
export type ReviewedGdcFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
export interface ReviewedGdcRetrievalAdapterOptions { fetch?: ReviewedGdcFetch }
export class ReviewedGdcRetrievalAdapter {
  readonly config: ReviewedGdcRetrievalConfig; private readonly fetcher?: ReviewedGdcFetch;
  constructor(config: ReviewedGdcRetrievalConfig, options: ReviewedGdcRetrievalAdapterOptions = {}) {
    if (!(config instanceof ReviewedGdcRetrievalConfig)) fail("GDC adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("GDC injected transport is malformed");
    if (options.fetch && config.transportId === BUILTIN_GDC_TRANSPORT_ID) fail("GDC injected transport requires a distinct reviewed identity");
    if (!options.fetch && (config.transportId !== BUILTIN_GDC_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_GDC_TRANSPORT_CONFIG_DIGEST)) fail("GDC built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; NativeObjectFreeze(this);
  }
  prepare(): ReviewedGdcRetrievalPlan { return ReviewedGdcRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedGdcRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedGdcRetrievalResult> {
    if (!(plan instanceof ReviewedGdcRetrievalPlan)) fail("GDC execution requires an exact reviewed plan");
    plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("GDC execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("GDC dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("GDC retrievedAt", options.retrievedAt);
    const projects: Record<string, JsonValue>[] = []; const sources: Record<string, JsonValue>[] = []; const sourceReceipts: Record<string, JsonValue>[] = []; let responseBytes = 0;
    for (const projectId of this.config.projectIds) {
      const url = queryUrl(projectId); let raw: unknown;
      try { raw = this.fetcher ? await this.fetcher(url, new NativeAbortController().signal) : await builtinFetch(url, this.config.timeoutMs); }
      catch (error) { if (error instanceof ReviewedGdcRetrievalError) throw error; throw new ReviewedGdcRetrievalError("GDC project request failed"); }
      const parsed = parseResponse(raw); responseBytes += parsed.bytes;
      if (responseBytes > MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES) fail("GDC aggregate response bytes exceed the plan bound");
      const project = normalizeProject(parsed.payload, projectId); const projectDigest = digestJsonSync(project); const sourceId = project.source_id as string;
      sources.push({ source_id: sourceId, authority: REVIEWED_GDC_AUTHORITY, uri: `${REVIEWED_GDC_ENDPOINT}/${projectId}`, retrieved_at: retrievedAt, content_sha256: projectDigest, record_count: 1, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA, lane: project.lane as string, source_id: sourceId, project_id: projectId, content_digest: projectDigest, metadata_completeness: project.metadata_completeness as string });
      projects.push(project);
    }
    const completeness = projects.every((project) => project.metadata_completeness === "complete") ? "complete" : "unknown";
    const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned = { schema: REVIEWED_GDC_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, projects, source_set_digest: sourceSetDigest, project_count: projects.length, completeness, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_GDC_BUNDLE_BYTES) fail("GDC bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned = { schema: REVIEWED_GDC_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, project_set_digest: plan.projectSetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, project_count: projects.length, request_count: projects.length, response_bytes: responseBytes, completeness, retrieved_at: retrievedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    const receipt = { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) };
    return new ReviewedGdcRetrievalResult(bundle as unknown as Record<string, JsonValue>, receipt as unknown as Record<string, JsonValue>);
  }
}

export interface ReviewedGdcExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_GDC_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true;
  retrieved_at: string | null; retention: "metadata_only"; credentials: "not_accepted"; metadata_digest: string;
}
export function createReviewedGdcExecutionMetadata(plan: ReviewedGdcRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedGdcExecutionMetadata {
  if (!(plan instanceof ReviewedGdcRetrievalPlan)) fail("GDC execution metadata requires an exact plan");
  plan.validate(); if (approveSourceDispatch !== true) fail("GDC execution metadata requires literal approval");
  const payload = { schema: REVIEWED_GDC_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("GDC retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateTransient(value: unknown, plan: ReviewedGdcRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("GDC transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_GDC_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_project_metadata") fail("GDC transient value is malformed");
  const bundle = exactObject("GDC transient bundle", transient.bundle, ["schema", "generated_at", "sources", "projects", "source_set_digest", "project_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("GDC transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "project_set_digest", "bundle_digest", "source_set_digest", "source_count", "project_count", "request_count", "response_bytes", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleUnsigned = Object.fromEntries(Object.entries(bundle).filter(([key]) => key !== "bundle_digest"));
  const receiptUnsigned = Object.fromEntries(Object.entries(receipt).filter(([key]) => key !== "receipt_digest"));
  if (digestJsonSync(bundleUnsigned) !== digest("GDC bundle digest", bundle.bundle_digest) || digestJsonSync(receiptUnsigned) !== digest("GDC receipt digest", receipt.receipt_digest)) fail("GDC transient digest is invalid");
  if (bundle.schema !== REVIEWED_GDC_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_GDC_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.project_set_digest !== plan.projectSetDigest) fail("GDC transient identity differs from its reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "not_accepted" || receipt.retention !== RETENTION || bundle.provider !== "none" || bundle.credentials !== "not_accepted") fail("GDC transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("GDC transient limitations are invalid");
  const retrievedAt = timestamp("GDC retrievedAt", receipt.retrieved_at);
  if (timestamp("GDC generatedAt", bundle.generated_at) !== retrievedAt) fail("GDC bundle and receipt timestamps do not match");
  const projects = bundle.projects; const sources = bundle.sources; const sourceReceipts = receipt.source_receipts;
  if (!Array.isArray(projects) || !Array.isArray(sources) || !Array.isArray(sourceReceipts) || projects.length !== plan.config.projectIds.length || sources.length !== projects.length || sourceReceipts.length !== projects.length) fail("GDC transient project coverage is incomplete");
  if (digestJsonSync(sources) !== bundle.source_set_digest || receipt.source_set_digest !== bundle.source_set_digest || receipt.bundle_digest !== bundle.bundle_digest) fail("GDC transient source binding is invalid");
  for (const [index, projectId] of plan.config.projectIds.entries()) {
    const project = projects[index]; const source = exactObject("GDC source metadata", sources[index], ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"]);
    const sourceReceipt = exactObject("GDC source receipt", sourceReceipts[index], ["schema", "lane", "source_id", "project_id", "content_digest", "metadata_completeness"]);
    if (typeof project !== "object" || project === null || Array.isArray(project)) fail("GDC project metadata is malformed");
    const row = project as Record<string, unknown>;
    const normalized = normalizeProject({ data: { ...row, project_id: projectId, disease_type: row.disease_types, primary_site: row.primary_sites, summary: { case_count: row.case_count, file_count: row.file_count, data_categories: row.data_categories } } }, projectId);
    const projectDigest = digestJsonSync(normalized); const sourceId = normalized.source_id as string;
    if (canonicalJson(row) !== canonicalJson(normalized) || source.source_id !== sourceId || source.authority !== REVIEWED_GDC_AUTHORITY || source.uri !== `${REVIEWED_GDC_ENDPOINT}/${projectId}` || source.retrieved_at !== retrievedAt || source.content_sha256 !== projectDigest || source.record_count !== 1 || source.provider !== "none" || source.credentials !== "not_accepted" || canonicalJson(source.limitations) !== canonicalJson(LIMITATIONS)) fail("GDC source metadata does not match its project");
    const expectedSourceReceipt = { schema: REVIEWED_GDC_SOURCE_RECEIPT_SCHEMA, lane: PROJECT_TO_LANE[projectId], source_id: sourceId, project_id: projectId, content_digest: projectDigest, metadata_completeness: normalized.metadata_completeness };
    if (canonicalJson(sourceReceipt) !== canonicalJson(expectedSourceReceipt)) fail("GDC source receipt does not match its project");
  }
  const expectedCompleteness = projects.every((project) => (project as Record<string, unknown>).metadata_completeness === "complete") ? "complete" : "unknown";
  if (receipt.request_count !== projects.length || receipt.source_count !== projects.length || receipt.project_count !== projects.length || bundle.project_count !== projects.length || receipt.completeness !== expectedCompleteness || bundle.completeness !== expectedCompleteness) fail("GDC transient counts or completeness are inconsistent");
  integer("GDC response_bytes", receipt.response_bytes, projects.length, MAX_REVIEWED_GDC_TOTAL_RESPONSE_BYTES);
  return { bundle, receipt };
}

export function createReviewedGdcAutonomousEvidenceRegistration(adapter: ReviewedGdcRetrievalAdapter, plan: ReviewedGdcRetrievalPlan, projectId: ReviewedGdcProjectId): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (!(adapter instanceof ReviewedGdcRetrievalAdapter) || !(plan instanceof ReviewedGdcRetrievalPlan)) fail("GDC registration requires exact adapter and plan values");
  plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || !plan.config.projectIds.includes(projectId) || plan.config.projectIds.length !== 1) fail("GDC registration requires the exact single-project plan");
  const frozenPlan = ReviewedGdcRetrievalPlan.create(plan.config); const lane = PROJECT_TO_LANE[projectId]; const sourceId = `gdc_${lane}`;
  const expectedNetwork = adapter.config.transportId === BUILTIN_GDC_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.gdc.${lane}`, version: REVIEWED_GDC_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["aggregate_cohort_landscape", "source_provenance"], sourceKinds: ["nci_gdc_project_aggregate_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request;
      if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("GDC acquisition request does not match its reviewed source");
      const metadata = exactObject("GDC execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("GDC metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_GDC_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("GDC execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("GDC retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("GDC projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}

export type ReviewedGdcSourceReceipt = Record<string, JsonValue>;
export type ReviewedGdcBundle = Record<string, JsonValue>;
