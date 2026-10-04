import pytest

from prism_sdk.goal_time import (
    AUTONOMOUS_GOAL_TIME_SCHEMA,
    autonomous_goal_timestamp_now_ns,
    migrate_legacy_autonomous_goal_timestamp_ns,
    normalize_autonomous_goal_timestamp_ns,
    AutonomousGoalTimeError,
)


def test_goal_timestamp_contract_preserves_exact_nanoseconds():
    exact = "1780000000123456789"
    assert AUTONOMOUS_GOAL_TIME_SCHEMA == "bioprism-autonomous-goal-time/0.2"
    assert normalize_autonomous_goal_timestamp_ns(exact) == exact
    assert normalize_autonomous_goal_timestamp_ns(1_780_000_000_123_456_789) == exact
    assert autonomous_goal_timestamp_now_ns(lambda: 1_780_000_000_123_456_789) == exact


def test_legacy_timestamp_migration_requires_explicit_units():
    assert migrate_legacy_autonomous_goal_timestamp_ns(1_780_000_000_123, "milliseconds") == "1780000000123000000"
    assert migrate_legacy_autonomous_goal_timestamp_ns(1_780_000_000, "nanoseconds") == "1780000000"
    with pytest.raises(AutonomousGoalTimeError, match="signed 64-bit"):
        migrate_legacy_autonomous_goal_timestamp_ns(9_223_372_036_854_776, "milliseconds")
    with pytest.raises(AutonomousGoalTimeError, match="canonical"):
        normalize_autonomous_goal_timestamp_ns("0178")
    with pytest.raises(AutonomousGoalTimeError, match="canonical"):
        normalize_autonomous_goal_timestamp_ns(True)
