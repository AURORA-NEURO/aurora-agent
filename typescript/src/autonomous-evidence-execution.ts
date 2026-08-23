import { ArgumentError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
import {
  AutonomousEvidencePlan,
} from "./autonomous-evidence.js";
import {
  AutonomousEvidenceRuntime,
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceEvaluator,
  type AutonomousEvidenceProjector,
  type AutonomousEvidenceRuntimeExecuteOptions,
  type AutonomousEvidenceRuntimeJournal,
  type AutonomousEvidenceRuntimeResult,
} from "./autonomous-evidence-runtime.js";
import {
  AutonomousEvidenceAdapterRegistry,
} from "./autonomous-evidence-adapters.js";
import {
  AutonomousEvidenceAdapterSelectionPlan as SelectionPlan,
  AutonomousEvidenceAdapterSelector,
  type AutonomousEvidenceAdapterSelectionOptions,
} from "./autonomous-evidence-adapter-selection.js";
import {
  AutonomousEvidenceAdapterHealthController,
  type AutonomousEvidenceAdapterHealthSelectionOptions,
  type AutonomousEvidenceAdapterHealthStore,
} from "./autonomous-evidence-adapter-health.js";
import {
  AutonomousEvidenceReadinessAuditor,
  AutonomousEvidenceReadinessPolicy,
  AutonomousEvidenceReadinessReport,
} from "./autonomous-evidence-readiness.js";
import {
  AutonomousEvidenceProviderContractRegistry,
} from "./autonomous-evidence-provider-contract.js";
import { AutonomousEvidenceSourcePolicy } from "./autonomous-evidence-source.js";
import {
  AutonomousEvidenceFailoverPolicy,
  createAutonomousEvidenceAdapterFailoverAcquirer,
  type AutonomousEvidenceFailoverAcquirerOptions,
} from "./autonomous-evidence-failover.js";
import {
  AutonomousEvidenceRetryPolicy,
  type AutonomousEvidenceRetryClassifier,
  type AutonomousEvidenceRetryAttempt,
} from "./autonomous-evidence-retry.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Reviewed source-dispatch orchestration schemas. */
export const AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-plan/0.1" as const;
export const AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-result/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_EXECUTION_REQUESTS = 128;
export const MAX_AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_BYTES = 512_000;

const RETENTION = "metadata_only;raw_source_values_and_provider_payloads_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;

export type AutonomousEvidenceExecutionPlanStatus = "ready_for_review" | "blocked";

export interface AutonomousEvidenceExecutionPlanJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA;
  evidence_plan_digest: string;
  domains: AutonomousDomainName[];
  registry_digest: string;
  provider_contract_registry_digest: string | null;
  source_policy_digest: string | null;
  source_kind: string | null;
  selection_plan: ReturnType<SelectionPlan["toJSON"]>;
  readiness: ReturnType<AutonomousEvidenceReadinessReport["toJSON"]>;
  readiness_policy: ReturnType<AutonomousEvidenceReadinessPolicy["toJSON"]>;
  retry_policy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  failover_policy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  degraded_dispatch_allowed: boolean;
  status: AutonomousEvidenceExecutionPlanStatus;
  approval_required: true;
  plan_digest: string;
  execution: "planning_only;source_dispatch_not_started";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

export interface AutonomousEvidenceExecutionPrepareOptions {
  selectionPlan?: SelectionPlan | unknown;
  selectionOptions?: AutonomousEvidenceAdapterSelectionOptions;
  adaptiveSelection?: boolean;
  healthSelectionOptions?: AutonomousEvidenceAdapterHealthSelectionOptions;
  readinessPolicy?: AutonomousEvidenceReadinessPolicy;
  retryPolicy?: AutonomousEvidenceRetryPolicy;
  failoverPolicy?: AutonomousEvidenceFailoverPolicy;
  providerContracts?: AutonomousEvidenceProviderContractRegistry;
  sourceBoundary?: {
    policy: AutonomousEvidenceSourcePolicy;
    sourceKind?: string;
  };
  allowDegradedDispatch?: boolean;
}

export interface AutonomousEvidenceExecutionOptions {
  approveSourceDispatch?: boolean;
  providerContracts?: AutonomousEvidenceProviderContractRegistry;
  projector?: AutonomousEvidenceProjector;
  evaluator?: AutonomousEvidenceEvaluator;
  journal?: AutonomousEvidenceRuntimeJournal;
  rehydrateValue?: AutonomousEvidenceRuntimeExecuteOptions["rehydrateValue"];
  parentEvidenceDigests?: readonly string[];
  stopOnFailure?: boolean;
  reevaluatePending?: boolean;
  classify?: AutonomousEvidenceRetryClassifier;
  observeFailover?: AutonomousEvidenceFailoverAcquirerOptions["observeFailover"];
  observeAttempt?: (attempt: AutonomousEvidenceRetryAttempt) => void | Promise<void>;
  clock?: AutonomousEvidenceFailoverAcquirerOptions["clock"];
  sleep?: AutonomousEvidenceFailoverAcquirerOptions["sleep"];
  sourceBoundary?: AutonomousEvidenceFailoverAcquirerOptions["sourceBoundary"];
}

export interface AutonomousEvidenceExecutionResultJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA;
  status: "completed" | "partial" | "awaiting_evaluation" | "failed" | "reconciliation_required";
  execution_plan_digest: string;
  readiness_report_digest: string;
  runtime: ReturnType<AutonomousEvidenceRuntimeResult["toJSON"]>;
  result_digest: string;
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function domains(value: readonly AutonomousDomainName[]): AutonomousDomainName[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > AUTONOMOUS_DOMAIN_NAMES.length) throw new ArgumentError("evidence execution domains are outside their bound");
  const normalized = value.map((domain, index) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`evidence execution domain ${index} is unsupported`);
    return domain;
  });
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError("evidence execution domains contain duplicates");
  return [...normalized];
}

