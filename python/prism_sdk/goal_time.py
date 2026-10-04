"""Exact cross-SDK time primitives for autonomous goal persistence.

The v0.2 wire representation is a canonical decimal string containing epoch
nanoseconds. The explicit legacy converter prevents an SDK-specific v0.1 unit
from being guessed during migration.
"""

from __future__ import annotations

import re
import time
from collections.abc import Callable
from typing import Literal


AUTONOMOUS_GOAL_TIME_SCHEMA = "bioprism-autonomous-goal-time/0.2"
AUTONOMOUS_GOAL_TIMESTAMP_MAX_NS = 2**63 - 1
AutonomousGoalLegacyTimestampUnit = Literal["milliseconds", "nanoseconds"]
_CANONICAL_NS = re.compile(r"(?:0|[1-9][0-9]*)\Z")


class AutonomousGoalTimeError(ValueError):
    """A timestamp is malformed, out of range, or cannot be migrated exactly."""


def normalize_autonomous_goal_timestamp_ns(value: object, *, name: str = "goal timestamp") -> str:
    """Return canonical decimal nanoseconds, bounded to SQLite's signed 64-bit range."""

    if isinstance(value, bool):
        raise AutonomousGoalTimeError(f"{name} must be a canonical non-negative integer timestamp")
    if isinstance(value, int):
        exact = value
    elif isinstance(value, str) and len(value) <= 19 and _CANONICAL_NS.fullmatch(value):
        exact = int(value)
    else:
        raise AutonomousGoalTimeError(f"{name} must be a canonical non-negative integer timestamp")
    if exact < 0 or exact > AUTONOMOUS_GOAL_TIMESTAMP_MAX_NS:
        raise AutonomousGoalTimeError(f"{name} is outside the signed 64-bit nanosecond range")
    return str(exact)


def require_autonomous_goal_timestamp_ns_wire(value: object, *, name: str = "goal timestamp") -> str:
    """Require the canonical decimal-string form used by persisted v0.2 artifacts."""

    if not isinstance(value, str):
        raise AutonomousGoalTimeError(f"{name} must use the canonical decimal-string wire format")
    return normalize_autonomous_goal_timestamp_ns(value, name=name)


def migrate_legacy_autonomous_goal_timestamp_ns(
    value: object,
    source_unit: AutonomousGoalLegacyTimestampUnit,
    *,
    name: str = "legacy goal timestamp",
) -> str:
    """Convert a v0.1 timestamp only when its source unit is provided explicitly."""

    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AutonomousGoalTimeError(f"{name} must be a non-negative integer before legacy migration")
    if source_unit not in ("milliseconds", "nanoseconds"):
        raise AutonomousGoalTimeError("legacy goal timestamp migration requires milliseconds or nanoseconds as the source unit")
    multiplier = 1_000_000 if source_unit == "milliseconds" else 1
    return normalize_autonomous_goal_timestamp_ns(value * multiplier, name=name)


def autonomous_goal_timestamp_now_ns(clock: Callable[[], int] = time.time_ns) -> str:
    """Read an epoch-nanosecond clock and normalize it for the shared wire contract."""

    return normalize_autonomous_goal_timestamp_ns(clock(), name="goal clock timestamp")
