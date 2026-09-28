/**
 * Reviewed, bounded retrieval of fixed public cBioPortal study/profile metadata.
 * Blueprint 23 (Adult Diffuse Glioma and Glioblastoma Worlds) calls for redistributable public
 * metadata or a bring-your-own-data connector. This is only a catalogue adapter; it does not
 * implement the temporal parent world, specimen lineage, or the blueprint's evaluation gates.
 */

import { ArgumentError } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonValue } from "./types.js";

export const REVIEWED_CBIOPORTAL_CONFIG_SCHEMA = "bioprism-reviewed-cbioportal-study-config/0.1" as const;
export const REVIEWED_CBIOPORTAL_PLAN_SCHEMA = "bioprism-reviewed-cbioportal-study-plan/0.1" as const;
export const REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA = "bioprism-reviewed-cbioportal-study-source-receipt/0.1" as const;
export const REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA = "bioprism-reviewed-cbioportal-study-bundle/0.1" as const;
export const REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA = "bioprism-reviewed-cbioportal-study-receipt/0.1" as const;
export const REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA = "bioprism-reviewed-cbioportal-study-transient/0.1" as const;
export const REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA = "bioprism-reviewed-cbioportal-study-execution-metadata/0.1" as const;
export const REVIEWED_CBIOPORTAL_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_CBIOPORTAL_HOST = "www.cbioportal.org" as const;
export const REVIEWED_CBIOPORTAL_PATH = "/api/studies" as const;
export const REVIEWED_CBIOPORTAL_ENDPOINT = `https://${REVIEWED_CBIOPORTAL_HOST}${REVIEWED_CBIOPORTAL_PATH}` as const;
export const REVIEWED_CBIOPORTAL_AUTHORITY = "cBioPortal for Cancer Genomics" as const;
export const REVIEWED_CBIOPORTAL_STUDIES = Object.freeze({ gbm: "gbm_tcga", lgg: "lgg_tcga" } as const);
export type ReviewedCBioPortalStudyId = (typeof REVIEWED_CBIOPORTAL_STUDIES)[keyof typeof REVIEWED_CBIOPORTAL_STUDIES];
export const MAX_REVIEWED_CBIOPORTAL_STUDIES = 2;
export const MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY = 128;
export const MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES = 1_000_000;
export const MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES = 4_000_000;
export const MAX_REVIEWED_CBIOPORTAL_BUNDLE_BYTES = 2_000_000;
export const MAX_REVIEWED_CBIOPORTAL_TREE_DEPTH = 32;
export const MAX_REVIEWED_CBIOPORTAL_TREE_NODES = 50_000;
export const BUILTIN_CBIOPORTAL_TRANSPORT_ID = "builtin.nci-cbioportal.fetch" as const;
export const BUILTIN_CBIOPORTAL_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream", scheme: "https", host: REVIEWED_CBIOPORTAL_HOST,
  paths: ["/api/studies/{studyId}", "/api/studies/{studyId}/molecular-profiles"], method: "GET",
  study_fields: ["studyId", "cancerTypeId", "name", "description", "publicStudy", "pmid", "allSampleCount", "referenceGenome", "importDate"],
  profile_projection: "SUMMARY", profile_page_size: MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY, profile_page_number: 0, profile_sort: ["molecularProfileId", "ASC"],
  redirects: "refused", credentials: "not_accepted",
});

const RETENTION = "public_study_and_molecular_profile_metadata_only;no_sample_patient_or_molecular_rows" as const;
const LIMITATIONS = Object.freeze([
  "cBioPortal study summaries and molecular profile definitions are catalogue metadata, not patient-level evidence or outcome",
  "aggregate sample counts and molecular profile definitions are source-reported and may change between retrievals",
  "the fixed TCGA GBM and LGG catalogue is not an exhaustive glioma cohort search",
  "independent review is required for freshness, omissions, study quality, and applicability",
  "the adapter does not request or retain sample, patient, clinical, genomic, assay-value, or controlled-access records",
  "the public cBioPortal API is documented as beta and may change; endpoint and response drift fail closed",
  "caller-injected transports own timeout, redirect, and network policy under their declared identity",
] as const);
const STUDY_TO_LANE: Readonly<Record<ReviewedCBioPortalStudyId, "gbm" | "lgg">> = Object.freeze({ gbm_tcga: "gbm", lgg_tcga: "lgg" });
const PROFILE_ALTERATIONS = new Set(["MUTATION_EXTENDED", "MUTATION_UNCALLED", "STRUCTURAL_VARIANT", "COPY_NUMBER_ALTERATION", "MICRO_RNA_EXPRESSION", "MRNA_EXPRESSION", "MRNA_EXPRESSION_NORMALS", "RNA_EXPRESSION", "METHYLATION", "METHYLATION_BINARY", "PHOSPHORYLATION", "PROTEIN_LEVEL", "PROTEIN_ARRAY_PROTEIN_LEVEL", "PROTEIN_ARRAY_PHOSPHORYLATION", "GENESET_SCORE", "GENERIC_ASSAY"]);
const PROFILE_ID_RE = /^[A-Za-z0-9_.:-]{1,256}$/;
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