function sameDomains(left: readonly AutonomousDomainName[], right: readonly AutonomousDomainName[]): boolean {
  return left.join("\u0000") === right.join("\u0000");
}

function bool(name: string, value: unknown, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") throw new ArgumentError(`${name} must be boolean`);
  return value;
}

function optionalSourceKind(value: unknown): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError("evidence execution sourceKind is malformed");
  return value;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

interface AutonomousEvidenceExecutionPlanPayload extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA;
  evidence_plan_digest: string;
  domains: AutonomousDomainName[];
  registry_digest: string;
  provider_contract_registry_digest: string | null;
  selection_plan: ReturnType<SelectionPlan["toJSON"]>;
  readiness: ReturnType<AutonomousEvidenceReadinessReport["toJSON"]>;
  readiness_policy: ReturnType<AutonomousEvidenceReadinessPolicy["toJSON"]>;
  retry_policy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  failover_policy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  source_policy_digest: string | null;
  source_kind: string | null;
  degraded_dispatch_allowed: boolean;
  status: AutonomousEvidenceExecutionPlanStatus;
  approval_required: true;
  execution: "planning_only;source_dispatch_not_started";
  retention: typeof RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function executionPlanPayload(input: {
  evidencePlanDigest: string;
  domains: readonly AutonomousDomainName[];
  registryDigest: string;
  providerContractRegistryDigest: string | null;
  selectionPlan: ReturnType<SelectionPlan["toJSON"]>;
  readiness: ReturnType<AutonomousEvidenceReadinessReport["toJSON"]>;
  readinessPolicy: ReturnType<AutonomousEvidenceReadinessPolicy["toJSON"]>;
  retryPolicy: ReturnType<AutonomousEvidenceRetryPolicy["toJSON"]>;
  failoverPolicy: ReturnType<AutonomousEvidenceFailoverPolicy["toJSON"]>;
  sourcePolicyDigest: string | null;
  sourceKind: string | null;
  degradedDispatchAllowed: boolean;
  status: AutonomousEvidenceExecutionPlanStatus;
}): AutonomousEvidenceExecutionPlanPayload {
  return {
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_SCHEMA,
    evidence_plan_digest: input.evidencePlanDigest,
    domains: [...input.domains],
    registry_digest: input.registryDigest,
    provider_contract_registry_digest: input.providerContractRegistryDigest,
    source_policy_digest: input.sourcePolicyDigest,
    source_kind: input.sourceKind,
    selection_plan: clone(input.selectionPlan),
    readiness: clone(input.readiness),
    readiness_policy: clone(input.readinessPolicy),
    retry_policy: clone(input.retryPolicy),
    failover_policy: clone(input.failoverPolicy),
    degraded_dispatch_allowed: input.degradedDispatchAllowed,
    status: input.status,
    approval_required: true,
    execution: "planning_only;source_dispatch_not_started",
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  };
}

