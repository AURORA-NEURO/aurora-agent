import { ArgumentError, isObject } from "./errors.js";
import type { AutonomousAuthorizationContext } from "./autonomous-authorization.js";
import type { ApiClient } from "./client.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type {
  DomainEvidenceSourceExecutionArgs,
  DomainEvidenceSourceExecutionResult,
  DomainEvidenceSourcePlanArgs,
  DomainEvidenceSourcePlanResult,
  DomainEvidenceProviderConnectorKind,
  DomainEvidenceProviderConnectorManifest,
  JsonObject,
  JsonValue,
} from "./types.js";
import type { AutonomousDomainName } from "./autonomous.js";

const AUTONOMOUS_DOMAIN_NAMES: readonly AutonomousDomainName[] = [
  "coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise",
  "multi_agent", "multimodal", "cross_domain", "evaluation",
];

/**
 * Caller-owned connector execution for the autonomous brain.
 *
 * This module deliberately stops at a typed, digest-bound handoff. It does not discover
 * providers, open sockets, accept credentials, or infer authorization. An embedding application
 * closes over its own configured client/session in the executor and receives a transient request.
 * Durable projections contain only identities, digests, status, and bounded failure classes.
 */
export const AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA = "bioprism-typescript-autonomous-connector-registry/0.1" as const;
export const AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA = "bioprism-typescript-autonomous-connector-dispatch/0.1" as const;
export const AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-connector-receipt/0.1" as const;
export const AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA = "bioprism-typescript-autonomous-connector-selection-plan/0.1" as const;
export const AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA = "bioprism-typescript-autonomous-connector-selection-row/0.1" as const;
export const AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA = "bioprism-typescript-autonomous-connector-receipt-journal/0.1" as const;
export const AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA = "bioprism-typescript-autonomous-connector-receipt-entry/0.1" as const;
export const AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES = ["lexicographic_connector_id", "weighted_evidence"] as const;
export const AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES = ["observed", "partial", "refused", "error", "unknown"] as const;
export const MAX_AUTONOMOUS_CONNECTORS = 256;
export const MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES = 2_000_000;
export const MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS = 128;
export const MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES = 100_000;
export const MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES = 50_000_000;
export const MAX_AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_BYTES = 24_000;
export const MAX_AUTONOMOUS_CONNECTOR_SELECTION_SIGNAL_BYTES = 64_000;

export type AutonomousConnectorSelectionStrategy = typeof AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES[number];
export type AutonomousConnectorDispatchStatus = typeof AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES[number];
export type AutonomousConnectorExecutor = (
  manifest: DomainEvidenceProviderConnectorManifest,
  request: JsonObject,
) => unknown | Promise<unknown>;

/** Metadata-only lifecycle event emitted by connector dispatch boundaries. */
export interface AutonomousConnectorTraceEvent extends JsonObject {
  phase: "connector_started" | "connector_finished";
  status: "running" | "completed" | "partial" | "refused" | "failed" | "unknown";
  domains: AutonomousDomainName[];
  route_digest: string | null;
  selection_digest: string | null;
  detail_digest: string | null;
  provider: string;
  failure_class: string | null;
  failure_code: string | null;
}

export type AutonomousConnectorTraceEventCallback = (event: AutonomousConnectorTraceEvent) => unknown | Promise<unknown>;

const SOURCE_CONNECTOR_STATUSES = ["observed", "partial", "refused", "error", "unknown"] as const;

function sourceConnectorStructured<T extends JsonObject>(response: unknown, label: string): T {
  if (!isObject(response) || response.ok !== true || !isObject(response.mcp) || !isObject(response.mcp.result) || !isObject(response.mcp.result.structuredContent)) throw new ArgumentError(`${label} returned no structured source report`);
  return response.mcp.result.structuredContent as T;
}

function sourceConnectorManifest(value: unknown): DomainEvidenceProviderConnectorManifest {
  if (!isObject(value)) throw new ArgumentError("autonomous API source connector manifest is malformed");
  return normalizeManifest(value as unknown as DomainEvidenceProviderConnectorManifest);
}

/**
 * Bridge a caller-configured ApiClient into the reviewed source connector runtime.
 *
 * The bridge performs source planning and source execution as two distinct calls. The execution
 * request always uses the plan digest returned by the planning response; a caller cannot smuggle
 * a different digest through the transient connector envelope. ApiClient owns transport, auth,
 * and session resolution. This helper never accepts, discovers, logs, or persists credentials.
 */
export function createAutonomousApiSourceConnectorExecutor(
  client: ApiClient,
  options: { useToolRoute?: boolean } = {},
): AutonomousConnectorExecutor {
  if (!client || typeof client.domainEvidenceSourcePlan !== "function" || typeof client.domainEvidenceSourceExecute !== "function") throw new ArgumentError("autonomous API source connector requires a configured ApiClient");
  if (!isObject(options)) throw new ArgumentError("autonomous API source connector options must be an object");
  if (options.useToolRoute !== undefined && typeof options.useToolRoute !== "boolean") throw new ArgumentError("autonomous API source connector useToolRoute must be boolean");
  const useToolRoute = options.useToolRoute === true;
  if (useToolRoute && (typeof client.domainEvidenceSourcePlanTool !== "function" || typeof client.domainEvidenceSourceExecuteTool !== "function")) throw new ArgumentError("autonomous API source connector tool route is unavailable on the ApiClient");

  return async (rawManifest, rawRequest): Promise<AutonomousConnectorObservation> => {
    const manifest = sourceConnectorManifest(rawManifest);
    if (!isObject(rawRequest)) throw new ArgumentError("autonomous API source connector request must be an object");
    const planRaw = rawRequest.plan;
    const executionRaw = rawRequest.execution ?? {};
    if (!isObject(planRaw) || !isObject(executionRaw)) throw new ArgumentError("autonomous API source connector requires plan and execution objects");
    const safePlan = safeJson("autonomous API source connector plan", planRaw, MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES);
    const safeExecution = safeJson("autonomous API source connector execution", executionRaw, MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES);
    if (!isObject(safePlan) || !isObject(safeExecution)) throw new ArgumentError("autonomous API source connector plan and execution must be objects");
    const planRequest = safePlan as unknown as DomainEvidenceSourcePlanArgs;
    const execution = safeExecution;
    if (planRequest.connector_kind !== manifest.connector_kind) throw new ArgumentError("autonomous API source connector kind does not match its manifest");
    if (!Array.isArray(planRequest.domains) || planRequest.domains.some((domain) => !manifest.domains.includes(domain))) throw new ArgumentError("autonomous API source connector plan exceeds manifest domain scope");

    const planResponse = useToolRoute
      ? sourceConnectorStructured<DomainEvidenceSourcePlanResult>(await client.domainEvidenceSourcePlanTool!(planRequest), "autonomous API source connector planning")
      : await client.domainEvidenceSourcePlan(planRequest);
    if (!isObject(planResponse) || planResponse.ok !== true) throw new ArgumentError("autonomous API source connector plan response was not successful");
    if (typeof planResponse.plan_digest !== "string" || !/^[0-9a-f]{64}$/.test(planResponse.plan_digest)) throw new ArgumentError("autonomous API source connector plan response omitted its digest");

    const sourceTool = execution.source_tool;
    if (sourceTool !== undefined && sourceTool !== null && typeof sourceTool !== "string") throw new ArgumentError("autonomous API source connector execution source_tool must be a string or null");
    const claimPosture = execution.claim_posture;
    if (claimPosture !== undefined && !isObject(claimPosture)) throw new ArgumentError("autonomous API source connector execution claim_posture must be an object");
    const parentDigestsRaw = execution.parent_digests ?? planRequest.parent_digests ?? [];
    if (!Array.isArray(parentDigestsRaw) || parentDigestsRaw.length > 128 || parentDigestsRaw.some((digest) => typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest))) throw new ArgumentError("autonomous API source connector parent_digests must contain at most 128 lowercase SHA-256 digests");
    const parentDigests = parentDigestsRaw as string[];
    const executionRequest: DomainEvidenceSourceExecutionArgs = {
      source_plan_digest: planResponse.plan_digest,
      source_tool: sourceTool === undefined ? planRequest.source_tool ?? null : sourceTool,
      request: execution.request,
      claim_posture: claimPosture as JsonObject | undefined,
      parent_digests: [...parentDigests],
    };
    const executionResponse = useToolRoute
      ? sourceConnectorStructured<DomainEvidenceSourceExecutionResult>(await client.domainEvidenceSourceExecuteTool!(executionRequest), "autonomous API source connector execution")
      : await client.domainEvidenceSourceExecute(executionRequest);
    if (!isObject(executionResponse) || executionResponse.ok !== true || typeof executionResponse.outcome !== "string" || !SOURCE_CONNECTOR_STATUSES.includes(executionResponse.outcome as typeof SOURCE_CONNECTOR_STATUSES[number])) throw new ArgumentError("autonomous API source connector execution response is malformed");
    return new AutonomousConnectorObservation(executionResponse, executionResponse.outcome as typeof SOURCE_CONNECTOR_STATUSES[number]);
  };
}

