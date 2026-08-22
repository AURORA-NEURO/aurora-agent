import { ArgumentError, ProviderRuntimeError } from "./errors.js";
import { AUTONOMOUS_DOMAIN_NAMES, type AutonomousDomainName } from "./autonomous.js";
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

/** Convenience constructor for the runtime returned by createBuiltinAutonomousConnectorRuntime. */
export function createAutonomousConnectorOperationFacade(options: {
  registry: AutonomousConnectorRegistry;
  runtime: AutonomousConnectorRuntime;
  operationRegistry?: AutonomousConnectorOperationRegistry;
}): AutonomousConnectorOperationFacade {
  return new AutonomousConnectorOperationFacade(options);
}