/** Digest-bound plan joining the evidence contract to the exact reviewed source route. */
export class AutonomousEvidenceExecutionPlan {
  readonly evidence_plan_digest: string;
  readonly domains: AutonomousDomainName[];
  readonly registry_digest: string;
  readonly provider_contract_registry_digest: string | null;
  readonly source_policy_digest: string | null;
  readonly source_kind: string | null;
  readonly selection_plan: SelectionPlan;
  readonly readiness: AutonomousEvidenceReadinessReport;
  readonly readiness_policy: AutonomousEvidenceReadinessPolicy;
  readonly retry_policy: AutonomousEvidenceRetryPolicy;
  readonly failover_policy: AutonomousEvidenceFailoverPolicy;
  readonly degraded_dispatch_allowed: boolean;
  readonly status: AutonomousEvidenceExecutionPlanStatus;
  readonly plan_digest: string;

  constructor(input: {
    evidencePlan: AutonomousEvidencePlan;
    selectionPlan: SelectionPlan;
    readiness: AutonomousEvidenceReadinessReport;
    readinessPolicy: AutonomousEvidenceReadinessPolicy;
    retryPolicy: AutonomousEvidenceRetryPolicy;
    failoverPolicy: AutonomousEvidenceFailoverPolicy;
    providerContracts?: AutonomousEvidenceProviderContractRegistry;
    sourceBoundary?: AutonomousEvidenceExecutionPrepareOptions["sourceBoundary"];
    allowDegradedDispatch?: boolean;
  }) {
    if (!(input.evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution plan requires a typed evidence plan");
    if (!(input.selectionPlan instanceof SelectionPlan)) throw new ArgumentError("evidence execution plan requires a typed selection plan");
    if (!(input.readiness instanceof AutonomousEvidenceReadinessReport)) throw new ArgumentError("evidence execution plan requires a typed readiness report");
    if (!(input.readinessPolicy instanceof AutonomousEvidenceReadinessPolicy)) throw new ArgumentError("evidence execution plan readiness policy is malformed");
    if (!(input.retryPolicy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence execution plan retry policy is malformed");
    if (!(input.failoverPolicy instanceof AutonomousEvidenceFailoverPolicy)) throw new ArgumentError("evidence execution plan failover policy is malformed");
    const requested = domains(input.evidencePlan.domains);
    if (!sameDomains(requested, input.selectionPlan.domains) || !sameDomains(requested, input.readiness.domains.map((row) => row.domain))) throw new ArgumentError("evidence execution plan domain scopes do not align");
    this.evidence_plan_digest = digest("evidence execution evidence plan digest", input.evidencePlan.plan_digest);
    this.domains = requested;
    this.registry_digest = digest("evidence execution registry digest", input.selectionPlan.registry_digest);
    if (input.providerContracts !== undefined && !(input.providerContracts instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("evidence execution provider contract registry is malformed");
    if (input.sourceBoundary !== undefined) {
      if (!(input.sourceBoundary.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("evidence execution source boundary policy is malformed");
      if (!input.providerContracts) throw new ArgumentError("evidence execution source boundary requires provider contracts");
    }
    this.provider_contract_registry_digest = input.providerContracts?.toJSON().registry_digest ?? null;
    this.source_policy_digest = input.sourceBoundary?.policy.policy_digest === undefined ? null : digest("evidence execution source policy digest", input.sourceBoundary.policy.policy_digest);
    this.source_kind = optionalSourceKind(input.sourceBoundary?.sourceKind);
    this.selection_plan = input.selectionPlan;
    this.readiness = input.readiness;
    this.readiness_policy = input.readinessPolicy;
    this.retry_policy = input.retryPolicy;
    this.failover_policy = input.failoverPolicy;
    this.degraded_dispatch_allowed = bool("evidence execution allowDegradedDispatch", input.allowDegradedDispatch, false);
    if (input.readiness.policy_digest !== digestJsonSync(input.readinessPolicy.toJSON())) throw new ArgumentError("evidence execution readiness policy does not match its report");
    if (input.readiness.registry_digest !== this.registry_digest || input.readiness.selection_plan_digest !== input.selectionPlan.plan_digest) throw new ArgumentError("evidence execution readiness is not bound to its selection plan");
    if (digestJsonSync(input.failoverPolicy.retry_policy.toJSON()) !== digestJsonSync(input.retryPolicy.toJSON())) throw new ArgumentError("evidence execution retry and failover policies do not match");
    if (this.readiness.status === "ready" || (this.readiness.status === "degraded" && this.degraded_dispatch_allowed)) this.status = "ready_for_review";
    else this.status = "blocked";
    this.plan_digest = digestJsonSync(executionPlanPayload({
      evidencePlanDigest: this.evidence_plan_digest,
      domains: this.domains,
      registryDigest: this.registry_digest,
      providerContractRegistryDigest: this.provider_contract_registry_digest,
      sourcePolicyDigest: this.source_policy_digest,
      sourceKind: this.source_kind,
      selectionPlan: this.selection_plan.toJSON(),
      readiness: this.readiness.toJSON(),
      readinessPolicy: this.readiness_policy.toJSON(),
      retryPolicy: this.retry_policy.toJSON(),
      failoverPolicy: this.failover_policy.toJSON(),
      degradedDispatchAllowed: this.degraded_dispatch_allowed,
      status: this.status,
    }));
  }

  verify(registry: AutonomousEvidenceAdapterRegistry, evidencePlan: AutonomousEvidencePlan, providerContracts?: AutonomousEvidenceProviderContractRegistry, sourceBoundary?: AutonomousEvidenceExecutionOptions["sourceBoundary"]): this {
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence execution verification requires a typed adapter registry");
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution verification requires a typed evidence plan");
    if (evidencePlan.plan_digest !== this.evidence_plan_digest) throw new ArgumentError("evidence execution evidence plan is stale or tampered");
    if (registry.toJSON().registry_digest !== this.registry_digest) throw new ArgumentError("evidence execution registry is stale or tampered");
    if (this.provider_contract_registry_digest !== null) {
      if (!(providerContracts instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("evidence execution requires its bound provider contract registry");
      if (providerContracts.toJSON().registry_digest !== this.provider_contract_registry_digest) throw new ArgumentError("evidence execution provider contract registry is stale or tampered");
    } else if (providerContracts !== undefined) {
      throw new ArgumentError("evidence execution plan was not prepared with a provider contract registry");
    }
    if (this.source_policy_digest !== null) {
      if (!sourceBoundary || !(sourceBoundary.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("evidence execution requires its bound source boundary policy");
      if (sourceBoundary.policy.policy_digest !== this.source_policy_digest) throw new ArgumentError("evidence execution source boundary policy changed after planning");
      if (optionalSourceKind(sourceBoundary.sourceKind) !== this.source_kind) throw new ArgumentError("evidence execution source boundary source kind changed after planning");
    } else if (sourceBoundary !== undefined) {
      throw new ArgumentError("evidence execution plan was not prepared with a source boundary");
    }
    if (!sameDomains(evidencePlan.domains, this.domains)) throw new ArgumentError("evidence execution evidence plan domains changed");
    this.selection_plan.verify(registry);
    if (this.selection_plan.plan_digest !== this.readiness.selection_plan_digest) throw new ArgumentError("evidence execution selection plan is not bound to readiness");
    if (this.plan_digest !== digestJsonSync(this.planPayload())) throw new ArgumentError("evidence execution plan digest is invalid");
    return this;
  }

  private planPayload(): AutonomousEvidenceExecutionPlanPayload {
    return executionPlanPayload({
      evidencePlanDigest: this.evidence_plan_digest,
      domains: this.domains,
      registryDigest: this.registry_digest,
      providerContractRegistryDigest: this.provider_contract_registry_digest,
      sourcePolicyDigest: this.source_policy_digest,
      sourceKind: this.source_kind,
      selectionPlan: this.selection_plan.toJSON(),
      readiness: this.readiness.toJSON(),
      readinessPolicy: this.readiness_policy.toJSON(),
      retryPolicy: this.retry_policy.toJSON(),
      failoverPolicy: this.failover_policy.toJSON(),
      degradedDispatchAllowed: this.degraded_dispatch_allowed,
      status: this.status,
    });
  }

  toJSON(): AutonomousEvidenceExecutionPlanJSON {
    const projection = { ...this.planPayload(), plan_digest: this.plan_digest };
    if (new TextEncoder().encode(JSON.stringify(projection)).byteLength > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_PLAN_BYTES) throw new ArgumentError("evidence execution plan exceeds its bound");
    return projection;
  }
}

export class AutonomousEvidenceExecutionResult {
  readonly plan: AutonomousEvidenceExecutionPlan;
  readonly readiness: AutonomousEvidenceReadinessReport;
  readonly runtime: AutonomousEvidenceRuntimeResult;

  constructor(plan: AutonomousEvidenceExecutionPlan, readiness: AutonomousEvidenceReadinessReport, runtime: AutonomousEvidenceRuntimeResult) {
    if (!(plan instanceof AutonomousEvidenceExecutionPlan) || !(readiness instanceof AutonomousEvidenceReadinessReport) || !runtime || typeof runtime.toJSON !== "function") throw new ArgumentError("evidence execution result is malformed");
    this.plan = plan;
    this.readiness = readiness;
    this.runtime = runtime;
  }

  get status(): ReturnType<AutonomousEvidenceRuntimeResult["toJSON"]>["status"] {
    return this.runtime.toJSON().status;
  }

  get result_digest(): string {
    return digestJsonSync({ execution_plan_digest: this.plan.plan_digest, readiness_report_digest: this.readiness.report_digest, runtime_result_digest: this.runtime.toJSON().result_digest });
  }

  toJSON(): AutonomousEvidenceExecutionResultJSON {
    return {
      schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESULT_SCHEMA,
      status: this.status,
      execution_plan_digest: this.plan.plan_digest,
      readiness_report_digest: this.readiness.report_digest,
      runtime: this.runtime.toJSON(),
      result_digest: this.result_digest,
      retention: RETENTION,
      secret_material: SECRET_MATERIAL,
    };
  }
}

/**
 * Composes planning, readiness, approval, reviewed adapter routing, bounded failover, and the
 * existing evidence runtime. Preparation never dispatches; execution rechecks the exact plan and
 * readiness image before it allows a source adapter to run.
 */
export class AutonomousEvidenceExecutionController {
  readonly selector: AutonomousEvidenceAdapterSelector;
  readonly readinessAuditor: AutonomousEvidenceReadinessAuditor;

  constructor(readonly registry: AutonomousEvidenceAdapterRegistry, readonly healthStore?: AutonomousEvidenceAdapterHealthStore) {
    if (!(registry instanceof AutonomousEvidenceAdapterRegistry)) throw new ArgumentError("evidence execution controller requires a typed adapter registry");
    this.selector = new AutonomousEvidenceAdapterSelector(registry);
    this.readinessAuditor = new AutonomousEvidenceReadinessAuditor(registry, healthStore);
  }

  async prepare(evidencePlan: AutonomousEvidencePlan, options: AutonomousEvidenceExecutionPrepareOptions = {}): Promise<AutonomousEvidenceExecutionPlan> {
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution prepare requires a typed evidence plan");
    const requested = domains(evidencePlan.domains);
    const policy = options.readinessPolicy ?? new AutonomousEvidenceReadinessPolicy();
    if (!(policy instanceof AutonomousEvidenceReadinessPolicy)) throw new ArgumentError("evidence execution readiness policy is malformed");
    const retryPolicy = options.retryPolicy ?? options.failoverPolicy?.retry_policy ?? new AutonomousEvidenceRetryPolicy();
    if (!(retryPolicy instanceof AutonomousEvidenceRetryPolicy)) throw new ArgumentError("evidence execution retry policy is malformed");
    const failoverPolicy = options.failoverPolicy ?? new AutonomousEvidenceFailoverPolicy({ retryPolicy });
    if (!(failoverPolicy instanceof AutonomousEvidenceFailoverPolicy)) throw new ArgumentError("evidence execution failover policy is malformed");
    const allowDegradedDispatch = bool("evidence execution allowDegradedDispatch", options.allowDegradedDispatch, false);
    if (options.sourceBoundary !== undefined && !(options.sourceBoundary.policy instanceof AutonomousEvidenceSourcePolicy)) throw new ArgumentError("evidence execution source boundary policy is malformed");
    if (options.sourceBoundary !== undefined && options.providerContracts === undefined) throw new ArgumentError("evidence execution source boundary requires provider contracts");
    if (options.providerContracts !== undefined) {
      if (!(options.providerContracts instanceof AutonomousEvidenceProviderContractRegistry)) throw new ArgumentError("evidence execution provider contract registry is malformed");
      options.providerContracts.verify();
    }
    const selectionPlan = await this.resolveSelection(requested, options);
    const readiness = await this.readinessAuditor.audit(requested, {
      selectionPlan,
      policy,
      retryPolicy,
      failoverPolicy,
    });
    return new AutonomousEvidenceExecutionPlan({ evidencePlan, selectionPlan, readiness, readinessPolicy: policy, retryPolicy, failoverPolicy, providerContracts: options.providerContracts, sourceBoundary: options.sourceBoundary, allowDegradedDispatch });
  }

  async execute(
    executionPlan: AutonomousEvidenceExecutionPlan,
    evidencePlan: AutonomousEvidencePlan,
    requests: readonly AutonomousEvidenceAcquisitionRequest[],
    options: AutonomousEvidenceExecutionOptions = {},
  ): Promise<AutonomousEvidenceExecutionResult> {
    if (!(executionPlan instanceof AutonomousEvidenceExecutionPlan)) throw new ArgumentError("evidence execution requires a typed execution plan");
    if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution requires a typed evidence plan");
    executionPlan.verify(this.registry, evidencePlan, options.providerContracts, options.sourceBoundary);
    if (options.approveSourceDispatch !== true) throw new ArgumentError("evidence source dispatch requires explicit approval");
    if (executionPlan.status !== "ready_for_review") throw new ArgumentError("evidence execution plan is blocked by its readiness posture");
    if (!Array.isArray(requests) || requests.length < 1 || requests.length > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_REQUESTS) throw new ArgumentError("evidence execution requests are outside their bound");
    const currentReadiness = await this.readinessAuditor.audit(executionPlan.domains, {
      selectionPlan: executionPlan.selection_plan,
      policy: executionPlan.readiness_policy,
      retryPolicy: executionPlan.retry_policy,
      failoverPolicy: executionPlan.failover_policy,
    });
    const currentAllowed = currentReadiness.status === "ready" || (currentReadiness.status === "degraded" && executionPlan.degraded_dispatch_allowed);
    if (!currentAllowed) throw new ArgumentError("evidence readiness no longer permits the reviewed execution");
    if (currentReadiness.report_digest !== executionPlan.readiness.report_digest) throw new ArgumentError("evidence readiness changed after planning; review is required again");
    const failoverOptions: AutonomousEvidenceFailoverAcquirerOptions = {
      maxFailovers: executionPlan.failover_policy.max_failovers,
      ...(options.providerContracts === undefined ? {} : { providerContracts: options.providerContracts }),
      retryPolicy: executionPlan.retry_policy,
      classify: options.classify,
      observeFailover: options.observeFailover,
      observeAttempt: options.observeAttempt,
      clock: options.clock,
      sleep: options.sleep,
      ...(options.sourceBoundary === undefined ? {} : { sourceBoundary: options.sourceBoundary }),
    };
    const acquirer = createAutonomousEvidenceAdapterFailoverAcquirer(this.registry, executionPlan.selection_plan, failoverOptions);
    const runtime = new AutonomousEvidenceRuntime({ plan: evidencePlan, journal: options.journal });
    // A restarted high-level execution must hydrate the journal before the runtime decides
    // whether a request is fresh. Without this boundary, a valid journal would be mistaken for
    // an empty runtime and the append-only chain would reject the second acquisition attempt.
    await runtime.rehydrate();
    const result = await runtime.execute(requests, {
      acquirer,
      ...(options.projector === undefined ? {} : { projector: options.projector }),
      ...(options.evaluator === undefined ? {} : { evaluator: options.evaluator }),
      ...(options.rehydrateValue === undefined ? {} : { rehydrateValue: options.rehydrateValue }),
      ...(options.parentEvidenceDigests === undefined ? {} : { parentEvidenceDigests: options.parentEvidenceDigests }),
      ...(options.stopOnFailure === undefined ? {} : { stopOnFailure: options.stopOnFailure }),
      ...(options.reevaluatePending === undefined ? {} : { reevaluatePending: options.reevaluatePending }),
    });
    return new AutonomousEvidenceExecutionResult(executionPlan, currentReadiness, result);
  }

  private async resolveSelection(requested: AutonomousDomainName[], options: AutonomousEvidenceExecutionPrepareOptions): Promise<SelectionPlan> {
    if (options.selectionPlan !== undefined) {
      const plan = options.selectionPlan instanceof SelectionPlan ? options.selectionPlan : SelectionPlan.fromJSON(options.selectionPlan);
      if (!sameDomains(requested, plan.domains)) throw new ArgumentError("evidence execution selection plan domains do not match the evidence plan");
      plan.verify(this.registry);
      return plan;
    }
    if (options.adaptiveSelection) {
      if (!this.healthStore) throw new ArgumentError("adaptive evidence execution selection requires a health store");
      return new AutonomousEvidenceAdapterHealthController(this.healthStore, this.registry).selectAdaptiveForDomains(requested, options.healthSelectionOptions);
    }
    return this.selector.selectForDomains(requested, options.selectionOptions);
  }
}
