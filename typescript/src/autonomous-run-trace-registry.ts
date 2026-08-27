import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousDomainName } from "./autonomous.js";
import {
  AUTONOMOUS_RUN_TRACE_PHASES,
  AUTONOMOUS_RUN_TRACE_SCHEMA,
  AUTONOMOUS_RUN_TRACE_SNAPSHOT_SCHEMA,
  AUTONOMOUS_RUN_TRACE_STATUSES,
  MAX_AUTONOMOUS_RUN_TRACE_EVENTS,
  MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES,
  validateAutonomousRunTraceEvent,
  validateAutonomousRunTraceSnapshot,
  type AutonomousRunTraceEvent,
  type AutonomousRunTraceSnapshot,
  type AutonomousRunTraceStatus,
  type AutonomousRunTraceStore,
  type AutonomousRunTraceSummary,
  type AutonomousRunTraceTextStore,
  type AutonomousRunTraceTransactionalTextStore,
} from "./autonomous-run-trace.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * A bounded operator index over completed or in-flight trace runs.
 *
 * The append-only trace store remains the source journal. This registry is a query/retention
 * projection: it validates the source chain before importing, retains only trace metadata, and
 * never becomes an authority to replay a provider, tool, source, learner, or external effect.
 */
export const AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-run-trace-registry/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-run-trace-registry-snapshot/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION = "metadata_only_no_prompts_responses_tool_payloads_credentials_evidence_or_effect_values" as const;
export const AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY = "operator_query_and_retention_projection_only;does_not_authorize_execution" as const;
export const AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL = "never_returned" as const;
export const AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA = "bioprism-typescript-autonomous-run-trace-registry-publication/0.1" as const;
export const MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS = 10_000;
export const MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS = MAX_AUTONOMOUS_RUN_TRACE_EVENTS;
export const MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES = MAX_AUTONOMOUS_RUN_TRACE_SNAPSHOT_BYTES;
const MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_COUNTER = Number.MAX_SAFE_INTEGER;

const TRACE_DOMAINS: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];

export interface AutonomousRunTraceRetentionPolicy extends JsonObject {
  max_runs: number;
  max_events: number;
  max_bytes: number;
  retain_events: boolean;
  keep_incomplete: boolean;
  eviction: "oldest_eligible_terminal_run";
}

export interface AutonomousRunTraceRetentionPolicyInput {
  max_runs?: number;
  max_events?: number;
  max_bytes?: number;
  retain_events?: boolean;
  keep_incomplete?: boolean;
}

export interface AutonomousRunTraceRegistryRecord extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA;
  run_id: string;
  summary: AutonomousRunTraceSummary;
  providers: string[];
  models: string[];
  source_snapshot_digest: string;
  source_sequence: number;
  source_head_digest: string;
  events: AutonomousRunTraceEvent[];
  retained_event_count: number;
  record_digest: string;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

export interface AutonomousRunTraceRegistrySnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA;
  snapshot_generation: number;
  previous_snapshot_digest: string | null;
  policy: AutonomousRunTraceRetentionPolicy;
  record_count: number;
  event_count: number;
  retained_event_count: number;
  records: AutonomousRunTraceRegistryRecord[];
  snapshot_digest: string;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

export interface AutonomousRunTraceRegistryQuery {
  run_id?: string;
  domain?: AutonomousDomainName;
  status?: AutonomousRunTraceStatus;
  provider?: string;
  model?: string;
  after_run_id?: string;
  limit?: number;
}

