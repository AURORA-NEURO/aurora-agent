import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, routeAutonomousTask, type AutonomousDomainName, type AutonomousRouteProposal } from "./autonomous.js";
import {
  AutonomousConnectorDispatchRequest,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousConnectorSelectionPlan,
  type AutonomousConnectorDispatchResult,
  type AutonomousConnectorDispatchStatus,
  type AutonomousConnectorSelectionStrategy,
} from "./autonomous-connectors.js";
import {
  AutonomousConnectorOperationContract,
  AutonomousConnectorOperationRegistry,
  AutonomousConnectorWorkItem,
  AutonomousConnectorWorkQueuePersistenceCoordinator,
  AutonomousConnectorWorker,
  InMemoryAutonomousConnectorWorkQueue,
  type AutonomousConnectorWorkQueuePersistence,
  type AutonomousConnectorWorkerRun,
} from "./autonomous-connector-worker.js";
import { digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/**
 * A high-level, provider-neutral operation boundary for the autonomous connector runtime.
 *
 * The lower-level connector classes intentionally expose every identity and approval field so
 * durable mission/workflow code can bind them itself. This facade composes those primitives for
 * ordinary embedding applications: it resolves the reviewed operation contract, selects the
 * connector, creates deterministic replay identities, validates the request, and dispatches
 * only through a digest-bound selection plan. Raw request metadata is used transiently and never
 * appears in a plan, batch summary, or error projection.
 */
export const AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA = "bioprism-typescript-autonomous-connector-operation-facade/0.1" as const;
export const AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA = "bioprism-typescript-autonomous-connector-operation-batch/0.1" as const;
export const MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH = 64;
export const MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM = 8;
export const MAX_AUTONOMOUS_CONNECTOR_FACADE_PARENT_DIGESTS = 128;
export const AUTONOMOUS_CONNECTOR_INTENT_SCHEMA = "bioprism-typescript-autonomous-connector-intent/0.1" as const;
export const MAX_AUTONOMOUS_CONNECTOR_INTENT_TASK_BYTES = 128_000;
export const MAX_AUTONOMOUS_CONNECTOR_INTENT_HINTS = 32;
export const AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA = "bioprism-typescript-autonomous-connector-intent-job/0.1" as const;
export const MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS = 32;
export const AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA = "bioprism-typescript-autonomous-connector-intent-controller/0.1" as const;

const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey",
  "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) {
    throw new ArgumentError(`${name} is outside its identifier contract`);
  }
  return value;
}

function capability(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || bytes(value) > 256 || !/^[A-Za-z0-9_.:+-]+$/.test(value)) {
    throw new ArgumentError(`${name} is outside its capability contract`);
  }
  return value;
}

function digest(name: string, value: unknown): string {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function domain(name: string, value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(value as AutonomousDomainName)) throw new ArgumentError(`${name} is not a supported autonomous domain`);
  return value as AutonomousDomainName;
}

function intentTokens(value: string): Set<string> {
  return new Set(value.toLowerCase().replace(/[^a-z0-9]+/g, " ").split(/\s+/).filter((token) => token.length >= 2));
}

function selectIntentCapability(
  operation: AutonomousConnectorOperationContract,
  text: string,
  requested: string | undefined,
): { capability: string; score: number; matchedTerms: string[]; reason: string } {
  if (requested !== undefined) {
    if (!operation.supports(requested)) throw new ArgumentError(`connector intent capability ${requested} is outside ${operation.operation_id}`);
    return { capability: requested, score: 1, matchedTerms: [requested], reason: "caller_capability" };
  }
  const task = intentTokens(text);
  const rows = operation.capabilities.map((candidate) => {
    const matchedTerms = [...intentTokens(candidate.replace(/\+/g, " "))].filter((term) => task.has(term)).sort();
    const exact = text.toLowerCase().includes(candidate.toLowerCase()) ? 1 : 0;
    const score = Math.min(1, matchedTerms.length * 0.25 + exact * 0.65);
    return { capability: candidate, score, matchedTerms, reason: score > 0 ? "exact_catalogue_terms" : "domain_default_capability" };
  });
  rows.sort((left, right) => right.score - left.score || left.capability.localeCompare(right.capability));
  return rows[0]!;
}

function rejectsSecretField(name: string): boolean {
  const normalized = name.toLowerCase().replace(/[^a-z0-9]/g, "");
  return SECRET_FIELD_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj");
}

/** Validate JSON metadata before it is hashed, including the facade's derived subject identity. */
function assertSafeMetadata(value: unknown, path = "$", depth = 0): void {
  if (depth > 32) throw new ArgumentError(`${path} is too deeply nested`);
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number" && Number.isFinite(value)) return;
  if (Array.isArray(value)) {
    for (let index = 0; index < value.length; index += 1) assertSafeMetadata(value[index], `${path}[${index}]`, depth + 1);
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      if (rejectsSecretField(key)) throw new ArgumentError(`${path} contains credential-shaped fields`);
      if (child === undefined) throw new ArgumentError(`${path}.${key} is undefined`);
      assertSafeMetadata(child, `${path}.${key}`, depth + 1);
    }
    return;
  }
  throw new ArgumentError(`${path} must be JSON-safe`);
}

function parentDigests(value: readonly string[] | undefined): string[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > MAX_AUTONOMOUS_CONNECTOR_FACADE_PARENT_DIGESTS) throw new ArgumentError("connector operation parent_digests exceeds its bound");
  const normalized = value.map((entry) => digest("connector operation parent_digest", entry));
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError("connector operation parent_digests contains duplicates");
  return normalized;
}

function stableIdentity(prefix: string, identityDigest: string): string {
  return `${prefix}-${identityDigest.slice(0, 48)}`;
}

function errorProjection(error: unknown): { error_class: string; failure_code: string } {
  if (error instanceof ProviderRuntimeError) return { error_class: error.constructor.name, failure_code: error.code };
  if (error instanceof Error && /^[A-Za-z0-9_.:-]+$/.test(error.constructor.name)) return { error_class: error.constructor.name, failure_code: "error" };
  return { error_class: "ConnectorOperationError", failure_code: "error" };
}

