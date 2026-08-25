import { ArgumentError, isObject } from "./errors.js";
import {
  AutonomousGoalControlLoop,
  type AutonomousGoalControlLoopResult,
} from "./autonomous-goal-control-loop.js";
import {
  AutonomousGoalControlLoopPersistenceCoordinator,
  validateAutonomousGoalControlLoopSnapshot,
  type AutonomousGoalControlLoopCheckpoint,
} from "./autonomous-goal-control-persistence.js";
import {
  AutonomousGoalWorkerJournalPersistenceCoordinator,
  type AutonomousGoalWorkerJournalPhase,
} from "./autonomous-goal-worker-journal.js";
import { InMemoryAutonomousGoalLedger } from "./autonomous-goals.js";
import { canonicalJson, digestJsonSync } from "./tooling.js";
import type { JsonObject } from "./types.js";

/** Ordered, metadata-only restart orchestration for long-horizon goal execution. */
export const AUTONOMOUS_GOAL_RECOVERY_SCHEMA = "bioprism-autonomous-goal-recovery/0.1" as const;
export const AUTONOMOUS_GOAL_RECOVERY_RETENTION = "metadata_only_goal_recovery;tasks_prompts_parameters_credentials_provider_values_and_results_not_retained" as const;
export const AUTONOMOUS_GOAL_RECOVERY_MAX_GOALS = 16_384;
export const AUTONOMOUS_GOAL_RECOVERY_MAX_REPORT_BYTES = 2_000_000;

export type AutonomousGoalRecoveryStatus = "fresh" | "restored" | "recovered";

export interface AutonomousGoalRecoveryEntry extends JsonObject {
  goal_id: string;
  from_phase: Extract<AutonomousGoalWorkerJournalPhase, "claimed" | "dispatch_started">;
  goal_status: "paused" | "blocked";
  outcome_digest: string;
}

export interface AutonomousGoalRecoveryReport extends JsonObject {
  schema: typeof AUTONOMOUS_GOAL_RECOVERY_SCHEMA;
  status: AutonomousGoalRecoveryStatus;
  active_count_before_recovery: number;
  recovered: AutonomousGoalRecoveryEntry[];
  recovery_digest: string;
  journal_snapshot_digest: string | null;
  journal_head_digest: string;
  control_loop_snapshot_digest: string | null;
  control_loop_generation: number;
  resume_snapshot: AutonomousGoalControlLoopCheckpoint | null;
  ready_to_resume: boolean;
  requires_external_reconciliation: boolean;
  retention: typeof AUTONOMOUS_GOAL_RECOVERY_RETENTION;
  secret_material: "never_returned";
  report_digest: string;
}

type GoalRecoveryResumeOptions = Omit<Parameters<AutonomousGoalControlLoop["run"]>[0], "resume_snapshot">;

function fail(message: string): never {
  throw new ArgumentError(`autonomous goal recovery ${message}`);
}

function digest(name: string, value: unknown, allowNull = false): string | null {
  if (value === null && allowNull) return null;
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/.test(value)) fail(`${name} must be a lowercase SHA-256 digest`);
  return value;
}

function integer(name: string, value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`${name} is outside its integer bounds`);
  return value;
}

function identifier(name: string, value: unknown, maximum = 256): string {
  if (typeof value !== "string" || !value.trim() || value.includes("\u0000") || new TextEncoder().encode(value).byteLength > maximum) fail(`${name} is outside its bounded identifier contract`);
  return value.trim();
}

function exactKeys(name: string, value: Record<string, unknown>, expected: readonly string[]): void {
  const allowed = new Set(expected);
  if (Object.keys(value).some((key) => !allowed.has(key)) || expected.some((key) => !(key in value))) fail(`${name} contains unsupported or missing fields`);
}

function normalizeEntry(value: unknown, index: number): AutonomousGoalRecoveryEntry {
  if (!isObject(value)) fail(`recovered entry ${index} is malformed`);
  exactKeys(`recovered entry ${index}`, value, ["goal_id", "from_phase", "goal_status", "outcome_digest"]);
  if (value.from_phase !== "claimed" && value.from_phase !== "dispatch_started") fail(`recovered entry ${index} phase is invalid`);
  if (value.goal_status !== "paused" && value.goal_status !== "blocked") fail(`recovered entry ${index} status is invalid`);
  return {
    goal_id: identifier(`recovered entry ${index} goal_id`, value.goal_id),
    from_phase: value.from_phase,
    goal_status: value.goal_status,
    outcome_digest: digest(`recovered entry ${index} outcome_digest`, value.outcome_digest)!,
  };
}

