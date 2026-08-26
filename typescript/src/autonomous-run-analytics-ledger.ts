import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY,
  AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES,
  AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION,
  AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES,
  AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES,
  type AutonomousRunTraceAnalyticsAlert,
  type AutonomousRunTraceAnalyticsDimension,
  type AutonomousRunTraceAnalyticsPolicy,
  type AutonomousRunTraceAnalyticsReport,
  validateAutonomousRunTraceAnalyticsReport,
} from "./autonomous-run-analytics.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import { AUTONOMOUS_RUN_TRACE_STATUSES } from "./autonomous-run-trace.js";
import type { JsonObject } from "./types.js";

/** Restart-safe longitudinal storage for already-verified run analytics reports. */
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA = "bioprism-typescript-autonomous-run-analytics-ledger/0.1" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA = "bioprism-typescript-autonomous-run-analytics-ledger-entry/0.1" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA = "bioprism-typescript-autonomous-run-analytics-ledger-ingest/0.1" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA = "bioprism-typescript-autonomous-run-analytics-ledger-summary/0.1" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION = "metadata_only_validated_reports_no_prompts_responses_tool_payloads_or_cost_claims" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY = "verified_report_aggregation_only;not_task_correctness_or_external_health" as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_STATUSES = AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_STATUSES = ["accepted", "duplicate", "conflict"] as const;
export const AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE = "not_aggregated_from_report_quantiles" as const;
export const MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS = 256;
export const MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES = 512;
export const MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES = 50_000_000;
export const MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_DIMENSIONS = 512;

export type AutonomousRunAnalyticsLedgerStatus = typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_STATUSES[number];
export type AutonomousRunAnalyticsLedgerIngestStatus = typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_STATUSES[number];

export interface AutonomousRunAnalyticsLedgerPolicy extends JsonObject {
  expected_domains: AutonomousDomainName[];
  max_reports: number;
}

export interface AutonomousRunAnalyticsLedgerEntry extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA;
  report: AutonomousRunTraceAnalyticsReport;
  ingested_at: number;
  entry_digest: string;
  retention: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousRunAnalyticsLedgerIngestResult extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA;
  status: AutonomousRunAnalyticsLedgerIngestStatus;
  report_digest: string;
  source_snapshot_digest: string;
  retained_report_count: number;
  evicted_report_count: number;
  retention: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousRunAnalyticsLedgerDimension extends JsonObject {
  kind: "domain" | "provider" | "model";
  identity: string;
  expected: boolean;
  observed: boolean;
  measurement_state: "measured" | "unmeasured";
  report_count: number;
  run_count: number;
  event_count: number;
  terminal_run_count: number;
  incomplete_run_count: number;
  status_counts: Record<string, number>;
  provider_invocations: number;
  provider_failures: number;
  failure_rate: number | null;
  latency_observation_count: number;
  latency_mean_ms: number | null;
  latency_p50_ms: null;
  latency_p95_ms: null;
  latency_quantile_posture: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE;
  input_token_observation_count: number;
  output_token_observation_count: number;
  input_tokens: number;
  output_tokens: number;
  tool_calls: number;
  failure_codes: string[];
}

export interface AutonomousRunAnalyticsLedgerAlert extends JsonObject {
  code: string;
  severity: typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES[number];
  scope: string;
  identity: string;
  occurrences: number;
  last_report_digest: string;
  detail: string;
}

