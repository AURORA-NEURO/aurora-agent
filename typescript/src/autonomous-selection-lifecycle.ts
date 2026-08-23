import { ArgumentError, isObject } from "./errors.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { AutonomousSelectionPromotionReport } from "./autonomous-selection-promotion.js";
import type { JsonObject } from "./types.js";

/** Stable schema for the digest-only adaptive-selection lifecycle state. */
export const AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA = "bioprism-typescript-autonomous-selection-lifecycle/0.1" as const;
/** Stable schema for a persisted adaptive-selection lifecycle snapshot. */
export const AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA = "bioprism-typescript-autonomous-selection-lifecycle-store/0.1" as const;

export const MAX_AUTONOMOUS_SELECTION_LIFECYCLE_REASON_BYTES = 2_000;
export const MAX_AUTONOMOUS_SELECTION_LIFECYCLE_BYTES = 128_000;
export const MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION = 1_000_000;

export type AutonomousSelectionLifecycleStatus = "uninitialized" | "held" | "admitted" | "rolled_back";
export type AutonomousSelectionLifecycleDecision = "none" | "admit" | "hold" | "rollback";

/** Restart-safe state for whether learned model selection may influence invocation. */
export interface AutonomousSelectionLifecycleState extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA;
  lifecycle_id: string;
  status: AutonomousSelectionLifecycleStatus;
  revision: number;
  generation: number;
  rollback_count: number;
  last_decision: AutonomousSelectionLifecycleDecision;
  promotion_digest: string | null;
  active_promotion_digest: string | null;
  source_report_digest: string | null;
  policy_digest: string | null;
  domain_decision_digest: string | null;
  last_reason: string | null;
  created_at: number;
  updated_at: number;
  state_digest: string;
  retention: "metadata_only;promotion_and_domain_digests_only";
  authorization: "admitted_selection_only;does_not_authorize_provider_or_tools";
  secret_material: "never_returned";
}

export interface AutonomousSelectionLifecycleSnapshot extends JsonObject {
  schema: typeof AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA;
  state: AutonomousSelectionLifecycleState;
  state_digest: string;
  snapshot_digest: string;
  retention: "metadata_only_hash_bound";
  secret_material: "never_returned";
}

export interface AutonomousSelectionLifecycleStore {
  load(): Promise<AutonomousSelectionLifecycleState | null> | AutonomousSelectionLifecycleState | null;
  save(state: AutonomousSelectionLifecycleState): Promise<void> | void;
  snapshot(): Promise<AutonomousSelectionLifecycleSnapshot> | AutonomousSelectionLifecycleSnapshot;
  restore(snapshot: AutonomousSelectionSnapshotLike): Promise<void> | void;
}

type AutonomousSelectionSnapshotLike = AutonomousSelectionLifecycleSnapshot;

const PROMOTION_SCHEMA = "bioprism-typescript-autonomous-selection-promotion/0.1";
const PROMOTION_DOMAIN_SCHEMA = "bioprism-typescript-autonomous-selection-promotion-domain/0.1";
const PROMOTION_DOMAINS = ["coding", "browser", "data", "science", "biomedical", "neuroscience", "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation"] as const;
const PROMOTION_EXECUTION = "gate_only;does_not_mutate_learner_or_invoke_provider";
const PROMOTION_RETENTION = "metadata_only;selection_metrics_and_digests";
const RETENTION = "metadata_only;promotion_and_domain_digests_only" as const;
const AUTHORIZATION = "admitted_selection_only;does_not_authorize_provider_or_tools" as const;
const SECRET_MATERIAL = "never_returned" as const;
const STORE_RETENTION = "metadata_only_hash_bound" as const;
const DIGEST = /^[0-9a-f]{64}$/;
const IDENTIFIER = /^[A-Za-z0-9_.:-]+$/;

function fail(message: string): never {
  throw new ArgumentError(`autonomous selection lifecycle ${message}`);
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function boundedIdentifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.length > maximum || !IDENTIFIER.test(value)) fail(`${name} is invalid`);
  return value;
}

function boundedDigest(name: string, value: unknown, allowNull = false): string | null {
  if (allowNull && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || !DIGEST.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function boundedCount(name: string, value: unknown, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > maximum) fail(`${name} is outside its bound`);
  return value as number;
}

function boundedReason(value: unknown): string | null {
  if (value === null) return null;
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > MAX_AUTONOMOUS_SELECTION_LIFECYCLE_REASON_BYTES) fail("last_reason is invalid");
  return value;
}

