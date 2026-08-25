import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous-domains.js";
import {
  AUTONOMOUS_RUN_TRACE_PHASES,
  AUTONOMOUS_RUN_TRACE_STATUSES,
  validateAutonomousRunTraceSnapshot,
  type AutonomousRunTraceEvent,
  type AutonomousRunTraceSnapshot,
} from "./autonomous-run-trace.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Conservative, value-free aggregation over a verified autonomous trace snapshot. */
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA = "bioprism-typescript-autonomous-run-trace-analytics/0.1" as const;
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION = "metadata_only_no_prompts_responses_tool_payloads_or_cost_claims" as const;
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY = "verified_trace_aggregation_only;not_task_correctness_or_external_health" as const;
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES = ["unmeasured", "observed", "degraded", "attention_required"] as const;
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES = ["measured", "unmeasured"] as const;
export const AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES = ["info", "warning", "critical"] as const;
export const MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_RUNS = 10_000;
export const MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_EVENTS = 100_000;
export const MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ROWS = 512;
export const MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS = 10_000;
export const MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES = 20_000_000;

export type AutonomousRunTraceAnalyticsStatus = typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES[number];
export type AutonomousRunTraceAnalyticsMeasurementState = typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES[number];
export type AutonomousRunTraceAnalyticsSeverity = typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES[number];

export interface AutonomousRunTraceAnalyticsPolicy extends JsonObject {
  expected_domains: AutonomousDomainName[];
  failure_rate_warning: number;
  failure_rate_critical: number;
  p95_latency_warning_ms: number | null;
  p95_latency_critical_ms: number | null;
  warn_on_incomplete_runs: boolean;
  warn_on_unmeasured_domains: boolean;
}

export interface AutonomousRunTraceAnalyticsDimension extends JsonObject {
  kind: "domain" | "provider" | "model";
  identity: string;
  expected: boolean;
  observed: boolean;
  measurement_state: AutonomousRunTraceAnalyticsMeasurementState;
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
  latency_p50_ms: number | null;
  latency_p95_ms: number | null;
  input_token_observation_count: number;
  output_token_observation_count: number;
  input_tokens: number;
  output_tokens: number;
  tool_calls: number;
  failure_codes: string[];
}

export interface AutonomousRunTraceAnalyticsAlert extends JsonObject {
  code: string;
  severity: AutonomousRunTraceAnalyticsSeverity;
  scope: string;
  identity: string;
  detail: string;
  observed_value: number | null;
  threshold: number | null;
}

export interface AutonomousRunTraceAnalyticsReport extends JsonObject {
  schema: typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA;
  source_snapshot_digest: string;
  policy_digest: string;
  status: AutonomousRunTraceAnalyticsStatus;
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
  latency_p50_ms: number | null;
  latency_p95_ms: number | null;
  first_recorded_at: number | null;
  last_recorded_at: number | null;
  status_counts: Record<string, number>;
  phase_counts: Record<string, number>;
  domains: AutonomousRunTraceAnalyticsDimension[];
  providers: AutonomousRunTraceAnalyticsDimension[];
  models: AutonomousRunTraceAnalyticsDimension[];
  alerts: AutonomousRunTraceAnalyticsAlert[];
  unattributed_provider_events: number;
  unattributed_model_events: number;
  cost_posture: "not_measured_by_trace";
  authority: typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY;
  retention: typeof AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION;
  secret_material: "never_returned";
  report_digest: string;
}

const terminalStatuses = new Set(["completed", "partial", "paused", "refused", "failed"]);
const secretMarkers = new Set([
  "task", "prompt", "response", "messages", "credential", "credentials", "secret", "token", "apikey",
  "authorization", "arguments", "argument", "payload", "output", "sourcevalue", "rawvalue", "cost", "price",
]);

function text(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || value.length === 0 || new TextEncoder().encode(value).byteLength > maximum || value.includes("\u0000")) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const result = text(name, value);
  if (!/^[A-Za-z0-9_.:/-]+$/.test(result)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return result;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new ArgumentError(`${name} must be a non-negative safe integer`);
  return value;
}

