import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousEvidencePlan,
} from "./autonomous-evidence.js";
import {
  AutonomousEvidenceExecutionController,
  AutonomousEvidenceExecutionPlan,
  AutonomousEvidenceExecutionResult,
  type AutonomousEvidenceExecutionOptions,
} from "./autonomous-evidence-execution.js";
import {
  AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
  AutonomousEvidenceRuntime,
  MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES,
  MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS,
  type AutonomousEvidenceAcquisitionRequest,
  type AutonomousEvidenceRuntimeJournal,
  type AutonomousEvidenceRuntimeJournalEntry,
} from "./autonomous-evidence-runtime.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject, JsonValue } from "./types.js";

/** Restart-safe metadata handoff for reviewed evidence source execution. */
export const AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-checkpoint/0.2" as const;
export const AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-resumable-result/0.1" as const;
export const AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-reconciliation-receipt/0.1" as const;
export const AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_POLICY_SCHEMA = "bioprism-typescript-autonomous-evidence-execution-resumable-policy/0.1" as const;
export const MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES = 128_000;
export const MAX_AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_BYTES = 128_000;

export type AutonomousEvidenceExecutionCheckpointStatus =
  | "approval_required"
  | "blocked"
  | "dispatch_pending"
  | "awaiting_evaluation"
  | "partial"
  | "failed"
  | "reconciliation_required"
  | "completed";

export interface AutonomousEvidenceExecutionCheckpointJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA;
  job_id: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_digest: string;
  readiness_report_digest: string;
  execution_policy_digest: string;
  required_requirement_count: number;
  checkpoint_generation: number;
  previous_checkpoint_digest: string | null;
  reconciliation_authority_id: string | null;
  reconciliation_authority_version: string | null;
  reconciliation_authority_config_digest: string | null;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  runtime_status: "completed" | "partial" | "awaiting_evaluation" | "failed" | "reconciliation_required" | null;
  runtime_result_digest: string | null;
  completed_request_count: number;
  pending_request_count: number;
  accepted_request_count: number;
  reconciliation_receipt_digest: string | null;
  checkpoint_digest: string;
  retention: "metadata_only;requests_readiness_and_source_values_caller_owned";
  secret_material: "never_returned";
}

export type AutonomousEvidenceExecutionReconciliationOutcome = "not_executed" | "succeeded" | "unknown";

export interface AutonomousEvidenceExecutionReconciliationOutcomeJSON extends JsonObject {
  request_index: number;
  request_digest: string;
  outcome: AutonomousEvidenceExecutionReconciliationOutcome;
  evidence_digest: string;
  evidence_kind: string;
  effect_absent: boolean;
  succeeded_receipt_digest: string | null;
}

export interface AutonomousEvidenceExecutionReconciliationReceiptJSON extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA;
  job_id: string;
  checkpoint_digest: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_set_digest: string;
  authority_id: string;
  authority_version: string;
  outcomes: AutonomousEvidenceExecutionReconciliationOutcomeJSON[];
  receipt_digest: string;
  retention: "metadata_only;source_values_and_reconciliation_evidence_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceExecutionReconciliationDecisionInput {
  outcome: AutonomousEvidenceExecutionReconciliationOutcome;
  evidenceDigest: string;
  evidenceKind: string;
  effectAbsent: boolean;
  succeededReceiptDigest?: string | null;
}

export interface AutonomousEvidenceExecutionReconciliationReceiptInput {
  jobId: string;
  checkpoint: AutonomousEvidenceExecutionCheckpointJSON;
  evidencePlan: AutonomousEvidencePlan;
  requests: readonly AutonomousEvidenceAcquisitionRequest[];
  authorityId: string;
  authorityVersion: string;
  outcomes: readonly AutonomousEvidenceExecutionReconciliationDecisionInput[];
}

/** Stable caller-owned identity for a result-affecting callback or mutable boundary. */
export interface AutonomousEvidenceExecutionResumableRoleIdentity extends JsonObject {
  id: string;
  version: string;
  config_digest?: string | null;
}

/**
 * Stable identities for JavaScript behavior that cannot be recovered from a function closure.
 * `journal` and `value_rehydrator` may be reserved before their boundaries are available so a
 * later replay can use those exact implementations without changing the checkpoint-bound policy.
 */
export interface AutonomousEvidenceExecutionResumablePolicyIdentity extends JsonObject {
  projector?: AutonomousEvidenceExecutionResumableRoleIdentity;
  evaluator?: AutonomousEvidenceExecutionResumableRoleIdentity;
  journal?: AutonomousEvidenceExecutionResumableRoleIdentity;
  value_rehydrator?: AutonomousEvidenceExecutionResumableRoleIdentity;
  retry_classifier?: AutonomousEvidenceExecutionResumableRoleIdentity;
  failover_observer?: AutonomousEvidenceExecutionResumableRoleIdentity;
  attempt_observer?: AutonomousEvidenceExecutionResumableRoleIdentity;
  clock?: AutonomousEvidenceExecutionResumableRoleIdentity;
  sleeper?: AutonomousEvidenceExecutionResumableRoleIdentity;
  source_boundary?: AutonomousEvidenceExecutionResumableRoleIdentity;
  authorization_context?: AutonomousEvidenceExecutionResumableRoleIdentity;
}

/** Deployment-owned trust-root identity; this is routing metadata, not a signature. */
export interface AutonomousEvidenceExecutionReconciliationAuthorityIdentity extends JsonObject {
  id: string;
  version: string;
  config_digest?: string | null;
}

export interface AutonomousEvidenceExecutionResumableControllerOptions {
  reconciliationAuthority?: AutonomousEvidenceExecutionReconciliationAuthorityIdentity;
}

export interface AutonomousEvidenceExecutionResumableOptions extends AutonomousEvidenceExecutionOptions {
  /** Stable identities/config digests for all custom execution behavior used across restarts. */
  executionPolicyIdentity?: AutonomousEvidenceExecutionResumablePolicyIdentity;
  reconciliationReceipt?: AutonomousEvidenceExecutionReconciliationReceiptJSON;
  /** @deprecated A boolean is not reconciliation evidence and never authorizes source dispatch. */
  resumeAfterReconciliation?: boolean;
}

