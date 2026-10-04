import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceAcquisitionContext, AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

export const REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA = "bioprism-reviewed-clinical-trials-config/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA = "bioprism-reviewed-clinical-trials-plan/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-clinical-trials-source-receipt/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA = "bioprism-reviewed-clinical-trials-receipt/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA = "bioprism-reviewed-clinical-trials-bundle/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA = "bioprism-reviewed-clinical-trials-transient/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-clinical-trials-execution-metadata/0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_CLINICAL_TRIALS_HOST = "clinicaltrials.gov" as const;
export const REVIEWED_CLINICAL_TRIALS_PATH = "/api/v2/studies" as const;
export const REVIEWED_CLINICAL_TRIALS_AUTHORITY = "ClinicalTrials.gov / U.S. National Library of Medicine" as const;
export const REVIEWED_CLINICAL_TRIALS_FIELDS = Object.freeze(["NCTId", "BriefTitle", "OverallStatus", "Phase", "LastUpdatePostDate", "StudyType", "EnrollmentCount", "InterventionName"] as const);
export const REVIEWED_CLINICAL_TRIALS_CONDITIONS = Object.freeze({ glioblastoma: "Glioblastoma", glioma: "Glioma" } as const);
export const MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE = 100;
export const MAX_REVIEWED_CLINICAL_TRIALS_PAGES = 5;
export const MAX_REVIEWED_CLINICAL_TRIALS_RECORDS = 2 * MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE * MAX_REVIEWED_CLINICAL_TRIALS_PAGES;
export const MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES = 8_000_000;
export const MAX_REVIEWED_CLINICAL_TRIALS_TOTAL_RESPONSE_BYTES = 24_000_000;
export const MAX_REVIEWED_CLINICAL_TRIALS_BUNDLE_BYTES = 8_000_000;
export const MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH = 64;
export const MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES = 200_000;
export const BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID = "builtin.clinicaltrials.gov.https-get" as const;
export const BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "allowlisted_https_get_v1",
  method: "GET",
  scheme: "https",
  host: REVIEWED_CLINICAL_TRIALS_HOST,
  path: REVIEWED_CLINICAL_TRIALS_PATH,
  redirects: "refused",
  fields: [...REVIEWED_CLINICAL_TRIALS_FIELDS],
  credentials: "not_accepted",
});

const SOURCE_BASE = `https://${REVIEWED_CLINICAL_TRIALS_HOST}${REVIEWED_CLINICAL_TRIALS_PATH}`;
const RETENTION = "metadata_only;trial_records_and_page_tokens_transient";
const LIMITATIONS = Object.freeze([
  "ClinicalTrials.gov registry metadata is not evidence of eligibility, treatment benefit, or clinical applicability",
  "a bounded page sequence can be incomplete; the receipt reports truncation and source-reported totals",
  "the adapter does not retrieve participant-level data, eligibility text, results, documents, or contact fields",
  "source records require independent review before any research conclusion or workflow promotion",
  "caller-injected transports must enforce their own timeout and network policy under the supplied transport identity",
]);
const CONFIG_KEYS = new Set(["conditionLanes", "pageSize", "maxPages", "timeoutSeconds", "transportId", "transportVersion", "transportConfigDigest"]);
const HEX = /^[0-9a-f]{64}$/;
const NCT = /^NCT[0-9]{8}$/;
const DATE = /^[0-9]{4}-[0-9]{2}(?:-[0-9]{2})?$/;
const TOKEN = /^[!-~]{1,512}$/;

export class ReviewedClinicalTrialsRetrievalError extends ArgumentError {
  override readonly name = "ReviewedClinicalTrialsRetrievalError";
}

function fail(message: string): never {
  throw new ReviewedClinicalTrialsRetrievalError(message);
}

function bytes(value: string): number {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) fail("ClinicalTrials.gov text contains invalid Unicode");
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) fail("ClinicalTrials.gov text contains invalid Unicode");
  }
  return new TextEncoder().encode(value).length;
}