function reportBody(value: unknown, requireDigest: boolean): AutonomousGoalRecoveryReport {
  if (!isObject(value)) fail("report must be an object");
  const required = [
    "schema", "status", "active_count_before_recovery", "recovered", "recovery_digest", "journal_snapshot_digest",
    "journal_head_digest", "control_loop_snapshot_digest", "control_loop_generation", "resume_snapshot", "ready_to_resume",
    "requires_external_reconciliation", "retention", "secret_material",
  ];
  const allowed = new Set([...required, "report_digest"]);
  if (Object.keys(value).some((key) => !allowed.has(key)) || required.some((key) => !(key in value)) || (requireDigest && !("report_digest" in value))) fail("report contains unsupported or missing fields");
  if (value.schema !== AUTONOMOUS_GOAL_RECOVERY_SCHEMA || value.retention !== AUTONOMOUS_GOAL_RECOVERY_RETENTION || value.secret_material !== "never_returned") fail("report markers are invalid");
  if (value.status !== "fresh" && value.status !== "restored" && value.status !== "recovered") fail("report status is invalid");
  const activeCount = integer("active_count_before_recovery", value.active_count_before_recovery, 0, AUTONOMOUS_GOAL_RECOVERY_MAX_GOALS);
  if (!Array.isArray(value.recovered) || value.recovered.length > AUTONOMOUS_GOAL_RECOVERY_MAX_GOALS) fail("recovered entries are outside their bounds");
  const recovered = value.recovered.map(normalizeEntry);
  if (recovered.length !== activeCount) fail("recovered entries do not account for every active journal boundary");
  const goalIds = recovered.map((entry) => entry.goal_id);
  if (new Set(goalIds).size !== goalIds.length) fail("recovered entries contain duplicate goals");
  const recoveryDigest = digest("recovery_digest", value.recovery_digest)!;
  if (recoveryDigest !== digestJsonSync(recovered)) fail("recovery digest does not match recovered entries");
  const journalSnapshotDigest = digest("journal_snapshot_digest", value.journal_snapshot_digest, true);
  const journalHeadDigest = value.journal_head_digest === "" ? "" : digest("journal_head_digest", value.journal_head_digest)!;
  const controlSnapshotDigest = digest("control_loop_snapshot_digest", value.control_loop_snapshot_digest, true);
  const controlGeneration = integer("control_loop_generation", value.control_loop_generation, 0, 2_147_483_647);
  const resumeSnapshot = value.resume_snapshot === null ? null : validateAutonomousGoalControlLoopSnapshot(value.resume_snapshot as AutonomousGoalControlLoopCheckpoint);
  if ((resumeSnapshot?.snapshot_digest ?? null) !== controlSnapshotDigest || (resumeSnapshot?.generation ?? 0) !== controlGeneration) fail("control-loop snapshot metadata is inconsistent");
  if (typeof value.ready_to_resume !== "boolean" || value.ready_to_resume !== true) fail("report is not ready to resume");
  if (typeof value.requires_external_reconciliation !== "boolean") fail("external reconciliation marker is invalid");
  const requiresReconciliation = recovered.some((entry) => entry.from_phase === "dispatch_started");
  if (value.requires_external_reconciliation !== requiresReconciliation) fail("external reconciliation marker is inconsistent");
  const body = {
    schema: value.schema,
    status: value.status,
    active_count_before_recovery: activeCount,
    recovered,
    recovery_digest: recoveryDigest,
    journal_snapshot_digest: journalSnapshotDigest,
    journal_head_digest: journalHeadDigest,
    control_loop_snapshot_digest: controlSnapshotDigest,
    control_loop_generation: controlGeneration,
    resume_snapshot: resumeSnapshot,
    ready_to_resume: value.ready_to_resume,
    requires_external_reconciliation: requiresReconciliation,
    retention: value.retention,
    secret_material: value.secret_material,
  } satisfies Omit<AutonomousGoalRecoveryReport, "report_digest">;
  if (requireDigest && digest("report_digest", value.report_digest) !== digestJsonSync(body)) fail("report digest does not match its content");
  const normalized = {
    ...body,
    status: value.status as AutonomousGoalRecoveryStatus,
    retention: value.retention as typeof AUTONOMOUS_GOAL_RECOVERY_RETENTION,
    secret_material: "never_returned" as const,
  } satisfies Omit<AutonomousGoalRecoveryReport, "report_digest">;
  return { ...normalized, report_digest: digestJsonSync(normalized) };
}

export function validateAutonomousGoalRecoveryReport(value: unknown): AutonomousGoalRecoveryReport {
  const normalized = reportBody(value, true);
  if (new TextEncoder().encode(canonicalJson(normalized)).byteLength > AUTONOMOUS_GOAL_RECOVERY_MAX_REPORT_BYTES) fail("report exceeds its bounded size");
  return structuredClone(normalized);
}

/**
 * Restore the journal before the loop checkpoint, reconcile every uncertain boundary, and only
 * then expose a resume snapshot. This prevents a restarted loop from admitting work while an
 * earlier provider/effect invocation is still ambiguous. The caller still owns the ledger,
 * stores, task rehydrator, provider credentials, external reconciliation, and evaluator truth.
 */
export class AutonomousGoalRecoveryCoordinator {
  private reportValue: AutonomousGoalRecoveryReport | null = null;