export interface AutonomousRunTraceRegistryPage extends JsonObject {
  records: AutonomousRunTraceRegistryRecord[];
  next_after_run_id: string | null;
  total_matches: number;
  retained_event_count: number;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

export interface AutonomousRunTraceRegistryEventQuery extends Omit<AutonomousRunTraceRegistryQuery, "after_run_id"> {
  phase?: typeof AUTONOMOUS_RUN_TRACE_PHASES[number];
  after_sequence?: number;
}

export interface AutonomousRunTraceRegistryImportReport extends JsonObject {
  imported_run_ids: string[];
  replaced_run_ids: string[];
  unchanged_run_ids: string[];
  evicted_run_ids: string[];
  snapshot: AutonomousRunTraceRegistrySnapshot;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

export interface AutonomousRunTraceRegistryPublication extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA;
  status: "published" | "failed";
  run_id: string;
  run_import_state: "imported" | "replaced" | "unchanged" | "not_present" | "unknown";
  source_snapshot_digest: string | null;
  registry_snapshot_digest: string | null;
  evicted_run_count: number;
  error_class: string | null;
  failure_code: "trace_snapshot_invalid" | "trace_registry_rejected" | "trace_registry_publication_failed" | null;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

export interface AutonomousRunTraceRegistryIntegrity extends JsonObject {
  verified: true;
  runs: number;
  events: number;
  retained_event_count: number;
  snapshot_digest: string;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
}

function boundedText(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function boundedLimit(name: string, value: unknown, maximum: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1 || value > maximum) throw new ArgumentError(`${name} is outside its bounds`);
  return value;
}

function stringList(name: string, value: unknown): string[] {
  if (!Array.isArray(value) || value.length > 512) throw new ArgumentError(`${name} must be a bounded array`);
  const result = value.map((item) => boundedText(name, item));
  if (new Set(result).size !== result.length || [...result].sort().some((item, index) => item !== result[index])) throw new ArgumentError(`${name} must be sorted and unique`);
  return result;
}

function normalizePolicy(input: AutonomousRunTraceRetentionPolicyInput | AutonomousRunTraceRetentionPolicy = {}): AutonomousRunTraceRetentionPolicy {
  if (!isObject(input)) throw new ArgumentError("autonomous run trace registry policy must be an object");
  const maxRuns = boundedLimit("autonomous run trace registry max_runs", input.max_runs ?? MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS, MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_RUNS);
  const maxEvents = boundedLimit("autonomous run trace registry max_events", input.max_events ?? MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS, MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_EVENTS);
  const maxBytes = boundedLimit("autonomous run trace registry max_bytes", input.max_bytes ?? MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES, MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES);
  if (maxBytes < 16_000) throw new ArgumentError("autonomous run trace registry max_bytes is too small for a registry snapshot");
  if (input.retain_events !== undefined && typeof input.retain_events !== "boolean") throw new ArgumentError("autonomous run trace registry retain_events must be boolean");
  if (input.keep_incomplete !== undefined && typeof input.keep_incomplete !== "boolean") throw new ArgumentError("autonomous run trace registry keep_incomplete must be boolean");
  if (input.eviction !== undefined && input.eviction !== "oldest_eligible_terminal_run") throw new ArgumentError("autonomous run trace registry eviction policy is unsupported");
  return {
    max_runs: maxRuns,
    max_events: maxEvents,
    max_bytes: maxBytes,
    retain_events: input.retain_events ?? true,
    keep_incomplete: input.keep_incomplete ?? true,
    eviction: "oldest_eligible_terminal_run",
  };
}

function cloneRecord(record: AutonomousRunTraceRegistryRecord): AutonomousRunTraceRegistryRecord {
  return structuredClone(record);
}

function sortRecords(records: readonly AutonomousRunTraceRegistryRecord[]): AutonomousRunTraceRegistryRecord[] {
  return [...records].sort((left, right) => (left.summary.last_sequence ?? 0) - (right.summary.last_sequence ?? 0) || left.run_id.localeCompare(right.run_id));
}

function summaryFromEvents(runId: string, events: readonly AutonomousRunTraceEvent[]): AutonomousRunTraceSummary {
  if (!events.length) throw new ArgumentError("autonomous run trace registry cannot index an empty run");
  const firstEvent = events[0]!;
  const lastEvent = events[events.length - 1]!;
  const taskDigest = firstEvent.task_digest;
  if (events.some((event) => event.run_id !== runId || event.task_digest !== taskDigest)) throw new ArgumentError("autonomous run trace registry run events have inconsistent identity");
  const domains = [...new Set(events.flatMap((event) => event.domains))].sort() as AutonomousDomainName[];
  const selectionDigests = [...new Set(events.map((event) => event.selection_digest).filter((value): value is string => value !== null))].sort();
  const failureCodes = [...new Set(events.map((event) => event.failure_code).filter((value): value is string => value !== null))].sort();
  const completedInvocations = events.filter((event) => event.phase === "provider_invocation_finished");
  const body = {
    schema: AUTONOMOUS_RUN_TRACE_SCHEMA,
    run_id: runId,
    task_digest: taskDigest,
    domains,
    status: lastEvent.status,
    first_sequence: firstEvent.sequence,
    last_sequence: lastEvent.sequence,
    event_count: events.length,
    provider_invocations: completedInvocations.length,
    provider_failures: completedInvocations.filter((event) => event.failure_code !== null).length,
    input_tokens: completedInvocations.reduce((total, event) => total + (event.input_tokens ?? 0), 0),
    output_tokens: completedInvocations.reduce((total, event) => total + (event.output_tokens ?? 0), 0),
    tool_calls: completedInvocations.reduce((total, event) => total + (event.tool_count ?? 0), 0),
    route_digest: [...events].reverse().find((event) => event.route_digest !== null)?.route_digest ?? null,
    plan_digest: [...events].reverse().find((event) => event.plan_digest !== null)?.plan_digest ?? null,
    selection_digests: selectionDigests,
    failure_codes: failureCodes,
    retention: "metadata_only_no_prompts_responses_or_tool_payloads" as const,
    secret_material: "never_returned" as const,
  };
  return { ...body, trace_digest: digestJsonSync(body) };
}

function summaryBody(summary: AutonomousRunTraceSummary): JsonObject {
  const { trace_digest: _traceDigest, ...body } = summary;
  return body;
}

function validateSummary(raw: unknown): AutonomousRunTraceSummary {
  if (!isObject(raw)) throw new ArgumentError("autonomous run trace registry summary is malformed");
  const required = ["schema", "run_id", "task_digest", "domains", "status", "first_sequence", "last_sequence", "event_count", "provider_invocations", "provider_failures", "input_tokens", "output_tokens", "tool_calls", "route_digest", "plan_digest", "selection_digests", "failure_codes", "trace_digest", "retention", "secret_material"];
  if (Object.keys(raw).some((key) => !required.includes(key)) || required.some((key) => !(key in raw))) throw new ArgumentError("autonomous run trace registry summary fields are incomplete");
  if (raw.schema !== AUTONOMOUS_RUN_TRACE_SCHEMA || raw.retention !== "metadata_only_no_prompts_responses_or_tool_payloads" || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous run trace registry summary retention is invalid");
  const runId = identifier("autonomous run trace registry summary run_id", raw.run_id);
  const taskDigest = digest("autonomous run trace registry summary task_digest", raw.task_digest);
  if (!Array.isArray(raw.domains) || raw.domains.length < 1 || raw.domains.some((value) => !TRACE_DOMAINS.includes(value as AutonomousDomainName)) || new Set(raw.domains).size !== raw.domains.length) throw new ArgumentError("autonomous run trace registry summary domains are invalid");
  if (!AUTONOMOUS_RUN_TRACE_STATUSES.includes(raw.status as AutonomousRunTraceStatus)) throw new ArgumentError("autonomous run trace registry summary status is invalid");
  for (const key of ["first_sequence", "last_sequence"]) boundedCount(`autonomous run trace registry summary ${key}`, raw[key], MAX_AUTONOMOUS_RUN_TRACE_EVENTS);
  for (const key of ["event_count", "provider_invocations", "provider_failures"]) boundedCount(`autonomous run trace registry summary ${key}`, raw[key], MAX_AUTONOMOUS_RUN_TRACE_EVENTS);
  for (const key of ["input_tokens", "output_tokens", "tool_calls"]) boundedCount(`autonomous run trace registry summary ${key}`, raw[key], MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_COUNTER);
  if (raw.first_sequence === null || raw.last_sequence === null || (raw.first_sequence as number) > (raw.last_sequence as number)) throw new ArgumentError("autonomous run trace registry summary sequence range is invalid");
  if ((raw.provider_failures as number) > (raw.provider_invocations as number)) throw new ArgumentError("autonomous run trace registry summary provider failure count is invalid");
  if (raw.route_digest !== null) digest("autonomous run trace registry summary route_digest", raw.route_digest);
  if (raw.plan_digest !== null) digest("autonomous run trace registry summary plan_digest", raw.plan_digest);
  const selectionDigests = raw.selection_digests as unknown[];
  const failureCodes = raw.failure_codes as unknown[];
  if (!Array.isArray(selectionDigests) || !Array.isArray(failureCodes)) throw new ArgumentError("autonomous run trace registry summary arrays are malformed");
  for (const value of selectionDigests) digest("autonomous run trace registry summary selection_digest", value);
  for (const value of failureCodes) boundedText("autonomous run trace registry summary failure_code", value);
  stringList("autonomous run trace registry summary selection_digests", selectionDigests);
  stringList("autonomous run trace registry summary failure_codes", failureCodes);
  const supplied = digest("autonomous run trace registry summary trace_digest", raw.trace_digest);
  const normalized = {
    schema: AUTONOMOUS_RUN_TRACE_SCHEMA,
    run_id: runId,
    task_digest: taskDigest,
    domains: [...raw.domains].sort() as AutonomousDomainName[],
    status: raw.status as AutonomousRunTraceStatus,
    first_sequence: raw.first_sequence as number,
    last_sequence: raw.last_sequence as number,
    event_count: raw.event_count as number,
    provider_invocations: raw.provider_invocations as number,
    provider_failures: raw.provider_failures as number,
    input_tokens: raw.input_tokens as number,
    output_tokens: raw.output_tokens as number,
    tool_calls: raw.tool_calls as number,
    route_digest: raw.route_digest as string | null,
    plan_digest: raw.plan_digest as string | null,
    selection_digests: [...selectionDigests] as string[],
    failure_codes: [...failureCodes] as string[],
    retention: "metadata_only_no_prompts_responses_or_tool_payloads" as const,
    secret_material: "never_returned" as const,
  };
  if (digestJsonSync(normalized) !== supplied) throw new ArgumentError("autonomous run trace registry summary digest is invalid");
  return { ...normalized, trace_digest: supplied };
}

type AutonomousRunTraceRegistryRecordBody = {
  schema: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA;
  run_id: string;
  summary: AutonomousRunTraceSummary;
  providers: readonly string[];
  models: readonly string[];
  source_snapshot_digest: string;
  source_sequence: number;
  source_head_digest: string;
  events: readonly AutonomousRunTraceEvent[];
  retained_event_count: number;
  retention: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION;
  authority: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY;
  secret_material: typeof AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL;
};

function recordBody(record: AutonomousRunTraceRegistryRecordBody): JsonObject {
  return {
    schema: record.schema,
    run_id: record.run_id,
    summary: { ...record.summary, selection_digests: [...record.summary.selection_digests], failure_codes: [...record.summary.failure_codes] },
    providers: [...record.providers],
    models: [...record.models],
    source_snapshot_digest: record.source_snapshot_digest,
    source_sequence: record.source_sequence,
    source_head_digest: record.source_head_digest,
    events: record.events.map((event) => structuredClone(event)),
    retained_event_count: record.retained_event_count,
    retention: record.retention,
    authority: record.authority,
    secret_material: record.secret_material,
  };
}

function validateRecord(raw: unknown, policy: AutonomousRunTraceRetentionPolicy): AutonomousRunTraceRegistryRecord {
  if (!isObject(raw)) throw new ArgumentError("autonomous run trace registry record is malformed");
  const allowed = ["schema", "run_id", "summary", "providers", "models", "source_snapshot_digest", "source_sequence", "source_head_digest", "events", "retained_event_count", "record_digest", "retention", "authority", "secret_material"];
  if (Object.keys(raw).some((key) => !allowed.includes(key)) || allowed.some((key) => !(key in raw))) throw new ArgumentError("autonomous run trace registry record fields are incomplete");
  if (raw.schema !== AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA || raw.retention !== AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION || raw.authority !== AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY || raw.secret_material !== AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL) throw new ArgumentError("autonomous run trace registry record retention is invalid");
  const runId = identifier("autonomous run trace registry run_id", raw.run_id);
  const summary = validateSummary(raw.summary);
  if (summary.run_id !== runId) throw new ArgumentError("autonomous run trace registry record summary identity does not match");
  const providers = stringList("autonomous run trace registry providers", raw.providers);
  const models = stringList("autonomous run trace registry models", raw.models);
  const sourceSnapshotDigest = digest("autonomous run trace registry source_snapshot_digest", raw.source_snapshot_digest);
  const sourceSequence = boundedCount("autonomous run trace registry source_sequence", raw.source_sequence, MAX_AUTONOMOUS_RUN_TRACE_EVENTS);
  const sourceHeadDigest = digest("autonomous run trace registry source_head_digest", raw.source_head_digest);
  if (sourceSequence < (summary.last_sequence ?? 0)) throw new ArgumentError("autonomous run trace registry source sequence predates the run");
  if (!Array.isArray(raw.events) || raw.events.length > policy.max_events) throw new ArgumentError("autonomous run trace registry retained events exceed policy");
  const events = raw.events.map((event) => validateAutonomousRunTraceEvent(event));
  if (policy.retain_events && events.length !== summary.event_count) throw new ArgumentError("autonomous run trace registry retained event count does not match the summary");
  if (!policy.retain_events && events.length !== 0) throw new ArgumentError("autonomous run trace registry policy forbids retained events");
  for (const event of events) if (event.run_id !== runId || event.task_digest !== summary.task_digest) throw new ArgumentError("autonomous run trace registry event identity does not match");
  if (policy.retain_events) {
    const recomputed = summaryFromEvents(runId, events);
    if (canonicalJson(recomputed) !== canonicalJson(summary)) throw new ArgumentError("autonomous run trace registry summary does not match retained events");
  }
  const retainedEventCount = boundedCount("autonomous run trace registry retained_event_count", raw.retained_event_count, policy.max_events);
  if (retainedEventCount !== events.length) throw new ArgumentError("autonomous run trace registry retained event count is inconsistent");
  const suppliedRecordDigest = digest("autonomous run trace registry record_digest", raw.record_digest);
  const body = {
    schema: AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA,
    run_id: runId,
    summary,
    providers,
    models,
    source_snapshot_digest: sourceSnapshotDigest,
    source_sequence: sourceSequence,
    source_head_digest: sourceHeadDigest,
    events,
    retained_event_count: retainedEventCount,
    retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
    authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
    secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
  } as AutonomousRunTraceRegistryRecordBody;
  if (digestJsonSync(recordBody(body)) !== suppliedRecordDigest) throw new ArgumentError("autonomous run trace registry record digest is invalid");
  return structuredClone({ ...body, record_digest: suppliedRecordDigest } as AutonomousRunTraceRegistryRecord);
}

function registrySnapshotBody(records: readonly AutonomousRunTraceRegistryRecord[], policy: AutonomousRunTraceRetentionPolicy, generation: number, previous: string | null): Omit<AutonomousRunTraceRegistrySnapshot, "snapshot_digest"> {
  return {
    schema: AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA,
    snapshot_generation: generation,
    previous_snapshot_digest: previous,
    policy: { ...policy },
    record_count: records.length,
    event_count: records.reduce((total, record) => total + record.summary.event_count, 0),
    retained_event_count: records.reduce((total, record) => total + record.retained_event_count, 0),
    records: sortRecords(records).map(cloneRecord),
    retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
    authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
    secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
  };
}

function validateRegistrySnapshot(raw: unknown, maximumBytes: number): AutonomousRunTraceRegistrySnapshot {
  if (!isObject(raw) || !Array.isArray(raw.records)) throw new ArgumentError("autonomous run trace registry snapshot is malformed");
  const allowed = ["schema", "snapshot_generation", "previous_snapshot_digest", "policy", "record_count", "event_count", "retained_event_count", "records", "snapshot_digest", "retention", "authority", "secret_material"];
  if (Object.keys(raw).some((key) => !allowed.includes(key)) || allowed.some((key) => !(key in raw))) throw new ArgumentError("autonomous run trace registry snapshot fields are incomplete");
  if (raw.schema !== AUTONOMOUS_RUN_TRACE_REGISTRY_SNAPSHOT_SCHEMA || raw.retention !== AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION || raw.authority !== AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY || raw.secret_material !== AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL) throw new ArgumentError("autonomous run trace registry snapshot retention is invalid");
  if (typeof raw.snapshot_generation !== "number" || !Number.isSafeInteger(raw.snapshot_generation) || raw.snapshot_generation < 1) throw new ArgumentError("autonomous run trace registry snapshot generation is invalid");
  if (raw.previous_snapshot_digest !== null) digest("autonomous run trace registry previous_snapshot_digest", raw.previous_snapshot_digest);
  if ((raw.snapshot_generation === 1) !== (raw.previous_snapshot_digest === null)) throw new ArgumentError("autonomous run trace registry snapshot lineage is inconsistent");
  const policy = normalizePolicy(raw.policy as AutonomousRunTraceRetentionPolicy);
  if (raw.records.length > policy.max_runs) throw new ArgumentError("autonomous run trace registry records exceed policy");
  const records = raw.records.map((record) => validateRecord(record, policy));
  if (new Set(records.map((record) => record.run_id)).size !== records.length) throw new ArgumentError("autonomous run trace registry contains duplicate run ids");
  const sorted = sortRecords(records);
  if (canonicalJson(sorted) !== canonicalJson(records)) throw new ArgumentError("autonomous run trace registry records are not deterministically ordered");
  if (raw.record_count !== records.length || raw.event_count !== records.reduce((total, record) => total + record.summary.event_count, 0) || raw.retained_event_count !== records.reduce((total, record) => total + record.retained_event_count, 0)) throw new ArgumentError("autonomous run trace registry snapshot counts are inconsistent");
  const body = registrySnapshotBody(records, policy, raw.snapshot_generation, raw.previous_snapshot_digest as string | null);
  const supplied = digest("autonomous run trace registry snapshot_digest", raw.snapshot_digest);
  if (digestJsonSync(body) !== supplied) throw new ArgumentError("autonomous run trace registry snapshot digest is invalid");
  const snapshot = { ...body, snapshot_digest: supplied } as AutonomousRunTraceRegistrySnapshot;
  if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > maximumBytes) throw new ArgumentError("autonomous run trace registry snapshot exceeds its byte capacity");
  return structuredClone(snapshot);
}

function incomplete(status: AutonomousRunTraceStatus): boolean {
  return status === "running" || status === "partial" || status === "paused" || status === "unknown";
}

function buildRecord(snapshot: AutonomousRunTraceSnapshot, runId: string, events: readonly AutonomousRunTraceEvent[], policy: AutonomousRunTraceRetentionPolicy): AutonomousRunTraceRegistryRecord {
  const summary = summaryFromEvents(runId, events);
  const providers = [...new Set(events.map((event) => event.provider).filter((value): value is string => value !== null))].sort();
  const models = [...new Set(events.map((event) => event.model).filter((value): value is string => value !== null))].sort();
  const body = {
    schema: AUTONOMOUS_RUN_TRACE_REGISTRY_SCHEMA,
    run_id: runId,
    summary,
    providers,
    models,
    source_snapshot_digest: snapshot.snapshot_digest,
    source_sequence: snapshot.sequence,
    source_head_digest: snapshot.head_digest || digestJsonSync({ snapshot: snapshot.snapshot_digest }),
    events: policy.retain_events ? events.map((event) => structuredClone(event)) : [],
    retained_event_count: policy.retain_events ? events.length : 0,
    retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
    authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
    secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
  } as AutonomousRunTraceRegistryRecordBody;
  if (body.retained_event_count > policy.max_events) throw new ArgumentError(`autonomous run trace registry run ${runId} exceeds max_events`);
  return { ...body, record_digest: digestJsonSync(recordBody(body)) } as AutonomousRunTraceRegistryRecord;
}

/** Deterministic metadata registry with bounded retention and restart-safe CAS persistence. */
export class AutonomousRunTraceRegistry {
  readonly policy: AutonomousRunTraceRetentionPolicy;
  private recordsValue = new Map<string, AutonomousRunTraceRegistryRecord>();
  private snapshotGeneration = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: AutonomousRunTraceRegistrySnapshot | null = null;
  private cachedSignature: string | null = null;