export interface AutonomousEvidenceExecutionCheckpointStore {
  read(): Promise<AutonomousEvidenceExecutionCheckpointJSON | null> | AutonomousEvidenceExecutionCheckpointJSON | null;
  write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> | void;
  writeIfUnchanged?(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceExecutionCheckpointTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousEvidenceExecutionTransactionalCheckpointTextStore extends AutonomousEvidenceExecutionCheckpointTextStore {
  writeIfUnchanged(expectedCheckpointDigest: string | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousEvidenceExecutionResumableRunProjection extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA;
  job_id: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  checkpoint_digest: string;
  execution_plan_digest: string;
  evidence_result_digest: string | null;
  replayed: boolean;
  retention: "metadata_only;source_values_and_runtime_payloads_caller_owned";
  secret_material: "never_returned";
}

export interface AutonomousEvidenceExecutionResumableRun {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA;
  job_id: string;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  checkpoint: AutonomousEvidenceExecutionCheckpointJSON;
  result: AutonomousEvidenceExecutionResult | null;
  replayed: boolean;
  toJSON(): AutonomousEvidenceExecutionResumableRunProjection;
}

const RETENTION = "metadata_only;requests_readiness_and_source_values_caller_owned" as const;
const SECRET_MATERIAL = "never_returned" as const;
const RESULT_RETENTION = "metadata_only;source_values_and_runtime_payloads_caller_owned" as const;
const RECONCILIATION_RETENTION = "metadata_only;source_values_and_reconciliation_evidence_caller_owned" as const;
const STATUSES: readonly AutonomousEvidenceExecutionCheckpointStatus[] = [
  "approval_required",
  "blocked",
  "dispatch_pending",
  "awaiting_evaluation",
  "partial",
  "failed",
  "reconciliation_required",
  "completed",
];
const RUNTIME_STATUSES = ["completed", "partial", "awaiting_evaluation", "failed", "reconciliation_required"] as const;
const RECONCILIATION_OUTCOMES: readonly AutonomousEvidenceExecutionReconciliationOutcome[] = ["not_executed", "succeeded", "unknown"];
const CHECKPOINT_SECRET_KEYS = new Set(["apikey", "authorization", "bearer", "credential", "credentials", "password", "secret", "token", "privatekey", "refreshtoken"]);

interface AutonomousEvidenceExecutionCheckpointPayload {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA;
  job_id: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_digest: string;
  readiness_report_digest: string;
  execution_policy_digest: string;
  required_requirement_count: number;
  checkpoint_generation: number;
  previous_checkpoint_digest: string | null;
  reconciliation_authority_id: string | null;
  reconciliation_authority_version: string | null;
  reconciliation_authority_config_digest: string | null;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  runtime_status: (typeof RUNTIME_STATUSES)[number] | null;
  runtime_result_digest: string | null;
  completed_request_count: number;
  pending_request_count: number;
  accepted_request_count: number;
  reconciliation_receipt_digest: string | null;
}

interface NormalizedCheckpointRequest extends JsonObject {
  requirement_id: string;
  source_id: string;
  source_digest: string | null;
  request_id: string | null;
  metadata_digest: string;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function identifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function digest(name: string, value: unknown, nullable = false): string | null {
  if (nullable && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function allowedKeys(value: Record<string, unknown>, allowed: readonly string[], name: string): void {
  const accepted = new Set(allowed);
  if (Object.keys(value).some((key) => !accepted.has(key))) throw new ArgumentError(`${name} contains unsupported fields`);
}

type NormalizedRoleIdentity = { id: string; version: string; config_digest: string | null } | null;

interface NormalizedExecutionPolicyIdentities extends JsonObject {
  projector: NormalizedRoleIdentity;
  evaluator: NormalizedRoleIdentity;
  journal: NormalizedRoleIdentity;
  value_rehydrator: NormalizedRoleIdentity;
  retry_classifier: NormalizedRoleIdentity;
  failover_observer: NormalizedRoleIdentity;
  attempt_observer: NormalizedRoleIdentity;
  clock: NormalizedRoleIdentity;
  sleeper: NormalizedRoleIdentity;
  source_boundary: NormalizedRoleIdentity;
  authorization_context: NormalizedRoleIdentity;
}

function roleIdentity(
  name: keyof AutonomousEvidenceExecutionResumablePolicyIdentity,
  value: AutonomousEvidenceExecutionResumableRoleIdentity | undefined,
): NormalizedRoleIdentity {
  if (value === undefined) return null;
  if (!isObject(value)) throw new ArgumentError(`evidence execution resumable policy ${name} identity is malformed`);
  allowedKeys(value, ["id", "version", "config_digest"], `evidence execution resumable policy ${name} identity`);
  return {
    id: identifier(`evidence execution resumable policy ${name} id`, value.id),
    version: identifier(`evidence execution resumable policy ${name} version`, value.version),
    config_digest: digest(`evidence execution resumable policy ${name} config_digest`, value.config_digest, true),
  };
}

function executionPolicyIdentities(options: AutonomousEvidenceExecutionResumableOptions): NormalizedExecutionPolicyIdentities {
  const raw = options.executionPolicyIdentity;
  if (raw !== undefined) {
    if (!isObject(raw)) throw new ArgumentError("evidence execution executionPolicyIdentity is malformed");
    allowedKeys(raw, ["projector", "evaluator", "journal", "value_rehydrator", "retry_classifier", "failover_observer", "attempt_observer", "clock", "sleeper", "source_boundary", "authorization_context"], "evidence execution executionPolicyIdentity");
  }
  const identities: NormalizedExecutionPolicyIdentities = {
    projector: roleIdentity("projector", raw?.projector),
    evaluator: roleIdentity("evaluator", raw?.evaluator),
    journal: roleIdentity("journal", raw?.journal),
    value_rehydrator: roleIdentity("value_rehydrator", raw?.value_rehydrator),
    retry_classifier: roleIdentity("retry_classifier", raw?.retry_classifier),
    failover_observer: roleIdentity("failover_observer", raw?.failover_observer),
    attempt_observer: roleIdentity("attempt_observer", raw?.attempt_observer),
    clock: roleIdentity("clock", raw?.clock),
    sleeper: roleIdentity("sleeper", raw?.sleeper),
    source_boundary: roleIdentity("source_boundary", raw?.source_boundary),
    authorization_context: roleIdentity("authorization_context", raw?.authorization_context),
  };
  const callbackContracts: Array<[keyof NormalizedExecutionPolicyIdentities, unknown, string]> = [
    ["projector", options.projector, "projector"],
    ["evaluator", options.evaluator, "evaluator"],
    ["retry_classifier", options.classify, "classify"],
    ["failover_observer", options.observeFailover, "observeFailover"],
    ["attempt_observer", options.observeAttempt, "observeAttempt"],
    ["clock", options.clock, "clock"],
    ["sleeper", options.sleep, "sleep"],
    ["source_boundary", options.sourceBoundary, "sourceBoundary"],
    ["authorization_context", options.authorizationContext, "authorizationContext"],
  ];
  for (const [role, configured, optionName] of callbackContracts) {
    if (configured !== undefined && identities[role] === null) throw new ArgumentError(`resumable evidence execution requires ${role} identity for custom ${optionName} behavior`);
    if (configured === undefined && identities[role] !== null) throw new ArgumentError(`resumable evidence execution ${role} identity requires custom ${optionName} behavior`);
  }
  if (options.journal !== undefined && identities.journal === null) throw new ArgumentError("resumable evidence execution requires journal identity for a caller-owned runtime journal");
  if (options.rehydrateValue !== undefined && identities.value_rehydrator === null) throw new ArgumentError("resumable evidence execution requires value_rehydrator identity for a custom rehydrateValue callback");
  if (options.evaluator !== undefined) {
    const evaluator = identities.evaluator!;
    if (evaluator.id !== identifier("resumable evidence execution evaluator id", options.evaluator.evaluator_id)
        || evaluator.version !== identifier("resumable evidence execution evaluator version", options.evaluator.evaluator_version)) {
      throw new ArgumentError("resumable evidence execution evaluator identity does not match the configured evaluator");
    }
  }
  return identities;
}

function optionalBoolean(name: string, value: unknown, fallback: boolean): boolean {
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") throw new ArgumentError(`${name} must be boolean`);
  return value;
}

function policyDigestList(name: string, value: readonly string[] | undefined, maximum: number): string[] {
  if (value === undefined) return [];
  if (!Array.isArray(value) || value.length > maximum) throw new ArgumentError(`${name} must be a bounded array`);
  // Order is retained because it is delivered to every projector/acquirer context and may be
  // behaviorally significant to caller-owned code.
  const normalized = value.map((item, index) => digest(`${name}[${index}]`, item) as string);
  if (new Set(normalized).size !== normalized.length) throw new ArgumentError(`${name} must not contain duplicates`);
  return normalized;
}

function overrideIdentifier(name: string, value: unknown): JsonObject {
  if (value === undefined) return { state: "inherit" };
  if (value === null) return { state: "explicit", value: null };
  return { state: "explicit", value: identifier(name, value) };
}

function executionPolicyDigest(options: AutonomousEvidenceExecutionResumableOptions): string {
  const identities = executionPolicyIdentities(options);
  return digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_POLICY_SCHEMA,
    implementation_identities: identities,
    parent_evidence_digests: policyDigestList("resumable evidence execution parentEvidenceDigests", options.parentEvidenceDigests, 64),
    stop_on_failure: optionalBoolean("resumable evidence execution stopOnFailure", options.stopOnFailure, false),
    reevaluate_pending: optionalBoolean("resumable evidence execution reevaluatePending", options.reevaluatePending, false),
    authorization_domain_override: overrideIdentifier("resumable evidence execution authorizationDomain", options.authorizationDomain),
    authorization_capability_override: overrideIdentifier("resumable evidence execution authorizationCapability", options.authorizationCapability),
    authorization_risk_class_override: overrideIdentifier("resumable evidence execution authorizationRiskClass", options.authorizationRiskClass),
    provider_contracts_configured: options.providerContracts !== undefined,
  });
}

function reconciliationAuthority(
  value: AutonomousEvidenceExecutionReconciliationAuthorityIdentity | undefined,
): { id: string; version: string; config_digest: string | null } | null {
  if (value === undefined) return null;
  if (!isObject(value)) throw new ArgumentError("evidence execution reconciliation authority is malformed");
  allowedKeys(value, ["id", "version", "config_digest"], "evidence execution reconciliation authority");
  return {
    id: identifier("evidence execution reconciliation authority id", value.id),
    version: identifier("evidence execution reconciliation authority version", value.version),
    config_digest: digest("evidence execution reconciliation authority config_digest", value.config_digest, true),
  };
}

function planRequirementIds(evidencePlan: AutonomousEvidencePlan): string[] {
  if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution checkpoint requires a typed evidence plan");
  const requirementIds = evidencePlan.requirements.map((requirement, index) => identifier(`evidence execution requirement ${index} id`, requirement.requirement_id));
  if (requirementIds.length < 1 || requirementIds.length > 128 || new Set(requirementIds).size !== requirementIds.length) {
    throw new ArgumentError("evidence execution checkpoint requires 1..128 unique plan requirements");
  }
  return requirementIds;
}

function assertCheckpointMetadata(value: unknown, name: string, depth = 0): void {
  if (depth > 16) throw new ArgumentError(`${name} is too deeply nested`);
  if (Array.isArray(value)) {
    if (value.length > 512) throw new ArgumentError(`${name} contains too many entries`);
    value.forEach((item, index) => assertCheckpointMetadata(item, `${name}[${index}]`, depth + 1));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      if (CHECKPOINT_SECRET_KEYS.has(key.toLowerCase().replace(/[^a-z0-9]/g, ""))) throw new ArgumentError(`${name}.${key} is credential-shaped metadata`);
      assertCheckpointMetadata(child, `${name}.${key}`, depth + 1);
    }
    return;
  }
  if (typeof value === "number" && !Number.isFinite(value)) throw new ArgumentError(`${name} contains a non-finite number`);
}

function checkpointMetadataBytes(value: unknown, name: string): number {
  try {
    const size = bytes(canonicalJson(value));
    if (size > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_METADATA_BYTES) throw new ArgumentError(`${name} exceeds its byte bound`);
    return size;
  } catch (error) {
    if (error instanceof ArgumentError) throw error;
    throw new ArgumentError(`${name} must be JSON-safe`);
  }
}

function normalizeRequests(requests: readonly AutonomousEvidenceAcquisitionRequest[]): NormalizedCheckpointRequest[] {
  if (!Array.isArray(requests) || requests.length < 1 || requests.length > 128) throw new ArgumentError("evidence execution checkpoint requests are outside their bound");
  return requests.map((request, index) => {
    if (!isObject(request)) throw new ArgumentError(`evidence execution checkpoint request ${index} is malformed`);
    const metadata = request.metadata ?? {};
    if (!isObject(metadata)) throw new ArgumentError(`evidence execution checkpoint request ${index} metadata is malformed`);
    assertCheckpointMetadata(metadata, `evidence execution checkpoint request ${index} metadata`);
    checkpointMetadataBytes(metadata, `evidence execution checkpoint request ${index} metadata`);
    return {
      requirement_id: identifier(`evidence execution checkpoint request ${index} requirement_id`, request.requirement_id),
      source_id: identifier(`evidence execution checkpoint request ${index} source_id`, request.source_id),
      source_digest: digest(`evidence execution checkpoint request ${index} source_digest`, request.source_digest, true),
      request_id: request.request_id === undefined || request.request_id === null ? null : identifier(`evidence execution checkpoint request ${index} request_id`, request.request_id),
      metadata_digest: digestJsonSync(metadata),
    };
  });
}

function requestsDigest(requests: readonly AutonomousEvidenceAcquisitionRequest[]): string {
  const normalized = normalizeRequests(requests);
  const identities = normalized.map((request) => digestJsonSync(request));
  if (new Set(identities).size !== identities.length) throw new ArgumentError("evidence execution checkpoint requests contain duplicates");
  return digestJsonSync({ schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA, requests: normalized });
}

function runtimeRequestDigests(evidencePlan: AutonomousEvidencePlan, requests: readonly AutonomousEvidenceAcquisitionRequest[]): string[] {
  if (!(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution reconciliation requires a typed evidence plan");
  const normalized = normalizeRequests(requests);
  const requestDigests = normalized.map((request, index) => digestJsonSync({
    schema: AUTONOMOUS_EVIDENCE_RUNTIME_SCHEMA,
    plan_digest: evidencePlan.plan_digest,
    requirement_id: request.requirement_id,
    source_id: request.source_id,
    source_digest: request.source_digest,
    request_id: request.request_id,
    metadata: requests[index]!.metadata ?? {},
  }));
  if (new Set(requestDigests).size !== requestDigests.length) throw new ArgumentError("evidence execution reconciliation requests must have unique runtime identities");
  return requestDigests;
}

interface AutonomousEvidenceExecutionReconciliationReceiptPayload extends JsonObject {
  schema: typeof AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA;
  job_id: string;
  checkpoint_digest: string;
  evidence_plan_digest: string;
  execution_plan_digest: string;
  request_set_digest: string;
  authority_id: string;
  authority_version: string;
  outcomes: AutonomousEvidenceExecutionReconciliationOutcomeJSON[];
  retention: typeof RECONCILIATION_RETENTION;
  secret_material: typeof SECRET_MATERIAL;
}

function reconciliationReceiptPayload(input: AutonomousEvidenceExecutionReconciliationReceiptPayload): AutonomousEvidenceExecutionReconciliationReceiptPayload {
  return input;
}

function validateReconciliationReceipt(value: unknown): AutonomousEvidenceExecutionReconciliationReceiptJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA) throw new ArgumentError("evidence execution reconciliation receipt schema is invalid");
  allowedKeys(value, ["schema", "job_id", "checkpoint_digest", "evidence_plan_digest", "execution_plan_digest", "request_set_digest", "authority_id", "authority_version", "outcomes", "receipt_digest", "retention", "secret_material"], "evidence execution reconciliation receipt");
  if (!Array.isArray(value.outcomes) || value.outcomes.length < 1 || value.outcomes.length > 128) throw new ArgumentError("evidence execution reconciliation outcomes are outside their bound");
  const outcomes = value.outcomes.map((raw, index) => {
    if (!isObject(raw)) throw new ArgumentError(`evidence execution reconciliation outcome ${index} is malformed`);
    allowedKeys(raw, ["request_index", "request_digest", "outcome", "evidence_digest", "evidence_kind", "effect_absent", "succeeded_receipt_digest"], `evidence execution reconciliation outcome ${index}`);
    const requestIndex = integer(`evidence execution reconciliation outcome ${index} request_index`, raw.request_index, 0, 127);
    if (requestIndex !== index) throw new ArgumentError("evidence execution reconciliation outcomes must be in exact request order");
    const outcome = raw.outcome as AutonomousEvidenceExecutionReconciliationOutcome;
    if (!RECONCILIATION_OUTCOMES.includes(outcome)) throw new ArgumentError(`evidence execution reconciliation outcome ${index} is invalid`);
    if (typeof raw.effect_absent !== "boolean" || (outcome === "not_executed") !== raw.effect_absent) throw new ArgumentError(`evidence execution reconciliation outcome ${index} contradicts effect_absent`);
    const succeededReceiptDigest = digest(`evidence execution reconciliation outcome ${index} succeeded_receipt_digest`, raw.succeeded_receipt_digest, true);
    if (outcome === "succeeded" && succeededReceiptDigest === null) throw new ArgumentError(`evidence execution reconciliation outcome ${index} requires its succeeded journal receipt digest`);
    if (outcome !== "succeeded" && succeededReceiptDigest !== null) throw new ArgumentError(`evidence execution reconciliation outcome ${index} cannot carry a succeeded journal receipt digest`);
    return {
      request_index: requestIndex,
      request_digest: digest(`evidence execution reconciliation outcome ${index} request_digest`, raw.request_digest) as string,
      outcome,
      evidence_digest: digest(`evidence execution reconciliation outcome ${index} evidence_digest`, raw.evidence_digest) as string,
      evidence_kind: identifier(`evidence execution reconciliation outcome ${index} evidence_kind`, raw.evidence_kind),
      effect_absent: raw.effect_absent,
      succeeded_receipt_digest: succeededReceiptDigest,
    } satisfies AutonomousEvidenceExecutionReconciliationOutcomeJSON;
  });
  if (value.retention !== RECONCILIATION_RETENTION || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evidence execution reconciliation receipt retention contract is invalid");
  const payload = reconciliationReceiptPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA,
    job_id: identifier("evidence execution reconciliation receipt job_id", value.job_id),
    checkpoint_digest: digest("evidence execution reconciliation receipt checkpoint_digest", value.checkpoint_digest) as string,
    evidence_plan_digest: digest("evidence execution reconciliation receipt evidence_plan_digest", value.evidence_plan_digest) as string,
    execution_plan_digest: digest("evidence execution reconciliation receipt execution_plan_digest", value.execution_plan_digest) as string,
    request_set_digest: digest("evidence execution reconciliation receipt request_set_digest", value.request_set_digest) as string,
    authority_id: identifier("evidence execution reconciliation receipt authority_id", value.authority_id),
    authority_version: identifier("evidence execution reconciliation receipt authority_version", value.authority_version),
    outcomes,
    retention: RECONCILIATION_RETENTION,
    secret_material: SECRET_MATERIAL,
  });
  const observedDigest = digest("evidence execution reconciliation receipt receipt_digest", value.receipt_digest) as string;
  if (digestJsonSync(payload) !== observedDigest) throw new ArgumentError("evidence execution reconciliation receipt digest is invalid");
  const result = { ...payload, receipt_digest: observedDigest } satisfies AutonomousEvidenceExecutionReconciliationReceiptJSON;
  if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_BYTES) throw new ArgumentError("evidence execution reconciliation receipt exceeds its bound");
  return clone(result);
}

export function validateAutonomousEvidenceExecutionReconciliationReceipt(value: unknown): AutonomousEvidenceExecutionReconciliationReceiptJSON {
  return validateReconciliationReceipt(value);
}

export function createAutonomousEvidenceExecutionReconciliationReceipt(input: AutonomousEvidenceExecutionReconciliationReceiptInput): AutonomousEvidenceExecutionReconciliationReceiptJSON {
  if (!input || !(input.evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution reconciliation receipt input is malformed");
  const checkpoint = validateCheckpoint(input.checkpoint);
  if (!["dispatch_pending", "reconciliation_required"].includes(checkpoint.status)) throw new ArgumentError("evidence execution reconciliation receipt requires an uncertain-dispatch checkpoint");
  const jobId = identifier("evidence execution reconciliation receipt jobId", input.jobId);
  if (checkpoint.job_id !== jobId) throw new ArgumentError("evidence execution reconciliation receipt job does not match its checkpoint");
  if (checkpoint.evidence_plan_digest !== input.evidencePlan.plan_digest) throw new ArgumentError("evidence execution reconciliation receipt evidence plan does not match its checkpoint");
  if (checkpoint.reconciliation_authority_id === null || checkpoint.reconciliation_authority_version === null) throw new ArgumentError("evidence execution reconciliation is unavailable because its checkpoint has no configured authority");
  const authorityId = identifier("evidence execution reconciliation receipt authorityId", input.authorityId);
  const authorityVersion = identifier("evidence execution reconciliation receipt authorityVersion", input.authorityVersion);
  if (authorityId !== checkpoint.reconciliation_authority_id || authorityVersion !== checkpoint.reconciliation_authority_version) throw new ArgumentError("evidence execution reconciliation receipt authority does not match its checkpoint trust root");
  const requestSetDigest = requestsDigest(input.requests);
  if (checkpoint.request_digest !== requestSetDigest) throw new ArgumentError("evidence execution reconciliation receipt request set does not match its checkpoint");
  if (!Array.isArray(input.outcomes) || input.outcomes.length !== input.requests.length) throw new ArgumentError("evidence execution reconciliation receipt must classify every request exactly once");
  const requestDigests = runtimeRequestDigests(input.evidencePlan, input.requests);
  const outcomes = input.outcomes.map((decision, index) => {
    if (!decision || !RECONCILIATION_OUTCOMES.includes(decision.outcome)) throw new ArgumentError(`evidence execution reconciliation decision ${index} is invalid`);
    if (typeof decision.effectAbsent !== "boolean" || (decision.outcome === "not_executed") !== decision.effectAbsent) throw new ArgumentError(`evidence execution reconciliation decision ${index} contradicts effectAbsent`);
    const succeededReceiptDigest = digest(`evidence execution reconciliation decision ${index} succeededReceiptDigest`, decision.succeededReceiptDigest, true);
    if (decision.outcome === "succeeded" && succeededReceiptDigest === null) throw new ArgumentError(`evidence execution reconciliation decision ${index} requires its succeeded journal receipt digest`);
    if (decision.outcome !== "succeeded" && succeededReceiptDigest !== null) throw new ArgumentError(`evidence execution reconciliation decision ${index} cannot carry a succeeded journal receipt digest`);
    return {
      request_index: index,
      request_digest: requestDigests[index]!,
      outcome: decision.outcome,
      evidence_digest: digest(`evidence execution reconciliation decision ${index} evidenceDigest`, decision.evidenceDigest) as string,
      evidence_kind: identifier(`evidence execution reconciliation decision ${index} evidenceKind`, decision.evidenceKind),
      effect_absent: decision.effectAbsent,
      succeeded_receipt_digest: succeededReceiptDigest,
    } satisfies AutonomousEvidenceExecutionReconciliationOutcomeJSON;
  });
  const payload = reconciliationReceiptPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_RECONCILIATION_RECEIPT_SCHEMA,
    job_id: jobId,
    checkpoint_digest: checkpoint.checkpoint_digest,
    evidence_plan_digest: checkpoint.evidence_plan_digest,
    execution_plan_digest: checkpoint.execution_plan_digest,
    request_set_digest: requestSetDigest,
    authority_id: authorityId,
    authority_version: authorityVersion,
    outcomes,
    retention: RECONCILIATION_RETENTION,
    secret_material: SECRET_MATERIAL,
  });
  return validateReconciliationReceipt({ ...payload, receipt_digest: digestJsonSync(payload) });
}

function checkpointPayload(input: AutonomousEvidenceExecutionCheckpointPayload): AutonomousEvidenceExecutionCheckpointPayload {
  return input;
}

function checkpointDigest(input: AutonomousEvidenceExecutionCheckpointPayload): string {
  return digestJsonSync(checkpointPayload(input));
}

function checkpointFor(input: {
  jobId: string;
  executionPlan: AutonomousEvidenceExecutionPlan;
  requestDigest: string;
  executionPolicyDigest: string;
  requiredRequirementIds: readonly string[];
  previousCheckpoint: AutonomousEvidenceExecutionCheckpointJSON | null;
  reconciliationAuthority: { id: string; version: string; config_digest: string | null } | null;
  status: AutonomousEvidenceExecutionCheckpointStatus;
  result?: AutonomousEvidenceExecutionResult | null;
  reconciliationReceiptDigest?: string | null;
}): AutonomousEvidenceExecutionCheckpointJSON {
  const runtime = input.result?.runtime.toJSON() ?? null;
  const previous = input.previousCheckpoint === null ? null : validateCheckpoint(input.previousCheckpoint);
  const requiredRequirementIds = input.requiredRequirementIds.map((requirementId, index) => identifier(`evidence execution checkpoint required requirement ${index}`, requirementId));
  if (requiredRequirementIds.length < 1 || requiredRequirementIds.length > 128 || new Set(requiredRequirementIds).size !== requiredRequirementIds.length) throw new ArgumentError("evidence execution checkpoint required requirements are not unique");
  if (runtime !== null) {
    const required = new Set(requiredRequirementIds);
    const runtimeGroups = [
      ["completed", runtime.completed_requirement_ids],
      ["pending evaluation", runtime.pending_evaluation_requirement_ids],
      ["missing", runtime.missing_requirement_ids],
    ] as const;
    for (const [name, ids] of runtimeGroups) {
      if (new Set(ids).size !== ids.length || ids.some((requirementId) => !required.has(requirementId))) throw new ArgumentError(`evidence execution runtime ${name} requirements do not belong uniquely to the checkpoint plan`);
    }
    if (input.status === "completed" && (runtime.completed_requirement_ids.length !== required.size || runtime.completed_requirement_ids.some((requirementId) => !required.has(requirementId)))) {
      throw new ArgumentError("completed evidence execution runtime does not cover every unique plan requirement");
    }
  }
  const payload = checkpointPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: identifier("evidence execution checkpoint job_id", input.jobId),
    evidence_plan_digest: input.executionPlan.evidence_plan_digest,
    execution_plan_digest: input.executionPlan.plan_digest,
    request_digest: input.requestDigest,
    readiness_report_digest: input.executionPlan.readiness.report_digest,
    execution_policy_digest: digest("evidence execution checkpoint execution policy digest", input.executionPolicyDigest) as string,
    required_requirement_count: requiredRequirementIds.length,
    checkpoint_generation: previous === null ? 1 : previous.checkpoint_generation + 1,
    previous_checkpoint_digest: previous?.checkpoint_digest ?? null,
    reconciliation_authority_id: input.reconciliationAuthority?.id ?? null,
    reconciliation_authority_version: input.reconciliationAuthority?.version ?? null,
    reconciliation_authority_config_digest: input.reconciliationAuthority?.config_digest ?? null,
    status: input.status,
    runtime_status: runtime?.status ?? null,
    runtime_result_digest: runtime?.result_digest ?? null,
    completed_request_count: runtime?.completed_requirement_ids.length ?? 0,
    pending_request_count: (runtime?.pending_evaluation_requirement_ids.length ?? 0) + (runtime?.missing_requirement_ids.length ?? 0),
    accepted_request_count: runtime?.assessments.filter((assessment) => assessment.verdict === "accepted").length ?? 0,
    reconciliation_receipt_digest: digest("evidence execution checkpoint reconciliation receipt digest", input.reconciliationReceiptDigest, true),
  });
  return {
    ...payload,
    checkpoint_digest: checkpointDigest(payload),
    retention: RETENTION,
    secret_material: SECRET_MATERIAL,
  };
}