  constructor(
    readonly ledger: InMemoryAutonomousGoalLedger,
    readonly journal: AutonomousGoalWorkerJournalPersistenceCoordinator,
    readonly control_loop: AutonomousGoalControlLoopPersistenceCoordinator,
  ) {
    if (!(ledger instanceof InMemoryAutonomousGoalLedger)) fail("ledger must be an InMemoryAutonomousGoalLedger");
    if (!(journal instanceof AutonomousGoalWorkerJournalPersistenceCoordinator)) fail("journal coordinator is invalid");
    if (!(control_loop instanceof AutonomousGoalControlLoopPersistenceCoordinator)) fail("control-loop coordinator is invalid");
  }

  get report(): AutonomousGoalRecoveryReport | null {
    return this.reportValue === null ? null : structuredClone(this.reportValue);
  }

  async restore(options: { now_ns?: number } = {}): Promise<AutonomousGoalRecoveryReport> {
    if (options.now_ns !== undefined) integer("now_ns", options.now_ns);
    // The order here is a correctness boundary. A control snapshot can be stale by one cycle,
    // but an active dispatch must never survive restart without a durable reconciliation event.
    const journalSnapshotBefore = await this.journal.restore();
    const activeBefore = this.journal.journal.active();
    let recovered: AutonomousGoalRecoveryEntry[] = [];
    let journalSnapshot = journalSnapshotBefore;
    if (activeBefore.length > 0) {
      const recovery = this.journal.journal.recover(this.ledger, { now_ns: options.now_ns });
      if (!Array.isArray(recovery.recovered)) fail("journal recovery returned malformed entries");
      recovered = recovery.recovered.map((entry, index) => normalizeEntry(entry, index));
      // Persist reconciliation before reading the control-loop checkpoint. A crash after this
      // write is safe: the next restore sees only terminal reconciled journal entries.
      journalSnapshot = await this.journal.flush();
    }
    const controlSnapshot = await this.control_loop.restore();
    const status: AutonomousGoalRecoveryStatus = recovered.length > 0 ? "recovered" : (journalSnapshotBefore !== null || controlSnapshot !== null ? "restored" : "fresh");
    const body = {
      schema: AUTONOMOUS_GOAL_RECOVERY_SCHEMA,
      status,
      active_count_before_recovery: activeBefore.length,
      recovered,
      recovery_digest: digestJsonSync(recovered),
      journal_snapshot_digest: journalSnapshot?.snapshot_digest ?? null,
      journal_head_digest: journalSnapshot?.head_digest ?? this.journal.journal.head_digest,
      control_loop_snapshot_digest: controlSnapshot?.snapshot_digest ?? null,
      control_loop_generation: controlSnapshot?.generation ?? 0,
      resume_snapshot: controlSnapshot,
      ready_to_resume: this.journal.journal.active().length === 0,
      requires_external_reconciliation: recovered.some((entry) => entry.from_phase === "dispatch_started"),
      retention: AUTONOMOUS_GOAL_RECOVERY_RETENTION,
      secret_material: "never_returned" as const,
    } satisfies Omit<AutonomousGoalRecoveryReport, "report_digest">;
    this.reportValue = reportBody({ ...body, report_digest: digestJsonSync(body) }, true);
    return structuredClone(this.reportValue);
  }

  assertReadyForResume(): AutonomousGoalRecoveryReport {
    if (this.reportValue === null) fail("restore must complete before resume");
    if (this.journal.journal.active().length > 0) fail("journal still contains active boundaries");
    return structuredClone(this.reportValue);
  }

  async resume(loop: AutonomousGoalControlLoop, options: GoalRecoveryResumeOptions = {}): Promise<AutonomousGoalControlLoopResult> {
    if (!(loop instanceof AutonomousGoalControlLoop)) fail("resume requires an AutonomousGoalControlLoop");
    const report = this.assertReadyForResume();
    const safeOptions = { ...options } as GoalRecoveryResumeOptions & { resume_snapshot?: unknown };
    if (Object.prototype.hasOwnProperty.call(safeOptions, "resume_snapshot")) fail("resume_snapshot is owned by the recovery coordinator");
    return loop.run({ ...safeOptions, resume_snapshot: report.resume_snapshot });
  }

  /**
   * Persist one completed loop checkpoint with the journal first. If control persistence fails,
   * the next restart still observes the settled journal and will not trust a stale loop cursor.
   */
  async checkpoint(snapshot: AutonomousGoalControlLoopCheckpoint): Promise<AutonomousGoalControlLoopCheckpoint> {
    const prior = this.assertReadyForResume();
    const journalSnapshot = await this.journal.flush();
    const controlSnapshot = await this.control_loop.flush(snapshot);
    const { report_digest: _reportDigest, ...priorBody } = prior;
    const body = {
      ...priorBody,
      journal_snapshot_digest: journalSnapshot.snapshot_digest,
      journal_head_digest: journalSnapshot.head_digest,
      control_loop_snapshot_digest: controlSnapshot.snapshot_digest,
      control_loop_generation: controlSnapshot.generation,
      resume_snapshot: controlSnapshot,
    } as Omit<AutonomousGoalRecoveryReport, "report_digest">;
    this.reportValue = reportBody({ ...body, report_digest: digestJsonSync(body) }, true);
    return structuredClone(controlSnapshot);
  }
}