  constructor(policy: AutonomousRunTraceRetentionPolicyInput = {}) {
    this.policy = normalizePolicy(policy);
  }

  get size(): number { return this.recordsValue.size; }

  get(runId: string): AutonomousRunTraceRegistryRecord | null {
    const record = this.recordsValue.get(identifier("autonomous run trace registry query run_id", runId));
    return record ? cloneRecord(record) : null;
  }

  query(query: AutonomousRunTraceRegistryQuery = {}): AutonomousRunTraceRegistryPage {
    if (!isObject(query)) throw new ArgumentError("autonomous run trace registry query must be an object");
    const normalizedQuery = query as AutonomousRunTraceRegistryQuery;
    const limit = boundedLimit("autonomous run trace registry query limit", normalizedQuery.limit ?? 256, 10_000);
    if (normalizedQuery.run_id !== undefined) identifier("autonomous run trace registry query run_id", normalizedQuery.run_id);
    if (normalizedQuery.after_run_id !== undefined) identifier("autonomous run trace registry query after_run_id", normalizedQuery.after_run_id);
    if (normalizedQuery.domain !== undefined && !TRACE_DOMAINS.includes(normalizedQuery.domain)) throw new ArgumentError("autonomous run trace registry query domain is unsupported");
    if (normalizedQuery.status !== undefined && !AUTONOMOUS_RUN_TRACE_STATUSES.includes(normalizedQuery.status)) throw new ArgumentError("autonomous run trace registry query status is unsupported");
    if (normalizedQuery.provider !== undefined) boundedText("autonomous run trace registry query provider", normalizedQuery.provider);
    if (normalizedQuery.model !== undefined) boundedText("autonomous run trace registry query model", normalizedQuery.model);
    const sorted = sortRecords([...this.recordsValue.values()]);
    let afterIndex = -1;
    if (normalizedQuery.after_run_id !== undefined) {
      afterIndex = sorted.findIndex((record) => record.run_id === normalizedQuery.after_run_id);
      if (afterIndex < 0) throw new ArgumentError("autonomous run trace registry query cursor is stale or unknown");
    }
    const matches = sorted.slice(afterIndex + 1).filter((record) =>
      (normalizedQuery.run_id === undefined || record.run_id === normalizedQuery.run_id) &&
      (normalizedQuery.domain === undefined || record.summary.domains.includes(normalizedQuery.domain)) &&
      (normalizedQuery.status === undefined || record.summary.status === normalizedQuery.status) &&
      (normalizedQuery.provider === undefined || record.providers.includes(normalizedQuery.provider)) &&
      (normalizedQuery.model === undefined || record.models.includes(normalizedQuery.model)));
    const records = matches.slice(0, limit).map(cloneRecord);
    return {
      records,
      next_after_run_id: matches.length > records.length ? records.at(-1)!.run_id : null,
      total_matches: matches.length,
      retained_event_count: records.reduce((total, record) => total + record.retained_event_count, 0),
      retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
      authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
      secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
    };
  }