function statePayload(state: Omit<AutonomousSelectionLifecycleState, "state_digest" | "retention" | "authorization" | "secret_material">): Omit<AutonomousSelectionLifecycleState, "state_digest" | "retention" | "authorization" | "secret_material"> {
  const { state_digest: _stateDigest, retention: _retention, authorization: _authorization, secret_material: _secretMaterial, ...payload } = state as AutonomousSelectionLifecycleState;
  return payload as Omit<AutonomousSelectionLifecycleState, "state_digest" | "retention" | "authorization" | "secret_material">;
}

function seal(raw: Omit<AutonomousSelectionLifecycleState, "state_digest" | "retention" | "authorization" | "secret_material">): AutonomousSelectionLifecycleState {
  const payload = statePayload(raw);
  const result = {
    ...payload,
    state_digest: digestJsonSync(payload),
    retention: RETENTION,
    authorization: AUTHORIZATION,
    secret_material: SECRET_MATERIAL,
  } as AutonomousSelectionLifecycleState;
  return validateAutonomousSelectionLifecycleState(result);
}

function promotionProjection(report: AutonomousSelectionPromotionReport): { promotionDigest: string; sourceReportDigest: string; policyDigest: string; domainDecisionDigest: string; reason: string | null } {
  if (!isObject(report) || report.schema !== PROMOTION_SCHEMA) fail("promotion report schema is invalid");
  if (typeof report.promotion_digest !== "string" || !DIGEST.test(report.promotion_digest)) fail("promotion report digest is invalid");
  const { promotion_digest: _digest, ...body } = report;
  if (digestJsonSync(body) !== report.promotion_digest) fail("promotion report digest does not match its contents");
  if (report.decision !== "admit" && report.decision !== "hold") fail("promotion report decision is invalid");
  if (!isObject(report.policy) || report.execution !== PROMOTION_EXECUTION || report.retention !== PROMOTION_RETENTION || report.secret_material !== "never_returned") fail("promotion report retention posture is invalid");
  if (!Array.isArray(report.reasons) || report.reasons.some((reason) => typeof reason !== "string" || !reason.trim())) fail("promotion report reasons are malformed");
  if (!Array.isArray(report.domains) || report.domains.length !== PROMOTION_DOMAINS.length) fail("promotion report must contain every autonomous domain");
  for (const [index, row] of report.domains.entries()) {
    if (!isObject(row) || row.schema !== PROMOTION_DOMAIN_SCHEMA || row.domain !== PROMOTION_DOMAINS[index] || (row.decision !== "admit" && row.decision !== "hold" && row.decision !== "not_required") || !Array.isArray(row.reasons) || row.reasons.some((reason) => typeof reason !== "string" || !reason.trim())) {
      fail("promotion report domain projection is malformed");
    }
    for (const metric of ["case_count", "evaluated_count", "evaluated_coverage", "oracle_agreement_count", "oracle_agreement_rate", "mean_regret", "abstention_rate", "selected_reward_missing_rate", "no_eligible_model_rate", "no_counterfactual_reward_rate"]) {
      const value = row[metric];
      if (value !== null && (typeof value !== "number" || !Number.isFinite(value))) fail("promotion report domain metrics are malformed");
    }
  }
  const sourceReportDigest = boundedDigest("promotion source report digest", report.source_report_digest);
  const policyDigest = digestJsonSync(report.policy);
  const domainDecisionDigest = digestJsonSync(report.domains.map((row) => ({
    domain: row.domain,
    decision: row.decision,
    reasons: row.reasons,
  })));
  const reasons = Array.isArray(report.reasons) ? report.reasons.filter((reason): reason is string => typeof reason === "string") : [];
  const reason = reasons.length ? reasons.join("; ") : report.decision === "hold" ? "selection promotion held" : null;
  return { promotionDigest: report.promotion_digest, sourceReportDigest: sourceReportDigest!, policyDigest, domainDecisionDigest, reason };
}

