import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousEvidenceAdapterRegistrationInput } from "./autonomous-evidence-adapters.js";
import type { AutonomousEvidenceAcquisitionContext, AutonomousEvidenceObservationInput } from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestBytesSync, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

// Retrieval awaits caller-controlled transports. Keep every primitive that turns bytes into
// reviewed data, builds an authorized URL, or freezes a public artifact independent of mutable
// ambient globals that a transport can replace while it is running.
const NativeTextEncoder = globalThis.TextEncoder;
const nativeEncodeUtf8 = NativeTextEncoder.prototype.encode.bind(new NativeTextEncoder());
const NativeTextDecoder = globalThis.TextDecoder;
const nativeDecodeUtf8 = NativeTextDecoder.prototype.decode.bind(new NativeTextDecoder("utf-8", { fatal: true }));
const nativeDecodeUtf8Lossy = NativeTextDecoder.prototype.decode.bind(new NativeTextDecoder("utf-8"));
const nativeJsonParse = globalThis.JSON.parse.bind(globalThis.JSON);
const nativeJsonStringify = globalThis.JSON.stringify.bind(globalThis.JSON);
const nativeEncodeURIComponent = globalThis.encodeURIComponent;
const nativeArrayIsArray = globalThis.Array.isArray.bind(globalThis.Array);
const nativeObjectKeys = globalThis.Object.keys.bind(globalThis.Object);
const nativeObjectEntries = globalThis.Object.entries.bind(globalThis.Object);
const nativeObjectFromEntries = globalThis.Object.fromEntries.bind(globalThis.Object);
const nativeObjectGetPrototypeOf = globalThis.Object.getPrototypeOf.bind(globalThis.Object);
const nativeObjectGetOwnPropertyDescriptors = globalThis.Object.getOwnPropertyDescriptors.bind(globalThis.Object);
const nativeObjectCreate = globalThis.Object.create.bind(globalThis.Object);
const nativeObjectAssign = globalThis.Object.assign.bind(globalThis.Object);
const nativeObjectDefineProperty = globalThis.Object.defineProperty.bind(globalThis.Object);
const nativeObjectFreeze = globalThis.Object.freeze.bind(globalThis.Object);
const nativeReflectOwnKeys = globalThis.Reflect.ownKeys.bind(globalThis.Reflect);
const nativeHasOwnProperty = Function.prototype.call.bind(Object.prototype.hasOwnProperty) as (value: object, key: PropertyKey) => boolean;
const NativeUint8Array = globalThis.Uint8Array;
const NativeArrayBuffer = globalThis.ArrayBuffer;
const nativeUint8ArraySlice = Function.prototype.call.bind(NativeUint8Array.prototype.slice) as (value: Uint8Array, start?: number, end?: number) => Uint8Array;
const nativeUint8ArraySet = Function.prototype.call.bind(NativeUint8Array.prototype.set) as (value: Uint8Array, source: ArrayLike<number>, offset?: number) => void;
const nativeArrayBufferSlice = Function.prototype.call.bind(NativeArrayBuffer.prototype.slice) as (value: ArrayBuffer, start?: number, end?: number) => ArrayBuffer;
const NativePromise = globalThis.Promise;
const nativePromiseRace = NativePromise.race.bind(NativePromise) as <T>(values: Iterable<T | PromiseLike<T>>) => Promise<Awaited<T>>;

/** Reviewed, bounded live PubMed retrieval for the autonomous evidence plane. */
export const REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA = "bioprism-typescript-reviewed-pubmed-retrieval-config/0.1" as const;
export const REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA = "bioprism-typescript-reviewed-pubmed-retrieval-plan/0.1" as const;
export const REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA = "bioprism-typescript-reviewed-pubmed-source-receipt/0.1" as const;
export const REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA = "bioprism-typescript-reviewed-pubmed-retrieval-receipt/0.1" as const;
export const REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA = "bioprism-typescript-reviewed-pubmed-transient-value/0.1" as const;
export const REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA = "bioprism-typescript-reviewed-pubmed-execution-metadata/0.1" as const;
export const REVIEWED_PUBMED_QUERY_SET_SCHEMA = "bioprism-typescript-reviewed-pubmed-query-set/0.1" as const;
export const REVIEWED_PUBMED_NCBI_REGISTRATION_SCHEMA = "bioprism-typescript-reviewed-pubmed-ncbi-registration/0.1" as const;
export const REVIEWED_PUBMED_ADAPTER_VERSION = "0.1" as const;
export const REVIEWED_PUBMED_HOST = "eutils.ncbi.nlm.nih.gov" as const;
export const REVIEWED_PUBMED_ENDPOINTS = nativeObjectFreeze(["esearch.fcgi", "esummary.fcgi", "efetch.fcgi"] as const);

export const PUBLIC_LITERATURE_SCHEMA_VERSION = "bioprism-neurosurgery-public-literature/0.1" as const;
export const PUBMED_AUTHORITY = "U.S. National Library of Medicine PubMed" as const;
export const MAX_PUBMED_LANES = 6;
export const MAX_PER_SPECIALTY_LIMIT = 50;
export const MAX_REVIEWED_PUBMED_REQUESTS = MAX_PUBMED_LANES * REVIEWED_PUBMED_ENDPOINTS.length;
export const MAX_REVIEWED_PUBMED_RECORDS = MAX_PUBMED_LANES * MAX_PER_SPECIALTY_LIMIT;
export const MAX_REVIEWED_PUBMED_RESPONSE_BYTES = 8_000_000;
export const MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES = 64_000_000;
export const MAX_REVIEWED_PUBMED_BUNDLE_BYTES = 8_000_000;
export const MAX_REVIEWED_PUBMED_RESPONSE_DEPTH = 32;
export const MAX_REVIEWED_PUBMED_RESPONSE_NODES = 100_000;
export const MAX_REVIEWED_PUBMED_ARTIFACT_BYTES = 64_000;
export const MAX_REVIEWED_PUBMED_ABSTRACT_BYTES = 12_000;
export const MAX_REVIEWED_PUBMED_TEXT_BYTES = 16_000;
export const MAX_REVIEWED_PUBMED_TAGS = 64;

export const BUILTIN_PUBMED_TRANSPORT_ID = "builtin.ncbi_eutils.fetch" as const;
export const BUILTIN_PUBMED_TRANSPORT_VERSION = "1" as const;
export const BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST = digestJsonSync({
  implementation: "web_fetch_stream",
  method: "GET",
  scheme: "https",
  host: REVIEWED_PUBMED_HOST,
  paths: REVIEWED_PUBMED_ENDPOINTS.map((endpoint) => `/entrez/eutils/${endpoint}`),
  redirects: "refused",
  rate_limit: "at_most_three_requests_per_second",
  request_body: "none",
  registration_parameters: "optional_registered_tool_and_developer_email",
  secret_material: "not_accepted",
});

const SPECIALTY_ENTRIES = nativeObjectFreeze([
  ["glioma", '(glioma OR glioblastoma OR astrocytoma OR oligodendroglioma OR "diffuse midline glioma") AND (molecular OR genomic OR pseudoprogression OR "radiation necrosis")'],
  ["cranial_base", '((skull base) OR (cranial base) OR petroclival OR "cavernous sinus" OR "cranial nerve" OR "CSF leak") AND (neurosurgery OR surgery)'],
  ["craniosynostosis", '(craniosynostosis OR scaphocephaly OR plagiocephaly OR "Apert syndrome" OR "Crouzon syndrome" OR "Pfeiffer syndrome")'],
  ["encephalocele", '(encephalocele OR meningoencephalocele OR "basal encephalocele" OR "occipital encephalocele" OR "CSF rhinorrhea")'],
  ["spina_bifida", '((spina bifida) OR (spinal dysraphism) OR myelomeningocele OR lipomeningocele OR "tethered cord" OR "neurogenic bladder" OR diastematomyelia)'],
  ["chiari_malformation", '((Chiari malformation) OR (craniocervical junction) OR syringomyelia OR "cine MRI" OR "CSF flow" OR "clivo-axial angle" OR "basilar invagination")'],
] as const);

export type ReviewedPubMedSpecialtyLane = typeof SPECIALTY_ENTRIES[number][0];
export const PUBMED_SPECIALTY_LANES: Readonly<Record<ReviewedPubMedSpecialtyLane, string>> = nativeObjectFreeze(nativeObjectFromEntries(SPECIALTY_ENTRIES) as Record<ReviewedPubMedSpecialtyLane, string>);

const SPECIALTY_LANES = nativeObjectFreeze(SPECIALTY_ENTRIES.map(([lane]) => lane));
const DIGEST_RE = /^[0-9a-f]{64}$/;
const IDENTIFIER_RE = /^[A-Za-z0-9_.:+-]+$/;
const UTC_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/;
const NCBI_TOOL_RE = /^[A-Za-z0-9_.:+-]{1,128}$/;
const NCBI_EMAIL_RE = /^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]{1,64}@(?:[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?\.)+[A-Za-z]{2,63}$/;
const CONTROL_RE = /[\u0000-\u001f\u007f]/;
const SYNTHETIC_MARKERS = ["synthetic fixture", "synthetic case", "synthetic patient", "synthetic cohort", "generated fixture", "fake patient"] as const;
const CONFIG_EXECUTION = "review_configuration_only;no_source_dispatch" as const;
const PLAN_EXECUTION = "explicit_literal_approval_required;bounded_public_https_get_only" as const;
const CONFIG_RETENTION = "metadata_only;query_transport_and_ncbi_contact_values_excluded" as const;
const RECEIPT_RETENTION = "metadata_only;transient_bundle_transport_and_ncbi_contact_values_excluded" as const;
const TRANSIENT_RETENTION = "caller_owned_transient_value;do_not_persist_without_separate_policy" as const;
const SECRET_MATERIAL = "api_keys_and_secrets_never_accepted_or_returned" as const;
const LIMITATIONS = nativeObjectFreeze([
  "PubMed metadata and abstracts are source text, not verified scientific conclusions",
  "retrieval coverage is limited to the selected fixed specialty lanes and record window",
  "deduplication is PMID-only and partial publication dates remain unknown",
  "a qualified reviewer must assess omissions, study quality, freshness, and applicability",
  "tool and developer email values must be registered separately with NCBI before use",
  "the registration digest is an integrity binding, not anonymization of guessable contact values",
  "built-in rate limiting is process-shared; deployments must coordinate the limit across processes",
  "same-process callable behavior beyond captured identity is caller-controlled",
] as const);

type Endpoint = typeof REVIEWED_PUBMED_ENDPOINTS[number];
type NcbiRegistration = readonly [] | readonly [readonly ["tool", string], readonly ["email", string]];

export class ReviewedPubMedRetrievalError extends ArgumentError {
  override readonly name = "ReviewedPubMedRetrievalError";
}

function fail(message: string): never {
  throw new ReviewedPubMedRetrievalError(message);
}

function bytes(value: string): number {
  return nativeEncodeUtf8(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value || value !== value.trim() || value.includes("\u0000") || bytes(value) > maximum) fail(`${name} is outside its bounded text contract`);
  return value as string;
}

function identifier(name: string, value: unknown, maximum = 256): string {
  const result = boundedText(name, value, maximum);
  if (!IDENTIFIER_RE.test(result)) fail(`${name} is outside its identifier contract`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !DIGEST_RE.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) fail(`${name} must be an integer between ${minimum} and ${maximum}`);
  return value as number;
}

function finiteNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) fail(`${name} must be a finite number between ${minimum} and ${maximum}`);
  return value;
}

function timestamp(name: string, value: unknown): string {
  if (typeof value !== "string" || !UTC_RE.test(value)) fail(`${name} must be a whole-second UTC timestamp`);
  const parsed = new Date(value);
  if (!Number.isFinite(parsed.getTime()) || parsed.toISOString().replace(".000Z", "Z") !== value) fail(`${name} must be a valid UTC timestamp`);
  return value;
}

function hasExactEnumerableKeys(value: object, expectedKeys: readonly string[]): boolean {
  const actual = nativeObjectKeys(value);
  if (actual.length !== expectedKeys.length) return false;
  for (let index = 0; index < expectedKeys.length; index += 1) {
    if (!nativeHasOwnProperty(value, expectedKeys[index]!)) return false;
  }
  return true;
}

function exactObject(name: string, value: unknown, expectedKeys: readonly string[]): JsonObject {
  if (typeof value !== "object" || value === null || nativeArrayIsArray(value)) fail(`${name} must be an object`);
  if (!hasExactEnumerableKeys(value, expectedKeys)) fail(`${name} must contain exactly its schema fields`);
  return value as unknown as JsonObject;
}

function enumerableWithout(value: JsonObject, omittedKey: string): JsonObject {
  const result = nativeObjectCreate(null) as JsonObject;
  const keys = nativeObjectKeys(value);
  for (let index = 0; index < keys.length; index += 1) {
    const key = keys[index]!;
    if (key !== omittedKey) nativeObjectDefineProperty(result, key, { value: value[key] as JsonValue, enumerable: true, writable: true, configurable: true });
  }
  return result;
}

interface JsonSnapshotState { nodes: number }

/** Snapshot JSON-shaped input through descriptors so validation never observes two getter views. */
function snapshotJsonValue(name: string, value: unknown, depth = 0, state: JsonSnapshotState = { nodes: 0 }): JsonValue {
  state.nodes += 1;
  if (state.nodes > MAX_REVIEWED_PUBMED_RESPONSE_NODES || depth > MAX_REVIEWED_PUBMED_RESPONSE_DEPTH) fail(`${name} exceeds its JSON snapshot bound`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) fail(`${name} contains a non-finite number`);
    return value;
  }
  if (typeof value !== "object") fail(`${name} contains a non-JSON value`);

  const descriptors = nativeObjectGetOwnPropertyDescriptors(value) as Record<PropertyKey, PropertyDescriptor>;
  const keys = nativeReflectOwnKeys(descriptors);
  if (nativeArrayIsArray(value)) {
    const lengthDescriptor = descriptors.length;
    const length = lengthDescriptor?.value;
    if (!lengthDescriptor || !nativeHasOwnProperty(lengthDescriptor, "value") || !Number.isSafeInteger(length) || length < 0 || keys.length !== length + 1) fail(`${name} contains a sparse or extended array`);
    const snapshot: JsonValue[] = [];
    for (let index = 0; index < length; index += 1) {
      const descriptor = descriptors[index];
      if (!descriptor || descriptor.enumerable !== true || !nativeHasOwnProperty(descriptor, "value")) fail(`${name} must contain only enumerable data properties`);
      nativeObjectDefineProperty(snapshot, index, {
        value: snapshotJsonValue(`${name}[${index}]`, descriptor.value, depth + 1, state),
        enumerable: true,
        writable: true,
        configurable: true,
      });
    }
    return snapshot;
  }

  const snapshot = nativeObjectCreate(null) as Record<string, JsonValue>;
  for (let index = 0; index < keys.length; index += 1) {
    const key = keys[index]!;
    if (typeof key !== "string") fail(`${name} contains a symbol property`);
    const descriptor = descriptors[key];
    if (!descriptor || descriptor.enumerable !== true || !nativeHasOwnProperty(descriptor, "value")) fail(`${name} must contain only enumerable data properties`);
    nativeObjectDefineProperty(snapshot, key, {
      value: snapshotJsonValue(`${name}.${key}`, descriptor.value, depth + 1, state),
      enumerable: true,
      writable: true,
      configurable: true,
    });
  }
  return snapshot;
}