  events(query: AutonomousRunTraceRegistryEventQuery = {}): AutonomousRunTraceEvent[] {
    if (!isObject(query)) throw new ArgumentError("autonomous run trace registry event query must be an object");
    const normalizedQuery = query as AutonomousRunTraceRegistryEventQuery;
    const after = normalizedQuery.after_sequence ?? 0;
    if (typeof after !== "number" || !Number.isSafeInteger(after) || after < 0) throw new ArgumentError("autonomous run trace registry event query after_sequence is invalid");
    const limit = boundedLimit("autonomous run trace registry event query limit", normalizedQuery.limit ?? 10_000, 10_000);
    if (normalizedQuery.run_id !== undefined) identifier("autonomous run trace registry event query run_id", normalizedQuery.run_id);
    if (normalizedQuery.domain !== undefined && !TRACE_DOMAINS.includes(normalizedQuery.domain)) throw new ArgumentError("autonomous run trace registry event query domain is unsupported");
    if (normalizedQuery.phase !== undefined && !AUTONOMOUS_RUN_TRACE_PHASES.includes(normalizedQuery.phase)) throw new ArgumentError("autonomous run trace registry event query phase is unsupported");
    if (normalizedQuery.status !== undefined && !AUTONOMOUS_RUN_TRACE_STATUSES.includes(normalizedQuery.status)) throw new ArgumentError("autonomous run trace registry event query status is unsupported");
    if (normalizedQuery.provider !== undefined) boundedText("autonomous run trace registry event query provider", normalizedQuery.provider);
    if (normalizedQuery.model !== undefined) boundedText("autonomous run trace registry event query model", normalizedQuery.model);
    return sortRecords([...this.recordsValue.values()]).flatMap((record) => record.events)
      .filter((event) => event.sequence > after)
      .filter((event) => normalizedQuery.run_id === undefined || event.run_id === normalizedQuery.run_id)
      .filter((event) => normalizedQuery.domain === undefined || event.domains.includes(normalizedQuery.domain))
      .filter((event) => normalizedQuery.phase === undefined || event.phase === normalizedQuery.phase)
      .filter((event) => normalizedQuery.status === undefined || event.status === normalizedQuery.status)
      .filter((event) => normalizedQuery.provider === undefined || event.provider === normalizedQuery.provider)
      .filter((event) => normalizedQuery.model === undefined || event.model === normalizedQuery.model)
      .sort((left, right) => left.sequence - right.sequence || left.run_id.localeCompare(right.run_id))
      .slice(0, limit)
      .map((event) => structuredClone(event));
  }