export interface AutonomousConnectorSelectionSignal {
  connector_id: string;
  eligible: boolean;
  health: number;
  success_rate: number;
  evaluator_reward: number;
  latency_ms: number | null;
  cost_per_million_tokens: number | null;
  score: number;
}

export interface AutonomousConnectorCoverageRow extends JsonObject {
  status: "selected" | "missing";
  connector_ids: string[];
  manifest_digests: string[];
  capability: string | null;
}

export interface AutonomousConnectorCoveragePlan extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA;
  domains: AutonomousDomainName[];
  capability: string | null;
  coverage: Record<string, AutonomousConnectorCoverageRow>;
  registry_digest: string;
  selection_plan_digest: string;
  plan_digest: string;
  execution: "planning_only;no_dispatch;no_authorization";
  secret_material: "never_returned";
}

export interface AutonomousConnectorReceiptStore {
  append(receipt: AutonomousConnectorDispatchReceipt): AutonomousConnectorReceiptJournalEntry | AutonomousConnectorDispatchReceipt | Promise<AutonomousConnectorReceiptJournalEntry | AutonomousConnectorDispatchReceipt>;
  find(query: AutonomousConnectorReceiptLookup): AutonomousConnectorDispatchReceipt | null | Promise<AutonomousConnectorDispatchReceipt | null>;
}

export interface AutonomousConnectorReceiptLookup extends JsonObject {
  execution_id: string;
  dispatch_id: string;
  call_id: string;
  connector_id: string;
  attempt_id: string | null;
}

export interface AutonomousConnectorReceiptJournalSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA;
  entries: JsonObject[];
  head_digest: string | null;
  snapshot_digest: string;
  retention: "metadata_only_hash_chained_no_request_or_payload";
  secret_material: "never_returned";
}

export interface AutonomousConnectorReceiptJournalPersistence {
  read(): Promise<AutonomousConnectorReceiptJournalSnapshot | null> | AutonomousConnectorReceiptJournalSnapshot | null;
  write(snapshot: AutonomousConnectorReceiptJournalSnapshot): Promise<void> | void;
}

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

function capabilityIdentifier(name: string, value: unknown): string {
  const text = boundedText(name, value, 256);
  if (!/^[A-Za-z0-9_.:+-]+$/.test(text)) throw new ArgumentError(`${name} must be a bounded capability identifier`);
  return text;
}

