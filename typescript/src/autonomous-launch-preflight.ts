import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousReadinessReport,
} from "./autonomous.js";
import type {
  AutonomousBrainFacade,
  AutonomousBrainReadinessOptions,
} from "./autonomous-brain-facade.js";
import {
  validateAutonomousDomainAuditReport,
  type AutonomousDomainAuditOptions,
  type AutonomousDomainAuditReport,
} from "./autonomous-domain-audit.js";
import {
  auditAutonomousDeploymentReadiness,
  validateAutonomousDeploymentReadinessReport,
  type AutonomousDeploymentCapabilityInput,
  type AutonomousDeploymentCapabilityName,
  type AutonomousDeploymentReadinessPolicy,
  type AutonomousDeploymentReadinessReport,
} from "./autonomous-deployment-readiness.js";
import { ProviderSetup, type ProviderSetupPlan } from "./provider-setup.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** One bounded, provider-free launch handoff for all reviewed autonomous domains. */
export const AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA = "bioprism-typescript-autonomous-launch-preflight/0.1" as const;
export const AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-launch-preflight-domain/0.1" as const;
export const MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_BYTES = 512_000;
export const MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS = 512;

export type AutonomousLaunchPreflightState = "blocked" | "partial" | "ready_for_review";

const STATES: readonly AutonomousLaunchPreflightState[] = ["blocked", "partial", "ready_for_review"];
const RETENTION = "metadata_only;source_reports_summarized;runtime_values_not_retained" as const;
const EXECUTION = "preflight_only;no_provider_source_tool_queue_credential_or_learner_dispatch" as const;
const SECRET_MATERIAL = "never_returned" as const;
const DISPATCH_AUTHORIZATION = "preflight_review_only;does_not_grant_provider_source_tool_or_effect_authority" as const;
const SECRET_KEYS = new Set([
  "apikey", "bearer", "body", "content", "credential", "credentials", "headers", "messages", "password",
  "prompt", "request", "response", "secret", "task", "token",
]);

export interface AutonomousLaunchPreflightOptions {
  availableToolNames?: readonly string[];
  availableEvidence?: readonly string[];
  completedStages?: Readonly<Record<string, readonly string[]>>;
  readinessOptions?: AutonomousBrainReadinessOptions;
  deploymentPolicy?: AutonomousDeploymentReadinessPolicy;
  deploymentCapabilities?: Partial<Record<AutonomousDeploymentCapabilityName, AutonomousDeploymentCapabilityInput>>;
}