  importSnapshot(raw: unknown): AutonomousRunTraceRegistryImportReport {
    const source = validateAutonomousRunTraceSnapshot(raw);
    const grouped = new Map<string, AutonomousRunTraceEvent[]>();
    for (const event of source.events) grouped.set(event.run_id, [...(grouped.get(event.run_id) ?? []), event]);
    const next = new Map(this.recordsValue);
    const imported: string[] = [];
    const replaced: string[] = [];
    const unchanged: string[] = [];
    for (const [runId, events] of grouped) {
      const record = buildRecord(source, runId, events, this.policy);
      const current = next.get(runId);
      if (!current) { next.set(runId, record); imported.push(runId); continue; }
      if (current.record_digest === record.record_digest) { unchanged.push(runId); continue; }
      if (record.source_sequence < current.source_sequence) throw new ArgumentError(`autonomous run trace registry rejected stale run ${runId}`);
      if (record.source_sequence === current.source_sequence) throw new ArgumentError(`autonomous run trace registry rejected conflicting run ${runId}`);
      if (current.summary.task_digest !== record.summary.task_digest) throw new ArgumentError(`autonomous run trace registry rejected run identity drift for ${runId}`);
      next.set(runId, record);
      replaced.push(runId);
    }
    const evicted = this.fitRetention(next);
    this.recordsValue = next;
    this.invalidate();
    return {
      imported_run_ids: imported.sort(),
      replaced_run_ids: replaced.sort(),
      unchanged_run_ids: unchanged.sort(),
      evicted_run_ids: evicted.sort(),
      snapshot: this.snapshot(),
      retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
      authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
      secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
    };
  }

