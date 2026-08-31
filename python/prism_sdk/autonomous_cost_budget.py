"""Process-local aggregate cost admission for composed autonomous work.

The provider quota controller accounts each provider/model bucket independently.  This smaller
primitive supplies the missing composition boundary: a caller can pass one ``reserve`` callback
through selection, failover, streaming, tool loops, and cross-domain fan-out so all attempts share
one atomic ceiling.  It is deliberately an estimate ledger, not a billing system.  The estimate
is retained after dispatch because a failed external request may still incur provider cost; only
local failures before dispatch receive a release callback.
"""

from __future__ import annotations

import math
import threading
from typing import Any, Callable, Mapping, TypedDict

from .errors import ArgumentError


AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS = 1_000_000


class AutonomousCostBudgetSnapshot(TypedDict):
    """Portable, metadata-only state for rehydrating an aggregate cost ceiling."""

    max_cost_units: float
    consumed_cost_units: float
    remaining_cost_units: float


AutonomousCostReservation = Callable[[], None]
AutonomousCostReservationCallback = Callable[[float], AutonomousCostReservation | None]


class AutonomousCostBudgetError(ArgumentError):
    """A caller-owned aggregate cost ceiling refused another provider attempt."""

    code = "quota_exceeded"

    def __init__(
        self,
        message: str,
        *,
        max_cost_units: float,
        consumed_cost_units: float,
        requested_cost_units: float,
    ) -> None:
        super().__init__(message)
        self.max_cost_units = max_cost_units
        self.consumed_cost_units = consumed_cost_units
        self.requested_cost_units = requested_cost_units


def _bounded_cost(name: str, value: Any) -> float:
    if (
        isinstance(value, bool)
        or not isinstance(value, (int, float))
        or not math.isfinite(float(value))
        or float(value) < 0
        or float(value) > AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS
    ):
        raise ArgumentError(
            f"{name} must be finite and within [0, {AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS}]"
        )
    return float(value)


class AutonomousCostBudget:
    """Thread-safe aggregate estimate budget for one composed autonomous execution.

    ``reserve`` increments under one lock and returns an idempotent release callback.  Callers
    should release only when admission or the local effect boundary fails before dispatch.  Once
    a provider request is dispatched, retaining the estimate preserves a conservative ceiling
    across transport failures and retries.
    """

    def __init__(self, max_cost_units: float) -> None:
        self.max_cost_units = _bounded_cost("max_total_cost_units", max_cost_units)
        self._consumed_cost_units = 0.0
        self._lock = threading.RLock()

    @classmethod
    def from_snapshot(cls, snapshot: Mapping[str, Any]) -> "AutonomousCostBudget":
        """Rehydrate without allowing persisted consumed accounting to reset."""

        if not isinstance(snapshot, Mapping):
            raise ArgumentError("cost budget snapshot is malformed")
        try:
            maximum = _bounded_cost("cost budget snapshot max_cost_units", snapshot["max_cost_units"])
            consumed = _bounded_cost(
                "cost budget snapshot consumed_cost_units", snapshot["consumed_cost_units"]
            )
            remaining = _bounded_cost(
                "cost budget snapshot remaining_cost_units", snapshot["remaining_cost_units"]
            )
        except (KeyError, TypeError) as error:
            raise ArgumentError("cost budget snapshot is malformed") from error
        if consumed > maximum or remaining != max(0.0, maximum - consumed):
            raise ArgumentError("cost budget snapshot is malformed")
        budget = cls(maximum)
        with budget._lock:
            budget._consumed_cost_units = consumed
        return budget

    @property
    def consumed_cost_units(self) -> float:
        with self._lock:
            return self._consumed_cost_units

    @property
    def remaining_cost_units(self) -> float:
        with self._lock:
            return max(0.0, self.max_cost_units - self._consumed_cost_units)

    def snapshot(self) -> AutonomousCostBudgetSnapshot:
        with self._lock:
            return {
                "max_cost_units": self.max_cost_units,
                "consumed_cost_units": self._consumed_cost_units,
                "remaining_cost_units": max(0.0, self.max_cost_units - self._consumed_cost_units),
            }

    def reserve(self, cost_units: float) -> AutonomousCostReservation:
        """Atomically reserve an estimate and return an idempotent pre-dispatch release."""

        requested = _bounded_cost("provider estimated cost", cost_units)
        with self._lock:
            if self._consumed_cost_units + requested > self.max_cost_units:
                raise AutonomousCostBudgetError(
                    "autonomous aggregate cost budget exceeded before provider dispatch",
                    max_cost_units=self.max_cost_units,
                    consumed_cost_units=self._consumed_cost_units,
                    requested_cost_units=requested,
                )
            self._consumed_cost_units += requested

        released = False
        release_lock = threading.Lock()

        def release() -> None:
            nonlocal released
            with release_lock:
                if released:
                    return
                released = True
            with self._lock:
                self._consumed_cost_units = max(0.0, self._consumed_cost_units - requested)

        return release


__all__ = [
    "AUTONOMOUS_COST_BUDGET_MAX_COST_UNITS",
    "AutonomousCostBudget",
    "AutonomousCostBudgetError",
    "AutonomousCostBudgetSnapshot",
    "AutonomousCostReservation",
    "AutonomousCostReservationCallback",
]