export interface AutonomousConnectorOperationInput {
  domain: AutonomousDomainName;
  capability: string;
  operation_id: string;
  /** Caller-owned subject identity. If omitted, a digest of the safe operation metadata is used. */
  subject_digest?: string;
  /** Metadata only. Values are transient and are never included in plans or durable errors. */
  request?: JsonObject;
  execution_id?: string;
  call_id?: string;
  attempt_id?: string | null;
  parent_digests?: readonly string[];
  /** Dispatch remains fail-closed unless this is explicitly true or the registration is non-approving. */
  approved?: boolean;
  selection_strategy?: AutonomousConnectorSelectionStrategy;
  selection_signals?: Readonly<Record<string, JsonObject>>;
}

export interface AutonomousConnectorOperationPlanJSON {
  schema: typeof AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA;
  domain: AutonomousDomainName;
  capability: string;
  operation_id: string;
  operation_digest: string;
  subject_digest: string;
  execution_id: string;
  call_id: string;
  attempt_id: string | null;
  parent_digests: string[];
  request_digest: string;
  selection_plan: JsonObject;
  selected_connector_id: string | null;
  status: "ready" | "connector_missing";
  approved: boolean;
  retention: "metadata_only_no_request_values";
  secret_material: "never_returned";
  plan_digest: string;
}

/** Digest-bound, request-free plan that can safely cross a persistence or review boundary. */
export class AutonomousConnectorOperationPlan {
  readonly domain: AutonomousDomainName;
  readonly capability: string;
  readonly operation_id: string;
  readonly operation_digest: string;
  readonly subject_digest: string;
  readonly execution_id: string;
  readonly call_id: string;
  readonly attempt_id: string | null;
  readonly parent_digests: string[];
  readonly request_digest: string;
  readonly selection_plan: AutonomousConnectorSelectionPlan;
  readonly selected_connector_id: string | null;
  readonly status: "ready" | "connector_missing";
  readonly approved: boolean;
  readonly plan_digest: string;

  constructor(input: {
    domain: AutonomousDomainName;
    capability: string;
    operation_id: string;
    operation_digest: string;
    subject_digest: string;
    execution_id: string;
    call_id: string;
    attempt_id: string | null;
    parent_digests: readonly string[];
    request_digest: string;
    selection_plan: AutonomousConnectorSelectionPlan;
    selected_connector_id: string | null;
    status: "ready" | "connector_missing";
    approved: boolean;
  }) {
    this.domain = domain("connector operation plan domain", input.domain);
    this.capability = capability("connector operation plan capability", input.capability);
    this.operation_id = identifier("connector operation plan operation_id", input.operation_id);
    this.operation_digest = digest("connector operation plan operation_digest", input.operation_digest);
    this.subject_digest = digest("connector operation plan subject_digest", input.subject_digest);
    this.execution_id = identifier("connector operation plan execution_id", input.execution_id);
    this.call_id = identifier("connector operation plan call_id", input.call_id);
    this.attempt_id = input.attempt_id === null ? null : identifier("connector operation plan attempt_id", input.attempt_id);
    this.parent_digests = [...input.parent_digests].map((entry) => digest("connector operation plan parent_digest", entry));
    this.request_digest = digest("connector operation plan request_digest", input.request_digest);
    if (!(input.selection_plan instanceof AutonomousConnectorSelectionPlan)) throw new ArgumentError("connector operation plan selection_plan is invalid");
    if (input.selection_plan.domains.length !== 1 || input.selection_plan.domains[0] !== this.domain || input.selection_plan.capability !== this.capability) throw new ArgumentError("connector operation plan selection does not match the operation");
    if (input.status !== "ready" && input.status !== "connector_missing") throw new ArgumentError("connector operation plan status is invalid");
    if (input.status === "ready" && input.selected_connector_id === null) throw new ArgumentError("ready connector operation plan requires a connector");
    if (input.status === "connector_missing" && input.selected_connector_id !== null) throw new ArgumentError("missing connector operation plan cannot select a connector");
    this.selection_plan = input.selection_plan;
    this.selected_connector_id = input.selected_connector_id === null ? null : identifier("connector operation plan selected_connector_id", input.selected_connector_id);
    this.status = input.status;
    if (typeof input.approved !== "boolean") throw new ArgumentError("connector operation plan approved must be boolean");
    this.approved = input.approved;
    this.plan_digest = digestJsonSync(this.descriptor());
  }

  static fromJSON(value: unknown): AutonomousConnectorOperationPlan {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError("connector operation plan projection is malformed");
    const raw = value as Partial<AutonomousConnectorOperationPlanJSON>;
    if (raw.schema !== AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA || raw.retention !== "metadata_only_no_request_values" || raw.secret_material !== "never_returned") throw new ArgumentError("connector operation plan projection metadata is invalid");
    if (!Array.isArray(raw.parent_digests)) throw new ArgumentError("connector operation plan parent_digests are invalid");
    const plan = new AutonomousConnectorOperationPlan({
      domain: raw.domain as AutonomousDomainName,
      capability: raw.capability as string,
      operation_id: raw.operation_id as string,
      operation_digest: raw.operation_digest as string,
      subject_digest: raw.subject_digest as string,
      execution_id: raw.execution_id as string,
      call_id: raw.call_id as string,
      attempt_id: raw.attempt_id ?? null,
      parent_digests: raw.parent_digests,
      request_digest: raw.request_digest as string,
      selection_plan: AutonomousConnectorSelectionPlan.fromJSON(raw.selection_plan),
      selected_connector_id: raw.selected_connector_id ?? null,
      status: raw.status as "ready" | "connector_missing",
      approved: raw.approved as boolean,
    });
    if (raw.plan_digest !== plan.plan_digest) throw new ArgumentError("connector operation plan projection digest is invalid");
    return plan;
  }