function statusForResult(result: AutonomousEvidenceExecutionResult): AutonomousEvidenceExecutionCheckpointStatus {
  switch (result.status) {
    case "completed": return "completed";
    case "awaiting_evaluation": return "awaiting_evaluation";
    case "partial": return "partial";
    case "failed": return "failed";
    case "reconciliation_required": return "reconciliation_required";
    default: return "reconciliation_required";
  }
}

function validateCheckpoint(value: unknown): AutonomousEvidenceExecutionCheckpointJSON {
  if (!isObject(value) || value.schema !== AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA) throw new ArgumentError("evidence execution checkpoint schema is invalid");
  allowedKeys(value, ["schema", "job_id", "evidence_plan_digest", "execution_plan_digest", "request_digest", "readiness_report_digest", "execution_policy_digest", "required_requirement_count", "checkpoint_generation", "previous_checkpoint_digest", "reconciliation_authority_id", "reconciliation_authority_version", "reconciliation_authority_config_digest", "status", "runtime_status", "runtime_result_digest", "completed_request_count", "pending_request_count", "accepted_request_count", "reconciliation_receipt_digest", "checkpoint_digest", "retention", "secret_material"], "evidence execution checkpoint");
  const status = value.status as AutonomousEvidenceExecutionCheckpointStatus;
  if (!STATUSES.includes(status)) throw new ArgumentError("evidence execution checkpoint status is invalid");
  const runtimeStatus = value.runtime_status === null ? null : value.runtime_status as (typeof RUNTIME_STATUSES)[number];
  if (runtimeStatus !== null && !RUNTIME_STATUSES.includes(runtimeStatus)) throw new ArgumentError("evidence execution checkpoint runtime status is invalid");
  const runtimeResultDigest = digest("evidence execution checkpoint runtime_result_digest", value.runtime_result_digest, true);
  const normalized = checkpointPayload({
    schema: AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_SCHEMA,
    job_id: identifier("evidence execution checkpoint job_id", value.job_id),
    evidence_plan_digest: digest("evidence execution checkpoint evidence_plan_digest", value.evidence_plan_digest) as string,
    execution_plan_digest: digest("evidence execution checkpoint execution_plan_digest", value.execution_plan_digest) as string,
    request_digest: digest("evidence execution checkpoint request_digest", value.request_digest) as string,
    readiness_report_digest: digest("evidence execution checkpoint readiness_report_digest", value.readiness_report_digest) as string,
    execution_policy_digest: digest("evidence execution checkpoint execution_policy_digest", value.execution_policy_digest) as string,
    required_requirement_count: integer("evidence execution checkpoint required_requirement_count", value.required_requirement_count, 1, 128),
    checkpoint_generation: integer("evidence execution checkpoint checkpoint_generation", value.checkpoint_generation, 1, Number.MAX_SAFE_INTEGER),
    previous_checkpoint_digest: digest("evidence execution checkpoint previous_checkpoint_digest", value.previous_checkpoint_digest, true),
    reconciliation_authority_id: value.reconciliation_authority_id === null ? null : identifier("evidence execution checkpoint reconciliation_authority_id", value.reconciliation_authority_id),
    reconciliation_authority_version: value.reconciliation_authority_version === null ? null : identifier("evidence execution checkpoint reconciliation_authority_version", value.reconciliation_authority_version),
    reconciliation_authority_config_digest: digest("evidence execution checkpoint reconciliation_authority_config_digest", value.reconciliation_authority_config_digest, true),
    status,
    runtime_status: runtimeStatus,
    runtime_result_digest: runtimeResultDigest,
    completed_request_count: integer("evidence execution checkpoint completed_request_count", value.completed_request_count, 0, 128),
    pending_request_count: integer("evidence execution checkpoint pending_request_count", value.pending_request_count, 0, 256),
    accepted_request_count: integer("evidence execution checkpoint accepted_request_count", value.accepted_request_count, 0, 128),
    reconciliation_receipt_digest: digest("evidence execution checkpoint reconciliation_receipt_digest", value.reconciliation_receipt_digest, true),
  });
  if ((normalized.checkpoint_generation === 1) !== (normalized.previous_checkpoint_digest === null)) throw new ArgumentError("evidence execution checkpoint lineage is inconsistent");
  if ((normalized.reconciliation_authority_id === null) !== (normalized.reconciliation_authority_version === null)) throw new ArgumentError("evidence execution checkpoint reconciliation authority is incomplete");
  if (normalized.reconciliation_authority_id === null && normalized.reconciliation_authority_config_digest !== null) throw new ArgumentError("evidence execution checkpoint reconciliation authority config has no authority identity");
  const hasRuntime = normalized.runtime_status !== null || normalized.runtime_result_digest !== null || normalized.completed_request_count > 0 || normalized.pending_request_count > 0 || normalized.accepted_request_count > 0;
  if (["approval_required", "blocked", "dispatch_pending"].includes(status) && hasRuntime) throw new ArgumentError("pre-dispatch evidence execution checkpoint cannot contain runtime state");
  if (normalized.completed_request_count > normalized.required_requirement_count) throw new ArgumentError("evidence execution checkpoint completed count exceeds its required plan coverage");
  if (normalized.pending_request_count > normalized.required_requirement_count) throw new ArgumentError("evidence execution checkpoint pending count exceeds its required plan coverage");
  if (normalized.accepted_request_count < normalized.completed_request_count) throw new ArgumentError("evidence execution checkpoint has fewer accepted receipts than completed requirements");
  if (["completed", "awaiting_evaluation", "partial", "failed"].includes(status) &&
      (normalized.runtime_status !== status || normalized.runtime_result_digest === null)) throw new ArgumentError("post-dispatch evidence execution checkpoint status does not match its runtime");
  if (status === "completed" && (normalized.completed_request_count !== normalized.required_requirement_count || normalized.pending_request_count !== 0)) throw new ArgumentError("completed evidence execution checkpoint does not cover every required plan requirement");
  // An awaiting-evaluation result may have completed every source requirement while the
  // caller-owned evaluator still owns the final acceptance boundary. In that case there are no
  // missing/pending source requests to count; partial source coverage still must expose at least
  // one pending or missing requirement.
  if (status === "partial" && normalized.pending_request_count === 0) throw new ArgumentError("incomplete evidence execution checkpoint requires pending requests");
  if (status === "failed" && normalized.completed_request_count !== 0) throw new ArgumentError("failed evidence execution checkpoint cannot contain completed requests");
  if (status === "reconciliation_required" && hasRuntime && (normalized.runtime_status !== "reconciliation_required" || normalized.runtime_result_digest === null)) throw new ArgumentError("evidence execution reconciliation checkpoint runtime state is inconsistent");
  if (["approval_required", "blocked"].includes(status) && normalized.reconciliation_receipt_digest !== null) throw new ArgumentError("pre-approval evidence execution checkpoint cannot carry reconciliation authority");
  if (value.retention !== RETENTION || value.secret_material !== SECRET_MATERIAL) throw new ArgumentError("evidence execution checkpoint retention contract is invalid");
  const observedDigest = digest("evidence execution checkpoint checkpoint_digest", value.checkpoint_digest) as string;
  if (checkpointDigest(normalized) !== observedDigest) throw new ArgumentError("evidence execution checkpoint digest is invalid");
  const result = { ...normalized, checkpoint_digest: observedDigest, retention: RETENTION, secret_material: SECRET_MATERIAL };
  if (bytes(canonicalJson(result)) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES) throw new ArgumentError("evidence execution checkpoint exceeds its bound");
  return clone(result);
}