export interface AutonomousRunAnalyticsLedgerSummary extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA;
  status: AutonomousRunAnalyticsLedgerStatus;
  report_count: number;
  source_snapshot_count: number;
  accepted_report_count: number;
  evicted_report_count: number;
  first_ingested_at: number | null;
  last_ingested_at: number | null;
  event_count: number;
  run_count: number;
  terminal_run_count: number;
  incomplete_run_count: number;
  terminal_coverage: number | null;
  provider_invocations: number;
  provider_failures: number;
  provider_failure_rate: number | null;
  input_tokens: number;
  output_tokens: number;
  tool_calls: number;
  latency_observation_count: number;
  latency_mean_ms: number | null;
  latency_p50_ms: null;
  latency_p95_ms: null;
  latency_quantile_posture: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE;
  status_counts: Record<string, number>;
  alert_counts: Record<string, number>;
  domains: AutonomousRunAnalyticsLedgerDimension[];
  providers: AutonomousRunAnalyticsLedgerDimension[];
  models: AutonomousRunAnalyticsLedgerDimension[];
  alerts: AutonomousRunAnalyticsLedgerAlert[];
  cost_posture: "not_measured_by_trace";
  authority: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY;
  retention: typeof AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION;
  secret_material: "never_returned";
  summary_digest: string;
}

export interface AutonomousRunAnalyticsLedgerPersistence {
  read(): Promise<unknown | null> | unknown | null;
  write(snapshot: unknown): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: unknown): Promise<boolean> | boolean;
}

export interface AutonomousRunAnalyticsLedgerTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousRunAnalyticsLedgerTransactionalTextStore extends AutonomousRunAnalyticsLedgerTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

function text(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || value.length === 0 || new TextEncoder().encode(value).byteLength > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, maximum?: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || (maximum !== undefined && value > maximum)) throw new ArgumentError(`${name} must be a bounded non-negative safe integer`);
  return value;
}

function timestamp(name: string, value: unknown): number { return integer(name, value, 253_402_300_799_999); }