function ratio(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be finite and within [0, 1]`);
  return value;
}

function latencyThreshold(name: string, value: unknown): number | null {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0 || value > 86_400_000) throw new ArgumentError(`${name} must be a finite positive millisecond threshold or null`);
  return value;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 20) throw new ArgumentError("autonomous trace analytics metadata is too deeply nested");
  if (Array.isArray(value)) { for (const child of value) safeMetadata(child, depth + 1); return; }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (secretMarkers.has(normalized)) throw new ArgumentError("autonomous trace analytics contains transient, secret, or cost-shaped metadata");
      safeMetadata(child, depth + 1);
    }
  } else if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError("autonomous trace analytics contains a non-finite number");
}

function policy(value?: Partial<AutonomousRunTraceAnalyticsPolicy>): AutonomousRunTraceAnalyticsPolicy {
  const selected = value ?? {};
  const expected = selected.expected_domains ?? [...AUTONOMOUS_DOMAIN_NAMES];
  if (!Array.isArray(expected) || expected.length < 1 || expected.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("autonomous trace analytics expected_domains is outside its bounds");
  const expectedDomains = [...new Set(expected)];
  if (expectedDomains.length !== expected.length || expectedDomains.some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName))) throw new ArgumentError("autonomous trace analytics expected_domains contains an unsupported or duplicate domain");
  const orderedDomains = AUTONOMOUS_DOMAIN_NAMES.filter((domain) => expectedDomains.includes(domain));
  const failureWarning = selected.failure_rate_warning ?? 0.25;
  const failureCritical = selected.failure_rate_critical ?? 0.5;
  const warning = ratio("failure_rate_warning", failureWarning);
  const critical = ratio("failure_rate_critical", failureCritical);
  if (warning > critical) throw new ArgumentError("failure_rate_warning cannot exceed failure_rate_critical");
  const latencyWarning = latencyThreshold("p95_latency_warning_ms", selected.p95_latency_warning_ms ?? 10_000);
  const latencyCritical = latencyThreshold("p95_latency_critical_ms", selected.p95_latency_critical_ms ?? 60_000);
  if (latencyWarning !== null && latencyCritical !== null && latencyWarning > latencyCritical) throw new ArgumentError("p95_latency_warning_ms cannot exceed p95_latency_critical_ms");
  const incomplete = selected.warn_on_incomplete_runs ?? true;
  const unmeasured = selected.warn_on_unmeasured_domains ?? false;
  if (typeof incomplete !== "boolean" || typeof unmeasured !== "boolean") throw new ArgumentError("autonomous trace analytics policy booleans must be boolean");
  return { expected_domains: orderedDomains, failure_rate_warning: warning, failure_rate_critical: critical, p95_latency_warning_ms: latencyWarning, p95_latency_critical_ms: latencyCritical, warn_on_incomplete_runs: incomplete, warn_on_unmeasured_domains: unmeasured };
}

function counts(values: readonly string[]): Record<string, number> {
  return Object.fromEntries(values.map((value) => [value, 0]));
}

function quantile(values: readonly number[], fraction: number): number | null {
  if (values.length === 0) return null;
  const ordered = [...values].sort((a, b) => a - b);
  return ordered[Math.max(0, Math.min(ordered.length - 1, Math.ceil(ordered.length * fraction) - 1))] ?? null;
}

function mean(values: readonly number[]): number | null {
  return values.length === 0 ? null : Math.round((values.reduce((sum, value) => sum + value, 0) / values.length) * 1_000_000) / 1_000_000;
}

type RunView = { task_digest: string; domains: AutonomousDomainName[]; events: AutonomousRunTraceEvent[] };

function dimension(kind: "domain" | "provider" | "model", identity: string, expected: boolean, events: readonly AutonomousRunTraceEvent[], runs: ReadonlyMap<string, RunView>): AutonomousRunTraceAnalyticsDimension {
  const runIds = [...new Set(events.map((event) => event.run_id))].sort();
  const selectedRuns = runIds.map((runId) => runs.get(runId)).filter((run): run is RunView => run !== undefined);
  const statusCounts = counts(AUTONOMOUS_RUN_TRACE_STATUSES);
  let terminal = 0;
  for (const run of selectedRuns) {
    const finalStatus = run.events.at(-1)?.status ?? "unknown";
    statusCounts[finalStatus] = (statusCounts[finalStatus] ?? 0) + 1;
    if (terminalStatuses.has(finalStatus)) terminal += 1;
  }
  const finished = events.filter((event) => event.phase === "provider_invocation_finished");
  const failures = finished.filter((event) => event.failure_code !== null || event.failure_class !== null);
  const latencies = finished.flatMap((event) => event.latency_ms === null ? [] : [event.latency_ms]);
  const inputTokens = finished.flatMap((event) => event.input_tokens === null ? [] : [event.input_tokens]);
  const outputTokens = finished.flatMap((event) => event.output_tokens === null ? [] : [event.output_tokens]);
  const observed = events.length > 0;
  return {
    kind, identity, expected, observed, measurement_state: observed ? "measured" : "unmeasured",
    run_count: selectedRuns.length, event_count: events.length, terminal_run_count: terminal,
    incomplete_run_count: selectedRuns.length - terminal, status_counts: statusCounts,
    provider_invocations: finished.length, provider_failures: failures.length,
    failure_rate: finished.length === 0 ? null : Math.round((failures.length / finished.length) * 1e12) / 1e12,
    latency_observation_count: latencies.length, latency_mean_ms: mean(latencies), latency_p50_ms: quantile(latencies, 0.5), latency_p95_ms: quantile(latencies, 0.95),
    input_token_observation_count: inputTokens.length, output_token_observation_count: outputTokens.length,
    input_tokens: inputTokens.reduce((sum, value) => sum + value, 0), output_tokens: outputTokens.reduce((sum, value) => sum + value, 0),
    tool_calls: finished.reduce((sum, event) => sum + (event.tool_count ?? 0), 0),
    failure_codes: [...new Set(failures.flatMap((event) => event.failure_code === null ? [] : [event.failure_code]))].sort(),
  };
}

function alertsFor(row: AutonomousRunTraceAnalyticsDimension, configured: AutonomousRunTraceAnalyticsPolicy): AutonomousRunTraceAnalyticsAlert[] {
  const alerts: AutonomousRunTraceAnalyticsAlert[] = [];
  if (row.failure_rate !== null) {
    if (row.failure_rate >= configured.failure_rate_critical) alerts.push({ code: "provider_failure_rate", severity: "critical", scope: row.kind, identity: row.identity, detail: `${row.kind} provider failure rate reached the critical threshold`, observed_value: row.failure_rate, threshold: configured.failure_rate_critical });
    else if (row.failure_rate >= configured.failure_rate_warning) alerts.push({ code: "provider_failure_rate", severity: "warning", scope: row.kind, identity: row.identity, detail: `${row.kind} provider failure rate reached the warning threshold`, observed_value: row.failure_rate, threshold: configured.failure_rate_warning });
  }
  if (row.latency_p95_ms !== null) {
    if (configured.p95_latency_critical_ms !== null && row.latency_p95_ms >= configured.p95_latency_critical_ms) alerts.push({ code: "p95_latency", severity: "critical", scope: row.kind, identity: row.identity, detail: `${row.kind} p95 latency reached the critical threshold`, observed_value: row.latency_p95_ms, threshold: configured.p95_latency_critical_ms });
    else if (configured.p95_latency_warning_ms !== null && row.latency_p95_ms >= configured.p95_latency_warning_ms) alerts.push({ code: "p95_latency", severity: "warning", scope: row.kind, identity: row.identity, detail: `${row.kind} p95 latency reached the warning threshold`, observed_value: row.latency_p95_ms, threshold: configured.p95_latency_warning_ms });
  }
  if (row.kind === "domain" && row.expected && !row.observed && configured.warn_on_unmeasured_domains) alerts.push({ code: "domain_unmeasured", severity: "info", scope: "domain", identity: row.identity, detail: "the expected domain has no trace observations", observed_value: null, threshold: null });
  if (configured.warn_on_incomplete_runs && row.incomplete_run_count > 0) alerts.push({ code: "run_not_terminal", severity: "warning", scope: row.kind, identity: row.identity, detail: `${row.kind} has runs without a terminal trace status`, observed_value: row.incomplete_run_count, threshold: null });
  return alerts;
}

function exactKeys(value: Record<string, unknown>, expected: readonly string[], name: string): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) throw new ArgumentError(`${name} contains unsupported or missing fields`);
}

function validateAlert(value: unknown): AutonomousRunTraceAnalyticsAlert {
  if (!isObject(value)) throw new ArgumentError("autonomous trace analytics alert is malformed");
  const keys = ["code", "detail", "identity", "observed_value", "scope", "severity", "threshold"] as const;
  exactKeys(value, keys, "autonomous trace analytics alert");
  identifier("autonomous trace analytics alert code", value.code);
  text("autonomous trace analytics alert detail", value.detail, 512);
  text("autonomous trace analytics alert identity", value.identity, 512);
  identifier("autonomous trace analytics alert scope", value.scope);
  if (!AUTONOMOUS_RUN_TRACE_ANALYTICS_SEVERITIES.includes(value.severity as AutonomousRunTraceAnalyticsSeverity)) throw new ArgumentError("autonomous trace analytics alert severity is invalid");
  for (const [name, item] of [["observed_value", value.observed_value], ["threshold", value.threshold]] as const) {
    if (item !== null && (typeof item !== "number" || !Number.isFinite(item) || item < 0)) throw new ArgumentError(`autonomous trace analytics alert ${name} is invalid`);
  }
  return value as unknown as AutonomousRunTraceAnalyticsAlert;
}

function validateDimension(value: unknown): AutonomousRunTraceAnalyticsDimension {
  if (!isObject(value)) throw new ArgumentError("autonomous trace analytics dimension is malformed");
  const keys = ["kind", "identity", "expected", "observed", "measurement_state", "run_count", "event_count", "terminal_run_count", "incomplete_run_count", "status_counts", "provider_invocations", "provider_failures", "failure_rate", "latency_observation_count", "latency_mean_ms", "latency_p50_ms", "latency_p95_ms", "input_token_observation_count", "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls", "failure_codes"] as const;
  exactKeys(value, keys, "autonomous trace analytics dimension");
  if (value.kind !== "domain" && value.kind !== "provider" && value.kind !== "model") throw new ArgumentError("autonomous trace analytics dimension kind is invalid");
  text("autonomous trace analytics dimension identity", value.identity, 512);
  if (typeof value.expected !== "boolean" || typeof value.observed !== "boolean" || !AUTONOMOUS_RUN_TRACE_ANALYTICS_MEASUREMENT_STATES.includes(value.measurement_state as AutonomousRunTraceAnalyticsMeasurementState)) throw new ArgumentError("autonomous trace analytics dimension state is invalid");
  if (value.observed !== (value.measurement_state === "measured")) throw new ArgumentError("autonomous trace analytics dimension measurement state does not reconcile");
  for (const name of ["run_count", "event_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "latency_observation_count", "input_token_observation_count", "output_token_observation_count", "input_tokens", "output_tokens", "tool_calls"] as const) integer(name, value[name]);
  const providerInvocations = integer("provider_invocations", value.provider_invocations);
  const providerFailures = integer("provider_failures", value.provider_failures);
  const runCount = integer("run_count", value.run_count);
  const eventCount = integer("event_count", value.event_count);
  const terminalRunCount = integer("terminal_run_count", value.terminal_run_count);
  const incompleteRunCount = integer("incomplete_run_count", value.incomplete_run_count);
  if (providerFailures > providerInvocations || terminalRunCount + incompleteRunCount !== runCount || (value.observed !== (eventCount > 0))) throw new ArgumentError("autonomous trace analytics dimension counts do not reconcile");
  if (value.failure_rate === null) { if (providerInvocations !== 0) throw new ArgumentError("measured provider invocations require a failure rate"); }
  else {
    const failureRate = ratio("failure_rate", value.failure_rate);
    if (providerInvocations === 0 || Math.abs(failureRate - Math.round((providerFailures / providerInvocations) * 1e12) / 1e12) > 1e-12) throw new ArgumentError("autonomous trace analytics failure rate does not reconcile");
  }
  const latencyFields = [value.latency_mean_ms, value.latency_p50_ms, value.latency_p95_ms];
  for (const item of latencyFields) if (item !== null && (typeof item !== "number" || !Number.isFinite(item) || item < 0)) throw new ArgumentError("autonomous trace analytics latency is invalid");
  if ((integer("latency_observation_count", value.latency_observation_count) === 0) !== latencyFields.every((item) => item === null)) throw new ArgumentError("autonomous trace analytics latency observations do not reconcile");
  if (!isObject(value.status_counts) || Object.keys(value.status_counts).sort().join() !== [...AUTONOMOUS_RUN_TRACE_STATUSES].sort().join() || Object.values(value.status_counts).some((item) => typeof item !== "number" || !Number.isSafeInteger(item) || item < 0) || Object.values(value.status_counts).reduce<number>((sum, item) => sum + (item as number), 0) !== runCount) throw new ArgumentError("autonomous trace analytics status counts are malformed");
  if (!Array.isArray(value.failure_codes)) throw new ArgumentError("autonomous trace analytics failure codes are malformed");
  const failureCodes = value.failure_codes.map((item) => text("autonomous trace analytics failure code", item));
  if (failureCodes.join("\u0000") !== [...new Set(failureCodes)].sort().join("\u0000")) throw new ArgumentError("autonomous trace analytics failure codes are malformed");
  return value as unknown as AutonomousRunTraceAnalyticsDimension;
}

/** Validate a report before persistence or use as an operational observation. */
export function validateAutonomousRunTraceAnalyticsReport(raw: unknown): AutonomousRunTraceAnalyticsReport {
  if (!isObject(raw)) throw new ArgumentError("autonomous trace analytics report must be an object");
  const keys = ["schema", "source_snapshot_digest", "policy_digest", "status", "event_count", "run_count", "terminal_run_count", "incomplete_run_count", "terminal_coverage", "provider_invocations", "provider_failures", "provider_failure_rate", "input_tokens", "output_tokens", "tool_calls", "latency_observation_count", "latency_mean_ms", "latency_p50_ms", "latency_p95_ms", "first_recorded_at", "last_recorded_at", "status_counts", "phase_counts", "domains", "providers", "models", "alerts", "unattributed_provider_events", "unattributed_model_events", "cost_posture", "authority", "retention", "secret_material", "report_digest"] as const;
  exactKeys(raw, keys, "autonomous trace analytics report");
  if (raw.schema !== AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA || !AUTONOMOUS_RUN_TRACE_ANALYTICS_STATUSES.includes(raw.status as AutonomousRunTraceAnalyticsStatus)) throw new ArgumentError("autonomous trace analytics report identity is invalid");
  digest("autonomous trace analytics source snapshot digest", raw.source_snapshot_digest);
  digest("autonomous trace analytics policy digest", raw.policy_digest);
  for (const name of ["event_count", "run_count", "terminal_run_count", "incomplete_run_count", "provider_invocations", "provider_failures", "input_tokens", "output_tokens", "tool_calls", "latency_observation_count", "unattributed_provider_events", "unattributed_model_events"] as const) integer(name, raw[name]);
  const reportProviderFailures = integer("provider_failures", raw.provider_failures);
  const reportProviderInvocations = integer("provider_invocations", raw.provider_invocations);
  const reportRunCount = integer("run_count", raw.run_count);
  const reportEventCount = integer("event_count", raw.event_count);
  const reportTerminalRunCount = integer("terminal_run_count", raw.terminal_run_count);
  const reportIncompleteRunCount = integer("incomplete_run_count", raw.incomplete_run_count);
  if (reportProviderFailures > reportProviderInvocations || reportTerminalRunCount + reportIncompleteRunCount !== reportRunCount) throw new ArgumentError("autonomous trace analytics report counts do not reconcile");
  if (raw.terminal_coverage === null) { if (reportRunCount !== 0) throw new ArgumentError("measured trace runs require terminal coverage"); }
  else {
    const coverage = ratio("terminal_coverage", raw.terminal_coverage);
    if (reportRunCount === 0 || Math.abs(coverage - Math.round((reportTerminalRunCount / reportRunCount) * 1e12) / 1e12) > 1e-12) throw new ArgumentError("autonomous trace analytics terminal coverage does not reconcile");
  }
  if (raw.provider_failure_rate === null) { if (reportProviderInvocations !== 0) throw new ArgumentError("measured provider invocations require provider_failure_rate"); }
  else {
    const failureRate = ratio("provider_failure_rate", raw.provider_failure_rate);
    if (reportProviderInvocations === 0 || Math.abs(failureRate - Math.round((reportProviderFailures / reportProviderInvocations) * 1e12) / 1e12) > 1e-12) throw new ArgumentError("autonomous trace analytics provider failure rate does not reconcile");
  }
  const latencyFields = [raw.latency_mean_ms, raw.latency_p50_ms, raw.latency_p95_ms];
  for (const item of latencyFields) if (item !== null && (typeof item !== "number" || !Number.isFinite(item) || item < 0)) throw new ArgumentError("autonomous trace analytics report latency is invalid");
  if ((integer("latency_observation_count", raw.latency_observation_count) === 0) !== latencyFields.every((item) => item === null)) throw new ArgumentError("autonomous trace analytics report latency observations do not reconcile");
  if (!isObject(raw.status_counts) || Object.keys(raw.status_counts).sort().join() !== [...AUTONOMOUS_RUN_TRACE_STATUSES].sort().join() || Object.values(raw.status_counts).some((item) => typeof item !== "number" || !Number.isSafeInteger(item) || item < 0) || Object.values(raw.status_counts).reduce<number>((sum, item) => sum + (item as number), 0) !== reportRunCount) throw new ArgumentError("autonomous trace analytics status counts are malformed");
  if (!isObject(raw.phase_counts) || Object.keys(raw.phase_counts).sort().join() !== [...AUTONOMOUS_RUN_TRACE_PHASES].sort().join() || Object.values(raw.phase_counts).some((item) => typeof item !== "number" || !Number.isSafeInteger(item) || item < 0) || Object.values(raw.phase_counts).reduce<number>((sum, item) => sum + (item as number), 0) !== reportEventCount) throw new ArgumentError("autonomous trace analytics phase counts are malformed");
  for (const [name, kind] of [["domains", "domain"], ["providers", "provider"], ["models", "model"]] as const) {
    if (!Array.isArray(raw[name]) || raw[name].length > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ROWS) throw new ArgumentError(`autonomous trace analytics ${name} are malformed`);
    const rows = raw[name].map(validateDimension);
    if (rows.some((row) => row.kind !== kind) || new Set(rows.map((row) => row.identity)).size !== rows.length) throw new ArgumentError(`autonomous trace analytics ${name} contain duplicate or mismatched rows`);
  }
  if (!Array.isArray(raw.alerts) || raw.alerts.length > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS) throw new ArgumentError("autonomous trace analytics alerts are malformed");
  const alerts = raw.alerts.map(validateAlert);
  const firstRecordedAt = raw.first_recorded_at;
  const lastRecordedAt = raw.last_recorded_at;
  for (const [name, item] of [["first_recorded_at", firstRecordedAt], ["last_recorded_at", lastRecordedAt]] as const) if (item !== null) integer(name, item);
  if (raw.cost_posture !== "not_measured_by_trace" || raw.authority !== AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY || raw.retention !== AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous trace analytics retention or authority is invalid");
  if ((firstRecordedAt === null) !== (reportEventCount === 0) || (lastRecordedAt === null) !== (reportEventCount === 0) || (typeof firstRecordedAt === "number" && typeof lastRecordedAt === "number" && firstRecordedAt > lastRecordedAt)) throw new ArgumentError("autonomous trace analytics recorded-at bounds do not reconcile");
  const expectedStatus: AutonomousRunTraceAnalyticsStatus = reportEventCount === 0 ? "unmeasured" : alerts.some((alert) => alert.severity === "critical") ? "attention_required" : alerts.some((alert) => alert.severity === "warning") ? "degraded" : "observed";
  if (raw.status !== expectedStatus) throw new ArgumentError("autonomous trace analytics status does not reconcile with alerts");
  const { report_digest: supplied, ...body } = raw as unknown as Record<string, unknown>;
  digest("autonomous trace analytics report digest", supplied);
  if (digestJsonSync(body as JsonObject) !== supplied) throw new ArgumentError("autonomous trace analytics report digest is invalid");
  safeMetadata(body);
  return structuredClone(raw) as unknown as AutonomousRunTraceAnalyticsReport;
}

/** Aggregate a verified trace snapshot without treating missing measurements as zero. */
export function analyzeAutonomousRunTrace(snapshot: unknown, options: { policy?: Partial<AutonomousRunTraceAnalyticsPolicy> } = {}): AutonomousRunTraceAnalyticsReport {
  if (!isObject(options)) throw new ArgumentError("autonomous analyzeRunTrace options must be an object");
  const verified = validateAutonomousRunTraceSnapshot(snapshot);
  if (verified.events.length > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_EVENTS) throw new ArgumentError("autonomous trace analytics event capacity is exceeded");
  const configured = policy(options.policy);
  const runs = new Map<string, RunView>();
  for (const event of verified.events) {
    const prior = runs.get(event.run_id);
    if (prior === undefined) runs.set(event.run_id, { task_digest: event.task_digest, domains: [...event.domains], events: [event] });
    else {
      if (prior.task_digest !== event.task_digest) throw new ArgumentError(`autonomous trace analytics run ${event.run_id} changes task identity`);
      prior.domains = AUTONOMOUS_DOMAIN_NAMES.filter((domain) => new Set([...prior.domains, ...event.domains]).has(domain));
      prior.events.push(event);
    }
  }
  if (runs.size > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_RUNS) throw new ArgumentError("autonomous trace analytics run capacity is exceeded");
  const statuses = counts(AUTONOMOUS_RUN_TRACE_STATUSES);
  const phases = counts(AUTONOMOUS_RUN_TRACE_PHASES);
  for (const event of verified.events) phases[event.phase] = (phases[event.phase] ?? 0) + 1;
  for (const run of runs.values()) {
    const finalStatus = run.events.at(-1)!.status;
    statuses[finalStatus] = (statuses[finalStatus] ?? 0) + 1;
  }
  const terminalRunCount = [...runs.values()].filter((run) => terminalStatuses.has(run.events.at(-1)!.status)).length;
  const finished = verified.events.filter((event) => event.phase === "provider_invocation_finished");
  const failures = finished.filter((event) => event.failure_code !== null || event.failure_class !== null);
  const latencies = finished.flatMap((event) => event.latency_ms === null ? [] : [event.latency_ms]);
  const inputTokens = finished.flatMap((event) => event.input_tokens === null ? [] : [event.input_tokens]);
  const outputTokens = finished.flatMap((event) => event.output_tokens === null ? [] : [event.output_tokens]);
  const observedDomains = new Set(verified.events.flatMap((event) => event.domains));
  const domainNames = AUTONOMOUS_DOMAIN_NAMES.filter((domain) => configured.expected_domains.includes(domain) || observedDomains.has(domain));
  const domains = domainNames.map((domain) => dimension("domain", domain, configured.expected_domains.includes(domain), verified.events.filter((event) => event.domains.includes(domain)), runs));
  const providerNames = [...new Set(verified.events.flatMap((event) => event.provider === null ? [] : [event.provider]))].sort();
  const providers = providerNames.map((provider) => dimension("provider", provider, true, verified.events.filter((event) => event.provider === provider), runs));
  const modelNames = [...new Set(verified.events.flatMap((event) => event.provider === null || event.model === null ? [] : [`${event.provider}/${event.model}`]))].sort();
  const models = modelNames.map((model) => dimension("model", model, true, verified.events.filter((event) => event.provider !== null && event.model !== null && `${event.provider}/${event.model}` === model), runs));
  const alerts = [...domains, ...providers, ...models].flatMap((row) => alertsFor(row, configured));
  if (configured.warn_on_incomplete_runs) for (const [runId, run] of [...runs.entries()].sort(([left], [right]) => left.localeCompare(right))) if (!terminalStatuses.has(run.events.at(-1)!.status)) alerts.push({ code: "run_not_terminal", severity: "warning", scope: "run", identity: runId, detail: "run has no terminal trace status", observed_value: null, threshold: null });
  if ([...runs.values()].some((run) => run.events.at(-1)!.status === "unknown")) alerts.push({ code: "unknown_terminal_status", severity: "warning", scope: "trace", identity: "snapshot", detail: "at least one run ended with an unknown status", observed_value: null, threshold: null });
  const severityOrder: Record<AutonomousRunTraceAnalyticsSeverity, number> = { critical: 0, warning: 1, info: 2 };
  alerts.sort((left, right) => severityOrder[left.severity] - severityOrder[right.severity] || left.code.localeCompare(right.code) || left.scope.localeCompare(right.scope) || left.identity.localeCompare(right.identity));
  if (alerts.length > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_ALERTS) throw new ArgumentError("autonomous trace analytics alert capacity is exceeded");
  const status: AutonomousRunTraceAnalyticsStatus = verified.events.length === 0 ? "unmeasured" : alerts.some((alert) => alert.severity === "critical") ? "attention_required" : alerts.some((alert) => alert.severity === "warning") ? "degraded" : "observed";
  const body: Omit<AutonomousRunTraceAnalyticsReport, "report_digest"> = {
    schema: AUTONOMOUS_RUN_TRACE_ANALYTICS_SCHEMA, source_snapshot_digest: verified.snapshot_digest, policy_digest: digestJsonSync(configured), status,
    event_count: verified.events.length, run_count: runs.size, terminal_run_count: terminalRunCount, incomplete_run_count: runs.size - terminalRunCount,
    terminal_coverage: runs.size === 0 ? null : Math.round((terminalRunCount / runs.size) * 1e12) / 1e12,
    provider_invocations: finished.length, provider_failures: failures.length, provider_failure_rate: finished.length === 0 ? null : Math.round((failures.length / finished.length) * 1e12) / 1e12,
    input_tokens: inputTokens.reduce((sum, value) => sum + value, 0), output_tokens: outputTokens.reduce((sum, value) => sum + value, 0), tool_calls: finished.reduce((sum, event) => sum + (event.tool_count ?? 0), 0),
    latency_observation_count: latencies.length, latency_mean_ms: mean(latencies), latency_p50_ms: quantile(latencies, 0.5), latency_p95_ms: quantile(latencies, 0.95),
    first_recorded_at: verified.events.length === 0 ? null : Math.min(...verified.events.map((event) => event.recorded_at)), last_recorded_at: verified.events.length === 0 ? null : Math.max(...verified.events.map((event) => event.recorded_at)),
    status_counts: statuses, phase_counts: phases, domains, providers, models, alerts,
    unattributed_provider_events: verified.events.filter((event) => event.phase === "provider_invocation_finished" && event.provider === null).length,
    unattributed_model_events: verified.events.filter((event) => event.phase === "provider_invocation_finished" && (event.provider === null || event.model === null)).length,
    cost_posture: "not_measured_by_trace", authority: AUTONOMOUS_RUN_TRACE_ANALYTICS_AUTHORITY, retention: AUTONOMOUS_RUN_TRACE_ANALYTICS_RETENTION, secret_material: "never_returned",
  };
  safeMetadata(body);
  if (new TextEncoder().encode(canonicalJson(body)).byteLength > MAX_AUTONOMOUS_RUN_TRACE_ANALYTICS_BYTES) throw new ArgumentError("autonomous trace analytics report exceeds its byte capacity");
  return validateAutonomousRunTraceAnalyticsReport({ ...body, report_digest: digestJsonSync(body) });
}