export function validateAutonomousEvidenceExecutionCheckpoint(value: unknown): AutonomousEvidenceExecutionCheckpointJSON {
  return validateCheckpoint(value);
}

function encodeCheckpoint(value: AutonomousEvidenceExecutionCheckpointJSON): string {
  return canonicalJson(validateCheckpoint(value));
}

function assertCheckpointSuccessor(
  current: AutonomousEvidenceExecutionCheckpointJSON | null,
  expectedCheckpointDigest: string | null,
  candidate: AutonomousEvidenceExecutionCheckpointJSON,
): void {
  const expectedGeneration = current === null ? 1 : current.checkpoint_generation + 1;
  if ((current?.checkpoint_digest ?? null) !== expectedCheckpointDigest
      || candidate.previous_checkpoint_digest !== expectedCheckpointDigest
      || candidate.checkpoint_generation !== expectedGeneration) {
    throw new ArgumentError("evidence execution checkpoint does not extend the expected checkpoint lineage");
  }
}

export class InMemoryAutonomousEvidenceExecutionCheckpointStore implements AutonomousEvidenceExecutionCheckpointStore {
  private checkpoint: AutonomousEvidenceExecutionCheckpointJSON | null = null;

  read(): AutonomousEvidenceExecutionCheckpointJSON | null {
    return this.checkpoint === null ? null : clone(this.checkpoint);
  }