function exactKeys(value: Record<string, unknown>, expected: readonly string[], name: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function normalizePolicy(value?: Partial<AutonomousRunAnalyticsLedgerPolicy>): AutonomousRunAnalyticsLedgerPolicy {
  if (value !== undefined && !isObject(value)) throw new ArgumentError("analytics ledger policy must be an object");
  const selected = value ?? {};
  const expected = selected.expected_domains ?? [...AUTONOMOUS_DOMAIN_NAMES];
  if (!Array.isArray(expected) || expected.length < 1 || expected.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("analytics ledger expected_domains is outside its bounds");
  const domains = [...new Set(expected)];
  if (domains.length !== expected.length || domains.some((domain) => typeof domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName))) throw new ArgumentError("analytics ledger expected_domains contains an unsupported or duplicate domain");
  const maxReports = integer("analytics ledger max_reports", selected.max_reports ?? MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS, MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_REPORTS);
  if (maxReports < 1) throw new ArgumentError("analytics ledger max_reports must be positive");
  return { expected_domains: AUTONOMOUS_DOMAIN_NAMES.filter((domain) => domains.includes(domain)), max_reports: maxReports };
}

function entryDigest(report: AutonomousRunTraceAnalyticsReport, ingestedAt: number): string {
  return digestJsonSync({ report_digest: report.report_digest, ingested_at: ingestedAt });
}

function validateEntry(raw: unknown): AutonomousRunAnalyticsLedgerEntry {
  if (!isObject(raw)) throw new ArgumentError("analytics ledger entry is malformed");
  exactKeys(raw, ["schema", "report", "ingested_at", "entry_digest", "retention", "secret_material"], "analytics ledger entry");
  if (raw.schema !== AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA || raw.retention !== AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("analytics ledger entry markers are invalid");
  const report = validateAutonomousRunTraceAnalyticsReport(raw.report);
  const ingestedAt = timestamp("analytics ledger entry ingested_at", raw.ingested_at);
  const supplied = digest("analytics ledger entry entry_digest", raw.entry_digest);
  if (entryDigest(report, ingestedAt) !== supplied) throw new ArgumentError("analytics ledger entry digest does not match its report");
  return structuredClone(raw) as unknown as AutonomousRunAnalyticsLedgerEntry;
}

function validateDimension(row: AutonomousRunAnalyticsLedgerDimension): AutonomousRunAnalyticsLedgerDimension {
  if (row.kind !== "domain" && row.kind !== "provider" && row.kind !== "model") throw new ArgumentError("analytics ledger dimension kind is invalid");
  text("analytics ledger dimension identity", row.identity);
  if (!AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES.includes(row.measurement_state)) throw new ArgumentError("analytics ledger dimension measurement state is invalid");
  if (row.observed !== (row.measurement_state === "measured")) throw new ArgumentError("analytics ledger dimension measurement state does not reconcile");
  for (const name of ["report_count", "run_count", "event_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "latency_observation_count", "input_token_observation_count", "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls"] as const) integer(`analytics ledger dimension ${name}`, row[name]);
  if (row.terminal_run_count + row.incomplete_run_count !== row.run_count || row.provider_failures > row.provider_invocations) throw new ArgumentError("analytics ledger dimension counts do not reconcile");
  if (row.failure_rate === null) { if (row.provider_invocations !== 0) throw new ArgumentError("analytics ledger dimension failure rate cannot be null"); }
  else if (row.provider_invocations === 0 || !Number.isFinite(row.failure_rate) || row.failure_rate < 0 || row.failure_rate > 1 || Math.abs(row.failure_rate - Math.round((row.provider_failures / row.provider_invocations) * 1e12) / 1e12) > 1e-12) throw new ArgumentError("analytics ledger dimension failure rate does not reconcile");
  if (row.latency_quantile_posture !== AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE || row.latency_p50_ms !== null || row.latency_p95_ms !== null) throw new ArgumentError("analytics ledger dimension quantile posture is invalid");
  if (row.latency_mean_ms !== null && (typeof row.latency_mean_ms !== "number" || !Number.isFinite(row.latency_mean_ms) || row.latency_mean_ms < 0)) throw new ArgumentError("analytics ledger dimension latency mean is invalid");
  if ((row.latency_observation_count === 0) !== (row.latency_mean_ms === null)) throw new ArgumentError("analytics ledger dimension latency observations do not reconcile");
  if (!isObject(row.status_counts) || Object.keys(row.status_counts).sort().join() !== [...AUTONOMOUS_RUN_TRACE_STATUSES].sort().join() || Object.values(row.status_counts).some((item) => typeof item !== "number" || !Number.isSafeInteger(item) || item < 0) || Object.values(row.status_counts).reduce<number>((sum, item) => sum + (item as number), 0) !== row.run_count) throw new ArgumentError("analytics ledger dimension status counts are invalid");
  if (!Array.isArray(row.failure_codes) || row.failure_codes.map((item) => text("analytics ledger failure code", item)).join("\u0000") !== [...new Set(row.failure_codes)].sort().join("\u0000")) throw new ArgumentError("analytics ledger dimension failure codes are invalid");
  return row;
}

function aggregateDimensions(entries: readonly AutonomousRunAnalyticsLedgerEntry[], field: "domains" | "providers" | "models", kind: "domain" | "provider" | "model", expectedDomains: readonly AutonomousDomainName[]): AutonomousRunAnalyticsLedgerDimension[] {
  type Accumulator = { expected: boolean; observed: boolean; report_count: number; run_count: number; event_count: number; terminal_run_count: number; incomplete_run_count: number; status_counts: Record<string, number>; provider_invocations: number; provider_failures: number; latency_observation_count: number; latency_weighted_sum: number; input_token_observation_count: number; output_token_observation_count: number; input_tokens: number; output_tokens: number; tool_calls: number; failure_codes: Set<string> };
  const rows = new Map<string, Accumulator>();
  if (kind === "domain") for (const domain of expectedDomains) rows.set(domain, { expected: true, observed: false, report_count: 0, run_count: 0, event_count: 0, terminal_run_count: 0, incomplete_run_count: 0, status_counts: Object.fromEntries(AUTONOMOUS_RUN_TRACE_STATUSES.map((status) => [status, 0])), provider_invocations: 0, provider_failures: 0, latency_observation_count: 0, latency_weighted_sum: 0, input_token_observation_count: 0, output_token_observation_count: 0, input_tokens: 0, output_tokens: 0, tool_calls: 0, failure_codes: new Set() });
  for (const entry of entries) {
    for (const row of entry.report[field]) {
      if (row.kind !== kind) continue;
      let accumulator = rows.get(row.identity);
      if (accumulator === undefined) {
        accumulator = { expected: kind === "domain" && expectedDomains.includes(row.identity as AutonomousDomainName), observed: false, report_count: 0, run_count: 0, event_count: 0, terminal_run_count: 0, incomplete_run_count: 0, status_counts: Object.fromEntries(AUTONOMOUS_RUN_TRACE_STATUSES.map((status) => [status, 0])), provider_invocations: 0, provider_failures: 0, latency_observation_count: 0, latency_weighted_sum: 0, input_token_observation_count: 0, output_token_observation_count: 0, input_tokens: 0, output_tokens: 0, tool_calls: 0, failure_codes: new Set() };
        rows.set(row.identity, accumulator);
      }
      accumulator.report_count += 1;
      for (const name of ["run_count", "event_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "latency_observation_count", "input_token_observation_count", "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls"] as const) accumulator[name] += row[name];
      for (const [status, count] of Object.entries(row.status_counts)) accumulator.status_counts[status] = (accumulator.status_counts[status] ?? 0) + count;
      if (row.latency_mean_ms !== null) accumulator.latency_weighted_sum += row.latency_mean_ms * row.latency_observation_count;
      for (const code of row.failure_codes) accumulator.failure_codes.add(code);
      accumulator.observed ||= row.observed;
    }
  }
  return [...rows.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([identity, accumulator]) => validateDimension({
    kind, identity, expected: accumulator.expected, observed: accumulator.observed, measurement_state: accumulator.observed ? "measured" : "unmeasured", report_count: accumulator.report_count, run_count: accumulator.run_count, event_count: accumulator.event_count, terminal_run_count: accumulator.terminal_run_count, incomplete_run_count: accumulator.incomplete_run_count, status_counts: accumulator.status_counts, provider_invocations: accumulator.provider_invocations, provider_failures: accumulator.provider_failures, failure_rate: accumulator.provider_invocations === 0 ? null : Math.round((accumulator.provider_failures / accumulator.provider_invocations) * 1e12) / 1e12, latency_observation_count: accumulator.latency_observation_count, latency_mean_ms: accumulator.latency_observation_count === 0 ? null : Math.round((accumulator.latency_weighted_sum / accumulator.latency_observation_count) * 1e6) / 1e6, latency_p50_ms: null, latency_p95_ms: null, latency_quantile_posture: AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE, input_token_observation_count: accumulator.input_token_observation_count, output_token_observation_count: accumulator.output_token_observation_count, input_tokens: accumulator.input_tokens, output_tokens: accumulator.output_tokens, tool_calls: accumulator.tool_calls, failure_codes: [...accumulator.failure_codes].sort(), }));
}

function aggregateAlerts(entries: readonly AutonomousRunAnalyticsLedgerEntry[]): { counts: Record<string, number>; alerts: AutonomousRunAnalyticsLedgerAlert[] } {
  const counts = Object.fromEntries(AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES.map((severity) => [severity, 0]));
  const grouped = new Map<string, AutonomousRunAnalyticsLedgerAlert>();
  for (const entry of entries) for (const alert of entry.report.alerts) {
    counts[alert.severity] = (counts[alert.severity] ?? 0) + 1;
    const key = [alert.code, alert.severity, alert.scope, alert.identity].join("\u0000");
    const previous = grouped.get(key);
    grouped.set(key, { code: alert.code, severity: alert.severity, scope: alert.scope, identity: alert.identity, occurrences: (previous?.occurrences ?? 0) + 1, last_report_digest: entry.report.report_digest, detail: alert.detail });
  }
  const severityOrder: Record<string, number> = { critical: 0, warning: 1, info: 2 };
  return { counts, alerts: [...grouped.values()].sort((left, right) => (severityOrder[left.severity] ?? 99) - (severityOrder[right.severity] ?? 99) || left.code.localeCompare(right.code) || left.scope.localeCompare(right.scope) || left.identity.localeCompare(right.identity)) };
}

function makeSummary(entries: readonly AutonomousRunAnalyticsLedgerEntry[], configured: AutonomousRunAnalyticsLedgerPolicy, acceptedReportCount: number, evictedReportCount: number): AutonomousRunAnalyticsLedgerSummary {
  const statusCounts = Object.fromEntries(AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES.map((status) => [status, 0]));
  let eventCount = 0; let runCount = 0; let terminalRunCount = 0; let incompleteRunCount = 0; let providerInvocations = 0; let providerFailures = 0; let inputTokens = 0; let outputTokens = 0; let toolCalls = 0; let latencyObservationCount = 0; let latencyWeightedSum = 0;
  const sources = new Set<string>();
  for (const entry of entries) {
    const report = entry.report;
    sources.add(report.source_snapshot_digest); statusCounts[report.status] = (statusCounts[report.status] ?? 0) + 1; eventCount += report.event_count; runCount += report.run_count; terminalRunCount += report.terminal_run_count; incompleteRunCount += report.incomplete_run_count; providerInvocations += report.provider_invocations; providerFailures += report.provider_failures; inputTokens += report.input_tokens; outputTokens += report.output_tokens; toolCalls += report.tool_calls; latencyObservationCount += report.latency_observation_count; if (report.latency_mean_ms !== null) latencyWeightedSum += report.latency_mean_ms * report.latency_observation_count;
  }
  const alertData = aggregateAlerts(entries);
  const dimensions = { domains: aggregateDimensions(entries, "domains", "domain", configured.expected_domains), providers: aggregateDimensions(entries, "providers", "provider", configured.expected_domains), models: aggregateDimensions(entries, "models", "model", configured.expected_domains) };
  const summaryStatus: AutonomousRunAnalyticsLedgerStatus = entries.length === 0 ? "unmeasured" : (alertData.counts.critical ?? 0) > 0 ? "attention_required" : (alertData.counts.warning ?? 0) > 0 ? "degraded" : "observed";
  const descriptor = {
    schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_SUMMARY_SCHEMA, status: summaryStatus, report_count: entries.length, source_snapshot_count: sources.size, accepted_report_count: acceptedReportCount, evicted_report_count: evictedReportCount, first_ingested_at: entries.length === 0 ? null : Math.min(...entries.map((entry) => entry.ingested_at)), last_ingested_at: entries.length === 0 ? null : Math.max(...entries.map((entry) => entry.ingested_at)), event_count: eventCount, run_count: runCount, terminal_run_count: terminalRunCount, incomplete_run_count: incompleteRunCount, terminal_coverage: runCount === 0 ? null : Math.round((terminalRunCount / runCount) * 1e12) / 1e12, provider_invocations: providerInvocations, provider_failures: providerFailures, provider_failure_rate: providerInvocations === 0 ? null : Math.round((providerFailures / providerInvocations) * 1e12) / 1e12, input_tokens: inputTokens, output_tokens: outputTokens, tool_calls: toolCalls, latency_observation_count: latencyObservationCount, latency_mean_ms: latencyObservationCount === 0 ? null : Math.round((latencyWeightedSum / latencyObservationCount) * 1e6) / 1e6, latency_p50_ms: null, latency_p95_ms: null, latency_quantile_posture: AUTONOMOUS_RUN_ANALYTICS_LEDGER_QUANTILE_POSTURE, status_counts: statusCounts, alert_counts: alertData.counts, ...dimensions, alerts: alertData.alerts, cost_posture: "not_measured_by_trace" as const, authority: AUTONOMOUS_RUN_ANALYTICS_LEDGER_AUTHORITY, retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" as const,
  };
  const summary = { ...descriptor, summary_digest: digestJsonSync(descriptor) } satisfies AutonomousRunAnalyticsLedgerSummary;
  return summary;
}

function validateSnapshot(raw: unknown, policy?: AutonomousRunAnalyticsLedgerPolicy): Record<string, unknown> {
  if (!isObject(raw)) throw new ArgumentError("analytics ledger snapshot must be an object");
  exactKeys(raw, ["schema", "policy", "entries", "accepted_report_count", "evicted_report_count", "generation", "previous_snapshot_digest", "snapshot_digest", "retention", "secret_material"], "analytics ledger snapshot");
  if (raw.schema !== AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA || raw.retention !== AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("analytics ledger snapshot markers are invalid");
  const snapshotPolicy = normalizePolicy(raw.policy as Partial<AutonomousRunAnalyticsLedgerPolicy>);
  if (policy !== undefined && canonicalJson(snapshotPolicy) !== canonicalJson(policy)) throw new ArgumentError("analytics ledger snapshot policy does not match the ledger");
  if (!Array.isArray(raw.entries) || raw.entries.length > snapshotPolicy.max_reports) throw new ArgumentError("analytics ledger snapshot entries are outside their bound");
  const entries = raw.entries.map(validateEntry);
  if (new Set(entries.map((entry) => entry.report.source_snapshot_digest)).size !== entries.length || new Set(entries.map((entry) => entry.report.report_digest)).size !== entries.length) throw new ArgumentError("analytics ledger snapshot contains duplicate report identities");
  if (entries.some((entry, index) => index > 0 && `${entries[index - 1]!.ingested_at}:${entries[index - 1]!.report.report_digest}` > `${entry.ingested_at}:${entry.report.report_digest}`)) throw new ArgumentError("analytics ledger snapshot entries are not deterministically ordered");
  const accepted = integer("analytics ledger accepted_report_count", raw.accepted_report_count);
  const evicted = integer("analytics ledger evicted_report_count", raw.evicted_report_count);
  if (accepted !== entries.length + evicted) throw new ArgumentError("analytics ledger accepted count does not cover retained and evicted reports");
  const generation = integer("analytics ledger generation", raw.generation);
  if (generation < 1) throw new ArgumentError("analytics ledger generation must be positive");
  if (raw.previous_snapshot_digest !== null) digest("analytics ledger previous_snapshot_digest", raw.previous_snapshot_digest);
  if ((generation === 1) !== (raw.previous_snapshot_digest === null)) throw new ArgumentError("analytics ledger generation and previous snapshot digest are inconsistent");
  const supplied = digest("analytics ledger snapshot_digest", raw.snapshot_digest);
  const { snapshot_digest: _ignored, ...body } = raw;
  if (digestJsonSync(body as JsonObject) !== supplied) throw new ArgumentError("analytics ledger snapshot digest does not match its contents");
  if (new TextEncoder().encode(canonicalJson(raw)).byteLength > MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES) throw new ArgumentError("analytics ledger snapshot exceeds its byte capacity");
  return structuredClone(raw);
}

export class AutonomousRunAnalyticsLedger {
  readonly policy: AutonomousRunAnalyticsLedgerPolicy;
  private readonly clock: () => number;
  private entriesValue: AutonomousRunAnalyticsLedgerEntry[] = [];
  private acceptedReportCount = 0;
  private evictedReportCount = 0;
  private generation = 0;
  private previousSnapshotDigest: string | null = null;
  private cachedSnapshot: Record<string, unknown> | null = null;
  private cachedSignature: string | null = null;

  constructor(options: { policy?: Partial<AutonomousRunAnalyticsLedgerPolicy>; clock?: () => number } = {}) {
    if (!isObject(options)) throw new ArgumentError("analytics ledger options must be an object");
    this.policy = normalizePolicy(options.policy);
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") throw new ArgumentError("analytics ledger clock must be callable");
  }

  entries(): AutonomousRunAnalyticsLedgerEntry[] { return structuredClone(this.entriesValue); }

  ingest(raw: unknown, options: { ingestedAt?: number } = {}): AutonomousRunAnalyticsLedgerIngestResult {
    const report = validateAutonomousRunTraceAnalyticsReport(raw);
    const existing = this.entriesValue.find((entry) => entry.report.source_snapshot_digest === report.source_snapshot_digest);
    if (existing !== undefined) {
      if (existing.report.report_digest === report.report_digest) return { schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA, status: "duplicate", report_digest: report.report_digest, source_snapshot_digest: report.source_snapshot_digest, retained_report_count: this.entriesValue.length, evicted_report_count: this.evictedReportCount, retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" };
      return { schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA, status: "conflict", report_digest: report.report_digest, source_snapshot_digest: report.source_snapshot_digest, retained_report_count: this.entriesValue.length, evicted_report_count: this.evictedReportCount, retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" };
    }
    const stamp = options.ingestedAt === undefined ? timestamp("analytics ledger ingested_at", Math.trunc(this.clock())) : timestamp("analytics ledger ingested_at", options.ingestedAt);
    const entry: AutonomousRunAnalyticsLedgerEntry = { schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRY_SCHEMA, report, ingested_at: stamp, entry_digest: entryDigest(report, stamp), retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" };
    this.entriesValue.push(entry);
    this.entriesValue.sort((left, right) => left.ingested_at - right.ingested_at || left.report.report_digest.localeCompare(right.report.report_digest));
    this.acceptedReportCount += 1;
    while (this.entriesValue.length > this.policy.max_reports) { this.entriesValue.shift(); this.evictedReportCount += 1; }
    this.cachedSnapshot = null; this.cachedSignature = null;
    return { schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_INGEST_SCHEMA, status: "accepted", report_digest: report.report_digest, source_snapshot_digest: report.source_snapshot_digest, retained_report_count: this.entriesValue.length, evicted_report_count: this.evictedReportCount, retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" };
  }

  history(options: { limit?: number; status?: AutonomousRunAnalyticsLedgerStatus } = {}): AutonomousRunAnalyticsLedgerEntry[] {
    const limit = integer("analytics ledger history limit", options.limit ?? 100, MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_ENTRIES);
    if (limit < 1) throw new ArgumentError("analytics ledger history limit must be positive");
    if (options.status !== undefined && !AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES.includes(options.status)) throw new ArgumentError("analytics ledger history status is invalid");
    return structuredClone(this.entriesValue.filter((entry) => options.status === undefined || entry.report.status === options.status).reverse().slice(0, limit));
  }

  summary(): AutonomousRunAnalyticsLedgerSummary { return makeSummary(this.entriesValue, this.policy, this.acceptedReportCount, this.evictedReportCount); }

  snapshot(): Record<string, unknown> {
    const signature = this.entriesValue.map((entry) => entry.entry_digest).join(":");
    if (this.cachedSnapshot !== null && this.cachedSignature === signature) return structuredClone(this.cachedSnapshot);
    const body = { schema: AUTONOMOUS_RUN_ANALYTICS_LEDGER_SCHEMA, policy: this.policy, entries: this.entriesValue, accepted_report_count: this.acceptedReportCount, evicted_report_count: this.evictedReportCount, generation: this.generation + 1, previous_snapshot_digest: this.previousSnapshotDigest, retention: AUTONOMOUS_RUN_ANALYTICS_LEDGER_RETENTION, secret_material: "never_returned" as const };
    const snapshot = { ...body, snapshot_digest: digestJsonSync(body) };
    if (new TextEncoder().encode(canonicalJson(snapshot)).byteLength > MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES) throw new ArgumentError("analytics ledger snapshot exceeds its byte capacity");
    this.generation = snapshot.generation; this.previousSnapshotDigest = snapshot.snapshot_digest; this.cachedSnapshot = structuredClone(snapshot); this.cachedSignature = signature;
    return structuredClone(snapshot);
  }

  restore(raw: unknown): void {
    const snapshot = validateSnapshot(raw, this.policy);
    const entries = (snapshot.entries as unknown[]).map(validateEntry);
    this.entriesValue = entries; this.acceptedReportCount = snapshot.accepted_report_count as number; this.evictedReportCount = snapshot.evicted_report_count as number; this.generation = snapshot.generation as number; this.previousSnapshotDigest = snapshot.snapshot_digest as string; this.cachedSnapshot = structuredClone(snapshot); this.cachedSignature = entries.map((entry) => entry.entry_digest).join(":");
  }
}

export class JsonAutonomousRunAnalyticsLedgerPersistence {
  constructor(readonly store: AutonomousRunAnalyticsLedgerTextStore, readonly maxBytes = MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("analytics ledger JSON persistence requires a text store");
    integer("analytics ledger persistence maxBytes", maxBytes, MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES);
  }
  async read(): Promise<Record<string, unknown> | null> { const value = await this.store.read(); if (value === null) return null; if (new TextEncoder().encode(value).byteLength > this.maxBytes) throw new ArgumentError("analytics ledger JSON exceeds its byte bound"); let parsed: unknown; try { parsed = JSON.parse(value); } catch { throw new ArgumentError("analytics ledger JSON is invalid"); } if (canonicalJson(parsed) !== value) throw new ArgumentError("analytics ledger JSON is not canonical"); return validateSnapshot(parsed); }
  async write(snapshot: unknown): Promise<void> { const validated = validateSnapshot(snapshot); const encoded = canonicalJson(validated); if (new TextEncoder().encode(encoded).byteLength > this.maxBytes) throw new ArgumentError("analytics ledger JSON exceeds its byte bound"); await this.store.write(encoded); }
}

export class TransactionalJsonAutonomousRunAnalyticsLedgerPersistence extends JsonAutonomousRunAnalyticsLedgerPersistence {
  declare readonly store: AutonomousRunAnalyticsLedgerTransactionalTextStore;
  constructor(store: AutonomousRunAnalyticsLedgerTransactionalTextStore, maxBytes = MAX_AUTONOMOUS_RUN_ANALYTICS_LEDGER_BYTES) { super(store, maxBytes); this.store = store; if (typeof store.writeIfUnchanged !== "function") throw new ArgumentError("transactional analytics ledger persistence requires writeIfUnchanged"); }
  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: unknown): Promise<boolean> { if (expectedSnapshotDigest !== null) digest("analytics ledger expectedSnapshotDigest", expectedSnapshotDigest); return this.store.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validateSnapshot(snapshot))); }
}

export class AutonomousRunAnalyticsLedgerPersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  constructor(readonly ledger: AutonomousRunAnalyticsLedger, readonly persistence: AutonomousRunAnalyticsLedgerPersistence) {
    if (!(ledger instanceof AutonomousRunAnalyticsLedger)) throw new ArgumentError("analytics ledger coordinator requires an analytics ledger");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("analytics ledger coordinator persistence is malformed");
  }
  async restore(): Promise<Record<string, unknown> | null> { const snapshot = await this.persistence.read(); if (snapshot === null) { this.expectedSnapshotDigest = null; return null; } if (!isObject(snapshot) || typeof snapshot.snapshot_digest !== "string") throw new ArgumentError("analytics ledger persistence returned a malformed snapshot"); this.ledger.restore(snapshot); this.expectedSnapshotDigest = snapshot.snapshot_digest; return snapshot; }
  async flush(): Promise<Record<string, unknown>> { const snapshot = this.ledger.snapshot(); if (typeof this.persistence.writeIfUnchanged === "function") { if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("analytics ledger persistence compare-and-swap conflict"); } else await this.persistence.write(snapshot); this.expectedSnapshotDigest = snapshot.snapshot_digest as string; return snapshot; }
}

export function validateAutonomousRunAnalyticsLedgerSnapshot(raw: unknown): Record<string, unknown> { return validateSnapshot(raw); }