function cloneJson<T>(name: string, value: T): T {
  let encoded: string | undefined;
  try {
    encoded = nativeJsonStringify(value);
  } catch {
    fail(`${name} must be JSON`);
  }
  if (encoded === undefined) fail(`${name} must be JSON`);
  return nativeJsonParse(encoded) as T;
}

function boundedArtifact(name: string, value: unknown, maximum = MAX_REVIEWED_PUBMED_ARTIFACT_BYTES): void {
  let encoded: string;
  try {
    encoded = canonicalJson(value);
  } catch {
    fail(`${name} must be canonical JSON`);
  }
  if (bytes(encoded) > maximum) fail(`${name} exceeds its byte bound`);
}

function normalizeLanes(value: unknown, name = "specialtyLanes"): ReviewedPubMedSpecialtyLane[] {
  if (!nativeArrayIsArray(value) || value.length < 1 || value.length > MAX_PUBMED_LANES) fail(`${name} must contain 1..${MAX_PUBMED_LANES} lanes`);
  const selected = nativeObjectCreate(null) as Record<string, true>;
  for (let index = 0; index < value.length; index += 1) {
    const candidate = value[index];
    let allowed = false;
    for (let laneIndex = 0; laneIndex < SPECIALTY_LANES.length; laneIndex += 1) if (candidate === SPECIALTY_LANES[laneIndex]) allowed = true;
    if (typeof candidate !== "string" || !allowed) fail(`${name} contains a non-allow-listed lane`);
    if (nativeHasOwnProperty(selected, candidate)) fail(`${name} contains duplicate lanes`);
    nativeObjectDefineProperty(selected, candidate, { value: true, enumerable: true, writable: false, configurable: false });
  }
  const normalized: ReviewedPubMedSpecialtyLane[] = [];
  for (let index = 0; index < SPECIALTY_LANES.length; index += 1) {
    const lane = SPECIALTY_LANES[index]!;
    if (nativeHasOwnProperty(selected, lane)) normalized[normalized.length] = lane;
  }
  return normalized;
}

function queryEntries(lanes: readonly ReviewedPubMedSpecialtyLane[]): readonly (readonly [ReviewedPubMedSpecialtyLane, string])[] {
  const entries: Array<readonly [ReviewedPubMedSpecialtyLane, string]> = [];
  for (let entryIndex = 0; entryIndex < SPECIALTY_ENTRIES.length; entryIndex += 1) {
    const [lane, term] = SPECIALTY_ENTRIES[entryIndex]!;
    for (let laneIndex = 0; laneIndex < lanes.length; laneIndex += 1) {
      if (lanes[laneIndex] === lane) entries[entries.length] = nativeObjectFreeze([lane, term] as const);
    }
  }
  return nativeObjectFreeze(entries);
}

function querySetDigest(entries: readonly (readonly [ReviewedPubMedSpecialtyLane, string])[]): string {
  const queries: JsonObject[] = [];
  for (let index = 0; index < entries.length; index += 1) {
    const [lane, term] = entries[index]!;
    queries[index] = { specialty_lane: lane, query_digest: digestJsonSync({ term }) };
  }
  return digestJsonSync({
    schema: REVIEWED_PUBMED_QUERY_SET_SCHEMA,
    queries,
  });
}

function ncbiRegistration(tool: unknown, email: unknown): NcbiRegistration {
  if ((tool === undefined || tool === null) !== (email === undefined || email === null)) fail("ncbiTool and ncbiEmail must be provided together");
  if (tool === undefined || tool === null) return nativeObjectFreeze([]) as NcbiRegistration;
  if (typeof tool !== "string" || !NCBI_TOOL_RE.test(tool)) fail("ncbiTool must be a bounded application name without spaces");
  if (typeof email !== "string" || !/^[\x00-\x7f]+$/.test(email) || email.length > 254 || !NCBI_EMAIL_RE.test(email)) fail("ncbiEmail must be a bounded complete developer email address");
  return nativeObjectFreeze([nativeObjectFreeze(["tool", tool] as const), nativeObjectFreeze(["email", email] as const)]) as NcbiRegistration;
}

function ncbiRegistrationDigest(registration: NcbiRegistration): string {
  const parameters: JsonObject[] = [];
  for (let index = 0; index < registration.length; index += 1) {
    const [name, value] = registration[index]!;
    parameters[index] = { name, value };
  }
  return digestJsonSync({
    schema: REVIEWED_PUBMED_NCBI_REGISTRATION_SCHEMA,
    configured: registration.length > 0,
    parameters,
  });
}

const UNCONFIGURED_NCBI_REGISTRATION_DIGEST = ncbiRegistrationDigest(nativeObjectFreeze([]) as NcbiRegistration);

function scope(): JsonObject {
  const paths: string[] = [];
  for (let index = 0; index < REVIEWED_PUBMED_ENDPOINTS.length; index += 1) paths[index] = `/entrez/eutils/${REVIEWED_PUBMED_ENDPOINTS[index]!}`;
  return {
    scheme: "https",
    host: REVIEWED_PUBMED_HOST,
    paths,
    method: "GET",
    request_body: "none",
  };
}

interface ConfigData {
  specialtyLanes: readonly ReviewedPubMedSpecialtyLane[];
  perSpecialtyLimit: number;
  timeoutMs: number;
  responseByteLimit: number;
  totalResponseByteLimit: number;
  bundleByteLimit: number;
  transportId: string;
  transportVersion: string;
  transportConfigDigest: string;
  querySetDigest: string;
  registration: NcbiRegistration;
  registrationDigest: string;
}

export interface ReviewedPubMedRetrievalConfigOptions {
  specialtyLanes: readonly ReviewedPubMedSpecialtyLane[];
  perSpecialtyLimit?: number;
  timeoutMs?: number;
  responseByteLimit?: number;
  totalResponseByteLimit?: number;
  bundleByteLimit?: number;
  transportId?: string;
  transportVersion?: string;
  transportConfigDigest?: string;
  querySetDigest?: string;
  ncbiTool?: string;
  ncbiEmail?: string;
  ncbiRegistrationDigest?: string;
}

export interface ReviewedPubMedRetrievalConfigJSON extends JsonObject {
  schema: typeof REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA;
  specialty_lanes: ReviewedPubMedSpecialtyLane[];
  per_specialty_limit: number;
  timeout_ms: number;
  request_limit: number;
  record_limit: number;
  response_byte_limit: number;
  total_response_byte_limit: number;
  bundle_byte_limit: number;
  transport_id: string;
  transport_version: string;
  transport_config_digest: string;
  query_set_digest: string;
  ncbi_registration_configured: boolean;
  ncbi_registration_digest: string;
  scope: JsonObject;
  execution: typeof CONFIG_EXECUTION;
  retention: typeof CONFIG_RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  config_digest: string;
}

const CONFIG_INPUT_KEYS = new Set(["specialtyLanes", "perSpecialtyLimit", "timeoutMs", "responseByteLimit", "totalResponseByteLimit", "bundleByteLimit", "transportId", "transportVersion", "transportConfigDigest", "querySetDigest", "ncbiTool", "ncbiEmail", "ncbiRegistrationDigest"]);
const CONFIG_JSON_KEYS = ["schema", "specialty_lanes", "per_specialty_limit", "timeout_ms", "request_limit", "record_limit", "response_byte_limit", "total_response_byte_limit", "bundle_byte_limit", "transport_id", "transport_version", "transport_config_digest", "query_set_digest", "ncbi_registration_configured", "ncbi_registration_digest", "scope", "execution", "retention", "secret_material", "config_digest"] as const;
const CONFIG_PRIVATE = new WeakMap<ReviewedPubMedRetrievalConfig, ConfigData>();

function configPayload(data: ConfigData): Omit<ReviewedPubMedRetrievalConfigJSON, "config_digest"> {
  return {
    schema: REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA,
    specialty_lanes: [...data.specialtyLanes],
    per_specialty_limit: data.perSpecialtyLimit,
    timeout_ms: data.timeoutMs,
    request_limit: data.specialtyLanes.length * REVIEWED_PUBMED_ENDPOINTS.length,
    record_limit: data.specialtyLanes.length * data.perSpecialtyLimit,
    response_byte_limit: data.responseByteLimit,
    total_response_byte_limit: data.totalResponseByteLimit,
    bundle_byte_limit: data.bundleByteLimit,
    transport_id: data.transportId,
    transport_version: data.transportVersion,
    transport_config_digest: data.transportConfigDigest,
    query_set_digest: data.querySetDigest,
    ncbi_registration_configured: data.registration.length > 0,
    ncbi_registration_digest: data.registrationDigest,
    scope: scope(),
    execution: CONFIG_EXECUTION,
    retention: CONFIG_RETENTION,
    secret_material: SECRET_MATERIAL,
  };
}

function requireConfigData(config: ReviewedPubMedRetrievalConfig): ConfigData {
  const data = CONFIG_PRIVATE.get(config);
  if (!data || nativeObjectGetPrototypeOf(config) !== ReviewedPubMedRetrievalConfig.prototype) fail("reviewed PubMed retrieval requires an exact config");
  return data;
}

export class ReviewedPubMedRetrievalConfig {
  readonly specialty_lanes: readonly ReviewedPubMedSpecialtyLane[];
  readonly per_specialty_limit: number;
  readonly timeout_ms: number;
  readonly request_limit: number;
  readonly record_limit: number;
  readonly response_byte_limit: number;
  readonly total_response_byte_limit: number;
  readonly bundle_byte_limit: number;
  readonly transport_id: string;
  readonly transport_version: string;
  readonly transport_config_digest: string;
  readonly query_set_digest: string;
  readonly ncbi_registration_configured: boolean;
  readonly ncbi_registration_digest: string;
  readonly config_digest: string;

  constructor(options: ReviewedPubMedRetrievalConfigOptions) {
    if (!options || typeof options !== "object" || nativeArrayIsArray(options)) fail("reviewed PubMed config options must be an object");
    for (const key of nativeObjectKeys(options)) if (!CONFIG_INPUT_KEYS.has(key)) fail("reviewed PubMed config options contain unsupported or credential-shaped fields");
    const lanes = nativeObjectFreeze(normalizeLanes(options.specialtyLanes));
    const perSpecialtyLimit = integer("perSpecialtyLimit", options.perSpecialtyLimit ?? 10, 1, MAX_PER_SPECIALTY_LIMIT);
    const timeoutMs = finiteNumber("timeoutMs", options.timeoutMs ?? 30_000, 1_000, 120_000);
    const responseByteLimit = integer("responseByteLimit", options.responseByteLimit ?? MAX_REVIEWED_PUBMED_RESPONSE_BYTES, 256, MAX_REVIEWED_PUBMED_RESPONSE_BYTES);
    const totalResponseByteLimit = integer("totalResponseByteLimit", options.totalResponseByteLimit ?? 48_000_000, responseByteLimit, MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES);
    const bundleByteLimit = integer("bundleByteLimit", options.bundleByteLimit ?? MAX_REVIEWED_PUBMED_BUNDLE_BYTES, 1_024, MAX_REVIEWED_PUBMED_BUNDLE_BYTES);
    const transportId = identifier("transportId", options.transportId ?? BUILTIN_PUBMED_TRANSPORT_ID);
    const transportVersion = identifier("transportVersion", options.transportVersion ?? BUILTIN_PUBMED_TRANSPORT_VERSION);
    const transportConfigDigest = digest("transportConfigDigest", options.transportConfigDigest ?? BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST);
    const entries = queryEntries(lanes);
    const expectedQueryDigest = querySetDigest(entries);
    if (options.querySetDigest !== undefined && digest("querySetDigest", options.querySetDigest) !== expectedQueryDigest) fail("querySetDigest does not match the fixed selected-lane queries");
    const registration = ncbiRegistration(options.ncbiTool, options.ncbiEmail);
    const expectedRegistrationDigest = ncbiRegistrationDigest(registration);
    if (options.ncbiRegistrationDigest !== undefined && digest("ncbiRegistrationDigest", options.ncbiRegistrationDigest) !== expectedRegistrationDigest) fail("ncbiRegistrationDigest does not match the configured NCBI contact pair");
    const data: ConfigData = nativeObjectFreeze({
      specialtyLanes: lanes,
      perSpecialtyLimit,
      timeoutMs,
      responseByteLimit,
      totalResponseByteLimit,
      bundleByteLimit,
      transportId,
      transportVersion,
      transportConfigDigest,
      querySetDigest: expectedQueryDigest,
      registration,
      registrationDigest: expectedRegistrationDigest,
    });
    CONFIG_PRIVATE.set(this, data);
    const payload = configPayload(data);
    this.specialty_lanes = nativeObjectFreeze([...lanes]);
    this.per_specialty_limit = perSpecialtyLimit;
    this.timeout_ms = timeoutMs;
    this.request_limit = payload.request_limit as number;
    this.record_limit = payload.record_limit as number;
    this.response_byte_limit = responseByteLimit;
    this.total_response_byte_limit = totalResponseByteLimit;
    this.bundle_byte_limit = bundleByteLimit;
    this.transport_id = transportId;
    this.transport_version = transportVersion;
    this.transport_config_digest = transportConfigDigest;
    this.query_set_digest = expectedQueryDigest;
    this.ncbi_registration_configured = registration.length > 0;
    this.ncbi_registration_digest = expectedRegistrationDigest;
    this.config_digest = digestJsonSync(payload);
    boundedArtifact("reviewed PubMed retrieval config", this.toJSON());
    nativeObjectFreeze(this);
  }

  toJSON(): ReviewedPubMedRetrievalConfigJSON {
    const data = requireConfigData(this);
    const payload = configPayload(data);
    return { ...payload, config_digest: digestJsonSync(payload) } as ReviewedPubMedRetrievalConfigJSON;
  }