  compact(): { evicted_run_ids: string[]; snapshot: AutonomousRunTraceRegistrySnapshot } {
    const next = new Map(this.recordsValue);
    const evicted = this.fitRetention(next);
    this.recordsValue = next;
    if (evicted.length) this.invalidate();
    return { evicted_run_ids: evicted.sort(), snapshot: this.snapshot() };
  }

  snapshot(): AutonomousRunTraceRegistrySnapshot {
    const signature = sortRecords([...this.recordsValue.values()]).map((record) => record.record_digest).join(":");
    if (this.cachedSnapshot && this.cachedSignature === signature) return structuredClone(this.cachedSnapshot);
    const body = registrySnapshotBody([...this.recordsValue.values()], this.policy, this.snapshotGeneration + 1, this.snapshotGeneration === 0 ? null : this.previousSnapshotDigest);
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) } as AutonomousRunTraceRegistrySnapshot;
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > this.policy.max_bytes) throw new ArgumentError("autonomous run trace registry snapshot exceeds its byte capacity");
    this.snapshotGeneration = snapshot.snapshot_generation;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    this.cachedSnapshot = structuredClone(snapshot);
    this.cachedSignature = signature;
    return structuredClone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateRegistrySnapshot(raw, this.policy.max_bytes);
    if (canonicalJson(snapshot.policy) !== canonicalJson(this.policy)) throw new ArgumentError("autonomous run trace registry restore policy does not match the configured policy");
    this.recordsValue = new Map(snapshot.records.map((record) => [record.run_id, cloneRecord(record)]));
    this.snapshotGeneration = snapshot.snapshot_generation;
    this.previousSnapshotDigest = snapshot.snapshot_digest;
    this.cachedSnapshot = structuredClone(snapshot);
    this.cachedSignature = sortRecords(snapshot.records).map((record) => record.record_digest).join(":");
  }

  verifyIntegrity(): AutonomousRunTraceRegistryIntegrity {
    const snapshot = this.snapshot();
    validateRegistrySnapshot(snapshot, this.policy.max_bytes);
    return {
      verified: true,
      runs: snapshot.record_count,
      events: snapshot.event_count,
      retained_event_count: snapshot.retained_event_count,
      snapshot_digest: snapshot.snapshot_digest,
      retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
      authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
      secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
    };
  }

  private fitRetention(records: Map<string, AutonomousRunTraceRegistryRecord>): string[] {
    const evicted: string[] = [];
    const violates = (): boolean => {
      if (records.size > this.policy.max_runs) return true;
      const retainedEvents = [...records.values()].reduce((total, record) => total + record.retained_event_count, 0);
      if (retainedEvents > this.policy.max_events) return true;
      const body = registrySnapshotBody([...records.values()], this.policy, 1, null);
      const probe = { ...body, snapshot_digest: digestJsonSync(body) };
      return new TextEncoder().encode(canonicalJson(probe)).byteLength > this.policy.max_bytes;
    };
    while (violates()) {
      const candidate = sortRecords([...records.values()]).find((record) => !this.policy.keep_incomplete || !incomplete(record.summary.status));
      if (!candidate) throw new ArgumentError("autonomous run trace registry retention cannot evict an eligible terminal run");
      records.delete(candidate.run_id);
      evicted.push(candidate.run_id);
    }
    return evicted;
  }

  private invalidate(): void {
    this.cachedSnapshot = null;
    this.cachedSignature = null;
  }
}

