"""Bounded logical-worker routing primitives for thousands of agents."""

from __future__ import annotations

import hashlib
from collections import deque
from dataclasses import dataclass, field

from .errors import LeaseError


@dataclass(frozen=True)
class Lease:
    task: str
    worker: str
    epoch: int
    expires_at: int


class LeaseTable:
    def __init__(self) -> None:
        self._leases: dict[str, Lease] = {}
        self._epochs: dict[str, int] = {}

    def grant(self, task: str, worker: str, now: int, ttl: int) -> Lease:
        if ttl <= 0:
            raise LeaseError("lease ttl must be positive")
        if task in self._leases:
            raise LeaseError(f"task is already leased: {task}")
        epoch = self._epochs.get(task, 0) + 1
        self._epochs[task] = epoch
        lease = Lease(task, worker, epoch, now + ttl)
        self._leases[task] = lease
        return lease

    def release(self, task: str, epoch: int) -> None:
        current = self._leases.get(task)
        if current is None:
            raise LeaseError(f"unknown lease: {task}")
        if current.epoch != epoch:
            raise LeaseError(f"stale lease epoch for {task}: {epoch}")
        del self._leases[task]

    def expire(self, now: int) -> tuple[Lease, ...]:
        expired = tuple(lease for lease in self._leases.values() if lease.expires_at <= now)
        for lease in expired:
            del self._leases[lease.task]
        return tuple(sorted(expired, key=lambda lease: lease.task))

    def live(self) -> int:
        return len(self._leases)


class BoundedQueue:
    def __init__(self, capacity: int) -> None:
        if capacity <= 0:
            raise ValueError("queue capacity must be positive")
        self.capacity = capacity
        self._items: deque[str] = deque()
        self.high_water = 0

    def push(self, item: str) -> bool:
        if len(self._items) >= self.capacity:
            return False
        self._items.append(item)
        self.high_water = max(self.high_water, len(self._items))
        return True

    def pop(self) -> str | None:
        return self._items.popleft() if self._items else None

    def __len__(self) -> int:
        return len(self._items)


def assign_shard(key: str, shard_count: int) -> int:
    if shard_count <= 0:
        raise ValueError("shard_count must be positive")
    digest = hashlib.sha256(key.encode()).digest()
    return int.from_bytes(digest[:8], "big") % shard_count


@dataclass
class Telemetry:
    submitted: int = 0
    dispatched: int = 0
    completed: int = 0
    rejected_backpressure: int = 0
    lease_expiries: int = 0
    peak_in_flight: int = 0
    _in_flight: int = field(default=0, repr=False)

    def dispatch(self) -> None:
        self.submitted += 1
        self.dispatched += 1
        self._in_flight += 1
        self.peak_in_flight = max(self.peak_in_flight, self._in_flight)

    def complete(self) -> None:
        self.completed += 1
        self._in_flight = max(0, self._in_flight - 1)

    def snapshot(self) -> dict[str, int]:
        return {"completed": self.completed, "dispatched": self.dispatched, "in_flight": self._in_flight, "lease_expiries": self.lease_expiries, "peak_in_flight": self.peak_in_flight, "rejected_backpressure": self.rejected_backpressure, "submitted": self.submitted}