  write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): void {
    this.checkpoint = clone(validateCheckpoint(checkpoint));
  }

  writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): boolean {
    const current = this.checkpoint?.checkpoint_digest ?? null;
    if (current !== expectedCheckpointDigest) return false;
    const validated = validateCheckpoint(checkpoint);
    assertCheckpointSuccessor(this.checkpoint, expectedCheckpointDigest, validated);
    this.checkpoint = clone(validated);
    return true;
  }
}

export class JsonAutonomousEvidenceExecutionCheckpointStore implements AutonomousEvidenceExecutionCheckpointStore {
  constructor(protected readonly store: AutonomousEvidenceExecutionCheckpointTextStore) {
    if (!store || typeof store.read !== "function" || typeof store.write !== "function") throw new ArgumentError("evidence execution checkpoint JSON store is malformed");
  }

  async read(): Promise<AutonomousEvidenceExecutionCheckpointJSON | null> {
    const value = await this.store.read();
    if (value === null) return null;
    if (typeof value !== "string" || bytes(value) > MAX_AUTONOMOUS_EVIDENCE_EXECUTION_CHECKPOINT_BYTES) throw new ArgumentError("evidence execution checkpoint JSON exceeds its bound");
    let parsed: unknown;
    try { parsed = JSON.parse(value); } catch { throw new ArgumentError("evidence execution checkpoint JSON is invalid"); }
    if (canonicalJson(parsed) !== value) throw new ArgumentError("evidence execution checkpoint JSON is not canonical");
    return validateCheckpoint(parsed);
  }