function publicationErrorClass(error: unknown): string {
  const name = error instanceof Error ? error.constructor.name : "";
  return /^[A-Za-z0-9_.:-]{1,128}$/.test(name) ? name : "AutonomousRunTraceRegistryPublicationError";
}

/**
 * Publish a source trace snapshot as a best-effort metadata projection.
 *
 * Publication is deliberately isolated from execution: a registry capacity, persistence, or
 * source-journal error becomes a bounded report instead of causing a provider retry after an
 * external effect may already have happened. Callers can alert on ``status === failed`` while
 * preserving the original run outcome and reconciliation boundary.
 */
export async function publishAutonomousRunTraceRegistrySnapshot(
  registry: AutonomousRunTraceRegistry,
  traceStore: AutonomousRunTraceStore,
  runId: string,
): Promise<AutonomousRunTraceRegistryPublication> {
  const normalizedRunId = identifier("autonomous run trace registry publication run_id", runId);
  const base = {
    schema: AUTONOMOUS_RUN_TRACE_REGISTRY_PUBLICATION_SCHEMA,
    run_id: normalizedRunId,
    run_import_state: "unknown" as const,
    source_snapshot_digest: null,
    registry_snapshot_digest: null,
    evicted_run_count: 0,
    retention: AUTONOMOUS_RUN_TRACE_REGISTRY_RETENTION,
    authority: AUTONOMOUS_RUN_TRACE_REGISTRY_AUTHORITY,
    secret_material: AUTONOMOUS_RUN_TRACE_REGISTRY_SECRET_MATERIAL,
  };
  let sourceSnapshotDigest: string | null = null;
  try {
    if (!(registry instanceof AutonomousRunTraceRegistry)) throw new ArgumentError("autonomous run trace registry publication requires a registry");
    if (!traceStore || typeof traceStore.snapshot !== "function") throw new ArgumentError("autonomous run trace registry publication requires a trace store");
    const source = await traceStore.snapshot();
    sourceSnapshotDigest = source.snapshot_digest;
    const report = registry.importSnapshot(source);
    const runImportState = report.imported_run_ids.includes(normalizedRunId)
      ? "imported"
      : report.replaced_run_ids.includes(normalizedRunId)
        ? "replaced"
        : report.unchanged_run_ids.includes(normalizedRunId)
          ? "unchanged"
          : "not_present";
    return {
      ...base,
      status: "published",
      run_import_state: runImportState,
      source_snapshot_digest: source.snapshot_digest,
      registry_snapshot_digest: report.snapshot.snapshot_digest,
      evicted_run_count: report.evicted_run_ids.length,
      error_class: null,
      failure_code: null,
    };
  } catch (error) {
    return {
      ...base,
      status: "failed",
      source_snapshot_digest: sourceSnapshotDigest,
      error_class: publicationErrorClass(error),
      failure_code: error instanceof ArgumentError && /trace snapshot/.test(error.message)
        ? "trace_snapshot_invalid"
        : error instanceof ArgumentError && /registry/.test(error.message)
          ? "trace_registry_rejected"
          : "trace_registry_publication_failed",
    };
  }
}