function capabilityIdentifiers(name: string, value: unknown, maximum: number, allowEmpty = false): string[] {
  if (!Array.isArray(value) || value.length > maximum || (!allowEmpty && value.length === 0)) throw new ArgumentError(`${name} must contain between ${allowEmpty ? 0 : 1} and ${maximum} entries`);
  const result = value.map((item) => capabilityIdentifier(`${name} entry`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedArray<T>(name: string, value: unknown, maximum: number): T[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > maximum) throw new ArgumentError(`${name} must contain between 1 and ${maximum} entries`);
  return [...value] as T[];
}

function identifiers(name: string, value: unknown, maximum: number, allowEmpty = false): string[] {
  if (!Array.isArray(value) || value.length > maximum || (!allowEmpty && value.length === 0)) throw new ArgumentError(`${name} must contain between ${allowEmpty ? 0 : 1} and ${maximum} entries`);
  const result = value.map((item) => identifier(`${name} entry`, item));
  if (new Set(result).size !== result.length) throw new ArgumentError(`${name} contains duplicate entries`);
  return result;
}

function boundedTexts(name: string, value: unknown, maximum: number, allowEmpty = false): string[] {
  if (!Array.isArray(value) || value.length > maximum || (!allowEmpty && value.length === 0)) throw new ArgumentError(`${name} must contain between ${allowEmpty ? 0 : 1} and ${maximum} entries`);
  return value.map((item) => boundedText(`${name} entry`, item));
}

function finiteNumber(name: string, value: unknown, minimum: number, maximum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) throw new ArgumentError(`${name} must be between ${minimum} and ${maximum}`);
  return value;
}

const SECRET_FIELD_MARKERS = new Set([
  "apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "secretkey",
  "token", "accesstoken", "refreshtoken", "privatekey", "clientsecret", "gsk", "sk",
]);

function rejectsSecretField(name: string): boolean {
  const normalized = name.toLowerCase().replace(/[^a-z0-9]/g, "");
  return SECRET_FIELD_MARKERS.has(normalized) || normalized.startsWith("gsk") || normalized.startsWith("skproj");
}

function safeJson(name: string, value: unknown, maximum: number, depth = 0): JsonValue {
  if (depth > 32) throw new ArgumentError(`${name} is too deeply nested`);
  if (value === null) return null;
  if (typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (Array.isArray(value)) {
    const result = value.map((item) => safeJson(name, item, maximum, depth + 1));
    if (bytes(canonicalJson(result)) > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  if (isObject(value)) {
    const result: JsonObject = {};
    for (const [key, child] of Object.entries(value)) {
      if (rejectsSecretField(key)) throw new ArgumentError(`${name} contains credential-shaped fields`);
      if (child === undefined) throw new ArgumentError(`${name} contains undefined JSON values`);
      result[key] = safeJson(name, child, maximum, depth + 1);
    }
    const encoded = canonicalJson(result);
    if (bytes(encoded) > maximum) throw new ArgumentError(`${name} exceeds ${maximum} bytes`);
    return result;
  }
  throw new ArgumentError(`${name} must be JSON-safe`);
}

function normalizeManifest(value: DomainEvidenceProviderConnectorManifest): DomainEvidenceProviderConnectorManifest {
  if (!isObject(value) || value.schema !== "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1") throw new ArgumentError("autonomous connector manifest schema is invalid");
  if (value.transport !== "caller_managed") throw new ArgumentError("autonomous connector manifest transport must be caller_managed");
  const connectorKinds: readonly DomainEvidenceProviderConnectorKind[] = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
  if (!connectorKinds.includes(value.connector_kind as DomainEvidenceProviderConnectorKind)) throw new ArgumentError("autonomous connector manifest connector_kind is invalid");
  const domains = identifiers("autonomous connector manifest domains", value.domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError(`autonomous connector manifest domain is unsupported: ${domain}`);
    return domain as AutonomousDomainName;
  });
  const capabilities = capabilityIdentifiers("autonomous connector manifest capabilities", value.capabilities, 128);
  if (!isObject(value.auth_posture)) throw new ArgumentError("autonomous connector manifest auth_posture must be an object");
  const authStatus = value.auth_posture.status;
  if (authStatus !== "none" && authStatus !== "caller_asserted" && authStatus !== "delegated" && authStatus !== "unknown") throw new ArgumentError("autonomous connector manifest auth posture status is invalid");
  const secretRefs = value.auth_posture.secret_refs === undefined ? [] : identifiers("autonomous connector manifest secret_refs", value.auth_posture.secret_refs, 32, true);
  const doesNotClaim = boundedTexts("autonomous connector manifest auth non-claims", value.auth_posture.does_not_claim, 64);
  return {
    schema: value.schema,
    connector_id: identifier("autonomous connector manifest connector_id", value.connector_id),
    version: identifier("autonomous connector manifest version", value.version),
    provider: identifier("autonomous connector manifest provider", value.provider),
    connector_kind: value.connector_kind,
    domains,
    capabilities,
    transport: "caller_managed",
    auth_posture: { status: authStatus, secret_refs: secretRefs, does_not_claim: doesNotClaim },
  };
}

function selectionSignal(connectorId: string, raw: JsonObject | undefined): AutonomousConnectorSelectionSignal {
  const source = raw === undefined ? {} : safeJson("autonomous connector selection signal", raw, MAX_AUTONOMOUS_CONNECTOR_SELECTION_SIGNAL_BYTES);
  if (!isObject(source)) throw new ArgumentError("autonomous connector selection signal must be an object");
  const allowed = new Set(["eligible", "health", "success_rate", "evaluator_reward", "latency_ms", "cost_per_million_tokens"]);
  if (Object.keys(source).some((key) => !allowed.has(key))) throw new ArgumentError("autonomous connector selection signal contains unsupported fields");
  const eligible = source.eligible === undefined ? true : source.eligible;
  if (typeof eligible !== "boolean") throw new ArgumentError("autonomous connector selection signal eligible must be boolean");
  const health = finiteNumber("autonomous connector selection signal health", source.health ?? 0.5, 0, 1);
  const successRate = finiteNumber("autonomous connector selection signal success_rate", source.success_rate ?? health, 0, 1);
  const evaluatorReward = finiteNumber("autonomous connector selection signal evaluator_reward", source.evaluator_reward ?? 0, -1, 1);
  const latency = source.latency_ms === undefined || source.latency_ms === null ? null : finiteNumber("autonomous connector selection signal latency_ms", source.latency_ms, 0, 86_400_000);
  const cost = source.cost_per_million_tokens === undefined || source.cost_per_million_tokens === null ? null : finiteNumber("autonomous connector selection signal cost_per_million_tokens", source.cost_per_million_tokens, 0, 1_000_000);
  const latencyScore = latency === null ? 0.5 : 1 / (1 + latency / 1_000);
  const costScore = cost === null ? 0.5 : 1 / (1 + cost / 100);
  return {
    connector_id: connectorId,
    eligible,
    health,
    success_rate: successRate,
    evaluator_reward: evaluatorReward,
    latency_ms: latency,
    cost_per_million_tokens: cost,
    score: Number((0.35 * health + 0.25 * successRate + 0.25 * ((evaluatorReward + 1) / 2) + 0.10 * latencyScore + 0.05 * costScore).toFixed(12)),
  };
}

export class AutonomousConnectorRegistration {
  readonly manifest: DomainEvidenceProviderConnectorManifest;
  readonly executor: AutonomousConnectorExecutor;
  readonly approval_required: boolean;

  constructor(manifest: DomainEvidenceProviderConnectorManifest, executor: AutonomousConnectorExecutor, approvalRequired = true) {
    if (typeof executor !== "function") throw new ArgumentError("autonomous connector executor must be callable");
    if (typeof approvalRequired !== "boolean") throw new ArgumentError("autonomous connector approval_required must be boolean");
    this.manifest = normalizeManifest(manifest);
    this.executor = executor;
    this.approval_required = approvalRequired;
  }

  get connector_id(): string { return this.manifest.connector_id; }
  get manifest_digest(): string { return digestJsonSync(this.manifest); }

  toJSON(): JsonObject {
    return {
      schema: AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA,
      manifest: structuredClone(this.manifest),
      manifest_digest: this.manifest_digest,
      approval_required: this.approval_required,
      execution: "caller_owned_executor;metadata_only_registration",
      secret_material: "never_returned",
    };
  }
}

export class AutonomousConnectorSelectionRow {
  readonly domain: AutonomousDomainName;
  readonly status: "selected" | "missing";
  readonly connector_id: string | null;
  readonly manifest_digest: string | null;
  readonly candidate_ids: string[];
  readonly candidate_manifest_digests: string[];
  readonly candidate_scores: number[];
  readonly candidate_eligible: boolean[];
  readonly reason: string;

  constructor(input: {
    domain: AutonomousDomainName; status: "selected" | "missing"; connector_id: string | null; manifest_digest: string | null;
    candidate_ids: readonly string[]; candidate_manifest_digests: readonly string[]; reason: string;
    candidate_scores?: readonly number[]; candidate_eligible?: readonly boolean[];
  }) {
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(input.domain)) throw new ArgumentError("autonomous connector selection row domain is unsupported");
    if (input.status !== "selected" && input.status !== "missing") throw new ArgumentError("autonomous connector selection row status is invalid");
    const candidateIds = identifiers("autonomous connector selection row candidate_ids", input.candidate_ids, MAX_AUTONOMOUS_CONNECTORS, true);
    const candidateDigests = input.candidate_manifest_digests.map((value) => digest("autonomous connector candidate manifest digest", value) as string);
    if (candidateIds.length !== candidateDigests.length) throw new ArgumentError("autonomous connector selection row candidates and digests must align");
    const scores = input.candidate_scores === undefined ? candidateIds.map(() => 0) : input.candidate_scores.map((value) => finiteNumber("autonomous connector candidate score", value, 0, 1));
    const eligible = input.candidate_eligible === undefined ? candidateIds.map(() => true) : [...input.candidate_eligible];
    if (scores.length !== candidateIds.length || eligible.length !== candidateIds.length || eligible.some((value) => typeof value !== "boolean")) throw new ArgumentError("autonomous connector selection row candidate metadata must align");
    if (input.status === "selected") {
      if (input.connector_id === null || input.manifest_digest === null) throw new ArgumentError("selected connector row requires connector and manifest identities");
      const selectedIndex = candidateIds.indexOf(input.connector_id);
      if (selectedIndex < 0 || !eligible[selectedIndex] || candidateDigests[selectedIndex] !== input.manifest_digest) throw new ArgumentError("selected connector row does not match an eligible candidate");
    } else if (input.connector_id !== null || input.manifest_digest !== null) throw new ArgumentError("missing connector row cannot select a connector");
    this.domain = input.domain;
    this.status = input.status;
    this.connector_id = input.connector_id === null ? null : identifier("autonomous connector selection row connector_id", input.connector_id);
    this.manifest_digest = input.manifest_digest === null ? null : digest("autonomous connector selection row manifest_digest", input.manifest_digest);
    this.candidate_ids = candidateIds;
    this.candidate_manifest_digests = candidateDigests;
    this.candidate_scores = scores;
    this.candidate_eligible = eligible;
    this.reason = identifier("autonomous connector selection row reason", input.reason);
  }

  toJSON(): JsonObject {
    return {
      schema: AUTONOMOUS_CONNECTOR_SELECTION_ROW_SCHEMA,
      domain: this.domain,
      status: this.status,
      connector_id: this.connector_id,
      manifest_digest: this.manifest_digest,
      candidate_ids: [...this.candidate_ids],
      candidate_manifest_digests: [...this.candidate_manifest_digests],
      candidate_scores: [...this.candidate_scores],
      candidate_eligible: [...this.candidate_eligible],
      reason: this.reason,
      retention: "metadata_only_manifest_catalogue",
      secret_material: "never_returned",
    };
  }
}

export class AutonomousConnectorSelectionPlan {
  readonly domains: AutonomousDomainName[];
  readonly capability: string | null;
  readonly registry_digest: string;
  readonly rows: AutonomousConnectorSelectionRow[];
  readonly strategy: AutonomousConnectorSelectionStrategy;
  readonly signal_digest: string | null;

  constructor(input: { domains: readonly AutonomousDomainName[]; capability: string | null; registry_digest: string; rows: readonly AutonomousConnectorSelectionRow[]; strategy?: AutonomousConnectorSelectionStrategy; signal_digest?: string | null }) {
    const domains = identifiers("autonomous connector selection plan domains", input.domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous connector selection plan domain is unsupported");
      return domain as AutonomousDomainName;
    });
    if (input.capability !== null) capabilityIdentifier("autonomous connector selection plan capability", input.capability);
    const registryDigest = digest("autonomous connector selection plan registry_digest", input.registry_digest) as string;
    const strategy = input.strategy ?? "lexicographic_connector_id";
    if (!AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES.includes(strategy)) throw new ArgumentError("autonomous connector selection plan strategy is invalid");
    const signalDigest = digest("autonomous connector selection plan signal_digest", input.signal_digest, true);
    if (input.rows.length !== domains.length || input.rows.some((row, index) => !(row instanceof AutonomousConnectorSelectionRow) || row.domain !== domains[index])) throw new ArgumentError("autonomous connector selection plan rows must align with domains");
    this.domains = domains;
    this.capability = input.capability === null ? null : capabilityIdentifier("autonomous connector selection plan capability", input.capability);
    this.registry_digest = registryDigest;
    this.rows = [...input.rows];
    this.strategy = strategy;
    this.signal_digest = signalDigest;
  }

  get complete(): boolean { return this.rows.every((row) => row.status === "selected"); }
  get plan_digest(): string { return digestJsonSync(this.payload()); }

  private payload(): JsonObject {
    return { schema: AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA, domains: [...this.domains], capability: this.capability, registry_digest: this.registry_digest, rows: this.rows.map((row) => row.toJSON()), strategy: this.strategy, signal_digest: this.signal_digest };
  }

  toJSON(): JsonObject {
    return { ...this.payload(), complete: this.complete, plan_digest: this.plan_digest, execution: "planning_only;review_required_before_dispatch", retention: "metadata_only_manifest_catalogue", secret_material: "never_returned" };
  }

  verify(registry: AutonomousConnectorRegistry): this {
    if (!(registry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("autonomous connector selection plan verification requires a registry");
    if (this.registry_digest !== registry.digest) throw new ArgumentError("autonomous connector selection plan registry is stale or tampered");
    for (const row of this.rows) {
      const candidates = registry.registrations().filter((registration) => registration.manifest.domains.includes(row.domain) && (this.capability === null || registration.manifest.capabilities.includes(this.capability)));
      if (candidates.map((candidate) => candidate.connector_id).join("\u0000") !== row.candidate_ids.join("\u0000") || candidates.map((candidate) => candidate.manifest_digest).join("\u0000") !== row.candidate_manifest_digests.join("\u0000")) throw new ArgumentError("autonomous connector selection plan candidate set changed");
    }
    return this;
  }

  static fromJSON(value: unknown): AutonomousConnectorSelectionPlan {
    if (!isObject(value) || value.schema !== AUTONOMOUS_CONNECTOR_SELECTION_PLAN_SCHEMA || !Array.isArray(value.domains) || !Array.isArray(value.rows)) throw new ArgumentError("autonomous connector selection plan is malformed");
    if (value.retention !== "metadata_only_manifest_catalogue" || value.secret_material !== "never_returned" || value.execution !== "planning_only;review_required_before_dispatch") throw new ArgumentError("autonomous connector selection plan retention is invalid");
    if (value.complete !== value.rows.every((row) => isObject(row) && row.status === "selected")) throw new ArgumentError("autonomous connector selection plan completeness is invalid");
    const rows = value.rows.map((row) => {
      if (!isObject(row)) throw new ArgumentError("autonomous connector selection row is malformed");
      return new AutonomousConnectorSelectionRow({ domain: row.domain as AutonomousDomainName, status: row.status as "selected" | "missing", connector_id: (row.connector_id as string | null) ?? null, manifest_digest: (row.manifest_digest as string | null) ?? null, candidate_ids: row.candidate_ids as string[], candidate_manifest_digests: row.candidate_manifest_digests as string[], candidate_scores: row.candidate_scores as number[], candidate_eligible: row.candidate_eligible as boolean[], reason: row.reason as string });
    });
    const plan = new AutonomousConnectorSelectionPlan({ domains: value.domains as AutonomousDomainName[], capability: (value.capability as string | null) ?? null, registry_digest: value.registry_digest as string, rows, strategy: value.strategy as AutonomousConnectorSelectionStrategy, signal_digest: (value.signal_digest as string | null) ?? null });
    if (value.plan_digest !== plan.plan_digest) throw new ArgumentError("autonomous connector selection plan digest is invalid");
    return plan;
  }
}

export class AutonomousConnectorRegistry {
  private readonly connectors = new Map<string, AutonomousConnectorRegistration>();

  constructor(registrations: readonly AutonomousConnectorRegistration[] = []) {
    if (!Array.isArray(registrations)) throw new ArgumentError("autonomous connector registrations must be an array");
    for (const registration of registrations) this.register(registration);
  }

  register(registration: AutonomousConnectorRegistration, options: { replace?: boolean } = {}): AutonomousConnectorRegistration {
    if (!(registration instanceof AutonomousConnectorRegistration)) throw new ArgumentError("autonomous connector registration is invalid");
    if (options.replace !== undefined && typeof options.replace !== "boolean") throw new ArgumentError("autonomous connector replace must be boolean");
    if (this.connectors.has(registration.connector_id) && options.replace !== true) throw new ArgumentError(`autonomous connector is already registered: ${registration.connector_id}`);
    if (!this.connectors.has(registration.connector_id) && this.connectors.size >= MAX_AUTONOMOUS_CONNECTORS) throw new ArgumentError("autonomous connector registry capacity is exhausted");
    this.connectors.set(registration.connector_id, registration);
    return registration;
  }

  resolve(connectorId: string): AutonomousConnectorRegistration {
    const id = identifier("autonomous connector id", connectorId);
    const registration = this.connectors.get(id);
    if (!registration) throw new ArgumentError(`autonomous connector is not registered: ${id}`);
    return registration;
  }

  registrations(): AutonomousConnectorRegistration[] {
    return [...this.connectors.values()].sort((left, right) => left.connector_id.localeCompare(right.connector_id));
  }

  get digest(): string { return digestJsonSync(this.registrations().map((registration) => registration.toJSON())); }

  planForDomains(domains: readonly AutonomousDomainName[], options: { capability?: string | null } = {}): AutonomousConnectorCoveragePlan {
    const requested = identifiers("autonomous connector plan domains", domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous connector plan domain is unsupported");
      return domain as AutonomousDomainName;
    });
    const capability = options.capability === undefined || options.capability === null ? null : capabilityIdentifier("autonomous connector plan capability", options.capability);
    const coverage: Record<string, AutonomousConnectorCoverageRow> = {};
    for (const domain of requested) {
      const candidates = this.registrations().filter((registration) => registration.manifest.domains.includes(domain) && (capability === null || registration.manifest.capabilities.includes(capability)));
      coverage[domain] = { status: candidates.length ? "selected" : "missing", connector_ids: candidates.map((item) => item.connector_id), manifest_digests: candidates.map((item) => item.manifest_digest), capability };
    }
    const selection = this.selectForDomains(requested, { capability });
    const descriptor = { schema: AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA, domains: requested, capability, coverage, registry_digest: this.digest, selection_plan_digest: selection.plan_digest, execution: "planning_only;no_dispatch;no_authorization" as const, secret_material: "never_returned" as const };
    return { ...descriptor, plan_digest: digestJsonSync(descriptor) };
  }

  selectForDomains(domains: readonly AutonomousDomainName[], options: { capability?: string | null; strategy?: AutonomousConnectorSelectionStrategy; selectionSignals?: Readonly<Record<string, JsonObject>> } = {}): AutonomousConnectorSelectionPlan {
    const requested = identifiers("autonomous connector selection domains", domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous connector selection domain is unsupported");
      return domain as AutonomousDomainName;
    });
    const capability = options.capability === undefined || options.capability === null ? null : capabilityIdentifier("autonomous connector selection capability", options.capability);
    const strategy = options.strategy ?? "lexicographic_connector_id";
    if (!AUTONOMOUS_CONNECTOR_SELECTION_STRATEGIES.includes(strategy)) throw new ArgumentError("autonomous connector selection strategy is invalid");
    if (strategy === "lexicographic_connector_id" && options.selectionSignals !== undefined) throw new ArgumentError("lexicographic connector selection cannot consume selection signals");
    if (strategy === "weighted_evidence" && options.selectionSignals === undefined) throw new ArgumentError("weighted connector selection requires selectionSignals");
    const normalizedSignals = new Map<string, AutonomousConnectorSelectionSignal>();
    if (options.selectionSignals !== undefined) {
      for (const [connectorId, signal] of Object.entries(options.selectionSignals)) {
        const id = identifier("autonomous connector selection signal connector_id", connectorId);
        if (!this.connectors.has(id)) throw new ArgumentError(`autonomous connector selection signal names an unknown connector: ${id}`);
        normalizedSignals.set(id, selectionSignal(id, signal));
      }
    }
    const signalDigest = strategy === "weighted_evidence" ? digestJsonSync([...normalizedSignals.values()].sort((left, right) => left.connector_id.localeCompare(right.connector_id))) : null;
    const rows = requested.map((domain) => {
      const candidates = this.registrations().filter((registration) => registration.manifest.domains.includes(domain) && (capability === null || registration.manifest.capabilities.includes(capability)));
      const descriptors = candidates.map((candidate) => normalizedSignals.get(candidate.connector_id) ?? selectionSignal(candidate.connector_id, undefined));
      const eligible = descriptors.map((descriptor) => strategy === "weighted_evidence" ? descriptor.eligible : true);
      const scores = descriptors.map((descriptor) => strategy === "weighted_evidence" ? descriptor.score : 0);
      const eligibleIndexes = eligible.map((value, index) => value ? index : -1).filter((index) => index >= 0);
      const selectedIndex = strategy === "weighted_evidence"
        ? [...eligibleIndexes].sort((left, right) => scores[right]! - scores[left]! || candidates[left]!.connector_id.localeCompare(candidates[right]!.connector_id))[0]
        : eligibleIndexes[0];
      const selected = selectedIndex === undefined ? undefined : candidates[selectedIndex];
      return new AutonomousConnectorSelectionRow({ domain, status: selected ? "selected" : "missing", connector_id: selected?.connector_id ?? null, manifest_digest: selected?.manifest_digest ?? null, candidate_ids: candidates.map((candidate) => candidate.connector_id), candidate_manifest_digests: candidates.map((candidate) => candidate.manifest_digest), candidate_scores: scores, candidate_eligible: eligible, reason: selected ? strategy : candidates.length ? "no_eligible_connector" : "no_matching_connector" });
    });
    return new AutonomousConnectorSelectionPlan({ domains: requested, capability, registry_digest: this.digest, rows, strategy, signal_digest: signalDigest });
  }

  selectAdaptiveForDomains(domains: readonly AutonomousDomainName[], capability: string, selectionSignals: Readonly<Record<string, JsonObject>>): AutonomousConnectorSelectionPlan {
    return this.selectForDomains(domains, { capability, strategy: "weighted_evidence", selectionSignals });
  }

  toJSON(): JsonObject {
    return { schema: AUTONOMOUS_CONNECTOR_REGISTRY_SCHEMA, digest: this.digest, connectors: this.registrations().map((registration) => registration.toJSON()), connector_count: this.connectors.size, execution: "metadata_only;registration_is_not_authorization", secret_material: "never_returned" };
  }
}

export class AutonomousConnectorDispatchRequest {
  readonly dispatch_id: string;
  readonly execution_id: string;
  readonly call_id: string;
  readonly connector_id: string;
  readonly domains: AutonomousDomainName[];
  readonly capability: string;
  readonly request: JsonObject;
  readonly parent_digests: string[];
  readonly attempt_id: string | null;
  readonly selection_plan_digest: string | null;
  readonly approved: boolean;

  constructor(input: { dispatch_id: string; execution_id: string; call_id: string; connector_id: string; domains: readonly AutonomousDomainName[]; capability: string; request: JsonObject; parent_digests?: readonly string[]; attempt_id?: string | null; selection_plan_digest?: string | null; approved?: boolean }) {
    this.dispatch_id = identifier("autonomous connector dispatch dispatch_id", input.dispatch_id);
    this.execution_id = identifier("autonomous connector dispatch execution_id", input.execution_id);
    this.call_id = identifier("autonomous connector dispatch call_id", input.call_id);
    this.connector_id = identifier("autonomous connector dispatch connector_id", input.connector_id);
    this.domains = identifiers("autonomous connector dispatch domains", input.domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous connector dispatch domain is unsupported");
      return domain as AutonomousDomainName;
    });
    this.capability = capabilityIdentifier("autonomous connector dispatch capability", input.capability);
    const safeRequest = safeJson("autonomous connector dispatch request", input.request, MAX_AUTONOMOUS_CONNECTOR_REQUEST_BYTES);
    if (!isObject(safeRequest)) throw new ArgumentError("autonomous connector dispatch request must be an object");
    this.request = safeRequest as JsonObject;
    if (input.parent_digests !== undefined && !Array.isArray(input.parent_digests)) throw new ArgumentError("autonomous connector dispatch parent_digests must be an array");
    this.parent_digests = [...(input.parent_digests ?? [])].map((value) => digest("autonomous connector dispatch parent digest", value) as string);
    if (this.parent_digests.length > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS) throw new ArgumentError("autonomous connector dispatch parent_digests exceeds its bound");
    this.attempt_id = input.attempt_id === undefined || input.attempt_id === null ? null : identifier("autonomous connector dispatch attempt_id", input.attempt_id);
    this.selection_plan_digest = digest("autonomous connector dispatch selection_plan_digest", input.selection_plan_digest, true);
    this.approved = input.approved ?? false;
    if (typeof this.approved !== "boolean") throw new ArgumentError("autonomous connector dispatch approved must be boolean");
  }

  get request_digest(): string {
    return digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA, dispatch_id: this.dispatch_id, execution_id: this.execution_id, call_id: this.call_id, connector_id: this.connector_id, domains: this.domains, capability: this.capability, request: this.request, parent_digests: this.parent_digests, attempt_id: this.attempt_id, selection_plan_digest: this.selection_plan_digest });
  }

  toMetadata(): JsonObject {
    return { schema: AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA, dispatch_id: this.dispatch_id, execution_id: this.execution_id, call_id: this.call_id, connector_id: this.connector_id, domains: [...this.domains], capability: this.capability, request_digest: this.request_digest, parent_digests: [...this.parent_digests], attempt_id: this.attempt_id, selection_plan_digest: this.selection_plan_digest, approved: this.approved, retention: "metadata_only_request_not_returned", secret_material: "never_returned" };
  }
}