  private descriptor(): Omit<AutonomousConnectorOperationPlanJSON, "plan_digest"> {
    return {
      schema: AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
      domain: this.domain,
      capability: this.capability,
      operation_id: this.operation_id,
      operation_digest: this.operation_digest,
      subject_digest: this.subject_digest,
      execution_id: this.execution_id,
      call_id: this.call_id,
      attempt_id: this.attempt_id,
      parent_digests: [...this.parent_digests],
      request_digest: this.request_digest,
      selection_plan: this.selection_plan.toJSON(),
      selected_connector_id: this.selected_connector_id,
      status: this.status,
      approved: this.approved,
      retention: "metadata_only_no_request_values",
      secret_material: "never_returned",
    };
  }

  toJSON(): AutonomousConnectorOperationPlanJSON {
    return { ...this.descriptor(), plan_digest: this.plan_digest };
  }
}

export interface AutonomousConnectorOperationExecution {
  schema: typeof AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA;
  status: AutonomousConnectorDispatchStatus;
  operation_plan: AutonomousConnectorOperationPlanJSON;
  dispatch: AutonomousConnectorDispatchResult;
  replay: "fresh" | "replayed";
  retention: "operation_plan_metadata_only;dispatch_value_transient";
  secret_material: "never_returned";
}

export interface AutonomousConnectorOperationBatchItem {
  index: number;
  status: "succeeded" | "refused" | "failed" | "omitted";
  plan_digest: string | null;
  execution?: AutonomousConnectorOperationExecution;
  error_class?: string;
  failure_code?: string;
}

export interface AutonomousConnectorOperationBatchResult {
  schema: typeof AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA;
  status: "completed" | "partial" | "failed";
  items: AutonomousConnectorOperationBatchItem[];
  completed_count: number;
  failed_count: number;
  omitted_count: number;
  max_parallelism: number;
  stop_on_error: boolean;
  retention: "operation_plans_metadata_only;dispatch_values_transient";
  secret_material: "never_returned";
  batch_digest: string;
}

export interface AutonomousConnectorIntentRouteOptions {
  hints?: readonly string[];
  minConfidence?: number;
  minMargin?: number;
  maxDomains?: number;
  allowCrossDomain?: boolean;
}

export interface AutonomousConnectorIntentInput extends AutonomousConnectorIntentRouteOptions {
  task: string;
  requestByDomain?: Readonly<Record<string, JsonObject>>;
  capability?: string;
  approved?: boolean;
  selectionStrategy?: AutonomousConnectorSelectionStrategy;
  selectionSignals?: Readonly<Record<string, JsonObject>>;
}

export interface AutonomousConnectorIntentSelectionJSON {
  domain: AutonomousDomainName;
  operation_id: string;
  operation_digest: string;
  capability: string;
  score: number;
  matched_terms: string[];
  selection_reason: "caller_capability" | "exact_catalogue_terms" | "domain_default_capability";
  operation_plan: AutonomousConnectorOperationPlanJSON;
  retention: "metadata_only_task_and_request_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousConnectorIntentPlanJSON {
  schema: typeof AUTONOMOUS_CONNECTOR_INTENT_SCHEMA;
  task_digest: string;
  route: AutonomousRouteProposal;
  selected_domains: AutonomousDomainName[];
  cross_domain: boolean;
  status: "ready" | "route_review_required" | "connector_review_required";
  selections: AutonomousConnectorIntentSelectionJSON[];
  plan_digest: string;
  retention: "metadata_only_task_and_request_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousConnectorIntentExecution {
  schema: typeof AUTONOMOUS_CONNECTOR_INTENT_SCHEMA;
  status: "completed" | "partial" | "failed" | "route_review_required" | "connector_review_required";
  plan: AutonomousConnectorIntentPlanJSON;
  items: Array<{
    index: number;
    domain?: AutonomousDomainName;
    status: "succeeded" | "refused" | "failed" | "omitted";
    plan_digest: string | null;
    execution?: AutonomousConnectorOperationExecution;
    error_class?: string;
    failure_code?: string;
  }>;
  executions: AutonomousConnectorOperationExecution[];
  retention: "metadata_only_task_and_request_values;dispatch_values_transient";
  secret_material: "never_returned";
}

export interface AutonomousConnectorIntentJob extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA;
  job_id: string;
  plan_digest: string;
  status: "queued" | "route_review_required" | "connector_review_required" | "partial";
  items: Array<{
    index: number;
    domain: AutonomousDomainName;
    status: "queued" | "omitted";
    work_id: string | null;
    operation_plan_digest: string;
    queue_item_digest: string | null;
  }>;
  enqueued_count: number;
  omitted_count: number;
  retention: "metadata_only_task_request_plan_and_connector_values_not_retained";
  secret_material: "never_returned";
  job_digest: string;
}

export interface AutonomousConnectorIntentControllerProjection extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA;
  status: "empty" | "restored" | "flushed" | "submitted" | "executed";
  snapshot_digest: string | null;
  items: number;
  persisted: true;
  retention: "metadata_only_task_request_plan_connector_values_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousConnectorIntentControllerSubmission extends AutonomousConnectorIntentControllerProjection {
  status: "submitted";
  job: AutonomousConnectorIntentJob;
}

export interface AutonomousConnectorIntentControllerExecution extends AutonomousConnectorIntentControllerProjection {
  status: "executed";
  worker: AutonomousConnectorWorkerRun;
}

interface PreparedOperation {
  readonly operation: AutonomousConnectorOperationContract;
  readonly request: JsonObject;
  readonly dispatch: AutonomousConnectorDispatchRequest | null;
  readonly plan: AutonomousConnectorOperationPlan;
}

export class AutonomousConnectorOperationFacade {
  readonly registry: AutonomousConnectorRegistry;
  readonly runtime: AutonomousConnectorRuntime;
  readonly operationRegistry: AutonomousConnectorOperationRegistry;