export class ReviewedCBioPortalRetrievalError extends ArgumentError {
  override readonly name = "ReviewedCBioPortalRetrievalError";
}
function fail(message: string): never { throw new ReviewedCBioPortalRetrievalError(message); }
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
    if (nodes > MAX_REVIEWED_CBIOPORTAL_TREE_NODES || next.depth > MAX_REVIEWED_CBIOPORTAL_TREE_DEPTH) fail("CBIOPORTAL response exceeds its structural bound");
    const current = next.value;
    if (typeof current === "number" && (!NativeIsFinite(current) || (NativeIsInteger(current) && !NativeIsSafeInteger(current)))) fail("CBIOPORTAL response contains an invalid number");
    if (current !== null && typeof current === "object") {
      if (seen.has(current)) fail("CBIOPORTAL response is not a JSON tree");
      seen.add(current);
      if (Array.isArray(current)) for (const child of current) pending.push({ value: child, depth: next.depth + 1 });
      else {
        if (Object.getPrototypeOf(current) !== Object.prototype) fail("CBIOPORTAL response contains a non-JSON object");
        for (const key of NativeObjectKeys(current)) {
          if (/[\u0000-\u001f\u007f]/.test(key)) fail("CBIOPORTAL response contains an invalid key");
          if (/[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(key)) fail("CBIOPORTAL response contains invalid Unicode");
        }
        for (const child of NativeObjectValues(current as Record<string, unknown>)) pending.push({ value: child, depth: next.depth + 1 });
      }
    } else if (typeof current === "string" && /[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]/u.test(current)) fail("CBIOPORTAL response contains invalid Unicode");
    else if (current !== null && !["string", "number", "boolean"].includes(typeof current)) fail("CBIOPORTAL response contains an unsupported value");
  }
}