export class AutonomousConnectorObservation {
  readonly value: JsonValue | null;
  readonly status: AutonomousConnectorDispatchStatus;
  readonly failure_class: string | null;

  constructor(value: unknown = null, status: AutonomousConnectorDispatchStatus = "observed", failureClass: string | null = null) {
    if (!AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES.includes(status)) throw new ArgumentError("autonomous connector observation status is invalid");
    this.value = value === undefined ? null : safeJson("autonomous connector observation value", value, MAX_AUTONOMOUS_CONNECTOR_RESULT_BYTES);
    this.status = status;
    this.failure_class = failureClass === null ? null : identifier("autonomous connector observation failure_class", failureClass);
  }
}

export class AutonomousConnectorDispatchReceipt {
  readonly dispatch_id: string;
  readonly execution_id: string;
  readonly call_id: string;
  readonly connector_id: string;
  readonly connector_version: string;
  readonly provider: string;
  readonly connector_kind: DomainEvidenceProviderConnectorKind;
  readonly manifest_digest: string;
  readonly domains: AutonomousDomainName[];
  readonly capability: string;
  readonly status: AutonomousConnectorDispatchStatus;
  readonly request_digest: string;
  readonly payload_digest: string | null;
  readonly parent_digests: string[];
  readonly attempt_id: string | null;
  readonly failure_class: string | null;