  constructor(options: {
    registry: AutonomousConnectorRegistry;
    runtime: AutonomousConnectorRuntime;
    operationRegistry?: AutonomousConnectorOperationRegistry;
  }) {
    if (!options || !(options.registry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("connector operation facade requires an AutonomousConnectorRegistry");
    if (!(options.runtime instanceof AutonomousConnectorRuntime) || options.runtime.registry !== options.registry) throw new ArgumentError("connector operation facade runtime must use the same registry");
    this.registry = options.registry;
    this.runtime = options.runtime;
    this.operationRegistry = options.operationRegistry ?? new AutonomousConnectorOperationRegistry();
    if (!(this.operationRegistry instanceof AutonomousConnectorOperationRegistry)) throw new ArgumentError("connector operation facade operationRegistry is invalid");
  }

  /** Build a reviewable plan without invoking a connector. The returned plan contains no request values. */
  plan(input: AutonomousConnectorOperationInput): AutonomousConnectorOperationPlan {
    return this.prepare(input).plan;
  }

  /** Execute one reviewed operation through the selected connector and replay boundary. */
  async execute(input: AutonomousConnectorOperationInput): Promise<AutonomousConnectorOperationExecution> {
    const prepared = this.prepare(input);
    if (!prepared.dispatch || prepared.plan.status !== "ready") throw new ProviderRuntimeError("connector operation has no eligible connector", { code: "configuration" });
    return this.dispatch(prepared);
  }

  /** Rehydrate a metadata-only plan by resupplying transient request metadata and verify exact identity. */
  async executePlanned(plan: AutonomousConnectorOperationPlan, input: AutonomousConnectorOperationInput): Promise<AutonomousConnectorOperationExecution> {
    if (!(plan instanceof AutonomousConnectorOperationPlan)) throw new ArgumentError("connector operation executePlanned requires a typed plan");
    const prepared = this.prepare(input);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("connector operation plan does not match the supplied transient request");
    if (!prepared.dispatch) throw new ProviderRuntimeError("connector operation plan has no eligible connector", { code: "configuration" });
    return this.dispatch(prepared);
  }

  /** Rehydrate a reviewed operation plan into a transient worker request without dispatching. */
  prepareDispatch(plan: AutonomousConnectorOperationPlan, input: AutonomousConnectorOperationInput): { plan: AutonomousConnectorSelectionPlan; request: AutonomousConnectorDispatchRequest } {
    if (!(plan instanceof AutonomousConnectorOperationPlan)) throw new ArgumentError("connector operation prepareDispatch requires a typed plan");
    const prepared = this.prepare(input);
    if (prepared.plan.plan_digest !== plan.plan_digest) throw new ArgumentError("connector operation plan does not match the supplied transient request");
    if (!prepared.dispatch) throw new ProviderRuntimeError("connector operation plan has no eligible connector", { code: "configuration" });
    return { plan: prepared.plan.selection_plan, request: prepared.dispatch };
  }

  /** Execute independent operations with bounded concurrency and deterministic result ordering. */
  async executeBatch(inputs: readonly AutonomousConnectorOperationInput[], options: { maxParallelism?: number; stopOnError?: boolean } = {}): Promise<AutonomousConnectorOperationBatchResult> {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH) throw new ArgumentError(`connector operation batch must contain 1..=${MAX_AUTONOMOUS_CONNECTOR_FACADE_BATCH} entries`);
    const maxParallelism = options.maxParallelism ?? 4;
    if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM) throw new ArgumentError("connector operation batch maxParallelism is outside its bound");
    const stopOnError = options.stopOnError ?? false;
    if (typeof stopOnError !== "boolean") throw new ArgumentError("connector operation batch stopOnError must be boolean");
    const items: Array<AutonomousConnectorOperationBatchItem | undefined> = new Array(inputs.length);
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= inputs.length) return;
        if (halted) {
          items[index] = { index, status: "omitted", plan_digest: null };
          continue;
        }
        try {
          const execution = await this.execute(inputs[index]!);
          items[index] = { index, status: execution.status === "observed" || execution.status === "partial" ? "succeeded" : "refused", plan_digest: execution.operation_plan.plan_digest, execution };
          if (stopOnError && execution.status !== "observed" && execution.status !== "partial") halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, status: stopOnError ? "failed" : "refused", plan_digest: null, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, inputs.length) }, () => worker()));
    const completed = items.filter((item) => item?.status === "succeeded").length;
    const failed = items.filter((item) => item?.status === "refused" || item?.status === "failed").length;
    const omitted = items.filter((item) => item?.status === "omitted").length;
    const status = failed === 0 && omitted === 0 ? "completed" : completed > 0 ? "partial" : "failed";
    const normalizedItems = items.map((item, index) => item ?? { index, status: "failed" as const, plan_digest: null, error_class: "ConnectorOperationError", failure_code: "missing_batch_result" });
    return {
      schema: AUTONOMOUS_CONNECTOR_OPERATION_BATCH_SCHEMA,
      status,
      items: normalizedItems,
      completed_count: completed,
      failed_count: failed,
      omitted_count: omitted,
      max_parallelism: maxParallelism,
      stop_on_error: stopOnError,
      retention: "operation_plans_metadata_only;dispatch_values_transient",
      secret_material: "never_returned",
      batch_digest: digestJsonSync(normalizedItems.map((item) => ({ index: item.index, status: item.status, plan_digest: item.plan_digest, error_class: item.error_class ?? null, failure_code: item.failure_code ?? null, dispatch: item.execution?.dispatch.receipt.toJSON() ?? null }))),
    };
  }

  private prepare(input: AutonomousConnectorOperationInput): PreparedOperation {
    if (!input || typeof input !== "object" || Array.isArray(input)) throw new ArgumentError("connector operation input must be an object");
    const selectedDomain = domain("connector operation domain", input.domain);
    const selectedCapability = capability("connector operation capability", input.capability);
    const operationId = identifier("connector operation operation_id", input.operation_id);
    const operation = this.operationRegistry.resolve(operationId);
    if (operation.domain !== selectedDomain) throw new ArgumentError("connector operation domain does not match its operation contract");
    if (!operation.supports(selectedCapability)) throw new ArgumentError("connector operation capability is outside its operation contract");
    const suppliedRequest = input.request ?? {};
    if (!suppliedRequest || typeof suppliedRequest !== "object" || Array.isArray(suppliedRequest)) throw new ArgumentError("connector operation request must be a JSON object");
    assertSafeMetadata(suppliedRequest);
    if (suppliedRequest.operation_id !== undefined && suppliedRequest.operation_id !== operationId) throw new ArgumentError("connector operation request operation_id does not match the operation");
    if (suppliedRequest.subject_digest !== undefined && input.subject_digest !== undefined && suppliedRequest.subject_digest !== input.subject_digest) throw new ArgumentError("connector operation request subject_digest does not match the operation input");
    const withoutIdentity = Object.fromEntries(Object.entries(suppliedRequest).filter(([key]) => key !== "operation_id" && key !== "subject_digest"));
    const subjectDigest = input.subject_digest === undefined
      ? digestJsonSync({ schema: "bioprism-typescript-autonomous-connector-subject/0.1", domain: selectedDomain, operation_id: operationId, metadata: withoutIdentity })
      : digest("connector operation subject_digest", input.subject_digest);
    const request: JsonObject = { ...withoutIdentity, operation_id: operationId, subject_digest: subjectDigest };
    assertSafeMetadata(request);
    const selection = this.registry.selectForDomains([selectedDomain], { capability: selectedCapability, strategy: input.selection_strategy, selectionSignals: input.selection_signals });
    const row = selection.rows[0];
    if (!row) throw new ProviderRuntimeError("connector operation selection returned no domain row", { code: "configuration" });
    const parent = parentDigests(input.parent_digests);
    const attemptId = input.attempt_id === undefined || input.attempt_id === null ? null : identifier("connector operation attempt_id", input.attempt_id);
    const approved = input.approved ?? false;
    if (typeof approved !== "boolean") throw new ArgumentError("connector operation approved must be boolean");
    // Approval is part of the dispatch identity. A refused attempt must not make a later,
    // explicitly approved attempt look like a replay of the refusal; callers may still supply an
    // attempt_id when they need several approved retries with otherwise identical metadata.
    const identity = digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA, domain: selectedDomain, capability: selectedCapability, operation_id: operationId, subject_digest: subjectDigest, request, parent_digests: parent, attempt_id: attemptId, selection_plan_digest: selection.plan_digest, approved });
    const executionId = input.execution_id === undefined ? stableIdentity("connector-execution", identity) : identifier("connector operation execution_id", input.execution_id);
    const callId = input.call_id === undefined ? stableIdentity("connector-call", identity) : identifier("connector operation call_id", input.call_id);
    let dispatch: AutonomousConnectorDispatchRequest | null = null;
    let requestDigest = digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA, domain: selectedDomain, capability: selectedCapability, operation_id: operationId, subject_digest: subjectDigest, request });
    if (row.status === "selected" && row.connector_id !== null) {
      dispatch = new AutonomousConnectorDispatchRequest({ dispatch_id: stableIdentity("connector-dispatch", identity), execution_id: executionId, call_id: callId, connector_id: row.connector_id, domains: [selectedDomain], capability: selectedCapability, request, parent_digests: parent, attempt_id: attemptId, selection_plan_digest: selection.plan_digest, approved });
      operation.assertRequest(dispatch);
      requestDigest = dispatch.request_digest;
    }
    const plan = new AutonomousConnectorOperationPlan({ domain: selectedDomain, capability: selectedCapability, operation_id: operationId, operation_digest: operation.operation_digest, subject_digest: subjectDigest, execution_id: executionId, call_id: callId, attempt_id: attemptId, parent_digests: parent, request_digest: requestDigest, selection_plan: selection, selected_connector_id: row.status === "selected" ? row.connector_id : null, status: row.status === "selected" ? "ready" : "connector_missing", approved });
    return { operation, request, dispatch, plan };
  }

  private async dispatch(prepared: PreparedOperation): Promise<AutonomousConnectorOperationExecution> {
    if (!prepared.dispatch) throw new ProviderRuntimeError("connector operation dispatch is unavailable", { code: "configuration" });
    const dispatch = await this.runtime.dispatchFromPlan(prepared.plan.selection_plan, prepared.dispatch);
    return {
      schema: AUTONOMOUS_CONNECTOR_OPERATION_FACADE_SCHEMA,
      status: dispatch.receipt.status,
      operation_plan: prepared.plan.toJSON(),
      dispatch,
      replay: dispatch.replay,
      retention: "operation_plan_metadata_only;dispatch_value_transient",
      secret_material: "never_returned",
    };
  }
}