  async write(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> {
    await this.store.write(encodeCheckpoint(checkpoint));
  }
}

export class TransactionalJsonAutonomousEvidenceExecutionCheckpointStore extends JsonAutonomousEvidenceExecutionCheckpointStore {
  private readonly transactionalStore: AutonomousEvidenceExecutionTransactionalCheckpointTextStore;

  constructor(store: AutonomousEvidenceExecutionTransactionalCheckpointTextStore) {
    super(store);
    this.transactionalStore = store;
  }

  async writeIfUnchanged(expectedCheckpointDigest: string | null, checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<boolean> {
    const validated = validateCheckpoint(checkpoint);
    const current = await this.read();
    if ((current?.checkpoint_digest ?? null) !== expectedCheckpointDigest) return false;
    assertCheckpointSuccessor(current, expectedCheckpointDigest, validated);
    const committed = await this.transactionalStore.writeIfUnchanged(expectedCheckpointDigest, encodeCheckpoint(validated));
    if (typeof committed !== "boolean") throw new ArgumentError("transactional evidence execution checkpoint text store returned a non-boolean result");
    return committed;
  }
}

export class WebStorageAutonomousEvidenceExecutionCheckpointTextStore implements AutonomousEvidenceExecutionCheckpointTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("evidence execution checkpoint web storage is malformed");
    if (!key || key.length > 256) throw new ArgumentError("evidence execution checkpoint web storage key is malformed");
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

function bindReconciliationReceipt(input: {
  receipt: unknown;
  checkpoint: AutonomousEvidenceExecutionCheckpointJSON;
  executionPlan: AutonomousEvidenceExecutionPlan;
  evidencePlan: AutonomousEvidencePlan;
  requests: readonly AutonomousEvidenceAcquisitionRequest[];
}): AutonomousEvidenceExecutionReconciliationReceiptJSON {
  const receipt = validateReconciliationReceipt(input.receipt);
  if (!["dispatch_pending", "reconciliation_required"].includes(input.checkpoint.status)) throw new ArgumentError("evidence execution reconciliation requires an uncertain-dispatch checkpoint");
  if (receipt.job_id !== input.checkpoint.job_id || receipt.job_id !== identifier("evidence execution reconciliation job_id", input.checkpoint.job_id)) throw new ArgumentError("evidence execution reconciliation receipt belongs to a different job");
  if (receipt.checkpoint_digest !== input.checkpoint.checkpoint_digest) throw new ArgumentError("evidence execution reconciliation receipt is stale or bound to a different checkpoint");
  if (receipt.evidence_plan_digest !== input.evidencePlan.plan_digest || receipt.evidence_plan_digest !== input.executionPlan.evidence_plan_digest) throw new ArgumentError("evidence execution reconciliation receipt belongs to a different evidence plan");
  if (receipt.execution_plan_digest !== input.executionPlan.plan_digest) throw new ArgumentError("evidence execution reconciliation receipt belongs to a different execution plan");
  if (input.checkpoint.reconciliation_authority_id === null || input.checkpoint.reconciliation_authority_version === null) throw new ArgumentError("evidence execution reconciliation is unavailable because its checkpoint has no configured authority");
  if (receipt.authority_id !== input.checkpoint.reconciliation_authority_id || receipt.authority_version !== input.checkpoint.reconciliation_authority_version) throw new ArgumentError("evidence execution reconciliation receipt authority does not match its checkpoint trust root");
  const requestSetDigest = requestsDigest(input.requests);
  if (receipt.request_set_digest !== requestSetDigest || input.checkpoint.request_digest !== requestSetDigest) throw new ArgumentError("evidence execution reconciliation receipt belongs to a different request set");
  const requestDigests = runtimeRequestDigests(input.evidencePlan, input.requests);
  if (receipt.outcomes.length !== requestDigests.length) throw new ArgumentError("evidence execution reconciliation receipt must classify every request exactly once");
  receipt.outcomes.forEach((outcome, index) => {
    if (outcome.request_index !== index || outcome.request_digest !== requestDigests[index]) throw new ArgumentError(`evidence execution reconciliation outcome ${index} is bound to a different request`);
  });
  return receipt;
}

function snapshotJournal(journal: AutonomousEvidenceRuntimeJournal, records: readonly AutonomousEvidenceRuntimeJournalEntry[]): AutonomousEvidenceRuntimeJournal {
  let snapshot = records.map((entry) => clone(entry));
  return {
    records: () => snapshot.map((entry) => clone(entry)),
    append: async (entry) => {
      const persisted = await journal.append(entry);
      snapshot = [...snapshot, clone(persisted)];
      return clone(persisted);
    },
  };
}

async function reconcileJournal(input: {
  receipt: AutonomousEvidenceExecutionReconciliationReceiptJSON;
  evidencePlan: AutonomousEvidencePlan;
  requests: readonly AutonomousEvidenceAcquisitionRequest[];
  journal: AutonomousEvidenceRuntimeJournal | undefined;
  rehydrateValue: AutonomousEvidenceExecutionOptions["rehydrateValue"];
}): Promise<{ journal: AutonomousEvidenceRuntimeJournal | undefined; rehydrateValue: AutonomousEvidenceExecutionOptions["rehydrateValue"] }> {
  const records = input.journal === undefined ? [] : await input.journal.records();
  if (!Array.isArray(records) || records.length > MAX_AUTONOMOUS_EVIDENCE_RUNTIME_RECEIPTS) throw new ArgumentError("evidence execution reconciliation journal is outside its bound");
  const journal = input.journal === undefined ? undefined : snapshotJournal(input.journal, records);
  if (journal !== undefined) await new AutonomousEvidenceRuntime({ plan: input.evidencePlan, journal }).rehydrate();
  const latestByRequest = new Map<string, AutonomousEvidenceRuntimeJournalEntry>();
  for (const entry of records) latestByRequest.set(entry.receipt.request_digest, entry);
  const normalized = normalizeRequests(input.requests);
  const rehydrated = new Map<string, JsonValue>();
  for (const outcome of input.receipt.outcomes) {
    const prior = latestByRequest.get(outcome.request_digest);
    if (outcome.outcome === "not_executed") {
      if (prior !== undefined) throw new ArgumentError(`evidence execution reconciliation request ${outcome.request_index} is recorded in the journal and cannot be classified not_executed`);
      continue;
    }
    if (outcome.outcome === "unknown") continue;
    if (input.journal === undefined || prior === undefined) throw new ArgumentError(`evidence execution reconciliation request ${outcome.request_index} has no journal-backed success`);
    const request = normalized[outcome.request_index]!;
    const receipt = prior.receipt;
    if (
      receipt.receipt_digest !== outcome.succeeded_receipt_digest
      || receipt.request_digest !== outcome.request_digest
      || receipt.plan_digest !== input.evidencePlan.plan_digest
      || receipt.requirement_id !== request.requirement_id
      || receipt.source_id !== request.source_id
      || (receipt.source_digest ?? null) !== request.source_digest
      || !["observed", "partial"].includes(receipt.status)
      || receipt.value_digest === null
    ) throw new ArgumentError(`evidence execution reconciliation request ${outcome.request_index} does not match a journal-backed source success`);
    if (input.rehydrateValue === undefined) throw new ArgumentError(`evidence execution reconciliation request ${outcome.request_index} requires caller-owned value rehydration`);
    const value = await input.rehydrateValue(receipt);
    if (value === null || digestJsonSync(value) !== receipt.value_digest) throw new ArgumentError(`evidence execution reconciliation request ${outcome.request_index} rehydration does not match its journal receipt digest`);
    rehydrated.set(receipt.receipt_digest, clone(value));
  }
  const rehydrateValue = input.rehydrateValue === undefined ? undefined : async (receipt: Parameters<NonNullable<AutonomousEvidenceExecutionOptions["rehydrateValue"]>>[0]) => {
    const value = rehydrated.get(receipt.receipt_digest);
    return value === undefined ? input.rehydrateValue!(receipt) : clone(value);
  };
  return { journal, rehydrateValue };
}

export class AutonomousEvidenceExecutionResumableController {
  private expectedCheckpointDigest: string | null = null;
  private restored = false;
  private mutation: Promise<void> = Promise.resolve();
  private checkpoint: AutonomousEvidenceExecutionCheckpointJSON | null = null;
  private readonly configuredReconciliationAuthority: { id: string; version: string; config_digest: string | null } | null;