  static fromJSON(value: unknown, contact: { ncbiTool?: string; ncbiEmail?: string } = {}): ReviewedPubMedRetrievalConfig {
    if (!contact || typeof contact !== "object" || nativeArrayIsArray(contact)) fail("reviewed PubMed rehydration contact options are malformed");
    for (const key of nativeObjectKeys(contact)) if (key !== "ncbiTool" && key !== "ncbiEmail") fail("reviewed PubMed rehydration contact options are malformed");
    const raw = exactObject("reviewed PubMed retrieval config", snapshotJsonValue("reviewed PubMed retrieval config", value), CONFIG_JSON_KEYS);
    if (raw.schema !== REVIEWED_PUBMED_RETRIEVAL_CONFIG_SCHEMA) fail("reviewed PubMed retrieval config schema is unsupported");
    const result = new ReviewedPubMedRetrievalConfig({
      specialtyLanes: normalizeLanes(raw.specialty_lanes, "specialty_lanes"),
      perSpecialtyLimit: raw.per_specialty_limit as number,
      timeoutMs: raw.timeout_ms as number,
      responseByteLimit: raw.response_byte_limit as number,
      totalResponseByteLimit: raw.total_response_byte_limit as number,
      bundleByteLimit: raw.bundle_byte_limit as number,
      transportId: raw.transport_id as string,
      transportVersion: raw.transport_version as string,
      transportConfigDigest: raw.transport_config_digest as string,
      querySetDigest: raw.query_set_digest as string,
      ncbiTool: contact.ncbiTool,
      ncbiEmail: contact.ncbiEmail,
      ncbiRegistrationDigest: raw.ncbi_registration_digest as string,
    });
    if (canonicalJson(raw) !== canonicalJson(result.toJSON())) fail("reviewed PubMed retrieval config is not canonical or its contact pair was not re-supplied");
    return result;
  }
}

interface PlanData {
  readonly json: ReviewedPubMedRetrievalPlanJSON;
  readonly canonical: string;
}

export interface ReviewedPubMedRetrievalPlanJSON extends JsonObject {
  schema: typeof REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA;
  status: "ready_for_review";
  config_digest: string;
  specialty_lanes: ReviewedPubMedSpecialtyLane[];
  per_specialty_limit: number;
  request_limit: number;
  record_limit: number;
  response_byte_limit: number;
  total_response_byte_limit: number;
  bundle_byte_limit: number;
  transport_id: string;
  transport_version: string;
  transport_config_digest: string;
  query_set_digest: string;
  ncbi_registration_configured: boolean;
  ncbi_registration_digest: string;
  authority: typeof PUBMED_AUTHORITY;
  scope: JsonObject;
  execution: typeof PLAN_EXECUTION;
  retention: typeof CONFIG_RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  limitations: string[];
  plan_digest: string;
}

const PLAN_JSON_KEYS = ["schema", "status", "config_digest", "specialty_lanes", "per_specialty_limit", "request_limit", "record_limit", "response_byte_limit", "total_response_byte_limit", "bundle_byte_limit", "transport_id", "transport_version", "transport_config_digest", "query_set_digest", "ncbi_registration_configured", "ncbi_registration_digest", "authority", "scope", "execution", "retention", "secret_material", "limitations", "plan_digest"] as const;
const PLAN_PRIVATE = new WeakMap<ReviewedPubMedRetrievalPlan, PlanData>();

function planPayload(config: ConfigData, configDigest: string): Omit<ReviewedPubMedRetrievalPlanJSON, "plan_digest"> {
  return {
    schema: REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA,
    status: "ready_for_review",
    config_digest: configDigest,
    specialty_lanes: [...config.specialtyLanes],
    per_specialty_limit: config.perSpecialtyLimit,
    request_limit: config.specialtyLanes.length * 3,
    record_limit: config.specialtyLanes.length * config.perSpecialtyLimit,
    response_byte_limit: config.responseByteLimit,
    total_response_byte_limit: config.totalResponseByteLimit,
    bundle_byte_limit: config.bundleByteLimit,
    transport_id: config.transportId,
    transport_version: config.transportVersion,
    transport_config_digest: config.transportConfigDigest,
    query_set_digest: config.querySetDigest,
    ncbi_registration_configured: config.registration.length > 0,
    ncbi_registration_digest: config.registrationDigest,
    authority: PUBMED_AUTHORITY,
    scope: scope(),
    execution: PLAN_EXECUTION,
    retention: CONFIG_RETENTION,
    secret_material: SECRET_MATERIAL,
    limitations: [...LIMITATIONS],
  };
}

function validatePlanJSON(value: unknown): ReviewedPubMedRetrievalPlanJSON {
  const raw = exactObject("reviewed PubMed retrieval plan", snapshotJsonValue("reviewed PubMed retrieval plan", value), PLAN_JSON_KEYS);
  if (raw.schema !== REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA || raw.status !== "ready_for_review" || raw.authority !== PUBMED_AUTHORITY || raw.execution !== PLAN_EXECUTION || raw.retention !== CONFIG_RETENTION || raw.secret_material !== SECRET_MATERIAL) fail("reviewed PubMed retrieval plan markers are invalid");
  const lanes = normalizeLanes(raw.specialty_lanes, "plan specialty_lanes");
  const perLane = integer("plan per_specialty_limit", raw.per_specialty_limit, 1, MAX_PER_SPECIALTY_LIMIT);
  if (integer("plan request_limit", raw.request_limit, 3, MAX_REVIEWED_PUBMED_REQUESTS) !== lanes.length * 3 || integer("plan record_limit", raw.record_limit, 1, MAX_REVIEWED_PUBMED_RECORDS) !== lanes.length * perLane) fail("reviewed PubMed plan bounds do not match its lanes");
  integer("plan response_byte_limit", raw.response_byte_limit, 256, MAX_REVIEWED_PUBMED_RESPONSE_BYTES);
  integer("plan total_response_byte_limit", raw.total_response_byte_limit, raw.response_byte_limit as number, MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES);
  integer("plan bundle_byte_limit", raw.bundle_byte_limit, 1_024, MAX_REVIEWED_PUBMED_BUNDLE_BYTES);
  identifier("plan transport_id", raw.transport_id);
  identifier("plan transport_version", raw.transport_version);
  digest("plan config_digest", raw.config_digest);
  digest("plan transport_config_digest", raw.transport_config_digest);
  if (digest("plan query_set_digest", raw.query_set_digest) !== querySetDigest(queryEntries(lanes))) fail("reviewed PubMed plan query set does not match its lanes");
  if (typeof raw.ncbi_registration_configured !== "boolean") fail("plan ncbi_registration_configured must be boolean");
  const registrationDigest = digest("plan ncbi_registration_digest", raw.ncbi_registration_digest);
  if (raw.ncbi_registration_configured === false && registrationDigest !== UNCONFIGURED_NCBI_REGISTRATION_DIGEST) fail("unconfigured reviewed PubMed plan has an invalid registration digest");
  if (!nativeArrayIsArray(raw.limitations) || canonicalJson(raw.limitations) !== canonicalJson(LIMITATIONS)) fail("reviewed PubMed plan limitations are invalid");
  if (canonicalJson(raw.scope) !== canonicalJson(scope())) fail("reviewed PubMed plan scope is invalid");
  const payload = enumerableWithout(raw, "plan_digest");
  if (digest("plan plan_digest", raw.plan_digest) !== digestJsonSync(payload)) fail("reviewed PubMed plan digest is invalid");
  boundedArtifact("reviewed PubMed retrieval plan", raw);
  return cloneJson("reviewed PubMed retrieval plan", raw) as ReviewedPubMedRetrievalPlanJSON;
}

export class ReviewedPubMedRetrievalPlan {
  readonly schema: typeof REVIEWED_PUBMED_RETRIEVAL_PLAN_SCHEMA;
  readonly status: "ready_for_review";
  readonly config_digest: string;
  readonly specialty_lanes: readonly ReviewedPubMedSpecialtyLane[];
  readonly per_specialty_limit: number;
  readonly request_limit: number;
  readonly record_limit: number;
  readonly response_byte_limit: number;
  readonly total_response_byte_limit: number;
  readonly bundle_byte_limit: number;
  readonly transport_id: string;
  readonly transport_version: string;
  readonly transport_config_digest: string;
  readonly query_set_digest: string;
  readonly ncbi_registration_configured: boolean;
  readonly ncbi_registration_digest: string;
  readonly plan_digest: string;

  private constructor(json: ReviewedPubMedRetrievalPlanJSON) {
    const validated = validatePlanJSON(json);
    this.schema = validated.schema;
    this.status = validated.status;
    this.config_digest = validated.config_digest;
    this.specialty_lanes = nativeObjectFreeze([...validated.specialty_lanes]);
    this.per_specialty_limit = validated.per_specialty_limit;
    this.request_limit = validated.request_limit;
    this.record_limit = validated.record_limit;
    this.response_byte_limit = validated.response_byte_limit;
    this.total_response_byte_limit = validated.total_response_byte_limit;
    this.bundle_byte_limit = validated.bundle_byte_limit;
    this.transport_id = validated.transport_id;
    this.transport_version = validated.transport_version;
    this.transport_config_digest = validated.transport_config_digest;
    this.query_set_digest = validated.query_set_digest;
    this.ncbi_registration_configured = validated.ncbi_registration_configured;
    this.ncbi_registration_digest = validated.ncbi_registration_digest;
    this.plan_digest = validated.plan_digest;
    PLAN_PRIVATE.set(this, { json: validated, canonical: canonicalJson(validated) });
    nativeObjectFreeze(this);
  }

  static fromConfig(config: ReviewedPubMedRetrievalConfig): ReviewedPubMedRetrievalPlan {
    const data = requireConfigData(config);
    const configJson = configPayload(data);
    const configDigest = digestJsonSync(configJson);
    const payload = planPayload(data, configDigest);
    return new ReviewedPubMedRetrievalPlan({ ...payload, plan_digest: digestJsonSync(payload) } as ReviewedPubMedRetrievalPlanJSON);
  }

  static fromJSON(value: unknown): ReviewedPubMedRetrievalPlan {
    return new ReviewedPubMedRetrievalPlan(validatePlanJSON(value));
  }

  toJSON(): ReviewedPubMedRetrievalPlanJSON {
    const data = PLAN_PRIVATE.get(this);
    if (!data || nativeObjectGetPrototypeOf(this) !== ReviewedPubMedRetrievalPlan.prototype) fail("reviewed PubMed plan identity is invalid");
    return cloneJson("reviewed PubMed retrieval plan", data.json);
  }
}

function assertPlanLive(plan: ReviewedPubMedRetrievalPlan): PlanData {
  const data = PLAN_PRIVATE.get(plan);
  if (!data || nativeObjectGetPrototypeOf(plan) !== ReviewedPubMedRetrievalPlan.prototype) fail("PubMed execution requires an exact reviewed plan");
  const publicProjection: ReviewedPubMedRetrievalPlanJSON = {
    ...data.json,
    schema: plan.schema,
    status: plan.status,
    config_digest: plan.config_digest,
    specialty_lanes: [...plan.specialty_lanes],
    per_specialty_limit: plan.per_specialty_limit,
    request_limit: plan.request_limit,
    record_limit: plan.record_limit,
    response_byte_limit: plan.response_byte_limit,
    total_response_byte_limit: plan.total_response_byte_limit,
    bundle_byte_limit: plan.bundle_byte_limit,
    transport_id: plan.transport_id,
    transport_version: plan.transport_version,
    transport_config_digest: plan.transport_config_digest,
    query_set_digest: plan.query_set_digest,
    ncbi_registration_configured: plan.ncbi_registration_configured,
    ncbi_registration_digest: plan.ncbi_registration_digest,
    plan_digest: plan.plan_digest,
  };
  if (canonicalJson(publicProjection) !== data.canonical || !hasExactEnumerableKeys(plan, ["bundle_byte_limit", "config_digest", "ncbi_registration_configured", "ncbi_registration_digest", "per_specialty_limit", "plan_digest", "query_set_digest", "record_limit", "request_limit", "response_byte_limit", "schema", "specialty_lanes", "status", "total_response_byte_limit", "transport_config_digest", "transport_id", "transport_version"])) fail("reviewed PubMed plan changed after review");
  return data;
}

export interface ReviewedPubMedSourceReceiptJSON extends JsonObject {
  schema: typeof REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA;
  specialty_lane: ReviewedPubMedSpecialtyLane;
  source_id: string;
  content_digest: string;
  record_count: number;
}

const SOURCE_RECEIPT_KEYS = ["schema", "specialty_lane", "source_id", "content_digest", "record_count"] as const;

export class ReviewedPubMedSourceReceipt {
  readonly schema = REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA;
  readonly specialty_lane: ReviewedPubMedSpecialtyLane;
  readonly source_id: string;
  readonly content_digest: string;
  readonly record_count: number;

  constructor(input: { specialtyLane: ReviewedPubMedSpecialtyLane; sourceId: string; contentDigest: string; recordCount: number }) {
    const lane = normalizeLanes([input.specialtyLane], "source receipt specialtyLane")[0]!;
    if (input.sourceId !== `pubmed_${lane}`) fail("source receipt ID does not match its specialty lane");
    this.specialty_lane = lane;
    this.source_id = input.sourceId;
    this.content_digest = digest("source receipt contentDigest", input.contentDigest);
    this.record_count = integer("source receipt recordCount", input.recordCount, 1, MAX_PER_SPECIALTY_LIMIT);
    nativeObjectFreeze(this);
  }

  toJSON(): ReviewedPubMedSourceReceiptJSON {
    return {
      schema: this.schema,
      specialty_lane: this.specialty_lane,
      source_id: this.source_id,
      content_digest: this.content_digest,
      record_count: this.record_count,
    };
  }

  static fromJSON(value: unknown): ReviewedPubMedSourceReceipt {
    const raw = exactObject("reviewed PubMed source receipt", snapshotJsonValue("reviewed PubMed source receipt", value), SOURCE_RECEIPT_KEYS);
    if (raw.schema !== REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA) fail("reviewed PubMed source receipt schema is unsupported");
    const receipt = new ReviewedPubMedSourceReceipt({
      specialtyLane: raw.specialty_lane as ReviewedPubMedSpecialtyLane,
      sourceId: raw.source_id as string,
      contentDigest: raw.content_digest as string,
      recordCount: raw.record_count as number,
    });
    if (canonicalJson(raw) !== canonicalJson(receipt.toJSON())) fail("reviewed PubMed source receipt is not canonical");
    return receipt;
  }
}

export interface ReviewedPubMedRetrievalReceiptJSON extends JsonObject {
  schema: typeof REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA;
  plan_digest: string;
  config_digest: string;
  specialty_lanes: ReviewedPubMedSpecialtyLane[];
  transport_id: string;
  transport_version: string;
  transport_config_digest: string;
  query_set_digest: string;
  ncbi_registration_configured: boolean;
  ncbi_registration_digest: string;
  generated_at: string;
  bundle_schema: typeof PUBLIC_LITERATURE_SCHEMA_VERSION;
  bundle_digest: string;
  source_set_digest: string;
  sources: ReviewedPubMedSourceReceiptJSON[];
  source_count: number;
  record_count: number;
  abstract_count: number;
  request_count: number;
  response_bytes: number;
  synthetic_data: false;
  human_review_required: true;
  retention: typeof RECEIPT_RETENTION;
  secret_material: typeof SECRET_MATERIAL;
  limitations: string[];
  receipt_digest: string;
}

