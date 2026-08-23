"""Restart-safe authority for activating, holding, and rolling back learned selection.

The replay promotion report is evidence; this module is the state machine that makes that
evidence operational.  It stores only report/policy/domain digests and bounded reasons.  It
never stores learner parameters, task text, candidates, rewards, prompts, provider output, or
credentials, and it does not itself mutate the bandit implementation.
"""

from __future__ import annotations

from dataclasses import dataclass, replace
import math
from threading import RLock
import time
import uuid
from typing import Any, Mapping

from .authoring import canonical_json, content_digest
from .autonomous_selection_promotion import validate_autonomous_selection_promotion_report
from .errors import ArgumentError


AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA = "bioprism-python-autonomous-selection-lifecycle/0.1"
AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA = "bioprism-python-autonomous-selection-lifecycle-store/0.1"
MAX_AUTONOMOUS_SELECTION_LIFECYCLE_REASON_BYTES = 2_000
MAX_AUTONOMOUS_SELECTION_LIFECYCLE_BYTES = 128_000
MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION = 1_000_000

_RETENTION = "metadata_only;promotion_and_domain_digests_only"
_AUTHORIZATION = "admitted_selection_only;does_not_authorize_provider_or_tools"
_SECRET_MATERIAL = "never_returned"
_STORE_RETENTION = "metadata_only_hash_bound"
_PROMOTION_SCHEMA = "bioprism-python-autonomous-selection-promotion/0.1"


def _fail(message: str) -> "NoReturn":
    raise ArgumentError(f"autonomous selection lifecycle {message}")