  constructor(input: { dispatch_id: string; execution_id: string; call_id: string; connector_id: string; connector_version: string; provider: string; connector_kind: DomainEvidenceProviderConnectorKind; manifest_digest: string; domains: readonly AutonomousDomainName[]; capability: string; status: AutonomousConnectorDispatchStatus; request_digest: string; payload_digest?: string | null; parent_digests?: readonly string[]; attempt_id?: string | null; failure_class?: string | null }) {
    this.dispatch_id = identifier("autonomous connector receipt dispatch_id", input.dispatch_id);
    this.execution_id = identifier("autonomous connector receipt execution_id", input.execution_id);
    this.call_id = identifier("autonomous connector receipt call_id", input.call_id);
    this.connector_id = identifier("autonomous connector receipt connector_id", input.connector_id);
    this.connector_version = identifier("autonomous connector receipt connector_version", input.connector_version);
    this.provider = identifier("autonomous connector receipt provider", input.provider);
    const connectorKinds: readonly DomainEvidenceProviderConnectorKind[] = ["literature", "clinical_trial", "fhir", "object_store", "provider_api"];
    if (!connectorKinds.includes(input.connector_kind)) throw new ArgumentError("autonomous connector receipt connector_kind is invalid");
    this.connector_kind = input.connector_kind;
    this.manifest_digest = digest("autonomous connector receipt manifest_digest", input.manifest_digest) as string;
    this.domains = identifiers("autonomous connector receipt domains", input.domains, AUTONOMOUS_DOMAIN_NAMES.length).map((domain) => {
      if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain as AutonomousDomainName)) throw new ArgumentError("autonomous connector receipt domain is unsupported");
      return domain as AutonomousDomainName;
    });
    this.capability = capabilityIdentifier("autonomous connector receipt capability", input.capability);
    if (!AUTONOMOUS_CONNECTOR_DISPATCH_STATUSES.includes(input.status)) throw new ArgumentError("autonomous connector receipt status is invalid");
    this.status = input.status;
    this.request_digest = digest("autonomous connector receipt request_digest", input.request_digest) as string;
    this.payload_digest = digest("autonomous connector receipt payload_digest", input.payload_digest, true);
    if (input.parent_digests !== undefined && !Array.isArray(input.parent_digests)) throw new ArgumentError("autonomous connector receipt parent_digests must be an array");
    this.parent_digests = [...(input.parent_digests ?? [])].map((value) => digest("autonomous connector receipt parent digest", value) as string);
    if (this.parent_digests.length > MAX_AUTONOMOUS_CONNECTOR_PARENT_DIGESTS) throw new ArgumentError("autonomous connector receipt parent_digests exceeds its bound");
    this.attempt_id = input.attempt_id === undefined || input.attempt_id === null ? null : identifier("autonomous connector receipt attempt_id", input.attempt_id);
    this.failure_class = input.failure_class === undefined || input.failure_class === null ? null : identifier("autonomous connector receipt failure_class", input.failure_class);
  }

  toJSON(): JsonObject {
    return { schema: AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA, dispatch_id: this.dispatch_id, execution_id: this.execution_id, call_id: this.call_id, connector_id: this.connector_id, connector_version: this.connector_version, provider: this.provider, connector_kind: this.connector_kind, manifest_digest: this.manifest_digest, domains: [...this.domains], capability: this.capability, status: this.status, request_digest: this.request_digest, payload_digest: this.payload_digest, parent_digests: [...this.parent_digests], attempt_id: this.attempt_id, failure_class: this.failure_class, retention: "metadata_only_no_request_or_payload", secret_material: "never_returned" };
  }
}

