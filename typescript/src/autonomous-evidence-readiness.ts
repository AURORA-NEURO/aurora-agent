import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousEvidenceAdapterRegistry,
  type AutonomousEvidenceAdapterCoverage,
} from "./autonomous-evidence-adapters.js";
import {
  AutonomousEvidenceAdapterSelectionPlan,
  AutonomousEvidenceAdapterSelector,
  type AutonomousEvidenceAdapterSelectionRow,
  type AutonomousEvidenceAdapterSelectionOptions,
} from "./autonomous-evidence-adapter-selection.js";
import {
  AutonomousEvidenceAdapterHealthController,
  type AutonomousEvidenceAdapterHealth,
  type AutonomousEvidenceAdapterHealthSelectionOptions,
  type AutonomousEvidenceAdapterHealthStore,
} from "./autonomous-evidence-adapter-health.js";
import { AutonomousEvidenceFailoverPolicy } from "./autonomous-evidence-failover.js";
import { AutonomousEvidenceRetryPolicy } from "./autonomous-evidence-retry.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Bounded, metadata-only operational readiness schemas for evidence routing. */
export const AUTONOMOUS_EVIDENCE_READINESS_SCHEMA = "bioprism-typescript-autonomous-evidence-readiness/0.1" as const;
export const AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-evidence-readiness-domain/0.1" as const;
export const AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA = "bioprism-typescript-autonomous-evidence-readiness-policy/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS = AUTONOMOUS_DOMAIN_NAMES.length;
export const MAX_AUTONOMOUS_EVIDENCE_READINESS_BYTES = 256_000;

const RETENTION = "metadata_only_coverage_selection_health_and_policy" as const;
const EXECUTION = "projection_only;no_source_dispatch" as const;
const SECRET_MATERIAL = "never_returned" as const;

export const AUTONOMOUS_EVIDENCE_READINESS_STATUSES = ["ready", "degraded", "blocked", "missing"] as const;
export type AutonomousEvidenceReadinessStatus = typeof AUTONOMOUS_EVIDENCE_READINESS_STATUSES[number];
export type AutonomousEvidenceReadinessOverallStatus = "ready" | "degraded" | "blocked";

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its identifier contract`);
  return value.trim();
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} is outside its bound`);
  return value as number;
}