/**
 * Task-to-operation composition for ordinary autonomous applications.
 *
 * Routing is evidence-only: exact catalogue terms may select a reviewed operation and
 * capability, but they never authorize a connector. Approval, replay, and executor boundaries
 * remain inside AutonomousConnectorOperationFacade.
 */
export class AutonomousConnectorIntentFacade {
  readonly operationFacade: AutonomousConnectorOperationFacade;
  readonly route: (task: string, options: AutonomousConnectorIntentRouteOptions) => Promise<AutonomousRouteProposal>;

  constructor(options: {
    operationFacade: AutonomousConnectorOperationFacade;
    route?: (task: string, options: AutonomousConnectorIntentRouteOptions) => Promise<AutonomousRouteProposal>;
  }) {
    if (!options || !(options.operationFacade instanceof AutonomousConnectorOperationFacade)) throw new ArgumentError("connector intent facade requires an operation facade");
    this.operationFacade = options.operationFacade;
    this.route = options.route ?? routeAutonomousTask;
  }

  async plan(input: AutonomousConnectorIntentInput): Promise<AutonomousConnectorIntentPlanJSON> {
    if (!input || typeof input !== "object" || Array.isArray(input)) throw new ArgumentError("connector intent input must be an object");
    if (typeof input.task !== "string" || !input.task.trim() || bytes(input.task) > MAX_AUTONOMOUS_CONNECTOR_INTENT_TASK_BYTES) throw new ArgumentError("connector intent task is outside its bound");
    const hints = input.hints ?? [];
    if (!Array.isArray(hints) || hints.length > MAX_AUTONOMOUS_CONNECTOR_INTENT_HINTS || hints.some((hint) => typeof hint !== "string" || !hint.trim() || bytes(hint) > 256)) throw new ArgumentError("connector intent hints are outside their bound");
    if (input.requestByDomain !== undefined) {
      if (!input.requestByDomain || typeof input.requestByDomain !== "object" || Array.isArray(input.requestByDomain)) throw new ArgumentError("connector intent requestByDomain must be an object");
      for (const [key, value] of Object.entries(input.requestByDomain)) {
        domain("connector intent request domain", key);
        if (!value || typeof value !== "object" || Array.isArray(value)) throw new ArgumentError("connector intent request metadata must be an object");
        assertSafeMetadata(value);
      }
    }
    const route = await this.route(input.task, {
      hints,
      minConfidence: input.minConfidence,
      minMargin: input.minMargin,
      maxDomains: input.maxDomains,
      allowCrossDomain: input.allowCrossDomain,
    });
    if (!route || typeof route !== "object") throw new ArgumentError("connector intent route is invalid");
    if (route.abstained || route.selected_domains.length === 0) {
      const descriptor = {
        schema: AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
        task_digest: route.task_digest,
        route: structuredClone(route),
        selected_domains: [],
        cross_domain: false,
        status: "route_review_required" as const,
        selections: [],
        retention: "metadata_only_task_and_request_values_not_retained" as const,
        secret_material: "never_returned" as const,
      };
      return { ...descriptor, plan_digest: digestJsonSync(descriptor) };
    }
    const text = `${input.task} ${hints.join(" ")}`;
    const selections: AutonomousConnectorIntentSelectionJSON[] = [];
    for (const selectedDomain of route.selected_domains) {
      const operations = this.operationFacade.operationRegistry.forDomain(selectedDomain);
      if (!operations.length) throw new ArgumentError(`no connector operation contract is registered for ${selectedDomain}`);
      const scored = operations.map((operation) => ({ operation, intent: selectIntentCapability(operation, text, input.capability) }));
      scored.sort((left, right) => right.intent.score - left.intent.score || left.operation.operation_id.localeCompare(right.operation.operation_id));
      const selected = scored[0]!;
      const operationPlan = this.operationFacade.plan({
        domain: selectedDomain,
        capability: selected.intent.capability,
        operation_id: selected.operation.operation_id,
        request: input.requestByDomain?.[selectedDomain] ?? {},
        approved: input.approved ?? false,
        selection_strategy: input.selectionStrategy,
        selection_signals: input.selectionSignals,
      });
      selections.push({
        domain: selectedDomain,
        operation_id: selected.operation.operation_id,
        operation_digest: selected.operation.operation_digest,
        capability: selected.intent.capability,
        score: selected.intent.score,
        matched_terms: selected.intent.matchedTerms,
        selection_reason: selected.intent.reason as AutonomousConnectorIntentSelectionJSON["selection_reason"],
        operation_plan: operationPlan.toJSON(),
        retention: "metadata_only_task_and_request_values_not_retained",
        secret_material: "never_returned",
      });
    }
    const status = selections.some((selection) => selection.operation_plan.status !== "ready")
      ? "connector_review_required" as const
      : "ready" as const;
    const descriptor = {
      schema: AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
      task_digest: route.task_digest,
      route: structuredClone(route),
      selected_domains: [...route.selected_domains],
      cross_domain: route.selected_domains.length > 1,
      status,
      selections,
      retention: "metadata_only_task_and_request_values_not_retained" as const,
      secret_material: "never_returned" as const,
    };
    return { ...descriptor, plan_digest: digestJsonSync(descriptor) };
  }