/** Scan JSON syntax before JSON.parse so duplicate names cannot be silently overwritten. */
function parseStrictJson(source: string): unknown {
  let index = 0; let nodes = 0;
  const whitespace = (): void => { while (index < source.length && /[ \t\r\n]/.test(source[index]!)) index += 1; };
  const string = (): string => {
    const start = index;
    if (source[index] !== "\"") fail("CBIOPORTAL response is not valid JSON");
    index += 1;
    while (index < source.length) {
      if (source[index] === "\"") { index += 1; try { return NativeJsonParse(source.slice(start, index)) as string; } catch { return fail("CBIOPORTAL response is not valid JSON"); } }
      if (source[index] === "\\") { index += 2; continue; }
      if (source.charCodeAt(index) < 32) fail("CBIOPORTAL response is not valid JSON");
      index += 1;
    }
    return fail("CBIOPORTAL response is not valid JSON");
  };
  const scan = (depth: number): void => {
    nodes += 1;
    if (nodes > MAX_REVIEWED_CBIOPORTAL_TREE_NODES || depth > MAX_REVIEWED_CBIOPORTAL_TREE_DEPTH) fail("CBIOPORTAL response exceeds its structural bound");
    whitespace();
    if (source[index] === "\"") { string(); return; }
    if (source[index] === "{") {
      index += 1; whitespace(); const keys = new Set<string>();
      if (source[index] === "}") { index += 1; return; }
      while (index < source.length) {
        whitespace(); const key = string();
        if (keys.has(key)) fail("CBIOPORTAL response contains duplicate JSON fields");
        keys.add(key); whitespace(); if (source[index] !== ":") fail("CBIOPORTAL response is not valid JSON");
        index += 1; scan(depth + 1); whitespace();
        if (source[index] === "}") { index += 1; return; }
        if (source[index] !== ",") fail("CBIOPORTAL response is not valid JSON"); index += 1;
      }
      return fail("CBIOPORTAL response is not valid JSON");
    }
    if (source[index] === "[") {
      index += 1; whitespace(); if (source[index] === "]") { index += 1; return; }
      while (index < source.length) {
        scan(depth + 1); whitespace();
        if (source[index] === "]") { index += 1; return; }
        if (source[index] !== ",") fail("CBIOPORTAL response is not valid JSON"); index += 1;
      }
      return fail("CBIOPORTAL response is not valid JSON");
    }
    const start = index;
    while (index < source.length && !/[\s,\]}]/.test(source[index]!)) index += 1;
    if (index === start) fail("CBIOPORTAL response is not valid JSON");
    try { NativeJsonParse(source.slice(start, index)); } catch { fail("CBIOPORTAL response is not valid JSON"); }
  };
  scan(0); whitespace();
  if (index !== source.length) fail("CBIOPORTAL response is not valid JSON");
  const parsed = NativeJsonParse(source) as unknown; validateTree(parsed); return parsed;
}
function parseResponse(value: unknown): { payload: unknown; bytes: number } {
  let body: string;
  if (typeof value === "string") body = value;
  else if (value instanceof Uint8Array) {
    try { body = new NativeTextDecoder("utf-8", { fatal: true }).decode(value); } catch { return fail("CBIOPORTAL response is not valid UTF-8"); }
  } else if (typeof value === "object" && value !== null) {
    try { body = NativeJsonStringify(value) as string; } catch { return fail("CBIOPORTAL injected response is not JSON"); }
  } else return fail("CBIOPORTAL transport returned an unsupported response type");
  const size = byteLength(body);
  if (size > MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES) fail("CBIOPORTAL response exceeds its byte bound");
  const parsed = parseStrictJson(body);
  if (typeof parsed !== "object" || parsed === null || (!Array.isArray(parsed) && Object.getPrototypeOf(parsed) !== Object.prototype)) fail("cBioPortal response root is not an object or array");
  return { payload: parsed, bytes: size };
}
function studyUrl(studyId: ReviewedCBioPortalStudyId): string {
  const url = `${REVIEWED_CBIOPORTAL_ENDPOINT}/${studyId}`;
  if (!url.startsWith(`${REVIEWED_CBIOPORTAL_ENDPOINT}/`) || byteLength(url) > 8_192) fail("CBIOPORTAL request escaped its reviewed endpoint");
  return url;
}
function profilesUrl(studyId: ReviewedCBioPortalStudyId): string {
  const parameters = new URLSearchParams({ projection: "SUMMARY", pageSize: String(MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY), pageNumber: "0", sortBy: "molecularProfileId", direction: "ASC" });
  const url = `${REVIEWED_CBIOPORTAL_ENDPOINT}/${studyId}/molecular-profiles?${parameters.toString()}`;
  if (!url.startsWith(`${REVIEWED_CBIOPORTAL_ENDPOINT}/`) || byteLength(url) > 8_192) fail("cBioPortal profile request escaped its reviewed endpoint");
  return url;
}
async function builtinFetch(url: string, timeoutMs: number): Promise<Uint8Array> {
  if (NativeFetch === undefined || NativeAbortController === undefined) fail("CBIOPORTAL built-in fetch is unavailable");
  if (!url.startsWith(`${REVIEWED_CBIOPORTAL_ENDPOINT}/`) || (!url.includes("/molecular-profiles?") && url.includes("?"))) fail("cBioPortal built-in transport refused an unpinned URL");
  const controller = new NativeAbortController(); let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_resolve, reject) => { timer = NativeSetTimeout(() => { controller.abort(); reject(new ReviewedCBioPortalRetrievalError("CBIOPORTAL study request timed out")); }, timeoutMs); });
  try {
    const request = NativeFetch(url, { method: "GET", headers: { Accept: "application/json" }, redirect: "error", signal: controller.signal });
    const operation = request.then(async (response) => {
      if (!response.ok || response.url !== url || response.redirected || !response.body) fail("CBIOPORTAL endpoint returned an unexpected response");
      const reader = response.body.getReader(); const chunks: Uint8Array[] = []; let total = 0;
      while (true) {
        const { done, value } = await reader.read(); if (done) break;
        if (!(value instanceof Uint8Array) || total + value.byteLength > MAX_REVIEWED_CBIOPORTAL_RESPONSE_BYTES) { try { await reader.cancel(); } catch { /* Preserve bounded refusal. */ } fail("CBIOPORTAL response exceeds its byte bound"); }
        chunks.push(value); total += value.byteLength;
      }
      const body = new Uint8Array(total); let offset = 0;
      for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
      return body;
    });
    return await NativePromiseRace([operation, timeout]);
  } catch (error) {
    if (error instanceof ReviewedCBioPortalRetrievalError) throw error;
    throw new ReviewedCBioPortalRetrievalError("CBIOPORTAL study request failed");
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
function normalizeStudy(raw: unknown, studyId: ReviewedCBioPortalStudyId): Record<string, JsonValue> {
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) fail("cBioPortal study response is malformed");
  const study = raw as Record<string, unknown>;
  if (study.studyId !== studyId || study.publicStudy !== true) fail("cBioPortal study identity or public status does not match the reviewed study");
  const sampleCount = count("cBioPortal allSampleCount", study.allSampleCount, true);
  return {
    source_id: `cbioportal_${STUDY_TO_LANE[studyId]}`, lane: STUDY_TO_LANE[studyId], study_id: studyId,
    name: text("cBioPortal study name", study.name, 2_000)!,
    description: text("cBioPortal study description", study.description, 8_000, true),
    cancer_type_id: text("cBioPortal cancerTypeId", study.cancerTypeId, 128, true), public_study: true,
    pmid: text("cBioPortal study PMID", study.pmid, 128, true), sample_count: sampleCount,
    reference_genome: text("cBioPortal referenceGenome", study.referenceGenome, 128, true),
    import_date: text("cBioPortal importDate", study.importDate, 64, true),
    metadata_completeness: sampleCount !== null ? "complete" : "unknown",
  };
}
function normalizeProfiles(raw: unknown, studyId: ReviewedCBioPortalStudyId): Record<string, JsonValue>[] {
  if (!Array.isArray(raw) || raw.length >= MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY) fail("cBioPortal molecular-profile response exceeds its catalogue bound");
  const ids = new Set<string>();
  const profiles = raw.map((item): Record<string, JsonValue> => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) fail("cBioPortal molecular profile is malformed");
    const row = item as Record<string, unknown>;
    if (row.studyId !== studyId) fail("cBioPortal molecular profile has a mismatched study identity");
    const profileId = text("cBioPortal molecularProfileId", row.molecularProfileId, 256)!;
    if (!PROFILE_ID_RE.test(profileId)) fail("cBioPortal molecularProfileId is outside its fixed identifier bound");
    if (ids.has(profileId)) fail("cBioPortal molecular profile catalogue contains duplicate IDs");
    ids.add(profileId);
    const alteration = text("cBioPortal molecularAlterationType", row.molecularAlterationType, 64)!;
    if (!PROFILE_ALTERATIONS.has(alteration)) fail("cBioPortal molecular profile has an unsupported alteration type");
    for (const key of ["showProfileInAnalysisTab", "patientLevel"]) if (row[key] !== null && row[key] !== undefined && typeof row[key] !== "boolean") fail(`cBioPortal ${key} must be a boolean or unknown`);
    return {
      study_id: studyId, profile_id: profileId, name: text("cBioPortal profile name", row.name, 2_000)!,
      molecular_alteration_type: alteration, generic_assay_type: text("cBioPortal genericAssayType", row.genericAssayType, 128, true),
      datatype: text("cBioPortal profile datatype", row.datatype, 128)!, description: text("cBioPortal profile description", row.description, 4_000, true),
      show_in_analysis: (row.showProfileInAnalysisTab ?? null) as JsonValue, patient_level: (row.patientLevel ?? null) as JsonValue,
    };
  });
  return profiles.sort((left, right) => String(left.profile_id) < String(right.profile_id) ? -1 : String(left.profile_id) > String(right.profile_id) ? 1 : 0);
}