  constructor(
    readonly controller: AutonomousEvidenceExecutionController,
    readonly persistence: AutonomousEvidenceExecutionCheckpointStore,
    readonly jobId: string,
    options: AutonomousEvidenceExecutionResumableControllerOptions = {},
  ) {
    if (!(controller instanceof AutonomousEvidenceExecutionController)) throw new ArgumentError("evidence execution resumable controller requires a typed execution controller");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("evidence execution resumable controller persistence is malformed");
    this.jobId = identifier("evidence execution resumable job_id", jobId);
    if (!isObject(options)) throw new ArgumentError("evidence execution resumable controller options are malformed");
    allowedKeys(options as Record<string, unknown>, ["reconciliationAuthority"], "evidence execution resumable controller options");
    this.configuredReconciliationAuthority = reconciliationAuthority((options as AutonomousEvidenceExecutionResumableControllerOptions).reconciliationAuthority);
  }

  async restore(): Promise<{ status: "empty" | "restored"; checkpoint_digest: string | null }> {
    return this.serial(() => this.restoreInternal());
  }

  async run(executionPlan: AutonomousEvidenceExecutionPlan, evidencePlan: AutonomousEvidencePlan, requests: readonly AutonomousEvidenceAcquisitionRequest[], options: AutonomousEvidenceExecutionResumableOptions = {}): Promise<AutonomousEvidenceExecutionResumableRun> {
    return this.serial(async () => {
      await this.restoreInternal();
      if (!(executionPlan instanceof AutonomousEvidenceExecutionPlan) || !(evidencePlan instanceof AutonomousEvidencePlan)) throw new ArgumentError("evidence execution resumable run requires typed plans");
      const requestDigest = requestsDigest(requests);
      const requiredRequirementIds = planRequirementIds(evidencePlan);
      const requiredRequirementCount = requiredRequirementIds.length;
      const policyDigest = executionPolicyDigest(options);
      const current = this.checkpoint;
      if (current !== null && (
        current.evidence_plan_digest !== executionPlan.evidence_plan_digest
        || current.execution_plan_digest !== executionPlan.plan_digest
        || current.request_digest !== requestDigest
        || current.readiness_report_digest !== executionPlan.readiness.report_digest
        || current.execution_policy_digest !== policyDigest
        || current.required_requirement_count !== requiredRequirementCount
      )) throw new ArgumentError("evidence execution checkpoint is bound to a different plan, request set, readiness report, requirement coverage, or execution policy");
      const nextCheckpoint = (
        status: AutonomousEvidenceExecutionCheckpointStatus,
        result: AutonomousEvidenceExecutionResult | null = null,
        reconciliationReceiptDigest: string | null = null,
      ): AutonomousEvidenceExecutionCheckpointJSON => checkpointFor({
        jobId: this.jobId,
        executionPlan,
        requestDigest,
        executionPolicyDigest: policyDigest,
        requiredRequirementIds,
        previousCheckpoint: this.checkpoint,
        reconciliationAuthority: this.configuredReconciliationAuthority,
        status,
        result,
        reconciliationReceiptDigest,
      });
      if (["completed", "awaiting_evaluation", "partial", "failed"].includes(current?.status ?? "") && options.journal === undefined) return this.resultFromCheckpoint(current!);
      if (options.approveSourceDispatch === true && executionPlan.status === "ready_for_review") {
        if (this.configuredReconciliationAuthority === null) throw new ArgumentError("evidence source dispatch requires a configured reconciliationAuthority trust root");
        if (typeof this.persistence.writeIfUnchanged !== "function") throw new ArgumentError("evidence source dispatch requires a transactional compare-and-swap checkpoint store");
        if (options.journal === undefined) throw new ArgumentError("evidence source dispatch requires a caller-owned runtime journal");
        if (options.executionPolicyIdentity?.value_rehydrator === undefined) throw new ArgumentError("evidence source dispatch requires an actual or reserved value_rehydrator identity");
      }
      const uncertain = current !== null && ["dispatch_pending", "reconciliation_required"].includes(current.status);
      let reconciliationReceipt: AutonomousEvidenceExecutionReconciliationReceiptJSON | null = null;
      let reconciledJournal = options.journal;
      let reconciledRehydrateValue = options.rehydrateValue;
      if (uncertain) {
        if (options.reconciliationReceipt === undefined) {
          if (current.status === "dispatch_pending") {
            const quarantined = nextCheckpoint("reconciliation_required", null, current.reconciliation_receipt_digest);
            await this.commit(quarantined);
            return this.resultFromCheckpoint(quarantined);
          }
          return this.resultFromCheckpoint(current);
        }
        reconciliationReceipt = bindReconciliationReceipt({ receipt: options.reconciliationReceipt, checkpoint: current, executionPlan, evidencePlan, requests });
        const reconciled = await reconcileJournal({ receipt: reconciliationReceipt, evidencePlan, requests, journal: options.journal, rehydrateValue: options.rehydrateValue });
        reconciledJournal = reconciled.journal;
        reconciledRehydrateValue = reconciled.rehydrateValue;
        if (reconciliationReceipt.outcomes.some((outcome) => outcome.outcome === "unknown")) {
          const quarantined = nextCheckpoint("reconciliation_required", null, reconciliationReceipt.receipt_digest);
          await this.commit(quarantined);
          return this.resultFromCheckpoint(quarantined);
        }
      } else if (options.reconciliationReceipt !== undefined) {
        throw new ArgumentError("evidence execution reconciliation receipt requires an uncertain-dispatch checkpoint");
      }
      if (uncertain && options.approveSourceDispatch !== true) return this.resultFromCheckpoint(current!);
      if (options.approveSourceDispatch !== true) {
        const gated = nextCheckpoint(executionPlan.status === "ready_for_review" ? "approval_required" : "blocked");
        await this.commit(gated);
        return this.resultFromCheckpoint(gated);
      }
      if (executionPlan.status !== "ready_for_review") {
        const blocked = nextCheckpoint("blocked");
        await this.commit(blocked);
        return this.resultFromCheckpoint(blocked);
      }
      const reconciliationReceiptDigest = reconciliationReceipt?.receipt_digest ?? null;
      const pending = nextCheckpoint("dispatch_pending", null, reconciliationReceiptDigest);
      await this.commit(pending);
      const { executionPolicyIdentity: _executionPolicyIdentity, reconciliationReceipt: _reconciliationReceipt, resumeAfterReconciliation: _resumeAfterReconciliation, ...baseExecuteOptions } = options;
      const executeOptions: AutonomousEvidenceExecutionOptions = {
        ...baseExecuteOptions,
        ...(reconciledJournal === undefined ? {} : { journal: reconciledJournal }),
        ...(reconciledRehydrateValue === undefined ? {} : { rehydrateValue: reconciledRehydrateValue }),
      };
      try {
        const result = await this.controller.execute(executionPlan, evidencePlan, requests, executeOptions);
        const settled = nextCheckpoint(statusForResult(result), result, reconciliationReceiptDigest);
        await this.commit(settled);
        return this.resultFromCheckpoint(settled, result, result.runtime.json.receipts.some((receipt) => receipt.replay === "replayed"));
      } catch (error) {
        const reconciliation = nextCheckpoint("reconciliation_required", null, reconciliationReceiptDigest);
        await this.commit(reconciliation);
        throw error;
      }
    });
  }