function finite(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be between ${minimum} and ${maximum}`);
  return value;
}

function domains(value: readonly AutonomousDomainName[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS) throw new ArgumentError("evidence readiness domains are outside their bound");
  const normalized = value.map((domain, index) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`evidence readiness domain ${index} is unsupported`);
    return domain;
  });
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError("evidence readiness domains contain duplicates");
  return [...normalized];
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

export interface AutonomousEvidenceReadinessPolicyJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA;
  require_health: boolean;
  min_attempts: number;
  failure_threshold: number;
  min_success_rate: number;
  execution: "audit_only;policy_does_not_authorize_source_dispatch";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousEvidenceReadinessPolicyOptions {
  requireHealth?: boolean;
  minAttempts?: number;
  failureThreshold?: number;
  minSuccessRate?: number;
}

/**
 * Explicit readiness policy. It describes what a caller considers operationally usable;
 * it never grants credentials, source access, provider authority, or side effects.
 */
export class AutonomousEvidenceReadinessPolicy {
  readonly require_health: boolean;
  readonly min_attempts: number;
  readonly failure_threshold: number;
  readonly min_success_rate: number;

  constructor(options: AutonomousEvidenceReadinessPolicyOptions = {}) {
    if (options.requireHealth !== undefined && typeof options.requireHealth !== "boolean") throw new ArgumentError("evidence readiness requireHealth must be boolean");
    this.require_health = options.requireHealth ?? true;
    this.min_attempts = integer("evidence readiness minAttempts", options.minAttempts ?? 1, 1, 1_000_000);
    this.failure_threshold = finite("evidence readiness failureThreshold", options.failureThreshold ?? 0.75, 0, 1);
    this.min_success_rate = finite("evidence readiness minSuccessRate", options.minSuccessRate ?? 0.5, 0, 1);
  }

  toJSON(): AutonomousEvidenceReadinessPolicyJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_READINESS_POLICY_SCHEMA,
      require_health: this.require_health,
      min_attempts: this.min_attempts,
      failure_threshold: this.failure_threshold,
      min_success_rate: this.min_success_rate,
      execution: "audit_only;policy_does_not_authorize_source_dispatch",
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }
}

export interface AutonomousEvidenceReadinessHealthProjection extends JsonObject {
  observed: boolean;
  attempts: number;
  successes: number;
  failures: number;
  success_rate: number | null;
  failure_rate: number | null;
  circuit: "closed" | "open" | "unknown";
  manifest_digest: string | null;
}

export interface AutonomousEvidenceReadinessDomainJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA;
  domain: AutonomousDomainName;
  status: AutonomousEvidenceReadinessStatus;
  coverage_state: AutonomousEvidenceAdapterCoverage["state"];
  adapter_ids: string[];
  selected_adapter_id: string | null;
  selected_manifest_digest: string | null;
  candidate_count: number;
  eligible_candidate_count: number;
  selection_reason: string;
  selection_strategy: AutonomousEvidenceAdapterSelectionPlan["strategy"];
  health: AutonomousEvidenceReadinessHealthProjection;
  retry_policy_digest: string;
  failover_policy_digest: string;
  reason: string;
  execution: typeof EXECUTION;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function healthProjection(row: AutonomousEvidenceAdapterHealth | undefined): AutonomousEvidenceReadinessHealthProjection {
  return {
    observed: row !== undefined && row.attempts > 0,
    attempts: row?.attempts ?? 0,
    successes: row?.successes ?? 0,
    failures: row?.failures ?? 0,
    success_rate: row === undefined || row.attempts === 0 ? null : row.success_rate,
    failure_rate: row === undefined || row.attempts === 0 ? null : row.failure_rate,
    circuit: row?.circuit ?? "unknown",
    manifest_digest: row?.manifest_digest ?? null,
  };
}

function domainReason(status: AutonomousEvidenceReadinessStatus, selection: AutonomousEvidenceAdapterSelectionRow, health: AutonomousEvidenceReadinessHealthProjection): string {
  if (status === "missing") return selection.reason === "no_matching_adapter" ? "no_registered_adapter_matches_domain_and_capability" : "no_registered_adapter_matches_requested_readiness_scope";
  if (status === "blocked") {
    if (selection.status === "missing") return selection.reason;
    if (health.circuit === "open") return "selected_adapter_health_circuit_open";
    if (!health.observed) return "selected_adapter_has_no_usable_health_observation";
    return "selected_adapter_health_below_readiness_threshold";
  }
  if (status === "degraded") return health.observed ? "selected_adapter_is_usable_but_health_is_not_required_or_insufficiently_observed" : "selected_adapter_has_no_health_observation";
  return "selected_adapter_has_current_manifest_and_usable_health";
}

export class AutonomousEvidenceReadinessDomain {
  readonly domain: AutonomousDomainName;
  readonly status: AutonomousEvidenceReadinessStatus;
  readonly coverage_state: AutonomousEvidenceAdapterCoverage["state"];
  readonly adapter_ids: string[];
  readonly selected_adapter_id: string | null;
  readonly selected_manifest_digest: string | null;
  readonly candidate_count: number;
  readonly eligible_candidate_count: number;
  readonly selection_reason: string;
  readonly selection_strategy: AutonomousEvidenceAdapterSelectionPlan["strategy"];
  readonly health: AutonomousEvidenceReadinessHealthProjection;
  readonly retry_policy_digest: string;
  readonly failover_policy_digest: string;
  readonly reason: string;

  constructor(input: {
    domain: AutonomousDomainName;
    status: AutonomousEvidenceReadinessStatus;
    coverage_state: AutonomousEvidenceAdapterCoverage["state"];
    adapter_ids: readonly string[];
    selected_adapter_id: string | null;
    selected_manifest_digest: string | null;
    candidate_count: number;
    eligible_candidate_count: number;
    selection_reason: string;
    selection_strategy: AutonomousEvidenceAdapterSelectionPlan["strategy"];
    health: AutonomousEvidenceReadinessHealthProjection;
    retry_policy_digest: string;
    failover_policy_digest: string;
    reason: string;
  }) {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(input.domain)) throw new ArgumentError("evidence readiness row domain is unsupported");
    if (!AUTONOMOUS_EVIDENCE_READINESS_STATUSES.includes(input.status)) throw new ArgumentError("evidence readiness row status is invalid");
    if (input.coverage_state !== "complete" && input.coverage_state !== "missing") throw new ArgumentError("evidence readiness coverage state is invalid");
    if (!Array.isArray(input.adapter_ids) || input.adapter_ids.length > 256) throw new ArgumentError("evidence readiness adapter ids exceed their bound");
    const adapterIds = input.adapter_ids.map((value) => identifier("evidence readiness adapter id", value));
    if (new Set(adapterIds).size !== adapterIds.length) throw new ArgumentError("evidence readiness adapter ids contain duplicates");
    if (!Number.isSafeInteger(input.candidate_count) || input.candidate_count < 0 || input.candidate_count > 256) throw new ArgumentError("evidence readiness candidate count is invalid");
    if (!Number.isSafeInteger(input.eligible_candidate_count) || input.eligible_candidate_count < 0 || input.eligible_candidate_count > input.candidate_count) throw new ArgumentError("evidence readiness eligible candidate count is invalid");
    if (input.selected_adapter_id !== null) identifier("evidence readiness selected adapter id", input.selected_adapter_id);
    if (input.selected_manifest_digest !== null) digest("evidence readiness selected manifest digest", input.selected_manifest_digest);
    identifier("evidence readiness selection reason", input.selection_reason);
    if (!Object.prototype.hasOwnProperty.call({ lexicographic_adapter_id: true, weighted_evidence: true }, input.selection_strategy)) throw new ArgumentError("evidence readiness selection strategy is invalid");
    if (!input.health || typeof input.health !== "object") throw new ArgumentError("evidence readiness health projection is malformed");
    if (typeof input.health.observed !== "boolean" || !Number.isSafeInteger(input.health.attempts) || input.health.attempts < 0 || !Number.isSafeInteger(input.health.successes) || input.health.successes < 0 || !Number.isSafeInteger(input.health.failures) || input.health.failures < 0 || input.health.successes + input.health.failures > input.health.attempts || !["closed", "open", "unknown"].includes(input.health.circuit)) throw new ArgumentError("evidence readiness health projection is invalid");
    if (input.health.success_rate !== null) finite("evidence readiness health success rate", input.health.success_rate, 0, 1);
    if (input.health.failure_rate !== null) finite("evidence readiness health failure rate", input.health.failure_rate, 0, 1);
    if (input.health.manifest_digest !== null) digest("evidence readiness health manifest digest", input.health.manifest_digest);
    digest("evidence readiness retry policy digest", input.retry_policy_digest);
    digest("evidence readiness failover policy digest", input.failover_policy_digest);
    identifier("evidence readiness reason", input.reason);
    this.domain = input.domain;
    this.status = input.status;
    this.coverage_state = input.coverage_state;
    this.adapter_ids = [...adapterIds];
    this.selected_adapter_id = input.selected_adapter_id;
    this.selected_manifest_digest = input.selected_manifest_digest;
    this.candidate_count = input.candidate_count;
    this.eligible_candidate_count = input.eligible_candidate_count;
    this.selection_reason = input.selection_reason;
    this.selection_strategy = input.selection_strategy;
    this.health = clone(input.health);
    this.retry_policy_digest = input.retry_policy_digest;
    this.failover_policy_digest = input.failover_policy_digest;
    this.reason = input.reason;
  }

  toJSON(): AutonomousEvidenceReadinessDomainJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_READINESS_DOMAIN_SCHEMA,
      domain: this.domain,
      status: this.status,
      coverage_state: this.coverage_state,
      adapter_ids: [...this.adapter_ids],
      selected_adapter_id: this.selected_adapter_id,
      selected_manifest_digest: this.selected_manifest_digest,
      candidate_count: this.candidate_count,
      eligible_candidate_count: this.eligible_candidate_count,
      selection_reason: this.selection_reason,
      selection_strategy: this.selection_strategy,
      health: clone(this.health),
      retry_policy_digest: this.retry_policy_digest,
      failover_policy_digest: this.failover_policy_digest,
      reason: this.reason,
      execution: EXECUTION,
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }
}

export interface AutonomousEvidenceReadinessReportJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_READINESS_SCHEMA;
  domains: AutonomousEvidenceReadinessDomainJSON[];
  registry_digest: string;
  selection_plan_digest: string;
  health_snapshot_digest: string | null;
  policy_digest: string;
  readiness_policy: AutonomousEvidenceReadinessPolicyJSON;
  retry_policy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  failover_policy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  status: AutonomousEvidenceReadinessOverallStatus;
  ready_count: number;
  degraded_count: number;
  blocked_count: number;
  missing_count: number;
  complete: boolean;
  report_digest: string;
  execution: typeof EXECUTION;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function overallStatus(rows: readonly AutonomousEvidenceReadinessDomain[]): AutonomousEvidenceReadinessOverallStatus {
  if (rows.some((row) => row.status === "blocked" || row.status === "missing")) return "blocked";
  if (rows.some((row) => row.status === "degraded")) return "degraded";
  return "ready";
}

interface AutonomousEvidenceReadinessReportPayload extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_READINESS_SCHEMA;
  domains: AutonomousEvidenceReadinessDomainJSON[];
  registry_digest: string;
  selection_plan_digest: string;
  health_snapshot_digest: string | null;
  policy_digest: string;
  readiness_policy: AutonomousEvidenceReadinessPolicyJSON;
  retry_policy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  failover_policy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  status: AutonomousEvidenceReadinessOverallStatus;
  ready_count: number;
  degraded_count: number;
  blocked_count: number;
  missing_count: number;
  complete: boolean;
  execution: typeof EXECUTION;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function reportPayload(input: AutonomousEvidenceReadinessReportPayload): AutonomousEvidenceReadinessReportPayload {
  return input;
}

export class AutonomousEvidenceReadinessReport {
  readonly domains: AutonomousEvidenceReadinessDomain[];
  readonly registry_digest: string;
  readonly selection_plan_digest: string;
  readonly health_snapshot_digest: string | null;
  readonly policy_digest: string;
  readonly readiness_policy: AutonomousEvidenceReadinessPolicyJSON;
  readonly retry_policy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  readonly failover_policy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  readonly status: AutonomousEvidenceReadinessOverallStatus;
  readonly ready_count: number;
  readonly degraded_count: number;
  readonly blocked_count: number;
  readonly missing_count: number;

  constructor(input: {
    domains: readonly AutonomousEvidenceReadinessDomain[];
    registryDigest: string;
    selectionPlanDigest: string;
    healthSnapshotDigest?: string | null;
    policy: AutonomousEvidenceReadinessPolicy;
    retryPolicy: AutonomousEvidenceRetryPolicy;
    failoverPolicy: AutonomousEvidenceFailoverPolicy;
  }) {
    if (!Array.isArray(input.domains) || input.domains.length < 1 || input.domains.length > MAX_AUTONOMOUS_EVIDENCE_READINESS_DOMAINS) throw new ArgumentError("evidence readiness report domains are outside their bound");
    if (input.domains.some((row) => !(row instanceof AutonomousEvidenceReadinessDomain))) throw new ArgumentError("evidence readiness report rows are malformed");
    if (new Set(input.domains.map((row) => row.domain)).size !== input.domains.length) throw new ArgumentError("evidence readiness report domains contain duplicates");
    this.domains = [...input.domains];
    this.registry_digest = digest("evidence readiness registry digest", input.registryDigest);
    this.selection_plan_digest = digest("evidence readiness selection plan digest", input.selectionPlanDigest);
    this.health_snapshot_digest = input.healthSnapshotDigest === undefined || input.healthSnapshotDigest === null ? null : digest("evidence readiness health snapshot digest", input.healthSnapshotDigest);
    if (!(input.policy instanceof AutonomousEvidenceReadinessPolicy)) throw new ArgumentError("evidence readiness policy is malformed");
    if (!(input.retryPolicy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence readiness retry policy is malformed");
    if (!(input.failoverPolicy instanceof AutonomousEvidenceFailoverPolicy)) throw new ArgumentError("evidence readiness failover policy is malformed");
    this.policy_digest = digestJsonSync(input.policy.toJSON());
    this.readiness_policy = clone(input.policy.toJSON());
    this.retry_policy = clone(input.retryPolicy.toJSON());
    this.failover_policy = clone(input.failoverPolicy.toJSON());
    this.status = overallStatus(this.domains);
    this.ready_count = this.domains.filter((row) => row.status === "ready").length;
    this.degraded_count = this.domains.filter((row) => row.status === "degraded").length;
    this.blocked_count = this.domains.filter((row) => row.status === "blocked").length;
    this.missing_count = this.domains.filter((row) => row.status === "missing").length;
  }

  get complete(): boolean {
    return this.status === "ready";
  }

  private payload(): AutonomousEvidenceReadinessReportPayload {
    return reportPayload({
      schema: AUTONOMOUS_EVIDENCE_READINESS_SCHEMA,
      domains: this.domains.map((row) => row.toJSON()),
      registry_digest: this.registry_digest,
      selection_plan_digest: this.selection_plan_digest,
      health_snapshot_digest: this.health_snapshot_digest,
      policy_digest: this.policy_digest,
      readiness_policy: this.readiness_policy,
      retry_policy: this.retry_policy,
      failover_policy: this.failover_policy,
      status: this.status,
      ready_count: this.ready_count,
      degraded_count: this.degraded_count,
      blocked_count: this.blocked_count,
      missing_count: this.missing_count,
      complete: this.complete,
      execution: EXECUTION,
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    });
  }

  get report_digest(): string {
    return digestJsonSync(this.payload());
  }

  toJSON(): AutonomousEvidenceReadinessReportJSON {
    const projection = { ...this.payload(), report_digest: this.report_digest };
    if (new TextEncoder().encode(JSON.stringify(projection)).byteLength > MAX_AUTONOMOUS_EVIDENCE_READINESS_BYTES) throw new ArgumentError("evidence readiness report exceeds its bound");
    return projection;
  }
}

export interface AutonomousEvidenceReadinessAuditOptions {
  selectionPlan?: AutonomousEvidenceAdapterSelectionPlan | unknown;
  selectionOptions?: AutonomousEvidenceAdapterSelectionOptions;
  adaptiveSelection?: boolean;
  healthSelectionOptions?: AutonomousEvidenceAdapterHealthSelectionOptions;
  policy?: AutonomousEvidenceReadinessPolicy;
  retryPolicy?: AutonomousEvidenceRetryPolicy;
  failoverPolicy?: AutonomousEvidenceFailoverPolicy;
}

function coverageFor(registry: AutonomousEvidenceAdapterRegistry, domain: AutonomousDomainName): AutonomousEvidenceAdapterCoverage {
  const row = registry.coverage().find((candidate) => candidate.domain === domain);
  if (!row) throw new ArgumentError(`evidence readiness coverage is missing for ${domain}`);
  return row;
}

function selectionStatus(selection: AutonomousEvidenceAdapterSelectionRow, coverage: AutonomousEvidenceAdapterCoverage, health: AutonomousEvidenceReadinessHealthProjection, policy: AutonomousEvidenceReadinessPolicy): AutonomousEvidenceReadinessStatus {
  if (selection.status === "missing") return coverage.state === "missing" || selection.reason === "no_matching_adapter" ? "missing" : "blocked";
  if (health.circuit === "open") return "blocked";
  if (!health.observed || health.attempts < policy.min_attempts || (health.success_rate ?? 0) < policy.min_success_rate) return policy.require_health ? "blocked" : "degraded";
  return "ready";
}

/**
 * Audits the complete evidence-routing posture without acquiring evidence. The returned
 * report is suitable for admission/UI/operations decisions, but it is never an authorization.
 */
export class AutonomousEvidenceReadinessAuditor {
  readonly selector: AutonomousEvidenceAdapterSelector;

  constructor(readonly registry: AutonomousEvidenceAdapterRegistry, readonly healthStore?: AutonomousEvidenceAdapterHealthStore) {
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence readiness auditor requires a typed adapter registry");
    if (healthStore !== undefined && (!healthStore || typeof healthStore.health !== "function" || typeof healthStore.snapshot !== "function")) throw new ArgumentError("evidence readiness auditor health store is malformed");
    this.selector = new AutonomousEvidenceAdapterSelector(registry);
  }

  async audit(requestedDomains: readonly AutonomousDomainName[], options: AutonomousEvidenceReadinessAuditOptions = {}): Promise<AutonomousEvidenceReadinessReport> {
    const requested = domains(requestedDomains);
    const policy = options.policy ?? new AutonomousEvidenceReadinessPolicy();
    if (!(policy instanceof AutonomousEvidenceReadinessPolicy)) throw new ArgumentError("evidence readiness policy is malformed");
    const retryPolicy = options.retryPolicy ?? options.failoverPolicy?.retry_policy ?? new AutonomousEvidenceRetryPolicy();
    if (!(retryPolicy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence readiness retry policy is malformed");
    const failoverPolicy = options.failoverPolicy ?? new AutonomousEvidenceFailoverPolicy({ retryPolicy });
    if (!(failoverPolicy instanceof AutonomousEvidenceFailoverPolicy)) throw new ArgumentError("evidence readiness failover policy is malformed");
    const plan = await this.resolvePlan(requested, options);
    plan.verify(this.registry);
    const coverage = new Map(requested.map((domain) => [domain, coverageFor(this.registry, domain)]));
    const healthSnapshot = this.healthStore ? await this.healthStore.snapshot() : null;
    const rows = requested.map((domain) => {
      const selection = plan.rows.find((row) => row.domain === domain);
      if (!selection) throw new ArgumentError(`evidence readiness selection plan does not cover ${domain}`);
      const coverageRow = coverage.get(domain)!;
      const selectedHealth = selection.adapter_id === null || selection.manifest_digest === null || !this.healthStore
        ? undefined
        : this.healthStore.health({ adapter_id: selection.adapter_id, manifest_digest: selection.manifest_digest, domain, min_attempts: policy.min_attempts, failure_threshold: policy.failure_threshold })[0];
      const health = healthProjection(selectedHealth);
      const status = selectionStatus(selection, coverageRow, health, policy);
      return new AutonomousEvidenceReadinessDomain({
        domain,
        status,
        coverage_state: coverageRow.state,
        adapter_ids: coverageRow.adapter_ids,
        selected_adapter_id: selection.adapter_id,
        selected_manifest_digest: selection.manifest_digest,
        candidate_count: selection.candidate_ids.length,
        eligible_candidate_count: selection.candidate_eligible.filter(Boolean).length,
        selection_reason: selection.reason,
        selection_strategy: plan.strategy,
        health,
        retry_policy_digest: digestJsonSync(retryPolicy.toJSON()),
        failover_policy_digest: digestJsonSync(failoverPolicy.toJSON()),
        reason: domainReason(status, selection, health),
      });
    });
    return new AutonomousEvidenceReadinessReport({
      domains: rows,
      registryDigest: this.registry.toJSON().registry_digest,
      selectionPlanDigest: plan.plan_digest,
      healthSnapshotDigest: healthSnapshot?.snapshot_digest ?? null,
      policy,
      retryPolicy,
      failoverPolicy,
    });
  }

  private async resolvePlan(requested: AutonomousDomainName[], options: AutonomousEvidenceReadinessAuditOptions): Promise<AutonomousEvidenceAdapterSelectionPlan> {
    if (options.selectionPlan !== undefined) {
      const plan = options.selectionPlan instanceof AutonomousEvidenceAdapterSelectionPlan ? options.selectionPlan : AutonomousEvidenceAdapterSelectionPlan.fromJSON(options.selectionPlan);
      if (plan.domains.join("\u0000") !== requested.join("\u0000")) throw new ArgumentError("evidence readiness selection plan domains do not match the audit request");
      return plan;
    }
    if (options.adaptiveSelection) {
      if (!this.healthStore) throw new ArgumentError("adaptive evidence readiness selection requires a health store");
      const controller = new AutonomousEvidenceAdapterHealthController(this.healthStore, this.registry);
      return controller.selectAdaptiveForDomains(requested, options.healthSelectionOptions);
    }
    return this.selector.selectForDomains(requested, options.selectionOptions);
  }
}
