import { ArgumentError } from "./errors.js";
import {
  AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX,
  AutonomousLearningController,
  type AutonomousLearningFeedbackOutboxDispatchRow,
} from "./autonomous-learning.js";
import type { JsonObject } from "./types.js";

export const AUTONOMOUS_LEARNING_FEEDBACK_WORKER_SCHEMA = "bioprism-typescript-autonomous-learning-feedback-worker/0.1" as const;
export const MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_ROUNDS = 64;
export const MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_COMMANDS = AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX;
export const MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_LEASE_MS = 10 * 60_000;

export type AutonomousLearningFeedbackWorkerStatus = "drained" | "bounded" | "failed" | "leased_elsewhere";

export interface AutonomousLearningFeedbackWorkerRun extends JsonObject {
  schema: typeof AUTONOMOUS_LEARNING_FEEDBACK_WORKER_SCHEMA;
  worker_id: string;
  status: AutonomousLearningFeedbackWorkerStatus;
  rounds: number;
  inspected: number;
  applied: number;
  failed: number;
  leased_elsewhere: number;
  remaining: number;
  rows: AutonomousLearningFeedbackOutboxDispatchRow[];
  retention: "value_only_feedback_commands_and_digests_no_task_or_provider_values";
  secret_material: "never_returned";
}

function boundedIdentifier(name: string, value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.length > 256 || value.includes("\u0000") || !/^[A-Za-z0-9_.:+-]+$/.test(value)) throw new ArgumentError(`${name} is outside its bounded identifier contract`);
  return value;
}

function boundedInteger(name: string, value: unknown, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || (value as number) < minimum || (value as number) > maximum) throw new ArgumentError(`${name} must be an integer in [${minimum}, ${maximum}]`);
  return value as number;
}

function nowValue(value: unknown): number {
  const now = value === undefined ? Date.now() : value;
  if (!Number.isSafeInteger(now) || (now as number) < 0) throw new ArgumentError("learning feedback worker now must be a non-negative safe integer");
  return now as number;
}

/**
 * Bounded worker loop for the caller-owned learning feedback outbox.
 *
 * The worker stores no task, prompt, provider response, credential, or evidence body. It only
 * claims value-only commands and delegates application to AutonomousLearningController, whose
 * settlement receipts make a crash after learner mutation safe to replay.
 */
export class AutonomousLearningFeedbackWorker {
  constructor(readonly controller: AutonomousLearningController) {
    if (!(controller instanceof AutonomousLearningController)) throw new ArgumentError("learning feedback worker requires an AutonomousLearningController");
  }

  async run(options: {
    workerId?: string;
    limit?: number;
    maxRounds?: number;
    maxCommands?: number;
    leaseMs?: number;
    now?: number;
  } = {}): Promise<AutonomousLearningFeedbackWorkerRun> {
    const workerId = boundedIdentifier("learning feedback worker workerId", options.workerId ?? "learning-feedback-worker");
    const limit = boundedInteger("learning feedback worker limit", options.limit ?? 64, 1, AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX);
    const maxRounds = boundedInteger("learning feedback worker maxRounds", options.maxRounds ?? 1, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_ROUNDS);
    const maxCommands = boundedInteger("learning feedback worker maxCommands", options.maxCommands ?? limit, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_COMMANDS);
    const leaseMs = boundedInteger("learning feedback worker leaseMs", options.leaseMs ?? 30_000, 1, MAX_AUTONOMOUS_LEARNING_FEEDBACK_WORKER_LEASE_MS);
    const fixedNow = options.now === undefined ? null : nowValue(options.now);
    const rows: AutonomousLearningFeedbackOutboxDispatchRow[] = [];
    let rounds = 0;
    let inspected = 0;
    let applied = 0;
    let failed = 0;
    let leasedElsewhere = 0;
    let remaining = 0;

    while (rounds < maxRounds && inspected < maxCommands) {
      const now = fixedNow ?? nowValue(undefined);
      const available = await this.controller.feedbackOutbox.pending(Math.min(limit, maxCommands - inspected), now);
      if (!available.length) {
        remaining = (await this.controller.feedbackOutbox.pending(AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX, now)).length;
        break;
      }
      const dispatch = await this.controller.dispatchFeedback({
        workerId,
        limit: available.length,
        leaseMs,
        now,
      });
      rounds += 1;
      inspected += dispatch.inspected;
      applied += dispatch.applied;
      failed += dispatch.failed;
      leasedElsewhere += dispatch.leased_elsewhere;
      rows.push(...dispatch.rows);
      remaining = (await this.controller.feedbackOutbox.pending(AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX, now)).length;
      if (dispatch.inspected === 0 || dispatch.leased_elsewhere === dispatch.inspected) break;
    }

    if (remaining === 0) {
      const now = fixedNow ?? nowValue(undefined);
      remaining = (await this.controller.feedbackOutbox.pending(AUTONOMOUS_LEARNING_MAX_FEEDBACK_OUTBOX, now)).length;
    }
    const status: AutonomousLearningFeedbackWorkerStatus = leasedElsewhere > 0 && applied === 0 && failed === 0
      ? "leased_elsewhere"
      : failed > 0
        ? "failed"
        : remaining > 0
          ? "bounded"
          : "drained";
    return {
      schema: AUTONOMOUS_LEARNING_FEEDBACK_WORKER_SCHEMA,
      worker_id: workerId,
      status,
      rounds,
      inspected,
      applied,
      failed,
      leased_elsewhere: leasedElsewhere,
      remaining,
      rows,
      retention: "value_only_feedback_commands_and_digests_no_task_or_provider_values",
      secret_material: "never_returned",
    };
  }
}