  private resultFromCheckpoint(checkpoint: AutonomousEvidenceExecutionCheckpointJSON, result: AutonomousEvidenceExecutionResult | null = null, replayed = false): AutonomousEvidenceExecutionResumableRun {
    const projection = {
      schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA,
      job_id: this.jobId,
      status: checkpoint.status,
      checkpoint_digest: checkpoint.checkpoint_digest,
      execution_plan_digest: checkpoint.execution_plan_digest,
      evidence_result_digest: result?.result_digest ?? null,
      replayed,
      retention: RESULT_RETENTION,
      secret_material: SECRET_MATERIAL,
    } satisfies AutonomousEvidenceExecutionResumableRunProjection;
    return {
      schema: AUTONOMOUS_EVIDENCE_EXECUTION_RESUMABLE_RESULT_SCHEMA,
      job_id: this.jobId,
      status: checkpoint.status,
      checkpoint: clone(checkpoint),
      result,
      replayed,
      toJSON: () => clone(projection),
    };
  }

  private async commit(checkpoint: AutonomousEvidenceExecutionCheckpointJSON): Promise<void> {
    const validated = validateCheckpoint(checkpoint);
    assertCheckpointSuccessor(this.checkpoint, this.expectedCheckpointDigest, validated);
    if (this.persistence.writeIfUnchanged !== undefined) {
      const committed = await this.persistence.writeIfUnchanged(this.expectedCheckpointDigest, validated);
      if (typeof committed !== "boolean") throw new ArgumentError("evidence execution checkpoint store returned a non-boolean compare-and-swap result");
      if (!committed) throw new ArgumentError("evidence execution checkpoint is stale; another worker committed after restore");
    } else {
      await this.persistence.write(validated);
    }
    this.checkpoint = clone(validated);
    this.expectedCheckpointDigest = validated.checkpoint_digest;
  }

  private async restoreInternal(): Promise<{ status: "empty" | "restored"; checkpoint_digest: string | null }> {
    if (this.restored) return { status: this.checkpoint === null ? "empty" : "restored", checkpoint_digest: this.expectedCheckpointDigest } as const;
    const restored = await this.persistence.read();
    this.checkpoint = restored === null ? null : validateCheckpoint(restored);
    if (this.checkpoint !== null && this.checkpoint.job_id !== this.jobId) throw new ArgumentError("evidence execution checkpoint belongs to a different job");
    if (this.checkpoint !== null && (
      this.checkpoint.reconciliation_authority_id !== (this.configuredReconciliationAuthority?.id ?? null)
      || this.checkpoint.reconciliation_authority_version !== (this.configuredReconciliationAuthority?.version ?? null)
      || this.checkpoint.reconciliation_authority_config_digest !== (this.configuredReconciliationAuthority?.config_digest ?? null)
    )) throw new ArgumentError("evidence execution checkpoint reconciliation authority does not match this controller trust root");
    this.expectedCheckpointDigest = this.checkpoint?.checkpoint_digest ?? null;
    this.restored = true;
    return { status: this.checkpoint === null ? "empty" : "restored", checkpoint_digest: this.expectedCheckpointDigest } as const;
  }

  private async serial<T>(operation: () => Promise<T>): Promise<T> {
    const next = this.mutation.then(operation, operation);
    this.mutation = next.then(() => undefined, () => undefined);
    return next;
  }
}