interface ReceiptValues {
  planDigest: string;
  configDigest: string;
  specialtyLanes: readonly ReviewedPubMedSpecialtyLane[];
  transportId: string;
  transportVersion: string;
  transportConfigDigest: string;
  querySetDigest: string;
  ncbiRegistrationConfigured: boolean;
  ncbiRegistrationDigest: string;
  generatedAt: string;
  bundleDigest: string;
  sourceSetDigest: string;
  sources: readonly ReviewedPubMedSourceReceipt[];
  recordCount: number;
  abstractCount: number;
  requestCount: number;
  responseBytes: number;
}

const RECEIPT_KEYS = ["schema", "plan_digest", "config_digest", "specialty_lanes", "transport_id", "transport_version", "transport_config_digest", "query_set_digest", "ncbi_registration_configured", "ncbi_registration_digest", "generated_at", "bundle_schema", "bundle_digest", "source_set_digest", "sources", "source_count", "record_count", "abstract_count", "request_count", "response_bytes", "synthetic_data", "human_review_required", "retention", "secret_material", "limitations", "receipt_digest"] as const;
const RECEIPT_PRIVATE = new WeakMap<ReviewedPubMedRetrievalReceipt, ReviewedPubMedRetrievalReceiptJSON>();

function exactReceiptJSON(receipt: ReviewedPubMedRetrievalReceipt): ReviewedPubMedRetrievalReceiptJSON {
  const json = RECEIPT_PRIVATE.get(receipt);
  if (!json || nativeObjectGetPrototypeOf(receipt) !== ReviewedPubMedRetrievalReceipt.prototype) fail("reviewed PubMed receipt identity is invalid");
  return cloneJson("reviewed PubMed retrieval receipt", json);
}

function receiptPayload(values: ReceiptValues): Omit<ReviewedPubMedRetrievalReceiptJSON, "receipt_digest"> {
  const sources: ReviewedPubMedSourceReceiptJSON[] = [];
  for (let index = 0; index < values.sources.length; index += 1) sources[index] = values.sources[index]!.toJSON();
  return {
    schema: REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA,
    plan_digest: values.planDigest,
    config_digest: values.configDigest,
    specialty_lanes: [...values.specialtyLanes],
    transport_id: values.transportId,
    transport_version: values.transportVersion,
    transport_config_digest: values.transportConfigDigest,
    query_set_digest: values.querySetDigest,
    ncbi_registration_configured: values.ncbiRegistrationConfigured,
    ncbi_registration_digest: values.ncbiRegistrationDigest,
    generated_at: values.generatedAt,
    bundle_schema: PUBLIC_LITERATURE_SCHEMA_VERSION,
    bundle_digest: values.bundleDigest,
    source_set_digest: values.sourceSetDigest,
    sources,
    source_count: values.sources.length,
    record_count: values.recordCount,
    abstract_count: values.abstractCount,
    request_count: values.requestCount,
    response_bytes: values.responseBytes,
    synthetic_data: false,
    human_review_required: true,
    retention: RECEIPT_RETENTION,
    secret_material: SECRET_MATERIAL,
    limitations: [...LIMITATIONS],
  };
}

function validateReceiptJSON(value: unknown): ReviewedPubMedRetrievalReceiptJSON {
  const raw = exactObject("reviewed PubMed retrieval receipt", snapshotJsonValue("reviewed PubMed retrieval receipt", value), RECEIPT_KEYS);
  if (raw.schema !== REVIEWED_PUBMED_RETRIEVAL_RECEIPT_SCHEMA || raw.bundle_schema !== PUBLIC_LITERATURE_SCHEMA_VERSION || raw.synthetic_data !== false || raw.human_review_required !== true || raw.retention !== RECEIPT_RETENTION || raw.secret_material !== SECRET_MATERIAL) fail("reviewed PubMed retrieval receipt markers are invalid");
  for (const [name, value] of [["plan_digest", raw.plan_digest], ["config_digest", raw.config_digest], ["transport_config_digest", raw.transport_config_digest], ["query_set_digest", raw.query_set_digest], ["ncbi_registration_digest", raw.ncbi_registration_digest], ["bundle_digest", raw.bundle_digest], ["source_set_digest", raw.source_set_digest], ["receipt_digest", raw.receipt_digest]] as const) digest(`retrieval receipt ${name}`, value);
  const lanes = normalizeLanes(raw.specialty_lanes, "retrieval receipt specialty_lanes");
  identifier("retrieval receipt transport_id", raw.transport_id);
  identifier("retrieval receipt transport_version", raw.transport_version);
  if (raw.query_set_digest !== querySetDigest(queryEntries(lanes))) fail("retrieval receipt query set does not match its lanes");
  if (typeof raw.ncbi_registration_configured !== "boolean") fail("retrieval receipt registration configured flag must be boolean");
  if (raw.ncbi_registration_configured === false && raw.ncbi_registration_digest !== UNCONFIGURED_NCBI_REGISTRATION_DIGEST) fail("unconfigured retrieval receipt has an invalid registration digest");
  timestamp("retrieval receipt generated_at", raw.generated_at);
  if (!nativeArrayIsArray(raw.sources)) fail("reviewed PubMed retrieval receipt sources must be an array");
  const sources: ReviewedPubMedSourceReceipt[] = [];
  for (let index = 0; index < raw.sources.length; index += 1) sources[index] = ReviewedPubMedSourceReceipt.fromJSON(raw.sources[index]);
  if (sources.length !== lanes.length) fail("retrieval receipt sources do not match its lanes");
  let sourceRecords = 0;
  const sourceJson: ReviewedPubMedSourceReceiptJSON[] = [];
  for (let index = 0; index < sources.length; index += 1) {
    const source = sources[index]!;
    if (source.specialty_lane !== lanes[index]) fail("retrieval receipt sources do not match its lanes");
    sourceRecords += source.record_count;
    sourceJson[index] = source.toJSON();
  }
  const sourceCount = integer("retrieval receipt source_count", raw.source_count, 1, MAX_PUBMED_LANES);
  const recordCount = integer("retrieval receipt record_count", raw.record_count, 1, MAX_REVIEWED_PUBMED_RECORDS);
  integer("retrieval receipt abstract_count", raw.abstract_count, 0, recordCount);
  const requestCount = integer("retrieval receipt request_count", raw.request_count, 3, MAX_REVIEWED_PUBMED_REQUESTS);
  integer("retrieval receipt response_bytes", raw.response_bytes, 1, MAX_REVIEWED_PUBMED_TOTAL_RESPONSE_BYTES);
  if (sourceCount !== lanes.length || requestCount !== lanes.length * 3 || recordCount !== sourceRecords) fail("retrieval receipt counts do not match its sources and lanes");
  if (raw.source_set_digest !== digestJsonSync(sourceJson)) fail("retrieval receipt source-set digest is invalid");
  if (!nativeArrayIsArray(raw.limitations) || canonicalJson(raw.limitations) !== canonicalJson(LIMITATIONS)) fail("retrieval receipt limitations are invalid");
  const payload = enumerableWithout(raw, "receipt_digest");
  if (raw.receipt_digest !== digestJsonSync(payload)) fail("retrieval receipt digest is invalid");
  boundedArtifact("reviewed PubMed retrieval receipt", raw);
  return cloneJson("reviewed PubMed retrieval receipt", raw) as ReviewedPubMedRetrievalReceiptJSON;
}

export class ReviewedPubMedRetrievalReceipt {
  readonly plan_digest: string;
  readonly config_digest: string;
  readonly specialty_lanes: readonly ReviewedPubMedSpecialtyLane[];
  readonly transport_id: string;
  readonly transport_version: string;
  readonly transport_config_digest: string;
  readonly query_set_digest: string;
  readonly ncbi_registration_configured: boolean;
  readonly ncbi_registration_digest: string;
  readonly generated_at: string;
  readonly bundle_digest: string;
  readonly source_set_digest: string;
  readonly sources: readonly ReviewedPubMedSourceReceipt[];
  readonly source_count: number;
  readonly record_count: number;
  readonly abstract_count: number;
  readonly request_count: number;
  readonly response_bytes: number;
  readonly receipt_digest: string;

  constructor(values: ReceiptValues) {
    const payload = receiptPayload(values);
    const json = validateReceiptJSON({ ...payload, receipt_digest: digestJsonSync(payload) });
    this.plan_digest = json.plan_digest;
    this.config_digest = json.config_digest;
    this.specialty_lanes = nativeObjectFreeze([...json.specialty_lanes]);
    this.transport_id = json.transport_id;
    this.transport_version = json.transport_version;
    this.transport_config_digest = json.transport_config_digest;
    this.query_set_digest = json.query_set_digest;
    this.ncbi_registration_configured = json.ncbi_registration_configured;
    this.ncbi_registration_digest = json.ncbi_registration_digest;
    this.generated_at = json.generated_at;
    this.bundle_digest = json.bundle_digest;
    this.source_set_digest = json.source_set_digest;
    const sources: ReviewedPubMedSourceReceipt[] = [];
    for (let index = 0; index < json.sources.length; index += 1) sources[index] = ReviewedPubMedSourceReceipt.fromJSON(json.sources[index]);
    this.sources = nativeObjectFreeze(sources);
    this.source_count = json.source_count;
    this.record_count = json.record_count;
    this.abstract_count = json.abstract_count;
    this.request_count = json.request_count;
    this.response_bytes = json.response_bytes;
    this.receipt_digest = json.receipt_digest;
    RECEIPT_PRIVATE.set(this, json);
    nativeObjectFreeze(this);
  }

  static fromJSON(value: unknown): ReviewedPubMedRetrievalReceipt {
    const json = validateReceiptJSON(value);
    const receipt = nativeObjectCreate(ReviewedPubMedRetrievalReceipt.prototype) as ReviewedPubMedRetrievalReceipt;
    const sources: ReviewedPubMedSourceReceipt[] = [];
    for (let index = 0; index < json.sources.length; index += 1) sources[index] = ReviewedPubMedSourceReceipt.fromJSON(json.sources[index]);
    nativeObjectAssign(receipt, {
      plan_digest: json.plan_digest,
      config_digest: json.config_digest,
      specialty_lanes: nativeObjectFreeze([...json.specialty_lanes]),
      transport_id: json.transport_id,
      transport_version: json.transport_version,
      transport_config_digest: json.transport_config_digest,
      query_set_digest: json.query_set_digest,
      ncbi_registration_configured: json.ncbi_registration_configured,
      ncbi_registration_digest: json.ncbi_registration_digest,
      generated_at: json.generated_at,
      bundle_digest: json.bundle_digest,
      source_set_digest: json.source_set_digest,
      sources: nativeObjectFreeze(sources),
      source_count: json.source_count,
      record_count: json.record_count,
      abstract_count: json.abstract_count,
      request_count: json.request_count,
      response_bytes: json.response_bytes,
      receipt_digest: json.receipt_digest,
    });
    RECEIPT_PRIVATE.set(receipt, json);
    nativeObjectFreeze(receipt);
    return receipt;
  }

  toJSON(): ReviewedPubMedRetrievalReceiptJSON {
    return exactReceiptJSON(this);
  }
}

export interface PublicLiteratureSource extends JsonObject {
  source_id: string;
  authority: typeof PUBMED_AUTHORITY;
  uri: string;
  retrieved_at: string;
  content_sha256: string;
  record_count: number;
}

export interface PublicLiteratureRecord extends JsonObject {
  source_id: string;
  specialty: ReviewedPubMedSpecialtyLane;
  pmid: string;
  title: string;
  journal: string;
  publication_date: string | null;
  doi: string | null;
  abstract_text: string | null;
  abstract_truncated: boolean;
  publication_types: string[];
  mesh_terms: string[];
}

export interface PublicLiteratureBundle extends JsonObject {
  schema_version: typeof PUBLIC_LITERATURE_SCHEMA_VERSION;
  generated_at: string;
  synthetic_data: false;
  sources: PublicLiteratureSource[];
  records: PublicLiteratureRecord[];
}

function recordProjection(record: PublicLiteratureRecord): JsonObject {
  return {
    source_id: record.source_id,
    specialty: record.specialty,
    pmid: record.pmid,
    title: record.title,
    journal: record.journal,
    publication_date: record.publication_date,
    ...(record.doi === null ? {} : { doi: record.doi }),
    ...(record.abstract_text === null ? {} : { abstract_text: record.abstract_text }),
    ...(record.abstract_truncated ? { abstract_truncated: true } : {}),
    ...(record.publication_types.length > 0 ? { publication_types: [...record.publication_types] } : {}),
    ...(record.mesh_terms.length > 0 ? { mesh_terms: [...record.mesh_terms] } : {}),
  };
}

function orderedJsonDigest(value: unknown): string {
  const encoded = nativeJsonStringify(value);
  if (encoded === undefined) fail("value cannot be represented as JSON");
  return digestBytesSync(nativeEncodeUtf8(encoded));
}

function sourceHash(records: readonly PublicLiteratureRecord[], sourceId: string): string {
  const selected: PublicLiteratureRecord[] = [];
  for (let index = 0; index < records.length; index += 1) if (records[index]!.source_id === sourceId) selected[selected.length] = records[index]!;
  for (let index = 1; index < selected.length; index += 1) {
    const current = selected[index]!;
    let cursor = index - 1;
    while (cursor >= 0) {
      const prior = selected[cursor]!;
      if (prior.specialty < current.specialty || (prior.specialty === current.specialty && prior.pmid <= current.pmid)) break;
      selected[cursor + 1] = prior;
      cursor -= 1;
    }
    selected[cursor + 1] = current;
  }
  const projections: JsonObject[] = [];
  for (let index = 0; index < selected.length; index += 1) projections[index] = recordProjection(selected[index]!);
  return orderedJsonDigest({ records: projections });
}

export function reviewedPubMedBundleDigest(bundle: PublicLiteratureBundle): string {
  const sources: JsonObject[] = [];
  for (let index = 0; index < bundle.sources.length; index += 1) {
    const source = bundle.sources[index]!;
    sources[index] = {
      source_id: source.source_id,
      authority: source.authority,
      uri: source.uri,
      retrieved_at: source.retrieved_at,
      content_sha256: source.content_sha256,
      record_count: source.record_count,
    };
  }
  const records: JsonObject[] = [];
  for (let index = 0; index < bundle.records.length; index += 1) records[index] = recordProjection(bundle.records[index]!);
  return orderedJsonDigest({
    schema_version: bundle.schema_version,
    generated_at: bundle.generated_at,
    synthetic_data: bundle.synthetic_data,
    sources,
    records,
  });
}

