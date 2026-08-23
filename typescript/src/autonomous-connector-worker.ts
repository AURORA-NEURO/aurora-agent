import { ArgumentError, isObject } from "./errors.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  type AutonomousDomainName,
} from "./autonomous.js";
import {
  AutonomousConnectorDispatchReceipt,
  AutonomousConnectorDispatchRequest,
  AutonomousConnectorRegistry,
  AutonomousConnectorRuntime,
  AutonomousConnectorSelectionPlan,
  type AutonomousConnectorDispatchResult,
} from "./autonomous-connectors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/**
 * Durable connector work is intentionally a separate abstraction from the connector
 * runtime. The runtime owns a transient caller executor; this module owns only the
 * replayable identities around that executor. A durable store can therefore persist
 * these records without becoming a credential store, a request archive, or an
 * accidental source of evaluator reward.
 */
export const AUTONOMOUS_CONNECTOR_OPERATION_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-connector-operation-registry/0.1" as const;
export const AUTONOMOUS_CONNECTOR_OPERATION_SCHEMA = "bioprism-typescript-autonomous-connector-operation/0.1" as const;
export const AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA = "bioprism-typescript-autonomous-connector-work-item/0.1" as const;
export const AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA = "bioprism-typescript-autonomous-connector-work-queue/0.1" as const;
export const AUTONOMOUS_CONNECTOR_WORKER_SCHEMA = "bioprism-typescript-autonomous-connector-worker/0.1" as const;
export const AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA = "bioprism-typescript-autonomous-connector-feedback/0.1" as const;
export const AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA = "bioprism-typescript-autonomous-connector-feedback-ledger/0.1" as const;

export const MAX_AUTONOMOUS_CONNECTOR_OPERATIONS = 128;
export const MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS = 4_096;
export const MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS = 32;
export const MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH = 128;
export const MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS = 600_000;
export const MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES = 8_000_000;
export const MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES = 20_000;
export const MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_SNAPSHOT_BYTES = 8_000_000;

export type AutonomousConnectorWorkStatus =
  | "queued"
  | "leased"
  | "completed"
  | "failed"
  | "reconciliation_required"
  | "cancelled";

export type AutonomousConnectorWorkFailureClass =
  | "rehydration_missing"
  | "rehydration_invalid"
  | "identity_conflict"
  | "lease_expired"
  | "approval_required"
  | "domain_scope"
  | "capability_scope"
  | "executor_error"
  | "transport_error"
  | "unknown";

export type AutonomousConnectorOperationRisk = "read_only" | "side_effecting" | "human_review";

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function boundedText(name: string, value: unknown, maximum = 512): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || bytes(value) > maximum) throw new ArgumentError(`${name} is outside its bounded text contract`);
  return value;
}

function identifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return text;
}

function capabilityValue(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded capability identifier`);
  return text;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function timestamp(name: string, value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 8_640_000_000_000_000) throw new ArgumentError(`${name} must be a bounded epoch millisecond timestamp`);
  return value as number;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer between ${minimum} and ${maximum}`);
  return value as number;
}

function finiteNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be between ${minimum} and ${maximum}`);
  return value;
}

function arrayOfDigests(name: string, value: unknown, maximum = 128): string[] {
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must contain at most ${maximum} entries`);
  return value.map((entry) => digest(`${name} entry`, entry) as string);
}

