import { ArgumentError, ProviderRuntimeError, isObject } from "./errors.js";
import type { ApiClient } from "./client.js";
import {
  AUTONOMOUS_DOMAIN_NAMES,
  builtinAutonomousDomainProfiles,
  type AutonomousAgent,
  type AutonomousCrossDomainRunResult,
  type AutonomousDomainName,
  type AutonomousRunResult,
} from "./autonomous.js";
import type { AutonomousWorkflowExecutionResult } from "./workflow-execution.js";
import type { AutonomousEpisodicMemoryStore } from "./autonomous-memory.js";
import type { AutonomousDomainResponseEvaluation } from "./autonomous-domain-response.js";
import {
  assertAutonomousEvaluatorCalibrationReady,
  validateAutonomousEvaluatorCalibrationReport,
  type AutonomousEvaluatorCalibrationReport,
} from "./autonomous-evaluator-calibration.js";
import { canonicalJson, digestJson, digestJsonSync } from "./tooling.js";
import type {
  BrainBanditState,
  BrainBanditContext,
  BrainEvaluatorAssessment,
  BrainLearningEvidence,
  BrainOutcomeRecordResult,
  BrainRunIdentity,
  JsonObject,
  RestToolResponse,
} from "./types.js";

export const AUTONOMOUS_EVALUATION_SCHEMA = "bioprism-typescript-autonomous-workflow-evaluation/0.1" as const;
export const AUTONOMOUS_LEARNING_EPISODE_SCHEMA = "bioprism-typescript-autonomous-learning-episode/0.1" as const;
export const AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA = "bioprism-typescript-autonomous-learning-trajectory/0.1" as const;
export const AUTONOMOUS_LEARNING_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-learning-snapshot/0.1" as const;
export const AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SCHEMA = "bioprism-typescript-autonomous-learning-settlement-receipt/0.1" as const;
export const AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-learning-settlement-receipt-snapshot/0.1" as const;
export const AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA = "bioprism-typescript-autonomous-learning-feedback-outbox/0.1" as const;
export const AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SNAPSHOT_SCHEMA = "bioprism-typescript-autonomous-learning-feedback-outbox-snapshot/0.1" as const;
export const AUTONOMOUS_EVALUATOR_MESH_SCHEMA = "bioprism-typescript-autonomous-evaluator-mesh/0.1" as const;
export const AUTONOMOUS_LEARNING_MAX_STAGES = 64;
export const AUTONOMOUS_LEARNING_MAX_TRAJECTORY_STEPS = 32;
export const AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX = 8_192;
export const AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX_SNAPSHOT_BYTES = 4_000_000;
export const AUTONOMOUS_LEARNING_MAX_SETTLEMENT_RECEIPTS = 8_192;
export const AUTONOMOUS_LEARNING_MAX_SETTLEMENT_RECEIPT_SNAPSHOT_BYTES = 4_000_000;

type Digest = string;

const PRIVATE_RETENTION = "value_only;task_prompt_response_credentials_and_evidence_not_retained" as const;

export interface AutonomousDomainEvaluatorProfile extends JsonObject {
  schema: "bioprism-typescript-autonomous-domain-evaluator/0.1";
  domain: AutonomousDomainName;
  evaluator_id: string;
  evaluator_version: string;
  required_signals: string[];
  signal_weights: Record<string, number>;
  pass_threshold: number;
  execution: "caller_declared_signal_scoring_only";
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousStageSignalEvidence extends JsonObject {
  stage_id: string;
  signals: Record<string, number>;
  evidence_digest?: Digest | null;
}

export interface AutonomousWorkflowEvaluationInput extends JsonObject {
  stages: AutonomousStageSignalEvidence[];
  evidence_digest?: Digest | null;
}

export interface AutonomousWorkflowEvaluation extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATION_SCHEMA;
  evaluator_id: string;
  evaluator_version: string;
  domain: AutonomousDomainName;
  task_digest: Digest;
  workflow_digest: Digest;
  /** Stable context identity used to isolate evaluator credit; absent only on legacy episodes. */
  context_digest?: Digest | null;
  learning_context?: BrainBanditContext;
  plan_digest: Digest;
  execution_status: AutonomousWorkflowExecutionResult["status"];
  stage_scores: Record<string, number>;
  signal_scores: Record<string, number>;
  missing_signals: string[];
  rejected_signals: string[];
  required_signals: string[];
  pass_threshold: number;
  reward: number;
  passed: boolean;
  status: "passed" | "failed" | "incomplete";
  evidence_digest: Digest;
  evaluation_digest: Digest;
  evaluator_authority: "caller_declared_signal_scoring_only";
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousLearningEpisode extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_EPISODE_SCHEMA;
  episode_id: string;
  run: BrainRunIdentity;
  domain: AutonomousDomainName;
  capability: string;
  workflow_id: string;
  stage_id: string | null;
  parent_job_id: string | null;
  /** Optional link to the value-only episodic-memory record for automatic evaluation annotation. */
  memory_episode_id?: string | null;
  workflow_digest: Digest;
  /** Digest of the accepted provider refinement that shaped this episode, if any. */
  plan_refinement_digest?: Digest | null;
  status: "pending" | "settled";
  settlement: AutonomousLearningSettlementMetadata | null;
  episode_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningSettlementMetadata extends JsonObject {
  evaluation_digest: Digest | null;
  reward: number;
  credited_reward: number;
  next_generation: number;
  settlement_digest: Digest;
  settled_at: number;
}

export interface AutonomousLearningEpisodeStore {
  load(episodeId: string): Promise<AutonomousLearningEpisode | null> | AutonomousLearningEpisode | null;
  save(episode: AutonomousLearningEpisode): Promise<void> | void;
  markSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): Promise<AutonomousLearningEpisode> | AutonomousLearningEpisode;
  pending(limit?: number): Promise<AutonomousLearningEpisode[]> | AutonomousLearningEpisode[];
}

export interface AutonomousLearningTrajectoryStep extends JsonObject {
  index: number;
  episode_id: string;
  arm_id: string;
  context_digest?: Digest | null;
  run_digest: Digest;
  raw_reward: number | null;
  credited_reward: number | null;
}

export interface AutonomousLearningTrajectory extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  trajectory_id: string;
  discount: number;
  steps: AutonomousLearningTrajectoryStep[];
  status: "pending" | "settled";
  trajectory_digest: Digest;
  settlement_digest: Digest | null;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningTrajectoryStore {
  load(trajectoryId: string): Promise<AutonomousLearningTrajectory | null> | AutonomousLearningTrajectory | null;
  save(trajectory: AutonomousLearningTrajectory): Promise<void> | void;
  markSettled(trajectoryId: string, settlementDigest: Digest): Promise<AutonomousLearningTrajectory> | AutonomousLearningTrajectory;
}

export interface AutonomousLearningStateSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_SNAPSHOT_SCHEMA;
  generation: number;
  episodes: AutonomousLearningEpisode[];
  trajectories: AutonomousLearningTrajectory[];
  snapshot_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

/** Adapter contract for SQLite, Postgres, IndexedDB, object storage, or another caller-owned store. */
export interface AutonomousLearningSnapshotPersistence {
  read(): Promise<AutonomousLearningStateSnapshot | null> | AutonomousLearningStateSnapshot | null;
  write(snapshot: AutonomousLearningStateSnapshot): Promise<void> | void;
}

/**
 * A replay receipt contains only value-level learning projections. It deliberately excludes
 * prompts, provider responses, credentials, tool arguments, and raw evaluator evidence. A
 * durable implementation should make `save` conditional on the idempotency key so concurrent
 * workers cannot publish contradictory settlements.
 */
export interface AutonomousLearningSettlementReceipt extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SCHEMA;
  operation: "single_run" | "trajectory";
  idempotency_key: string;
  target_id: string;
  target_digest: Digest;
  request_digest: Digest;
  settlement_digest: Digest;
  settlement: AutonomousLearningSettlement | AutonomousTrajectorySettlement;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

/** Adapter contract for a durable settlement journal (SQLite, Postgres, IndexedDB, etc.). */
export interface AutonomousLearningSettlementReceiptStore {
  load(idempotencyKey: string): Promise<AutonomousLearningSettlementReceipt | null> | AutonomousLearningSettlementReceipt | null;
  save(receipt: AutonomousLearningSettlementReceipt): Promise<void> | void;
}

export interface AutonomousLearningSettlementReceiptSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SNAPSHOT_SCHEMA;
  receipts: AutonomousLearningSettlementReceipt[];
  snapshot_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningSettlementReceiptSnapshotPersistence {
  read(): Promise<AutonomousLearningSettlementReceiptSnapshot | null> | AutonomousLearningSettlementReceiptSnapshot | null;
  write(snapshot: AutonomousLearningSettlementReceiptSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: Digest | null, snapshot: AutonomousLearningSettlementReceiptSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousLearningSettlementReceiptTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousLearningSettlementReceiptTransactionalTextStore extends AutonomousLearningSettlementReceiptTextStore {
  writeIfUnchanged(expectedSnapshotDigest: Digest | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousLearningSettlementReceiptSnapshotStore extends AutonomousLearningSettlementReceiptStore {
  snapshot(): AutonomousLearningSettlementReceiptSnapshot;
  restore(snapshot: AutonomousLearningSettlementReceiptSnapshot): void;
}

export type AutonomousLearningFeedbackOutboxPayload =
  | {
    operation: "single_run";
    episode_id: string;
    reward_input: AutonomousEvaluatorRewardInput;
    credited_reward: number;
  }
  | {
    operation: "trajectory";
    trajectory_id: string;
    rewards: Record<string, AutonomousEvaluatorRewardInput>;
  };

/**
 * A value-only command which lets an application coordinate evaluator feedback across process
 * restarts. The payload intentionally contains evaluator values and digests only; it never
 * contains a prompt, provider response, credential, tool argument, or raw evidence body.
 */
export interface AutonomousLearningFeedbackOutboxCommand extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA;
  command_id: string;
  operation: "single_run" | "trajectory";
  target_id: string;
  target_digest: Digest;
  request_digest: Digest;
  remote: boolean;
  payload: AutonomousLearningFeedbackOutboxPayload;
  status: "pending" | "leased" | "applied" | "failed";
  attempts: number;
  available_at: number;
  lease_owner: string | null;
  lease_until: number | null;
  last_error_class: string | null;
  result_digest: Digest | null;
  created_at: number;
  updated_at: number;
  command_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningFeedbackOutboxStore {
  load(commandId: string): Promise<AutonomousLearningFeedbackOutboxCommand | null> | AutonomousLearningFeedbackOutboxCommand | null;
  save(command: AutonomousLearningFeedbackOutboxCommand): Promise<void> | void;
  pending(limit?: number, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand[]> | AutonomousLearningFeedbackOutboxCommand[];
  claim(commandId: string, workerId: string, leaseMs: number, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand | null> | AutonomousLearningFeedbackOutboxCommand | null;
  markApplied(commandId: string, workerId: string, resultDigest: Digest, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand> | AutonomousLearningFeedbackOutboxCommand;
  markFailed(commandId: string, workerId: string, errorClass: string, retryable: boolean, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand> | AutonomousLearningFeedbackOutboxCommand;
}

export interface AutonomousLearningFeedbackOutboxSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SNAPSHOT_SCHEMA;
  commands: AutonomousLearningFeedbackOutboxCommand[];
  snapshot_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningFeedbackOutboxSnapshotPersistence {
  read(): Promise<AutonomousLearningFeedbackOutboxSnapshot | null> | AutonomousLearningFeedbackOutboxSnapshot | null;
  write(snapshot: AutonomousLearningFeedbackOutboxSnapshot): Promise<void> | void;
  writeIfUnchanged?(expectedSnapshotDigest: Digest | null, snapshot: AutonomousLearningFeedbackOutboxSnapshot): Promise<boolean> | boolean;
}

export interface AutonomousLearningFeedbackOutboxSnapshotStore extends AutonomousLearningFeedbackOutboxStore {
  snapshot(): AutonomousLearningFeedbackOutboxSnapshot;
  restore(snapshot: AutonomousLearningFeedbackOutboxSnapshot): void;
}

export interface AutonomousLearningFeedbackOutboxTextStore {
  read(): Promise<string | null> | string | null;
  write(value: string): Promise<void> | void;
}

export interface AutonomousLearningFeedbackOutboxTransactionalTextStore extends AutonomousLearningFeedbackOutboxTextStore {
  writeIfUnchanged(expectedSnapshotDigest: Digest | null, value: string): Promise<boolean> | boolean;
}

export interface AutonomousLearningFeedbackOutboxDispatchRow extends JsonObject {
  command_id: string;
  operation: "single_run" | "trajectory";
  status: "applied" | "failed" | "leased_elsewhere";
  attempts: number;
  result_digest: Digest | null;
  error_class: string | null;
}

export interface AutonomousLearningFeedbackOutboxDispatch extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA;
  worker_id: string;
  inspected: number;
  applied: number;
  failed: number;
  leased_elsewhere: number;
  rows: AutonomousLearningFeedbackOutboxDispatchRow[];
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

/** Opt-in durable settlement boundary for higher-level workflow/cycle adapters. */
export interface AutonomousLearningOutboxSettlementOptions extends JsonObject {
  workerId?: string;
  leaseMs?: number;
}

export interface AutonomousLearningStateStore {
  loadEpisode(episodeId: string): Promise<AutonomousLearningEpisode | null> | AutonomousLearningEpisode | null;
  saveEpisode(episode: AutonomousLearningEpisode): Promise<void> | void;
  markEpisodeSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): Promise<AutonomousLearningEpisode> | AutonomousLearningEpisode;
  pendingEpisodes(limit?: number): Promise<AutonomousLearningEpisode[]> | AutonomousLearningEpisode[];
  loadTrajectory(trajectoryId: string): Promise<AutonomousLearningTrajectory | null> | AutonomousLearningTrajectory | null;
  saveTrajectory(trajectory: AutonomousLearningTrajectory): Promise<void> | void;
  markTrajectorySettled(trajectoryId: string, settlementDigest: Digest): Promise<AutonomousLearningTrajectory> | AutonomousLearningTrajectory;
  snapshot(): Promise<AutonomousLearningStateSnapshot>;
  restore(snapshot: AutonomousLearningStateSnapshot): Promise<void>;
}

export interface AutonomousEvaluatorRewardInput extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number;
  passed: boolean;
  failed?: boolean;
  feedback_digest?: Digest | null;
  failure_class?: string | null;
  evidence_digest?: Digest | null;
}

export interface AutonomousEvaluatorMeshMember {
  evaluator_id: string;
  evaluator_version: string;
  evaluate: (result: AutonomousRunResult) => AutonomousEvaluatorRewardInput | Promise<AutonomousEvaluatorRewardInput>;
}

export interface AutonomousEvaluatorMeshMemberProjection extends JsonObject {
  evaluator_id: string;
  evaluator_version: string;
  reward: number | null;
  passed: boolean | null;
  failed: boolean | null;
  feedback_digest: Digest | null;
  evidence_digest: Digest | null;
  failure_class: string | null;
}

export interface AutonomousEvaluatorMeshResult extends JsonObject {
  schema: typeof AUTONOMOUS_EVALUATOR_MESH_SCHEMA;
  status: "accepted" | "disagreement" | "member_error";
  evaluator_id: string;
  evaluator_version: string;
  reward: number | null;
  passed: boolean | null;
  failed: boolean;
  feedback_digest: Digest | null;
  evidence_digest: Digest | null;
  failure_class: string | null;
  reward_spread: number | null;
  max_reward_spread: number;
  member_results: AutonomousEvaluatorMeshMemberProjection[];
  mesh_digest: Digest;
  retention: typeof PRIVATE_RETENTION;
  secret_material: "never_returned";
}

export interface AutonomousLearningSettlement extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_EPISODE_SCHEMA;
  episode: AutonomousLearningEpisode;
  assessment: BrainEvaluatorAssessment;
  next_state: BrainBanditState;
  learning_evidence: BrainLearningEvidence | null;
  memory_evaluation?: AutonomousLearningMemoryEvaluationProjection;
  remote: boolean;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousLearningMemoryEvaluationProjection extends JsonObject {
  status: "recorded" | "not_configured" | "not_linked" | "failed";
  memory_episode_id: string | null;
  evaluation_digest: Digest | null;
  error_class: string | null;
}

export interface AutonomousTrajectorySettlement extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  trajectory: AutonomousLearningTrajectory;
  settlements: AutonomousLearningSettlement[];
  return_to_go: Record<string, number>;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousWorkflowLearningSettlement extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  evaluation: AutonomousWorkflowEvaluation;
  trajectory: AutonomousTrajectorySettlement;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousCrossDomainLearningSettlement {
  schema: typeof AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA;
  result: AutonomousCrossDomainRunResult;
  trajectory: AutonomousTrajectorySettlement;
  retention: typeof PRIVATE_RETENTION;
}

export interface AutonomousLearningControllerOptions {
  store?: AutonomousLearningStateStore;
  episodes?: AutonomousLearningEpisodeStore;
  trajectories?: AutonomousLearningTrajectoryStore;
  settlementReceipts?: AutonomousLearningSettlementReceiptStore;
  feedbackOutbox?: AutonomousLearningFeedbackOutboxStore;
  evaluator?: AutonomousWorkflowEvaluator;
  apiClient?: ApiClient;
  memoryStore?: AutonomousEpisodicMemoryStore;
  /** Optional metadata-only evaluator calibration report used by the learning admission gate. */
  calibrationReport?: AutonomousEvaluatorCalibrationReport;
  /** When true, every settlement refuses before bandit mutation unless its domain is admitted. */
  requireCalibratedLearning?: boolean;
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || !/^[A-Za-z0-9_.:-]+$/.test(value)) throw new ArgumentError(`${name} must be a bounded identifier`);
  return value;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) throw new ArgumentError(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedReward(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) throw new ArgumentError(`${name} must be within [0, 1]`);
  return value;
}

function boundedThreshold(name: string, value: unknown): number {
  const threshold = boundedReward(name, value);
  if (threshold <= 0 || threshold > 1) throw new ArgumentError(`${name} must be within (0, 1]`);
  return threshold;
}

function boundedGeneration(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) throw new ProviderRuntimeError("brain learning response returned an invalid generation");
  return value as number;
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

const SETTLEMENT_RECEIPT_FORBIDDEN_KEYS = new Set([
  "prompt",
  "response",
  "credentials",
  "credential",
  "api_key",
  "access_token",
  "refresh_token",
  "secret",
  "tool_arguments",
  "raw_response",
  "result",
]);

function assertValueOnlySettlement(value: unknown, depth = 0): void {
  if (depth > 12) throw new ArgumentError("learning settlement receipt is too deeply nested");
  if (Array.isArray(value)) {
    if (value.length > 4096) throw new ArgumentError("learning settlement receipt contains too many values");
    for (const item of value) assertValueOnlySettlement(item, depth + 1);
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (SETTLEMENT_RECEIPT_FORBIDDEN_KEYS.has(key.toLowerCase())) throw new ArgumentError(`learning settlement receipt cannot retain ${key}`);
    assertValueOnlySettlement(child, depth + 1);
  }
}

function assertSettlementReceiptShape(value: unknown): asserts value is AutonomousLearningSettlementReceipt {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SCHEMA || !isObject(value.settlement)) throw new ArgumentError("learning settlement receipt is malformed");
  assertExactKeys(value, ["schema", "operation", "idempotency_key", "target_id", "target_digest", "request_digest", "settlement", "settlement_digest", "retention", "secret_material"], "learning settlement receipt");
  if (value.operation !== "single_run" && value.operation !== "trajectory") throw new ArgumentError("learning settlement receipt operation is malformed");
  boundedIdentifier("settlement receipt idempotency_key", value.idempotency_key);
  boundedIdentifier("settlement receipt target_id", value.target_id);
  boundedDigest("settlement receipt target_digest", value.target_digest);
  boundedDigest("settlement receipt request_digest", value.request_digest);
  boundedDigest("settlement receipt settlement_digest", value.settlement_digest);
  if (value.retention !== PRIVATE_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("learning settlement receipt retention contract is malformed");
  if (value.operation === "single_run" && !isObject(value.settlement.episode)) throw new ArgumentError("single-run settlement receipt is missing its episode projection");
  if (value.operation === "trajectory" && (!isObject(value.settlement.trajectory) || !Array.isArray(value.settlement.settlements))) throw new ArgumentError("trajectory settlement receipt is missing its trajectory projection");
  assertValueOnlySettlement(value.settlement);
}

function assertSettlementReceiptSnapshotShape(value: unknown): asserts value is AutonomousLearningSettlementReceiptSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SNAPSHOT_SCHEMA || !Array.isArray(value.receipts)) throw new ArgumentError("learning settlement receipt snapshot is malformed");
  assertExactKeys(value, ["schema", "receipts", "snapshot_digest", "retention", "secret_material"], "learning settlement receipt snapshot");
  if (value.receipts.length > AUTONOMOUS_LEARNING_MAX_SETTLEMENT_RECEIPTS) throw new ArgumentError("learning settlement receipt snapshot exceeds its bound");
  if (value.retention !== PRIVATE_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("learning settlement receipt snapshot retention contract is malformed");
  boundedDigest("learning settlement receipt snapshot_digest", value.snapshot_digest);
  const { snapshot_digest: observed, ...descriptor } = value;
  if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("learning settlement receipt snapshot digest does not match");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > AUTONOMOUS_LEARNING_MAX_SETTLEMENT_RECEIPT_SNAPSHOT_BYTES) throw new ArgumentError("learning settlement receipt snapshot exceeds its byte bound");
  const ids = new Set<string>();
  for (const receipt of value.receipts) {
    assertSettlementReceiptShape(receipt);
    if (ids.has(receipt.idempotency_key)) throw new ArgumentError(`learning settlement receipt snapshot contains duplicate key ${receipt.idempotency_key}`);
    ids.add(receipt.idempotency_key);
  }
}

/** Validate a receipt restart image without mutating a caller-owned receipt store. */
export function validateAutonomousLearningSettlementReceiptSnapshot(raw: unknown): AutonomousLearningSettlementReceiptSnapshot {
  assertSettlementReceiptSnapshotShape(raw);
  return clone(raw);
}

function assertStageEvidence(value: unknown): asserts value is AutonomousStageSignalEvidence {
  if (!isObject(value) || typeof value.stage_id !== "string" || !value.stage_id.trim() || !isObject(value.signals)) throw new ArgumentError("stage evaluator evidence must contain stage_id and signals");
  boundedIdentifier("stage evaluator stage_id", value.stage_id);
  const signalEntries = Object.entries(value.signals);
  if (signalEntries.length > 64) throw new ArgumentError("stage evaluator evidence contains too many signals");
  for (const [signal, score] of signalEntries) {
    boundedIdentifier("stage evaluator signal", signal);
    boundedReward(`stage evaluator signal ${signal}`, score);
  }
  if (value.evidence_digest !== undefined) boundedDigest("stage evaluator evidence_digest", value.evidence_digest, true);
}

function assertRewardInput(value: AutonomousEvaluatorRewardInput): void {
  if (!isObject(value)) throw new ArgumentError("evaluator reward must be an object");
  boundedIdentifier("evaluator_id", value.evaluator_id);
  boundedIdentifier("evaluator_version", value.evaluator_version);
  boundedReward("evaluator reward", value.reward);
  if (typeof value.passed !== "boolean") throw new ArgumentError("evaluator passed must be boolean");
  if (value.failed !== undefined && typeof value.failed !== "boolean") throw new ArgumentError("evaluator failed must be boolean");
  if (value.feedback_digest !== undefined) boundedDigest("evaluator feedback_digest", value.feedback_digest, true);
  if (value.evidence_digest !== undefined) boundedDigest("evaluator evidence_digest", value.evidence_digest, true);
  if (value.failure_class !== undefined && value.failure_class !== null) boundedIdentifier("evaluator failure_class", value.failure_class);
}

function normalizeRewardInput(value: AutonomousEvaluatorRewardInput): AutonomousEvaluatorRewardInput {
  assertRewardInput(value);
  return {
    evaluator_id: value.evaluator_id,
    evaluator_version: value.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed ?? !value.passed,
    feedback_digest: value.feedback_digest ?? null,
    failure_class: value.failure_class ?? null,
    evidence_digest: value.evidence_digest ?? null,
  };
}

function boundedOutboxTimestamp(name: string, value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw new ArgumentError(`${name} must be a non-negative safe integer timestamp`);
  return value;
}

function boundedOutboxAttempts(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0 || value > 1_000_000) throw new ArgumentError("feedback outbox attempts are outside their bounds");
  return value;
}

function assertExactKeys(value: object, allowed: readonly string[], name: string): void {
  const permitted = new Set(allowed);
  const unexpected = Object.keys(value).filter((key) => !permitted.has(key));
  if (unexpected.length) throw new ArgumentError(`${name} contains unsupported fields`);
}

function assertRewardInputKeys(value: unknown, name: string): void {
  if (!isObject(value)) throw new ArgumentError(`${name} is malformed`);
  assertExactKeys(value, ["evaluator_id", "evaluator_version", "reward", "passed", "failed", "feedback_digest", "failure_class", "evidence_digest"], name);
}

function assertFeedbackOutboxPayload(value: unknown): asserts value is AutonomousLearningFeedbackOutboxPayload {
  if (!isObject(value) || (value.operation !== "single_run" && value.operation !== "trajectory")) throw new ArgumentError("feedback outbox payload is malformed");
  if (value.operation === "single_run") {
    assertExactKeys(value, ["operation", "episode_id", "reward_input", "credited_reward"], "feedback outbox single-run payload");
    boundedIdentifier("feedback outbox episode_id", value.episode_id);
    assertRewardInputKeys(value.reward_input, "feedback outbox reward_input");
    normalizeRewardInput(value.reward_input as AutonomousEvaluatorRewardInput);
    boundedReward("feedback outbox credited_reward", value.credited_reward);
  } else {
    assertExactKeys(value, ["operation", "trajectory_id", "rewards"], "feedback outbox trajectory payload");
    boundedIdentifier("feedback outbox trajectory_id", value.trajectory_id);
    if (!isObject(value.rewards)) throw new ArgumentError("feedback outbox trajectory rewards are malformed");
    const rewardIds = Object.keys(value.rewards);
    if (rewardIds.length < 1 || rewardIds.length > AUTONOMOUS_LEARNING_MAX_TRAJECTORY_STEPS) throw new ArgumentError("feedback outbox trajectory rewards are outside their bounds");
    for (const episodeId of rewardIds) {
      boundedIdentifier("feedback outbox reward episode_id", episodeId);
      assertRewardInputKeys(value.rewards[episodeId], `feedback outbox reward ${episodeId}`);
      normalizeRewardInput(value.rewards[episodeId] as AutonomousEvaluatorRewardInput);
    }
  }
  assertValueOnlySettlement(value);
}

function assertFeedbackOutboxCommandShape(value: unknown): asserts value is AutonomousLearningFeedbackOutboxCommand {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA || !isObject(value.payload)) throw new ArgumentError("feedback outbox command is malformed");
  assertExactKeys(value, ["schema", "command_id", "operation", "target_id", "target_digest", "request_digest", "remote", "payload", "status", "attempts", "available_at", "lease_owner", "lease_until", "last_error_class", "result_digest", "created_at", "updated_at", "command_digest", "retention", "secret_material"], "feedback outbox command");
  boundedIdentifier("feedback outbox command_id", value.command_id);
  if (value.operation !== "single_run" && value.operation !== "trajectory") throw new ArgumentError("feedback outbox operation is malformed");
  boundedIdentifier("feedback outbox target_id", value.target_id);
  boundedDigest("feedback outbox target_digest", value.target_digest);
  boundedDigest("feedback outbox request_digest", value.request_digest);
  if (typeof value.remote !== "boolean") throw new ArgumentError("feedback outbox remote flag is malformed");
  assertFeedbackOutboxPayload(value.payload);
  if (value.payload.operation !== value.operation) throw new ArgumentError("feedback outbox payload operation does not match command");
  boundedOutboxAttempts(value.attempts);
  boundedOutboxTimestamp("feedback outbox available_at", value.available_at);
  if (value.lease_owner !== null) boundedIdentifier("feedback outbox lease_owner", value.lease_owner);
  if (value.lease_until !== null) boundedOutboxTimestamp("feedback outbox lease_until", value.lease_until);
  if (value.status !== "pending" && value.status !== "leased" && value.status !== "applied" && value.status !== "failed") throw new ArgumentError("feedback outbox status is malformed");
  if (value.status === "leased" && (value.lease_owner === null || value.lease_until === null)) throw new ArgumentError("leased feedback outbox command must have an active lease");
  if (value.status !== "leased" && value.lease_owner !== null) throw new ArgumentError("non-leased feedback outbox command cannot retain a lease owner");
  boundedDigest("feedback outbox result_digest", value.result_digest, true);
  if (value.last_error_class !== null) boundedIdentifier("feedback outbox error class", value.last_error_class);
  const createdAt = boundedOutboxTimestamp("feedback outbox created_at", value.created_at);
  const updatedAt = boundedOutboxTimestamp("feedback outbox updated_at", value.updated_at);
  if (updatedAt < createdAt) throw new ArgumentError("feedback outbox updated_at cannot precede created_at");
  if (value.retention !== PRIVATE_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("feedback outbox retention contract is malformed");
  const { command_digest: observed, ...descriptor } = value;
  boundedDigest("feedback outbox command_digest", observed);
  if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("feedback outbox command digest does not match");
}

function assertFeedbackOutboxSnapshotShape(value: unknown): asserts value is AutonomousLearningFeedbackOutboxSnapshot {
  if (!isObject(value) || value.schema !== AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SNAPSHOT_SCHEMA || !Array.isArray(value.commands)) throw new ArgumentError("feedback outbox snapshot is malformed");
  assertExactKeys(value, ["schema", "commands", "snapshot_digest", "retention", "secret_material"], "feedback outbox snapshot");
  if (value.commands.length > AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX) throw new ArgumentError("feedback outbox snapshot exceeds its command bound");
  if (value.retention !== PRIVATE_RETENTION || value.secret_material !== "never_returned") throw new ArgumentError("feedback outbox snapshot retention contract is malformed");
  boundedDigest("feedback outbox snapshot_digest", value.snapshot_digest);
  const { snapshot_digest: observed, ...descriptor } = value;
  if (digestJsonSync(descriptor) !== observed) throw new ArgumentError("feedback outbox snapshot digest does not match");
  if (new TextEncoder().encode(canonicalJson(value)).byteLength > AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX_SNAPSHOT_BYTES) throw new ArgumentError("feedback outbox snapshot exceeds its byte bound");
  const ids = new Set<string>();
  for (const command of value.commands) {
    assertFeedbackOutboxCommandShape(command);
    if (ids.has(command.command_id)) throw new ArgumentError(`feedback outbox snapshot contains duplicate command ${command.command_id}`);
    ids.add(command.command_id);
  }
}

/** Validate a metadata-only feedback queue restart image without mutating a live store. */
export function validateAutonomousLearningFeedbackOutboxSnapshot(raw: unknown): AutonomousLearningFeedbackOutboxSnapshot {
  assertFeedbackOutboxSnapshotShape(raw);
  return clone(raw);
}

function refreshFeedbackOutboxCommand(command: AutonomousLearningFeedbackOutboxCommand): AutonomousLearningFeedbackOutboxCommand {
  const { command_digest: _ignored, ...descriptor } = command;
  return { ...descriptor, command_digest: digestJsonSync(descriptor) };
}

function feedbackOutboxErrorClass(error: unknown): string {
  return error instanceof Error && error.constructor.name.trim() ? error.constructor.name : "FeedbackSettlementError";
}

function feedbackOutboxRetryable(error: unknown): boolean {
  return !(error instanceof ArgumentError);
}

function requiredSignalsFor(execution: AutonomousWorkflowExecutionResult): string[] {
  const blueprint = execution.blueprint;
  if (!blueprint) throw new ArgumentError("workflow evaluation requires an executed blueprint");
  return [...new Set(blueprint.workflow.stages.flatMap((stage) => stage.evaluator_signals.map((signal) => `${stage.id}/${signal}`)))].sort();
}

/** Return one reviewed, caller-declared evaluator profile for every built-in domain. */
export async function builtinAutonomousDomainEvaluatorProfiles(): Promise<AutonomousDomainEvaluatorProfile[]> {
  const profiles = await builtinAutonomousDomainProfiles();
  return profiles.map((profile) => {
    const requiredSignals = [...new Set(profile.workflow.stages.flatMap((stage) => stage.evaluator_signals))].sort();
    const signalWeights = Object.fromEntries(requiredSignals.map((signal) => [signal, 1]));
    return {
      schema: "bioprism-typescript-autonomous-domain-evaluator/0.1",
      domain: profile.domain,
      evaluator_id: `typescript-${profile.domain}-workflow-evaluator`,
      evaluator_version: "0.1",
      required_signals: requiredSignals,
      signal_weights: signalWeights,
      pass_threshold: 0.8,
      execution: "caller_declared_signal_scoring_only",
      retention: PRIVATE_RETENTION,
    };
  });
}

/**
 * Construct the reviewed evaluator contract for one built-in domain.
 *
 * This is intentionally an async factory because the domain workflow catalogue is
 * content-addressed. Callers can therefore select a domain evaluator without copying
 * signal names or weights out of the catalogue, while the returned evaluator remains
 * caller-owned and scores only explicit evidence packets.
 */
export async function autonomousWorkflowEvaluatorForDomain(
  domain: AutonomousDomainName,
  options: { evaluatorVersion?: string; passThreshold?: number; signalWeights?: Readonly<Record<string, number>> } = {},
): Promise<AutonomousWorkflowEvaluator> {
  if (!AUTONOMOUS_DOMAIN_NAMES.includes(domain)) throw new ArgumentError(`unsupported autonomous evaluator domain: ${domain}`);
  const profile = (await builtinAutonomousDomainEvaluatorProfiles()).find((candidate) => candidate.domain === domain);
  if (!profile) throw new ArgumentError(`no built-in evaluator profile exists for domain: ${domain}`);
  return new AutonomousWorkflowEvaluator({
    evaluatorId: profile.evaluator_id,
    evaluatorVersion: options.evaluatorVersion ?? profile.evaluator_version,
    passThreshold: options.passThreshold ?? profile.pass_threshold,
    signalWeights: options.signalWeights ?? profile.signal_weights,
  });
}

function meshMemberProjection(member: AutonomousEvaluatorMeshMember, value: AutonomousEvaluatorRewardInput): AutonomousEvaluatorMeshMemberProjection {
  assertRewardInput(value);
  return {
    evaluator_id: member.evaluator_id,
    evaluator_version: member.evaluator_version,
    reward: value.reward,
    passed: value.passed,
    failed: value.failed ?? !value.passed,
    feedback_digest: value.feedback_digest ?? null,
    evidence_digest: value.evidence_digest ?? null,
    failure_class: value.failure_class ?? null,
  };
}

/**
 * Optional independent evaluator quorum for high-impact or ambiguous work. Disagreement is
 * retained and refuses learning credit; it is never averaged into a plausible-looking reward.
 */
export class AutonomousEvaluatorMesh {
  readonly members: readonly AutonomousEvaluatorMeshMember[];
  readonly evaluatorId: string;
  readonly evaluatorVersion: string;
  readonly maxRewardSpread: number;

  constructor(options: { members: readonly AutonomousEvaluatorMeshMember[]; evaluatorId?: string; evaluatorVersion?: string; maxRewardSpread?: number }) {
    if (!isObject(options) || !Array.isArray(options.members) || options.members.length < 2 || options.members.length > 8) throw new ArgumentError("evaluator mesh requires between 2 and 8 independent members");
    this.evaluatorId = boundedIdentifier("evaluator mesh evaluatorId", options.evaluatorId ?? "typescript-evaluator-mesh");
    this.evaluatorVersion = boundedIdentifier("evaluator mesh evaluatorVersion", options.evaluatorVersion ?? "0.1");
    this.maxRewardSpread = boundedReward("evaluator mesh maxRewardSpread", options.maxRewardSpread ?? 0.1);
    const seen = new Set<string>();
    this.members = options.members.map((member) => {
      if (!isObject(member) || typeof member.evaluate !== "function") throw new ArgumentError("evaluator mesh member must provide an evaluate function");
      const evaluatorId = boundedIdentifier("evaluator mesh member evaluator_id", member.evaluator_id);
      const evaluatorVersion = boundedIdentifier("evaluator mesh member evaluator_version", member.evaluator_version);
      if (seen.has(evaluatorId)) throw new ArgumentError(`evaluator mesh member ${evaluatorId} is duplicated`);
      seen.add(evaluatorId);
      return { evaluator_id: evaluatorId, evaluator_version: evaluatorVersion, evaluate: member.evaluate as AutonomousEvaluatorMeshMember["evaluate"] };
    });
  }

  async evaluateDetailed(result: AutonomousRunResult): Promise<AutonomousEvaluatorMeshResult> {
    if (!isObject(result)) throw new ArgumentError("evaluator mesh requires an autonomous run result");
    const outcomes = await Promise.allSettled(this.members.map((member) => member.evaluate(result)));
    const memberResults: AutonomousEvaluatorMeshMemberProjection[] = outcomes.map((outcome, index) => {
      const member = this.members[index]!;
      if (outcome.status === "rejected") return { evaluator_id: member.evaluator_id, evaluator_version: member.evaluator_version, reward: null, passed: null, failed: true, feedback_digest: null, evidence_digest: null, failure_class: "evaluator_member_error" };
      try {
        return meshMemberProjection(member, outcome.value);
      } catch {
        return { evaluator_id: member.evaluator_id, evaluator_version: member.evaluator_version, reward: null, passed: null, failed: true, feedback_digest: null, evidence_digest: null, failure_class: "evaluator_member_invalid" };
      }
    });
    const memberError = memberResults.some((member) => member.failure_class === "evaluator_member_error" || member.failure_class === "evaluator_member_invalid");
    let status: AutonomousEvaluatorMeshResult["status"] = "accepted";
    let reward: number | null = null;
    let passed: boolean | null = null;
    let failed = false;
    let failureClass: string | null = null;
    let rewardSpread: number | null = null;
    let feedbackDigest: Digest | null = null;
    let evidenceDigest: Digest | null = null;
    if (memberError) {
      status = "member_error";
      failed = true;
      failureClass = "evaluator_mesh_member_error";
    } else {
      const rewards = memberResults.map((member) => member.reward!);
      rewardSpread = Number((Math.max(...rewards) - Math.min(...rewards)).toFixed(12));
      const first = memberResults[0]!;
      const agreement = memberResults.every((member) => member.passed === first.passed && member.failed === first.failed && member.failure_class === first.failure_class) && rewardSpread <= this.maxRewardSpread;
      status = agreement ? "accepted" : "disagreement";
      if (agreement) {
        reward = Number((rewards.reduce((sum, value) => sum + value, 0) / rewards.length).toFixed(12));
        passed = first.passed;
        failed = first.failed ?? !first.passed;
        failureClass = first.failure_class;
      } else {
        failed = true;
        failureClass = "evaluator_disagreement";
      }
      feedbackDigest = await digestJson(memberResults.map((member) => ({ evaluator_id: member.evaluator_id, evaluator_version: member.evaluator_version, reward: member.reward, passed: member.passed, failed: member.failed, feedback_digest: member.feedback_digest, failure_class: member.failure_class })));
      evidenceDigest = await digestJson(memberResults.map((member) => member.evidence_digest).sort());
    }
    const descriptor = {
      schema: AUTONOMOUS_EVALUATOR_MESH_SCHEMA,
      status,
      evaluator_id: this.evaluatorId,
      evaluator_version: this.evaluatorVersion,
      reward,
      passed,
      failed,
      feedback_digest: feedbackDigest,
      evidence_digest: evidenceDigest,
      failure_class: failureClass,
      reward_spread: rewardSpread,
      max_reward_spread: this.maxRewardSpread,
      member_results: memberResults,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    return { ...descriptor, mesh_digest: await digestJson(descriptor) };
  }

  async evaluate(result: AutonomousRunResult): Promise<AutonomousEvaluatorRewardInput> {
    const mesh = await this.evaluateDetailed(result);
    if (mesh.status !== "accepted" || mesh.reward === null || mesh.passed === null) throw new ArgumentError(`evaluator mesh refused learning credit: ${mesh.failure_class ?? mesh.status}`);
    return {
      evaluator_id: mesh.evaluator_id,
      evaluator_version: mesh.evaluator_version,
      reward: mesh.reward,
      passed: mesh.passed,
      failed: mesh.failed,
      feedback_digest: mesh.feedback_digest,
      evidence_digest: mesh.evidence_digest,
      failure_class: mesh.failure_class,
    };
  }
}

/** Score only explicit evaluator signals; provider completion alone never creates reward. */
export class AutonomousWorkflowEvaluator {
  readonly evaluatorVersion: string;
  readonly passThreshold: number;
  readonly signalWeights: Readonly<Record<string, number>>;
  readonly evaluatorId?: string;

  constructor(options: { evaluatorId?: string; evaluatorVersion?: string; passThreshold?: number; signalWeights?: Readonly<Record<string, number>> } = {}) {
    this.evaluatorId = options.evaluatorId === undefined ? undefined : boundedIdentifier("evaluatorId", options.evaluatorId);
    this.evaluatorVersion = boundedIdentifier("evaluatorVersion", options.evaluatorVersion ?? "0.1");
    this.passThreshold = boundedThreshold("passThreshold", options.passThreshold ?? 0.8);
    this.signalWeights = { ...(options.signalWeights ?? {}) };
    for (const [signal, weight] of Object.entries(this.signalWeights)) {
      boundedIdentifier("evaluator signal", signal);
      if (typeof weight !== "number" || !Number.isFinite(weight) || weight <= 0) throw new ArgumentError(`evaluator signal weight ${signal} must be positive`);
    }
  }

  async evaluate(execution: AutonomousWorkflowExecutionResult, input: AutonomousWorkflowEvaluationInput): Promise<AutonomousWorkflowEvaluation> {
    if (!isObject(input) || !Array.isArray(input.stages)) throw new ArgumentError("workflow evaluator input must contain stages");
    if (input.stages.length > AUTONOMOUS_LEARNING_MAX_STAGES) throw new ArgumentError("workflow evaluator input contains too many stages");
    for (const evidence of input.stages) assertStageEvidence(evidence);
    if (input.evidence_digest !== undefined) boundedDigest("workflow evaluator evidence_digest", input.evidence_digest, true);
    const blueprint = execution.blueprint;
    if (!blueprint) throw new ArgumentError("workflow evaluation requires a blueprint");
    if (!AUTONOMOUS_DOMAIN_NAMES.includes(blueprint.domain_profile.domain)) throw new ArgumentError("workflow evaluation blueprint has an unsupported domain");
    const stageById = new Map(blueprint.workflow.stages.map((stage) => [stage.id, stage]));
    const evidenceById = new Map<string, AutonomousStageSignalEvidence>();
    for (const evidence of input.stages) {
      if (!stageById.has(evidence.stage_id)) throw new ArgumentError(`workflow evaluator received an unknown stage ${evidence.stage_id}`);
      if (evidenceById.has(evidence.stage_id)) throw new ArgumentError(`workflow evaluator received duplicate stage ${evidence.stage_id}`);
      evidenceById.set(evidence.stage_id, evidence);
    }
    const stageScores: Record<string, number> = {};
    const signalScores: Record<string, number> = {};
    const missingSignals: string[] = [];
    const rejectedSignals: string[] = [];
    const allSignalScores: Array<{ score: number; weight: number }> = [];
    for (const stage of blueprint.workflow.stages) {
      const evidence = evidenceById.get(stage.id);
      const declared = [...new Set(stage.evaluator_signals)];
      let total = 0;
      let weightTotal = 0;
      for (const [signal, score] of Object.entries(evidence?.signals ?? {})) {
        if (!declared.includes(signal)) rejectedSignals.push(`${stage.id}/${signal}`);
        else signalScores[`${stage.id}/${signal}`] = score;
      }
      for (const signal of declared) {
        const key = `${stage.id}/${signal}`;
        const weight = this.signalWeights[signal] ?? 1;
        const score = evidence?.signals[signal];
        weightTotal += weight;
        if (score === undefined) missingSignals.push(key);
        else total += score * weight;
        const resolved = score ?? 0;
        allSignalScores.push({ score: resolved, weight });
      }
      stageScores[stage.id] = weightTotal === 0 ? 0 : Number((total / weightTotal).toFixed(12));
    }
    const totalWeight = allSignalScores.reduce((sum, row) => sum + row.weight, 0);
    const reward = totalWeight === 0 ? 0 : Number((allSignalScores.reduce((sum, row) => sum + row.score * row.weight, 0) / totalWeight).toFixed(12));
    const passed = execution.status === "completed" && missingSignals.length === 0 && rejectedSignals.length === 0 && Object.values(signalScores).every((score) => score >= this.passThreshold);
    const status: AutonomousWorkflowEvaluation["status"] = passed ? "passed" : execution.status === "completed" && missingSignals.length === 0 ? "failed" : "incomplete";
    const evidenceDescriptor = {
      task_digest: blueprint.task_digest,
      workflow_digest: blueprint.workflow.workflow_digest,
      context_digest: blueprint.learning_context_digest,
      learning_context: { ...blueprint.selection_context },
      plan_digest: blueprint.plan.plan_digest,
      stages: input.stages.map((stage) => ({ stage_id: stage.stage_id, signals: Object.fromEntries(Object.entries(stage.signals).sort(([left], [right]) => left.localeCompare(right))), evidence_digest: stage.evidence_digest ?? null })).sort((left, right) => left.stage_id.localeCompare(right.stage_id)),
      evidence_digest: null,
    };
    const computedEvidenceDigest = await digestJson(evidenceDescriptor);
    if (input.evidence_digest !== undefined && input.evidence_digest !== computedEvidenceDigest) throw new ArgumentError("workflow evaluator evidence_digest does not match the normalized evidence packet");
    const evidenceDigest = computedEvidenceDigest;
    const descriptor = {
      schema: AUTONOMOUS_EVALUATION_SCHEMA,
      evaluator_id: this.evaluatorId ?? `typescript-${blueprint.domain_profile.domain}-workflow-evaluator`,
      evaluator_version: this.evaluatorVersion,
      domain: blueprint.domain_profile.domain,
      task_digest: blueprint.task_digest,
      workflow_digest: blueprint.workflow.workflow_digest,
      context_digest: blueprint.learning_context_digest,
      learning_context: { ...blueprint.selection_context },
      plan_digest: blueprint.plan.plan_digest,
      execution_status: execution.status,
      stage_scores: stageScores,
      signal_scores: signalScores,
      missing_signals: [...new Set(missingSignals)].sort(),
      rejected_signals: [...new Set(rejectedSignals)].sort(),
      required_signals: requiredSignalsFor(execution),
      pass_threshold: this.passThreshold,
      reward,
      passed,
      status,
      evidence_digest: evidenceDigest,
      evaluator_authority: "caller_declared_signal_scoring_only" as const,
      retention: PRIVATE_RETENTION,
    };
    return { ...descriptor, evaluation_digest: await digestJson(descriptor) };
  }
}

export class InMemoryAutonomousLearningEpisodeStore implements AutonomousLearningEpisodeStore {
  private readonly episodes = new Map<string, AutonomousLearningEpisode>();

  load(episodeId: string): AutonomousLearningEpisode | null {
    return clone(this.episodes.get(boundedIdentifier("episodeId", episodeId)) ?? null);
  }

  save(episode: AutonomousLearningEpisode): void {
    boundedIdentifier("episode_id", episode.episode_id);
    if (episode.status !== "pending" || episode.settlement !== null) throw new ArgumentError("only pending unsettled learning episodes can be saved");
    const prior = this.episodes.get(episode.episode_id);
    if (prior) {
      if (prior.episode_digest !== episode.episode_digest) throw new ArgumentError(`learning episode ${episode.episode_id} conflicts with an existing identity`);
      if (prior.status === "settled") throw new ArgumentError(`learning episode ${episode.episode_id} is already settled`);
    }
    if (this.episodes.size >= 4096 && !prior) throw new ArgumentError("learning episode store is full");
    this.episodes.set(episode.episode_id, clone(episode));
  }

  markSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): AutonomousLearningEpisode {
    const id = boundedIdentifier("episodeId", episodeId);
    const prior = this.episodes.get(id);
    if (!prior) throw new ArgumentError(`learning episode ${id} was not found`);
    if (prior.status === "settled") throw new ArgumentError(`learning episode ${id} has already been settled`);
    const next = { ...prior, status: "settled" as const, settlement: clone(settlement) };
    this.episodes.set(id, clone(next));
    return clone(next);
  }

  pending(limit = 256): AutonomousLearningEpisode[] {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 256) throw new ArgumentError("learning episode pending limit is outside its bounds");
    return [...this.episodes.values()].filter((episode) => episode.status === "pending").slice(0, limit).map((episode) => clone(episode));
  }

  snapshotRows(): AutonomousLearningEpisode[] {
    return [...this.episodes.values()].map((episode) => clone(episode));
  }

  restoreRows(rows: readonly AutonomousLearningEpisode[]): void {
    if (!Array.isArray(rows) || rows.length > 4096) throw new ArgumentError("learning episode snapshot is outside its bounds");
    for (const episode of rows) {
      if (!isObject(episode) || typeof episode.episode_id !== "string" || typeof episode.episode_digest !== "string" || typeof episode.status !== "string" || !["pending", "settled"].includes(episode.status)) throw new ArgumentError("learning episode snapshot row is malformed");
      const row = episode as unknown as AutonomousLearningEpisode;
      boundedIdentifier("episode_id", row.episode_id);
      boundedDigest("episode_digest", row.episode_digest);
      const prior = this.episodes.get(row.episode_id);
      if (prior && prior.episode_digest !== row.episode_digest) throw new ArgumentError(`learning episode ${row.episode_id} conflicts during restore`);
      this.episodes.set(row.episode_id, clone(row));
    }
  }
}

export class InMemoryAutonomousLearningTrajectoryStore implements AutonomousLearningTrajectoryStore {
  private readonly trajectories = new Map<string, AutonomousLearningTrajectory>();

  load(trajectoryId: string): AutonomousLearningTrajectory | null {
    return clone(this.trajectories.get(boundedIdentifier("trajectoryId", trajectoryId)) ?? null);
  }

  save(trajectory: AutonomousLearningTrajectory): void {
    boundedIdentifier("trajectory_id", trajectory.trajectory_id);
    if (trajectory.status !== "pending") throw new ArgumentError("only pending learning trajectories can be saved");
    const prior = this.trajectories.get(trajectory.trajectory_id);
    if (prior && prior.trajectory_digest !== trajectory.trajectory_digest) throw new ArgumentError(`learning trajectory ${trajectory.trajectory_id} conflicts with an existing identity`);
    if (this.trajectories.size >= 1024 && !prior) throw new ArgumentError("learning trajectory store is full");
    this.trajectories.set(trajectory.trajectory_id, clone(trajectory));
  }

  markSettled(trajectoryId: string, settlementDigest: Digest): AutonomousLearningTrajectory {
    const id = boundedIdentifier("trajectoryId", trajectoryId);
    boundedDigest("trajectory settlement_digest", settlementDigest);
    const prior = this.trajectories.get(id);
    if (!prior) throw new ArgumentError(`learning trajectory ${id} was not found`);
    if (prior.status === "settled") throw new ArgumentError(`learning trajectory ${id} has already been settled`);
    const next = { ...prior, status: "settled" as const, settlement_digest: settlementDigest };
    this.trajectories.set(id, clone(next));
    return clone(next);
  }

  snapshotRows(): AutonomousLearningTrajectory[] {
    return [...this.trajectories.values()].map((trajectory) => clone(trajectory));
  }

  restoreRows(rows: readonly AutonomousLearningTrajectory[]): void {
    if (!Array.isArray(rows) || rows.length > 1024) throw new ArgumentError("learning trajectory snapshot is outside its bounds");
    for (const trajectory of rows) {
      if (!isObject(trajectory) || typeof trajectory.trajectory_id !== "string" || typeof trajectory.trajectory_digest !== "string" || typeof trajectory.status !== "string" || !["pending", "settled"].includes(trajectory.status)) throw new ArgumentError("learning trajectory snapshot row is malformed");
      const row = trajectory as unknown as AutonomousLearningTrajectory;
      boundedIdentifier("trajectory_id", row.trajectory_id);
      boundedDigest("trajectory_digest", row.trajectory_digest);
      const prior = this.trajectories.get(row.trajectory_id);
      if (prior && prior.trajectory_digest !== row.trajectory_digest) throw new ArgumentError(`learning trajectory ${row.trajectory_id} conflicts during restore`);
      this.trajectories.set(row.trajectory_id, clone(row));
    }
  }
}

/** Bounded process-local receipt journal for tests and single-process deployments. */
export class InMemoryAutonomousLearningSettlementReceiptStore implements AutonomousLearningSettlementReceiptStore {
  private readonly receipts = new Map<string, AutonomousLearningSettlementReceipt>();

  load(idempotencyKey: string): AutonomousLearningSettlementReceipt | null {
    const key = boundedIdentifier("settlement receipt idempotency_key", idempotencyKey);
    const receipt = this.receipts.get(key);
    return receipt ? clone(receipt) : null;
  }

  save(receipt: AutonomousLearningSettlementReceipt): void {
    assertSettlementReceiptShape(receipt);
    const prior = this.receipts.get(receipt.idempotency_key);
    if (prior && (prior.request_digest !== receipt.request_digest || prior.target_digest !== receipt.target_digest || prior.operation !== receipt.operation || prior.settlement_digest !== receipt.settlement_digest)) {
      throw new ArgumentError(`settlement receipt ${receipt.idempotency_key} conflicts with an existing identity`);
    }
    if (this.receipts.size >= 8192 && !prior) throw new ArgumentError("learning settlement receipt store is full");
    this.receipts.set(receipt.idempotency_key, clone(receipt));
  }

  rows(): AutonomousLearningSettlementReceipt[] {
    return [...this.receipts.values()].map((receipt) => clone(receipt));
  }

  snapshot(): AutonomousLearningSettlementReceiptSnapshot {
    const descriptor = {
      schema: AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SNAPSHOT_SCHEMA,
      receipts: this.rows().sort((left, right) => left.idempotency_key.localeCompare(right.idempotency_key)),
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
    assertSettlementReceiptSnapshotShape(snapshot);
    return clone(snapshot);
  }

  restore(snapshot: AutonomousLearningSettlementReceiptSnapshot): void {
    const validated = validateAutonomousLearningSettlementReceiptSnapshot(snapshot);
    this.receipts.clear();
    for (const receipt of validated.receipts) this.receipts.set(receipt.idempotency_key, clone(receipt));
  }
}

/** Strict canonical JSON persistence for value-only settlement receipt snapshots. */
export class JsonAutonomousLearningSettlementReceiptPersistence implements AutonomousLearningSettlementReceiptSnapshotPersistence {
  constructor(readonly textStore: AutonomousLearningSettlementReceiptTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("learning settlement receipt text store is malformed");
  }

  async read(): Promise<AutonomousLearningSettlementReceiptSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_LEARNING_MAX_SETTLEMENT_RECEIPT_SNAPSHOT_BYTES) throw new ArgumentError("learning settlement receipt JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("learning settlement receipt JSON is invalid"); }
    return validateAutonomousLearningSettlementReceiptSnapshot(parsed);
  }

  async write(snapshot: AutonomousLearningSettlementReceiptSnapshot): Promise<void> {
    const validated = validateAutonomousLearningSettlementReceiptSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

/** Canonical receipt persistence with atomic compare-and-swap for concurrent settlement workers. */
export class TransactionalJsonAutonomousLearningSettlementReceiptPersistence extends JsonAutonomousLearningSettlementReceiptPersistence {
  declare readonly textStore: AutonomousLearningSettlementReceiptTransactionalTextStore;

  constructor(textStore: AutonomousLearningSettlementReceiptTransactionalTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("learning settlement receipt text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: Digest | null, snapshot: AutonomousLearningSettlementReceiptSnapshot): Promise<boolean> {
    const validated = validateAutonomousLearningSettlementReceiptSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

/** Browser-compatible receipt text storage; the embedding application owns encryption and retention. */
export class WebStorageAutonomousLearningSettlementReceiptTextStore implements AutonomousLearningSettlementReceiptTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("learning settlement receipt Web Storage adapter is malformed");
    boundedIdentifier("learning settlement receipt storage key", key);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/** Restart-aware receipt store that flushes every publication and fences stale writers. */
export class AutonomousLearningSettlementReceiptPersistenceCoordinator implements AutonomousLearningSettlementReceiptStore {
  private expectedSnapshotDigest: Digest | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousLearningSettlementReceiptSnapshotStore, readonly persistence: AutonomousLearningSettlementReceiptSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("learning settlement receipt snapshot store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("learning settlement receipt persistence is malformed");
  }

  async restore(): Promise<AutonomousLearningSettlementReceiptSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        const empty = new InMemoryAutonomousLearningSettlementReceiptStore().snapshot();
        this.store.restore(empty);
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = validateAutonomousLearningSettlementReceiptSnapshot(raw);
      this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousLearningSettlementReceiptSnapshot> {
    return this.enqueue(() => this.flushCurrent());
  }

  async load(idempotencyKey: string): Promise<AutonomousLearningSettlementReceipt | null> {
    return this.enqueue(() => this.store.load(idempotencyKey));
  }

  async save(receipt: AutonomousLearningSettlementReceipt): Promise<void> {
    await this.mutate(() => this.store.save(receipt));
  }

  private async flushCurrent(): Promise<AutonomousLearningSettlementReceiptSnapshot> {
    const snapshot = validateAutonomousLearningSettlementReceiptSnapshot(this.store.snapshot());
    if (typeof this.persistence.writeIfUnchanged === "function") {
      if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("learning settlement receipt persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return clone(snapshot);
  }

  private async mutate<T>(operation: () => T | Promise<T>): Promise<T> {
    return this.enqueue(async () => {
      const before = this.store.snapshot();
      const result = await operation();
      try {
        await this.flushCurrent();
        return result;
      } catch (error) {
        this.store.restore(before);
        throw error;
      }
    });
  }

  private enqueue<T>(operation: () => Promise<T> | T): Promise<T> {
    const queued = this.operationTail.then(operation);
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

/**
 * Bounded process-local feedback outbox. Durable deployments should implement the same contract
 * with a conditional update/lease in their database; this implementation is useful for tests and
 * single-worker applications and deliberately models the same claim semantics.
 */
export class InMemoryAutonomousLearningFeedbackOutboxStore implements AutonomousLearningFeedbackOutboxStore {
  private readonly commands = new Map<string, AutonomousLearningFeedbackOutboxCommand>();

  load(commandId: string): AutonomousLearningFeedbackOutboxCommand | null {
    const id = boundedIdentifier("feedback outbox command_id", commandId);
    const command = this.commands.get(id);
    if (command) assertFeedbackOutboxCommandShape(command);
    return command ? clone(command) : null;
  }

  save(command: AutonomousLearningFeedbackOutboxCommand): void {
    assertFeedbackOutboxCommandShape(command);
    const prior = this.commands.get(command.command_id);
    if (prior && (prior.target_digest !== command.target_digest || prior.request_digest !== command.request_digest || prior.operation !== command.operation)) {
      throw new ArgumentError(`feedback outbox command ${command.command_id} conflicts with an existing identity`);
    }
    if (this.commands.size >= AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX && !prior) throw new ArgumentError("feedback outbox is full");
    this.commands.set(command.command_id, clone(command));
  }

  pending(limit = 64, now = Date.now()): AutonomousLearningFeedbackOutboxCommand[] {
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX) throw new ArgumentError("feedback outbox pending limit is outside its bounds");
    boundedOutboxTimestamp("feedback outbox pending now", now);
    return [...this.commands.values()]
      .filter((command) => command.status === "pending" || (command.status === "leased" && (command.lease_until ?? 0) <= now))
      .filter((command) => command.available_at <= now)
      .sort((left, right) => left.created_at - right.created_at || left.command_id.localeCompare(right.command_id))
      .slice(0, limit)
      .map((command) => clone(command));
  }

  claim(commandId: string, workerId: string, leaseMs: number, now = Date.now()): AutonomousLearningFeedbackOutboxCommand | null {
    const id = boundedIdentifier("feedback outbox command_id", commandId);
    const owner = boundedIdentifier("feedback outbox worker_id", workerId);
    if (!Number.isSafeInteger(leaseMs) || leaseMs < 1 || leaseMs > 10 * 60_000) throw new ArgumentError("feedback outbox leaseMs is outside its bounds");
    boundedOutboxTimestamp("feedback outbox claim now", now);
    const prior = this.commands.get(id);
    if (!prior) throw new ArgumentError(`feedback outbox command ${id} was not found`);
    if (prior.status === "applied" || prior.status === "failed") return null;
    if (prior.status === "leased" && (prior.lease_until ?? 0) > now) return null;
    const claimed = refreshFeedbackOutboxCommand({
      ...prior,
      status: "leased",
      attempts: prior.attempts + 1,
      lease_owner: owner,
      lease_until: now + leaseMs,
      updated_at: now,
    });
    assertFeedbackOutboxCommandShape(claimed);
    this.commands.set(id, clone(claimed));
    return clone(claimed);
  }

  markApplied(commandId: string, workerId: string, resultDigest: Digest, now = Date.now()): AutonomousLearningFeedbackOutboxCommand {
    const id = boundedIdentifier("feedback outbox command_id", commandId);
    const owner = boundedIdentifier("feedback outbox worker_id", workerId);
    boundedDigest("feedback outbox result_digest", resultDigest);
    boundedOutboxTimestamp("feedback outbox applied now", now);
    const prior = this.commands.get(id);
    if (!prior) throw new ArgumentError(`feedback outbox command ${id} was not found`);
    if (prior.status === "applied") {
      if (prior.result_digest !== resultDigest) throw new ArgumentError(`feedback outbox command ${id} has a conflicting result digest`);
      return clone(prior);
    }
    if (prior.status !== "leased" || prior.lease_owner !== owner) throw new ArgumentError(`feedback outbox command ${id} is not leased by this worker`);
    const applied = refreshFeedbackOutboxCommand({ ...prior, status: "applied", lease_owner: null, lease_until: null, last_error_class: null, result_digest: resultDigest, updated_at: now });
    assertFeedbackOutboxCommandShape(applied);
    this.commands.set(id, clone(applied));
    return clone(applied);
  }

  markFailed(commandId: string, workerId: string, errorClass: string, retryable: boolean, now = Date.now()): AutonomousLearningFeedbackOutboxCommand {
    const id = boundedIdentifier("feedback outbox command_id", commandId);
    const owner = boundedIdentifier("feedback outbox worker_id", workerId);
    const boundedError = boundedIdentifier("feedback outbox error class", errorClass);
    boundedOutboxTimestamp("feedback outbox failed now", now);
    const prior = this.commands.get(id);
    if (!prior) throw new ArgumentError(`feedback outbox command ${id} was not found`);
    if (prior.status === "failed" && !retryable) return clone(prior);
    if (prior.status !== "leased" || prior.lease_owner !== owner) throw new ArgumentError(`feedback outbox command ${id} is not leased by this worker`);
    const delay = retryable ? Math.min(60_000, 250 * (2 ** Math.min(prior.attempts - 1, 8))) : 0;
    const failed = refreshFeedbackOutboxCommand({
      ...prior,
      status: retryable ? "pending" : "failed",
      available_at: now + delay,
      lease_owner: null,
      lease_until: null,
      last_error_class: boundedError,
      updated_at: now,
    });
    assertFeedbackOutboxCommandShape(failed);
    this.commands.set(id, clone(failed));
    return clone(failed);
  }

  rows(): AutonomousLearningFeedbackOutboxCommand[] {
    return [...this.commands.values()].map((command) => clone(command));
  }

  snapshot(): AutonomousLearningFeedbackOutboxSnapshot {
    const descriptor = {
      schema: AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SNAPSHOT_SCHEMA,
      commands: this.rows().sort((left, right) => left.command_id.localeCompare(right.command_id)),
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...descriptor, snapshot_digest: digestJsonSync(descriptor) };
    assertFeedbackOutboxSnapshotShape(snapshot);
    return clone(snapshot);
  }

  restore(snapshot: AutonomousLearningFeedbackOutboxSnapshot): void {
    const validated = validateAutonomousLearningFeedbackOutboxSnapshot(snapshot);
    this.commands.clear();
    for (const command of validated.commands) this.commands.set(command.command_id, clone(command));
  }
}

/** Strict canonical JSON persistence for evaluator-feedback outbox snapshots. */
export class JsonAutonomousLearningFeedbackOutboxPersistence implements AutonomousLearningFeedbackOutboxSnapshotPersistence {
  constructor(readonly textStore: AutonomousLearningFeedbackOutboxTextStore) {
    if (!textStore || typeof textStore.read !== "function" || typeof textStore.write !== "function") throw new ArgumentError("feedback outbox text store is malformed");
  }

  async read(): Promise<AutonomousLearningFeedbackOutboxSnapshot | null> {
    const encoded = await this.textStore.read();
    if (encoded === null) return null;
    if (new TextEncoder().encode(encoded).byteLength > AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX_SNAPSHOT_BYTES) throw new ArgumentError("feedback outbox JSON exceeds its byte bound");
    let parsed: unknown;
    try { parsed = JSON.parse(encoded); } catch { throw new ArgumentError("feedback outbox JSON is invalid"); }
    return validateAutonomousLearningFeedbackOutboxSnapshot(parsed);
  }

  async write(snapshot: AutonomousLearningFeedbackOutboxSnapshot): Promise<void> {
    const validated = validateAutonomousLearningFeedbackOutboxSnapshot(snapshot);
    await this.textStore.write(canonicalJson(validated));
  }
}

/** Canonical JSON persistence with atomic compare-and-swap support for multiple learning workers. */
export class TransactionalJsonAutonomousLearningFeedbackOutboxPersistence extends JsonAutonomousLearningFeedbackOutboxPersistence {
  declare readonly textStore: AutonomousLearningFeedbackOutboxTransactionalTextStore;

  constructor(textStore: AutonomousLearningFeedbackOutboxTransactionalTextStore) {
    super(textStore);
    this.textStore = textStore;
    if (typeof textStore.writeIfUnchanged !== "function") throw new ArgumentError("feedback outbox text store lacks compare-and-swap");
  }

  async writeIfUnchanged(expectedSnapshotDigest: Digest | null, snapshot: AutonomousLearningFeedbackOutboxSnapshot): Promise<boolean> {
    const validated = validateAutonomousLearningFeedbackOutboxSnapshot(snapshot);
    return this.textStore.writeIfUnchanged(expectedSnapshotDigest, canonicalJson(validated));
  }
}

/** Browser-compatible text storage for feedback queue snapshots; the host owns encryption and lifetime. */
export class WebStorageAutonomousLearningFeedbackOutboxTextStore implements AutonomousLearningFeedbackOutboxTextStore {
  constructor(readonly storage: { getItem(key: string): string | null; setItem(key: string, value: string): void }, readonly key: string) {
    if (!storage || typeof storage.getItem !== "function" || typeof storage.setItem !== "function") throw new ArgumentError("feedback outbox Web Storage adapter is malformed");
    boundedIdentifier("feedback outbox storage key", key);
  }

  read(): string | null { return this.storage.getItem(this.key); }
  write(value: string): void { this.storage.setItem(this.key, value); }
}

/**
 * Restart-aware outbox facade that flushes every queue mutation and fences stale workers.
 * Call `restore()` once before a worker begins consuming commands.
 */
export class AutonomousLearningFeedbackOutboxPersistenceCoordinator implements AutonomousLearningFeedbackOutboxStore {
  private expectedSnapshotDigest: Digest | null = null;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(readonly store: AutonomousLearningFeedbackOutboxSnapshotStore, readonly persistence: AutonomousLearningFeedbackOutboxSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("feedback outbox snapshot store is malformed");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("feedback outbox snapshot persistence is malformed");
  }

  async restore(): Promise<AutonomousLearningFeedbackOutboxSnapshot | null> {
    return this.enqueue(async () => {
      const raw = await this.persistence.read();
      if (raw === null) {
        const empty = new InMemoryAutonomousLearningFeedbackOutboxStore().snapshot();
        this.store.restore(empty);
        this.expectedSnapshotDigest = null;
        return null;
      }
      const snapshot = validateAutonomousLearningFeedbackOutboxSnapshot(raw);
      this.store.restore(snapshot);
      this.expectedSnapshotDigest = snapshot.snapshot_digest;
      return clone(snapshot);
    });
  }

  async flush(): Promise<AutonomousLearningFeedbackOutboxSnapshot> {
    return this.enqueue(() => this.flushCurrent());
  }

  async load(commandId: string): Promise<AutonomousLearningFeedbackOutboxCommand | null> {
    return this.enqueue(() => this.store.load(commandId));
  }

  async save(command: AutonomousLearningFeedbackOutboxCommand): Promise<void> {
    await this.mutate(() => this.store.save(command));
  }

  async pending(limit?: number, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand[]> {
    return this.enqueue(() => this.store.pending(limit, now));
  }

  async claim(commandId: string, workerId: string, leaseMs: number, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand | null> {
    return this.mutate(() => this.store.claim(commandId, workerId, leaseMs, now));
  }

  async markApplied(commandId: string, workerId: string, resultDigest: Digest, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand> {
    return this.mutate(() => this.store.markApplied(commandId, workerId, resultDigest, now));
  }

  async markFailed(commandId: string, workerId: string, errorClass: string, retryable: boolean, now?: number): Promise<AutonomousLearningFeedbackOutboxCommand> {
    return this.mutate(() => this.store.markFailed(commandId, workerId, errorClass, retryable, now));
  }

  private async flushCurrent(): Promise<AutonomousLearningFeedbackOutboxSnapshot> {
    const snapshot = validateAutonomousLearningFeedbackOutboxSnapshot(this.store.snapshot());
    if (typeof this.persistence.writeIfUnchanged === "function") {
      if (!await this.persistence.writeIfUnchanged(this.expectedSnapshotDigest, snapshot)) throw new ArgumentError("feedback outbox persistence compare-and-swap conflict");
    } else await this.persistence.write(snapshot);
    this.expectedSnapshotDigest = snapshot.snapshot_digest;
    return clone(snapshot);
  }

  private async mutate<T>(operation: () => T | Promise<T>): Promise<T> {
    return this.enqueue(async () => {
      const before = this.store.snapshot();
      const result = await operation();
      try {
        await this.flushCurrent();
        return result;
      } catch (error) {
        this.store.restore(before);
        throw error;
      }
    });
  }

  private enqueue<T>(operation: () => Promise<T> | T): Promise<T> {
    const queued = this.operationTail.then(operation);
    this.operationTail = queued.then(() => undefined, () => undefined);
    return queued;
  }
}

/** Unified caller-owned state store with integrity-checked restart snapshots. */
export class InMemoryAutonomousLearningStateStore implements AutonomousLearningStateStore {
  private readonly episodeStore = new InMemoryAutonomousLearningEpisodeStore();
  private readonly trajectoryStore = new InMemoryAutonomousLearningTrajectoryStore();
  private generation = 0;

  loadEpisode(episodeId: string): AutonomousLearningEpisode | null {
    return this.episodeStore.load(episodeId);
  }

  saveEpisode(episode: AutonomousLearningEpisode): void {
    this.episodeStore.save(episode);
  }

  markEpisodeSettled(episodeId: string, settlement: AutonomousLearningSettlementMetadata): AutonomousLearningEpisode {
    return this.episodeStore.markSettled(episodeId, settlement);
  }

  pendingEpisodes(limit = 256): AutonomousLearningEpisode[] {
    return this.episodeStore.pending(limit);
  }

  loadTrajectory(trajectoryId: string): AutonomousLearningTrajectory | null {
    return this.trajectoryStore.load(trajectoryId);
  }

  saveTrajectory(trajectory: AutonomousLearningTrajectory): void {
    this.trajectoryStore.save(trajectory);
  }

  markTrajectorySettled(trajectoryId: string, settlementDigest: Digest): AutonomousLearningTrajectory {
    return this.trajectoryStore.markSettled(trajectoryId, settlementDigest);
  }

  async snapshot(): Promise<AutonomousLearningStateSnapshot> {
    const descriptor = {
      schema: AUTONOMOUS_LEARNING_SNAPSHOT_SCHEMA,
      generation: this.generation + 1,
      episodes: this.episodeStore.snapshotRows(),
      trajectories: this.trajectoryStore.snapshotRows(),
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    const snapshot = { ...descriptor, snapshot_digest: await digestJson(descriptor) };
    this.generation = snapshot.generation;
    return clone(snapshot);
  }

  async restore(snapshot: AutonomousLearningStateSnapshot): Promise<void> {
    if (!isObject(snapshot) || snapshot.schema !== AUTONOMOUS_LEARNING_SNAPSHOT_SCHEMA || !Array.isArray(snapshot.episodes) || !Array.isArray(snapshot.trajectories)) throw new ArgumentError("learning state snapshot is malformed");
    boundedGeneration(snapshot.generation);
    const { snapshot_digest: observed, ...descriptor } = snapshot;
    boundedDigest("snapshot_digest", observed);
    const expected = await digestJson(descriptor);
    if (expected !== observed) throw new ArgumentError("learning state snapshot digest does not match");
    if (snapshot.episodes.length > 4096 || snapshot.trajectories.length > 1024) throw new ArgumentError("learning state snapshot exceeds its bounds");
    this.episodeStore.restoreRows(snapshot.episodes);
    this.trajectoryStore.restoreRows(snapshot.trajectories);
    this.generation = snapshot.generation;
  }
}

/** Coordinates an integrity-checked state store with a caller-owned durable adapter. */
export class AutonomousLearningPersistenceCoordinator {
  readonly store: AutonomousLearningStateStore;
  readonly persistence: AutonomousLearningSnapshotPersistence;

  constructor(store: AutonomousLearningStateStore, persistence: AutonomousLearningSnapshotPersistence) {
    if (!store || typeof store.snapshot !== "function" || typeof store.restore !== "function") throw new ArgumentError("learning persistence requires a state store");
    if (!persistence || typeof persistence.read !== "function" || typeof persistence.write !== "function") throw new ArgumentError("learning persistence requires read and write functions");
    this.store = store;
    this.persistence = persistence;
  }

  async restore(): Promise<AutonomousLearningStateSnapshot | null> {
    const snapshot = await this.persistence.read();
    if (snapshot) await this.store.restore(snapshot);
    return snapshot;
  }

  async flush(): Promise<AutonomousLearningStateSnapshot> {
    const snapshot = await this.store.snapshot();
    await this.persistence.write(snapshot);
    return snapshot;
  }
}

function projectOutcome(response: RestToolResponse<BrainOutcomeRecordResult>): BrainOutcomeRecordResult {
  if (!response.ok || response.mcp.error || response.mcp.result?.isError) throw new ProviderRuntimeError("brain outcome record returned a refusal");
  const projected = response.mcp.result?.structuredContent;
  if (!projected || typeof projected !== "object") throw new ProviderRuntimeError("brain outcome record returned no structured projection");
  return projected as BrainOutcomeRecordResult;
}

/** Explicit evaluator, delayed-credit, and value-only bandit handoff for every domain. */
export class AutonomousLearningController {
  readonly agent: AutonomousAgent;
  readonly episodes: AutonomousLearningEpisodeStore;
  readonly trajectories: AutonomousLearningTrajectoryStore;
  readonly settlementReceipts: AutonomousLearningSettlementReceiptStore;
  /** Caller-owned queue for restart-safe evaluator settlement dispatch. */
  readonly feedbackOutbox: AutonomousLearningFeedbackOutboxStore;
  /** Optional caller-owned memory sink used to attach explicit evaluator feedback to recallable episodes. */
  readonly memoryStore?: AutonomousEpisodicMemoryStore;
  readonly evaluator: AutonomousWorkflowEvaluator;
  readonly apiClient?: ApiClient;
  readonly calibrationReport?: AutonomousEvaluatorCalibrationReport;
  readonly requireCalibratedLearning: boolean;

  constructor(agent: AutonomousAgent, options: AutonomousLearningControllerOptions = {}) {
    if (!agent || typeof agent.recordEvaluatorReward !== "function") throw new ArgumentError("learning controller requires an AutonomousAgent");
    if (options.requireCalibratedLearning !== undefined && typeof options.requireCalibratedLearning !== "boolean") throw new ArgumentError("learning controller requireCalibratedLearning must be boolean");
    if (options.requireCalibratedLearning === true && options.calibrationReport === undefined) throw new ArgumentError("learning controller requires a calibrationReport when requireCalibratedLearning is true");
    this.agent = agent;
    const stateStore = options.store;
    this.episodes = options.episodes ?? (stateStore ? {
      load: (episodeId: string) => stateStore.loadEpisode(episodeId),
      save: (episode: AutonomousLearningEpisode) => stateStore.saveEpisode(episode),
      markSettled: (episodeId: string, settlement: AutonomousLearningSettlementMetadata) => stateStore.markEpisodeSettled(episodeId, settlement),
      pending: (limit?: number) => stateStore.pendingEpisodes(limit),
    } : new InMemoryAutonomousLearningEpisodeStore());
    this.trajectories = options.trajectories ?? (stateStore ? {
      load: (trajectoryId: string) => stateStore.loadTrajectory(trajectoryId),
      save: (trajectory: AutonomousLearningTrajectory) => stateStore.saveTrajectory(trajectory),
      markSettled: (trajectoryId: string, settlementDigest: Digest) => stateStore.markTrajectorySettled(trajectoryId, settlementDigest),
    } : new InMemoryAutonomousLearningTrajectoryStore());
    this.settlementReceipts = options.settlementReceipts ?? new InMemoryAutonomousLearningSettlementReceiptStore();
    if (options.feedbackOutbox !== undefined && (
      typeof options.feedbackOutbox.load !== "function"
      || typeof options.feedbackOutbox.save !== "function"
      || typeof options.feedbackOutbox.pending !== "function"
      || typeof options.feedbackOutbox.claim !== "function"
      || typeof options.feedbackOutbox.markApplied !== "function"
      || typeof options.feedbackOutbox.markFailed !== "function"
    )) throw new ArgumentError("learning feedbackOutbox is malformed");
    this.feedbackOutbox = options.feedbackOutbox ?? new InMemoryAutonomousLearningFeedbackOutboxStore();
    this.memoryStore = options.memoryStore ?? agent.memoryStore;
    this.evaluator = options.evaluator ?? new AutonomousWorkflowEvaluator();
    this.apiClient = options.apiClient;
    this.calibrationReport = options.calibrationReport === undefined ? undefined : validateAutonomousEvaluatorCalibrationReport(options.calibrationReport);
    this.requireCalibratedLearning = options.requireCalibratedLearning === true;
  }

  private assertLearningAdmission(domain: AutonomousDomainName): void {
    if (!this.requireCalibratedLearning) return;
    if (!this.calibrationReport) throw new ProviderRuntimeError("learning settlement is missing its required evaluator calibration report");
    assertAutonomousEvaluatorCalibrationReady(this.calibrationReport, domain);
  }

  private async loadReceipt(idempotencyKey: string, operation: AutonomousLearningSettlementReceipt["operation"], targetId: string, targetDigest: Digest, requestDigest: Digest): Promise<AutonomousLearningSettlementReceipt | null> {
    const receipt = await this.settlementReceipts.load(idempotencyKey);
    if (!receipt) return null;
    assertSettlementReceiptShape(receipt);
    const expectedSettlementDigest = await digestJson(receipt.settlement);
    if (expectedSettlementDigest !== receipt.settlement_digest) throw new ArgumentError(`settlement receipt ${idempotencyKey} failed integrity verification`);
    if (receipt.operation !== operation || receipt.target_id !== targetId || receipt.target_digest !== targetDigest || receipt.request_digest !== requestDigest) throw new ArgumentError(`settlement idempotency key ${idempotencyKey} conflicts with a different learning settlement`);
    return clone(receipt);
  }

  private async saveReceipt(operation: AutonomousLearningSettlementReceipt["operation"], idempotencyKey: string, targetId: string, targetDigest: Digest, requestDigest: Digest, settlement: AutonomousLearningSettlement | AutonomousTrajectorySettlement): Promise<AutonomousLearningSettlementReceipt> {
    assertValueOnlySettlement(settlement);
    const receipt = {
      schema: AUTONOMOUS_LEARNING_SETTLEMENT_RECEIPT_SCHEMA,
      operation,
      idempotency_key: idempotencyKey,
      target_id: targetId,
      target_digest: targetDigest,
      request_digest: requestDigest,
      settlement,
      settlement_digest: await digestJson(settlement),
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    } satisfies AutonomousLearningSettlementReceipt;
    await this.settlementReceipts.save(receipt);
    return clone(receipt);
  }

  private async episodeSettlementKey(trajectoryId: string, episodeId: string): Promise<string> {
    return `trajectory-episode:${await digestJson({ trajectory_id: trajectoryId, episode_id: episodeId })}`;
  }

  private async assertEpisodeLearningAdmission(episodeId: string): Promise<void> {
    if (!this.requireCalibratedLearning) return;
    const id = boundedIdentifier("episodeId", episodeId);
    const episode = await this.episodes.load(id);
    if (!episode) throw new ArgumentError(`learning episode ${episodeId} was not found`);
    this.assertLearningAdmission(episode.domain);
  }

  private async assertTrajectoryLearningAdmission(trajectoryId: string): Promise<void> {
    if (!this.requireCalibratedLearning) return;
    const id = boundedIdentifier("trajectoryId", trajectoryId);
    const trajectory = await this.trajectories.load(id);
    if (!trajectory) throw new ArgumentError(`learning trajectory ${trajectoryId} was not found`);
    for (const step of trajectory.steps) await this.assertEpisodeLearningAdmission(step.episode_id);
  }

  async evaluateWorkflow(execution: AutonomousWorkflowExecutionResult, input: AutonomousWorkflowEvaluationInput): Promise<AutonomousWorkflowEvaluation> {
    return this.evaluator.evaluate(execution, input);
  }

  async prepareRun(result: AutonomousRunResult, options: { episodeId: string; runId?: string; stageId?: string; parentJobId?: string; planRefinementDigest?: string | null; memoryEpisodeId?: string | null }): Promise<AutonomousLearningEpisode> {
    if (!isObject(options)) throw new ArgumentError("learning episode options must be an object");
    const episodeId = boundedIdentifier("episodeId", options.episodeId);
    const planRefinementDigest = options.planRefinementDigest === undefined ? null : boundedDigest("planRefinementDigest", options.planRefinementDigest, true);
    const memoryEpisodeId = options.memoryEpisodeId === undefined || options.memoryEpisodeId === null ? null : boundedIdentifier("memoryEpisodeId", options.memoryEpisodeId);
    if (result.status !== "completed" || !result.blueprint || !result.selection?.selected_model) throw new ArgumentError("learning episode requires a completed autonomous run");
    const runId = boundedIdentifier("runId", options.runId ?? episodeId);
    const selectionDigest = await digestJson(result.selection);
    const outcomeDigest = await digestJson({ status: result.status, route_digest: result.route.route_digest, selection: result.selection, response: result.response });
    const run: BrainRunIdentity = {
      run_id: runId,
      selection_digest: selectionDigest,
      prompt_digest: result.blueprint.prompt.prompt_digest,
      plan_digest: result.blueprint.plan.plan_digest,
      provider: result.selection.selected_model.provider,
      model: result.selection.selected_model.model,
      outcome_digest: outcomeDigest,
      request_id: null,
    };
    const descriptor = {
      schema: AUTONOMOUS_LEARNING_EPISODE_SCHEMA,
      episode_id: episodeId,
      run,
      domain: result.blueprint.domain_profile.domain,
      capability: result.blueprint.selection_context.capability,
      workflow_id: result.blueprint.workflow.workflow_id,
      stage_id: options.stageId === undefined ? null : boundedIdentifier("stageId", options.stageId),
      parent_job_id: options.parentJobId === undefined ? null : boundedIdentifier("parentJobId", options.parentJobId),
      memory_episode_id: memoryEpisodeId,
      workflow_digest: result.blueprint.workflow.workflow_digest,
      plan_refinement_digest: planRefinementDigest,
      context_digest: result.blueprint.learning_context_digest ?? null,
      learning_context: { ...result.blueprint.selection_context },
      status: "pending" as const,
      settlement: null,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned" as const,
    };
    const episode = { ...descriptor, episode_digest: await digestJson(descriptor) };
    const prior = await this.episodes.load(episodeId);
    if (prior) {
      if (prior.episode_digest !== episode.episode_digest) throw new ArgumentError(`learning episode ${episodeId} conflicts with an existing identity`);
      return clone(prior);
    }
    this.episodes.save(episode);
    return clone(episode);
  }

  /** Build a trajectory from the pending stage episodes emitted by AutonomousWorkflowExecutor. */
  async prepareWorkflowTrajectory(execution: AutonomousWorkflowExecutionResult, options: { trajectoryId: string; discount?: number }): Promise<AutonomousLearningTrajectory> {
    const ids = [...new Set((execution.learning_episode_ids ?? []))];
    if (!ids.length) throw new ArgumentError("workflow execution has no learning episodes");
    const pending: string[] = [];
    for (const id of ids) if ((await this.episodes.load(id))?.status === "pending") pending.push(id);
    if (pending.length) return this.prepareTrajectory(pending, options);
    if (await this.trajectories.load(options.trajectoryId)) return this.prepareTrajectory(ids, options);
    throw new ArgumentError("workflow execution has no pending learning episodes");
  }

  /** Evaluate and settle the pending workflow stages with one explicit signal packet. */
  async settleWorkflow(execution: AutonomousWorkflowExecutionResult, input: AutonomousWorkflowEvaluationInput, options: { trajectoryId: string; discount?: number; remote?: boolean; idempotencyKey?: string; outbox?: AutonomousLearningOutboxSettlementOptions }): Promise<AutonomousWorkflowLearningSettlement> {
    const evaluation = await this.evaluateWorkflow(execution, input);
    const trajectory = await this.prepareWorkflowTrajectory(execution, options);
    const rewards: Record<string, AutonomousEvaluatorRewardInput> = {};
    for (const step of trajectory.steps) {
      const episode = await this.episodes.load(step.episode_id);
      if (!episode) throw new ArgumentError(`workflow learning episode ${step.episode_id} disappeared during settlement`);
      const stageId = episode.stage_id;
      const score = stageId === null ? 0 : evaluation.stage_scores[stageId] ?? 0;
      const stage = execution.blueprint?.workflow.stages.find((candidate) => candidate.id === stageId);
      const evidence = input.stages.find((candidate) => candidate.stage_id === stageId);
      const missing = stage ? stage.evaluator_signals.some((signal) => evidence?.signals[signal] === undefined) : true;
      rewards[step.episode_id] = {
        evaluator_id: evaluation.evaluator_id,
        evaluator_version: evaluation.evaluator_version,
        reward: score,
        passed: !missing && score >= evaluation.pass_threshold,
        evidence_digest: evidence?.evidence_digest ?? evaluation.evidence_digest,
      };
    }
    const settled = await this.settleTrajectory(trajectory.trajectory_id, rewards, { remote: options.remote, idempotencyKey: options.idempotencyKey, outbox: options.outbox });
    return { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, evaluation, trajectory: settled, retention: PRIVATE_RETENTION };
  }

  /** Build a trajectory from completed specialist and synthesis episodes emitted by runCrossDomain. */
  async prepareCrossDomainTrajectory(result: AutonomousCrossDomainRunResult, options: { trajectoryId: string; discount?: number }): Promise<AutonomousLearningTrajectory> {
    const ids = [...new Set(result.learning_episode_ids ?? [])];
    if (!ids.length) throw new ArgumentError("cross-domain result has no learning episodes");
    const pending: string[] = [];
    for (const id of ids) if ((await this.episodes.load(id))?.status === "pending") pending.push(id);
    if (pending.length) return this.prepareTrajectory(pending, options);
    if (await this.trajectories.load(options.trajectoryId)) return this.prepareTrajectory(ids, options);
    throw new ArgumentError("cross-domain result has no pending learning episodes");
  }

  /** Settle child and synthesis episodes with exact caller-provided evaluator rewards. */
  async settleCrossDomain(result: AutonomousCrossDomainRunResult, rewards: Record<string, AutonomousEvaluatorRewardInput>, options: { trajectoryId: string; discount?: number; remote?: boolean; idempotencyKey?: string; outbox?: AutonomousLearningOutboxSettlementOptions }): Promise<AutonomousCrossDomainLearningSettlement> {
    const trajectory = await this.prepareCrossDomainTrajectory(result, options);
    const settled = await this.settleTrajectory(trajectory.trajectory_id, rewards, { remote: options.remote, idempotencyKey: options.idempotencyKey, outbox: options.outbox });
    return { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, result, trajectory: settled, retention: PRIVATE_RETENTION };
  }

  private async recordMemoryEvaluation(
    episode: AutonomousLearningEpisode,
    input: AutonomousEvaluatorRewardInput,
    store: AutonomousEpisodicMemoryStore | undefined,
  ): Promise<AutonomousLearningMemoryEvaluationProjection> {
    if (!store) return { status: "not_configured", memory_episode_id: episode.memory_episode_id ?? null, evaluation_digest: null, error_class: null };
    if (!episode.memory_episode_id) return { status: "not_linked", memory_episode_id: null, evaluation_digest: null, error_class: null };
    const normalized = {
      evaluator_id: input.evaluator_id,
      evaluator_version: input.evaluator_version,
      reward: input.reward,
      passed: input.passed,
      failed: input.failed ?? !input.passed,
      feedback_digest: input.feedback_digest ?? null,
      failure_class: input.failure_class ?? null,
      evidence_digest: input.evidence_digest ?? null,
    };
    const evaluationDigest = await digestJson(normalized);
    try {
      await store.recordEvaluation(episode.memory_episode_id, normalized);
      return { status: "recorded", memory_episode_id: episode.memory_episode_id, evaluation_digest: evaluationDigest, error_class: null };
    } catch (error) {
      return {
        status: "failed",
        memory_episode_id: episode.memory_episode_id,
        evaluation_digest: null,
        error_class: error instanceof Error && error.constructor.name.trim() ? error.constructor.name : "MemoryError",
      };
    }
  }

  private async makeFeedbackCommand(
    command: Omit<AutonomousLearningFeedbackOutboxCommand, "command_digest">,
  ): Promise<AutonomousLearningFeedbackOutboxCommand> {
    const completed = { ...command, command_digest: await digestJson(command) };
    assertFeedbackOutboxCommandShape(completed);
    return clone(completed);
  }

  /** Queue one evaluator packet without mutating the learner or memory store. */
  async enqueueRunSettlement(episodeId: string, input: AutonomousEvaluatorRewardInput, options: { creditedReward?: number; remote?: boolean; idempotencyKey?: string } = {}): Promise<AutonomousLearningFeedbackOutboxCommand> {
    const id = boundedIdentifier("episodeId", episodeId);
    const episode = await this.episodes.load(id);
    if (!episode) throw new ArgumentError(`learning episode ${episodeId} was not found`);
    const normalizedInput = normalizeRewardInput(input);
    const creditedReward = boundedReward("credited reward", options.creditedReward ?? normalizedInput.reward);
    const commandId = boundedIdentifier("feedback outbox idempotencyKey", options.idempotencyKey ?? `episode:${id}`);
    const requestDigest = await digestJson({ episode_digest: episode.episode_digest, input: normalizedInput, credited_reward: creditedReward, remote: options.remote === true });
    const existing = await this.feedbackOutbox.load(commandId);
    if (existing) {
      if (existing.operation !== "single_run" || existing.target_id !== id || existing.target_digest !== episode.episode_digest || existing.request_digest !== requestDigest) throw new ArgumentError(`feedback outbox command ${commandId} conflicts with a different learning settlement`);
      return clone(existing);
    }
    const now = Date.now();
    const command = await this.makeFeedbackCommand({
      schema: AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
      command_id: commandId,
      operation: "single_run",
      target_id: id,
      target_digest: episode.episode_digest,
      request_digest: requestDigest,
      remote: options.remote === true,
      payload: { operation: "single_run", episode_id: id, reward_input: normalizedInput, credited_reward: creditedReward },
      status: "pending",
      attempts: 0,
      available_at: now,
      lease_owner: null,
      lease_until: null,
      last_error_class: null,
      result_digest: null,
      created_at: now,
      updated_at: now,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned",
    });
    await this.feedbackOutbox.save(command);
    return clone(command);
  }

  /** Queue a delayed-credit trajectory settlement as one replay-safe outbox command. */
  async enqueueTrajectorySettlement(trajectoryId: string, rewards: Record<string, AutonomousEvaluatorRewardInput>, options: { remote?: boolean; idempotencyKey?: string } = {}): Promise<AutonomousLearningFeedbackOutboxCommand> {
    const id = boundedIdentifier("trajectoryId", trajectoryId);
    const trajectory = await this.trajectories.load(id);
    if (!trajectory) throw new ArgumentError(`learning trajectory ${trajectoryId} was not found`);
    if (!isObject(rewards)) throw new ArgumentError("trajectory rewards must be an object keyed by episode ID");
    const expected = new Set(trajectory.steps.map((step) => step.episode_id));
    const supplied = Object.keys(rewards);
    if (supplied.length !== expected.size || supplied.some((episodeId) => !expected.has(episodeId))) throw new ArgumentError("trajectory rewards must cover exactly every episode");
    const normalizedRewards = Object.fromEntries(trajectory.steps.map((step) => [step.episode_id, normalizeRewardInput(rewards[step.episode_id]!) ]));
    const commandId = boundedIdentifier("feedback outbox idempotencyKey", options.idempotencyKey ?? `trajectory:${id}`);
    const requestDigest = await digestJson({ trajectory_digest: trajectory.trajectory_digest, rewards: normalizedRewards, remote: options.remote === true });
    const existing = await this.feedbackOutbox.load(commandId);
    if (existing) {
      if (existing.operation !== "trajectory" || existing.target_id !== id || existing.target_digest !== trajectory.trajectory_digest || existing.request_digest !== requestDigest) throw new ArgumentError(`feedback outbox command ${commandId} conflicts with a different learning trajectory settlement`);
      return clone(existing);
    }
    const now = Date.now();
    const command = await this.makeFeedbackCommand({
      schema: AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
      command_id: commandId,
      operation: "trajectory",
      target_id: id,
      target_digest: trajectory.trajectory_digest,
      request_digest: requestDigest,
      remote: options.remote === true,
      payload: { operation: "trajectory", trajectory_id: id, rewards: normalizedRewards },
      status: "pending",
      attempts: 0,
      available_at: now,
      lease_owner: null,
      lease_until: null,
      last_error_class: null,
      result_digest: null,
      created_at: now,
      updated_at: now,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned",
    });
    await this.feedbackOutbox.save(command);
    return clone(command);
  }

  /**
   * Claim and apply pending feedback commands. Settlement receipts make the operation safe when
   * a worker crashes after applying learning but before acknowledging the outbox command.
   */
  async dispatchFeedback(options: { workerId?: string; limit?: number; leaseMs?: number; now?: number } = {}): Promise<AutonomousLearningFeedbackOutboxDispatch> {
    const workerId = boundedIdentifier("feedback outbox workerId", options.workerId ?? "learning-worker");
    const limit = options.limit ?? 64;
    const leaseMs = options.leaseMs ?? 30_000;
    const now = options.now ?? Date.now();
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX) throw new ArgumentError("feedback outbox dispatch limit is outside its bounds");
    if (!Number.isSafeInteger(leaseMs) || leaseMs < 1 || leaseMs > 10 * 60_000) throw new ArgumentError("feedback outbox dispatch leaseMs is outside its bounds");
    boundedOutboxTimestamp("feedback outbox dispatch now", now);
    const candidates = await this.feedbackOutbox.pending(limit, now);
    const rows: AutonomousLearningFeedbackOutboxDispatchRow[] = [];
    for (const candidate of candidates) {
      const claimed = await this.feedbackOutbox.claim(candidate.command_id, workerId, leaseMs, now);
      if (!claimed) {
        rows.push({ command_id: candidate.command_id, operation: candidate.operation, status: "leased_elsewhere", attempts: candidate.attempts, result_digest: candidate.result_digest, error_class: null });
        continue;
      }
      try {
        assertFeedbackOutboxCommandShape(claimed);
        const settlement = claimed.payload.operation === "single_run"
          ? await this.settleRun(claimed.payload.episode_id, claimed.payload.reward_input, { creditedReward: claimed.payload.credited_reward, remote: claimed.remote, idempotencyKey: claimed.command_id })
          : await this.settleTrajectory(claimed.payload.trajectory_id, claimed.payload.rewards, { remote: claimed.remote, idempotencyKey: claimed.command_id });
        const resultDigest = await digestJson(settlement);
        const applied = await this.feedbackOutbox.markApplied(claimed.command_id, workerId, resultDigest, now);
        rows.push({ command_id: applied.command_id, operation: applied.operation, status: "applied", attempts: applied.attempts, result_digest: applied.result_digest, error_class: null });
      } catch (error) {
        const errorClass = feedbackOutboxErrorClass(error);
        const retryable = feedbackOutboxRetryable(error);
        const failed = await this.feedbackOutbox.markFailed(claimed.command_id, workerId, errorClass, retryable, now);
        rows.push({ command_id: failed.command_id, operation: failed.operation, status: "failed", attempts: failed.attempts, result_digest: failed.result_digest, error_class: failed.last_error_class });
      }
    }
    return {
      schema: AUTONOMOUS_LEARNING_FEEDBACK_OUTBOX_SCHEMA,
      worker_id: workerId,
      inspected: candidates.length,
      applied: rows.filter((row) => row.status === "applied").length,
      failed: rows.filter((row) => row.status === "failed").length,
      leased_elsewhere: rows.filter((row) => row.status === "leased_elsewhere").length,
      rows,
      retention: PRIVATE_RETENTION,
      secret_material: "never_returned",
    };
  }

  /** Apply one settlement directly after the outbox boundary has already been claimed. */
  private async settleRunInline(episodeId: string, input: AutonomousEvaluatorRewardInput, options: { creditedReward?: number; remote?: boolean; idempotencyKey?: string; memoryStore?: AutonomousEpisodicMemoryStore } = {}): Promise<AutonomousLearningSettlement> {
    const id = boundedIdentifier("episodeId", episodeId);
    const episode = await this.episodes.load(id);
    if (!episode) throw new ArgumentError(`learning episode ${episodeId} was not found`);
    this.assertLearningAdmission(episode.domain);
    const normalizedInput = normalizeRewardInput(input);
    const creditedReward = boundedReward("credited reward", options.creditedReward ?? normalizedInput.reward);
    const idempotencyKey = boundedIdentifier("settlement idempotencyKey", options.idempotencyKey ?? `episode:${id}`);
    const requestDigest = await digestJson({ episode_digest: episode.episode_digest, input: normalizedInput, credited_reward: creditedReward, remote: options.remote === true });
    let priorReceipt: AutonomousLearningSettlementReceipt | null;
    try {
      priorReceipt = await this.loadReceipt(idempotencyKey, "single_run", id, episode.episode_digest, requestDigest);
    } catch (error) {
      if (episode.status === "settled") throw new ArgumentError(`learning episode ${id} has already been settled with conflicting reward evidence`);
      throw error;
    }
    if (priorReceipt) {
      const priorSettlement = priorReceipt.settlement as AutonomousLearningSettlement;
      if (!priorSettlement.episode || priorSettlement.episode.episode_id !== id || priorSettlement.episode.status !== "settled" || !priorSettlement.episode.settlement) throw new ArgumentError(`settlement receipt ${idempotencyKey} does not contain a settled episode projection`);
      if (episode.status === "pending") await this.episodes.markSettled(id, priorSettlement.episode.settlement);
      else if (episode.settlement?.settlement_digest !== priorSettlement.episode.settlement.settlement_digest) throw new ArgumentError(`learning episode ${id} has a conflicting settled projection`);
      return clone(priorSettlement);
    }
    if (episode.status === "settled") throw new ArgumentError(`learning episode ${episodeId} has already been settled; its settlement receipt is unavailable`);
    if (!this.agent.learner) throw new ArgumentError("learning settlement requires an AutonomousOnlineLearner on the agent");
    const creditedOutcomeDigest = await digestJson({ run_id: episode.run.run_id, outcome_digest: episode.run.outcome_digest });
    const assessment: BrainEvaluatorAssessment = {
      evaluator_id: normalizedInput.evaluator_id,
      evaluator_version: normalizedInput.evaluator_version,
      reward: creditedReward,
      passed: normalizedInput.passed,
      failed: normalizedInput.failed ?? !normalizedInput.passed,
      feedback_digest: normalizedInput.feedback_digest ?? null,
      failure_class: normalizedInput.failure_class ?? null,
      evidence_digest: normalizedInput.evidence_digest ?? null,
    };
    let nextState: BrainBanditState;
    let learningEvidence: BrainLearningEvidence | null = null;
    let remote = false;
    const armId = `${episode.run.provider}/${episode.run.model}`;
    const contextDigest = typeof episode.context_digest === "string" ? episode.context_digest : null;
    const learningContext = isObject(episode.learning_context) ? episode.learning_context as unknown as BrainBanditContext : undefined;
    if (contextDigest !== null) {
      if (!/^[0-9a-f]{64}$/.test(contextDigest)) throw new ArgumentError("learning episode context_digest is malformed");
      if (!learningContext) throw new ArgumentError("contextual learning episode is missing its bounded context");
    }
    if (options.remote === true) {
      if (!this.apiClient || typeof this.apiClient.brainOutcomeRecord !== "function") throw new ArgumentError("remote learning settlement requires an ApiClient with brainOutcomeRecord");
      const projected = projectOutcome(await this.apiClient.brainOutcomeRecord({ run: episode.run, assessment, bandit_state: this.agent.learner.snapshot(), arm_id: armId, ...(contextDigest === null ? {} : { context_digest: contextDigest, context: learningContext }), idempotency_key: idempotencyKey }));
      if (!projected.next_state || !Array.isArray(projected.next_state.arms) || !projected.learning_evidence) throw new ProviderRuntimeError("brain outcome record returned an incomplete learning projection");
      nextState = this.agent.learner.restore(projected.next_state);
      learningEvidence = projected.learning_evidence;
      remote = true;
    } else {
      nextState = await this.agent.recordEvaluatorReward(armId, creditedReward, { failed: assessment.failed, outcomeDigest: creditedOutcomeDigest, contextDigest, context: learningContext });
    }
    const settlementBase = { evaluation_digest: normalizedInput.evidence_digest ?? null, reward: normalizedInput.reward, credited_reward: creditedReward, next_generation: boundedGeneration(nextState.generation ?? 0), settled_at: Date.now() };
    const settlement: AutonomousLearningSettlementMetadata = { ...settlementBase, settlement_digest: await digestJson(settlementBase) };
    const projectedEpisode = { ...episode, status: "settled" as const, settlement };
    const memoryEvaluation = await this.recordMemoryEvaluation(episode, input, options.memoryStore ?? this.memoryStore);
    const result = { schema: AUTONOMOUS_LEARNING_EPISODE_SCHEMA, episode: projectedEpisode, assessment, next_state: clone(nextState), learning_evidence: learningEvidence, memory_evaluation: memoryEvaluation, remote, retention: PRIVATE_RETENTION } satisfies AutonomousLearningSettlement;
    await this.saveReceipt("single_run", idempotencyKey, id, episode.episode_digest, requestDigest, result);
    let settledEpisode: AutonomousLearningEpisode;
    try {
      settledEpisode = await this.episodes.markSettled(episode.episode_id, settlement);
    } catch (error) {
      const observed = await this.episodes.load(episode.episode_id);
      if (!observed || observed.status !== "settled" || observed.settlement?.settlement_digest !== settlement.settlement_digest) throw error;
      settledEpisode = observed;
    }
    return clone({ ...result, episode: settledEpisode });
  }

  /**
   * Settle one episode either directly or through the caller-owned feedback outbox. Outbox mode
   * preserves the historical return type: after dispatch it rehydrates the value-only receipt,
   * while a worker crash before dispatch leaves a durable command for `dispatchFeedback()`.
   */
  async settleRun(episodeId: string, input: AutonomousEvaluatorRewardInput, options: { creditedReward?: number; remote?: boolean; idempotencyKey?: string; memoryStore?: AutonomousEpisodicMemoryStore; outbox?: AutonomousLearningOutboxSettlementOptions } = {}): Promise<AutonomousLearningSettlement> {
    await this.assertEpisodeLearningAdmission(episodeId);
    if (!options.outbox) return this.settleRunInline(episodeId, input, options);
    if (options.memoryStore !== undefined && options.memoryStore !== this.memoryStore) throw new ArgumentError("outbox settlement cannot use a per-call memoryStore override; configure the controller memoryStore");
    const command = await this.enqueueRunSettlement(episodeId, input, { creditedReward: options.creditedReward, remote: options.remote, idempotencyKey: options.idempotencyKey });
    if (command.status === "applied") return this.settleRunInline(episodeId, input, { creditedReward: options.creditedReward, remote: options.remote, idempotencyKey: command.command_id });
    const dispatch = await this.dispatchFeedback({ workerId: options.outbox.workerId, leaseMs: options.outbox.leaseMs, limit: 1 });
    const row = dispatch.rows.find((candidate) => candidate.command_id === command.command_id);
    if (!row || row.status !== "applied") throw new ProviderRuntimeError(`feedback outbox settlement ${command.command_id} was not applied${row?.error_class ? ` (${row.error_class})` : ""}`);
    return this.settleRunInline(episodeId, input, { creditedReward: options.creditedReward, remote: options.remote, idempotencyKey: command.command_id });
  }

  /**
   * Settle the deterministic structured-response composition signal emitted by an autonomous run.
   * This is an explicit opt-in learning boundary: the signal can improve format/model adaptation,
   * but it is never presented as task correctness or evidence of an external effect.
   */
  async settleStructuredResponse(
    result: AutonomousRunResult,
    options: { creditedReward?: number; remote?: boolean; idempotencyKey?: string; memoryStore?: AutonomousEpisodicMemoryStore; outbox?: AutonomousLearningOutboxSettlementOptions } = {},
  ): Promise<AutonomousLearningSettlement> {
    const evaluation = result.response_evaluation as AutonomousDomainResponseEvaluation | null | undefined;
    if (!evaluation) throw new ArgumentError("structured-response settlement requires a completed structured domain response evaluation");
    if (!result.learning_episode_id) throw new ArgumentError("structured-response settlement requires a pending learning episode on the run result");
    return this.settleRun(result.learning_episode_id, evaluation.reward_input, options);
  }

  async prepareTrajectory(episodeIds: readonly string[], options: { trajectoryId: string; discount?: number }): Promise<AutonomousLearningTrajectory> {
    const trajectoryId = boundedIdentifier("trajectoryId", options.trajectoryId);
    if (!Array.isArray(episodeIds) || episodeIds.length < 1 || episodeIds.length > AUTONOMOUS_LEARNING_MAX_TRAJECTORY_STEPS) throw new ArgumentError("learning trajectory must contain between 1 and 32 episodes");
    const discount = boundedReward("trajectory discount", options.discount ?? 0.9);
    if (discount >= 1) throw new ArgumentError("trajectory discount must be below 1");
    const ids = episodeIds.map((id) => boundedIdentifier("trajectory episodeId", id));
    if (new Set(ids).size !== ids.length) throw new ArgumentError("learning trajectory episode IDs must be unique");
    const episodes = await Promise.all(ids.map((id) => this.episodes.load(id)));
    if (episodes.some((episode) => !episode)) throw new ArgumentError("learning trajectory references a missing episode");
    const steps = episodes.map((episode, index) => ({ index, episode_id: episode!.episode_id, arm_id: `${episode!.run.provider}/${episode!.run.model}`, context_digest: typeof episode!.context_digest === "string" ? episode!.context_digest : null, run_digest: episode!.run.outcome_digest, raw_reward: null, credited_reward: null }));
    const descriptor = { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, trajectory_id: trajectoryId, discount, steps, status: "pending" as const, settlement_digest: null, retention: PRIVATE_RETENTION, secret_material: "never_returned" as const };
    const trajectory = { ...descriptor, trajectory_digest: await digestJson(descriptor) };
    const prior = await this.trajectories.load(trajectoryId);
    if (prior) {
      if (prior.trajectory_digest !== trajectory.trajectory_digest) throw new ArgumentError(`learning trajectory ${trajectoryId} conflicts with an existing identity`);
      return clone(prior);
    }
    if (episodes.some((episode) => episode?.status !== "pending")) throw new ArgumentError("learning trajectory can only contain pending episodes");
    this.trajectories.save(trajectory);
    return clone(trajectory);
  }

  private async settleTrajectoryInline(trajectoryId: string, rewards: Record<string, AutonomousEvaluatorRewardInput>, options: { remote?: boolean; idempotencyKey?: string } = {}): Promise<AutonomousTrajectorySettlement> {
    const id = boundedIdentifier("trajectoryId", trajectoryId);
    const trajectory = await this.trajectories.load(id);
    if (!trajectory) throw new ArgumentError(`learning trajectory ${trajectoryId} was not found`);
    if (!isObject(rewards)) throw new ArgumentError("trajectory rewards must be an object keyed by episode ID");
    const expected = new Set(trajectory.steps.map((step) => step.episode_id));
    const supplied = Object.keys(rewards);
    if (supplied.length !== expected.size || supplied.some((id) => !expected.has(id))) throw new ArgumentError("trajectory rewards must cover exactly every episode");
    for (const step of trajectory.steps) normalizeRewardInput(rewards[step.episode_id]!);
    const idempotencyKey = boundedIdentifier("trajectory settlement idempotencyKey", options.idempotencyKey ?? `trajectory:${id}`);
    const normalizedRewards = Object.fromEntries(trajectory.steps.map((step) => {
      return [step.episode_id, normalizeRewardInput(rewards[step.episode_id]!)];
    }));
    const requestDigest = await digestJson({ trajectory_digest: trajectory.trajectory_digest, rewards: normalizedRewards, remote: options.remote === true });
    const priorReceipt = await this.loadReceipt(idempotencyKey, "trajectory", id, trajectory.trajectory_digest, requestDigest);
    if (priorReceipt) {
      const priorSettlement = priorReceipt.settlement as AutonomousTrajectorySettlement;
      if (!priorSettlement.trajectory || priorSettlement.trajectory.trajectory_id !== id || priorSettlement.trajectory.status !== "settled") throw new ArgumentError(`settlement receipt ${idempotencyKey} does not contain a settled trajectory projection`);
      if (trajectory.status === "pending") {
        try {
          await this.trajectories.markSettled(id, priorSettlement.trajectory.settlement_digest!);
        } catch (error) {
          const observed = await this.trajectories.load(id);
          if (!observed || observed.status !== "settled" || observed.settlement_digest !== priorSettlement.trajectory.settlement_digest) throw error;
        }
      }
      return clone(priorSettlement);
    }
    const returnToGo: Record<string, number> = {};
    let next = 0;
    for (let index = trajectory.steps.length - 1; index >= 0; index -= 1) {
      const step = trajectory.steps[index]!;
      const raw = rewards[step.episode_id]!.reward;
      boundedReward(`trajectory reward ${step.episode_id}`, raw);
      next = Number(Math.min(1, raw + trajectory.discount * next).toFixed(12));
      returnToGo[step.episode_id] = next;
    }
    const settlements: AutonomousLearningSettlement[] = [];
    const replayingSettledTrajectory = trajectory.status === "settled";
    for (const step of trajectory.steps) {
      const reward = rewards[step.episode_id]!;
      const episode = await this.episodes.load(step.episode_id);
      if (!episode) throw new ArgumentError(`learning episode ${step.episode_id} disappeared during settlement`);
      const settlement = await this.settleRun(step.episode_id, reward, { creditedReward: returnToGo[step.episode_id], remote: options.remote, idempotencyKey: await this.episodeSettlementKey(id, step.episode_id) });
      if (replayingSettledTrajectory || episode.status === "pending") settlements.push(settlement);
    }
    const settlementDigest = await digestJson({ trajectory_digest: trajectory.trajectory_digest, return_to_go: returnToGo, settlement_digests: settlements.map((settlement) => settlement.episode.settlement?.settlement_digest ?? null) });
    const projectedTrajectory = { ...trajectory, status: "settled" as const, settlement_digest: settlementDigest };
    const result = { schema: AUTONOMOUS_LEARNING_TRAJECTORY_SCHEMA, trajectory: projectedTrajectory, settlements, return_to_go: returnToGo, retention: PRIVATE_RETENTION } satisfies AutonomousTrajectorySettlement;
    await this.saveReceipt("trajectory", idempotencyKey, id, trajectory.trajectory_digest, requestDigest, result);
    if (replayingSettledTrajectory) return clone({ ...result, trajectory });
    let settledTrajectory: AutonomousLearningTrajectory;
    try {
      settledTrajectory = await this.trajectories.markSettled(trajectory.trajectory_id, settlementDigest);
    } catch (error) {
      const observed = await this.trajectories.load(trajectory.trajectory_id);
      if (!observed || observed.status !== "settled" || observed.settlement_digest !== settlementDigest) throw error;
      settledTrajectory = observed;
    }
    return clone({ ...result, trajectory: settledTrajectory });
  }

  /** Settle a trajectory directly or through the durable feedback outbox boundary. */
  async settleTrajectory(trajectoryId: string, rewards: Record<string, AutonomousEvaluatorRewardInput>, options: { remote?: boolean; idempotencyKey?: string; outbox?: AutonomousLearningOutboxSettlementOptions } = {}): Promise<AutonomousTrajectorySettlement> {
    await this.assertTrajectoryLearningAdmission(trajectoryId);
    if (!options.outbox) return this.settleTrajectoryInline(trajectoryId, rewards, options);
    const command = await this.enqueueTrajectorySettlement(trajectoryId, rewards, { remote: options.remote, idempotencyKey: options.idempotencyKey });
    if (command.status === "applied") return this.settleTrajectoryInline(trajectoryId, rewards, { remote: options.remote, idempotencyKey: command.command_id });
    const dispatch = await this.dispatchFeedback({ workerId: options.outbox.workerId, leaseMs: options.outbox.leaseMs, limit: 1 });
    const row = dispatch.rows.find((candidate) => candidate.command_id === command.command_id);
    if (!row || row.status !== "applied") throw new ProviderRuntimeError(`feedback outbox trajectory settlement ${command.command_id} was not applied${row?.error_class ? ` (${row.error_class})` : ""}`);
    return this.settleTrajectoryInline(trajectoryId, rewards, { remote: options.remote, idempotencyKey: command.command_id });
  }
}