  async execute(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput,
    options: { maxParallelism?: number; stopOnError?: boolean } = {},
  ): Promise<AutonomousConnectorIntentExecution> {
    if (!plan || typeof plan !== "object" || typeof plan.plan_digest !== "string") throw new ArgumentError("connector intent execute requires a typed plan projection");
    const current = await this.plan(input);
    if (current.plan_digest !== plan.plan_digest) throw new ArgumentError("connector intent plan does not match the supplied transient task metadata");
    if (current.status !== "ready") {
      return {
        schema: AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
        status: current.status,
        plan: current,
        items: current.selections.map((selection, index) => ({ index, domain: selection.domain, status: "omitted" as const, plan_digest: selection.operation_plan.plan_digest })),
        executions: [],
        retention: "metadata_only_task_and_request_values;dispatch_values_transient",
        secret_material: "never_returned",
      };
    }
    const maxParallelism = options.maxParallelism ?? Math.min(4, current.selections.length);
    if (!Number.isSafeInteger(maxParallelism) || maxParallelism < 1 || maxParallelism > MAX_AUTONOMOUS_CONNECTOR_FACADE_PARALLELISM) throw new ArgumentError("connector intent maxParallelism is outside its bound");
    const stopOnError = options.stopOnError ?? true;
    if (typeof stopOnError !== "boolean") throw new ArgumentError("connector intent stopOnError must be boolean");
    const items: Array<AutonomousConnectorIntentExecution["items"][number] | undefined> = new Array(current.selections.length);
    const executions: AutonomousConnectorOperationExecution[] = [];
    let nextIndex = 0;
    let halted = false;
    const worker = async (): Promise<void> => {
      while (true) {
        const index = nextIndex++;
        if (index >= current.selections.length) return;
        const selection = current.selections[index]!;
        if (halted) {
          items[index] = { index, domain: selection.domain, status: "omitted", plan_digest: selection.operation_plan.plan_digest };
          continue;
        }
        try {
          const execution = await this.operationFacade.executePlanned(
            AutonomousConnectorOperationPlan.fromJSON(selection.operation_plan),
            {
              domain: selection.domain,
              capability: selection.capability,
              operation_id: selection.operation_id,
              request: input.requestByDomain?.[selection.domain] ?? {},
              approved: input.approved ?? false,
              selection_strategy: input.selectionStrategy,
              selection_signals: input.selectionSignals,
            },
          );
          const succeeded = execution.status === "observed" || execution.status === "partial";
          items[index] = { index, domain: selection.domain, status: succeeded ? "succeeded" : "refused", plan_digest: selection.operation_plan.plan_digest, execution };
          executions.push(execution);
          if (stopOnError && !succeeded) halted = true;
        } catch (error) {
          const projection = errorProjection(error);
          items[index] = { index, domain: selection.domain, status: "failed", plan_digest: selection.operation_plan.plan_digest, ...projection };
          if (stopOnError) halted = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(maxParallelism, current.selections.length) }, () => worker()));
    const normalized = items.map((item, index) => item ?? { index, status: "failed" as const, plan_digest: null, error_class: "ConnectorOperationError", failure_code: "missing_result" });
    const failures = normalized.filter((item) => item.status === "refused" || item.status === "failed").length;
    const omitted = normalized.filter((item) => item.status === "omitted").length;
    const status = failures === 0 && omitted === 0 ? "completed" as const : executions.length > 0 ? "partial" as const : "failed" as const;
    return {
      schema: AUTONOMOUS_CONNECTOR_INTENT_SCHEMA,
      status,
      plan: current,
      items: normalized,
      executions,
      retention: "metadata_only_task_and_request_values;dispatch_values_transient",
      secret_material: "never_returned",
    };
  }

