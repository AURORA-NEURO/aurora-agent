import { ArgumentError, isObject } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES } from "./autonomous.js";
import {
  validateAutonomousLaunchPreflightReport,
  type AutonomousLaunchPreflightReport,
} from "./autonomous-launch-preflight.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Value-only caller review handoff bound to one exact all-domain launch preflight. */
export const AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA = "bioprism-typescript-autonomous-launch-admission/0.1" as const;
export const AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-launch-admission-domain/0.1" as const;
export const MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES = 256_000;
export const MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS = 512;

export type AutonomousLaunchAdmissionDecision = "approve" | "hold";
export type AutonomousLaunchAdmissionStatus = "approved" | "held";
export type AutonomousLaunchAdmissionDomainState = "approved" | "held" | "blocked" | "not_selected";

const DECISIONS: readonly AutonomousLaunchAdmissionDecision[] = ["approve", "hold"];
const DOMAIN_STATES: readonly AutonomousLaunchAdmissionDomainState[] = ["approved", "held", "blocked", "not_selected"];
const PREFLIGHT_STATES = ["blocked", "partial", "ready_for_review"] as const;
const RETENTION = "metadata_only;preflight_and_review_digests_only;runtime_values_not_retained" as const;
const EXECUTION = "admission_only;does_not_grant_provider_source_tool_effect_credential_or_learner_authority" as const;
const AUTHORITY = "caller_review_record_only;authorization_digest_is_not_verified_by_sdk" as const;
const SECRET_MATERIAL = "never_returned" as const;
const CREDENTIAL_POSTURE = "caller_owned_opaque_handles_only;none_consumed" as const;
const SECRET_KEYS = new Set([
  "apikey", "bearer", "body", "content", "credential", "credentials", "headers", "messages", "password",
  "prompt", "request", "response", "secret", "task", "token",
]);

export interface AutonomousLaunchAdmissionOptions {
  decision: AutonomousLaunchAdmissionDecision;
  approvedDomains?: readonly typeof AUTONOMOUS_DOMAIN_NAMES[number][];
  authorizationDigest?: string | null;
  reason?: string | null;
  admissionId?: string;
}

export interface AutonomousLaunchAdmissionDomain extends JsonObject {
  schema: typeof AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA;
  domain: typeof AUTONOMOUS_DOMAIN_NAMES[number];
  preflight_state: typeof PREFLIGHT_STATES[number];
  admission_state: AutonomousLaunchAdmissionDomainState;
  contract_row_digest: string;
  readiness_state: string;
  deployment_state: string;
  next_actions: string[];
  retention: typeof RETENTION;
  execution: typeof EXECUTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousLaunchAdmissionReport extends JsonObject {
  schema: typeof AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA;
  admission_id: string;
  preflight_report_digest: string;
  decision: AutonomousLaunchAdmissionDecision;
  status: AutonomousLaunchAdmissionStatus;
  authorization_digest: string | null;
  reason_digest: string | null;
  domains: AutonomousLaunchAdmissionDomain[];
  summary: {
    domain_count: number;
    selected_domain_count: number;
    approved_domain_count: number;
    held_domain_count: number;
    blocked_domain_count: number;
    not_selected_domain_count: number;
  };
  next_actions: string[];
  authority: typeof AUTHORITY;
  retention: typeof RETENTION;
  execution: typeof EXECUTION;
  credential_posture: typeof CREDENTIAL_POSTURE;
  secret_material: typeof SECRET_MATERIAL;
  admission_digest: string;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function text(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} must be a bounded non-empty string`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const result = text(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(result)) throw new ArgumentError(`${name} is not a safe identifier`);
  return result;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && value === null) return null;
  const result = text(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(result)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return result;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 10) throw new ArgumentError("launch admission metadata nesting exceeds its bound");
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError("launch admission cannot contain non-finite numbers");
    return;
  }
  if (value instanceof Uint8Array) throw new ArgumentError("launch admission cannot contain binary material");
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError("launch admission metadata sequence exceeds its bound");
    for (const child of value) safeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 512) throw new ArgumentError("launch admission metadata mapping exceeds its bound");
    for (const [key, child] of Object.entries(value)) {
      if (SECRET_KEYS.has(key.toLowerCase().replace(/[^a-z0-9]/g, ""))) throw new ArgumentError("launch admission contains transient or secret-shaped metadata");
      safeMetadata(child, depth + 1);
    }
    return;
  }
  throw new ArgumentError("launch admission metadata contains an unsupported value");
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function strings(name: string, value: unknown, maximum = 512): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bounded sequence contract`);
  return [...new Set(value.map((item, index) => text(`${name}[${index}]`, item, 1_024)))].sort();
}