export interface ReviewedCBioPortalRetrievalConfigOptions {
  studyIds?: readonly ReviewedCBioPortalStudyId[];
  timeoutMs?: number;
  transportId?: string;
  transportVersion?: string;
  transportConfigDigest?: string;
}
export interface ReviewedCBioPortalRetrievalConfigPayload extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CBIOPORTAL_CONFIG_SCHEMA; study_ids: ReviewedCBioPortalStudyId[]; timeout_ms: number; request_limit: number;
  transport_id: string; transport_version: string; transport_config_digest: string; study_set_digest: string;
  retention: typeof RETENTION; credentials: "not_accepted";
}
export interface ReviewedCBioPortalRetrievalConfigJSON extends ReviewedCBioPortalRetrievalConfigPayload {
  config_digest: string;
}
export class ReviewedCBioPortalRetrievalConfig {
  readonly studyIds: readonly ReviewedCBioPortalStudyId[]; readonly timeoutMs: number; readonly transportId: string; readonly transportVersion: string; readonly transportConfigDigest: string; readonly studySetDigest: string;
  constructor(options: ReviewedCBioPortalRetrievalConfigOptions = {}) {
    const studies = options.studyIds ?? ["gbm_tcga"];
    if (!Array.isArray(studies) || studies.length < 1 || studies.length > MAX_REVIEWED_CBIOPORTAL_STUDIES || studies.some((study) => typeof study !== "string" || !Object.values(REVIEWED_CBIOPORTAL_STUDIES).includes(study as ReviewedCBioPortalStudyId)) || new Set(studies).size !== studies.length) fail("cBioPortal studyIds contain an unsupported or duplicate study");
    this.studyIds = Object.freeze((Object.values(REVIEWED_CBIOPORTAL_STUDIES) as ReviewedCBioPortalStudyId[]).filter((study) => studies.includes(study)));
    this.timeoutMs = integer("CBIOPORTAL timeoutMs", options.timeoutMs ?? 30_000, 100, 120_000);
    this.transportId = options.transportId ?? BUILTIN_CBIOPORTAL_TRANSPORT_ID; this.transportVersion = options.transportVersion ?? BUILTIN_CBIOPORTAL_TRANSPORT_VERSION;
    if (!IDENTIFIER_RE.test(this.transportId) || !IDENTIFIER_RE.test(this.transportVersion)) fail("CBIOPORTAL transport identity is invalid");
    this.transportConfigDigest = digest("CBIOPORTAL transportConfigDigest", options.transportConfigDigest ?? BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST);
    this.studySetDigest = digestJsonSync({ studies: this.studyIds.map((studyId) => ({ lane: STUDY_TO_LANE[studyId], study_id: studyId })) });
    NativeObjectFreeze(this);
  }
  get requestLimit(): number { return this.studyIds.length * 2; }
  private payload(): ReviewedCBioPortalRetrievalConfigPayload {
    return { schema: REVIEWED_CBIOPORTAL_CONFIG_SCHEMA, study_ids: [...this.studyIds], timeout_ms: this.timeoutMs, request_limit: this.requestLimit, transport_id: this.transportId, transport_version: this.transportVersion, transport_config_digest: this.transportConfigDigest, study_set_digest: this.studySetDigest, retention: RETENTION, credentials: "not_accepted" };
  }
  get configDigest(): string { return digestJsonSync(this.payload()); }
  toJSON(): ReviewedCBioPortalRetrievalConfigJSON { return { ...this.payload(), config_digest: this.configDigest }; }
  static fromJSON(value: unknown): ReviewedCBioPortalRetrievalConfig {
    const raw = exactObject("CBIOPORTAL config", value, ["schema", "study_ids", "timeout_ms", "request_limit", "transport_id", "transport_version", "transport_config_digest", "study_set_digest", "retention", "credentials", "config_digest"]);
    if (raw.schema !== REVIEWED_CBIOPORTAL_CONFIG_SCHEMA || !Array.isArray(raw.study_ids)) fail("CBIOPORTAL config has an invalid shape");
    const config = new ReviewedCBioPortalRetrievalConfig({ studyIds: raw.study_ids as ReviewedCBioPortalStudyId[], timeoutMs: raw.timeout_ms as number, transportId: raw.transport_id as string, transportVersion: raw.transport_version as string, transportConfigDigest: raw.transport_config_digest as string });
    if (canonicalJson(config.toJSON()) !== canonicalJson(raw)) fail("CBIOPORTAL config is not normalized or its digest is invalid");
    return config;
  }
}