  /** Submit a reviewed intent as metadata-only, restart-safe connector work items. */
  enqueue(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput & { jobId: string },
    queue: InMemoryAutonomousConnectorWorkQueue,
    options: { maxAttempts?: number; now?: number } = {},
  ): Promise<AutonomousConnectorIntentJob> {
    return this.enqueueInternal(plan, input, queue, options);
  }

  private async enqueueInternal(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput & { jobId: string },
    queue: InMemoryAutonomousConnectorWorkQueue,
    options: { maxAttempts?: number; now?: number },
  ): Promise<AutonomousConnectorIntentJob> {
    if (!plan || typeof plan !== "object" || typeof plan.plan_digest !== "string") throw new ArgumentError("connector intent enqueue requires a typed plan projection");
    if (!(queue instanceof InMemoryAutonomousConnectorWorkQueue)) throw new ArgumentError("connector intent enqueue requires a typed work queue");
    const jobId = identifier("connector intent jobId", input.jobId);
    const current = await this.plan(input);
    if (current.plan_digest !== plan.plan_digest) throw new ArgumentError("connector intent job plan does not match the supplied transient task metadata");
    if (current.selections.length > MAX_AUTONOMOUS_CONNECTOR_INTENT_JOB_ITEMS) throw new ArgumentError("connector intent job contains too many selections");
    const descriptor = (status: AutonomousConnectorIntentJob["status"], items: AutonomousConnectorIntentJob["items"], enqueued: number, omitted: number): AutonomousConnectorIntentJob => {
      const base = {
        schema: AUTONOMOUS_CONNECTOR_INTENT_JOB_SCHEMA,
        job_id: jobId,
        plan_digest: current.plan_digest,
        status,
        items,
        enqueued_count: enqueued,
        omitted_count: omitted,
        retention: "metadata_only_task_request_plan_and_connector_values_not_retained" as const,
        secret_material: "never_returned" as const,
      };
      return { ...base, job_digest: digestJsonSync(base) };
    };
    if (current.status !== "ready") {
      const items = current.selections.map((selection, index) => ({ index, domain: selection.domain, status: "omitted" as const, work_id: null, operation_plan_digest: selection.operation_plan.plan_digest, queue_item_digest: null }));
      return descriptor(current.status, items, 0, items.length);
    }
    const items: AutonomousConnectorIntentJob["items"] = [];
    for (let index = 0; index < current.selections.length; index += 1) {
      const selection = current.selections[index]!;
      const prepared = this.operationFacade.prepareDispatch(AutonomousConnectorOperationPlan.fromJSON(selection.operation_plan), {
        domain: selection.domain,
        capability: selection.capability,
        operation_id: selection.operation_id,
        request: input.requestByDomain?.[selection.domain] ?? {},
        approved: input.approved ?? false,
        selection_strategy: input.selectionStrategy,
        selection_signals: input.selectionSignals,
      });
      const workId = `${jobId}-${index}`;
      const queued = queue.enqueue({
        work_id: workId,
        operation_id: selection.operation_id,
        request: prepared.request,
        selection_plan_digest: prepared.plan.plan_digest,
        max_attempts: options.maxAttempts ?? 3,
        now: options.now,
      });
      items.push({ index, domain: selection.domain, status: "queued", work_id: queued.work_id, operation_plan_digest: selection.operation_plan.plan_digest, queue_item_digest: queued.item_digest });
    }
    return descriptor("queued", items, items.length, 0);
  }