function arrayOfIdentifiers(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  const result = value.map((entry) => identifier(`${name} entry`, entry));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function arrayOfCapabilities(name: string, value: unknown, maximum: number): string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  const result = value.map((entry) => capabilityValue(`${name} entry`, entry));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function domain(name: string, value: unknown): AutonomousDomainName {
  if (typeof value !== "string" || !AUTONOMOUS_DOMAIN_NAMES.includes(value as AutonomousDomainName)) throw new ArgumentError(`${name} is not a supported autonomous domain`);
  return value as AutonomousDomainName;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function nowMs(value: number | undefined): number {
  return timestamp("time", value ?? Date.now());
}

function operationDigestPayload(operation: AutonomousConnectorOperationContract): JsonObject {
  return {
    schema: AUTONOMOUS_CONNECTOR_OPERATION_SCHEMA,
    operation_id: operation.operation_id,
    domain: operation.domain,
    capabilities: [...operation.capabilities],
    description: operation.description,
    request_fields: [...operation.request_fields],
    evaluator_signals: [...operation.evaluator_signals],
    risk_class: operation.risk_class,
  };
}

/** A bounded, domain-specific vocabulary for connector work. */
export class AutonomousConnectorOperationContract {
  readonly operation_id: string;
  readonly domain: AutonomousDomainName;
  readonly capabilities: string[];
  readonly description: string;
  readonly request_fields: string[];
  readonly evaluator_signals: string[];
  readonly risk_class: AutonomousConnectorOperationRisk;
  readonly operation_digest: string;

  constructor(input: {
    operation_id: string;
    domain: AutonomousDomainName;
    capabilities: readonly string[];
    description: string;
    request_fields?: readonly string[];
    evaluator_signals: readonly string[];
    risk_class?: AutonomousConnectorOperationRisk;
  }) {
    this.operation_id = identifier("autonomous connector operation_id", input.operation_id);
    this.domain = domain("autonomous connector operation domain", input.domain);
    this.capabilities = arrayOfCapabilities("autonomous connector operation capabilities", input.capabilities, 128);
    this.description = boundedText("autonomous connector operation description", input.description, 1_024);
    this.request_fields = input.request_fields === undefined ? ["operation_id"] : arrayOfIdentifiers("autonomous connector operation request_fields", input.request_fields, 64);
    if (!this.request_fields.includes("operation_id")) throw new ArgumentError("autonomous connector operation request_fields must include operation_id");
    this.evaluator_signals = arrayOfIdentifiers("autonomous connector operation evaluator_signals", input.evaluator_signals, 64);
    const riskClass = input.risk_class ?? "read_only";
    if (riskClass !== "read_only" && riskClass !== "side_effecting" && riskClass !== "human_review") throw new ArgumentError("autonomous connector operation risk_class is invalid");
    this.risk_class = riskClass;
    this.operation_digest = digestJsonSync(operationDigestPayload(this));
  }

  supports(capability: string): boolean {
    return this.capabilities.includes(capabilityValue("autonomous connector operation capability", capability));
  }

  assertRequest(request: AutonomousConnectorDispatchRequest): void {
    if (!(request instanceof AutonomousConnectorDispatchRequest)) throw new ArgumentError("autonomous connector operation request must be typed");
    if (request.domains.length !== 1 || request.domains[0] !== this.domain) throw new ArgumentError("autonomous connector operation request must target exactly its contract domain");
    if (!this.supports(request.capability)) throw new ArgumentError("autonomous connector operation capability is outside the contract");
    for (const field of this.request_fields) {
      if (!(field in request.request)) throw new ArgumentError(`autonomous connector operation request is missing ${field}`);
    }
    if (request.request.operation_id !== this.operation_id) throw new ArgumentError("autonomous connector operation request operation_id does not match the contract");
  }

  toJSON(): JsonObject {
    return {
      ...operationDigestPayload(this),
      operation_digest: this.operation_digest,
      retention: "metadata_only_contract_no_request_values",
      secret_material: "never_returned",
    };
  }
}

function capabilities(...values: string[]): string[] {
  return [...new Set(values)];
}

/**
 * The default operation vocabulary covers every built-in autonomous domain and the
 * composite stage capabilities emitted by the workflow profiles. Applications may
 * add narrower operations, but they cannot silently omit a domain from this registry.
 */
export function defaultAutonomousConnectorOperationContracts(): AutonomousConnectorOperationContract[] {
  return [
    new AutonomousConnectorOperationContract({ operation_id: "coding.repository_change_analysis", domain: "coding", capabilities: capabilities("review", "debugging", "implementation", "testing", "review+debugging", "review+implementation", "review+testing"), description: "Inspect repository state and return caller-owned change observations.", request_fields: ["operation_id"], evaluator_signals: ["correctness", "testability", "reproducibility"] }),
    new AutonomousConnectorOperationContract({ operation_id: "browser.web_evidence_retrieval", domain: "browser", capabilities: capabilities("web_research", "navigation", "source_comparison", "web_research+navigation", "web_research+source_comparison"), description: "Acquire bounded web evidence through a caller-managed browser connector.", request_fields: ["operation_id"], evaluator_signals: ["source_quality", "citation_completeness", "freshness"] }),
    new AutonomousConnectorOperationContract({ operation_id: "data.dataset_quality_profile", domain: "data", capabilities: capabilities("schema_validation", "lineage", "quality_control", "data_analysis", "quality_control+data_analysis", "data_analysis+schema_validation"), description: "Profile a caller-owned dataset and expose quality or lineage observations.", request_fields: ["operation_id"], evaluator_signals: ["schema_validity", "lineage_completeness", "quality"] }),
    new AutonomousConnectorOperationContract({ operation_id: "science.reproducible_evidence_acquisition", domain: "science", capabilities: capabilities("hypothesis", "literature", "statistics", "experiment", "reproducibility", "hypothesis+statistics", "experiment+statistics"), description: "Acquire and align scientific evidence under explicit reproducibility boundaries.", request_fields: ["operation_id"], evaluator_signals: ["evidence_strength", "reproducibility", "uncertainty"] }),
    new AutonomousConnectorOperationContract({ operation_id: "biomedical.clinical_data_review", domain: "biomedical", capabilities: capabilities("biomedical_review", "safety_boundary", "provenance", "human_review", "biomedical_review+safety_boundary"), description: "Review biomedical evidence with provenance, safety, and human-review boundaries.", request_fields: ["operation_id"], evaluator_signals: ["safety", "provenance", "review_completeness"], risk_class: "human_review" }),
    new AutonomousConnectorOperationContract({ operation_id: "neuroscience.signal_study_analysis", domain: "neuroscience", capabilities: capabilities("neuroscience_analysis", "signal_interpretation", "study_design", "reproducibility", "neuroscience_analysis+signal_interpretation", "study_design+reproducibility"), description: "Analyze neuroscience signal or study evidence without retaining raw participant data.", request_fields: ["operation_id"], evaluator_signals: ["signal_quality", "study_design", "reproducibility"] }),
    new AutonomousConnectorOperationContract({ operation_id: "operations.incident_runbook_observation", domain: "operations", capabilities: capabilities("observability", "incident_response", "risk_review", "rollback", "approval", "runbook", "observability+incident_response"), description: "Observe operational incidents and runbooks while leaving mutation authorization to the caller.", request_fields: ["operation_id"], evaluator_signals: ["incident_completeness", "risk_containment", "runbook_alignment"], risk_class: "side_effecting" }),
    new AutonomousConnectorOperationContract({ operation_id: "enterprise.workflow_record_governance", domain: "enterprise", capabilities: capabilities("workflow", "coordination", "governance", "compliance", "analytics", "workflow+coordination", "governance+compliance", "analytics+governance", "governance+analytics"), description: "Inspect enterprise workflow records for governance, compliance, and coordination evidence.", request_fields: ["operation_id"], evaluator_signals: ["policy_compliance", "workflow_integrity", "record_completeness"] }),
    new AutonomousConnectorOperationContract({ operation_id: "multi_agent.delegated_consensus_handoff", domain: "multi_agent", capabilities: capabilities("delegation", "coordination", "consensus", "conflict_resolution", "handoff", "delegation+coordination", "consensus+conflict_resolution", "handoff+coordination"), description: "Coordinate delegated agent evidence and handoffs without granting implicit authority.", request_fields: ["operation_id"], evaluator_signals: ["delegation_quality", "consensus", "handoff_integrity"] }),
    new AutonomousConnectorOperationContract({ operation_id: "multimodal.asset_alignment", domain: "multimodal", capabilities: capabilities("document", "cross_modal_alignment", "image", "audio", "video", "document+cross_modal_alignment", "image+audio+video+document"), description: "Align caller-owned document, image, audio, and video observations by digest.", request_fields: ["operation_id"], evaluator_signals: ["modality_support", "alignment_quality", "comparability"] }),
    new AutonomousConnectorOperationContract({ operation_id: "cross_domain.evidence_fanout_synthesis", domain: "cross_domain", capabilities: capabilities("routing", "synthesis", "evidence_alignment", "workflow_composition", "routing+synthesis"), description: "Fan out bounded evidence work and synthesize cross-domain metadata.", request_fields: ["operation_id"], evaluator_signals: ["coverage", "alignment", "synthesis_quality"] }),
    new AutonomousConnectorOperationContract({ operation_id: "evaluation.benchmark_replay_analysis", domain: "evaluation", capabilities: capabilities("rubric", "benchmarking", "replay", "failure_analysis", "reproducibility"), description: "Run evaluator-owned benchmark and replay analysis over metadata-only outcomes.", request_fields: ["operation_id"], evaluator_signals: ["benchmark_integrity", "replay_fidelity", "failure_coverage"] }),
  ];
}

export class AutonomousConnectorOperationRegistry {
  private readonly contracts = new Map<string, AutonomousConnectorOperationContract>();

  constructor(contracts: readonly AutonomousConnectorOperationContract[] = defaultAutonomousConnectorOperationContracts()) {
    if (!Array.isArray(contracts) || contracts.length < 1 || contracts.length > MAX_AUTONOMOUS_CONNECTOR_OPERATIONS) throw new ArgumentError("autonomous connector operation registry size is outside its bound");
    for (const contract of contracts) this.add(contract);
    this.assertCoverage();
  }

  register(contract: AutonomousConnectorOperationContract, options: { replace?: boolean } = {}): AutonomousConnectorOperationContract {
    const previous = this.contracts.get(contract instanceof AutonomousConnectorOperationContract ? contract.operation_id : "");
    this.add(contract, options.replace === true);
    try {
      this.assertCoverage();
    } catch (error) {
      if (previous) this.contracts.set(previous.operation_id, previous);
      else this.contracts.delete(contract.operation_id);
      throw error;
    }
    return contract;
  }

  private add(contract: AutonomousConnectorOperationContract, replace = false): void {
    if (!(contract instanceof AutonomousConnectorOperationContract)) throw new ArgumentError("autonomous connector operation contract is invalid");
    if (this.contracts.has(contract.operation_id) && !replace) throw new ArgumentError(`autonomous connector operation is already registered: ${contract.operation_id}`);
    if (!this.contracts.has(contract.operation_id) && this.contracts.size >= MAX_AUTONOMOUS_CONNECTOR_OPERATIONS) throw new ArgumentError("autonomous connector operation registry is full");
    this.contracts.set(contract.operation_id, contract);
  }

  private assertCoverage(): void {
    const covered = new Set([...this.contracts.values()].map((contract) => contract.domain));
    if (covered.size !== AUTONOMOUS_DOMAIN_NAMES.length || AUTONOMOUS_DOMAIN_NAMES.some((name) => !covered.has(name))) throw new ArgumentError("autonomous connector operation registry must cover every autonomous domain");
  }

  resolve(operationId: string): AutonomousConnectorOperationContract {
    const operation = this.contracts.get(identifier("autonomous connector operation_id", operationId));
    if (!operation) throw new ArgumentError(`autonomous connector operation is not registered: ${operationId}`);
    return operation;
  }

  operations(): AutonomousConnectorOperationContract[] {
    return [...this.contracts.values()].sort((left, right) => left.operation_id.localeCompare(right.operation_id));
  }

  forDomain(domainName: AutonomousDomainName): AutonomousConnectorOperationContract[] {
    const selectedDomain = domain("autonomous connector operation domain", domainName);
    return this.operations().filter((operation) => operation.domain === selectedDomain);
  }

  get digest(): string {
    return digestJsonSync(this.operations().map((operation) => operation.toJSON()));
  }

  assertRequest(operationId: string, request: AutonomousConnectorDispatchRequest): AutonomousConnectorOperationContract {
    const operation = this.resolve(operationId);
    operation.assertRequest(request);
    return operation;
  }

  toJSON(): JsonObject {
    return {
      schema: AUTONOMOUS_CONNECTOR_OPERATION_REGISTRY_SCHEMA,
      digest: this.digest,
      operations: this.operations().map((operation) => operation.toJSON()),
      operation_count: this.contracts.size,
      coverage: Object.fromEntries(AUTONOMOUS_DOMAIN_NAMES.map((name) => [name, this.forDomain(name).map((operation) => operation.operation_id)])),
      retention: "metadata_only_contract_catalogue",
      secret_material: "never_returned",
    };
  }
}

export interface AutonomousConnectorWorkItem extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA;
  work_id: string;
  operation_id: string;
  operation_digest: string;
  domain: AutonomousDomainName;
  capability: string;
  connector_id: string;
  selection_plan_digest: string;
  request_digest: string;
  dispatch_id: string;
  execution_id: string;
  call_id: string;
  attempt_id: string | null;
  parent_digests: string[];
  approved: boolean;
  max_attempts: number;
  attempts: number;
  status: AutonomousConnectorWorkStatus;
  available_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  receipt_digest: string | null;
  payload_digest: string | null;
  failure_class: AutonomousConnectorWorkFailureClass | null;
  last_error_class: AutonomousConnectorWorkFailureClass | null;
  created_at: number;
  updated_at: number;
  item_digest: string;
  retention: "metadata_only_request_plan_and_payload_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousConnectorWorkQueueSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA;
  operation_registry_digest: string;
  items: AutonomousConnectorWorkItem[];
  snapshot_digest: string;
  retention: "metadata_only_request_plan_and_payload_not_retained";
  secret_material: "never_returned";
}

export interface AutonomousConnectorWorkQueuePersistence {
  read(): Promise<AutonomousConnectorWorkQueueSnapshot | null> | AutonomousConnectorWorkQueueSnapshot | null;
  write(snapshot: AutonomousConnectorWorkQueueSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: string | null, snapshot: AutonomousConnectorWorkQueueSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousConnectorWorkQueueSnapshotTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousConnectorWorkQueueTransactionalSnapshotTextStore extends AutonomousConnectorWorkQueueSnapshotTextStore {
  writeIfUnchanged(expectedSnapshotDigest: string | null, value: string): Promise<boolean> | boolean;
}

function workItemPayload(item: AutonomousConnectorWorkItem): JsonObject {
  const { item_digest: _itemDigest, ...payload } = item;
  return payload;
}

function itemDigest(item: AutonomousConnectorWorkItem): string {
  return digestJsonSync(workItemPayload(item));
}

function validateWorkItem(raw: unknown, operationRegistry: AutonomousConnectorOperationRegistry): AutonomousConnectorWorkItem {
  if (!isObject(raw) || raw.schema !== AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA) throw new ArgumentError("autonomous connector work item is malformed");
  if (raw.retention !== "metadata_only_request_plan_and_payload_not_retained" || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous connector work item retention is invalid");
  const statuses: readonly AutonomousConnectorWorkStatus[] = ["queued", "leased", "completed", "failed", "reconciliation_required", "cancelled"];
  if (!statuses.includes(raw.status as AutonomousConnectorWorkStatus)) throw new ArgumentError("autonomous connector work item status is invalid");
  const failureClasses: readonly (AutonomousConnectorWorkFailureClass | null)[] = [null, "rehydration_missing", "rehydration_invalid", "identity_conflict", "lease_expired", "approval_required", "domain_scope", "capability_scope", "executor_error", "transport_error", "unknown"];
  if (!failureClasses.includes((raw.failure_class as AutonomousConnectorWorkFailureClass | null) ?? null) || !failureClasses.includes((raw.last_error_class as AutonomousConnectorWorkFailureClass | null) ?? null)) throw new ArgumentError("autonomous connector work item failure class is invalid");
  const item: AutonomousConnectorWorkItem = {
    schema: AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA,
    work_id: identifier("autonomous connector work_id", raw.work_id),
    operation_id: identifier("autonomous connector work operation_id", raw.operation_id),
    operation_digest: digest("autonomous connector work operation_digest", raw.operation_digest) as string,
    domain: domain("autonomous connector work domain", raw.domain),
    capability: capabilityValue("autonomous connector work capability", raw.capability),
    connector_id: identifier("autonomous connector work connector_id", raw.connector_id),
    selection_plan_digest: digest("autonomous connector work selection_plan_digest", raw.selection_plan_digest) as string,
    request_digest: digest("autonomous connector work request_digest", raw.request_digest) as string,
    dispatch_id: identifier("autonomous connector work dispatch_id", raw.dispatch_id),
    execution_id: identifier("autonomous connector work execution_id", raw.execution_id),
    call_id: identifier("autonomous connector work call_id", raw.call_id),
    attempt_id: raw.attempt_id === null ? null : identifier("autonomous connector work attempt_id", raw.attempt_id),
    parent_digests: arrayOfDigests("autonomous connector work parent_digests", raw.parent_digests),
    approved: raw.approved as boolean,
    max_attempts: boundedInteger("autonomous connector work max_attempts", raw.max_attempts, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS),
    attempts: boundedInteger("autonomous connector work attempts", raw.attempts, 0, MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS),
    status: raw.status as AutonomousConnectorWorkStatus,
    available_at: timestamp("autonomous connector work available_at", raw.available_at),
    lease_owner: raw.lease_owner === null ? null : identifier("autonomous connector work lease_owner", raw.lease_owner),
    lease_until: raw.lease_until === null ? null : timestamp("autonomous connector work lease_until", raw.lease_until),
    receipt_digest: raw.receipt_digest === null ? null : digest("autonomous connector work receipt_digest", raw.receipt_digest),
    payload_digest: raw.payload_digest === null ? null : digest("autonomous connector work payload_digest", raw.payload_digest),
    failure_class: (raw.failure_class as AutonomousConnectorWorkFailureClass | null) ?? null,
    last_error_class: (raw.last_error_class as AutonomousConnectorWorkFailureClass | null) ?? null,
    created_at: timestamp("autonomous connector work created_at", raw.created_at),
    updated_at: timestamp("autonomous connector work updated_at", raw.updated_at),
    item_digest: digest("autonomous connector work item_digest", raw.item_digest) as string,
    retention: "metadata_only_request_plan_and_payload_not_retained",
    secret_material: "never_returned",
  };
  if (typeof item.approved !== "boolean") throw new ArgumentError("autonomous connector work approved must be boolean");
  if (item.attempts > item.max_attempts || (item.status === "leased") !== (item.lease_owner !== null && item.lease_until !== null)) throw new ArgumentError("autonomous connector work lease state is inconsistent");
  const operation = operationRegistry.resolve(item.operation_id);
  if (operation.operation_digest !== item.operation_digest || operation.domain !== item.domain || !operation.supports(item.capability)) throw new ArgumentError("autonomous connector work operation identity is stale or invalid");
  if (item.item_digest !== itemDigest(item)) throw new ArgumentError("autonomous connector work item digest is invalid");
  return item;
}

function workFailureClass(value: unknown): AutonomousConnectorWorkFailureClass {
  const known: readonly AutonomousConnectorWorkFailureClass[] = ["rehydration_missing", "rehydration_invalid", "identity_conflict", "lease_expired", "approval_required", "domain_scope", "capability_scope", "executor_error", "transport_error", "unknown"];
  return known.includes(value as AutonomousConnectorWorkFailureClass) ? value as AutonomousConnectorWorkFailureClass : "unknown";
}

function refreshItem(item: AutonomousConnectorWorkItem, updates: Partial<AutonomousConnectorWorkItem>, time: number): AutonomousConnectorWorkItem {
  const next = { ...item, ...updates, updated_at: time, item_digest: "" } as AutonomousConnectorWorkItem;
  next.item_digest = itemDigest(next);
  return next;
}

/** A metadata-only queue with expiry-based recovery and worker fencing. */
export class InMemoryAutonomousConnectorWorkQueue {
  private readonly items = new Map<string, AutonomousConnectorWorkItem>();

  constructor(readonly operationRegistry = new AutonomousConnectorOperationRegistry(), readonly maxItems = MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS) {
    if (!(operationRegistry instanceof AutonomousConnectorOperationRegistry)) throw new ArgumentError("autonomous connector work queue requires an operation registry");
    if (!Number.isSafeInteger(maxItems) || maxItems < 1 || maxItems > MAX_AUTONOMOUS_CONNECTOR_WORK_ITEMS) throw new ArgumentError("autonomous connector work queue maxItems is outside its bound");
  }

  enqueue(input: { work_id: string; operation_id: string; request: AutonomousConnectorDispatchRequest; selection_plan_digest?: string; max_attempts?: number; available_at?: number; now?: number }): AutonomousConnectorWorkItem {
    const workId = identifier("autonomous connector work_id", input.work_id);
    if (!(input.request instanceof AutonomousConnectorDispatchRequest)) throw new ArgumentError("autonomous connector work enqueue requires a typed request");
    const operation = this.operationRegistry.assertRequest(input.operation_id, input.request);
    const selectionPlanDigest = digest("autonomous connector work selection_plan_digest", input.selection_plan_digest ?? input.request.selection_plan_digest) as string;
    if (input.request.selection_plan_digest !== selectionPlanDigest) throw new ArgumentError("autonomous connector work selection plan digest does not match request");
    const time = nowMs(input.now);
    const existing = this.items.get(workId);
    const maxAttempts = boundedInteger("autonomous connector work max_attempts", input.max_attempts ?? 3, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_ATTEMPTS);
    if (existing) {
      if (existing.operation_id !== operation.operation_id || existing.request_digest !== input.request.request_digest || existing.selection_plan_digest !== selectionPlanDigest) throw new ArgumentError("autonomous connector work identity conflicts with an existing work item");
      return clone(existing);
    }
    if (this.items.size >= this.maxItems) throw new ArgumentError("autonomous connector work queue is full");
    const availableAt = timestamp("autonomous connector work available_at", input.available_at ?? time);
    const item = {
      schema: AUTONOMOUS_CONNECTOR_WORK_ITEM_SCHEMA,
      work_id: workId,
      operation_id: operation.operation_id,
      operation_digest: operation.operation_digest,
      domain: operation.domain,
      capability: input.request.capability,
      connector_id: input.request.connector_id,
      selection_plan_digest: selectionPlanDigest,
      request_digest: input.request.request_digest,
      dispatch_id: input.request.dispatch_id,
      execution_id: input.request.execution_id,
      call_id: input.request.call_id,
      attempt_id: input.request.attempt_id,
      parent_digests: [...input.request.parent_digests],
      approved: input.request.approved,
      max_attempts: maxAttempts,
      attempts: 0,
      status: "queued" as const,
      available_at: availableAt,
      lease_owner: null,
      lease_until: null,
      receipt_digest: null,
      payload_digest: null,
      failure_class: null,
      last_error_class: null,
      created_at: time,
      updated_at: time,
      item_digest: "",
      retention: "metadata_only_request_plan_and_payload_not_retained" as const,
      secret_material: "never_returned" as const,
    } satisfies AutonomousConnectorWorkItem;
    item.item_digest = itemDigest(item);
    this.items.set(workId, item);
    return clone(item);
  }

  get(workId: string): AutonomousConnectorWorkItem | null {
    const item = this.items.get(identifier("autonomous connector work_id", workId));
    return item ? clone(item) : null;
  }

  pending(limit = 64, now = Date.now()): AutonomousConnectorWorkItem[] {
    const time = nowMs(now);
    const boundedLimit = boundedInteger("autonomous connector work pending limit", limit, 1, Math.min(MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH, this.maxItems));
    return [...this.items.values()]
      .filter((item) => (item.status === "queued" && item.available_at <= time && item.attempts < item.max_attempts) || (item.status === "leased" && item.lease_until !== null && item.lease_until <= time && item.attempts < item.max_attempts))
      .sort((left, right) => left.available_at - right.available_at || left.created_at - right.created_at || left.work_id.localeCompare(right.work_id))
      .slice(0, boundedLimit)
      .map((item) => clone(item));
  }

  claim(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousConnectorWorkItem | null {
    const id = identifier("autonomous connector work_id", workId);
    const worker = identifier("autonomous connector worker_id", workerId);
    const lease = boundedInteger("autonomous connector work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status === "completed" || item.status === "failed" || item.status === "reconciliation_required" || item.status === "cancelled") return null;
    if (item.status === "leased" && item.lease_until !== null && item.lease_until > time) return null;
    if (item.attempts >= item.max_attempts) {
      this.items.set(id, refreshItem(item, { status: "reconciliation_required", failure_class: "lease_expired", last_error_class: "lease_expired", lease_owner: null, lease_until: null }, time));
      return null;
    }
    const next = refreshItem(item, { status: "leased", attempts: item.attempts + 1, lease_owner: worker, lease_until: time + lease, last_error_class: null }, time);
    this.items.set(id, next);
    return clone(next);
  }

  renew(workId: string, workerId: string, leaseMs = 30_000, now = Date.now()): AutonomousConnectorWorkItem {
    const id = identifier("autonomous connector work_id", workId);
    const worker = identifier("autonomous connector worker_id", workerId);
    const lease = boundedInteger("autonomous connector work lease_ms", leaseMs, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous connector work lease cannot be renewed by this worker");
    const next = refreshItem(item, { lease_until: time + lease }, time);
    this.items.set(id, next);
    return clone(next);
  }

  complete(workId: string, workerId: string, receipt: AutonomousConnectorDispatchReceipt, now = Date.now()): AutonomousConnectorWorkItem {
    const id = identifier("autonomous connector work_id", workId);
    const worker = identifier("autonomous connector worker_id", workerId);
    if (!(receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("autonomous connector work completion requires a typed receipt");
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous connector work completion is fenced by an expired or foreign lease");
    if (receipt.request_digest !== item.request_digest || receipt.dispatch_id !== item.dispatch_id || receipt.execution_id !== item.execution_id || receipt.call_id !== item.call_id || receipt.connector_id !== item.connector_id) throw new ArgumentError("autonomous connector work receipt identity conflicts with the work item");
    const next = refreshItem(item, { status: "completed", lease_owner: null, lease_until: null, receipt_digest: digestJsonSync(receipt.toJSON()), payload_digest: receipt.payload_digest }, time);
    this.items.set(id, next);
    return clone(next);
  }

  fail(workId: string, workerId: string, errorClass: AutonomousConnectorWorkFailureClass, retryable: boolean, now = Date.now(), receipt: AutonomousConnectorDispatchReceipt | null = null): AutonomousConnectorWorkItem {
    const id = identifier("autonomous connector work_id", workId);
    const worker = identifier("autonomous connector worker_id", workerId);
    const failure = workFailureClass(errorClass);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous connector work failure is fenced by an expired or foreign lease");
    if (receipt !== null && !(receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("autonomous connector work failure receipt must be typed");
    const canRetry = retryable && item.attempts < item.max_attempts;
    const delay = Math.min(3_600_000, 1_000 * (2 ** Math.max(0, item.attempts - 1)));
    const next = refreshItem(item, {
      status: canRetry ? "queued" : "failed",
      available_at: canRetry ? time + delay : item.available_at,
      lease_owner: null,
      lease_until: null,
      receipt_digest: receipt === null ? item.receipt_digest : digestJsonSync(receipt.toJSON()),
      payload_digest: receipt === null ? item.payload_digest : receipt.payload_digest,
      failure_class: canRetry ? null : failure,
      last_error_class: failure,
    }, time);
    this.items.set(id, next);
    return clone(next);
  }

  reconcile(workId: string, workerId: string, errorClass: AutonomousConnectorWorkFailureClass = "rehydration_missing", now = Date.now()): AutonomousConnectorWorkItem {
    const id = identifier("autonomous connector work_id", workId);
    const worker = identifier("autonomous connector worker_id", workerId);
    const failure = workFailureClass(errorClass);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status !== "leased" || item.lease_owner !== worker || item.lease_until === null || item.lease_until <= time) throw new ArgumentError("autonomous connector reconciliation is fenced by an expired or foreign lease");
    const next = refreshItem(item, { status: "reconciliation_required", lease_owner: null, lease_until: null, failure_class: failure, last_error_class: failure }, time);
    this.items.set(id, next);
    return clone(next);
  }

  cancel(workId: string, reason: AutonomousConnectorWorkFailureClass = "unknown", now = Date.now()): AutonomousConnectorWorkItem {
    const id = identifier("autonomous connector work_id", workId);
    const time = nowMs(now);
    const item = this.items.get(id);
    if (!item || item.status === "completed" || item.status === "failed" || item.status === "reconciliation_required" || item.status === "cancelled") throw new ArgumentError("autonomous connector work cannot be cancelled in its current state");
    const next = refreshItem(item, { status: "cancelled", lease_owner: null, lease_until: null, failure_class: workFailureClass(reason), last_error_class: workFailureClass(reason) }, time);
    this.items.set(id, next);
    return clone(next);
  }

  rows(): AutonomousConnectorWorkItem[] {
    return [...this.items.values()].sort((left, right) => left.created_at - right.created_at || left.work_id.localeCompare(right.work_id)).map((item) => clone(item));
  }

  verifyIntegrity(): { schema: typeof AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA; verified: true; items: number; operation_registry_digest: string; retention: "metadata_only_request_plan_and_payload_not_retained"; secret_material: "never_returned" } {
    for (const item of this.items.values()) validateWorkItem(item, this.operationRegistry);
    return { schema: AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA, verified: true, items: this.items.size, operation_registry_digest: this.operationRegistry.digest, retention: "metadata_only_request_plan_and_payload_not_retained", secret_material: "never_returned" };
  }

  snapshot(): AutonomousConnectorWorkQueueSnapshot {
    this.verifyIntegrity();
    const descriptor = { schema: AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA, operation_registry_digest: this.operationRegistry.digest, items: this.rows(), retention: "metadata_only_request_plan_and_payload_not_retained" as const, secret_material: "never_returned" as const };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) } satisfies AutonomousConnectorWorkQueueSnapshot;
    if (bytes(canonicalJson(snapshot)) > MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES) throw new ArgumentError("autonomous connector work queue snapshot exceeds its bound");
    return snapshot;
  }

  restore(snapshot: AutonomousConnectorWorkQueueSnapshot): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_CONNECTOR_WORK_QUEUE_SCHEMA || !Array.isArray(snapshot.items)) throw new ArgumentError("autonomous connector work queue snapshot is malformed");
    if (snapshot.retention !== "metadata_only_request_plan_and_payload_not_retained" || snapshot.secret_material !== "never_returned") throw new ArgumentError("autonomous connector work queue snapshot retention is invalid");
    if (snapshot.operation_registry_digest !== this.operationRegistry.digest) throw new ArgumentError("autonomous connector work queue snapshot operation registry is stale");
    const { snapshot_digest: observed, ...descriptor } = snapshot;
    if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("autonomous connector work queue snapshot digest is invalid");
    if (snapshot.items.length > this.maxItems) throw new ArgumentError("autonomous connector work queue snapshot exceeds maxItems");
    const restored = new Map<string, AutonomousConnectorWorkItem>();
    for (const raw of snapshot.items) {
      const item = validateWorkItem(raw, this.operationRegistry);
      if (restored.has(item.work_id)) throw new ArgumentError("autonomous connector work queue snapshot contains duplicate work ids");
      restored.set(item.work_id, item);
    }
    this.items.clear();
    for (const [workId, item] of restored) this.items.set(workId, item);
  }
}

export class AutonomousConnectorWorkQueuePersistenceCoordinator {
  private expectedSnapshotDigest: string | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly queue: InMemoryAutonomousConnectorWorkQueue, readonly persistence: AutonomousConnectorWorkQueuePersistence) {
    if (!(queue instanceof InMemoryAutonomousConnectorWorkQueue)) throw new ArgumentError("autonomous connector work persistence requires a typed queue");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("autonomous connector work persistence adapter is malformed");
  }

  async restore(): Promise<{ status: "empty" | "restored"; snapshot_digest: string | null; items: number }> {
    return this.enqueue(async () => {
      const snapshot = await this.persistence.read();
      if (snapshot === null) {
        this.expectedSnapshotDigest = null;
        return { status: "empty", snapshot_digest: null, items: 0 };
      }
      this.queue.restore(snapshot);
      const verified = this.queue.verifyIntegrity();
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return { status: "restored", snapshot_digest: snapshot.snapshot_digest, items: verified.items };
    });
  }

  async flush(): Promise<AutonomousConnectorWorkQueueSnapshot> {
    return this.enqueue(async () => {
      const snapshot = this.queue.snapshot();
      if (typeof this.persistence.writeIfUnchanged === "function") {
        if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("connector work persistence compare-and-swap conflict");
      } else await this.persistence.write(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return snapshot;
    });
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const queued = this.operationTail.then(() => operation());
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

export class JsonAutonomousConnectorWorkQueueSnapshotPersistence implements AutonomousConnectorWorkQueuePersistence {
  constructor(readonly textStore: AutonomousConnectorWorkQueueSnapshotTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("connector work text store is malformed");
  }

  async read(): Promise<AutonomousConnectorWorkQueueSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (bytes(encoded) > MAX_AUTONOMOUS_CONNECTOR_WORK_SNAPSHOT_BYTES) throw new ArgumentError("connector work JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("connector work JSON is invalid"); }
    if (!isObject(parsed)) throw new ArgumentError("connector work JSON must be an object");
    return parsed as unknown as AutonomousConnectorWorkQueueSnapshot;
  }

  async write(snapshot: AutonomousConnectorWorkQueueSnapshot): Promise<void> {
    await this.textStore.write(canonicalJson(snapshot));
  }
}

export class TransactionalJsonAutonomousConnectorWorkQueueSnapshotPersistence extends JsonAutonomousConnectorWorkQueueSnapshotPersistence {
  declare readonly textStore: AutonomousConnectorWorkQueueTransactionalSnapshotTextStore;

  constructor(textStore: AutonomousConnectorWorkQueueTransactionalSnapshotTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("connector work text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: string | null, snapshot: AutonomousConnectorWorkQueueSnapshot): Promise<boolean> {
    if (expectedSnapshotDigest !== null && !/^[0-9a-f]{64}$/.test(expectedSnapshotDigest)) throw new ArgumentError("connector work expected snapshot digest is invalid");
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(snapshot));
  }
}

export interface AutonomousConnectorWorkRehydration {
  plan: AutonomousConnectorSelectionPlan | unknown;
  request: AutonomousConnectorDispatchRequest;
}

export type AutonomousConnectorWorkRehydrator = (item: AutonomousConnectorWorkItem) => AutonomousConnectorWorkRehydration | Promise<AutonomousConnectorWorkRehydration>;

export interface AutonomousConnectorWorkerRow extends JsonObject {
  work_id: string;
  outcome: "completed" | "replayed" | "retry_scheduled" | "failed" | "reconciliation_required" | "leased_elsewhere";
  attempts: number;
  receipt: JsonObject | null;
  value_retained: false;
  payload_digest: string | null;
  error_class: AutonomousConnectorWorkFailureClass | null;
}

export interface AutonomousConnectorWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_WORKER_SCHEMA;
  worker_id: string;
  inspected: number;
  completed: number;
  retried: number;
  failed: number;
  reconciled: number;
  leased_elsewhere: number;
  rows: AutonomousConnectorWorkerRow[];
  retention: "metadata_only_receipts_no_request_or_plan_or_payload_values";
  secret_material: "never_returned";
}

function workerRow(item: AutonomousConnectorWorkItem, outcome: AutonomousConnectorWorkerRow["outcome"], receipt: AutonomousConnectorDispatchReceipt | null = null, errorClass: AutonomousConnectorWorkFailureClass | null = null): AutonomousConnectorWorkerRow {
  return {
    work_id: item.work_id,
    outcome,
    attempts: item.attempts,
    receipt: receipt?.toJSON() ?? null,
    value_retained: false,
    payload_digest: receipt?.payload_digest ?? item.payload_digest,
    error_class: errorClass,
  };
}

/** Executes queued work through caller-owned rehydration and a digest-bound runtime. */
export class AutonomousConnectorWorker {
  constructor(readonly runtime: AutonomousConnectorRuntime, readonly queue: InMemoryAutonomousConnectorWorkQueue, readonly rehydrate: AutonomousConnectorWorkRehydrator) {
    if (!(runtime instanceof AutonomousConnectorRuntime)) throw new ArgumentError("autonomous connector worker requires a connector runtime");
    if (!(queue instanceof InMemoryAutonomousConnectorWorkQueue)) throw new ArgumentError("autonomous connector worker requires a typed work queue");
    if (typeof rehydrate !== "function") throw new ArgumentError("autonomous connector worker requires a rehydrator");
  }

  async run(options: { workerId?: string; limit?: number; leaseMs?: number; now?: number; signal?: { readonly aborted: boolean }; workIds?: readonly string[] } = {}): Promise<AutonomousConnectorWorkerRun> {
    const workerId = identifier("autonomous connector worker_id", options.workerId ?? "connector-worker");
    const limit = boundedInteger("autonomous connector worker limit", options.limit ?? 64, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH);
    const leaseMs = boundedInteger("autonomous connector worker lease_ms", options.leaseMs ?? 30_000, 1, MAX_AUTONOMOUS_CONNECTOR_WORK_LEASE_MS);
    const normalizedWorkIds = options.workIds === undefined ? null : options.workIds.map((workId) => identifier("autonomous connector worker work_id", workId));
    if (normalizedWorkIds !== null && (normalizedWorkIds.length < 1 || normalizedWorkIds.length > MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH || new Set(normalizedWorkIds).size !== normalizedWorkIds.length)) throw new ArgumentError("autonomous connector worker workIds are outside their bound");
    const time = nowMs(options.now);
    const currentTime = () => options.now === undefined ? Date.now() : time;
    const pending = this.queue.pending(normalizedWorkIds === null ? limit : MAX_AUTONOMOUS_CONNECTOR_WORK_BATCH, time);
    const candidates = pending.filter((item) => normalizedWorkIds === null || normalizedWorkIds.includes(item.work_id)).slice(0, limit);
    const rows: AutonomousConnectorWorkerRow[] = [];
    for (const candidate of candidates) {
      if (options.signal?.aborted) break;
      const claimed = this.queue.claim(candidate.work_id, workerId, leaseMs, time);
      if (!claimed) {
        rows.push(workerRow(candidate, "leased_elsewhere"));
        continue;
      }
      try {
        const hydrated = await this.rehydrate(claimed);
        if (!hydrated || !(hydrated.request instanceof AutonomousConnectorDispatchRequest)) {
          this.queue.reconcile(claimed.work_id, workerId, "rehydration_missing", time);
          rows.push(workerRow(this.queue.get(claimed.work_id) ?? claimed, "reconciliation_required", null, "rehydration_missing"));
          continue;
        }
        const plan = hydrated.plan instanceof AutonomousConnectorSelectionPlan ? hydrated.plan : AutonomousConnectorSelectionPlan.fromJSON(hydrated.plan);
        const request = hydrated.request;
        this.assertHydratedIdentity(claimed, plan, request);
        const result = await this.runtime.dispatchFromPlan(plan, request);
        if (result.receipt.status === "observed" || result.receipt.status === "partial") {
          const finished = this.queue.complete(claimed.work_id, workerId, result.receipt, currentTime());
          const outcome = result.replay === "replayed" ? "replayed" : "completed";
          rows.push(workerRow(finished, outcome, result.receipt));
        } else {
          const classification = workFailureClass(result.receipt.failure_class);
          const failed = this.queue.fail(claimed.work_id, workerId, classification, result.receipt.status === "error" || result.receipt.status === "unknown", currentTime(), result.receipt);
          rows.push(workerRow(failed, failed.status === "queued" ? "retry_scheduled" : "failed", result.receipt, classification));
        }
      } catch (error) {
        const classification = this.classify(error);
        if (classification === "rehydration_missing" || classification === "rehydration_invalid" || classification === "identity_conflict") {
          const reconciled = this.queue.reconcile(claimed.work_id, workerId, classification, currentTime());
          rows.push(workerRow(reconciled, "reconciliation_required", null, classification));
        } else {
          const failed = this.queue.fail(claimed.work_id, workerId, classification, classification === "executor_error" || classification === "transport_error" || classification === "unknown", currentTime());
          rows.push(workerRow(failed, failed.status === "queued" ? "retry_scheduled" : "failed", null, failed.failure_class));
        }
      }
    }
    return {
      schema: AUTONOMOUS_CONNECTOR_WORKER_SCHEMA,
      worker_id: workerId,
      inspected: candidates.length,
      completed: rows.filter((row) => row.outcome === "completed" || row.outcome === "replayed").length,
      retried: rows.filter((row) => row.outcome === "retry_scheduled").length,
      failed: rows.filter((row) => row.outcome === "failed").length,
      reconciled: rows.filter((row) => row.outcome === "reconciliation_required").length,
      leased_elsewhere: rows.filter((row) => row.outcome === "leased_elsewhere").length,
      rows,
      retention: "metadata_only_receipts_no_request_or_plan_or_payload_values",
      secret_material: "never_returned",
    };
  }

  private assertHydratedIdentity(item: AutonomousConnectorWorkItem, plan: AutonomousConnectorSelectionPlan, request: AutonomousConnectorDispatchRequest): void {
    if (request.request_digest !== item.request_digest || request.selection_plan_digest !== item.selection_plan_digest || request.dispatch_id !== item.dispatch_id || request.execution_id !== item.execution_id || request.call_id !== item.call_id || request.connector_id !== item.connector_id || request.capability !== item.capability || request.attempt_id !== item.attempt_id || request.approved !== item.approved || request.domains.length !== 1 || request.domains[0] !== item.domain) throw new ArgumentError("autonomous connector worker hydrated request identity conflicts with the work item");
    this.queue.operationRegistry.assertRequest(item.operation_id, request);
    plan.verify(this.runtime.registry);
    if (!plan.complete || plan.plan_digest !== item.selection_plan_digest || plan.capability !== request.capability || plan.domains.length !== 1 || plan.domains[0] !== item.domain) throw new ArgumentError("autonomous connector worker hydrated selection plan is stale or incomplete");
    const row = plan.rows[0];
    if (!row || row.connector_id !== item.connector_id) throw new ArgumentError("autonomous connector worker hydrated selection plan selects a different connector");
  }

  private classify(error: unknown): AutonomousConnectorWorkFailureClass {
    const message = error instanceof Error ? error.message.toLowerCase() : "";
    if (message.includes("rehydrat") || message.includes("selection plan") || message.includes("request identity") || message.includes("operation")) return message.includes("missing") ? "rehydration_missing" : message.includes("identity") ? "identity_conflict" : "rehydration_invalid";
    if (message.includes("approval_required")) return "approval_required";
    if (message.includes("domain_scope")) return "domain_scope";
    if (message.includes("capability_scope")) return "capability_scope";
    if (message.includes("executor")) return "executor_error";
    if (message.includes("transport")) return "transport_error";
    return "unknown";
  }
}

export interface AutonomousConnectorFeedbackInput extends JsonObject {
  feedback_id: string;
  domain?: AutonomousDomainName;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  source: "caller_evaluator";
  evidence_digest?: string | null;
  failure_class?: string | null;
  created_at?: number;
}

export interface AutonomousConnectorFeedbackEntry extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA;
  feedback_id: string;
  domain: AutonomousDomainName;
  capability: string;
  connector_id: string;
  receipt_digest: string;
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  evidence_digest: string | null;
  failure_class: string | null;
  created_at: number;
  entry_digest: string;
  retention: "metadata_only_explicit_evaluator_signal_no_request_or_payload";
  secret_material: "never_returned";
}

function feedbackPayload(entry: AutonomousConnectorFeedbackEntry): JsonObject {
  const { entry_digest: _entryDigest, ...payload } = entry;
  return payload;
}

function feedbackDigest(entry: AutonomousConnectorFeedbackEntry): string {
  return digestJsonSync(feedbackPayload(entry));
}

/**
 * Explicit evaluator feedback is kept separate from transport receipts. A successful
 * HTTP call, an observed connector result, or a replay is never treated as reward.
 */
export class InMemoryAutonomousConnectorFeedbackLedger {
  private readonly entries = new Map<string, AutonomousConnectorFeedbackEntry>();

  constructor(readonly maxEntries = MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES) {
    if (!Number.isSafeInteger(maxEntries) || maxEntries < 1 || maxEntries > MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_ENTRIES) throw new ArgumentError("autonomous connector feedback ledger maxEntries is outside its bound");
  }

  record(input: { feedback: AutonomousConnectorFeedbackInput; receipt: AutonomousConnectorDispatchReceipt; now?: number }): AutonomousConnectorFeedbackEntry {
    if (!isObject(input.feedback) || input.feedback.source !== "caller_evaluator") throw new ArgumentError("autonomous connector feedback must be explicitly caller_evaluator sourced");
    if (!(input.receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("autonomous connector feedback requires a typed receipt");
    const feedbackId = identifier("autonomous connector feedback_id", input.feedback.feedback_id);
    const existing = this.entries.get(feedbackId);
    const receiptDigest = digestJsonSync(input.receipt.toJSON());
    const receiptDomain = input.receipt.domains[0];
    if (receiptDomain === undefined) throw new ArgumentError("autonomous connector feedback receipt has no domain");
    const feedbackDomain = input.feedback.domain === undefined ? receiptDomain : domain("autonomous connector feedback domain", input.feedback.domain);
    if (!input.receipt.domains.includes(feedbackDomain)) throw new ArgumentError("autonomous connector feedback domain is not present on the receipt");
    const entry: AutonomousConnectorFeedbackEntry = {
      schema: AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA,
      feedback_id: feedbackId,
      domain: feedbackDomain,
      capability: identifier("autonomous connector feedback capability", input.receipt.capability),
      connector_id: identifier("autonomous connector feedback connector_id", input.receipt.connector_id),
      receipt_digest: receiptDigest,
      evaluator_id: identifier("autonomous connector evaluator_id", input.feedback.evaluator_id),
      evaluator_version: identifier("autonomous connector evaluator_version", input.feedback.evaluator_version),
      reward: finiteNumber("autonomous connector evaluator reward", input.feedback.reward, -1, 1),
      passed: input.feedback.passed,
      evidence_digest: input.feedback.evidence_digest === undefined || input.feedback.evidence_digest === null ? null : digest("autonomous connector feedback evidence_digest", input.feedback.evidence_digest),
      failure_class: input.feedback.failure_class === undefined || input.feedback.failure_class === null ? null : identifier("autonomous connector feedback failure_class", input.feedback.failure_class),
      created_at: nowMs(input.feedback.created_at ?? input.now),
      entry_digest: "",
      retention: "metadata_only_explicit_evaluator_signal_no_request_or_payload",
      secret_material: "never_returned",
    };
    if (typeof entry.passed !== "boolean") throw new ArgumentError("autonomous connector evaluator passed must be boolean");
    entry.entry_digest = feedbackDigest(entry);
    if (existing) {
      if (existing.entry_digest !== entry.entry_digest) throw new ArgumentError("autonomous connector feedback identity conflicts with an existing entry");
      return clone(existing);
    }
    if (this.entries.size >= this.maxEntries) throw new ArgumentError("autonomous connector feedback ledger is full");
    this.entries.set(feedbackId, entry);
    return clone(entry);
  }

  rows(): AutonomousConnectorFeedbackEntry[] {
    return [...this.entries.values()].sort((left, right) => left.created_at - right.created_at || left.feedback_id.localeCompare(right.feedback_id)).map((entry) => clone(entry));
  }

  signals(options: { domain?: AutonomousDomainName; capability?: string } = {}): Record<string, JsonObject> {
    const selectedDomain = options.domain === undefined ? undefined : domain("autonomous connector feedback signal domain", options.domain);
    const selectedCapability = options.capability === undefined ? undefined : identifier("autonomous connector feedback signal capability", options.capability);
    const grouped = new Map<string, AutonomousConnectorFeedbackEntry[]>();
    for (const entry of this.entries.values()) {
      if (selectedDomain !== undefined && entry.domain !== selectedDomain) continue;
      if (selectedCapability !== undefined && entry.capability !== selectedCapability) continue;
      const current = grouped.get(entry.connector_id) ?? [];
      current.push(entry);
      grouped.set(entry.connector_id, current);
    }
    return Object.fromEntries([...grouped.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([connectorId, rows]) => {
      const reward = rows.reduce((sum, row) => sum + row.reward, 0) / rows.length;
      const passed = rows.filter((row) => row.passed).length / rows.length;
      return [connectorId, {
        eligible: true,
        health: (reward + 1) / 2,
        success_rate: passed,
        evaluator_reward: reward,
        latency_ms: null,
        cost_per_million_tokens: null,
      } satisfies JsonObject];
    }));
  }

  verifyIntegrity(): { schema: typeof AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA; verified: true; entries: number; retention: "metadata_only_explicit_evaluator_signal_no_request_or_payload"; secret_material: "never_returned" } {
    for (const entry of this.entries.values()) {
      if (entry.entry_digest !== feedbackDigest(entry)) throw new ArgumentError("autonomous connector feedback ledger entry digest is invalid");
    }
    return { schema: AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA, verified: true, entries: this.entries.size, retention: "metadata_only_explicit_evaluator_signal_no_request_or_payload", secret_material: "never_returned" };
  }

  snapshot(): JsonObject {
    this.verifyIntegrity();
    const descriptor = { schema: AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA, entries: this.rows(), retention: "metadata_only_explicit_evaluator_signal_no_request_or_payload" as const, secret_material: "never_returned" as const };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
    if (bytes(canonicalJson(snapshot)) > MAX_AUTONOMOUS_CONNECTOR_FEEDBACK_SNAPSHOT_BYTES) throw new ArgumentError("autonomous connector feedback snapshot exceeds its bound");
    return snapshot;
  }

  restore(snapshot: unknown): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_CONNECTOR_FEEDBACK_LEDGER_SCHEMA || !Array.isArray(snapshot.entries)) throw new ArgumentError("autonomous connector feedback snapshot is malformed");
    if (snapshot.retention !== "metadata_only_explicit_evaluator_signal_no_request_or_payload" || snapshot.secret_material !== "never_returned") throw new ArgumentError("autonomous connector feedback snapshot retention is invalid");
    const { snapshot_digest: observed, ...descriptor } = snapshot;
    if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("autonomous connector feedback snapshot digest is invalid");
    if (snapshot.entries.length > this.maxEntries) throw new ArgumentError("autonomous connector feedback snapshot exceeds maxEntries");
    const restored = new Map<string, AutonomousConnectorFeedbackEntry>();
    for (const raw of snapshot.entries) {
      if (!isObject(raw) || raw.schema !== AUTONOMOUS_CONNECTOR_FEEDBACK_SCHEMA || raw.retention !== "metadata_only_explicit_evaluator_signal_no_request_or_payload" || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous connector feedback snapshot entry is malformed");
      const entry = { ...raw } as AutonomousConnectorFeedbackEntry;
      if (entry.entry_digest !== feedbackDigest(entry)) throw new ArgumentError("autonomous connector feedback snapshot entry digest is invalid");
      if (restored.has(entry.feedback_id)) throw new ArgumentError("autonomous connector feedback snapshot contains duplicate feedback ids");
      restored.set(entry.feedback_id, entry);
    }
    this.entries.clear();
    for (const [feedbackId, entry] of restored) this.entries.set(feedbackId, entry);
  }
}

export type { AutonomousConnectorDispatchResult };