export interface ReviewedCBioPortalRetrievalPlanJSON extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CBIOPORTAL_PLAN_SCHEMA; config: ReviewedCBioPortalRetrievalConfigJSON; config_digest: string; study_set_digest: string; request_limit: number;
  scope: "fixed_public_cbioportal_study_and_profile_metadata"; execution: "bounded_https_get_after_literal_approval"; retention: typeof RETENTION; credentials: "not_accepted"; plan_digest: string;
}
export class ReviewedCBioPortalRetrievalPlan {
  readonly config: ReviewedCBioPortalRetrievalConfig; readonly configDigest: string; readonly studySetDigest: string; readonly planDigest: string;
  private constructor(config: ReviewedCBioPortalRetrievalConfig, configDigest: string, studySetDigest: string, planDigest: string) { this.config = config; this.configDigest = configDigest; this.studySetDigest = studySetDigest; this.planDigest = planDigest; NativeObjectFreeze(this); }
  static create(config: ReviewedCBioPortalRetrievalConfig): ReviewedCBioPortalRetrievalPlan {
    if (!(config instanceof ReviewedCBioPortalRetrievalConfig)) fail("CBIOPORTAL plan requires an exact config");
    const fields = { schema: REVIEWED_CBIOPORTAL_PLAN_SCHEMA, config: config.toJSON(), config_digest: config.configDigest, study_set_digest: config.studySetDigest, request_limit: config.requestLimit, scope: "fixed_public_cbioportal_study_and_profile_metadata", execution: "bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "not_accepted" };
    return new ReviewedCBioPortalRetrievalPlan(config, config.configDigest, config.studySetDigest, digestJsonSync(fields));
  }
  toJSON(): ReviewedCBioPortalRetrievalPlanJSON { return { schema: REVIEWED_CBIOPORTAL_PLAN_SCHEMA, config: this.config.toJSON(), config_digest: this.configDigest, study_set_digest: this.studySetDigest, request_limit: this.config.requestLimit, scope: "fixed_public_cbioportal_study_and_profile_metadata", execution: "bounded_https_get_after_literal_approval", retention: RETENTION, credentials: "not_accepted", plan_digest: this.planDigest }; }
  validate(): void { const expected = ReviewedCBioPortalRetrievalPlan.create(this.config); if (this.configDigest !== expected.configDigest || this.studySetDigest !== expected.studySetDigest || this.planDigest !== expected.planDigest) fail("CBIOPORTAL plan has drifted from its reviewed identity"); }
  static fromJSON(value: unknown): ReviewedCBioPortalRetrievalPlan {
    const raw = exactObject("CBIOPORTAL plan", value, ["schema", "config", "config_digest", "study_set_digest", "request_limit", "scope", "execution", "retention", "credentials", "plan_digest"]);
    if (raw.schema !== REVIEWED_CBIOPORTAL_PLAN_SCHEMA) fail("CBIOPORTAL plan has an invalid shape");
    const plan = ReviewedCBioPortalRetrievalPlan.create(ReviewedCBioPortalRetrievalConfig.fromJSON(raw.config));
    if (canonicalJson(plan.toJSON()) !== canonicalJson(raw)) fail("CBIOPORTAL plan is not normalized or its digest is invalid");
    return plan;
  }
}

export type CbioPortalStudyMetadata = Record<string, JsonValue>;
export type ReviewedCBioPortalRetrievalReceiptJSON = Record<string, JsonValue>;
export type ReviewedCBioPortalTransientJSON = Record<string, JsonValue>;
export class ReviewedCBioPortalRetrievalResult {
  private readonly bundle: Record<string, JsonValue>; private readonly receiptValue: Record<string, JsonValue>;
  constructor(bundle: Record<string, JsonValue>, receipt: Record<string, JsonValue>) { this.bundle = JSON.parse(canonicalJson(bundle)) as Record<string, JsonValue>; this.receiptValue = JSON.parse(canonicalJson(receipt)) as Record<string, JsonValue>; NativeObjectFreeze(this); }
  get receipt(): Record<string, JsonValue> { return JSON.parse(canonicalJson(this.receiptValue)) as Record<string, JsonValue>; }
  toJSON(): { receipt: Record<string, JsonValue>; retention: string } { return { receipt: JSON.parse(canonicalJson(this.receiptValue)) as Record<string, JsonValue>, retention: "aggregate_metadata_only" }; }
  toTransientJSON(): ReviewedCBioPortalTransientJSON { return { schema: REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA, bundle: JSON.parse(canonicalJson(this.bundle)) as JsonValue, receipt: JSON.parse(canonicalJson(this.receiptValue)) as JsonValue, retention: "caller_owned_transient_study_profile_metadata" }; }
}
export type ReviewedCBioPortalFetch = (url: string, signal: AbortSignal) => Promise<unknown> | unknown;
export interface ReviewedCBioPortalRetrievalAdapterOptions { fetch?: ReviewedCBioPortalFetch }
export class ReviewedCBioPortalRetrievalAdapter {
  readonly config: ReviewedCBioPortalRetrievalConfig; private readonly fetcher?: ReviewedCBioPortalFetch;
  constructor(config: ReviewedCBioPortalRetrievalConfig, options: ReviewedCBioPortalRetrievalAdapterOptions = {}) {
    if (!(config instanceof ReviewedCBioPortalRetrievalConfig)) fail("CBIOPORTAL adapter requires an exact config");
    if (options.fetch !== undefined && typeof options.fetch !== "function") fail("CBIOPORTAL injected transport is malformed");
    if (options.fetch && config.transportId === BUILTIN_CBIOPORTAL_TRANSPORT_ID) fail("CBIOPORTAL injected transport requires a distinct reviewed identity");
    if (!options.fetch && (config.transportId !== BUILTIN_CBIOPORTAL_TRANSPORT_ID || config.transportConfigDigest !== BUILTIN_CBIOPORTAL_TRANSPORT_CONFIG_DIGEST)) fail("CBIOPORTAL built-in transport identity is not exact");
    this.config = config; this.fetcher = options.fetch; NativeObjectFreeze(this);
  }
  prepare(): ReviewedCBioPortalRetrievalPlan { return ReviewedCBioPortalRetrievalPlan.create(this.config); }
  async execute(plan: ReviewedCBioPortalRetrievalPlan, options: { approveSourceDispatch: boolean; retrievedAt?: string }): Promise<ReviewedCBioPortalRetrievalResult> {
    if (!(plan instanceof ReviewedCBioPortalRetrievalPlan)) fail("CBIOPORTAL execution requires an exact reviewed plan");
    plan.validate();
    if (canonicalJson(plan.config.toJSON()) !== canonicalJson(this.config.toJSON())) fail("CBIOPORTAL execution config differs from its reviewed plan");
    if (options.approveSourceDispatch !== true) fail("CBIOPORTAL dispatch requires literal approval");
    const retrievedAt = options.retrievedAt === undefined ? now() : timestamp("CBIOPORTAL retrievedAt", options.retrievedAt);
    const studies: Record<string, JsonValue>[] = []; const molecularProfiles: Record<string, JsonValue>[] = []; const sources: Record<string, JsonValue>[] = []; const sourceReceipts: Record<string, JsonValue>[] = []; let responseBytes = 0;
    for (const studyId of this.config.studyIds) {
      let studyRaw: unknown;
      try { studyRaw = this.fetcher ? await this.fetcher(studyUrl(studyId), new NativeAbortController().signal) : await builtinFetch(studyUrl(studyId), this.config.timeoutMs); }
      catch (error) { if (error instanceof ReviewedCBioPortalRetrievalError) throw error; throw new ReviewedCBioPortalRetrievalError("cBioPortal metadata request failed"); }
      const parsedStudy = parseResponse(studyRaw); responseBytes += parsedStudy.bytes;
      if (responseBytes > MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES) fail("CBIOPORTAL aggregate response bytes exceed the plan bound");
      const study = normalizeStudy(parsedStudy.payload, studyId);
      let profileRaw: unknown;
      try { profileRaw = this.fetcher ? await this.fetcher(profilesUrl(studyId), new NativeAbortController().signal) : await builtinFetch(profilesUrl(studyId), this.config.timeoutMs); }
      catch (error) { if (error instanceof ReviewedCBioPortalRetrievalError) throw error; throw new ReviewedCBioPortalRetrievalError("cBioPortal metadata request failed"); }
      const parsedProfiles = parseResponse(profileRaw); responseBytes += parsedProfiles.bytes;
      if (responseBytes > MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES) fail("CBIOPORTAL aggregate response bytes exceed the plan bound");
      const profiles = normalizeProfiles(parsedProfiles.payload, studyId); const sourceDigest = digestJsonSync({ study, molecular_profiles: profiles }); const sourceId = study.source_id as string;
      sources.push({ source_id: sourceId, authority: REVIEWED_CBIOPORTAL_AUTHORITY, uri: studyUrl(studyId), profile_uri: profilesUrl(studyId), retrieved_at: retrievedAt, content_sha256: sourceDigest, record_count: 1 + profiles.length, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] });
      sourceReceipts.push({ schema: REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA, lane: study.lane as string, source_id: sourceId, study_id: studyId, content_digest: sourceDigest, profile_count: profiles.length, metadata_completeness: study.metadata_completeness === "complete" && profiles.length > 0 ? "complete" : "unknown" });
      studies.push(study);
      molecularProfiles.push(...profiles);
    }
    const completeness = sourceReceipts.every((receipt) => receipt.metadata_completeness === "complete") ? "complete" : "unknown";
    const sourceSetDigest = digestJsonSync(sources);
    const bundleUnsigned = { schema: REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA, generated_at: retrievedAt, sources, studies, molecular_profiles: molecularProfiles, source_set_digest: sourceSetDigest, study_count: studies.length, profile_count: molecularProfiles.length, completeness, provider: "none", credentials: "not_accepted", limitations: [...LIMITATIONS] };
    if (byteLength(canonicalJson(bundleUnsigned)) > MAX_REVIEWED_CBIOPORTAL_BUNDLE_BYTES) fail("CBIOPORTAL bundle exceeds its byte bound");
    const bundle = { ...bundleUnsigned, bundle_digest: digestJsonSync(bundleUnsigned) };
    const receiptUnsigned = { schema: REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA, plan_digest: plan.planDigest, config_digest: plan.configDigest, study_set_digest: plan.studySetDigest, bundle_digest: bundle.bundle_digest, source_set_digest: sourceSetDigest, source_count: sources.length, study_count: studies.length, profile_count: molecularProfiles.length, request_count: 2 * studies.length, response_bytes: responseBytes, completeness, retrieved_at: retrievedAt, source_receipts: sourceReceipts, provider: "none", network: this.fetcher ? "caller_transport" : "builtin_https", effect: "read_only", retention: RETENTION, credentials: "not_accepted", limitations: [...LIMITATIONS] };
    const receipt = { ...receiptUnsigned, receipt_digest: digestJsonSync(receiptUnsigned) };
    return new ReviewedCBioPortalRetrievalResult(bundle as unknown as Record<string, JsonValue>, receipt as unknown as Record<string, JsonValue>);
  }
}