function safeSourceText(name: string, value: unknown, required = true, maximum: number | null = MAX_REVIEWED_PUBMED_TEXT_BYTES): string | null {
  if (value === null || value === undefined) {
    if (required) fail(`${name} is missing`);
    return null;
  }
  const normalized = String(value).replace(/\s+/g, " ").trim();
  if (!normalized) {
    if (required) fail(`${name} is empty`);
    return null;
  }
  if ((maximum !== null && bytes(normalized) > maximum) || CONTROL_RE.test(normalized)) fail(`${name} exceeds the text safety bound`);
  const lowered = normalized.toLowerCase();
  if (SYNTHETIC_MARKERS.some((marker) => lowered.includes(marker))) fail(`synthetic marker found in ${name}`);
  return normalized;
}

function normalizePublicationDate(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const text = String(value).replace(/\s+/g, " ").trim();
  if (!text) return null;
  if (/^\d{4}-\d{2}-\d{2}$/.test(text)) {
    const date = new Date(`${text}T00:00:00Z`);
    return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === text ? text : null;
  }
  const match = /^(\d{4})\s+([A-Za-z]{3})\s+(\d{1,2})$/.exec(text);
  if (!match) return null;
  const months: Record<string, number> = { Jan: 1, Feb: 2, Mar: 3, Apr: 4, May: 5, Jun: 6, Jul: 7, Aug: 8, Sep: 9, Oct: 10, Nov: 11, Dec: 12 };
  const month = months[`${match[2]![0]!.toUpperCase()}${match[2]!.slice(1).toLowerCase()}`];
  if (month === undefined) return null;
  const candidate = `${match[1]}-${String(month).padStart(2, "0")}-${String(Number(match[3])).padStart(2, "0")}`;
  const date = new Date(`${candidate}T00:00:00Z`);
  return Number.isFinite(date.getTime()) && date.toISOString().slice(0, 10) === candidate ? candidate : null;
}

function boundedAbstract(value: string | null): readonly [string | null, boolean] {
  if (value === null) return [null, false];
  const encoded = nativeEncodeUtf8(value);
  if (encoded.byteLength <= MAX_REVIEWED_PUBMED_ABSTRACT_BYTES) return [value, false];
  let end = MAX_REVIEWED_PUBMED_ABSTRACT_BYTES;
  while (end > 0 && (encoded[end]! & 0xc0) === 0x80) end -= 1;
  return [nativeDecodeUtf8Lossy(nativeUint8ArraySlice(encoded, 0, end)), true];
}

function validateJsonTree(value: unknown, name: string, byteLimit: number): void {
  let nodes = 0;
  let scalarBytes = 0;
  const stack: Array<readonly [unknown, number]> = [[value, 0]];
  while (stack.length > 0) {
    const [item, depth] = stack.pop()!;
    nodes += 1;
    if (nodes > MAX_REVIEWED_PUBMED_RESPONSE_NODES) fail(`${name} contains too many nodes`);
    if (depth > MAX_REVIEWED_PUBMED_RESPONSE_DEPTH) fail(`${name} is too deeply nested`);
    if (item === null || typeof item === "boolean") continue;
    if (typeof item === "string") {
      scalarBytes += bytes(item);
      if (scalarBytes > byteLimit) fail(`${name} exceeds its scalar byte bound`);
      continue;
    }
    if (typeof item === "number") {
      if (!Number.isFinite(item) || (!Number.isSafeInteger(item) && Number.isInteger(item))) fail(`${name} contains an unsupported number`);
      continue;
    }
    if (nativeArrayIsArray(item)) {
      if (item.length > MAX_REVIEWED_PUBMED_RESPONSE_NODES) fail(`${name} contains an oversized array`);
      for (const child of item) stack.push([child, depth + 1]);
      continue;
    }
    if (isObject(item)) {
      const entries = nativeObjectEntries(item);
      if (entries.length > MAX_REVIEWED_PUBMED_RESPONSE_NODES) fail(`${name} contains an oversized object`);
      for (const [key, child] of entries) {
        if (!key || key.includes("\u0000") || bytes(key) > MAX_REVIEWED_PUBMED_TEXT_BYTES || child === undefined) fail(`${name} contains an invalid object field`);
        scalarBytes += bytes(key);
        if (scalarBytes > byteLimit) fail(`${name} exceeds its scalar byte bound`);
        stack.push([child, depth + 1]);
      }
      continue;
    }
    fail(`${name} contains an unsupported value type`);
  }
}

interface XmlNode {
  readonly name: string;
  readonly attributes: Readonly<Record<string, string>>;
  readonly children: XmlNode[];
  readonly content: Array<string | XmlNode>;
}

const PUBMED_DOCTYPE_RE = /<!DOCTYPE\s+PubmedArticleSet\s+PUBLIC\s+"-\/\/NLM\/\/DTD PubMedArticle,\s+[0-9A-Za-z ]{1,48}\/\/EN"\s+"https:\/\/dtd\.nlm\.nih\.gov\/ncbi\/pubmed\/out\/pubmed_[0-9]{6}\.dtd"\s*>/g;
const PUBMED_XML_PREFIX_RE = /^(?:\uFEFF)?[ \t\r\n]*(?:<\?xml\s+version=["']1\.0["'](?:\s+encoding=["']utf-8["'])?(?:\s+standalone=["'](?:yes|no)["'])?\s*\?>[ \t\r\n]*)?$/i;
const XML_NAME_RE = /^[A-Za-z_][A-Za-z0-9_.:-]*$/;

function decodeXmlEntities(value: string): string {
  const decoded = value.replace(/&([^;]{1,32});/g, (_whole, entity: string) => {
    if (entity === "amp") return "&";
    if (entity === "lt") return "<";
    if (entity === "gt") return ">";
    if (entity === "quot") return '"';
    if (entity === "apos") return "'";
    const numeric = entity.startsWith("#x") ? Number.parseInt(entity.slice(2), 16) : entity.startsWith("#") ? Number.parseInt(entity.slice(1), 10) : Number.NaN;
    if (!Number.isInteger(numeric) || numeric < 1 || numeric > 0x10ffff || (numeric >= 0xd800 && numeric <= 0xdfff)) fail("PubMed XML contains an unsupported entity reference");
    return String.fromCodePoint(numeric);
  });
  if (value.includes("&") && value.replace(/&([^;]{1,32});/g, "").includes("&")) fail("PubMed XML contains a malformed entity reference");
  return decoded;
}

function parseAttributes(value: string): Readonly<Record<string, string>> {
  const attributes: Record<string, string> = {};
  let rest = value;
  let count = 0;
  const matcher = /^\s+([A-Za-z_][A-Za-z0-9_.:-]*)\s*=\s*("[^"]*"|'[^']*')/;
  while (rest.length > 0) {
    if (/^\s*$/.test(rest)) break;
    const match = matcher.exec(rest);
    if (!match) fail("PubMed XML contains malformed attributes");
    const name = match[1]!;
    if (bytes(name) > 512) fail("PubMed XML contains an oversized attribute name");
    if (Object.prototype.hasOwnProperty.call(attributes, name)) fail("PubMed XML contains duplicate attributes");
    count += 1;
    if (count > 64) fail("PubMed XML node contains too many attributes");
    const attributeValue = decodeXmlEntities(match[2]!.slice(1, -1));
    if (bytes(attributeValue) > MAX_REVIEWED_PUBMED_TEXT_BYTES || attributeValue.includes("\u0000")) fail("PubMed XML contains an oversized attribute value");
    attributes[name] = attributeValue;
    rest = rest.slice(match[0].length);
  }
  return nativeObjectFreeze(attributes);
}

function parsePubMedXml(raw: Uint8Array): XmlNode {
  let xml: string;
  try {
    xml = nativeDecodeUtf8(raw);
  } catch {
    fail("PubMed efetch response is not valid UTF-8");
  }
  if (/<!ENTITY/i.test(xml)) fail("PubMed efetch response contains a forbidden document declaration");
  const doctypeOccurrences = xml.match(/<!DOCTYPE/gi) ?? [];
  if (doctypeOccurrences.length > 0) {
    PUBMED_DOCTYPE_RE.lastIndex = 0;
    const matches = [...xml.matchAll(PUBMED_DOCTYPE_RE)];
    if (doctypeOccurrences.length !== 1 || matches.length !== 1) fail("PubMed efetch response contains a forbidden document declaration");
    const match = matches[0]!;
    const prefix = xml.slice(0, match.index);
    if (bytes(prefix) > 256 || !PUBMED_XML_PREFIX_RE.test(prefix)) fail("PubMed efetch response contains a misplaced document declaration");
    xml = `${xml.slice(0, match.index)}${xml.slice(match.index! + match[0].length)}`;
  }
  if (/<!DOCTYPE/i.test(xml)) fail("PubMed efetch response contains a forbidden document declaration");
  const documentNode: XmlNode = { name: "#document", attributes: nativeObjectFreeze({}), children: [], content: [] };
  const stack: XmlNode[] = [documentNode];
  let offset = 0;
  let nodes = 0;
  while (offset < xml.length) {
    if (xml.startsWith("<!--", offset)) {
      const end = xml.indexOf("-->", offset + 4);
      if (end < 0) fail("PubMed efetch response contains an unterminated comment");
      offset = end + 3;
      nodes += 1;
    } else if (xml.startsWith("<![CDATA[", offset)) {
      const end = xml.indexOf("]]>", offset + 9);
      if (end < 0) fail("PubMed efetch response contains unterminated CDATA");
      stack.at(-1)!.content.push(xml.slice(offset + 9, end));
      offset = end + 3;
      nodes += 1;
    } else if (xml.startsWith("<?", offset)) {
      const end = xml.indexOf("?>", offset + 2);
      if (end < 0) fail("PubMed efetch response contains an unterminated processing instruction");
      const instruction = xml.slice(offset, end + 2);
      if (!/^<\?xml\s+version=["']1\.0["'](?:\s+encoding=["']utf-8["'])?(?:\s+standalone=["'](?:yes|no)["'])?\s*\?>$/i.test(instruction) || !/^(?:\uFEFF)?[ \t\r\n]*$/.test(xml.slice(0, offset))) fail("PubMed efetch response contains an unsupported processing instruction");
      offset = end + 2;
      nodes += 1;
    } else if (xml[offset] === "<") {
      const end = xml.indexOf(">", offset + 1);
      if (end < 0) fail("PubMed efetch response contains an unterminated tag");
      const token = xml.slice(offset + 1, end);
      if (token.startsWith("!")) fail("PubMed efetch response contains an unsupported declaration");
      if (token.startsWith("/")) {
        const name = token.slice(1).trim();
        if (!XML_NAME_RE.test(name) || stack.length <= 1 || stack.at(-1)!.name !== name) fail("PubMed efetch response has mismatched XML tags");
        stack.pop();
      } else {
        const selfClosing = /\/\s*$/.test(token);
        const body = selfClosing ? token.replace(/\/\s*$/, "") : token;
        const nameMatch = /^([A-Za-z_][A-Za-z0-9_.:-]*)/.exec(body);
        if (!nameMatch) fail("PubMed efetch response contains a malformed XML tag");
        const name = nameMatch[1]!;
        const node: XmlNode = { name, attributes: parseAttributes(body.slice(name.length)), children: [], content: [] };
        stack.at(-1)!.children.push(node);
        stack.at(-1)!.content.push(node);
        if (!selfClosing) stack.push(node);
        if (stack.length - 1 > MAX_REVIEWED_PUBMED_RESPONSE_DEPTH) fail("PubMed efetch response XML is too deeply nested");
      }
      offset = end + 1;
      nodes += 1;
    } else {
      const end = xml.indexOf("<", offset);
      const next = end < 0 ? xml.length : end;
      const value = xml.slice(offset, next);
      if (value.length > 0) {
        stack.at(-1)!.content.push(decodeXmlEntities(value));
        if (value.trim()) nodes += 1;
      }
      offset = next;
    }
    if (nodes > MAX_REVIEWED_PUBMED_RESPONSE_NODES) fail("PubMed efetch response contains too many XML nodes");
  }
  if (stack.length !== 1 || documentNode.children.length !== 1 || documentNode.content.some((item) => typeof item === "string" && item.trim())) fail("PubMed efetch response is malformed XML");
  return documentNode.children[0]!;
}

function directChild(node: XmlNode | undefined, name: string): XmlNode | undefined {
  return node?.children.find((child) => child.name === name);
}

function pathNode(node: XmlNode | undefined, path: readonly string[]): XmlNode | undefined {
  let current = node;
  for (const name of path) current = directChild(current, name);
  return current;
}

function pathNodes(node: XmlNode | undefined, path: readonly string[]): XmlNode[] {
  if (!node || path.length === 0) return node ? [node] : [];
  const [head, ...tail] = path;
  return node.children.filter((child) => child.name === head).flatMap((child) => pathNodes(child, tail));
}

function descendants(node: XmlNode, name: string): XmlNode[] {
  const result: XmlNode[] = [];
  const stack = [node];
  while (stack.length > 0) {
    const current = stack.pop()!;
    if (current.name === name) result.push(current);
    stack.push(...current.children.slice().reverse());
  }
  return result;
}

function xmlText(node: XmlNode | undefined): string {
  if (!node) return "";
  const parts: string[] = [];
  const visit = (current: XmlNode): void => {
    for (const item of current.content) {
      if (typeof item === "string") parts.push(item);
      else visit(item);
    }
  };
  visit(node);
  return parts.join("").replace(/\s+/g, " ").trim();
}

interface XmlArticleProjection {
  abstractText: string | null;
  abstractTruncated: boolean;
  publicationTypes: string[];
  meshTerms: string[];
}