/** Validate a lifecycle state before it crosses a process or persistence boundary. */
export function validateAutonomousSelectionLifecycleState(value: unknown): AutonomousSelectionLifecycleState {
  if (!isObject(value)) fail("state must be an object");
  const state = value as unknown as AutonomousSelectionLifecycleState;
  if (state.schema !== AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA) fail("state schema is invalid");
  boundedIdentifier("state lifecycle_id", state.lifecycle_id);
  if (!(["uninitialized", "held", "admitted", "rolled_back"] as readonly string[]).includes(state.status)) fail("state status is invalid");
  boundedCount("state revision", state.revision, MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION);
  boundedCount("state generation", state.generation, MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION);
  boundedCount("state rollback_count", state.rollback_count, MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION);
  if (!(["none", "admit", "hold", "rollback"] as readonly string[]).includes(state.last_decision)) fail("state last_decision is invalid");
  boundedDigest("state promotion_digest", state.promotion_digest, true);
  boundedDigest("state active_promotion_digest", state.active_promotion_digest, true);
  boundedDigest("state source_report_digest", state.source_report_digest, true);
  boundedDigest("state policy_digest", state.policy_digest, true);
  boundedDigest("state domain_decision_digest", state.domain_decision_digest, true);
  boundedReason(state.last_reason);
  if (typeof state.created_at !== "number" || !Number.isFinite(state.created_at) || state.created_at < 0 || typeof state.updated_at !== "number" || !Number.isFinite(state.updated_at) || state.updated_at < state.created_at) fail("state timestamps are invalid");
  if (state.status === "admitted" && state.active_promotion_digest === null) fail("admitted state must have an active promotion digest");
  if (state.status !== "admitted" && state.active_promotion_digest !== null) fail("non-admitted state cannot have an active promotion digest");
  if (state.retention !== RETENTION || state.authorization !== AUTHORIZATION || state.secret_material !== SECRET_MATERIAL) fail("state retention markers are invalid");
  const payload = statePayload(state);
  if (state.state_digest !== digestJsonSync(payload)) fail("state digest does not match its contents");
  if (new TextEncoder().encode(canonicalJson(state)).byteLength > MAX_AUTONOMOUS_SELECTION_LIFECYCLE_BYTES) fail("state exceeds its byte bound");
  return clone(state);
}

export function validateAutonomousSelectionLifecycleSnapshot(value: unknown): AutonomousSelectionLifecycleSnapshot {
  if (!isObject(value)) fail("snapshot must be an object");
  const snapshot = value as unknown as AutonomousSelectionLifecycleSnapshot;
  if (snapshot.schema !== AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA || snapshot.retention !== STORE_RETENTION || snapshot.secret_material !== SECRET_MATERIAL) fail("snapshot retention markers are invalid");
  const state = validateAutonomousSelectionLifecycleState(snapshot.state);
  if (snapshot.state_digest !== state.state_digest || typeof snapshot.snapshot_digest !== "string" || !DIGEST.test(snapshot.snapshot_digest)) fail("snapshot digests are invalid");
  const body = { schema: snapshot.schema, state, state_digest: snapshot.state_digest, retention: snapshot.retention, secret_material: snapshot.secret_material };
  if (snapshot.snapshot_digest !== digestJsonSync(body)) fail("snapshot digest does not match its contents");
  return clone({ ...snapshot, state });
}

function initialState(lifecycleId: string, now: number): AutonomousSelectionLifecycleState {
  return seal({
    schema: AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA,
    lifecycle_id: lifecycleId,
    status: "uninitialized",
    revision: 0,
    generation: 0,
    rollback_count: 0,
    last_decision: "none",
    promotion_digest: null,
    active_promotion_digest: null,
    source_report_digest: null,
    policy_digest: null,
    domain_decision_digest: null,
    last_reason: null,
    created_at: now,
    updated_at: now,
  });
}

/**
 * State machine that turns replay admission into an explicit, restart-safe selector authority.
 * It stores no learner parameters, prompts, task text, rewards, candidates, or credentials.
 */
export class AutonomousSelectionPromotionLifecycle {
  private stateValue: AutonomousSelectionLifecycleState;
  private readonly clock: () => number;

  constructor(options: { lifecycleId?: string; state?: AutonomousSelectionLifecycleState | JsonObject; clock?: () => number } = {}) {
    this.clock = options.clock ?? (() => Date.now());
    if (typeof this.clock !== "function") fail("clock must be callable");
    const now = this.now();
    this.stateValue = options.state === undefined
      ? initialState(boundedIdentifier("lifecycle_id", options.lifecycleId ?? `selection-lifecycle-${Math.random().toString(36).slice(2)}`), now)
      : validateAutonomousSelectionLifecycleState(options.state);
  }