def _identifier(name: str, value: Any, maximum: int = 256) -> str:
    if not isinstance(value, str) or not value.strip() or len(value) > maximum or any(character not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.:-" for character in value):
        _fail(f"{name} is invalid")
    return value


def _digest(name: str, value: Any, *, allow_none: bool = False) -> str | None:
    if allow_none and value is None:
        return None
    if not isinstance(value, str) or len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        _fail(f"{name} must be a lowercase SHA-256 digest")
    return value


def _count(name: str, value: Any, maximum: int = MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= maximum:
        _fail(f"{name} is outside its bound")
    return value


def _reason(value: Any) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip() or "\x00" in value or len(value.encode("utf-8")) > MAX_AUTONOMOUS_SELECTION_LIFECYCLE_REASON_BYTES:
        _fail("last_reason is invalid")
    return value


@dataclass(frozen=True, slots=True)
class AutonomousSelectionLifecycleState:
    lifecycle_id: str
    status: str = "uninitialized"
    revision: int = 0
    generation: int = 0
    rollback_count: int = 0
    last_decision: str = "none"
    promotion_digest: str | None = None
    active_promotion_digest: str | None = None
    source_report_digest: str | None = None
    policy_digest: str | None = None
    domain_decision_digest: str | None = None
    last_reason: str | None = None
    created_at: float = 0.0
    updated_at: float = 0.0

    def __post_init__(self) -> None:
        _identifier("lifecycle_id", self.lifecycle_id)
        if self.status not in {"uninitialized", "held", "admitted", "rolled_back"}:
            _fail("state status is invalid")
        _count("state revision", self.revision)
        _count("state generation", self.generation)
        _count("state rollback_count", self.rollback_count)
        if self.last_decision not in {"none", "admit", "hold", "rollback"}:
            _fail("state last_decision is invalid")
        for name, value in (
            ("promotion_digest", self.promotion_digest),
            ("active_promotion_digest", self.active_promotion_digest),
            ("source_report_digest", self.source_report_digest),
            ("policy_digest", self.policy_digest),
            ("domain_decision_digest", self.domain_decision_digest),
        ):
            _digest(f"state {name}", value, allow_none=True)
        _reason(self.last_reason)
        if isinstance(self.created_at, bool) or not isinstance(self.created_at, (int, float)) or not math.isfinite(float(self.created_at)) or self.created_at < 0:
            _fail("state created_at is invalid")
        if isinstance(self.updated_at, bool) or not isinstance(self.updated_at, (int, float)) or not math.isfinite(float(self.updated_at)) or self.updated_at < self.created_at:
            _fail("state updated_at is invalid")
        if self.status == "admitted" and self.active_promotion_digest is None:
            _fail("admitted state must have an active promotion digest")
        if self.status != "admitted" and self.active_promotion_digest is not None:
            _fail("non-admitted state cannot have an active promotion digest")

    def _payload(self) -> dict[str, Any]:
        return {
            "schema": AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA,
            "lifecycle_id": self.lifecycle_id,
            "status": self.status,
            "revision": self.revision,
            "generation": self.generation,
            "rollback_count": self.rollback_count,
            "last_decision": self.last_decision,
            "promotion_digest": self.promotion_digest,
            "active_promotion_digest": self.active_promotion_digest,
            "source_report_digest": self.source_report_digest,
            "policy_digest": self.policy_digest,
            "domain_decision_digest": self.domain_decision_digest,
            "last_reason": self.last_reason,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        }

    @property
    def state_digest(self) -> str:
        return content_digest(self._payload())

    def to_dict(self) -> dict[str, Any]:
        return {
            **self._payload(),
            "state_digest": self.state_digest,
            "retention": _RETENTION,
            "authorization": _AUTHORIZATION,
            "secret_material": _SECRET_MATERIAL,
        }

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "AutonomousSelectionLifecycleState":
        if not isinstance(value, Mapping):
            _fail("state must be a mapping")
        allowed = set(cls(lifecycle_id="placeholder")._payload()) | {"state_digest", "retention", "authorization", "secret_material", "schema"}
        if set(value).difference(allowed):
            _fail("state contains unsupported fields")
        if value.get("schema", AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA) != AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA:
            _fail("state schema is invalid")
        if value.get("retention", _RETENTION) != _RETENTION or value.get("authorization", _AUTHORIZATION) != _AUTHORIZATION or value.get("secret_material", _SECRET_MATERIAL) != _SECRET_MATERIAL:
            _fail("state retention markers are invalid")
        state = cls(
            lifecycle_id=value.get("lifecycle_id"),
            status=value.get("status", "uninitialized"),
            revision=value.get("revision", 0),
            generation=value.get("generation", 0),
            rollback_count=value.get("rollback_count", 0),
            last_decision=value.get("last_decision", "none"),
            promotion_digest=value.get("promotion_digest"),
            active_promotion_digest=value.get("active_promotion_digest"),
            source_report_digest=value.get("source_report_digest"),
            policy_digest=value.get("policy_digest"),
            domain_decision_digest=value.get("domain_decision_digest"),
            last_reason=value.get("last_reason"),
            created_at=value.get("created_at", 0.0),
            updated_at=value.get("updated_at", 0.0),
        )
        if value.get("state_digest") is not None and value.get("state_digest") != state.state_digest:
            _fail("state digest does not match its contents")
        if len(canonical_json(state.to_dict()).encode("utf-8")) > MAX_AUTONOMOUS_SELECTION_LIFECYCLE_BYTES:
            _fail("state exceeds its byte bound")
        return state


def _promotion_projection(report: Mapping[str, Any]) -> dict[str, Any]:
    validated = validate_autonomous_selection_promotion_report(report)
    if validated.get("schema") != _PROMOTION_SCHEMA:
        _fail("promotion report schema is invalid")
    domains = validated.get("domains")
    if not isinstance(domains, list) or len(domains) != 12:
        _fail("promotion report must contain every autonomous domain")
    reasons = validated.get("reasons", [])
    reason = "; ".join(reasons) if reasons else "selection promotion held" if validated["decision"] == "hold" else None
    return {
        "promotion_digest": validated["promotion_digest"],
        "source_report_digest": validated["source_report_digest"],
        "policy_digest": content_digest(validated["policy"]),
        "domain_decision_digest": content_digest([
            {"domain": row["domain"], "decision": row["decision"], "reasons": row["reasons"]}
            for row in domains
        ]),
        "reason": reason,
        "decision": validated["decision"],
    }


class AutonomousSelectionPromotionLifecycle:
    """Apply promotion evidence and gate learned selection with explicit rollback semantics."""

    def __init__(
        self,
        lifecycle_id: str | None = None,
        *,
        state: AutonomousSelectionLifecycleState | Mapping[str, Any] | None = None,
        clock: Any = time.time,
    ) -> None:
        if not callable(clock):
            _fail("clock must be callable")
        self._clock = clock
        self._lock = RLock()
        if state is not None:
            self._state = state if isinstance(state, AutonomousSelectionLifecycleState) else AutonomousSelectionLifecycleState.from_mapping(state)
        else:
            now = self._now()
            self._state = AutonomousSelectionLifecycleState(
                lifecycle_id=lifecycle_id or f"selection-lifecycle-{uuid.uuid4().hex}",
                created_at=now,
                updated_at=now,
            )

    @property
    def state(self) -> AutonomousSelectionLifecycleState:
        with self._lock:
            return self._state

    def is_admitted(self) -> bool:
        return self._state.status == "admitted" and self._state.active_promotion_digest is not None

    def apply(self, report: Mapping[str, Any]) -> AutonomousSelectionLifecycleState:
        projection = _promotion_projection(report)
        with self._lock:
            was_admitted = self.is_admitted()
            status = "admitted" if projection["decision"] == "admit" else "rolled_back" if was_admitted else "held"
            self._commit(
                status=status,
                generation=self._state.generation + 1 if projection["decision"] == "admit" else self._state.generation,
                rollback_count=self._state.rollback_count + 1 if projection["decision"] == "hold" and was_admitted else self._state.rollback_count,
                last_decision=projection["decision"],
                promotion_digest=projection["promotion_digest"],
                active_promotion_digest=projection["promotion_digest"] if projection["decision"] == "admit" else None,
                source_report_digest=projection["source_report_digest"],
                policy_digest=projection["policy_digest"],
                domain_decision_digest=projection["domain_decision_digest"],
                last_reason=projection["reason"],
            )
            return self._state

    def rollback(self, *, reason: str = "selection_promotion_rollback") -> AutonomousSelectionLifecycleState:
        _reason(reason)
        with self._lock:
            if not self.is_admitted():
                return self._state
            self._commit(
                status="rolled_back",
                rollback_count=self._state.rollback_count + 1,
                last_decision="rollback",
                active_promotion_digest=None,
                last_reason=reason,
            )
            return self._state

    def restore(self, raw: AutonomousSelectionLifecycleState | Mapping[str, Any]) -> AutonomousSelectionLifecycleState:
        next_state = raw if isinstance(raw, AutonomousSelectionLifecycleState) else AutonomousSelectionLifecycleState.from_mapping(raw)
        with self._lock:
            if self._state.revision > 0 and next_state.lifecycle_id != self._state.lifecycle_id:
                _fail("lifecycle identity cannot change after initialization")
            if next_state.revision < self._state.revision:
                _fail("lifecycle revision cannot move backwards")
            self._state = next_state
            return self._state

    def _now(self) -> float:
        value = self._clock()
        if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)) or value < 0:
            _fail("clock must return a finite non-negative timestamp")
        return float(value)

    def _commit(self, **changes: Any) -> None:
        if not any(getattr(self._state, key) != value for key, value in changes.items()):
            return
        self._state = replace(
            self._state,
            **changes,
            revision=self._state.revision + 1,
            updated_at=max(self._now(), self._state.updated_at),
        )
        AutonomousSelectionLifecycleState.from_mapping(self._state.to_dict())