function exactObject(name: string, value: unknown, keys: readonly string[]): Record<string, unknown> {
  if (!isObject(value) || Object.getPrototypeOf(value) !== Object.prototype) fail(`${name} must be a plain object`);
  const expected = new Set(keys);
  for (const key of Object.keys(value)) if (!expected.has(key)) fail(`${name} contains an unsupported field`);
  for (const key of keys) if (!(key in value)) fail(`${name} is missing a field`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !HEX.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} must be an integer between ${minimum} and ${maximum}`);
  return value as number;
}

function text(name: string, value: unknown, maximum: number, optional = false): string | null {
  if (value === null && optional) return null;
  if (typeof value !== "string") fail(`${name} must be text`);
  const normalized = value.trim().replace(/\s+/g, " ");
  if (!normalized && optional) return null;
  for (let index = 0; index < normalized.length; index += 1) {
    const unit = normalized.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = normalized.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) fail(`${name} contains invalid Unicode`);
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) fail(`${name} contains invalid Unicode`);
  }
  if (!normalized || normalized.includes("\0") || bytes(normalized) > maximum || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(normalized)) fail(`${name} is outside its text bound`);
  return normalized;
}

function timestamp(name: string, value: unknown): string {
  if (typeof value !== "string" || value.startsWith("0000-") || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value) || Number.isNaN(Date.parse(value))) fail(`${name} must be a UTC timestamp with second precision`);
  try { if (new Date(value).toISOString().replace(/\.\d{3}Z$/, "Z") !== value) fail(`${name} is not a valid UTC calendar timestamp`); } catch { fail(`${name} is not a valid UTC calendar timestamp`); }
  return value;
}

function validClinicalDate(value: string): boolean {
  if (!DATE.test(value) || value.startsWith("0000-")) return false;
  try {
    const normalized = value.length === 7 ? `${value}-01` : value;
    return new Date(`${normalized}T00:00:00.000Z`).toISOString().slice(0, 10) === normalized;
  } catch { return false; }
}

function canonicalBytes(name: string, value: unknown, maximum: number): string {
  let encoded: string;
  try { encoded = canonicalJson(value); } catch { return fail(`${name} is not canonical JSON`); }
  if (bytes(encoded) > maximum) fail(`${name} exceeds its byte bound`);
  return encoded;
}

function parseStrictResponseJson(source: string): unknown {
  let index = 0;
  let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[\u0020\u0009\u000a\u000d]/.test(source[index]!)) index += 1; };
  const stringToken = (): string => {
    const start = index;
    if (source[index] !== '"') fail("ClinicalTrials.gov response is not valid JSON");
    index += 1;
    while (index < source.length) {
      const code = source.charCodeAt(index);
      if (source[index] === '"') {
        index += 1;
        try { return JSON.parse(source.slice(start, index)) as string; } catch { return fail("ClinicalTrials.gov response is not valid JSON"); }
      }
      if (source[index] === "\\") {
        index += 2;
        continue;
      }
      if (code < 0x20) fail("ClinicalTrials.gov response is not valid JSON");
      index += 1;
    }
    return fail("ClinicalTrials.gov response is not valid JSON");
  };
  const value = (depth: number): void => {
    nodes += 1;
    if (nodes > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES || depth > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH) fail("ClinicalTrials.gov response exceeds its tree bound");
    whitespace();
    const current = source[index];
    if (current === '"') { stringToken(); return; }
    if (current === "{") {
      index += 1;
      whitespace();
      const keys = new Set<string>();
      if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace();
        const key = stringToken();
        if (keys.has(key)) fail("ClinicalTrials.gov response contains a duplicate JSON field");
        keys.add(key);
        whitespace();
        if (source[index] !== ":") fail("ClinicalTrials.gov response is not valid JSON");
        index += 1;
        value(depth + 1);
        whitespace();
        if (source[index] === "}") { index += 1; return; }
        if (source[index] !== ",") fail("ClinicalTrials.gov response is not valid JSON");
        index += 1;
      }
      return fail("ClinicalTrials.gov response is not valid JSON");
    }
    if (current === "[") {
      index += 1;
      whitespace();
      if (source[index] === "]") { index += 1; return; }
      while (index < source.length) {
        value(depth + 1);
        whitespace();
        if (source[index] === "]") { index += 1; return; }
        if (source[index] !== ",") fail("ClinicalTrials.gov response is not valid JSON");
        index += 1;
      }
      return fail("ClinicalTrials.gov response is not valid JSON");
    }
    const start = index;
    while (index < source.length && !/[\u0020\u0009\u000a\u000d,\]}]/.test(source[index]!)) index += 1;
    if (index === start) fail("ClinicalTrials.gov response is not valid JSON");
  };
  value(0);
  whitespace();
  if (index !== source.length) fail("ClinicalTrials.gov response is not valid JSON");
  try { return JSON.parse(source) as unknown; } catch { return fail("ClinicalTrials.gov response is not valid JSON"); }
}

function validateResponseTree(root: unknown): void {
  const stack: Array<{ value: unknown; depth: number }> = [{ value: root, depth: 0 }];
  const seen = new WeakSet<object>();
  let nodes = 0;
  while (stack.length > 0) {
    const next = stack.pop()!;
    nodes += 1;
    if (nodes > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_NODES || next.depth > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_DEPTH) fail("ClinicalTrials.gov response exceeds its tree bound");
    if (next.value !== null && typeof next.value === "object") {
      if (seen.has(next.value)) fail("ClinicalTrials.gov response is not a JSON tree");
      seen.add(next.value);
      if (Array.isArray(next.value)) for (const child of next.value) stack.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(next.value) !== Object.prototype) fail("ClinicalTrials.gov response contains a non-JSON object");
        for (const child of Object.values(next.value as Record<string, unknown>)) stack.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof next.value === "number" && !Number.isFinite(next.value)) fail("ClinicalTrials.gov response contains a non-finite number");
    else if (typeof next.value === "function" || typeof next.value === "symbol" || typeof next.value === "bigint" || typeof next.value === "undefined") fail("ClinicalTrials.gov response contains a non-JSON value");
  }
}

function urlFor(condition: string, pageSize: number, pageToken: string | null): string {
  const url = new URL(SOURCE_BASE);
  url.searchParams.set("format", "json");
  url.searchParams.set("fields", REVIEWED_CLINICAL_TRIALS_FIELDS.join(","));
  url.searchParams.set("pageSize", String(pageSize));
  url.searchParams.set("query.cond", condition);
  if (pageToken !== null) url.searchParams.set("pageToken", pageToken);
  return url.toString();
}

function assertURL(value: string, condition: string, pageSize: number, pageToken: string | null): void {
  let url: URL;
  try { url = new URL(value); } catch { return fail("ClinicalTrials.gov request URL is malformed"); }
  const expected = new URL(SOURCE_BASE);
  expected.searchParams.set("format", "json");
  expected.searchParams.set("fields", REVIEWED_CLINICAL_TRIALS_FIELDS.join(","));
  expected.searchParams.set("pageSize", String(pageSize));
  expected.searchParams.set("query.cond", condition);
  if (pageToken !== null) expected.searchParams.set("pageToken", pageToken);
  if (url.toString() !== expected.toString() || url.protocol !== "https:" || url.hostname !== REVIEWED_CLINICAL_TRIALS_HOST || url.pathname !== REVIEWED_CLINICAL_TRIALS_PATH || url.username || url.password || url.hash) fail("ClinicalTrials.gov request escaped its exact HTTPS query");
}

function nowUTC(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

function path(value: unknown, ...keys: string[]): unknown {
  let current: unknown = value;
  for (const key of keys) {
    if (!isObject(current)) return undefined;
    current = current[key];
  }
  return current;
}

function textList(value: unknown, name: string, maximum: number): string[] {
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value) || value.length > maximum) fail(`${name} is outside its list bound`);
  const result: string[] = [];
  const seen = new Set<string>();
  for (const item of value) {
    if (item === undefined || item === null || item === "") continue;
    const normalized = text(name, item, 1024)!;
    if (!seen.has(normalized)) { seen.add(normalized); result.push(normalized); }
  }
  return result;
}

function trial(study: unknown, sourceId: string): Record<string, JsonValue> {
  if (!isObject(study) || !isObject(study.protocolSection)) fail("ClinicalTrials.gov study omitted protocolSection");
  const protocol = study.protocolSection;
  const nctId = text("ClinicalTrials.gov NCT ID", path(protocol, "identificationModule", "nctId"), 16)!;
  if (!NCT.test(nctId)) fail("ClinicalTrials.gov study has an invalid NCT identifier");
  const title = text("ClinicalTrials.gov brief title", path(protocol, "identificationModule", "briefTitle"), 4096)!;
  const status = text("ClinicalTrials.gov overall status", path(protocol, "statusModule", "overallStatus"), 128)!;
  const design = path(protocol, "designModule");
  const phases = textList(path(design, "phases"), "ClinicalTrials.gov phases", 16);
  const lastUpdate = text("ClinicalTrials.gov last update", path(protocol, "statusModule", "lastUpdatePostDateStruct", "date"), 10, true);
  if (lastUpdate !== null && !validClinicalDate(lastUpdate)) fail("ClinicalTrials.gov last update is outside the source date contract");
  const studyType = text("ClinicalTrials.gov study type", path(design, "studyType"), 128, true);
  const rawEnrollment = path(design, "enrollmentInfo", "count");
  const enrollment = rawEnrollment === undefined || rawEnrollment === null ? null : integer("ClinicalTrials.gov enrollment target", rawEnrollment, 0, 10_000_000);
  const rawInterventions = path(protocol, "armsInterventionsModule", "interventions");
  if (rawInterventions !== undefined && rawInterventions !== null && (!Array.isArray(rawInterventions) || rawInterventions.length > 128)) fail("ClinicalTrials.gov interventions are outside their list bound");
  const names = textList(Array.isArray(rawInterventions) ? rawInterventions.map((row) => isObject(row) ? row.name : null) : [], "ClinicalTrials.gov intervention name", 128);
  return { source_id: sourceId, nct_id: nctId, title, overall_status: status, phases, last_update: lastUpdate, study_type: studyType, enrollment_count: enrollment, intervention_names: names };
}

function bundleDigest(value: Record<string, unknown>): string {
  const unsigned = { ...value };
  delete unsigned.bundle_digest;
  return digestJsonSync(unsigned);
}

function clone<T>(value: T): T {
  return JSON.parse(canonicalJson(value)) as T;
}

interface ConfigData {
  conditionLanes: Array<keyof typeof REVIEWED_CLINICAL_TRIALS_CONDITIONS>;
  pageSize: number;
  maxPages: number;
  timeoutSeconds: number;
  transportId: string;
  transportVersion: string;
  transportConfigDigest: string;
}

export interface ReviewedClinicalTrialsRetrievalConfigOptions extends Partial<ConfigData> {}
export type ReviewedClinicalTrialsLane = keyof typeof REVIEWED_CLINICAL_TRIALS_CONDITIONS;
/** Caller transports must enforce their own timeout and network policy under the configured identity. */
export type ReviewedClinicalTrialsFetch = (url: string) => Promise<unknown> | unknown;
export interface ReviewedClinicalTrialsRetrievalAdapterOptions { fetch?: ReviewedClinicalTrialsFetch; }

const CONFIG_PRIVATE = new WeakMap<ReviewedClinicalTrialsRetrievalConfig, ConfigData>();

export class ReviewedClinicalTrialsRetrievalConfig {
  constructor(options: ReviewedClinicalTrialsRetrievalConfigOptions = {}) {
    if (!isObject(options)) fail("ClinicalTrials.gov config options must be an object");
    for (const key of Object.keys(options)) if (!CONFIG_KEYS.has(key)) fail("ClinicalTrials.gov config contains an unsupported field");
    const lanes = options.conditionLanes ?? ["glioblastoma"];
    if (!Array.isArray(lanes) || lanes.length < 1 || lanes.length > 2 || lanes.some((lane) => typeof lane !== "string" || !Object.prototype.hasOwnProperty.call(REVIEWED_CLINICAL_TRIALS_CONDITIONS, lane)) || new Set(lanes).size !== lanes.length) fail("conditionLanes must select one or more fixed, unique glioma lanes");
    const transportId = options.transportId ?? BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID;
    const transportVersion = options.transportVersion ?? BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION;
    const transportConfigDigest = options.transportConfigDigest ?? BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST;
    if (typeof transportId !== "string" || !/^[A-Za-z0-9_.:+-]{1,128}$/.test(transportId) || typeof transportVersion !== "string" || !/^[A-Za-z0-9_.:+-]{1,128}$/.test(transportVersion)) fail("ClinicalTrials.gov transport identity is invalid");
    const data: ConfigData = {
      conditionLanes: [...lanes].sort() as ConfigData["conditionLanes"],
      pageSize: integer("pageSize", options.pageSize ?? 25, 1, MAX_REVIEWED_CLINICAL_TRIALS_PAGE_SIZE),
      maxPages: integer("maxPages", options.maxPages ?? 2, 1, MAX_REVIEWED_CLINICAL_TRIALS_PAGES),
      timeoutSeconds: integer("timeoutSeconds", options.timeoutSeconds ?? 30, 1, 120),
      transportId,
      transportVersion,
      transportConfigDigest: digest("transportConfigDigest", transportConfigDigest),
    };
    CONFIG_PRIVATE.set(this, data);
    canonicalBytes("ClinicalTrials.gov config", this.toJSON(), 64_000);
    Object.freeze(this);
  }

  get condition_lanes(): readonly ReviewedClinicalTrialsLane[] { return [...requiredConfig(this).conditionLanes]; }
  get page_size(): number { return requiredConfig(this).pageSize; }
  get max_pages(): number { return requiredConfig(this).maxPages; }
  get request_limit(): number { return this.condition_lanes.length * this.max_pages; }
  get record_limit(): number { return this.request_limit * this.page_size; }
  get transport_id(): string { return requiredConfig(this).transportId; }
  get transport_version(): string { return requiredConfig(this).transportVersion; }
  get transport_config_digest(): string { return requiredConfig(this).transportConfigDigest; }
  get config_digest(): string { return digestJsonSync(this.toJSON()); }

  toJSON(): ReviewedClinicalTrialsRetrievalConfigJSON {
    const data = requiredConfig(this);
    return { schema: REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA, condition_lanes: [...data.conditionLanes], page_size: data.pageSize, max_pages: data.maxPages, request_limit: data.conditionLanes.length * data.maxPages, record_limit: data.conditionLanes.length * data.maxPages * data.pageSize, timeout_seconds: data.timeoutSeconds, fields: [...REVIEWED_CLINICAL_TRIALS_FIELDS], transport_id: data.transportId, transport_version: data.transportVersion, transport_config_digest: data.transportConfigDigest, retention: "metadata_only;source_values_transient", credentials: "not_accepted" };
  }

}

function requiredConfig(config: ReviewedClinicalTrialsRetrievalConfig): ConfigData {
  const value = CONFIG_PRIVATE.get(config);
  if (!value || Object.getPrototypeOf(config) !== ReviewedClinicalTrialsRetrievalConfig.prototype || Object.keys(config).length !== 0) fail("ClinicalTrials.gov config identity is invalid");
  return value;
}

const PLAN_PRIVATE = new WeakMap<ReviewedClinicalTrialsRetrievalPlan, ReviewedClinicalTrialsRetrievalConfig>();

export class ReviewedClinicalTrialsRetrievalPlan {
  private constructor(config: ReviewedClinicalTrialsRetrievalConfig, readonly config_digest: string, readonly query_set_digest: string, readonly plan_digest: string) {
    PLAN_PRIVATE.set(this, config);
    Object.freeze(this);
  }

  static fromConfig(config: ReviewedClinicalTrialsRetrievalConfig): ReviewedClinicalTrialsRetrievalPlan {
    if (Object.getPrototypeOf(config) !== ReviewedClinicalTrialsRetrievalConfig.prototype) fail("ClinicalTrials.gov plan requires an exact config");
    const data = requiredConfig(config);
    const queries = data.conditionLanes.map((lane) => ({ lane, condition: REVIEWED_CLINICAL_TRIALS_CONDITIONS[lane], fields: [...REVIEWED_CLINICAL_TRIALS_FIELDS], page_size: data.pageSize, max_pages: data.maxPages }));
    const querySetDigest = digestJsonSync({ schema: REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, queries });
    const configDigest = config.config_digest;
    const payload = { schema: REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, config: config.toJSON(), config_digest: configDigest, query_set_digest: querySetDigest, request_limit: data.conditionLanes.length * data.maxPages, record_limit: data.conditionLanes.length * data.maxPages * data.pageSize, scope: "fixed_public_glioma_registry_metadata", execution: "GET_https_clinicaltrials.gov_api_v2_studies_after_literal_approval", retention: RETENTION, credentials: "not_accepted" };
    return new ReviewedClinicalTrialsRetrievalPlan(config, configDigest, querySetDigest, digestJsonSync(payload));
  }

  toJSON(): ReviewedClinicalTrialsRetrievalPlanJSON {
    const config = PLAN_PRIVATE.get(this);
    if (!config || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalPlan.prototype || Object.keys(this).some((key) => !["config_digest", "query_set_digest", "plan_digest"].includes(key))) fail("ClinicalTrials.gov plan identity is invalid");
    return { schema: REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA, config: config.toJSON(), config_digest: this.config_digest, query_set_digest: this.query_set_digest, request_limit: config.request_limit, record_limit: config.record_limit, scope: "fixed_public_glioma_registry_metadata", execution: "GET_https_clinicaltrials.gov_api_v2_studies_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.plan_digest };
  }

  get config(): ReviewedClinicalTrialsRetrievalConfig {
    const config = PLAN_PRIVATE.get(this);
    if (!config || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalPlan.prototype) fail("ClinicalTrials.gov plan identity is invalid");
    return config;
  }

  validate(): void {
    const config = PLAN_PRIVATE.get(this);
    if (!config || ReviewedClinicalTrialsRetrievalPlan.fromConfig(config).plan_digest !== this.plan_digest || this.config_digest !== config.config_digest) fail("ClinicalTrials.gov plan digest or config is invalid");
  }
}

async function builtinFetch(url: string, timeoutSeconds: number): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutSeconds * 1000);
  try {
    const response = await fetch(url, { method: "GET", headers: { Accept: "application/json" }, redirect: "manual", signal: controller.signal });
    if (response.status >= 300 && response.status < 400) fail("ClinicalTrials.gov redirect is refused");
    if (!response.ok || !response.body) fail("ClinicalTrials.gov returned a non-success response");
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let total = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      total += value.byteLength;
      if (total > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES) { await reader.cancel(); fail("ClinicalTrials.gov response exceeds its byte bound"); }
      chunks.push(value);
    }
    const joined = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) { joined.set(chunk, offset); offset += chunk.byteLength; }
    return new TextDecoder("utf-8", { fatal: true }).decode(joined);
  } catch (error) {
    if (error instanceof ReviewedClinicalTrialsRetrievalError) throw error;
    throw new ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov request failed");
  } finally { clearTimeout(timeout); }
}

function responseObject(value: unknown): Record<string, unknown> {
  if (typeof value === "string") {
    if (bytes(value) > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES) fail("ClinicalTrials.gov response exceeds its byte bound");
    value = parseStrictResponseJson(value);
  } else if (value instanceof Uint8Array) {
    if (value.byteLength > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES) fail("ClinicalTrials.gov response exceeds its byte bound");
    try { value = parseStrictResponseJson(new TextDecoder("utf-8", { fatal: true }).decode(value)); } catch (error) {
      if (error instanceof ReviewedClinicalTrialsRetrievalError) throw error;
      return fail("ClinicalTrials.gov response is not valid UTF-8 JSON");
    }
  }
  if (!isObject(value)) fail("ClinicalTrials.gov response root is not an object");
  validateResponseTree(value);
  canonicalBytes("ClinicalTrials.gov response", value, MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES);
  return value;
}

const ADAPTER_PRIVATE = new WeakMap<ReviewedClinicalTrialsRetrievalAdapter, { config: ReviewedClinicalTrialsRetrievalConfig; fetcher: ReviewedClinicalTrialsFetch | null }>();

export class ReviewedClinicalTrialsRetrievalAdapter {
  constructor(config: ReviewedClinicalTrialsRetrievalConfig, options: ReviewedClinicalTrialsRetrievalAdapterOptions = {}) {
    const data = requiredConfig(config);
    if (!isObject(options)) fail("ClinicalTrials.gov adapter options must be an object");
    for (const key of Object.keys(options)) if (key !== "fetch") fail("ClinicalTrials.gov adapter options contain an unsupported field");
    const injected = options.fetch as ReviewedClinicalTrialsFetch | undefined;
    if (injected !== undefined && typeof injected !== "function") fail("ClinicalTrials.gov fetch must be callable");
    if (injected && data.transportId === BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID) fail("injected ClinicalTrials.gov transport requires a distinct reviewed identity");
    if (!injected && (data.transportId !== BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID || data.transportVersion !== BUILTIN_CLINICAL_TRIALS_TRANSPORT_VERSION || data.transportConfigDigest !== BUILTIN_CLINICAL_TRIALS_TRANSPORT_CONFIG_DIGEST)) fail("built-in ClinicalTrials.gov transport identity is not exact");
    ADAPTER_PRIVATE.set(this, { config, fetcher: injected ?? null });
    Object.freeze(this);
  }

  get config(): ReviewedClinicalTrialsRetrievalConfig {
    const state = ADAPTER_PRIVATE.get(this);
    if (!state || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalAdapter.prototype) fail("ClinicalTrials.gov adapter identity is invalid");
    return state.config;
  }

  prepare(): ReviewedClinicalTrialsRetrievalPlan { return ReviewedClinicalTrialsRetrievalPlan.fromConfig(this.config); }

  async execute(plan: ReviewedClinicalTrialsRetrievalPlan, options: { approveSourceDispatch: true; retrievedAt?: string }): Promise<ReviewedClinicalTrialsRetrievalResult> {
    const state = ADAPTER_PRIVATE.get(this);
    if (!state || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalAdapter.prototype || Object.keys(this).length !== 0) fail("ClinicalTrials.gov adapter identity is invalid");
    if (Object.getPrototypeOf(plan) !== ReviewedClinicalTrialsRetrievalPlan.prototype) fail("ClinicalTrials.gov execute requires an exact reviewed plan");
    plan.validate();
    if (plan.config_digest !== state.config.config_digest || canonicalJson((PLAN_PRIVATE.get(plan) ?? state.config).toJSON()) !== canonicalJson(state.config.toJSON())) fail("ClinicalTrials.gov plan differs from this adapter config");
    if (!isObject(options) || options.approveSourceDispatch !== true) fail("ClinicalTrials.gov execution requires literal source-dispatch approval");
    for (const key of Object.keys(options)) if (key !== "approveSourceDispatch" && key !== "retrievedAt") fail("ClinicalTrials.gov execution options contain an unsupported field");
    const timestampValue = options.retrievedAt === undefined ? nowUTC() : timestamp("retrievedAt", options.retrievedAt);
    const timestampText = timestamp("retrievedAt", timestampValue);
    const data = requiredConfig(state.config);
    let responseTotal = 0;
    let requestCount = 0;
    let omitted = 0;
    let reportedTotalKnown = true;
    let reportedTotal = 0;
    let truncated = false;
    const sources: Record<string, JsonValue>[] = [];
    const sourceReceipts: Record<string, JsonValue>[] = [];
    const records: Record<string, JsonValue>[] = [];
    for (const lane of data.conditionLanes) {
      const condition = REVIEWED_CLINICAL_TRIALS_CONDITIONS[lane];
      const sourceId = `clinicaltrials_${lane}`;
      const seenTokens = new Set<string>();
      const seenIds = new Set<string>();
      const laneRecords: Record<string, JsonValue>[] = [];
      let token: string | null = null;
      let laneTotal: number | null = null;
      for (let pageNumber = 0; pageNumber < data.maxPages; pageNumber += 1) {
        const url = urlFor(condition, data.pageSize, token);
        assertURL(url, condition, data.pageSize, token);
        let raw: unknown;
        try { raw = state.fetcher ? await state.fetcher(url) : await builtinFetch(url, data.timeoutSeconds); }
        catch (error) {
          if (error instanceof ReviewedClinicalTrialsRetrievalError) throw error;
          throw new ReviewedClinicalTrialsRetrievalError("ClinicalTrials.gov caller transport failed");
        }
        const size = raw instanceof Uint8Array ? raw.byteLength : typeof raw === "string" ? bytes(raw) : bytes(canonicalBytes("ClinicalTrials.gov response", raw, MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES));
        responseTotal += size;
        if (size > MAX_REVIEWED_CLINICAL_TRIALS_RESPONSE_BYTES || responseTotal > MAX_REVIEWED_CLINICAL_TRIALS_TOTAL_RESPONSE_BYTES) fail("ClinicalTrials.gov aggregate response exceeds its byte bound");
        const payload = responseObject(raw);
        requestCount += 1;
        if (payload.totalCount !== undefined && payload.totalCount !== null) {
          const pageTotal = integer("ClinicalTrials.gov totalCount", payload.totalCount, 0, Number.MAX_SAFE_INTEGER);
          if (laneTotal !== null && laneTotal !== pageTotal) fail("ClinicalTrials.gov totalCount changed during one reviewed page sequence");
          laneTotal = pageTotal;
        }
        if (!Array.isArray(payload.studies) || payload.studies.length > data.pageSize) fail("ClinicalTrials.gov page has an invalid studies list");
        for (const study of payload.studies) {
          const row = trial(study, sourceId);
          const nct = row.nct_id as string;
          if (seenIds.has(nct)) fail("ClinicalTrials.gov returned a duplicate NCT identifier");
          seenIds.add(nct);
          laneRecords.push(row);
        }
        const next = payload.nextPageToken;
        if (next === undefined || next === null || next === "") { token = null; break; }
        if (typeof next !== "string" || !TOKEN.test(next) || seenTokens.has(next)) fail("ClinicalTrials.gov pagination token is malformed or cyclic");
        seenTokens.add(next);
        token = next;
      }
      laneRecords.sort((left, right) => String(left.nct_id).localeCompare(String(right.nct_id)));
      const laneTruncated = token !== null || (laneTotal !== null && laneRecords.length < laneTotal);
      if (laneTotal !== null && laneTotal < laneRecords.length) fail("ClinicalTrials.gov totalCount is smaller than the returned study count");
      truncated ||= laneTruncated;
      if (laneTotal === null) reportedTotalKnown = false;
      else {
        reportedTotal += laneTotal;
        omitted += Math.max(0, laneTotal - laneRecords.length);
        if (!Number.isSafeInteger(reportedTotal) || !Number.isSafeInteger(omitted) || reportedTotal > 9_000_000_000_000_000 || omitted > 9_000_000_000_000_000) fail("ClinicalTrials.gov aggregate totals exceed the portable integer contract");
      }
      const sourceDigest = digestJsonSync(laneRecords);
      sources.push({ source_id: sourceId, authority: REVIEWED_CLINICAL_TRIALS_AUTHORITY, uri: SOURCE_BASE, retrieved_at: timestampText, content_sha256: sourceDigest, record_count: laneRecords.length });
      sourceReceipts.push({ schema: REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA, lane, source_id: sourceId, content_digest: sourceDigest, record_count: laneRecords.length, reported_total_count: laneTotal, truncated: laneTruncated });
      records.push(...laneRecords);
    }
    records.sort((left, right) => `${left.source_id}\0${left.nct_id}`.localeCompare(`${right.source_id}\0${right.nct_id}`));
    const totalValue = reportedTotalKnown ? reportedTotal : null;
    const unsignedBundle: Record<string, JsonValue> = { schema: REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA, generated_at: timestampText, sources, trials: records, source_set_digest: digestJsonSync(sources), record_count: records.length, reported_total_count: totalValue, omitted_record_count: omitted, truncated, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    canonicalBytes("ClinicalTrials.gov transient bundle", unsignedBundle, MAX_REVIEWED_CLINICAL_TRIALS_BUNDLE_BYTES);
    const computedBundleDigest = digestJsonSync(unsignedBundle);
    const bundle = { ...unsignedBundle, bundle_digest: computedBundleDigest };
    const receiptPayload: Record<string, JsonValue> = { schema: REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA, plan_digest: plan.plan_digest, config_digest: plan.config_digest, query_set_digest: plan.query_set_digest, bundle_digest: computedBundleDigest, source_set_digest: digestJsonSync(sources), source_count: sources.length, record_count: records.length, request_count: requestCount, reported_total_count: totalValue, omitted_record_count: omitted, truncated, retrieved_at: timestampText, source_receipts: sourceReceipts, provider: "none", network: state.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    return new ReviewedClinicalTrialsRetrievalResult(bundle, { ...receiptPayload, receipt_digest: digestJsonSync(receiptPayload) } as Record<string, JsonValue>);
  }
}

const RESULT_PRIVATE = new WeakMap<ReviewedClinicalTrialsRetrievalResult, { bundle: Record<string, JsonValue>; receipt: Record<string, JsonValue> }>();

export class ReviewedClinicalTrialsRetrievalResult {
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) {
    RESULT_PRIVATE.set(this, { bundle: clone(bundle), receipt: clone(receipt) });
    Object.freeze(this);
  }
  get receipt(): ReviewedClinicalTrialsReceiptJSON {
    const state = RESULT_PRIVATE.get(this);
    if (!state || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalResult.prototype || Object.keys(this).length !== 0) fail("ClinicalTrials.gov result identity is invalid");
    return clone(state.receipt) as unknown as ReviewedClinicalTrialsReceiptJSON;
  }
  get bundle(): ReviewedClinicalTrialsBundle {
    const state = RESULT_PRIVATE.get(this);
    if (!state || Object.getPrototypeOf(this) !== ReviewedClinicalTrialsRetrievalResult.prototype) fail("ClinicalTrials.gov result identity is invalid");
    return clone(state.bundle) as unknown as ReviewedClinicalTrialsBundle;
  }
  toJSON(): Record<string, JsonValue> { return { receipt: this.receipt, retention: "metadata_only;bundle_is_transient" }; }
  toTransientJSON(): ReviewedClinicalTrialsTransientJSON { return { schema: REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA, bundle: this.bundle, receipt: this.receipt, retention: "caller_owned_transient_bundle" }; }
}

export function createReviewedClinicalTrialsExecutionMetadata(plan: ReviewedClinicalTrialsRetrievalPlan, options: { approveSourceDispatch: true; retrievedAt?: string } ): ReviewedClinicalTrialsExecutionMetadata {
  if (Object.getPrototypeOf(plan) !== ReviewedClinicalTrialsRetrievalPlan.prototype) fail("ClinicalTrials.gov execution metadata requires an exact plan");
  plan.validate();
  if (!isObject(options) || options.approveSourceDispatch !== true) fail("ClinicalTrials.gov execution metadata requires literal approval");
  for (const key of Object.keys(options)) if (key !== "approveSourceDispatch" && key !== "retrievedAt") fail("ClinicalTrials.gov execution metadata options contain an unsupported field");
  const retrievedAt = options.retrievedAt === undefined ? null : timestamp("retrievedAt", options.retrievedAt);
  const payload = { schema: REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.plan_digest, approve_source_dispatch: true as const, retrieved_at: retrievedAt, retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateTransient(value: unknown, plan: ReviewedClinicalTrialsRetrievalPlan, expectedNetwork: "builtin_https" | "caller_transport"): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("generic ClinicalTrials.gov transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_bundle") fail("generic ClinicalTrials.gov transient value identity is invalid");
  const bundle = exactObject("generic ClinicalTrials.gov bundle", transient.bundle, ["schema", "generated_at", "sources", "trials", "source_set_digest", "record_count", "reported_total_count", "omitted_record_count", "truncated", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("generic ClinicalTrials.gov receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "query_set_digest", "bundle_digest", "source_set_digest", "source_count", "record_count", "request_count", "reported_total_count", "omitted_record_count", "truncated", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleHash = digest("bundle_digest", bundle.bundle_digest);
  const receiptHash = digest("receipt_digest", receipt.receipt_digest);
  const unsignedReceipt = { ...receipt };
  delete unsignedReceipt.receipt_digest;
  if (bundleDigest(bundle) !== bundleHash || digestJsonSync(unsignedReceipt) !== receiptHash) fail("generic ClinicalTrials.gov bundle or receipt digest is invalid");
  if (bundle.schema !== REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA || receipt.plan_digest !== plan.plan_digest || receipt.config_digest !== plan.config_digest || receipt.query_set_digest !== plan.query_set_digest) fail("generic ClinicalTrials.gov receipt is not bound to its reviewed plan");
  if (receipt.bundle_digest !== bundleHash || receipt.source_set_digest !== bundle.source_set_digest) fail("generic ClinicalTrials.gov receipt does not bind its bundle");
  if (bundle.provider !== "none" || receipt.provider !== "none" || bundle.credentials !== "not_accepted" || receipt.credentials !== "not_accepted" || receipt.effect !== "read_only" || receipt.retention !== RETENTION || receipt.network !== expectedNetwork) fail("generic ClinicalTrials.gov bundle or receipt exceeds the reviewed source boundary");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("generic ClinicalTrials.gov limitations differ from the reviewed contract");

  const lanes = plan.config.condition_lanes;
  const sourceIds = lanes.map((lane) => `clinicaltrials_${lane}`);
  if (!Array.isArray(bundle.sources) || bundle.sources.length !== lanes.length || !Array.isArray(bundle.trials) || bundle.trials.length > plan.config.record_limit || !Array.isArray(receipt.source_receipts) || receipt.source_receipts.length !== lanes.length) fail("generic ClinicalTrials.gov source inventory is incomplete or exceeds its reviewed bound");
  const generatedAt = timestamp("bundle.generated_at", bundle.generated_at);
  if (receipt.retrieved_at !== generatedAt) fail("generic ClinicalTrials.gov timestamps do not match");
  if (bundle.sources.some((source, index) => !isObject(source) || source.source_id !== sourceIds[index])) fail("generic ClinicalTrials.gov source order or identity is invalid");
  if (bundle.trials.some((row) => !isObject(row) || !sourceIds.includes(String(row.source_id)))) fail("generic ClinicalTrials.gov trial inventory contains an unknown source");

  const sourceKeys = ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count"] as const;
  const sourceReceiptKeys = ["schema", "lane", "source_id", "content_digest", "record_count", "reported_total_count", "truncated"] as const;
  const trialKeys = ["source_id", "nct_id", "title", "overall_status", "phases", "last_update", "study_type", "enrollment_count", "intervention_names"] as const;
  let expectedOmitted = 0;
  let expectedReportedTotal = 0;
  let allReported = true;
  let expectedTruncated = false;
  for (let index = 0; index < lanes.length; index += 1) {
    const lane = lanes[index]!;
    const sourceId = sourceIds[index]!;
    const source = exactObject("generic ClinicalTrials.gov source", bundle.sources[index], sourceKeys);
    const sourceReceipt = exactObject("generic ClinicalTrials.gov source receipt", receipt.source_receipts[index], sourceReceiptKeys);
    if (sourceReceipt.schema !== REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA || sourceReceipt.lane !== lane || sourceReceipt.source_id !== sourceId) fail("generic ClinicalTrials.gov source receipt identity is invalid");
    if (source.source_id !== sourceId || source.authority !== REVIEWED_CLINICAL_TRIALS_AUTHORITY || source.uri !== SOURCE_BASE || source.retrieved_at !== generatedAt) fail("generic ClinicalTrials.gov source metadata is invalid");
    const sourceHash = digest("source.content_sha256", source.content_sha256);
    if (digest("source_receipt.content_digest", sourceReceipt.content_digest) !== sourceHash) fail("generic ClinicalTrials.gov source receipt digest does not match");
    const laneRows = bundle.trials.filter((row): row is Record<string, unknown> => isObject(row) && row.source_id === sourceId);
    const ids = new Set<string>();
    for (const value of laneRows) {
      const row = exactObject("generic ClinicalTrials.gov trial", value, trialKeys);
      const nct = text("trial.nct_id", row.nct_id, 16)!;
      if (!NCT.test(nct) || ids.has(nct)) fail("generic ClinicalTrials.gov trial identifier is invalid or duplicated");
      ids.add(nct);
      text("trial.source_id", row.source_id, 128);
      text("trial.title", row.title, 4096);
      text("trial.overall_status", row.overall_status, 128);
      const phases = textList(row.phases, "trial.phases", 16);
      if (canonicalJson(phases) !== canonicalJson(row.phases)) fail("generic ClinicalTrials.gov phase list is invalid");
      if (row.last_update !== null) {
        const updated = text("trial.last_update", row.last_update, 10)!;
        if (!validClinicalDate(updated)) fail("generic ClinicalTrials.gov trial date is invalid");
      }
      if (row.study_type !== null) text("trial.study_type", row.study_type, 128);
      if (row.enrollment_count !== null) integer("trial.enrollment_count", row.enrollment_count, 0, 10_000_000);
      const names = textList(row.intervention_names, "trial.intervention_names", 128);
      if (canonicalJson(names) !== canonicalJson(row.intervention_names)) fail("generic ClinicalTrials.gov intervention list is invalid");
    }
    if (canonicalJson(laneRows) !== canonicalJson([...laneRows].sort((left, right) => String(left.nct_id).localeCompare(String(right.nct_id))))) fail("generic ClinicalTrials.gov trial ordering is invalid");
    if (integer("source.record_count", source.record_count, 0, plan.config.record_limit) !== laneRows.length || integer("source_receipt.record_count", sourceReceipt.record_count, 0, plan.config.record_limit) !== laneRows.length) fail("generic ClinicalTrials.gov source record count is invalid");
    if (digestJsonSync(laneRows) !== sourceHash) fail("generic ClinicalTrials.gov source content digest is invalid");
    if (sourceReceipt.reported_total_count === null) allReported = false;
    else {
      const total = integer("source_receipt.reported_total_count", sourceReceipt.reported_total_count, 0, 9_000_000_000_000_000);
      if (total < laneRows.length) fail("generic ClinicalTrials.gov reported total is smaller than returned records");
      expectedReportedTotal += total;
      expectedOmitted += total - laneRows.length;
      if (!Number.isSafeInteger(expectedReportedTotal) || !Number.isSafeInteger(expectedOmitted) || expectedReportedTotal > 9_000_000_000_000_000 || expectedOmitted > 9_000_000_000_000_000) fail("generic ClinicalTrials.gov aggregate totals exceed the portable integer contract");
    }
    if (typeof sourceReceipt.truncated !== "boolean") fail("generic ClinicalTrials.gov truncation state is invalid");
    if (sourceReceipt.reported_total_count !== null && integer("source_receipt.reported_total_count", sourceReceipt.reported_total_count, 0, 9_000_000_000_000_000) > laneRows.length && !sourceReceipt.truncated) fail("generic ClinicalTrials.gov truncation state hides omitted source records");
    expectedTruncated ||= sourceReceipt.truncated;
  }
  const requestCount = integer("receipt.request_count", receipt.request_count, lanes.length, plan.config.request_limit);
  void requestCount;
  if (integer("receipt.source_count", receipt.source_count, 0, lanes.length) !== lanes.length || integer("receipt.record_count", receipt.record_count, 0, plan.config.record_limit) !== bundle.trials.length || integer("bundle.record_count", bundle.record_count, 0, plan.config.record_limit) !== bundle.trials.length) fail("generic ClinicalTrials.gov aggregate counts are invalid");
  if (canonicalJson(bundle.trials) !== canonicalJson([...bundle.trials].sort((left, right) => `${String(left.source_id)}\0${String(left.nct_id)}`.localeCompare(`${String(right.source_id)}\0${String(right.nct_id)}`)))) fail("generic ClinicalTrials.gov aggregate trial ordering is invalid");
  if (digest("bundle.source_set_digest", bundle.source_set_digest) !== digestJsonSync(bundle.sources)) fail("generic ClinicalTrials.gov source-set digest is invalid");
  const expectedTotal = allReported ? expectedReportedTotal : null;
  if (bundle.reported_total_count !== expectedTotal || receipt.reported_total_count !== expectedTotal) fail("generic ClinicalTrials.gov reported total is inconsistent");
  if (integer("bundle.omitted_record_count", bundle.omitted_record_count, 0, 9_000_000_000_000_000) !== expectedOmitted || integer("receipt.omitted_record_count", receipt.omitted_record_count, 0, 9_000_000_000_000_000) !== expectedOmitted || bundle.truncated !== expectedTruncated || receipt.truncated !== expectedTruncated) fail("generic ClinicalTrials.gov truncation accounting is inconsistent");
  return { bundle, receipt };
}

export function createReviewedClinicalTrialsAutonomousEvidenceRegistration(adapter: ReviewedClinicalTrialsRetrievalAdapter, plan: ReviewedClinicalTrialsRetrievalPlan, lane: ReviewedClinicalTrialsLane): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (Object.getPrototypeOf(adapter) !== ReviewedClinicalTrialsRetrievalAdapter.prototype || Object.getPrototypeOf(plan) !== ReviewedClinicalTrialsRetrievalPlan.prototype) fail("ClinicalTrials.gov registration requires exact adapter and plan values");
  plan.validate();
  if (plan.config_digest !== adapter.config.config_digest || adapter.config.condition_lanes.length !== 1 || adapter.config.condition_lanes[0] !== lane) fail("generic ClinicalTrials.gov registration requires the exact single reviewed lane");
  const expectedSource = `clinicaltrials_${lane}`;
  const frozenConfig = requiredConfig(adapter.config);
  const frozenPlan = ReviewedClinicalTrialsRetrievalPlan.fromConfig(new ReviewedClinicalTrialsRetrievalConfig({ conditionLanes: [...adapter.config.condition_lanes], pageSize: adapter.config.page_size, maxPages: adapter.config.max_pages, timeoutSeconds: frozenConfig.timeoutSeconds, transportId: frozenConfig.transportId, transportVersion: frozenConfig.transportVersion, transportConfigDigest: frozenConfig.transportConfigDigest }));
  return {
    adapterId: `clinicaltrials_${lane}_${plan.plan_digest.slice(0, 16)}`,
    version: REVIEWED_CLINICAL_TRIALS_ADAPTER_VERSION,
    domains: ["biomedical", "neuroscience"],
    capabilities: ["evidence", "provenance", "clinical_trials_registry"],
    sourceKinds: ["clinicaltrials", "public_registry"],
    acquire: async (context: AutonomousEvidenceAcquisitionContext): Promise<JsonValue> => {
      if (!context || !isObject(context.request)) fail("generic ClinicalTrials.gov acquisition context is malformed");
      const request = context.request as Record<string, unknown>;
      if (request.source_id !== expectedSource || request.source_digest !== frozenPlan.plan_digest) fail("generic ClinicalTrials.gov request does not match its reviewed source");
      const raw = exactObject("generic ClinicalTrials.gov execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...raw };
      const supplied = digest("metadata_digest", unsigned.metadata_digest);
      delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || raw.schema !== REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA || raw.reviewed_plan_digest !== frozenPlan.plan_digest || raw.approve_source_dispatch !== true || raw.retention !== "metadata_only" || raw.credentials !== "not_accepted") fail("generic ClinicalTrials.gov execution metadata failed review binding");
      const retrievedAt = raw.retrieved_at === null ? undefined : timestamp("retrieved_at", raw.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON();
    },
    project: async (value: JsonValue, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { bundle, receipt } = validateTransient(value, frozenPlan, adapter.config.transport_id === BUILTIN_CLINICAL_TRIALS_TRANSPORT_ID ? "builtin_https" : "caller_transport");
      const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("generic ClinicalTrials.gov project context has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}

export interface ReviewedClinicalTrialsRetrievalConfigJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_CONFIG_SCHEMA;
  condition_lanes: ReviewedClinicalTrialsLane[];
  page_size: number;
  max_pages: number;
  request_limit: number;
  record_limit: number;
  timeout_seconds: number;
  fields: string[];
  transport_id: string;
  transport_version: string;
  transport_config_digest: string;
  retention: "metadata_only;source_values_transient";
  credentials: "not_accepted";
}

export interface ReviewedClinicalTrialsRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_PLAN_SCHEMA;
  config: ReviewedClinicalTrialsRetrievalConfigJSON;
  config_digest: string;
  query_set_digest: string;
  request_limit: number;
  record_limit: number;
  scope: "fixed_public_glioma_registry_metadata";
  execution: "GET_https_clinicaltrials.gov_api_v2_studies_after_literal_approval";
  retention: typeof RETENTION;
  credentials: "not_accepted";
  plan_digest: string;
}

export interface ReviewedClinicalTrialsTrial extends Record<string, JsonValue> {
  source_id: string;
  nct_id: string;
  title: string;
  overall_status: string;
  phases: string[];
  last_update: string | null;
  study_type: string | null;
  enrollment_count: number | null;
  intervention_names: string[];
}

export interface ReviewedClinicalTrialsSource extends Record<string, JsonValue> {
  source_id: string;
  authority: string;
  uri: string;
  retrieved_at: string;
  content_sha256: string;
  record_count: number;
  provider: "none";
  credentials: "not_accepted";
  limitations: string[];
}

export interface ReviewedClinicalTrialsSourceReceipt extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_SOURCE_RECEIPT_SCHEMA;
  lane: ReviewedClinicalTrialsLane;
  source_id: string;
  content_digest: string;
  record_count: number;
  reported_total_count: number | null;
  truncated: boolean;
}

export interface ReviewedClinicalTrialsBundle extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_BUNDLE_SCHEMA;
  generated_at: string;
  sources: ReviewedClinicalTrialsSource[];
  trials: ReviewedClinicalTrialsTrial[];
  source_set_digest: string;
  record_count: number;
  reported_total_count: number | null;
  omitted_record_count: number;
  truncated: boolean;
  provider: "none";
  credentials: "not_accepted";
  limitations: string[];
  bundle_digest: string;
}

export interface ReviewedClinicalTrialsReceiptJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_RECEIPT_SCHEMA;
  plan_digest: string;
  config_digest: string;
  query_set_digest: string;
  bundle_digest: string;
  source_set_digest: string;
  source_count: number;
  record_count: number;
  request_count: number;
  reported_total_count: number | null;
  omitted_record_count: number;
  truncated: boolean;
  retrieved_at: string;
  source_receipts: ReviewedClinicalTrialsSourceReceipt[];
  provider: "none";
  network: "builtin_https" | "caller_transport";
  effect: "read_only";
  retention: typeof RETENTION;
  credentials: "not_accepted";
  limitations: string[];
  receipt_digest: string;
}

export interface ReviewedClinicalTrialsTransientJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_TRANSIENT_SCHEMA;
  bundle: ReviewedClinicalTrialsBundle;
  receipt: ReviewedClinicalTrialsReceiptJSON;
  retention: "caller_owned_transient_bundle";
}

export interface ReviewedClinicalTrialsExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CLINICAL_TRIALS_EXECUTION_METADATA_SCHEMA;
  reviewed_plan_digest: string;
  approve_source_dispatch: true;
  retrieved_at: string | null;
  retention: "metadata_only";
  credentials: "not_accepted";
  metadata_digest: string;
}