export interface ReviewedCBioPortalExecutionMetadata extends Record<string, JsonValue> {
  schema: typeof REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA; reviewed_plan_digest: string; approve_source_dispatch: true;
  retrieved_at: string | null; retention: "metadata_only"; credentials: "not_accepted"; metadata_digest: string;
}
export function createReviewedCBioPortalExecutionMetadata(plan: ReviewedCBioPortalRetrievalPlan, approveSourceDispatch: boolean, retrievedAt?: string): ReviewedCBioPortalExecutionMetadata {
  if (!(plan instanceof ReviewedCBioPortalRetrievalPlan)) fail("CBIOPORTAL execution metadata requires an exact plan");
  plan.validate(); if (approveSourceDispatch !== true) fail("CBIOPORTAL execution metadata requires literal approval");
  const payload = { schema: REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA, reviewed_plan_digest: plan.planDigest, approve_source_dispatch: true as const, retrieved_at: retrievedAt === undefined ? null : timestamp("CBIOPORTAL retrievedAt", retrievedAt), retention: "metadata_only" as const, credentials: "not_accepted" as const };
  return { ...payload, metadata_digest: digestJsonSync(payload) };
}

function validateTransient(value: unknown, plan: ReviewedCBioPortalRetrievalPlan, expectedNetwork: string): { bundle: Record<string, unknown>; receipt: Record<string, unknown> } {
  const transient = exactObject("CBIOPORTAL transient value", value, ["schema", "bundle", "receipt", "retention"]);
  if (transient.schema !== REVIEWED_CBIOPORTAL_TRANSIENT_SCHEMA || transient.retention !== "caller_owned_transient_study_profile_metadata") fail("CBIOPORTAL transient value is malformed");
  const bundle = exactObject("CBIOPORTAL transient bundle", transient.bundle, ["schema", "generated_at", "sources", "studies", "molecular_profiles", "source_set_digest", "study_count", "profile_count", "completeness", "provider", "credentials", "limitations", "bundle_digest"]);
  const receipt = exactObject("CBIOPORTAL transient receipt", transient.receipt, ["schema", "plan_digest", "config_digest", "study_set_digest", "bundle_digest", "source_set_digest", "source_count", "study_count", "profile_count", "request_count", "response_bytes", "completeness", "retrieved_at", "source_receipts", "provider", "network", "effect", "retention", "credentials", "limitations", "receipt_digest"]);
  const bundleUnsigned = Object.fromEntries(Object.entries(bundle).filter(([key]) => key !== "bundle_digest"));
  const receiptUnsigned = Object.fromEntries(Object.entries(receipt).filter(([key]) => key !== "receipt_digest"));
  if (digestJsonSync(bundleUnsigned) !== digest("CBIOPORTAL bundle digest", bundle.bundle_digest) || digestJsonSync(receiptUnsigned) !== digest("CBIOPORTAL receipt digest", receipt.receipt_digest)) fail("CBIOPORTAL transient digest is invalid");
  if (bundle.schema !== REVIEWED_CBIOPORTAL_BUNDLE_SCHEMA || receipt.schema !== REVIEWED_CBIOPORTAL_RECEIPT_SCHEMA || receipt.plan_digest !== plan.planDigest || receipt.config_digest !== plan.configDigest || receipt.study_set_digest !== plan.studySetDigest) fail("CBIOPORTAL transient identity differs from its reviewed plan");
  if (receipt.network !== expectedNetwork || receipt.effect !== "read_only" || receipt.provider !== "none" || receipt.credentials !== "not_accepted" || receipt.retention !== RETENTION || bundle.provider !== "none" || bundle.credentials !== "not_accepted") fail("CBIOPORTAL transient boundary metadata is invalid");
  if (canonicalJson(bundle.limitations) !== canonicalJson(LIMITATIONS) || canonicalJson(receipt.limitations) !== canonicalJson(LIMITATIONS)) fail("CBIOPORTAL transient limitations are invalid");
  const retrievedAt = timestamp("CBIOPORTAL retrievedAt", receipt.retrieved_at);
  if (timestamp("CBIOPORTAL generatedAt", bundle.generated_at) !== retrievedAt) fail("CBIOPORTAL bundle and receipt timestamps do not match");
  const studies = bundle.studies; const profiles = bundle.molecular_profiles; const sources = bundle.sources; const sourceReceipts = receipt.source_receipts;
  if (!Array.isArray(studies) || !Array.isArray(profiles) || !Array.isArray(sources) || !Array.isArray(sourceReceipts) || studies.length !== plan.config.studyIds.length || sources.length !== studies.length || sourceReceipts.length !== studies.length || profiles.length > MAX_REVIEWED_CBIOPORTAL_STUDIES * MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY) fail("CBIOPORTAL transient study coverage is incomplete");
  if (profiles.some((profile) => typeof profile !== "object" || profile === null || !plan.config.studyIds.includes((profile as Record<string, unknown>).study_id as ReviewedCBioPortalStudyId))) fail("cBioPortal transient profiles contain an unreviewed study");
  if (digestJsonSync(sources) !== bundle.source_set_digest || receipt.source_set_digest !== bundle.source_set_digest || receipt.bundle_digest !== bundle.bundle_digest) fail("CBIOPORTAL transient source binding is invalid");
  const expectedAllProfiles: Record<string, JsonValue>[] = [];
  for (const [index, studyId] of plan.config.studyIds.entries()) {
    const study = studies[index]; const source = exactObject("CBIOPORTAL source metadata", sources[index], ["source_id", "authority", "uri", "profile_uri", "retrieved_at", "content_sha256", "record_count", "provider", "credentials", "limitations"]);
    const sourceReceipt = exactObject("CBIOPORTAL source receipt", sourceReceipts[index], ["schema", "lane", "source_id", "study_id", "content_digest", "profile_count", "metadata_completeness"]);
    if (typeof study !== "object" || study === null || Array.isArray(study)) fail("CBIOPORTAL study metadata is malformed");
    const row = study as Record<string, unknown>;
    const normalized = normalizeStudy({ studyId, name: row.name, description: row.description, cancerTypeId: row.cancer_type_id, publicStudy: row.public_study, pmid: row.pmid, allSampleCount: row.sample_count, referenceGenome: row.reference_genome, importDate: row.import_date }, studyId);
    const studyProfiles = profiles.filter((profile) => (profile as Record<string, unknown>).study_id === studyId) as Record<string, unknown>[];
    const reconstructed = studyProfiles.map((profile) => ({ studyId, molecularProfileId: profile.profile_id, name: profile.name, molecularAlterationType: profile.molecular_alteration_type, genericAssayType: profile.generic_assay_type, datatype: profile.datatype, description: profile.description, showProfileInAnalysisTab: profile.show_in_analysis, patientLevel: profile.patient_level }));
    const normalizedProfiles = normalizeProfiles(reconstructed, studyId);
    if (canonicalJson(studyProfiles) !== canonicalJson(normalizedProfiles)) fail("cBioPortal transient molecular profiles are not normalized");
    if (normalizedProfiles.length >= MAX_REVIEWED_CBIOPORTAL_PROFILES_PER_STUDY) fail("cBioPortal transient profile catalogue may be truncated");
    expectedAllProfiles.push(...normalizedProfiles);
    const sourceDigest = digestJsonSync({ study: normalized, molecular_profiles: normalizedProfiles }); const sourceId = normalized.source_id as string;
    const profileCompleteness = normalized.metadata_completeness === "complete" && normalizedProfiles.length > 0 ? "complete" : "unknown";
    if (canonicalJson(row) !== canonicalJson(normalized) || source.source_id !== sourceId || source.authority !== REVIEWED_CBIOPORTAL_AUTHORITY || source.uri !== studyUrl(studyId) || source.profile_uri !== profilesUrl(studyId) || source.retrieved_at !== retrievedAt || source.content_sha256 !== sourceDigest || source.record_count !== 1 + normalizedProfiles.length || source.provider !== "none" || source.credentials !== "not_accepted" || canonicalJson(source.limitations) !== canonicalJson(LIMITATIONS)) fail("CBIOPORTAL source metadata does not match its study");
    const expectedSourceReceipt = { schema: REVIEWED_CBIOPORTAL_SOURCE_RECEIPT_SCHEMA, lane: STUDY_TO_LANE[studyId], source_id: sourceId, study_id: studyId, content_digest: sourceDigest, profile_count: normalizedProfiles.length, metadata_completeness: profileCompleteness };
    if (canonicalJson(sourceReceipt) !== canonicalJson(expectedSourceReceipt)) fail("CBIOPORTAL source receipt does not match its study");
  }
  if (canonicalJson(profiles) !== canonicalJson(expectedAllProfiles)) fail("cBioPortal transient profile order or coverage is invalid");
  const expectedCompleteness = sourceReceipts.every((sourceReceipt) => (sourceReceipt as Record<string, unknown>).metadata_completeness === "complete") ? "complete" : "unknown";
  if (receipt.request_count !== studies.length * 2 || receipt.source_count !== studies.length || receipt.study_count !== studies.length || bundle.study_count !== studies.length || receipt.profile_count !== profiles.length || bundle.profile_count !== profiles.length || receipt.completeness !== expectedCompleteness || bundle.completeness !== expectedCompleteness) fail("CBIOPORTAL transient counts or completeness are inconsistent");
  integer("CBIOPORTAL response_bytes", receipt.response_bytes, studies.length * 2, MAX_REVIEWED_CBIOPORTAL_TOTAL_RESPONSE_BYTES);
  return { bundle, receipt };
}