export interface AutonomousLaunchPreflightDomain extends JsonObject {
  schema: typeof AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA;
  domain: typeof AUTONOMOUS_DOMAIN_NAMES[number];
  state: AutonomousLaunchPreflightState;
  contract_status: "valid" | "invalid";
  contract_runtime_status: string;
  contract_row_digest: string;
  readiness_state: string;
  deployment_state: string;
  blocker_count: number;
  warning_count: number;
  next_actions: string[];
  retention: typeof RETENTION;
  execution: typeof EXECUTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousLaunchPreflightReport extends JsonObject {
  schema: typeof AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA;
  contract_audit: {
    report_digest: string;
    static_contract_status: "valid" | "invalid";
    runtime_status: string;
    domain_count: number;
    valid_domain_count: number;
    runtime_ready_domain_count: number;
    runtime_partial_domain_count: number;
    runtime_unassessed_domain_count: number;
  };
  agent_readiness: JsonObject;
  deployment_readiness: JsonObject;
  domains: AutonomousLaunchPreflightDomain[];
  summary: {
    state: AutonomousLaunchPreflightState;
    domain_count: number;
    ready_domain_count: number;
    partial_domain_count: number;
    blocked_domain_count: number;
    contract_report_digest: string;
    readiness_report_digest: string;
    deployment_report_digest: string;
  };
  next_actions: string[];
  dispatch: {
    status: "not_started";
    authorization: typeof DISPATCH_AUTHORIZATION;
    provider_calls: 0;
    source_calls: 0;
    tool_calls: 0;
    learner_mutations: 0;
    credential_resolution: 0;
  };
  retention: typeof RETENTION;
  execution: typeof EXECUTION;
  credential_posture: "caller_owned_opaque_handles_only;none_consumed";
  secret_material: typeof SECRET_MATERIAL;
  report_digest: string;
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function text(name: string, value: unknown, maximum = 2_048): string {
  if (typeof value !== "string" || value.trim().length === 0 || value.includes("\u0000")) throw new ArgumentError(`${name} must be a bounded non-empty string`);
  if (bytes(value) > maximum) throw new ArgumentError(`${name} exceeds its bounded size`);
  return value;
}

function digest(name: string, value: unknown): string {
  const candidate = text(name, value, 64);
  if (!/^[0-9a-f]{64}$/.test(candidate)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return candidate;
}

function integer(name: string, value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bounded integer contract`);
  return value as number;
}

function safeMetadata(value: unknown, depth = 0): void {
  if (depth > 10) throw new ArgumentError("launch preflight metadata nesting exceeds its bound");
  if (value === null) return;
  if (typeof value === "string") {
    if (bytes(value) > 8_192) throw new ArgumentError("launch preflight text field exceeds its bound");
    return;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new ArgumentError("launch preflight cannot contain non-finite numbers");
    return;
  }
  if (typeof value === "boolean") return;
  if (value instanceof Uint8Array) throw new ArgumentError("launch preflight cannot contain binary material");
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError("launch preflight metadata sequence exceeds its bound");
    for (const child of value) safeMetadata(child, depth + 1);
    return;
  }
  if (isObject(value)) {
    if (Object.keys(value).length > 512) throw new ArgumentError("launch preflight metadata mapping exceeds its bound");
    for (const [key, child] of Object.entries(value)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "");
      if (SECRET_KEYS.has(normalized)) throw new ArgumentError("launch preflight contains transient or secret-shaped metadata");
      safeMetadata(child, depth + 1);
    }
    return;
  }
  throw new ArgumentError("launch preflight metadata contains an unsupported value");
}

function strings(name: string, value: unknown, maximum = 512): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} is outside its bounded sequence contract`);
  return [...new Set(value.map((item, index) => text(`${name}[${index}]`, item, 1_024)))].sort();
}

function exactDomainRows<T extends { domain: string }>(name: string, rows: readonly T[]): T[] {
  if (!Array.isArray(rows) || rows.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError(`${name} must cover all twelve domains`);
  const domains = new Set(rows.map((row) => row.domain));
  if (domains.size !== AUTONOMOUS_DOMAIN_NAMES.length || AUTONOMOUS_DOMAIN_NAMES.some((domain) => !domains.has(domain))) throw new ArgumentError(`${name} must cover every built-in domain exactly once`);
  return [...rows];
}

function validateReadinessDigest(report: AutonomousReadinessReport): string {
  const supplied = digest("launch preflight readiness digest", report.readiness_digest);
  const { readiness_digest: _digest, ...withoutDigest } = report;
  if (digestJsonSync(withoutDigest) !== supplied) throw new ArgumentError("launch preflight readiness digest does not match its metadata");
  return supplied;
}

function readinessSummary(report: AutonomousReadinessReport): JsonObject {
  const readinessDigest = validateReadinessDigest(report);
  const domainRows = exactDomainRows("launch preflight readiness domains", report.domains).map((row) => ({
    domain: row.domain,
    state: text("launch preflight readiness state", row.state, 128),
    compatible_model_count: integer("launch preflight compatible model count", row.compatible_model_count),
    eligible_model_count: integer("launch preflight eligible model count", row.eligible_model_count),
    next_actions: strings("launch preflight readiness next_actions", row.next_actions, 64),
  }));
  const providerRows = report.providers.map((row) => ({
    provider: text("launch preflight provider", row.provider, 128),
    provider_registered: row.provider_registered === true,
    credential_ready: row.credential_ready === true,
    next_action: text("launch preflight provider next_action", row.next_action, 512),
  }));
  integer("launch preflight readiness model count", report.models.length);
  return {
    readiness_digest: readinessDigest,
    readiness_state: text("launch preflight readiness_state", report.readiness_state, 128),
    provider_count: providerRows.length,
    ready_provider_count: providerRows.filter((row) => row.credential_ready).length,
    providers: providerRows,
    model_count: report.models.length,
    domains: domainRows.sort((left, right) => left.domain.localeCompare(right.domain)),
  };
}

function deploymentSummary(report: AutonomousDeploymentReadinessReport): JsonObject {
  const validated = validateAutonomousDeploymentReadinessReport(report);
  const domainRows = exactDomainRows("launch preflight deployment domains", validated.domains).map((row) => ({
    domain: row.domain,
    state: text("launch preflight deployment state", row.state, 128),
    agent_state: text("launch preflight deployment agent_state", row.agent_state, 128),
    blockers: boundedBlockers(row.blockers, "deployment blockers"),
    warnings: boundedBlockers(row.warnings, "deployment warnings"),
    next_actions: strings("launch preflight deployment next_actions", row.next_actions, 64),
  }));
  return {
    readiness_digest: digest("launch preflight deployment readiness digest", validated.readiness_digest),
    state: text("launch preflight deployment overall state", validated.state, 128),
    ready_domain_count: integer("launch preflight deployment ready count", validated.ready_domain_count, 0, AUTONOMOUS_DOMAIN_NAMES.length),
    partial_domain_count: integer("launch preflight deployment partial count", validated.partial_domain_count, 0, AUTONOMOUS_DOMAIN_NAMES.length),
    blocked_domain_count: integer("launch preflight deployment blocked count", validated.blocked_domain_count, 0, AUTONOMOUS_DOMAIN_NAMES.length),
    provider_gate: {
      candidate_provider_count: integer("launch preflight candidate provider count", validated.provider_gate.candidate_provider_count),
      ready_provider_count: integer("launch preflight ready provider count", validated.provider_gate.ready_provider_count),
      unresolved_provider_count: integer("launch preflight unresolved provider count", validated.provider_gate.unresolved_provider_count),
    },
    capabilities: validated.capabilities.map((row) => ({ name: text("launch preflight capability name", row.name, 128), required: row.required === true, satisfies_requirement: row.satisfies_requirement === true })).sort((left, right) => left.name.localeCompare(right.name)),
    domains: domainRows.sort((left, right) => left.domain.localeCompare(right.domain)),
    global_blocker_count: boundedBlockers(validated.global_blockers, "global blockers").length,
    warning_count: boundedBlockers(validated.warnings, "global warnings").length,
  };
}

function boundedBlockers(value: unknown, name: string): JsonObject[] {
  if (!Array.isArray(value) || value.length > 512) throw new ArgumentError(`launch preflight ${name} are outside their bound`);
  return value.map((raw) => {
    if (!isObject(raw)) throw new ArgumentError(`launch preflight ${name} contain a malformed row`);
    return {
      code: text(`launch preflight ${name} code`, raw.code, 128),
      severity: text(`launch preflight ${name} severity`, raw.severity ?? "blocking", 32),
      scope: text(`launch preflight ${name} scope`, raw.scope, 32),
      domain: typeof raw.domain === "string" ? raw.domain : null,
      next_action: text(`launch preflight ${name} next_action`, raw.next_action, 1_024),
    };
  });
}

function combinedState(contract: JsonObject, readiness: JsonObject, deployment: JsonObject): AutonomousLaunchPreflightState {
  if (contract.contract_status === "invalid" || contract.runtime_status === "blocked" || deployment.state === "blocked") return "blocked";
  if (contract.runtime_status !== "ready_for_review" || readiness.state !== "ready_for_caller_approval" || deployment.state !== "ready_for_review") return "partial";
  return "ready_for_review";
}

function sourceAgentReadiness(readiness: AutonomousReadinessReport): JsonObject {
  const { readiness_digest: _digest, ...withoutDigest } = readiness;
  return { ...withoutDigest, readiness_digest: validateReadinessDigest(readiness) };
}

/**
 * Compose domain contracts, model/provider readiness, and deployment capabilities into one
 * bounded handoff. It is review-only and never resolves credentials or dispatches work.
 */
export async function auditAutonomousLaunchPreflight(
  source: {
    domainAudit: (options: AutonomousDomainAuditOptions) => Promise<AutonomousDomainAuditReport>;
    readiness: (options: AutonomousBrainReadinessOptions) => Promise<AutonomousReadinessReport>;
    providerPlan: ProviderSetupPlan;
  },
  options: AutonomousLaunchPreflightOptions = {},
): Promise<AutonomousLaunchPreflightReport> {
  if (!source || typeof source !== "object" || typeof source.domainAudit !== "function" || typeof source.readiness !== "function" || !isObject(source.providerPlan)) throw new ArgumentError("launch preflight source is malformed");
  if (!options || typeof options !== "object" || Array.isArray(options)) throw new ArgumentError("launch preflight options are malformed");
  if (options.deploymentCapabilities !== undefined) safeMetadata(options.deploymentCapabilities);
  const contractReport = await validateAutonomousDomainAuditReport(await source.domainAudit({
    ...(options.availableToolNames === undefined ? {} : { availableToolNames: options.availableToolNames }),
    ...(options.availableEvidence === undefined ? {} : { availableEvidence: options.availableEvidence }),
    ...(options.completedStages === undefined ? {} : { completedStages: options.completedStages }),
  }));
  const readiness = sourceAgentReadiness(await source.readiness(options.readinessOptions ?? {}));
  const deploymentReport = auditAutonomousDeploymentReadiness({
    agent: readiness as unknown as AutonomousReadinessReport,
    provider_plan: source.providerPlan,
    capabilities: options.deploymentCapabilities,
  }, options.deploymentPolicy ?? {});
  const readinessProjection = readinessSummary(readiness as unknown as AutonomousReadinessReport);
  const deploymentProjection = deploymentSummary(deploymentReport);
  const auditRows = new Map(contractReport.rows.map((row) => [row.domain, row]));
  const readinessRows = new Map((readinessProjection.domains as JsonObject[]).map((row) => [row.domain as string, row]));
  const deploymentRows = new Map((deploymentProjection.domains as JsonObject[]).map((row) => [row.domain as string, row]));
  const domains: AutonomousLaunchPreflightDomain[] = AUTONOMOUS_DOMAIN_NAMES.map((domain) => {
    const contract = auditRows.get(domain);
    const readinessRow = readinessRows.get(domain);
    const deploymentRow = deploymentRows.get(domain);
    if (!contract || !readinessRow || !deploymentRow) throw new ArgumentError(`launch preflight is missing the ${domain} domain`);
    const state = combinedState(contract as unknown as JsonObject, readinessRow, deploymentRow);
    const actionSet = new Set<string>([
      ...contract.next_actions,
      ...(readinessRow.next_actions as string[]),
      ...(deploymentRow.next_actions as string[]),
    ]);
    if (state === "blocked") actionSet.add("resolve blocking launch-preflight gates before dispatch");
    else if (state === "partial") actionSet.add("complete caller-owned launch-preflight inputs before dispatch review");
    return {
      schema: AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA,
      domain,
      state,
      contract_status: contract.contract_status,
      contract_runtime_status: contract.runtime_status,
      contract_row_digest: contract.row_digest,
      readiness_state: readinessRow.state as string,
      deployment_state: deploymentRow.state as string,
      blocker_count: (deploymentRow.blockers as JsonObject[]).length,
      warning_count: (deploymentRow.warnings as JsonObject[]).length,
      next_actions: [...actionSet].sort().slice(0, 64),
      retention: RETENTION,
      execution: EXECUTION,
      secret_material: SECRET_MATERIAL,
    };
  });
  const blockedCount = domains.filter((row) => row.state === "blocked").length;
  const partialCount = domains.filter((row) => row.state === "partial").length;
  const readyCount = domains.filter((row) => row.state === "ready_for_review").length;
  const state: AutonomousLaunchPreflightState = blockedCount > 0 ? "blocked" : partialCount > 0 ? "partial" : "ready_for_review";
  const nextActions = [...new Set([
    ...contractReport.next_actions,
    ...((readiness.next_actions ?? []) as string[]),
    ...((deploymentReport.next_actions ?? []) as string[]),
    ...domains.flatMap((row) => row.next_actions),
  ])].sort().slice(0, MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS);
  const body = {
    schema: AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA,
    contract_audit: {
      report_digest: contractReport.report_digest,
      static_contract_status: contractReport.summary.static_contract_status,
      runtime_status: contractReport.summary.runtime_status,
      domain_count: contractReport.summary.domain_count,
      valid_domain_count: contractReport.summary.valid_domain_count,
      runtime_ready_domain_count: contractReport.summary.runtime_ready_domain_count,
      runtime_partial_domain_count: contractReport.summary.runtime_partial_domain_count,
      runtime_unassessed_domain_count: contractReport.summary.runtime_unassessed_domain_count,
    },
    agent_readiness: readinessProjection,
    deployment_readiness: deploymentProjection,
    domains,
    summary: {
      state,
      domain_count: domains.length,
      ready_domain_count: readyCount,
      partial_domain_count: partialCount,
      blocked_domain_count: blockedCount,
      contract_report_digest: contractReport.report_digest,
      readiness_report_digest: readinessProjection.readiness_digest as string,
      deployment_report_digest: deploymentProjection.readiness_digest as string,
    },
    next_actions: nextActions,
    dispatch: {
      status: "not_started" as const,
      authorization: DISPATCH_AUTHORIZATION,
      provider_calls: 0 as const,
      source_calls: 0 as const,
      tool_calls: 0 as const,
      learner_mutations: 0 as const,
      credential_resolution: 0 as const,
    },
    retention: RETENTION,
    execution: EXECUTION,
    credential_posture: "caller_owned_opaque_handles_only;none_consumed" as const,
    secret_material: SECRET_MATERIAL,
  };
  safeMetadata(body);
  const report = { ...body, report_digest: digestJsonSync(body) } as AutonomousLaunchPreflightReport;
  if (bytes(JSON.stringify(report)) > MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_BYTES) throw new ArgumentError("launch preflight report exceeds its bounded size");
  return structuredClone(report);
}

/** Run the launch preflight through the high-level brain facade. */
export async function auditAutonomousBrainLaunchPreflight(
  brain: Pick<AutonomousBrainFacade, "domainAudit" | "readiness"> & { agent: { llm: ConstructorParameters<typeof ProviderSetup>[0] } },
  options: AutonomousLaunchPreflightOptions = {},
): Promise<AutonomousLaunchPreflightReport> {
  if (!brain || typeof brain.domainAudit !== "function" || typeof brain.readiness !== "function" || !brain.agent || !brain.agent.llm) throw new ArgumentError("launch preflight brain is malformed");
  return auditAutonomousLaunchPreflight({
    domainAudit: (auditOptions) => brain.domainAudit(auditOptions),
    readiness: (readinessOptions) => brain.readiness(readinessOptions),
    providerPlan: new ProviderSetup(brain.agent.llm).plan(),
  }, options);
}

/** Validate the aggregate launch handoff and its zero-dispatch posture. */
export function validateAutonomousLaunchPreflightReport(value: unknown): AutonomousLaunchPreflightReport {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LAUNCH_PREFLIGHT_SCHEMA) throw new ArgumentError("launch preflight report is malformed");
  safeMetadata(value);
  const expected = new Set(["schema", "contract_audit", "agent_readiness", "deployment_readiness", "domains", "summary", "next_actions", "dispatch", "retention", "execution", "credential_posture", "secret_material", "report_digest"]);
  if (Object.keys(value).length !== expected.size || Object.keys(value).some((key) => !expected.has(key))) throw new ArgumentError("launch preflight report contains unsupported or missing fields");
  if (value.retention !== RETENTION || value.execution !== EXECUTION || value.credential_posture !== "caller_owned_opaque_handles_only;none_consumed" || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("launch preflight report execution posture is unsafe");
  const supplied = digest("launch preflight report_digest", value.report_digest);
  const { report_digest: _digest, ...withoutDigest } = value;
  if (digestJsonSync(withoutDigest) !== supplied) throw new ArgumentError("launch preflight report_digest does not match its metadata");
  const domains = value.domains;
  if (!Array.isArray(domains) || domains.length !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch preflight domains are outside their bound");
  const seen = new Set<string>();
  for (const raw of domains) {
    if (!isObject(raw) || raw.schema !== AUTONOMOUS_LAUNCH_PREFLIGHT_DOMAIN_SCHEMA || !AUTONOMOUS_DOMAIN_NAMES.includes(raw.domain as typeof AUTONOMOUS_DOMAIN_NAMES[number]) || seen.has(raw.domain as string)) throw new ArgumentError("launch preflight domains are duplicated or unsupported");
    seen.add(raw.domain as string);
    if (!STATES.includes(raw.state as AutonomousLaunchPreflightState) || raw.contract_status !== "valid" && raw.contract_status !== "invalid") throw new ArgumentError("launch preflight domain state is invalid");
    text("launch preflight contract_runtime_status", raw.contract_runtime_status, 128);
    text("launch preflight readiness_state", raw.readiness_state, 128);
    text("launch preflight deployment_state", raw.deployment_state, 128);
    digest("launch preflight contract_row_digest", raw.contract_row_digest);
    integer("launch preflight blocker_count", raw.blocker_count, 0, 512);
    integer("launch preflight warning_count", raw.warning_count, 0, 512);
    strings("launch preflight domain next_actions", raw.next_actions, 64);
  }
  if (seen.size !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch preflight does not cover all twelve domains");
  const summary = value.summary;
  if (!isObject(summary) || summary.domain_count !== AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("launch preflight summary is malformed");
  for (const key of ["ready_domain_count", "partial_domain_count", "blocked_domain_count"] as const) integer(`launch preflight summary ${key}`, summary[key], 0, AUTONOMOUS_DOMAIN_NAMES.length);
  if ((["ready_domain_count", "partial_domain_count", "blocked_domain_count"] as const).reduce((sum, key) => sum + (summary[key] as number), 0) !== domains.length) throw new ArgumentError("launch preflight summary counts do not reconcile");
  strings("launch preflight next_actions", value.next_actions, MAX_AUTONOMOUS_LAUNCH_PREFLIGHT_ACTIONS);
  const dispatch = value.dispatch;
  if (!isObject(dispatch) || dispatch.status !== "not_started" || dispatch.authorization !== DISPATCH_AUTHORIZATION) throw new ArgumentError("launch preflight dispatch posture is unsafe");
  for (const key of ["provider_calls", "source_calls", "tool_calls", "learner_mutations", "credential_resolution"] as const) if (dispatch[key] !== 0) throw new ArgumentError("launch preflight reports unexpected dispatch activity");
  return structuredClone(value as unknown as AutonomousLaunchPreflightReport);
}

export default auditAutonomousBrainLaunchPreflight;