function receiptIdentity(receipt: AutonomousConnectorDispatchReceipt): string {
  return digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA, execution_id: receipt.execution_id, dispatch_id: receipt.dispatch_id, call_id: receipt.call_id, connector_id: receipt.connector_id, attempt_id: receipt.attempt_id });
}

function requestIdentity(request: AutonomousConnectorDispatchRequest): string {
  return digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA, execution_id: request.execution_id, dispatch_id: request.dispatch_id, call_id: request.call_id, connector_id: request.connector_id, attempt_id: request.attempt_id });
}

export class AutonomousConnectorReceiptJournalEntry {
  readonly sequence: number;
  readonly previous_entry_digest: string | null;
  readonly receipt: AutonomousConnectorDispatchReceipt;
  readonly receipt_identity_digest: string;
  readonly entry_digest: string;

  constructor(sequence: number, previousEntryDigest: string | null, receipt: AutonomousConnectorDispatchReceipt, receiptIdentityDigest?: string, entryDigest?: string) {
    if (!Number.isSafeInteger(sequence) || sequence < 1 || sequence > MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES) throw new ArgumentError("autonomous connector journal sequence is outside its bound");
    this.sequence = sequence;
    this.previous_entry_digest = digest("autonomous connector journal previous_entry_digest", previousEntryDigest, true);
    if (!(receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("autonomous connector journal receipt must be typed");
    this.receipt = receipt;
    this.receipt_identity_digest = digest("autonomous connector journal receipt_identity_digest", receiptIdentityDigest ?? receiptIdentity(receipt)) as string;
    if (this.receipt_identity_digest !== receiptIdentity(receipt)) throw new ArgumentError("autonomous connector journal receipt identity digest is invalid");
    const descriptor = { schema: AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA, sequence: this.sequence, previous_entry_digest: this.previous_entry_digest, receipt: receipt.toJSON(), receipt_identity_digest: this.receipt_identity_digest, retention: "metadata_only_hash_chained_no_request_or_payload" as const, secret_material: "never_returned" as const };
    const computed = digestJsonSync(descriptor);
    if (entryDigest !== undefined && entryDigest !== computed) throw new ArgumentError("autonomous connector journal entry digest is invalid");
    this.entry_digest = computed;
  }

  toJSON(): JsonObject {
    return { schema: AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA, sequence: this.sequence, previous_entry_digest: this.previous_entry_digest, receipt: this.receipt.toJSON(), receipt_identity_digest: this.receipt_identity_digest, entry_digest: this.entry_digest, retention: "metadata_only_hash_chained_no_request_or_payload", secret_material: "never_returned" };
  }
}

export class InMemoryAutonomousConnectorReceiptJournal implements AutonomousConnectorReceiptStore {
  private readonly rows: AutonomousConnectorReceiptJournalEntry[] = [];
  constructor(readonly maxEntries = MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES, readonly maxBytes = MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES) {
    if (!Number.isSafeInteger(maxEntries) || maxEntries < 1 || maxEntries > MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_ENTRIES) throw new ArgumentError("autonomous connector journal maxEntries is outside its bound");
    if (!Number.isSafeInteger(maxBytes) || maxBytes < 1 || maxBytes > MAX_AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_BYTES) throw new ArgumentError("autonomous connector journal maxBytes is outside its bound");
  }

  append(receipt: AutonomousConnectorDispatchReceipt): AutonomousConnectorReceiptJournalEntry {
    if (!(receipt instanceof AutonomousConnectorDispatchReceipt)) throw new ArgumentError("autonomous connector journal append requires a typed receipt");
    if (this.rows.length >= this.maxEntries) throw new ArgumentError("autonomous connector receipt journal exceeds maxEntries");
    const identity = receiptIdentity(receipt);
    if (this.rows.some((row) => row.receipt_identity_digest === identity)) throw new ArgumentError("autonomous connector receipt journal contains duplicate identities");
    const entry = new AutonomousConnectorReceiptJournalEntry(this.rows.length + 1, this.rows.at(-1)?.entry_digest ?? null, receipt);
    const nextBytes = bytes(JSON.stringify(entry.toJSON()));
    if (bytes(JSON.stringify(this.rows.map((row) => row.toJSON()))) + nextBytes > this.maxBytes) throw new ArgumentError("autonomous connector receipt journal exceeds maxBytes");
    this.rows.push(entry);
    return entry;
  }

  find(query: AutonomousConnectorReceiptLookup): AutonomousConnectorDispatchReceipt | null {
    const identity = digestJsonSync({ schema: AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA, execution_id: identifier("autonomous connector journal execution_id", query.execution_id), dispatch_id: identifier("autonomous connector journal dispatch_id", query.dispatch_id), call_id: identifier("autonomous connector journal call_id", query.call_id), connector_id: identifier("autonomous connector journal connector_id", query.connector_id), attempt_id: query.attempt_id === null ? null : identifier("autonomous connector journal attempt_id", query.attempt_id) });
    return [...this.rows].reverse().find((row) => row.receipt_identity_digest === identity)?.receipt ?? null;
  }

  receipts(options: { executionId?: string; connectorId?: string; afterSequence?: number; limit?: number } = {}): AutonomousConnectorReceiptJournalEntry[] {
    const executionId = options.executionId === undefined ? undefined : identifier("autonomous connector journal executionId", options.executionId);
    const connectorId = options.connectorId === undefined ? undefined : identifier("autonomous connector journal connectorId", options.connectorId);
    const afterSequence = options.afterSequence ?? 0;
    const limit = options.limit ?? 256;
    if (!Number.isSafeInteger(afterSequence) || afterSequence < 0) throw new ArgumentError("autonomous connector journal afterSequence must be non-negative");
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > this.maxEntries) throw new ArgumentError("autonomous connector journal limit is outside its bound");
    return this.rows.filter((row) => row.sequence > afterSequence && (executionId === undefined || row.receipt.execution_id === executionId) && (connectorId === undefined || row.receipt.connector_id === connectorId)).slice(0, limit);
  }

  verifyIntegrity(): { schema: typeof AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA; verified: true; entries: number; head_digest: string | null; retention: "metadata_only_hash_chained_no_request_or_payload"; secret_material: "never_returned" } {
    let previous: string | null = null;
    const identities = new Set<string>();
    for (const [index, row] of this.rows.entries()) {
      if (row.sequence !== index + 1 || row.previous_entry_digest !== previous || identities.has(row.receipt_identity_digest) || new AutonomousConnectorReceiptJournalEntry(row.sequence, row.previous_entry_digest, row.receipt, row.receipt_identity_digest, row.entry_digest).entry_digest !== row.entry_digest) throw new ArgumentError("autonomous connector receipt journal hash chain is invalid");
      identities.add(row.receipt_identity_digest);
      previous = row.entry_digest;
    }
    return { schema: AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA, verified: true, entries: this.rows.length, head_digest: previous, retention: "metadata_only_hash_chained_no_request_or_payload", secret_material: "never_returned" };
  }

  snapshot(): AutonomousConnectorReceiptJournalSnapshot {
    const descriptor = { schema: AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA, entries: this.rows.map((row) => row.toJSON()), head_digest: this.rows.at(-1)?.entry_digest ?? null, retention: "metadata_only_hash_chained_no_request_or_payload" as const, secret_material: "never_returned" as const };
    return { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
  }

  restore(snapshot: AutonomousConnectorReceiptJournalSnapshot): void {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_CONNECTOR_RECEIPT_JOURNAL_SCHEMA || !Array.isArray(snapshot.entries)) throw new ArgumentError("autonomous connector receipt journal snapshot is malformed");
    if (snapshot.retention !== "metadata_only_hash_chained_no_request_or_payload" || snapshot.secret_material !== "never_returned") throw new ArgumentError("autonomous connector receipt journal snapshot retention is invalid");
    const { snapshot_digest: observed, ...descriptor } = snapshot;
    if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("autonomous connector receipt journal snapshot digest is invalid");
    const restored = new InMemoryAutonomousConnectorReceiptJournal(this.maxEntries, this.maxBytes);
    for (const raw of snapshot.entries) {
      if (!isObject(raw) || !isObject(raw.receipt)) throw new ArgumentError("autonomous connector receipt journal snapshot entry is malformed");
      if (raw.schema !== AUTONOMOUS_CONNECTOR_RECEIPT_ENTRY_SCHEMA || raw.retention !== "metadata_only_hash_chained_no_request_or_payload" || raw.secret_material !== "never_returned") throw new ArgumentError("autonomous connector receipt journal snapshot entry retention is invalid");
      const receipt = raw.receipt;
      if (receipt.schema !== AUTONOMOUS_CONNECTOR_RECEIPT_SCHEMA || receipt.retention !== "metadata_only_no_request_or_payload" || receipt.secret_material !== "never_returned") throw new ArgumentError("autonomous connector receipt journal snapshot receipt retention is invalid");
      const typed = new AutonomousConnectorDispatchReceipt({ dispatch_id: receipt.dispatch_id as string, execution_id: receipt.execution_id as string, call_id: receipt.call_id as string, connector_id: receipt.connector_id as string, connector_version: receipt.connector_version as string, provider: receipt.provider as string, connector_kind: receipt.connector_kind as DomainEvidenceProviderConnectorKind, manifest_digest: receipt.manifest_digest as string, domains: receipt.domains as AutonomousDomainName[], capability: receipt.capability as string, status: receipt.status as AutonomousConnectorDispatchStatus, request_digest: receipt.request_digest as string, payload_digest: (receipt.payload_digest as string | null) ?? null, parent_digests: receipt.parent_digests as string[], attempt_id: (receipt.attempt_id as string | null) ?? null, failure_class: (receipt.failure_class as string | null) ?? null });
      const entry = restored.append(typed);
      const previousEntryDigest = raw.previous_entry_digest === undefined || raw.previous_entry_digest === null ? null : raw.previous_entry_digest as string;
      if (entry.entry_digest !== raw.entry_digest || entry.receipt_identity_digest !== raw.receipt_identity_digest || entry.previous_entry_digest !== previousEntryDigest) throw new ArgumentError("autonomous connector receipt journal snapshot entry digest is invalid");
    }
    if ((snapshot.head_digest ?? null) !== (restored.rows.at(-1)?.entry_digest ?? null)) throw new ArgumentError("autonomous connector receipt journal snapshot head digest is invalid");
    this.rows.splice(0, this.rows.length, ...restored.rows);
  }
}

/** Coordinates verified connector receipt snapshots with caller-owned durable storage. */
export class AutonomousConnectorReceiptJournalPersistenceCoordinator {
  readonly journal: InMemoryAutonomousConnectorReceiptJournal;
  readonly persistence: AutonomousConnectorReceiptJournalPersistence;