function projectXmlArticles(root: XmlNode): Map<string, XmlArticleProjection> {
  if (root.name !== "PubmedArticleSet") fail("PubMed efetch response has an unexpected root element");
  const result = new Map<string, XmlArticleProjection>();
  for (const article of descendants(root, "PubmedArticle")) {
    const pmid = xmlText(pathNode(article, ["MedlineCitation", "PMID"]));
    if (!/^\d{1,32}$/.test(pmid)) continue;
    const abstractParts = pathNodes(article, ["MedlineCitation", "Article", "Abstract", "AbstractText"]).map((node) => {
      const value = xmlText(node);
      const label = node.attributes.Label?.trim();
      return value && label ? `${label}: ${value}` : value;
    }).filter(Boolean);
    // The raw response already has a reviewed byte ceiling. Validate the full abstract for
    // controls/synthetic markers, then apply the narrower durable 12 KB abstract projection.
    const [abstractText, abstractTruncated] = boundedAbstract(abstractParts.length > 0 ? safeSourceText(`PubMed ${pmid} abstract`, abstractParts.join(" "), true, null) : null);
    const publicationTypes = pathNodes(article, ["MedlineCitation", "Article", "PublicationTypeList", "PublicationType"]).map((node) => safeSourceText(`PubMed ${pmid} publication type`, xmlText(node))).filter((value): value is string => value !== null);
    const meshTerms = pathNodes(article, ["MedlineCitation", "MeshHeadingList", "MeshHeading", "DescriptorName"]).map((node) => safeSourceText(`PubMed ${pmid} mesh term`, xmlText(node))).filter((value): value is string => value !== null);
    if (publicationTypes.length > MAX_REVIEWED_PUBMED_TAGS || meshTerms.length > MAX_REVIEWED_PUBMED_TAGS) fail(`PubMed ${pmid} tags exceed their safety bound`);
    result.set(pmid, { abstractText, abstractTruncated, publicationTypes, meshTerms });
  }
  return result;
}

type ReviewedPubMedRawResponse = Uint8Array | ArrayBuffer | string;
export type ReviewedPubMedFetch = (url: string) => ReviewedPubMedRawResponse | Promise<ReviewedPubMedRawResponse>;

function responseBytes(value: unknown, endpoint: Endpoint, byteLimit: number): readonly [Uint8Array, JsonObject | XmlNode] {
  let encoded: Uint8Array;
  if (typeof value === "string") encoded = nativeEncodeUtf8(value);
  else if (value instanceof NativeUint8Array) {
    if (value.byteLength > byteLimit) fail("PubMed response exceeds the reviewed per-response byte limit");
    encoded = nativeUint8ArraySlice(value);
  } else if (value instanceof NativeArrayBuffer) {
    if (value.byteLength > byteLimit) fail("PubMed response exceeds the reviewed per-response byte limit");
    encoded = new NativeUint8Array(nativeArrayBufferSlice(value, 0));
  }
  else fail("PubMed transport returned an unsupported response type");
  if (encoded.byteLength > byteLimit) fail("PubMed response exceeds the reviewed per-response byte limit");
  if (endpoint === "efetch.fcgi") return [encoded, parsePubMedXml(encoded)];
  let decoded: unknown;
  try {
    decoded = nativeJsonParse(nativeDecodeUtf8(encoded));
  } catch {
    fail(`PubMed ${endpoint} response is malformed JSON`);
  }
  validateJsonTree(decoded, `PubMed ${endpoint} response`, byteLimit);
  if (!isObject(decoded)) fail(`PubMed ${endpoint} response must be an object`);
  return [encoded, decoded as JsonObject];
}

const NativeURL = globalThis.URL;
const NativeAbortController = globalThis.AbortController;
const nativeSetTimeout = globalThis.setTimeout.bind(globalThis);
const nativeClearTimeout = globalThis.clearTimeout.bind(globalThis);
const nativeMonotonicNow = globalThis.performance && typeof globalThis.performance.now === "function" ? globalThis.performance.now.bind(globalThis.performance) : Date.now.bind(Date);
let builtinRateTail: Promise<void> = Promise.resolve();
let builtinLastDispatchAt: number | null = null;

function buildRequestUrl(endpoint: Endpoint, parameters: readonly (readonly [string, string])[]): string {
  const quote = (value: string): string => {
    const encoded = nativeEncodeURIComponent(value);
    let quoted = "";
    for (let index = 0; index < encoded.length; index += 1) {
      const character = encoded[index]!;
      if (character === "!") quoted += "%21";
      else if (character === "'") quoted += "%27";
      else if (character === "(") quoted += "%28";
      else if (character === ")") quoted += "%29";
      else if (character === "*") quoted += "%2A";
      else if (character === "%" && encoded[index + 1] === "2" && encoded[index + 2] === "C") {
        quoted += ",";
        index += 2;
      } else quoted += character;
    }
    return quoted;
  };
  let query = "";
  for (let index = 0; index < parameters.length; index += 1) {
    if (index > 0) query += "&";
    query += `${quote(parameters[index]![0])}=${quote(parameters[index]![1])}`;
  }
  return `https://${REVIEWED_PUBMED_HOST}/entrez/eutils/${endpoint}?${query}`;
}

function searchParameters(term: string, perSpecialtyLimit: number, registration: NcbiRegistration): readonly (readonly [string, string])[] {
  return nativeObjectFreeze([
    nativeObjectFreeze(["db", "pubmed"] as const),
    nativeObjectFreeze(["term", term] as const),
    nativeObjectFreeze(["retmax", String(perSpecialtyLimit)] as const),
    nativeObjectFreeze(["retmode", "json"] as const),
    nativeObjectFreeze(["sort", "pub_date"] as const),
    ...registration,
  ]);
}

function summaryParameters(ids: string, registration: NcbiRegistration): readonly (readonly [string, string])[] {
  return nativeObjectFreeze([
    nativeObjectFreeze(["db", "pubmed"] as const),
    nativeObjectFreeze(["id", ids] as const),
    nativeObjectFreeze(["retmode", "json"] as const),
    ...registration,
  ]);
}

function fetchParameters(ids: string, registration: NcbiRegistration): readonly (readonly [string, string])[] {
  return nativeObjectFreeze([
    nativeObjectFreeze(["db", "pubmed"] as const),
    nativeObjectFreeze(["id", ids] as const),
    nativeObjectFreeze(["rettype", "abstract"] as const),
    nativeObjectFreeze(["retmode", "xml"] as const),
    ...registration,
  ]);
}

function assertExactRequestUrl(url: string, endpoint: Endpoint, parameters: readonly (readonly [string, string])[]): void {
  if (typeof url !== "string" || bytes(url) > MAX_REVIEWED_PUBMED_ARTIFACT_BYTES || url !== buildRequestUrl(endpoint, parameters)) fail("PubMed request differs from its exact reviewed URL");
  let parsed: URL;
  try {
    parsed = new NativeURL(url);
  } catch {
    fail("PubMed request URL is malformed");
  }
  if (parsed.protocol !== "https:" || parsed.hostname !== REVIEWED_PUBMED_HOST || parsed.host !== REVIEWED_PUBMED_HOST || parsed.port !== "" || parsed.username !== "" || parsed.password !== "" || parsed.pathname !== `/entrez/eutils/${endpoint}` || parsed.hash !== "") fail("PubMed request escaped the reviewed E-utilities scope");
  const required = endpoint === "esearch.fcgi" ? ["db", "term", "retmax", "retmode", "sort"] : endpoint === "esummary.fcgi" ? ["db", "id", "retmode"] : ["db", "id", "rettype", "retmode"];
  if (parameters.length !== required.length && parameters.length !== required.length + 2) fail("PubMed request parameters differ from the reviewed scope");
  for (let index = 0; index < required.length; index += 1) {
    if (parameters[index]?.[0] !== required[index]) fail("PubMed request parameters differ from the reviewed scope");
  }
  if (parameters.length === required.length + 2 && (parameters[required.length]?.[0] !== "tool" || parameters[required.length + 1]?.[0] !== "email")) fail("PubMed request parameters differ from the reviewed scope");
}

async function delay(milliseconds: number): Promise<void> {
  await new Promise<void>((resolve) => nativeSetTimeout(resolve, milliseconds));
}

async function acquireBuiltinRateSlot(): Promise<void> {
  const predecessor = builtinRateTail;
  let release!: () => void;
  builtinRateTail = new Promise<void>((resolve) => { release = resolve; });
  await predecessor;
  try {
    if (builtinLastDispatchAt !== null) {
      while (true) {
        const remaining = 340 - (nativeMonotonicNow() - builtinLastDispatchAt);
        if (remaining <= 0) break;
        await delay(remaining);
      }
    }
    builtinLastDispatchAt = nativeMonotonicNow();
  } finally {
    release();
  }
}

async function readBoundedResponse(response: Response, expectedUrl: string, maximum: number): Promise<Uint8Array> {
  if (!response || typeof response !== "object" || typeof response.status !== "number" || response.status < 200 || response.status >= 300 || response.redirected === true) fail("reviewed PubMed request returned a non-success response");
  if (response.url && response.url !== expectedUrl) fail("reviewed PubMed transport followed or reported a different URL");
  const declared = response.headers?.get("content-length");
  if (declared !== null && declared !== undefined) {
    const parsed = Number(declared);
    if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > maximum) fail("PubMed response exceeds the reviewed per-response byte limit");
  }
  if (!response.body) return new NativeUint8Array();
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const part = await reader.read();
      if (part.done) break;
      if (!(part.value instanceof NativeUint8Array)) fail("PubMed response stream returned an invalid chunk");
      length += part.value.byteLength;
      if (length > maximum) {
        await reader.cancel().catch(() => undefined);
        fail("PubMed response exceeds the reviewed per-response byte limit");
      }
      chunks.push(nativeUint8ArraySlice(part.value));
    }
  } finally {
    reader.releaseLock();
  }
  const output = new NativeUint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    nativeUint8ArraySet(output, chunk, offset);
    offset += chunk.byteLength;
  }
  return output;
}

function createBuiltinFetch(data: ConfigData): { readonly fetch: ReviewedPubMedFetch; readonly implementation: typeof globalThis.fetch } {
  const implementation = globalThis.fetch;
  if (typeof implementation !== "function" || typeof NativeAbortController !== "function") fail("built-in PubMed retrieval requires the Fetch and AbortController APIs");
  const captured = implementation.bind(globalThis);
  const fetch: ReviewedPubMedFetch = async (url) => {
    const controller = new NativeAbortController();
    const timer = nativeSetTimeout(() => controller.abort(), data.timeoutMs);
    try {
      const response = await captured(url, {
        method: "GET",
        headers: { Accept: "application/json, application/xml", "User-Agent": "aurora-agent/0.1" },
        redirect: "error",
        signal: controller.signal,
      });
      return await readBoundedResponse(response, url, data.responseByteLimit);
    } catch (error) {
      if (error instanceof ReviewedPubMedRetrievalError) throw error;
      fail(controller.signal.aborted ? "reviewed PubMed request timed out" : "reviewed PubMed request failed");
    } finally {
      nativeClearTimeout(timer);
    }
  };
  return nativeObjectFreeze({ fetch, implementation });
}

const BUNDLE_KEYS = ["schema_version", "generated_at", "synthetic_data", "sources", "records"] as const;
const SOURCE_KEYS = ["source_id", "authority", "uri", "retrieved_at", "content_sha256", "record_count"] as const;
const RECORD_KEYS = ["source_id", "specialty", "pmid", "title", "journal", "publication_date", "doi", "abstract_text", "abstract_truncated", "publication_types", "mesh_terms"] as const;

function validateStringTags(name: string, value: unknown): string[] {
  if (!nativeArrayIsArray(value) || value.length > MAX_REVIEWED_PUBMED_TAGS) fail(`${name} is outside its bound`);
  return value.map((item, index) => safeSourceText(`${name}[${index}]`, item)!).filter(Boolean);
}

function validateBundle(value: unknown, data: ConfigData): PublicLiteratureBundle {
  const raw = exactObject("transient PubMed bundle", value, BUNDLE_KEYS);
  if (raw.schema_version !== PUBLIC_LITERATURE_SCHEMA_VERSION || raw.synthetic_data !== false) fail("transient PubMed bundle markers are invalid");
  const generatedAt = timestamp("transient PubMed bundle generated_at", raw.generated_at);
  if (!nativeArrayIsArray(raw.sources) || raw.sources.length !== data.specialtyLanes.length || !nativeArrayIsArray(raw.records) || raw.records.length < 1 || raw.records.length > data.specialtyLanes.length * data.perSpecialtyLimit) fail("transient PubMed bundle collections exceed the reviewed plan");
  const entries = queryEntries(data.specialtyLanes);
  const noRegistration = nativeObjectFreeze([]) as NcbiRegistration;
  const sources: PublicLiteratureSource[] = raw.sources.map((source, index) => {
    const item = exactObject(`transient PubMed source ${index}`, source, SOURCE_KEYS);
    const lane = data.specialtyLanes[index]!;
    const expectedSourceId = `pubmed_${lane}`;
    const expectedParameters = searchParameters(entries[index]![1], data.perSpecialtyLimit, noRegistration);
    const contactFreeUrl = buildRequestUrl("esearch.fcgi", expectedParameters);
    assertExactRequestUrl(item.uri as string, "esearch.fcgi", expectedParameters);
    if (item.source_id !== expectedSourceId || item.authority !== PUBMED_AUTHORITY || item.uri !== contactFreeUrl || item.retrieved_at !== generatedAt) fail("transient PubMed source differs from the reviewed lane order");
    return {
      source_id: expectedSourceId,
      authority: PUBMED_AUTHORITY,
      uri: contactFreeUrl,
      retrieved_at: timestamp("source retrieved_at", item.retrieved_at),
      content_sha256: digest("source content_sha256", item.content_sha256),
      record_count: integer("source record_count", item.record_count, 1, data.perSpecialtyLimit),
    };
  });
  const seenPmids = new Set<string>();
  const laneCounts = new Map(data.specialtyLanes.map((lane) => [lane, 0]));
  const records: PublicLiteratureRecord[] = raw.records.map((record, index) => {
    const item = exactObject(`transient PubMed record ${index}`, record, RECORD_KEYS);
    const lane = normalizeLanes([item.specialty], `record ${index} specialty`)[0]!;
    if (!laneCounts.has(lane) || item.source_id !== `pubmed_${lane}`) fail("transient PubMed record escaped its reviewed specialty lane");
    const pmid = boundedText(`record ${index} pmid`, item.pmid, 32);
    if (!/^\d+$/.test(pmid) || seenPmids.has(pmid)) fail("transient PubMed record PMID is invalid or duplicated");
    seenPmids.add(pmid);
    laneCounts.set(lane, laneCounts.get(lane)! + 1);
    if (laneCounts.get(lane)! > data.perSpecialtyLimit) fail("transient PubMed lane exceeds its reviewed record limit");
    const publicationDate = item.publication_date === null ? null : normalizePublicationDate(item.publication_date);
    if (item.publication_date !== null && publicationDate !== item.publication_date) fail("transient PubMed record publication date is invalid");
    const doi = item.doi === null ? null : safeSourceText(`record ${index} doi`, item.doi);
    if (doi !== null && (!doi.startsWith("10.") || bytes(doi) > 512)) fail("transient PubMed record DOI is invalid");
    const abstractText = item.abstract_text === null ? null : safeSourceText(`record ${index} abstract`, item.abstract_text);
    if (abstractText !== null && bytes(abstractText) > MAX_REVIEWED_PUBMED_ABSTRACT_BYTES) fail("transient PubMed record abstract exceeds its safety bound");
    if (typeof item.abstract_truncated !== "boolean" || (item.abstract_truncated && abstractText === null)) fail("transient PubMed record abstract truncation marker is invalid");
    return {
      source_id: item.source_id as string,
      specialty: lane,
      pmid,
      title: safeSourceText(`record ${index} title`, item.title)!,
      journal: safeSourceText(`record ${index} journal`, item.journal)!,
      publication_date: publicationDate,
      doi,
      abstract_text: abstractText,
      abstract_truncated: item.abstract_truncated,
      publication_types: validateStringTags(`record ${index} publication_types`, item.publication_types),
      mesh_terms: validateStringTags(`record ${index} mesh_terms`, item.mesh_terms),
    };
  });
  if ([...laneCounts.values()].some((count) => count < 1)) fail("transient PubMed bundle is missing a reviewed specialty lane");
  for (const source of sources) {
    const count = records.filter((record) => record.source_id === source.source_id).length;
    if (source.record_count !== count || source.content_sha256 !== sourceHash(records, source.source_id)) fail(`source ${source.source_id} content hash or count is invalid`);
  }
  const bundle: PublicLiteratureBundle = { schema_version: PUBLIC_LITERATURE_SCHEMA_VERSION, generated_at: generatedAt, synthetic_data: false, sources, records };
  const encoded = canonicalJson(bundle);
  if (bytes(encoded) > data.bundleByteLimit) fail("transient PubMed bundle exceeds its reviewed byte limit");
  return nativeJsonParse(encoded) as PublicLiteratureBundle;
}

