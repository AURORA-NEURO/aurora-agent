import test from "node:test";
import assert from "node:assert/strict";
import {
  AUTONOMOUS_GOAL_TIME_SCHEMA,
  autonomousGoalTimestampNowNs,
  compareAutonomousGoalTimestampNs,
  migrateLegacyAutonomousGoalTimestampNs,
  normalizeAutonomousGoalTimestampNs,
} from "../dist/index.js";

test("goal timestamp contract preserves nanoseconds beyond JavaScript safe integers", () => {
  const exact = "1780000000123456789";
  assert.equal(AUTONOMOUS_GOAL_TIME_SCHEMA, "bioprism-autonomous-goal-time/0.2");
  assert.equal(normalizeAutonomousGoalTimestampNs(exact), exact);
  assert.equal(compareAutonomousGoalTimestampNs("1780000000123456788", exact), -1);
  assert.equal(compareAutonomousGoalTimestampNs(exact, exact), 0);
  assert.equal(compareAutonomousGoalTimestampNs("1780000000123456790", exact), 1);
});

test("legacy timestamps require an explicit SDK source unit", () => {
  assert.equal(migrateLegacyAutonomousGoalTimestampNs(1_780_000_000_123, "milliseconds"), "1780000000123000000");
  assert.equal(migrateLegacyAutonomousGoalTimestampNs(1_780_000_000, "nanoseconds"), "1780000000");
  assert.throws(() => migrateLegacyAutonomousGoalTimestampNs(9_223_372_036_855, "milliseconds"), /signed 64-bit/);
  assert.throws(() => migrateLegacyAutonomousGoalTimestampNs(Number.MAX_SAFE_INTEGER + 1, "nanoseconds"), /safe integer/);
});

test("millisecond clocks are promoted to nanoseconds without float arithmetic", () => {
  assert.equal(autonomousGoalTimestampNowNs(() => 1_780_000_000_123), "1780000000123000000");
  assert.throws(() => normalizeAutonomousGoalTimestampNs("0178"), /canonical/);
  assert.throws(() => normalizeAutonomousGoalTimestampNs("9223372036854775808"), /signed 64-bit/);
});