  /** Recover and execute queued intent work after transient task metadata is re-supplied. */
  async runQueued(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput & { jobId: string },
    queue: InMemoryAutonomousConnectorWorkQueue,
    options: { workerId?: string; limit?: number; leaseMs?: number; now?: number } = {},
  ): Promise<AutonomousConnectorWorkerRun> {
    if (!plan || typeof plan !== "object" || typeof plan.plan_digest !== "string") throw new ArgumentError("connector intent runQueued requires a typed plan projection");
    if (!(queue instanceof InMemoryAutonomousConnectorWorkQueue)) throw new ArgumentError("connector intent runQueued requires a typed work queue");
    const jobId = identifier("connector intent jobId", input.jobId);
    const current = await this.plan(input);
    if (current.plan_digest !== plan.plan_digest) throw new ArgumentError("connector intent worker plan does not match the supplied transient task metadata");
    const selections = new Map(current.selections.map((selection, index) => [`${jobId}-${index}`, selection]));
    const worker = new AutonomousConnectorWorker(this.operationFacade.runtime, queue, (item: AutonomousConnectorWorkItem) => {
      const selection = selections.get(item.work_id);
      if (!selection || selection.domain !== item.domain || selection.operation_id !== item.operation_id) throw new ArgumentError("connector intent worker item is outside the reviewed plan");
      const prepared = this.operationFacade.prepareDispatch(AutonomousConnectorOperationPlan.fromJSON(selection.operation_plan), {
        domain: selection.domain,
        capability: selection.capability,
        operation_id: selection.operation_id,
        request: input.requestByDomain?.[selection.domain] ?? {},
        approved: input.approved ?? false,
        selection_strategy: input.selectionStrategy,
        selection_signals: input.selectionSignals,
      });
      if (prepared.request.request_digest !== item.request_digest || prepared.plan.plan_digest !== item.selection_plan_digest) throw new ArgumentError("connector intent worker item identity does not match the reviewed plan");
      return { plan: prepared.plan, request: prepared.request };
    });
    return worker.run({ ...options, workIds: [...selections.keys()] });
  }
}

/**
 * Application-facing restart boundary for intent jobs.
 *
 * This controller makes the safe lifecycle hard to misuse: restore is explicit at startup,
 * every accepted submission is flushed as one verified metadata-only snapshot, worker state is
 * flushed even when a transient rehydrator throws, and partial enqueue attempts restore the
 * previous queue image. It never stores task text, request values, connector payloads, or keys.
 */
export class AutonomousConnectorIntentJobController {
  readonly persistence: AutonomousConnectorWorkQueuePersistenceCoordinator;
  private restored = false;

  constructor(
    readonly intent: AutonomousConnectorIntentFacade,
    readonly queue: InMemoryAutonomousConnectorWorkQueue,
    persistence: AutonomousConnectorWorkQueuePersistence,
  ) {
    if (!(intent instanceof AutonomousConnectorIntentFacade)) throw new ArgumentError("connector intent job controller requires an intent facade");
    if (!(queue instanceof InMemoryAutonomousConnectorWorkQueue)) throw new ArgumentError("connector intent job controller requires a typed work queue");
    this.persistence = new AutonomousConnectorWorkQueuePersistenceCoordinator(queue, persistence);
  }

  private requireRestored(): void {
    if (!this.restored) throw new ArgumentError("connector intent job controller must restore before enqueue or execution");
  }

  private projection<S extends AutonomousConnectorIntentControllerProjection["status"]>(status: S, snapshotDigest: string | null, items: number): AutonomousConnectorIntentControllerProjection & { status: S } {
    return {
      schema: AUTONOMOUS_CONNECTOR_INTENT_CONTROLLER_SCHEMA,
      status,
      snapshot_digest: snapshotDigest,
      items,
      persisted: true,
      retention: "metadata_only_task_request_plan_connector_values_not_retained",
      secret_material: "never_returned",
    };
  }

  async restore(): Promise<AutonomousConnectorIntentControllerProjection> {
    const result = await this.persistence.restore();
    this.restored = true;
    return this.projection(result.status, result.snapshot_digest, result.items);
  }

  async flush(): Promise<AutonomousConnectorIntentControllerProjection> {
    this.requireRestored();
    const snapshot = await this.persistence.flush();
    return this.projection("flushed", snapshot.snapshot_digest, snapshot.items.length);
  }

  async enqueue(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput & { jobId: string },
    options: { maxAttempts?: number; now?: number } = {},
  ): Promise<AutonomousConnectorIntentControllerSubmission> {
    this.requireRestored();
    if (!input || typeof input !== "object" || typeof input.jobId !== "string") throw new ArgumentError("connector intent controller input requires jobId");
    const before = this.queue.snapshot();
    try {
      const job = await this.intent.enqueue(plan, input, this.queue, options);
      const snapshot = await this.persistence.flush();
      return { ...this.projection("submitted", snapshot.snapshot_digest, snapshot.items.length), job };
    } catch (error) {
      this.queue.restore(before);
      try {
        await this.persistence.persistence.write(before);
      } catch {
        // Preserve the original error. The in-process queue is restored and the caller can
        // retry or surface the persistence adapter's own I/O failure explicitly.
      }
      throw error;
    }
  }

  async runQueued(
    plan: AutonomousConnectorIntentPlanJSON,
    input: AutonomousConnectorIntentInput & { jobId: string },
    options: { workerId?: string; limit?: number; leaseMs?: number; now?: number } = {},
  ): Promise<AutonomousConnectorIntentControllerExecution> {
    this.requireRestored();
    if (!input || typeof input !== "object" || typeof input.jobId !== "string") throw new ArgumentError("connector intent controller input requires jobId");
    let worker: AutonomousConnectorWorkerRun | null = null;
    let workerError: unknown = null;
    try {
      worker = await this.intent.runQueued(plan, input, this.queue, options);
    } catch (error) {
      workerError = error;
    }
    const snapshot = await this.persistence.flush();
    if (workerError !== null) throw workerError;
    if (worker === null) throw new ArgumentError("connector intent worker returned no result");
    return { ...this.projection("executed", snapshot.snapshot_digest, snapshot.items.length), worker };
  }
}

export function createAutonomousConnectorIntentFacade(options: {
  operationFacade: AutonomousConnectorOperationFacade;
  route?: (task: string, options: AutonomousConnectorIntentRouteOptions) => Promise<AutonomousRouteProposal>;
}): AutonomousConnectorIntentFacade {
  return new AutonomousConnectorIntentFacade(options);
}

/** Convenience constructor for the runtime returned by createBuiltinAutonomousConnectorRuntime. */
export function createAutonomousConnectorOperationFacade(options: {
  registry: AutonomousConnectorRegistry;
  runtime: AutonomousConnectorRuntime;
  operationRegistry?: AutonomousConnectorOperationRegistry;
}): AutonomousConnectorOperationFacade {
  return new AutonomousConnectorOperationFacade(options);
}