const CONFIG_PUBLIC_KEYS = ["bundle_byte_limit", "config_digest", "ncbi_registration_configured", "ncbi_registration_digest", "per_specialty_limit", "query_set_digest", "record_limit", "request_limit", "response_byte_limit", "specialty_lanes", "timeout_ms", "total_response_byte_limit", "transport_config_digest", "transport_id", "transport_version"] as const;

function assertConfigLive(config: ReviewedPubMedRetrievalConfig, expected: ConfigData): void {
  const current = requireConfigData(config);
  if (current !== expected) fail("PubMed retrieval config identity changed after review");
  const payload = configPayload(expected);
  const projection = {
    specialty_lanes: [...config.specialty_lanes],
    per_specialty_limit: config.per_specialty_limit,
    timeout_ms: config.timeout_ms,
    request_limit: config.request_limit,
    record_limit: config.record_limit,
    response_byte_limit: config.response_byte_limit,
    total_response_byte_limit: config.total_response_byte_limit,
    bundle_byte_limit: config.bundle_byte_limit,
    transport_id: config.transport_id,
    transport_version: config.transport_version,
    transport_config_digest: config.transport_config_digest,
    query_set_digest: config.query_set_digest,
    ncbi_registration_configured: config.ncbi_registration_configured,
    ncbi_registration_digest: config.ncbi_registration_digest,
    config_digest: config.config_digest,
  };
  const expectedProjection = {
    specialty_lanes: payload.specialty_lanes,
    per_specialty_limit: payload.per_specialty_limit,
    timeout_ms: payload.timeout_ms,
    request_limit: payload.request_limit,
    record_limit: payload.record_limit,
    response_byte_limit: payload.response_byte_limit,
    total_response_byte_limit: payload.total_response_byte_limit,
    bundle_byte_limit: payload.bundle_byte_limit,
    transport_id: payload.transport_id,
    transport_version: payload.transport_version,
    transport_config_digest: payload.transport_config_digest,
    query_set_digest: payload.query_set_digest,
    ncbi_registration_configured: payload.ncbi_registration_configured,
    ncbi_registration_digest: payload.ncbi_registration_digest,
    config_digest: digestJsonSync(payload),
  };
  if (canonicalJson(projection) !== canonicalJson(expectedProjection) || !hasExactEnumerableKeys(config, CONFIG_PUBLIC_KEYS)) fail("PubMed retrieval config changed after review");
  if (ncbiRegistrationDigest(expected.registration) !== expected.registrationDigest || querySetDigest(queryEntries(expected.specialtyLanes)) !== expected.querySetDigest) fail("PubMed retrieval bindings changed after review");
}

interface AdapterState {
  readonly config: ReviewedPubMedRetrievalConfig;
  readonly data: ConfigData;
  readonly fetch: ReviewedPubMedFetch;
  readonly fetchAnchor: ReviewedPubMedFetch;
  readonly builtinImplementation: typeof globalThis.fetch | null;
  readonly entries: readonly (readonly [ReviewedPubMedSpecialtyLane, string])[];
  readonly now: () => number;
  lastDispatchAt: number | null;
}

export interface ReviewedPubMedRetrievalAdapterOptions {
  fetch?: ReviewedPubMedFetch;
}

const ADAPTER_PRIVATE = new WeakMap<ReviewedPubMedRetrievalAdapter, AdapterState>();

function expectedPlanJSON(data: ConfigData): ReviewedPubMedRetrievalPlanJSON {
  const configDigest = digestJsonSync(configPayload(data));
  const payload = planPayload(data, configDigest);
  return { ...payload, plan_digest: digestJsonSync(payload) } as ReviewedPubMedRetrievalPlanJSON;
}

function guardAdapter(adapter: ReviewedPubMedRetrievalAdapter, plan: ReviewedPubMedRetrievalPlan): AdapterState {
  const state = ADAPTER_PRIVATE.get(adapter);
  if (!state || nativeObjectGetPrototypeOf(adapter) !== ReviewedPubMedRetrievalAdapter.prototype || nativeObjectKeys(adapter).length !== 0) fail("reviewed PubMed adapter identity changed after review");
  assertConfigLive(state.config, state.data);
  const planState = assertPlanLive(plan);
  if (planState.canonical !== canonicalJson(expectedPlanJSON(state.data))) fail("PubMed retrieval plan or config drifted after review");
  if (state.fetch !== state.fetchAnchor || typeof state.fetch !== "function") fail("PubMed fetch callable changed after review");
  if (canonicalJson(state.entries) !== canonicalJson(queryEntries(state.data.specialtyLanes)) || querySetDigest(state.entries) !== state.data.querySetDigest) fail("fixed PubMed specialty queries changed after review");
  if (state.data.registration.length > 0) {
    const registered = nativeObjectFromEntries(state.data.registration);
    if (registered.tool === undefined || registered.email === undefined || ncbiRegistration(registered.tool, registered.email).length !== 2) fail("NCBI registration changed after review");
  }
  return state;
}

async function observeRateLimit(state: AdapterState): Promise<void> {
  if (state.builtinImplementation !== null) {
    await acquireBuiltinRateSlot();
    return;
  }
  if (state.lastDispatchAt === null) return;
  while (true) {
    const remaining = 340 - (state.now() - state.lastDispatchAt);
    if (remaining <= 0) return;
    await delay(remaining);
  }
}

interface ExecutionCounters {
  requestCount: number;
  responseBytes: number;
}

async function performRequest(
  adapter: ReviewedPubMedRetrievalAdapter,
  plan: ReviewedPubMedRetrievalPlan,
  counters: ExecutionCounters,
  endpoint: Endpoint,
  parameters: readonly (readonly [string, string])[],
): Promise<JsonObject | XmlNode> {
  let state = guardAdapter(adapter, plan);
  if (counters.requestCount >= state.data.specialtyLanes.length * 3) fail("PubMed retrieval exceeded its reviewed request count");
  await observeRateLimit(state);
  state = guardAdapter(adapter, plan);
  const expectedEndpoint = REVIEWED_PUBMED_ENDPOINTS[counters.requestCount % REVIEWED_PUBMED_ENDPOINTS.length];
  if (expectedEndpoint !== endpoint) fail("PubMed retrieval departed from its exact reviewed request sequence");
  const url = buildRequestUrl(endpoint, parameters);
  assertExactRequestUrl(url, endpoint, parameters);
  state.lastDispatchAt = state.now();
  counters.requestCount += 1;
  let raw: ReviewedPubMedRawResponse;
  let timeoutHandle: ReturnType<typeof globalThis.setTimeout> | undefined;
  try {
    const pending = state.fetch(url);
    if (state.builtinImplementation === null) {
      const timeout = new NativePromise<never>((_resolve, reject) => {
        timeoutHandle = nativeSetTimeout(() => reject(new ReviewedPubMedRetrievalError("reviewed PubMed request timed out")), state.data.timeoutMs);
      });
      raw = await nativePromiseRace<ReviewedPubMedRawResponse>([pending, timeout]);
    } else {
      raw = await pending;
    }
  } catch (error) {
    if (error instanceof ReviewedPubMedRetrievalError) throw error;
    fail("reviewed PubMed request failed");
  } finally {
    if (timeoutHandle !== undefined) nativeClearTimeout(timeoutHandle);
  }
  state = guardAdapter(adapter, plan);
  const remaining = state.data.totalResponseByteLimit - counters.responseBytes;
  if (remaining < 1) fail("PubMed retrieval exceeds its reviewed total response byte limit");
  const [encoded, parsed] = responseBytes(raw, endpoint, Math.min(state.data.responseByteLimit, remaining));
  counters.responseBytes += encoded.byteLength;
  if (counters.responseBytes > state.data.totalResponseByteLimit) fail("PubMed retrieval exceeds its reviewed total response byte limit");
  return parsed;
}

function currentWholeSecond(): string {
  return new Date(Math.floor(Date.now() / 1_000) * 1_000).toISOString().replace(".000Z", "Z");
}

function extractIds(search: JsonObject, limit: number, lane: ReviewedPubMedSpecialtyLane): string[] {
  const result = search.esearchresult;
  const list = isObject(result) ? result.idlist : undefined;
  if (!nativeArrayIsArray(list) || list.length < 1 || list.length > limit) fail(`PubMed returned an invalid record set for specialty lane ${lane}`);
  const ids = list.map((value) => {
    if (typeof value !== "string" || !/^\d{1,32}$/.test(value)) fail(`PubMed returned an invalid PMID for specialty lane ${lane}`);
    return value;
  });
  if (new Set(ids).size !== ids.length) fail(`PubMed returned duplicate PMIDs for specialty lane ${lane}`);
  return ids;
}

function summaryRecord(result: JsonObject, pmid: string): JsonObject | null {
  const value = result[pmid];
  return isObject(value) ? value : null;
}

function summaryDoi(article: JsonObject, pmid: string): string | null {
  const identifiers = article.articleids;
  if (identifiers === undefined || identifiers === null) return null;
  if (!nativeArrayIsArray(identifiers) || identifiers.length > MAX_REVIEWED_PUBMED_TAGS) fail(`PubMed ${pmid} article identifiers exceed their bound`);
  for (const value of identifiers) {
    if (!isObject(value)) continue;
    if (value.idtype === "doi") {
      const doi = safeSourceText(`PubMed ${pmid} doi`, value.value, false);
      if (doi !== null && (!doi.startsWith("10.") || bytes(doi) > 512)) fail(`PubMed ${pmid} DOI is invalid`);
      return doi;
    }
  }
  return null;
}

async function executeAdapter(
  adapter: ReviewedPubMedRetrievalAdapter,
  plan: ReviewedPubMedRetrievalPlan,
  options: { approveSourceDispatch: true; retrievedAt?: string },
): Promise<ReviewedPubMedRetrievalResult> {
  if (!options || typeof options !== "object" || options.approveSourceDispatch !== true) fail("PubMed retrieval requires explicit literal approval");
  for (const key of nativeObjectKeys(options)) if (key !== "approveSourceDispatch" && key !== "retrievedAt") fail("PubMed execution options contain unsupported fields");
  const generatedAt = options.retrievedAt === undefined ? currentWholeSecond() : timestamp("retrievedAt", options.retrievedAt);
  const state = guardAdapter(adapter, plan);
  const counters: ExecutionCounters = { requestCount: 0, responseBytes: 0 };
  const records: PublicLiteratureRecord[] = [];
  const sources: PublicLiteratureSource[] = [];
  const seenPmids = new Set<string>();
  const registration = state.data.registration;

  for (const [lane, term] of state.entries) {
    const searchParams = searchParameters(term, state.data.perSpecialtyLimit, registration);
    const search = await performRequest(adapter, plan, counters, "esearch.fcgi", searchParams);
    if (!isObject(search)) fail("PubMed search response must be JSON");
    const ids = extractIds(search, state.data.perSpecialtyLimit, lane);
    let joined = "";
    for (let index = 0; index < ids.length; index += 1) joined += `${index === 0 ? "" : ","}${ids[index]!}`;
    const summary = await performRequest(adapter, plan, counters, "esummary.fcgi", summaryParameters(joined, registration));
    if (!isObject(summary) || !isObject(summary.result)) fail("PubMed summary response has no result object");
    const fetched = await performRequest(adapter, plan, counters, "efetch.fcgi", fetchParameters(joined, registration));
    if (!isXmlNode(fetched)) fail("PubMed fetch response must be XML");
    const xml = projectXmlArticles(fetched);
    const laneRecords: PublicLiteratureRecord[] = [];
    for (const pmid of ids) {
      if (seenPmids.has(pmid)) continue;
      const article = summaryRecord(summary.result, pmid);
      if (!article) continue;
      const title = safeSourceText(`PubMed ${pmid} title`, article.title);
      const journal = safeSourceText(`PubMed ${pmid} journal`, article.fulljournalname ?? article.source);
      const content = xml.get(pmid);
      const record: PublicLiteratureRecord = {
        source_id: `pubmed_${lane}`,
        specialty: lane,
        pmid,
        title: title!,
        journal: journal!,
        publication_date: normalizePublicationDate(article.epubdate ?? article.pubdate),
        doi: summaryDoi(article, pmid),
        abstract_text: content?.abstractText ?? null,
        abstract_truncated: content?.abstractTruncated ?? false,
        publication_types: [...(content?.publicationTypes ?? [])],
        mesh_terms: [...(content?.meshTerms ?? [])],
      };
      laneRecords.push(record);
      records.push(record);
      seenPmids.add(pmid);
    }
    if (laneRecords.length < 1) fail(`PubMed lane ${lane} produced no unique citation records`);
    const sourceId = `pubmed_${lane}`;
    sources.push({
      source_id: sourceId,
      authority: PUBMED_AUTHORITY,
      uri: buildRequestUrl("esearch.fcgi", searchParameters(term, state.data.perSpecialtyLimit, nativeObjectFreeze([]) as NcbiRegistration)),
      retrieved_at: generatedAt,
      content_sha256: sourceHash(records, sourceId),
      record_count: laneRecords.length,
    });
  }
  if (counters.requestCount !== state.data.specialtyLanes.length * 3) fail("PubMed retrieval did not complete its reviewed request sequence");
  guardAdapter(adapter, plan);
  const validatedBundle = validateBundle({ schema_version: PUBLIC_LITERATURE_SCHEMA_VERSION, generated_at: generatedAt, synthetic_data: false, sources, records }, state.data);
  const bundleDigest = reviewedPubMedBundleDigest(validatedBundle);
  const sourceReceipts: ReviewedPubMedSourceReceipt[] = [];
  const sourceReceiptJson: ReviewedPubMedSourceReceiptJSON[] = [];
  for (let index = 0; index < validatedBundle.sources.length; index += 1) {
    const source = validatedBundle.sources[index]!;
    const sourceReceipt = new ReviewedPubMedSourceReceipt({ specialtyLane: state.data.specialtyLanes[index]!, sourceId: source.source_id, contentDigest: source.content_sha256, recordCount: source.record_count });
    sourceReceipts[index] = sourceReceipt;
    sourceReceiptJson[index] = sourceReceipt.toJSON();
  }
  const sourceSetDigest = digestJsonSync(sourceReceiptJson);
  let abstractCount = 0;
  for (let index = 0; index < validatedBundle.records.length; index += 1) if (validatedBundle.records[index]!.abstract_text !== null) abstractCount += 1;
  const receipt = new ReviewedPubMedRetrievalReceipt({
    planDigest: plan.plan_digest,
    configDigest: state.config.config_digest,
    specialtyLanes: state.data.specialtyLanes,
    transportId: state.data.transportId,
    transportVersion: state.data.transportVersion,
    transportConfigDigest: state.data.transportConfigDigest,
    querySetDigest: state.data.querySetDigest,
    ncbiRegistrationConfigured: state.data.registration.length > 0,
    ncbiRegistrationDigest: state.data.registrationDigest,
    generatedAt,
    bundleDigest,
    sourceSetDigest,
    sources: sourceReceipts,
    recordCount: validatedBundle.records.length,
    abstractCount,
    requestCount: counters.requestCount,
    responseBytes: counters.responseBytes,
  });
  return new ReviewedPubMedRetrievalResult(validatedBundle, receipt);
}