  constructor(journal: InMemoryAutonomousConnectorReceiptJournal, persistence: AutonomousConnectorReceiptJournalPersistence) {
    if (!(journal instanceof InMemoryAutonomousConnectorReceiptJournal)) throw new ArgumentError("connector receipt persistence requires an in-memory receipt journal");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("connector receipt persistence adapter is malformed");
    this.journal = journal;
    this.persistence = persistence;
  }

  async restore(): Promise<{ status: "empty" | "restored"; snapshot_digest: string | null; entries: number }> {
    const snapshot = await this.persistence.read();
    if (snapshot === null) return { status: "empty", snapshot_digest: null, entries: 0 };
    this.journal.restore(snapshot);
    const verified = this.journal.verifyIntegrity();
    return { status: "restored", snapshot_digest: snapshot.snapshot_digest, entries: verified.entries };
  }

  async flush(): Promise<AutonomousConnectorReceiptJournalSnapshot> {
    const snapshot = this.journal.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

export interface AutonomousConnectorDispatchResult {
  schema: typeof AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA;
  receipt: AutonomousConnectorDispatchReceipt;
  value: JsonValue | null;
  replay: "fresh" | "replayed";
  retention: "receipt_metadata_only;value_transient";
  secret_material: "never_returned";
}

export class AutonomousConnectorRuntime {
  readonly registry: AutonomousConnectorRegistry;
  readonly receiptStore?: AutonomousConnectorReceiptStore;
  readonly receiptSink?: (receipt: AutonomousConnectorDispatchReceipt) => unknown | Promise<unknown>;
  private readonly inFlight = new Map<string, Promise<AutonomousConnectorDispatchResult>>();

  constructor(registry: AutonomousConnectorRegistry, options: { receiptStore?: AutonomousConnectorReceiptStore; receiptSink?: (receipt: AutonomousConnectorDispatchReceipt) => unknown | Promise<unknown> } = {}) {
    if (!(registry instanceof AutonomousConnectorRegistry)) throw new ArgumentError("autonomous connector runtime requires a registry");
    if (options.receiptSink !== undefined && typeof options.receiptSink !== "function") throw new ArgumentError("autonomous connector receiptSink must be callable");
    if (options.receiptStore !== undefined && (typeof options.receiptStore.append !== "function" || typeof options.receiptStore.find !== "function")) throw new ArgumentError("autonomous connector receiptStore is malformed");
    this.registry = registry;
    this.receiptStore = options.receiptStore;
    this.receiptSink = options.receiptSink;
  }

  async dispatch(request: AutonomousConnectorDispatchRequest, options: { traceEventCallback?: AutonomousConnectorTraceEventCallback; authorizationContext?: AutonomousAuthorizationContext; authorizationDomain?: string; authorizationCapability?: string | null; authorizationRiskClass?: string | null } = {}): Promise<AutonomousConnectorDispatchResult> {
    if (!(request instanceof AutonomousConnectorDispatchRequest)) throw new ArgumentError("autonomous connector dispatch requires a typed request");
    if (options.traceEventCallback !== undefined && typeof options.traceEventCallback !== "function") throw new ArgumentError("autonomous connector traceEventCallback must be callable");
    if (options.authorizationContext !== undefined && typeof options.authorizationContext.authorizeOperation !== "function") throw new ArgumentError("autonomous connector authorizationContext must expose authorizeOperation");
    const registration = this.registry.resolve(request.connector_id);
    const replay = await this.findReplay(request, registration);
    if (replay) {
      await this.emitTrace(options.traceEventCallback, request, registration, "connector_started", "running");
      await this.emitTrace(options.traceEventCallback, request, registration, "connector_finished", traceStatus(replay.receipt.status), replay.receipt);
      return replay;
    }
    const identity = requestIdentity(request);
    const prior = this.inFlight.get(identity);
    if (prior) {
      await this.emitTrace(options.traceEventCallback, request, registration, "connector_started", "running");
      const outcome = await prior;
      await this.emitTrace(options.traceEventCallback, request, registration, "connector_finished", traceStatus(outcome.receipt.status), outcome.receipt);
      return { ...outcome, replay: "replayed", value: null };
    }
    const work = this.dispatchFresh(request, registration, options);
    this.inFlight.set(identity, work);
    try {
      return await work;
    } finally {
      if (this.inFlight.get(identity) === work) this.inFlight.delete(identity);
    }
  }

  async dispatchFromPlan(plan: AutonomousConnectorSelectionPlan | unknown, request: AutonomousConnectorDispatchRequest, options: { traceEventCallback?: AutonomousConnectorTraceEventCallback; authorizationContext?: AutonomousAuthorizationContext; authorizationDomain?: string; authorizationCapability?: string | null; authorizationRiskClass?: string | null } = {}): Promise<AutonomousConnectorDispatchResult> {
    const typedPlan = plan instanceof AutonomousConnectorSelectionPlan ? plan : AutonomousConnectorSelectionPlan.fromJSON(plan);
    if (!(request instanceof AutonomousConnectorDispatchRequest)) throw new ArgumentError("autonomous connector planned dispatch requires a typed request");
    typedPlan.verify(this.registry);
    if (typedPlan.capability !== request.capability || request.selection_plan_digest !== typedPlan.plan_digest) throw new ArgumentError("autonomous connector planned dispatch is not bound to the selection plan");
    const rows = new Map(typedPlan.rows.map((row) => [row.domain, row]));
    for (const domain of request.domains) {
      const row = rows.get(domain);
      if (!row || row.status !== "selected" || row.connector_id !== request.connector_id) throw new ArgumentError("autonomous connector planned dispatch does not select the requested connector");
    }
    return this.dispatch(request, options);
  }

  private async findReplay(request: AutonomousConnectorDispatchRequest, registration: AutonomousConnectorRegistration): Promise<AutonomousConnectorDispatchResult | null> {
    if (!this.receiptStore) return null;
    const stored = await this.receiptStore.find({ execution_id: request.execution_id, dispatch_id: request.dispatch_id, call_id: request.call_id, connector_id: request.connector_id, attempt_id: request.attempt_id });
    if (!stored) return null;
    if (stored.request_digest !== request.request_digest) throw new ArgumentError("autonomous connector replay identity conflicts with request metadata");
    if (stored.manifest_digest !== registration.manifest_digest) throw new ArgumentError("autonomous connector replay manifest digest changed");
    return { schema: AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA, receipt: stored, value: null, replay: "replayed", retention: "receipt_metadata_only;value_transient", secret_material: "never_returned" };
  }

  private async dispatchFresh(request: AutonomousConnectorDispatchRequest, registration: AutonomousConnectorRegistration, options: { traceEventCallback?: AutonomousConnectorTraceEventCallback; authorizationContext?: AutonomousAuthorizationContext; authorizationDomain?: string; authorizationCapability?: string | null; authorizationRiskClass?: string | null }): Promise<AutonomousConnectorDispatchResult> {
    await this.emitTrace(options.traceEventCallback, request, registration, "connector_started", "running");
    const missingDomains = request.domains.filter((domain) => !registration.manifest.domains.includes(domain));
    if (missingDomains.length) return this.finish(request, registration, "refused", "domain_scope", null, null, options.traceEventCallback);
    if (!registration.manifest.capabilities.includes(request.capability)) return this.finish(request, registration, "refused", "capability_scope", null, null, options.traceEventCallback);
    if (registration.approval_required && !request.approved) return this.finish(request, registration, "refused", "approval_required", null, null, options.traceEventCallback);
    if (options.authorizationContext) {
      options.authorizationContext.authorizeOperation({
        operation: "connector_dispatch",
        domains: options.authorizationDomain === undefined ? request.domains : undefined,
        domain: options.authorizationDomain,
        capability: options.authorizationCapability === undefined ? request.capability : options.authorizationCapability,
        riskClass: options.authorizationRiskClass,
        resourceDigest: request.request_digest,
      });
    }
    try {
      const raw = await registration.executor(registration.manifest, request.request);
      const observation = raw instanceof AutonomousConnectorObservation ? raw : new AutonomousConnectorObservation(raw);
      const payloadDigest = observation.value === null ? null : digestJsonSync(observation.value);
      return this.finish(request, registration, observation.status, observation.failure_class, payloadDigest, observation.value, options.traceEventCallback);
    } catch {
      return this.finish(request, registration, "error", "executor_error", null, null, options.traceEventCallback);
    }
  }

  private async finish(request: AutonomousConnectorDispatchRequest, registration: AutonomousConnectorRegistration, status: AutonomousConnectorDispatchStatus, failureClass: string | null, payloadDigest: string | null = null, value: JsonValue | null = null, traceEventCallback?: AutonomousConnectorTraceEventCallback): Promise<AutonomousConnectorDispatchResult> {
    const receipt = new AutonomousConnectorDispatchReceipt({ dispatch_id: request.dispatch_id, execution_id: request.execution_id, call_id: request.call_id, connector_id: registration.manifest.connector_id, connector_version: registration.manifest.version, provider: registration.manifest.provider, connector_kind: registration.manifest.connector_kind, manifest_digest: registration.manifest_digest, domains: request.domains, capability: request.capability, status, request_digest: request.request_digest, payload_digest: payloadDigest, parent_digests: request.parent_digests, attempt_id: request.attempt_id, failure_class: failureClass });
    let persisted = receipt;
    if (this.receiptStore) {
      try {
        const stored = await this.receiptStore.append(receipt);
        persisted = stored instanceof AutonomousConnectorReceiptJournalEntry ? stored.receipt : stored instanceof AutonomousConnectorDispatchReceipt ? stored : receipt;
      } catch (error) {
        throw new ArgumentError(`autonomous connector receipt store failed: ${error instanceof Error ? error.constructor.name : "UnknownError"}`);
      }
    }
    if (this.receiptSink) {
      try { await this.receiptSink(persisted); } catch (error) { throw new ArgumentError(`autonomous connector receipt sink failed: ${error instanceof Error ? error.constructor.name : "UnknownError"}`); }
    }
    await this.emitTrace(traceEventCallback, request, registration, "connector_finished", traceStatus(persisted.status), persisted);
    return { schema: AUTONOMOUS_CONNECTOR_DISPATCH_SCHEMA, receipt: persisted, value, replay: "fresh", retention: "receipt_metadata_only;value_transient", secret_material: "never_returned" };
  }

  private async emitTrace(
    callback: AutonomousConnectorTraceEventCallback | undefined,
    request: AutonomousConnectorDispatchRequest,
    registration: AutonomousConnectorRegistration,
    phase: AutonomousConnectorTraceEvent["phase"],
    status: AutonomousConnectorTraceEvent["status"],
    receipt?: AutonomousConnectorDispatchReceipt,
  ): Promise<void> {
    if (!callback) return;
    await callback({
      phase,
      status,
      domains: [...request.domains],
      route_digest: request.selection_plan_digest,
      selection_digest: request.selection_plan_digest,
      detail_digest: receipt?.payload_digest ?? (receipt ? null : request.request_digest),
      provider: registration.manifest.provider,
      failure_class: receipt?.failure_class ?? null,
      failure_code: receipt?.failure_class ?? null,
    });
  }
}

function traceStatus(status: AutonomousConnectorDispatchStatus): AutonomousConnectorTraceEvent["status"] {
  if (status === "observed") return "completed";
  if (status === "partial") return "partial";
  if (status === "refused") return "refused";
  if (status === "error") return "failed";
  return "unknown";
}