class AutonomousSelectionPromotionLifecycleStore:
    """In-memory reference store with digest and revision checks for caller persistence."""

    def __init__(self) -> None:
        self._value: AutonomousSelectionLifecycleState | None = None

    def load(self) -> AutonomousSelectionLifecycleState | None:
        return self._value

    def save(self, state: AutonomousSelectionLifecycleState | Mapping[str, Any]) -> None:
        normalized = state if isinstance(state, AutonomousSelectionLifecycleState) else AutonomousSelectionLifecycleState.from_mapping(state)
        if self._value is not None and normalized.state_digest != self._value.state_digest and normalized.revision != self._value.revision + 1:
            _fail("lifecycle revision continuity check failed")
        self._value = normalized

    def snapshot(self) -> dict[str, Any]:
        state = self._value or AutonomousSelectionLifecycleState("selection-lifecycle-empty")
        body = {
            "schema": AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA,
            "state": state.to_dict(),
            "state_digest": state.state_digest,
            "retention": _STORE_RETENTION,
            "secret_material": _SECRET_MATERIAL,
        }
        return {**body, "snapshot_digest": content_digest(body)}

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        if not isinstance(snapshot, Mapping) or snapshot.get("schema") != AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA or snapshot.get("retention") != _STORE_RETENTION or snapshot.get("secret_material") != _SECRET_MATERIAL:
            _fail("snapshot retention markers are invalid")
        state = AutonomousSelectionLifecycleState.from_mapping(snapshot.get("state"))
        if snapshot.get("state_digest") != state.state_digest:
            _fail("snapshot state digest is invalid")
        body = {key: value for key, value in snapshot.items() if key != "snapshot_digest"}
        if snapshot.get("snapshot_digest") != content_digest(body):
            _fail("snapshot digest does not match its contents")
        self._value = state


__all__ = [
    "AUTONOMOUS_SELECTION_LIFECYCLE_SCHEMA",
    "AUTONOMOUS_SELECTION_LIFECYCLE_STORE_SCHEMA",
    "MAX_AUTONOMOUS_SELECTION_LIFECYCLE_REASON_BYTES",
    "MAX_AUTONOMOUS_SELECTION_LIFECYCLE_BYTES",
    "MAX_AUTONOMOUS_SELECTION_LIFECYCLE_GENERATION",
    "AutonomousSelectionLifecycleState",
    "AutonomousSelectionPromotionLifecycle",
    "AutonomousSelectionPromotionLifecycleStore",
]