function isXmlNode(value: JsonObject | XmlNode): value is XmlNode {
  return typeof (value as XmlNode).name === "string" && nativeArrayIsArray((value as XmlNode).children);
}

export class ReviewedPubMedRetrievalAdapter {
  constructor(config: ReviewedPubMedRetrievalConfig, options: ReviewedPubMedRetrievalAdapterOptions = {}) {
    const data = requireConfigData(config);
    if (!options || typeof options !== "object" || nativeArrayIsArray(options)) fail("reviewed PubMed adapter options are malformed");
    for (const key of nativeObjectKeys(options)) if (key !== "fetch") fail("reviewed PubMed adapter options are malformed");
    let selectedFetch: ReviewedPubMedFetch;
    let builtinImplementation: typeof globalThis.fetch | null = null;
    if (options.fetch === undefined) {
      if (data.transportId !== BUILTIN_PUBMED_TRANSPORT_ID || data.transportVersion !== BUILTIN_PUBMED_TRANSPORT_VERSION || data.transportConfigDigest !== BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST) fail("built-in PubMed retrieval requires its exact transport identity");
      const builtin = createBuiltinFetch(data);
      selectedFetch = builtin.fetch;
      builtinImplementation = builtin.implementation;
    } else {
      if (typeof options.fetch !== "function") fail("injected PubMed fetch must be callable");
      if (data.transportId === BUILTIN_PUBMED_TRANSPORT_ID || data.transportConfigDigest === BUILTIN_PUBMED_TRANSPORT_CONFIG_DIGEST) fail("an injected PubMed fetch requires a distinct reviewed transport identity");
      selectedFetch = options.fetch;
    }
    ADAPTER_PRIVATE.set(this, {
      config,
      data,
      fetch: selectedFetch,
      fetchAnchor: selectedFetch,
      builtinImplementation,
      entries: queryEntries(data.specialtyLanes),
      now: nativeMonotonicNow,
      lastDispatchAt: null,
    });
    assertConfigLive(config, data);
    nativeObjectFreeze(this);
  }

  get config(): ReviewedPubMedRetrievalConfig {
    const state = ADAPTER_PRIVATE.get(this);
    if (!state) fail("reviewed PubMed adapter identity is invalid");
    return state.config;
  }

  prepare(): ReviewedPubMedRetrievalPlan {
    const state = ADAPTER_PRIVATE.get(this);
    if (!state || nativeObjectGetPrototypeOf(this) !== ReviewedPubMedRetrievalAdapter.prototype) fail("reviewed PubMed adapter identity is invalid");
    assertConfigLive(state.config, state.data);
    return ReviewedPubMedRetrievalPlan.fromConfig(state.config);
  }

  async execute(plan: ReviewedPubMedRetrievalPlan, options: { approveSourceDispatch: true; retrievedAt?: string }): Promise<ReviewedPubMedRetrievalResult> {
    return executeAdapter(this, plan, options);
  }
}

const RESULT_PRIVATE = new WeakMap<ReviewedPubMedRetrievalResult, string>();

export interface ReviewedPubMedTransientValueJSON extends JsonObject {
  schema: typeof REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA;
  lane: ReviewedPubMedSpecialtyLane;
  bundle: PublicLiteratureBundle;
  receipt: ReviewedPubMedRetrievalReceiptJSON;
  retention: typeof TRANSIENT_RETENTION;
}

export class ReviewedPubMedRetrievalResult {
  readonly receipt: ReviewedPubMedRetrievalReceipt;

  constructor(bundle: PublicLiteratureBundle, receipt: ReviewedPubMedRetrievalReceipt) {
    const receiptJson = exactReceiptJSON(receipt);
    const snapshot = snapshotJsonValue("transient PubMed bundle", bundle) as PublicLiteratureBundle;
    const text = canonicalJson(snapshot);
    if (reviewedPubMedBundleDigest(snapshot) !== receiptJson.bundle_digest) fail("transient PubMed bundle does not match its receipt");
    RESULT_PRIVATE.set(this, text);
    this.receipt = receipt;
    nativeObjectFreeze(this);
  }

  get bundle(): PublicLiteratureBundle {
    const text = RESULT_PRIVATE.get(this);
    if (text === undefined || nativeObjectGetPrototypeOf(this) !== ReviewedPubMedRetrievalResult.prototype) fail("transient PubMed result identity is invalid");
    return nativeJsonParse(text) as PublicLiteratureBundle;
  }

  get report(): ReviewedPubMedRetrievalReceipt {
    return this.receipt;
  }

  toTransientJSON(): ReviewedPubMedTransientValueJSON {
    const receipt = exactReceiptJSON(this.receipt);
    if (receipt.specialty_lanes.length !== 1) fail("generic adapter values require a single-lane reviewed plan");
    return {
      schema: REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA,
      lane: receipt.specialty_lanes[0]!,
      bundle: this.bundle,
      receipt,
      retention: TRANSIENT_RETENTION,
    };
  }
}

export interface ReviewedPubMedExecutionMetadata extends JsonObject {
  schema: typeof REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA;
  reviewed_plan_digest: string;
  approve_source_dispatch: true;
  retrieved_at: string | null;
}

const EXECUTION_METADATA_KEYS = ["schema", "reviewed_plan_digest", "approve_source_dispatch", "retrieved_at"] as const;

export function createReviewedPubMedExecutionMetadata(
  plan: ReviewedPubMedRetrievalPlan,
  options: { approveSourceDispatch: true; retrievedAt?: string },
): ReviewedPubMedExecutionMetadata {
  assertPlanLive(plan);
  if (!options || typeof options !== "object" || options.approveSourceDispatch !== true) fail("PubMed execution metadata requires explicit literal approval");
  for (const key of nativeObjectKeys(options)) if (key !== "approveSourceDispatch" && key !== "retrievedAt") fail("PubMed execution metadata options contain unsupported fields");
  const metadata: ReviewedPubMedExecutionMetadata = {
    schema: REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA,
    reviewed_plan_digest: plan.plan_digest,
    approve_source_dispatch: true,
    retrieved_at: options.retrievedAt === undefined ? null : timestamp("retrievedAt", options.retrievedAt),
  };
  boundedArtifact("reviewed PubMed execution metadata", metadata);
  return metadata;
}

function validateExecutionMetadata(value: unknown, planDigest: string): ReviewedPubMedExecutionMetadata {
  const raw = exactObject("reviewed PubMed execution metadata", value, EXECUTION_METADATA_KEYS);
  if (raw.schema !== REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA || raw.reviewed_plan_digest !== planDigest || raw.approve_source_dispatch !== true) fail("reviewed PubMed execution metadata does not authorize the reviewed plan");
  const metadata: ReviewedPubMedExecutionMetadata = {
    schema: REVIEWED_PUBMED_EXECUTION_METADATA_SCHEMA,
    reviewed_plan_digest: digest("execution metadata reviewed_plan_digest", raw.reviewed_plan_digest),
    approve_source_dispatch: true,
    retrieved_at: raw.retrieved_at === null ? null : timestamp("execution metadata retrieved_at", raw.retrieved_at),
  };
  boundedArtifact("reviewed PubMed execution metadata", metadata);
  return metadata;
}

function acquisitionRequest(context: AutonomousEvidenceAcquisitionContext): JsonObject {
  if (!context || typeof context !== "object" || nativeArrayIsArray(context) || !isObject(context.request)) fail("generic PubMed acquire context is malformed");
  return context.request;
}

function assertReceiptMatchesBundle(receipt: ReviewedPubMedRetrievalReceipt, bundle: PublicLiteratureBundle, data: ConfigData): void {
  const receiptJson = exactReceiptJSON(receipt);
  const expectedSources: ReviewedPubMedSourceReceiptJSON[] = [];
  for (let index = 0; index < bundle.sources.length; index += 1) {
    const source = bundle.sources[index]!;
    expectedSources[index] = {
      schema: REVIEWED_PUBMED_RETRIEVAL_SOURCE_RECEIPT_SCHEMA,
      specialty_lane: data.specialtyLanes[index]!,
      source_id: source.source_id,
      content_digest: source.content_sha256,
      record_count: source.record_count,
    };
  }
  let abstractCount = 0;
  for (let index = 0; index < bundle.records.length; index += 1) if (bundle.records[index]!.abstract_text !== null) abstractCount += 1;
  const expectedSourceSetDigest = digestJsonSync(expectedSources);
  if (
    receiptJson.generated_at !== bundle.generated_at
    || receiptJson.source_count !== bundle.sources.length
    || receiptJson.record_count !== bundle.records.length
    || receiptJson.abstract_count !== abstractCount
    || receiptJson.source_set_digest !== expectedSourceSetDigest
    || canonicalJson(receiptJson.sources) !== canonicalJson(expectedSources)
  ) fail("generic PubMed transient receipt metadata does not match its bundle");
}

function validateTransientValue(value: JsonValue, lane: ReviewedPubMedSpecialtyLane, plan: ReviewedPubMedRetrievalPlan, data: ConfigData): { bundle: PublicLiteratureBundle; receipt: ReviewedPubMedRetrievalReceipt } {
  const raw = exactObject("generic PubMed transient value", snapshotJsonValue("generic PubMed transient value", value), ["schema", "lane", "bundle", "receipt", "retention"]);
  if (raw.schema !== REVIEWED_PUBMED_TRANSIENT_VALUE_SCHEMA || raw.lane !== lane || raw.retention !== TRANSIENT_RETENTION) fail("generic PubMed transient value identity is invalid");
  const receipt = ReviewedPubMedRetrievalReceipt.fromJSON(raw.receipt);
  if (receipt.plan_digest !== plan.plan_digest || receipt.specialty_lanes.length !== 1 || receipt.specialty_lanes[0] !== lane) fail("generic PubMed transient value names a different reviewed plan");
  if (receipt.config_digest !== plan.config_digest || receipt.transport_id !== plan.transport_id || receipt.transport_version !== plan.transport_version || receipt.transport_config_digest !== plan.transport_config_digest || receipt.query_set_digest !== plan.query_set_digest || receipt.ncbi_registration_configured !== plan.ncbi_registration_configured || receipt.ncbi_registration_digest !== plan.ncbi_registration_digest) fail("generic PubMed transient receipt differs from its reviewed plan");
  const bundle = validateBundle(raw.bundle, data);
  if (reviewedPubMedBundleDigest(bundle) !== receipt.bundle_digest) fail("generic PubMed transient bundle does not match its receipt");
  assertReceiptMatchesBundle(receipt, bundle, data);
  return { bundle, receipt };
}

export function createReviewedPubMedAutonomousEvidenceRegistration(
  adapter: ReviewedPubMedRetrievalAdapter,
  plan: ReviewedPubMedRetrievalPlan,
  specialtyLane: ReviewedPubMedSpecialtyLane,
): Omit<AutonomousEvidenceAdapterRegistrationInput, "domains"> & { domains: ["biomedical", "neuroscience"] } {
  const lane = normalizeLanes([specialtyLane], "registration specialtyLane")[0]!;
  const state = guardAdapter(adapter, plan);
  if (plan.specialty_lanes.length !== 1 || plan.specialty_lanes[0] !== lane) fail("generic PubMed registration requires a single-lane reviewed plan");
  const frozenPlan = ReviewedPubMedRetrievalPlan.fromJSON(plan.toJSON());
  const adapterId = `ncbi_pubmed_${lane}_${frozenPlan.plan_digest.slice(0, 16)}`;
  const expectedSourceId = `pubmed_${lane}`;
  return {
    adapterId,
    version: REVIEWED_PUBMED_ADAPTER_VERSION,
    domains: ["biomedical", "neuroscience"],
    capabilities: ["evidence", "literature", "provenance", "public_literature_refresh"],
    sourceKinds: ["pubmed", "public_literature"],
    acquire: async (context) => {
      const request = acquisitionRequest(context);
      if (request.source_id !== expectedSourceId || request.source_digest !== frozenPlan.plan_digest) fail("generic PubMed request does not match its reviewed source");
      const metadata = validateExecutionMetadata(request.metadata, frozenPlan.plan_digest);
      const result = await executeAdapter(adapter, frozenPlan, {
        approveSourceDispatch: metadata.approve_source_dispatch,
        ...(metadata.retrieved_at === null ? {} : { retrievedAt: metadata.retrieved_at }),
      });
      return result.toTransientJSON();
    },
    project: async (value, context): Promise<readonly AutonomousEvidenceObservationInput[]> => {
      guardAdapter(adapter, frozenPlan);
      const validated = validateTransientValue(value, lane, frozenPlan, state.data);
      const label = context?.requirement?.label;
      if (typeof label !== "string" || !label.trim()) fail("generic PubMed project context has no requirement label");
      return [{
        label,
        kind: "provenance",
        status: "observed",
        value_digest: validated.receipt.bundle_digest,
        source_digest: validated.receipt.source_set_digest,
        confidence: null,
        limitations: [...LIMITATIONS],
      }];
    },
  };
}