function selectedDomains(value: readonly string[] | undefined): Set<string> {
  const values = value === undefined ? [...AUTONOMOUS_DOMAIN_NAMES] : [...value];
  if (values.length < 1 || values.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch admission approvedDomains must contain one to twelve domains");
  const result = new Set(values.map((domain) => text("launch admission approved domain", domain, 128)));
  if (result.size !== values.length || [...result].some((domain) => !AUTONOMOUS_DOMAIN_NAMES.includes(domain as typeof AUTONOMOUS_DOMAIN_NAMES[number]))) throw new ArgumentError("launch admission approvedDomains must contain unique built-in domains");
  return result;
}

/** Record an explicit caller decision against one exact preflight report. */
export function createAutonomousLaunchAdmission(
  preflightReport: AutonomousLaunchPreflightReport,
  options: AutonomousLaunchAdmissionOptions,
): AutonomousLaunchAdmissionReport {
  const preflight = validateAutonomousLaunchPreflightReport(preflightReport);
  if (!options || typeof options !== "object") throw new ArgumentError("launch admission options are malformed");
  if (!DECISIONS.includes(options.decision)) throw new ArgumentError("launch admission decision must be approve or hold");
  const selected = selectedDomains(options.approvedDomains);
  const admissionId = identifier("launch admission admissionId", options.admissionId ?? "autonomous-launch-admission");
  if (options.reason !== undefined && options.reason !== null) text("launch admission reason", options.reason, 4_096);
  const authorizationDigest = digest("launch admission authorizationDigest", options.authorizationDigest ?? null, true);
  if (options.decision === "approve" && authorizationDigest === null) throw new ArgumentError("launch admission approve requires an authorizationDigest");
  const domains: AutonomousLaunchAdmissionDomain[] = preflight.domains.map((row) => {
    const admissionState: AutonomousLaunchAdmissionDomainState = !selected.has(row.domain)
      ? "not_selected"
      : row.state === "blocked"
        ? "blocked"
        : options.decision === "approve" && row.state === "ready_for_review"
          ? "approved"
          : "held";
    const actions = new Set(row.next_actions);
    if (admissionState === "blocked") actions.add("resolve the blocked preflight gate before requesting launch approval");
    else if (admissionState === "held") actions.add("complete the preflight or obtain an explicit deployment decision before launch");
    else if (admissionState === "not_selected") actions.add("select this domain explicitly before dispatch");
    return {
      schema: AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA,
      domain: row.domain,
      preflight_state: row.state,
      admission_state: admissionState,
      contract_row_digest: row.contract_row_digest,
      readiness_state: row.readiness_state,
      deployment_state: row.deployment_state,
      next_actions: [...actions].sort().slice(0, 64),
      retention: RETENTION,
      execution: EXECUTION,
      secret_material: SECRET_MATERIAL,
    };
  });
  const approvedCount = domains.filter((row) => row.admission_state === "approved").length;
  const heldCount = domains.filter((row) => row.admission_state === "held").length;
  const blockedCount = domains.filter((row) => row.admission_state === "blocked").length;
  const selectedCount = domains.filter((row) => row.admission_state !== "not_selected").length;
  const status: AutonomousLaunchAdmissionStatus = options.decision === "approve" && selectedCount > 0 && approvedCount === selectedCount ? "approved" : "held";
  const body = {
    schema: AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA,
    admission_id: admissionId,
    preflight_report_digest: preflight.report_digest,
    decision: options.decision,
    status,
    authorization_digest: authorizationDigest,
    reason_digest: options.reason === undefined || options.reason === null ? null : digestJsonSync(options.reason),
    domains,
    summary: {
      domain_count: domains.length,
      selected_domain_count: selectedCount,
      approved_domain_count: approvedCount,
      held_domain_count: heldCount,
      blocked_domain_count: blockedCount,
      not_selected_domain_count: domains.length - selectedCount,
    },
    next_actions: [...new Set([
      ...preflight.next_actions,
      ...domains.flatMap((row) => row.admission_state === "approved" ? [] : row.next_actions),
    ])].sort().slice(0, MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS),
    authority: AUTHORITY,
    retention: RETENTION,
    execution: EXECUTION,
    credential_posture: CREDENTIAL_POSTURE,
    secret_material: SECRET_MATERIAL,
  };
  safeMetadata(body);
  const report = { ...body, admission_digest: digestJsonSync(body) } as AutonomousLaunchAdmissionReport;
  if (bytes(JSON.stringify(report)) > MAX_AUTONOMOUS_LAUNCH_ADMISSION_BYTES) throw new ArgumentError("launch admission report exceeds its bounded size");
  return clone(report);
}

/** Validate an admission before binding it to a deployment-owned execution controller. */
export function validateAutonomousLaunchAdmission(value: unknown): AutonomousLaunchAdmissionReport {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LAUNCH_ADMISSION_SCHEMA) throw new ArgumentError("launch admission report is malformed");
  safeMetadata(value);
  const expected = new Set(["schema", "admission_id", "preflight_report_digest", "decision", "status", "authorization_digest", "reason_digest", "domains", "summary", "next_actions", "authority", "retention", "execution", "credential_posture", "secret_material", "admission_digest"]);
  if (Object.keys(value).length !== expected.size || Object.keys(value).some((key) => !expected.has(key))) throw new ArgumentError("launch admission report contains unsupported or missing fields");
  if (value.authority !== AUTHORITY || value.retention !== RETENTION || value.execution !== EXECUTION || value.credential_posture !== CREDENTIAL_POSTURE || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("launch admission report execution posture is unsafe");
  const supplied = digest("launch admission admission_digest", value.admission_digest) as string;
  const { admission_digest: _digest, ...withoutDigest } = value;
  if (digestJsonSync(withoutDigest) !== supplied) throw new ArgumentError("launch admission admission_digest does not match its metadata");
  identifier("launch admission admissionId", value.admission_id);
  digest("launch admission preflight_report_digest", value.preflight_report_digest);
  if (!DECISIONS.includes(value.decision as AutonomousLaunchAdmissionDecision) || !(["approved", "held"] as const).includes(value.status as AutonomousLaunchAdmissionStatus)) throw new ArgumentError("launch admission decision or status is invalid");
  digest("launch admission authorizationDigest", value.authorization_digest, true);
  digest("launch admission reasonDigest", value.reason_digest, true);
  const domains = value.domains;
  if (!Array.isArray(domains) || domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch admission domains are outside their bound");
  const seen = new Set<string>();
  const counts = new Map<AutonomousLaunchAdmissionDomainState, number>(DOMAIN_STATES.map((state) => [state, 0]));
  for (const raw of domains) {
    if (!isObject(raw) || raw.schema !== AUTONOMOUS_LAUNCH_ADMISSION_DOMAIN_SCHEMA || !AUTONOMOUS_DOMAIN_NAMES.includes(raw.domain as typeof AUTONOMOUS_DOMAIN_NAMES[number]) || seen.has(raw.domain as string)) throw new ArgumentError("launch admission domains are duplicated or unsupported");
    seen.add(raw.domain as string);
    if (!PREFLIGHT_STATES.includes(raw.preflight_state as typeof PREFLIGHT_STATES[number]) || !DOMAIN_STATES.includes(raw.admission_state as AutonomousLaunchAdmissionDomainState)) throw new ArgumentError("launch admission domain state is invalid");
    digest("launch admission domain contract row digest", raw.contract_row_digest);
    text("launch admission domain readiness state", raw.readiness_state, 128);
    text("launch admission domain deployment state", raw.deployment_state, 128);
    strings("launch admission domain next_actions", raw.next_actions, 64);
    if (raw.retention !== RETENTION || raw.execution !== EXECUTION || raw.secret_material !== SECRET_MATERIAL) throw new ArgumentError("launch admission domain row execution posture is unsafe");
    counts.set(raw.admission_state as AutonomousLaunchAdmissionDomainState, (counts.get(raw.admission_state as AutonomousLaunchAdmissionDomainState) ?? 0) + 1);
  }
  if (seen.size !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch admission does not cover all twelve domains");
  const summary = value.summary;
  if (!isObject(summary) || summary.domain_count !== domains.length) throw new ArgumentError("launch admission summary is malformed");
  const summaryKeys = ["selected_domain_count", "approved_domain_count", "held_domain_count", "blocked_domain_count", "not_selected_domain_count"] as const;
  for (const key of summaryKeys) if (!Number.isSafeInteger(summary[key]) || (summary[key] as number) < 0 || (summary[key] as number) > domains.length) throw new ArgumentError(`launch admission summary ${key} is malformed`);
  if (summary.approved_domain_count !== counts.get("approved") || summary.held_domain_count !== counts.get("held") || summary.blocked_domain_count !== counts.get("blocked") || summary.not_selected_domain_count !== counts.get("not_selected") || summary.selected_domain_count !== domains.length - (counts.get("not_selected") ?? 0)) throw new ArgumentError("launch admission summary counts do not reconcile");
  if (value.status === "approved" && (value.decision !== "approve" || summary.approved_domain_count !== summary.selected_domain_count || summary.selected_domain_count === 0)) throw new ArgumentError("launch admission approved status is inconsistent");
  if (value.decision === "approve" && value.authorization_digest === null) throw new ArgumentError("launch admission approval is missing its authorization digest");
  strings("launch admission next_actions", value.next_actions, MAX_AUTONOMOUS_LAUNCH_ADMISSION_ACTIONS);
  return clone(value as unknown as AutonomousLaunchAdmissionReport);
}

/** Enforce an approved launch record immediately before execution dispatch. */
export function authorizeAutonomousLaunchDomains(
  value: unknown,
  requestedDomains: readonly typeof AUTONOMOUS_DOMAIN_NAMES[number][],
): AutonomousLaunchAdmissionReport {
  const report = validateAutonomousLaunchAdmission(value);
  if (report.status !== "approved") throw new ArgumentError("launch admission is not approved for execution");
  if (!Array.isArray(requestedDomains) || requestedDomains.length < 1 || requestedDomains.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch admission requestedDomains must contain one to twelve domains");
  const unique = new Set<string>();
  for (const [index, domain] of requestedDomains.entries()) {
    if (typeof domain !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(domain as typeof AUTONOMOUS_DOMAIN_NAMES[number])) throw new ArgumentError(`launch admission requestedDomains[${index}] contains an unsupported domain`);
    unique.add(domain);
  }
  if (unique.size !== requestedDomains.length) throw new ArgumentError("launch admission requestedDomains must be unique");
  const approved = new Set<typeof AUTONOMOUS_DOMAIN_NAMES[number]>(report.domains.filter((row) => row.admission_state === "approved").map((row) => row.domain));
  const missing = [...unique].filter((domain) => !approved.has(domain as typeof AUTONOMOUS_DOMAIN_NAMES[number])).sort();
  if (missing.length) throw new ArgumentError(`launch admission does not approve requested domains: ${missing.join(", ")}`);
  return report;
}