export function createReviewedCBioPortalAutonomousEvidenceRegistration(adapter: ReviewedCBioPortalRetrievalAdapter, plan: ReviewedCBioPortalRetrievalPlan, studyId: ReviewedCBioPortalStudyId): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  if (!(adapter instanceof ReviewedCBioPortalRetrievalAdapter) || !(plan instanceof ReviewedCBioPortalRetrievalPlan)) fail("CBIOPORTAL registration requires exact adapter and plan values");
  plan.validate();
  if (canonicalJson(adapter.config.toJSON()) !== canonicalJson(plan.config.toJSON()) || !plan.config.studyIds.includes(studyId) || plan.config.studyIds.length !== 1) fail("CBIOPORTAL registration requires the exact single-study plan");
  const frozenPlan = ReviewedCBioPortalRetrievalPlan.create(plan.config); const lane = STUDY_TO_LANE[studyId]; const sourceId = `cbioportal_${lane}`;
  const expectedNetwork = adapter.config.transportId === BUILTIN_CBIOPORTAL_TRANSPORT_ID ? "builtin_https" : "caller_transport";
  return {
    adapterId: `reviewed.cbioportal.${lane}`, version: REVIEWED_CBIOPORTAL_ADAPTER_VERSION, domains: ["biomedical", "neuroscience"],
    capabilities: ["aggregate_cohort_landscape", "source_provenance"], sourceKinds: ["cbioportal_public_study_profile_metadata"],
    acquire: async (context): Promise<JsonValue> => {
      const request = context?.request;
      if (!request || request.source_id !== sourceId || request.source_digest !== frozenPlan.planDigest) fail("CBIOPORTAL acquisition request does not match its reviewed source");
      const metadata = exactObject("CBIOPORTAL execution metadata", request.metadata, ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at", "retention", "credentials", "metadata_digest"]);
      const unsigned = { ...metadata }; const supplied = digest("CBIOPORTAL metadata digest", unsigned.metadata_digest); delete unsigned.metadata_digest;
      if (supplied !== digestJsonSync(unsigned) || metadata.schema !== REVIEWED_CBIOPORTAL_EXECUTION_METADATA_SCHEMA || metadata.reviewed_plan_digest !== frozenPlan.planDigest || metadata.approve_source_dispatch !== true || metadata.retention !== "metadata_only" || metadata.credentials !== "not_accepted") fail("CBIOPORTAL execution metadata failed review binding");
      const retrievedAt = metadata.retrieved_at === null ? undefined : timestamp("CBIOPORTAL retrievedAt", metadata.retrieved_at);
      return (await adapter.execute(frozenPlan, { approveSourceDispatch: true, ...(retrievedAt ? { retrievedAt } : {}) })).toTransientJSON() as unknown as JsonValue;
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      const { receipt } = validateTransient(value, frozenPlan, expectedNetwork); const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("CBIOPORTAL projection has no requirement label");
      return [{ label, kind: "provenance", status: "observed", value_digest: receipt.bundle_digest as string, source_digest: receipt.source_set_digest as string, confidence: null, limitations: [...LIMITATIONS] }];
    },
  };
}

export type ReviewedCBioPortalSourceReceipt = Record<string, JsonValue>;
export type ReviewedCBioPortalBundle = Record<string, JsonValue>;