/** Validate a registry snapshot before it is handed to a caller-owned persistence adapter. */
export function validateAutonomousRunTraceRegistrySnapshot(raw: unknown, options: { maxBytes?: number } = {}): AutonomousRunTraceRegistrySnapshot {
  const maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES;
  if (!Number.isSafeInteger(maxBytes) || maxBytes < 16_000 || maxBytes > MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES) throw new ArgumentError("autonomous run trace registry validation maxBytes is outside its bounds");
  return validateRegistrySnapshot(raw, maxBytes);
}

/** Canonical JSON persistence for the registry projection. */
export class JsonAutonomousRunTraceRegistryPersistence {
  protected readonly store: AutonomousRunTraceTextStore;
  readonly maxBytes: number;

  constructor(store: AutonomousRunTraceTextStore, options: { maxBytes?: number } = {}) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("autonomous run trace registry JSON persistence requires a text store");
    this.store = store;
    this.maxBytes = options.maxBytes ?? MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES;
    if (!Number.isSafeInteger(this.maxBytes) || this.maxBytes < 16_000 || this.maxBytes > MAX_AUTONOMOUS_RUN_TRACE_REGISTRY_BYTES) throw new ArgumentError("autonomous run trace registry persistence maxBytes is outside its bounds");
  }

  async read(): Promise<AutonomousRunTraceRegistrySnapshot | null> {
    const text = await this.store.read();
    if (text === null) return null;
    if (new TextEncoder().encode(text).byteLength > this.maxBytes) throw new ArgumentError("autonomous run trace registry JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(text); } catch { throw new ArgumentError("autonomous run trace registry JSON is invalid"); }
    if (canonicalJson(parsed) !== text) throw new ArgumentError("autonomous run trace registry JSON is not canonical");
    return validateRegistrySnapshot(parsed, this.maxBytes);
  }

  async write(snapshot: AutonomousRunTraceRegistrySnapshot): Promise<void> {
    const validated = validateRegistrySnapshot(snapshot, this.maxBytes);
    await this.store.write(canonicalJson(validated));
  }
}

/** CAS variant for cooperating local or remote registry writers. */
export class TransactionalJsonAutonomousRunTraceRegistryPersistence extends JsonAutonomousRunTraceRegistryPersistence {
  declare protected readonly store: AutonomousRunTraceTransactionalTextStore;

  constructor(store: AutonomousRunTraceTransactionalTextStore, options: { maxBytes?: number } = {}) {
    super(store, options);
    this.store = store;
    if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("autonomous run trace registry transactional persistence requires writeIfUnchanged");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousRunTraceRegistrySnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null) digest("autonomous run trace registry expected snapshot_digest", expectedSnapshotDigest);
    const validated = validateRegistrySnapshot(snapshot, this.maxBytes);
    return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

/** Serializes registry restore/flush operations and retains the CAS fence between them. */
export class AutonomousRunTraceRegistryPersistenceCoordinator {
  readonly registry: AutonomousRunTraceRegistry;
  readonly persistence: JsonAutonomousRunTraceRegistryPersistence;
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(registry: AutonomousRunTraceRegistry, persistence: JsonAutonomousRunTraceRegistryPersistence) {
    if (!registry || typeof registry.snapshot !== "function" || typeof registry.restore !== "function") throw new ArgumentError("autonomous run trace registry persistence requires a registry");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous run trace registry persistence adapter is malformed");
    this.registry = registry;
    this.persistence = persistence;
  }

  async restore(): Promise<AutonomousRunTraceRegistrySnapshot | null> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) { this.expectedSnapshotDigest = null; return null; }
      this.registry.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return structuredClone(snapshot);
    });
  }

  async flush(): Promise<AutonomousRunTraceRegistrySnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.registry.snapshot();
      const transactional = this.persistence as JsonAutonomousRunTraceRegistryPersistence & { writeIfUnchanged?: (expected: string | null, value: AutonomousRunTraceRegistrySnapshot) => Promise<boolean> | boolean };
      if (typeof transactional.writeIfUnchanged === "function") {
        if (!await transactional.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("autonomous run trace registry persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(operation);
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}