  get state(): AutonomousSelectionLifecycleState { return clone(this.stateValue); }
  isAdmitted(): boolean { return this.stateValue.status === "admitted" && this.stateValue.active_promotion_digest !== null; }

  apply(report: AutonomousSelectionPromotionReport): AutonomousSelectionLifecycleState {
    const projection = promotionProjection(report);
    const wasAdmitted = this.isAdmitted();
    const status: AutonomousSelectionLifecycleStatus = report.decision === "admit" ? "admitted" : wasAdmitted ? "rolled_back" : "held";
    this.commit({
      status,
      generation: report.decision === "admit" ? this.stateValue.generation + 1 : this.stateValue.generation,
      rollback_count: report.decision === "hold" && wasAdmitted ? this.stateValue.rollback_count + 1 : this.stateValue.rollback_count,
      last_decision: report.decision,
      promotion_digest: projection.promotionDigest,
      active_promotion_digest: report.decision === "admit" ? projection.promotionDigest : null,
      source_report_digest: projection.sourceReportDigest,
      policy_digest: projection.policyDigest,
      domain_decision_digest: projection.domainDecisionDigest,
      last_reason: projection.reason,
    });
    return this.state;
  }

  rollback(reason = "selection_promotion_rollback"): AutonomousSelectionLifecycleState {
    if (typeof reason !== "string" || !reason.trim()) fail("rollback reason must be non-empty");
    if (!this.isAdmitted()) return this.state;
    this.commit({ status: "rolled_back", rollback_count: this.stateValue.rollback_count + 1, last_decision: "rollback", active_promotion_digest: null, last_reason: reason });
    return this.state;
  }

  restore(raw: AutonomousSelectionLifecycleState): AutonomousSelectionLifecycleState {
    const next = validateAutonomousSelectionLifecycleState(raw);
    if (next.lifecycle_id !== this.stateValue.lifecycle_id && this.stateValue.revision > 0) fail("lifecycle identity cannot change after initialization");
    if (next.revision < this.stateValue.revision) fail("lifecycle revision cannot move backwards");
    this.stateValue = clone(next);
    return this.state;
  }

  private now(): number {
    const value = this.clock();
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0) fail("clock must return a finite non-negative timestamp");
    return value;
  }

  private commit(changes: Partial<Omit<AutonomousSelectionLifecycleState, "schema" | "state_digest" | "retention" | "authorization" | "secret_material">>): void {
    const changed = Object.entries(changes).some(([key, value]) => (this.stateValue as unknown as Record<string, unknown>)[key] !== value);
    if (!changed) return;
    const next = { ...this.stateValue, ...changes, revision: this.stateValue.revision + 1, updated_at: Math.max(this.now(), this.stateValue.updated_at) };
    this.stateValue = seal(next);
  }
}

/** Small in-memory reference store with revision and digest validation for caller integration. */
export class AutonomousSelectionPromotionLifecycleStore implements AutonomousSelectionLifecycleStore {
  private value: AutonomousSelectionLifecycleState | null = null;

  async load(): Promise<AutonomousSelectionLifecycleState | null> { return clone(this.value); }

  async save(raw: AutonomousSelectionLifecycleState): Promise<void> {
    const state = validateAutonomousSelectionLifecycleState(raw);
    if (this.value && state.state_digest !== this.value.state_digest && state.revision !== this.value.revision + 1) fail("lifecycle revision continuity check failed");
    this.value = clone(state);
  }

  async snapshot(): Promise<AutonomousSelectionLifecycleSnapshot> {
    const state = this.value ?? initialState("selection-lifecycle-empty", 0);
    const body = { schema: AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA, state, state_digest: state.state_digest, retention: STORE_RETENTION, secret_material: SECRET_MATERIAL } as const;
    return validateAutonomousSelectionLifecycleSnapshot({ ...body, snapshot_digest: digestJsonSync(body) });
  }

  async restore(raw: AutonomousSelectionSnapshotLike): Promise<void> {
    this.value = clone(validateAutonomousSelectionLifecycleSnapshot(raw).state);
  }
}
