import { ArgumentError } from "./errors.js";

/** Exact, cross-SDK representation for goal timestamps. Wire values are decimal strings. */
export const AUTONOMOUS_GOAL_TIME_SCHEMA = "bioprism-autonomous-goal-time/0.2" as const;
export const AUTONOMOUS_GOAL_TIMESTAMP_MAX_NS = "9223372036854775807" as const;

export type AutonomousGoalTimestampNs = string;
export type AutonomousGoalTimestampInput = string | number | bigint;
export type AutonomousGoalLegacyTimestampUnit = "milliseconds" | "nanoseconds";

const MAX_NS = BigInt(AUTONOMOUS_GOAL_TIMESTAMP_MAX_NS);

/** Normalize a v0.2 timestamp to canonical unsigned decimal nanoseconds. */
export function normalizeAutonomousGoalTimestampNs(value: unknown, name = "goal timestamp"): AutonomousGoalTimestampNs {
  let exact: bigint;
  if (typeof value === "bigint") {
    exact = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new ArgumentError(`${name} number must be a safe integer; use a decimal string or bigint for exact timestamps`);
    exact = BigInt(value);
  } else if (typeof value === "string" && value.length <= 19 && /^(0|[1-9][0-9]*)$/.test(value)) {
    exact = BigInt(value);
  } else {
    throw new ArgumentError(`${name} must be a canonical non-negative integer timestamp`);
  }
  if (exact < 0n || exact > MAX_NS) throw new ArgumentError(`${name} is outside the signed 64-bit nanosecond range`);
  return exact.toString();
}

/** Require the canonical decimal-string form used by persisted v0.2 artifacts. */
export function requireAutonomousGoalTimestampNsWire(value: unknown, name = "goal timestamp"): AutonomousGoalTimestampNs {
  if (typeof value !== "string") throw new ArgumentError(`${name} must use the canonical decimal-string wire format`);
  return normalizeAutonomousGoalTimestampNs(value, name);
}

/** Convert an explicitly identified v0.1 numeric timestamp; unit choice is mandatory. */
export function migrateLegacyAutonomousGoalTimestampNs(
  value: unknown,
  sourceUnit: AutonomousGoalLegacyTimestampUnit,
  name = "legacy goal timestamp",
): AutonomousGoalTimestampNs {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new ArgumentError(`${name} must be a non-negative safe integer before legacy migration`);
  }
  if (sourceUnit !== "milliseconds" && sourceUnit !== "nanoseconds") {
    throw new ArgumentError("legacy goal timestamp migration requires milliseconds or nanoseconds as the source unit");
  }
  const multiplier = sourceUnit === "milliseconds" ? 1_000_000n : 1n;
  return normalizeAutonomousGoalTimestampNs(BigInt(value) * multiplier, name);
}

/** Compare canonical timestamps without converting them through imprecise JS numbers. */
export function compareAutonomousGoalTimestampNs(left: unknown, right: unknown): -1 | 0 | 1 {
  const leftExact = BigInt(normalizeAutonomousGoalTimestampNs(left, "left goal timestamp"));
  const rightExact = BigInt(normalizeAutonomousGoalTimestampNs(right, "right goal timestamp"));
  return leftExact < rightExact ? -1 : leftExact > rightExact ? 1 : 0;
}

/** Produce current epoch nanoseconds from a millisecond clock without losing integer precision. */
export function autonomousGoalTimestampNowNs(clock: () => number = Date.now): AutonomousGoalTimestampNs {
  const milliseconds = clock();
  if (!Number.isSafeInteger(milliseconds) || milliseconds < 0) {
    throw new ArgumentError("goal millisecond clock must return a non-negative safe integer");
  }
  return normalizeAutonomousGoalTimestampNs(BigInt(milliseconds) * 1_000_000n, "goal clock timestamp");
}
