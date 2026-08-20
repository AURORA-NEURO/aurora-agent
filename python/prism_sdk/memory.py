"""Durable, value-only episodic memory for autonomous brain runs.

The memory layer deliberately stores decision metadata and caller-authored lessons rather than
provider prompts, responses, tool arguments, credentials, or opaque transport envelopes.  It is
implemented with a small SQLite append log plus a materialized query index so an embedding
application can survive process restarts without turning the SDK into a secret store or a truth
oracle.  Every event participates in a tamper-evident hash chain.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
import sqlite3
import threading
import time
from typing import Any, Callable, Mapping, Sequence
import uuid


MEMORY_SCHEMA = "bioprism-brain-episodic-memory/0.1"
MEMORY_EVENT_SCHEMA = "bioprism-brain-episodic-event/0.1"
MAX_MEMORY_ID_BYTES = 256
MAX_MEMORY_LABEL_BYTES = 256
MAX_MEMORY_LESSON_BYTES = 4_096
MAX_MEMORY_TAGS = 64
MAX_MEMORY_TAG_BYTES = 128
MAX_MEMORY_CONTEXT_KEYS = 32
MAX_MEMORY_PROVENANCE_BYTES = 16_000


class BrainMemoryError(RuntimeError):
    """A durable memory request, record, or integrity check was refused."""


def _canonical(value: Any) -> str:
    try:
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
    except (TypeError, ValueError) as error:
        raise BrainMemoryError("memory value must be JSON-safe") from error


def _digest(value: Any) -> str:
    return hashlib.sha256(_canonical(value).encode("utf-8")).hexdigest()


def _valid_digest(value: Any) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(
        character in "0123456789abcdef" for character in value
    )


def _bounded_string(value: Any, *, name: str, maximum: int) -> str:
    if not isinstance(value, str) or not value.strip():
        raise BrainMemoryError(f"{name} must be a non-empty string")
    if "\x00" in value or len(value.encode("utf-8")) > maximum:
        raise BrainMemoryError(f"{name} exceeds its bounded size")
    return value


_FORBIDDEN_FIELDS = {
    "api_key",
    "apikey",
    "authorization",
    "bearer",
    "credential",
    "password",
    "secret",
    "access_token",
    "refresh_token",
    "prompt",
    "messages",
    "response",
    "content",
    "raw",
    "body",
    "headers",
    "arguments",
    "input",
    "output",
    "task",
}
_FORBIDDEN_NORMALIZED_FIELDS = {
    "".join(character for character in field if character.isalnum())
    for field in _FORBIDDEN_FIELDS
}
_SENSITIVE_STRING_PATTERNS = (
    re.compile(
        r"(?i)\b(?:api[_ -]?key|access[_ -]?token|refresh[_ -]?token|password|authorization|secret)\b\s*[:=]\s*\S+"
    ),
    re.compile(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}"),
    re.compile(r"\b(?:sk|rk|pk)-[A-Za-z0-9_-]{16,}\b"),
)


def _safe_value(value: Any, *, depth: int = 0) -> Any:
    """Copy a bounded value while rejecting fields that can carry raw sensitive material."""

    if depth > 8:
        raise BrainMemoryError("memory value exceeds maximum nesting depth")
    if value is None or isinstance(value, bool):
        return value
    if isinstance(value, int):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise BrainMemoryError("memory value contains a non-finite number")
        return value
    if isinstance(value, str):
        if "\x00" in value or len(value.encode("utf-8")) > MAX_MEMORY_PROVENANCE_BYTES:
            raise BrainMemoryError("memory string exceeds the bounded size")
        if any(pattern.search(value) for pattern in _SENSITIVE_STRING_PATTERNS):
            raise BrainMemoryError("memory string resembles secret material")
        return value
    if isinstance(value, Mapping):
        if len(value) > MAX_MEMORY_CONTEXT_KEYS:
            raise BrainMemoryError("memory mapping exceeds the bounded key count")
        copied: dict[str, Any] = {}
        for key, child in value.items():
            if not isinstance(key, str) or not key.strip():
                raise BrainMemoryError("memory mapping keys must be non-empty strings")
            normalized = "".join(character for character in key.lower() if character.isalnum())
            if normalized in _FORBIDDEN_NORMALIZED_FIELDS:
                raise BrainMemoryError("memory record contains a forbidden raw-content or secret field")
            copied[key] = _safe_value(child, depth=depth + 1)
        return copied
    if isinstance(value, (list, tuple)):
        if len(value) > MAX_MEMORY_TAGS * 2:
            raise BrainMemoryError("memory sequence exceeds the bounded item count")
        return [_safe_value(child, depth=depth + 1) for child in value]
    raise BrainMemoryError(f"memory value has unsupported type {type(value).__name__}")


def _safe_digest_map(value: Any) -> dict[str, str | None]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise BrainMemoryError("episode digests must be a mapping")
    if len(value) > MAX_MEMORY_CONTEXT_KEYS:
        raise BrainMemoryError("episode digests exceed the bounded key count")
    result: dict[str, str | None] = {}
    for key, item in value.items():
        name = _bounded_string(key, name="digest field", maximum=MAX_MEMORY_LABEL_BYTES)
        if not name.endswith("_digest"):
            raise BrainMemoryError("episode digest fields must end in _digest")
        if item is not None and not _valid_digest(item):
            raise BrainMemoryError(f"{name} must be a lowercase SHA-256 digest or None")
        result[name] = item
    return result


@dataclass(frozen=True, slots=True)
class MemoryQuery:
    """Deterministic metadata query used to recall related episodes."""

    domain: str | None = None
    capability: str | None = None
    risk_class: str | None = None
    task_digest: str | None = None
    tags: tuple[str, ...] = ()
    statuses: tuple[str, ...] = ()
    include_failed: bool = True
    limit: int = 8

    def __post_init__(self) -> None:
        for name, value in (
            ("domain", self.domain),
            ("capability", self.capability),
            ("risk_class", self.risk_class),
        ):
            if value is not None:
                _bounded_string(value, name=f"query.{name}", maximum=MAX_MEMORY_LABEL_BYTES)
        if self.task_digest is not None and not _valid_digest(self.task_digest):
            raise BrainMemoryError("query.task_digest must be a lowercase SHA-256 digest")
        for name, values in (("tags", self.tags), ("statuses", self.statuses)):
            if not isinstance(values, tuple) or len(values) > MAX_MEMORY_TAGS:
                raise BrainMemoryError(f"query.{name} must be a bounded tuple")
            for value in values:
                _bounded_string(value, name=f"query.{name} item", maximum=MAX_MEMORY_TAG_BYTES)
        if not isinstance(self.include_failed, bool):
            raise BrainMemoryError("query.include_failed must be boolean")
        if not isinstance(self.limit, int) or isinstance(self.limit, bool) or not 1 <= self.limit <= 128:
            raise BrainMemoryError("query.limit must be within [1, 128]")

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any] | None) -> "MemoryQuery":
        if value is None:
            return cls()
        if not isinstance(value, Mapping):
            raise BrainMemoryError("memory query must be a mapping")
        if any(not isinstance(key, str) for key in value):
            raise BrainMemoryError("memory query keys must be strings")
        allowed = {
            "domain",
            "capability",
            "risk_class",
            "task_digest",
            "tags",
            "statuses",
            "include_failed",
            "limit",
        }
        unknown = sorted(set(value).difference(allowed))
        if unknown:
            raise BrainMemoryError("memory query contains unsupported fields: " + ", ".join(unknown))
        def tuple_field(name: str) -> tuple[str, ...]:
            raw = value.get(name, ())
            if not isinstance(raw, Sequence) or isinstance(raw, (str, bytes)):
                raise BrainMemoryError(f"query.{name} must be a string sequence")
            return tuple(raw)
        return cls(
            domain=value.get("domain"),
            capability=value.get("capability"),
            risk_class=value.get("risk_class"),
            task_digest=value.get("task_digest"),
            tags=tuple_field("tags"),
            statuses=tuple_field("statuses"),
            include_failed=value.get("include_failed", True),
            limit=value.get("limit", 8),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "domain": self.domain,
            "capability": self.capability,
            "risk_class": self.risk_class,
            "task_digest": self.task_digest,
            "tags": list(self.tags),
            "statuses": list(self.statuses),
            "include_failed": self.include_failed,
            "limit": self.limit,
        }


@dataclass(frozen=True, slots=True)
class MemoryReceipt:
    """Receipt for an append-only memory event."""

    event_type: str
    episode_id: str
    sequence: int
    event_digest: str
    head_digest: str
    idempotent: bool = False

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": MEMORY_EVENT_SCHEMA,
            "event_type": self.event_type,
            "episode_id": self.episode_id,
            "sequence": self.sequence,
            "event_digest": self.event_digest,
            "head_digest": self.head_digest,
            "idempotent": self.idempotent,
            "retention": "value_only_hash_chained",
        }


class BrainEpisodicMemory:
    """Restart-safe bounded memory for completed brain decisions and evaluator updates.

    ``record_episode`` accepts a deliberately narrow packet.  A caller supplies digests and
    optional safe labels/lessons; it cannot accidentally pass a task, prompt, provider response,
    tool arguments, or credential-shaped field through this boundary.  ``record_evaluation`` is a
    second append-only event, so replay can distinguish what the brain did from what an evaluator
    later judged.
    """

    _EPISODE_FIELDS = {
        "episode_id",
        "run_id",
        "result_kind",
        "status",
        "task_digest",
        "context",
        "selected_model",
        "digests",
        "route",
        "tags",
        "lesson",
        "provenance",
    }
    _EVALUATION_FIELDS = {
        "evaluator_id",
        "evaluator_version",
        "reward",
        "passed",
        "failed",
        "feedback_digest",
        "failure_class",
        "evidence_digest",
        "decision_digest",
        "replan_requested",
        "replan_instruction",
        "replan_instruction_digest",
    }

    def __init__(
        self,
        path: str | Path,
        *,
        max_episodes: int = 4_096,
        max_bytes: int = 64_000_000,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if not isinstance(path, (str, Path)) or not str(path):
            raise BrainMemoryError("memory path must be non-empty")
        if not isinstance(max_episodes, int) or isinstance(max_episodes, bool) or max_episodes <= 0:
            raise BrainMemoryError("max_episodes must be positive")
        if not isinstance(max_bytes, int) or isinstance(max_bytes, bool) or max_bytes <= 0:
            raise BrainMemoryError("max_bytes must be positive")
        if not callable(clock):
            raise BrainMemoryError("clock must be callable")
        self.path = str(path)
        self.max_episodes = max_episodes
        self.max_bytes = max_bytes
        self._clock = clock
        self._lock = threading.RLock()
        if self.path != ":memory:":
            parent = Path(self.path).parent
            parent.mkdir(parents=True, exist_ok=True)
        self._connection = sqlite3.connect(
            self.path,
            isolation_level=None,
            check_same_thread=False,
        )
        self._connection.row_factory = sqlite3.Row
        with self._lock:
            self._connection.execute("PRAGMA foreign_keys=ON")
            self._connection.execute("PRAGMA synchronous=FULL")
            self._connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS memory_events (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_type TEXT NOT NULL,
                    episode_id TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    previous_digest TEXT NOT NULL,
                    event_digest TEXT NOT NULL UNIQUE,
                    created_ns INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS memory_episodes (
                    episode_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    result_kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    task_digest TEXT NOT NULL,
                    domain TEXT,
                    capability TEXT,
                    risk_class TEXT,
                    tags_json TEXT NOT NULL,
                    packet_json TEXT NOT NULL,
                    evaluation_json TEXT,
                    record_sequence INTEGER NOT NULL,
                    record_digest TEXT NOT NULL,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS memory_episodes_context_idx
                    ON memory_episodes(domain, capability, risk_class, created_ns DESC);
                CREATE INDEX IF NOT EXISTS memory_episodes_task_idx
                    ON memory_episodes(task_digest);
                """
            )

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "BrainEpisodicMemory":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()

    def record_episode(self, packet: Mapping[str, Any]) -> MemoryReceipt:
        normalized = self._normalize_episode(packet)
        episode_id = normalized["episode_id"]
        payload = {
            "schema": MEMORY_EVENT_SCHEMA,
            "event": "episode_recorded",
            "episode": normalized,
        }
        payload_json = _canonical(payload)
        with self._lock:
            self._begin_locked()
            try:
                existing = self._connection.execute(
                    "SELECT record_sequence, record_digest, packet_json FROM memory_episodes WHERE episode_id = ?",
                    (episode_id,),
                ).fetchone()
                if existing is not None:
                    if existing["packet_json"] != _canonical(normalized):
                        raise BrainMemoryError("episode_id already exists with different metadata")
                    head = self._head_locked()
                    self._connection.execute("COMMIT")
                    return MemoryReceipt(
                        event_type="episode_recorded",
                        episode_id=episode_id,
                        sequence=int(existing["record_sequence"]),
                        event_digest=str(existing["record_digest"]),
                        head_digest=head,
                        idempotent=True,
                    )
                count = int(self._connection.execute("SELECT COUNT(*) FROM memory_episodes").fetchone()[0])
                if count >= self.max_episodes:
                    raise BrainMemoryError("episodic memory episode capacity is exhausted")
                receipt = self._append_event_locked(
                    event_type="episode_recorded",
                    episode_id=episode_id,
                    payload_json=payload_json,
                )
                context = normalized.get("context", {})
                domain = context.get("domain") if isinstance(context, Mapping) else None
                capability = context.get("capability") if isinstance(context, Mapping) else None
                risk_class = context.get("risk_class") if isinstance(context, Mapping) else None
                now_ns = self._now_ns()
                self._connection.execute(
                    """
                    INSERT INTO memory_episodes (
                        episode_id, run_id, result_kind, status, task_digest, domain, capability,
                        risk_class, tags_json, packet_json, evaluation_json, record_sequence,
                        record_digest, created_ns, updated_ns
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)
                    """,
                    (
                        episode_id,
                        normalized["run_id"],
                        normalized["result_kind"],
                        normalized["status"],
                        normalized["task_digest"],
                        domain if isinstance(domain, str) else None,
                        capability if isinstance(capability, str) else None,
                        risk_class if isinstance(risk_class, str) else None,
                        _canonical(normalized["tags"]),
                        _canonical(normalized),
                        receipt.sequence,
                        receipt.event_digest,
                        now_ns,
                        now_ns,
                    ),
                )
                self._ensure_capacity_locked()
                self._connection.execute("COMMIT")
                return receipt
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def record_evaluation(self, episode_id: str, evaluation: Mapping[str, Any]) -> MemoryReceipt:
        episode_id = _bounded_string(episode_id, name="episode_id", maximum=MAX_MEMORY_ID_BYTES)
        normalized = self._normalize_evaluation(evaluation)
        payload = {
            "schema": MEMORY_EVENT_SCHEMA,
            "event": "evaluation_recorded",
            "episode_id": episode_id,
            "evaluation": normalized,
        }
        payload_json = _canonical(payload)
        evaluation_json = _canonical(normalized)
        with self._lock:
            self._begin_locked()
            try:
                existing = self._connection.execute(
                    "SELECT episode_id, evaluation_json FROM memory_episodes WHERE episode_id = ?",
                    (episode_id,),
                ).fetchone()
                if existing is None:
                    raise BrainMemoryError("cannot evaluate an unknown episode")
                if existing["evaluation_json"] == evaluation_json:
                    head = self._head_locked()
                    self._connection.execute("COMMIT")
                    row = self._connection.execute(
                        "SELECT sequence, event_digest FROM memory_events WHERE event_type = 'evaluation_recorded' AND episode_id = ? ORDER BY sequence DESC LIMIT 1",
                        (episode_id,),
                    ).fetchone()
                    if row is None:
                        raise BrainMemoryError("evaluation index is inconsistent")
                    return MemoryReceipt(
                        event_type="evaluation_recorded",
                        episode_id=episode_id,
                        sequence=int(row["sequence"]),
                        event_digest=str(row["event_digest"]),
                        head_digest=head,
                        idempotent=True,
                    )
                receipt = self._append_event_locked(
                    event_type="evaluation_recorded",
                    episode_id=episode_id,
                    payload_json=payload_json,
                )
                now_ns = self._now_ns()
                self._connection.execute(
                    "UPDATE memory_episodes SET evaluation_json = ?, updated_ns = ? WHERE episode_id = ?",
                    (evaluation_json, now_ns, episode_id),
                )
                self._ensure_capacity_locked()
                self._connection.execute("COMMIT")
                return receipt
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def get(self, episode_id: str) -> dict[str, Any] | None:
        episode_id = _bounded_string(episode_id, name="episode_id", maximum=MAX_MEMORY_ID_BYTES)
        with self._lock:
            row = self._connection.execute(
                "SELECT * FROM memory_episodes WHERE episode_id = ?",
                (episode_id,),
            ).fetchone()
            return None if row is None else self._project_row(row)

    def retrieve(
        self,
        query: MemoryQuery | Mapping[str, Any] | None = None,
        *,
        limit: int | None = None,
    ) -> list[dict[str, Any]]:
        resolved = query if isinstance(query, MemoryQuery) else MemoryQuery.from_mapping(query)
        if limit is not None:
            resolved = MemoryQuery(
                domain=resolved.domain,
                capability=resolved.capability,
                risk_class=resolved.risk_class,
                task_digest=resolved.task_digest,
                tags=resolved.tags,
                statuses=resolved.statuses,
                include_failed=resolved.include_failed,
                limit=limit,
            )
        with self._lock:
            rows = self._connection.execute(
                "SELECT * FROM memory_episodes ORDER BY created_ns DESC, record_sequence DESC, episode_id ASC"
            ).fetchall()
        ranked: list[tuple[int, int, dict[str, Any]]] = []
        query_tags = set(resolved.tags)
        for row in rows:
            if resolved.domain is not None and row["domain"] != resolved.domain:
                continue
            if resolved.capability is not None and row["capability"] != resolved.capability:
                continue
            if resolved.risk_class is not None and row["risk_class"] != resolved.risk_class:
                continue
            if resolved.task_digest is not None and row["task_digest"] != resolved.task_digest:
                continue
            if resolved.statuses and row["status"] not in resolved.statuses:
                continue
            try:
                tags = set(json.loads(row["tags_json"]))
                evaluation = None if row["evaluation_json"] is None else json.loads(row["evaluation_json"])
            except (TypeError, ValueError, json.JSONDecodeError) as error:
                raise BrainMemoryError("episodic memory index contains invalid JSON") from error
            if query_tags and not query_tags.intersection(tags):
                continue
            if not resolved.include_failed and isinstance(evaluation, Mapping) and evaluation.get("failed"):
                continue
            score = 0
            if resolved.task_digest is not None:
                score += 100
            if resolved.domain is not None:
                score += 20
            if resolved.capability is not None:
                score += 20
            if resolved.risk_class is not None:
                score += 10
            score += 5 * len(query_tags.intersection(tags))
            if isinstance(evaluation, Mapping):
                if evaluation.get("passed") is True:
                    score += 2
                if evaluation.get("failed") is True:
                    score -= 1
            ranked.append((score, int(row["created_ns"]), self._project_row(row)))
        ranked.sort(key=lambda item: (-item[0], -item[1], item[2]["episode_id"]))
        return [item[2] for item in ranked[: resolved.limit]]

    def verify_integrity(self) -> dict[str, Any]:
        """Verify the event hash chain and its materialized episode index."""

        with self._lock:
            try:
                rows = self._connection.execute(
                    "SELECT * FROM memory_events ORDER BY sequence ASC"
                ).fetchall()
                previous = ""
                episode_events: set[str] = set()
                for row in rows:
                    if row["previous_digest"] != previous:
                        raise BrainMemoryError(f"memory hash chain breaks at sequence {row['sequence']}")
                    try:
                        payload = json.loads(row["payload_json"])
                    except (TypeError, ValueError, json.JSONDecodeError) as error:
                        raise BrainMemoryError("memory event contains invalid JSON") from error
                    if not isinstance(payload, Mapping) or payload.get("schema") != MEMORY_EVENT_SCHEMA:
                        raise BrainMemoryError(f"memory event schema mismatch at sequence {row['sequence']}")
                    if row["event_type"] == "episode_recorded":
                        episode = payload.get("episode")
                        if not isinstance(episode, Mapping) or episode.get("episode_id") != row["episode_id"]:
                            raise BrainMemoryError(f"memory episode payload mismatch at sequence {row['sequence']}")
                    elif row["event_type"] == "evaluation_recorded":
                        if payload.get("episode_id") != row["episode_id"] or not isinstance(
                            payload.get("evaluation"), Mapping
                        ):
                            raise BrainMemoryError(f"memory evaluation payload mismatch at sequence {row['sequence']}")
                    else:
                        raise BrainMemoryError(f"memory event type is unknown at sequence {row['sequence']}")
                    envelope = {
                        "schema": MEMORY_EVENT_SCHEMA,
                        "event_type": row["event_type"],
                        "episode_id": row["episode_id"],
                        "payload": payload,
                        "previous_digest": row["previous_digest"],
                        "sequence": row["sequence"],
                        "created_ns": row["created_ns"],
                    }
                    expected = _digest(envelope)
                    if row["event_digest"] != expected:
                        raise BrainMemoryError(f"memory event digest mismatch at sequence {row['sequence']}")
                    if row["event_type"] == "episode_recorded":
                        episode_events.add(row["episode_id"])
                    previous = row["event_digest"]
                indexed = {
                    row["episode_id"]
                    for row in self._connection.execute("SELECT episode_id FROM memory_episodes").fetchall()
                }
                if not indexed.issubset(episode_events):
                    raise BrainMemoryError("memory index contains an episode without a record event")
                return {
                    "schema": MEMORY_SCHEMA,
                    "ok": True,
                    "event_count": len(rows),
                    "episode_count": len(indexed),
                    "head_digest": previous,
                    "chain": "sha256_prev_digest",
                }
            except BrainMemoryError as error:
                return {
                    "schema": MEMORY_SCHEMA,
                    "ok": False,
                    "event_count": 0,
                    "episode_count": 0,
                    "head_digest": None,
                    "chain": "sha256_prev_digest",
                    "reason": str(error),
                }

    def stats(self) -> dict[str, Any]:
        with self._lock:
            event_count = int(self._connection.execute("SELECT COUNT(*) FROM memory_events").fetchone()[0])
            episode_count = int(self._connection.execute("SELECT COUNT(*) FROM memory_episodes").fetchone()[0])
            evaluation_count = int(
                self._connection.execute(
                    "SELECT COUNT(*) FROM memory_episodes WHERE evaluation_json IS NOT NULL"
                ).fetchone()[0]
            )
            return {
                "schema": MEMORY_SCHEMA,
                "episode_count": episode_count,
                "event_count": event_count,
                "evaluation_count": evaluation_count,
                "max_episodes": self.max_episodes,
                "max_bytes": self.max_bytes,
                "retention": "value_only_hash_chained",
            }

    def _normalize_episode(self, packet: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(packet, Mapping):
            raise BrainMemoryError("episode packet must be a mapping")
        if any(not isinstance(key, str) for key in packet):
            raise BrainMemoryError("episode packet keys must be strings")
        unknown = sorted(set(packet).difference(self._EPISODE_FIELDS))
        if unknown:
            raise BrainMemoryError("episode packet contains unsupported fields: " + ", ".join(unknown))
        episode_id = packet.get("episode_id") or f"episode-{uuid.uuid4().hex}"
        raw_tags = packet.get("tags", ())
        if not isinstance(raw_tags, Sequence) or isinstance(raw_tags, (str, bytes)):
            raise BrainMemoryError("episode.tags must be a string sequence")
        normalized = {
            "episode_id": _bounded_string(episode_id, name="episode_id", maximum=MAX_MEMORY_ID_BYTES),
            "run_id": _bounded_string(packet.get("run_id"), name="run_id", maximum=MAX_MEMORY_ID_BYTES),
            "result_kind": _bounded_string(packet.get("result_kind"), name="result_kind", maximum=MAX_MEMORY_LABEL_BYTES),
            "status": _bounded_string(packet.get("status"), name="status", maximum=MAX_MEMORY_LABEL_BYTES),
            "task_digest": packet.get("task_digest"),
            "context": _safe_value(packet.get("context", {})),
            "selected_model": _safe_value(packet.get("selected_model", {})),
            "digests": _safe_digest_map(packet.get("digests")),
            "route": _safe_value(packet.get("route", {})),
            "tags": list(raw_tags),
            "lesson": packet.get("lesson"),
            "provenance": _safe_value(packet.get("provenance", {})),
        }
        if not _valid_digest(normalized["task_digest"]):
            raise BrainMemoryError("episode.task_digest must be a lowercase SHA-256 digest")
        context = normalized["context"]
        if not isinstance(context, Mapping):
            raise BrainMemoryError("episode.context must be a mapping")
        for field in ("domain", "capability", "risk_class"):
            value = context.get(field)
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise BrainMemoryError(f"episode.context.{field} must be a non-empty string when supplied")
        selected = normalized["selected_model"]
        if not isinstance(selected, Mapping):
            raise BrainMemoryError("episode.selected_model must be a mapping")
        for field in ("provider", "model"):
            value = selected.get(field)
            if value is not None and (not isinstance(value, str) or not value.strip()):
                raise BrainMemoryError(f"episode.selected_model.{field} must be non-empty when supplied")
        tags = normalized["tags"]
        if not isinstance(tags, list) or len(tags) > MAX_MEMORY_TAGS:
            raise BrainMemoryError("episode.tags must be a bounded sequence")
        for index, tag in enumerate(tags):
            tags[index] = _bounded_string(tag, name="episode tag", maximum=MAX_MEMORY_TAG_BYTES)
        lesson = normalized["lesson"]
        if lesson is not None:
            normalized["lesson"] = _bounded_string(lesson, name="episode.lesson", maximum=MAX_MEMORY_LESSON_BYTES)
            normalized["lesson"] = _safe_value(normalized["lesson"])
        packet_json = _canonical(normalized)
        if len(packet_json.encode("utf-8")) > MAX_MEMORY_PROVENANCE_BYTES:
            raise BrainMemoryError("episode packet exceeds the bounded size")
        return normalized

    def _normalize_evaluation(self, evaluation: Mapping[str, Any]) -> dict[str, Any]:
        if not isinstance(evaluation, Mapping):
            raise BrainMemoryError("evaluation must be a mapping")
        if any(not isinstance(key, str) for key in evaluation):
            raise BrainMemoryError("evaluation keys must be strings")
        unknown = sorted(set(evaluation).difference(self._EVALUATION_FIELDS))
        if unknown:
            raise BrainMemoryError("evaluation contains unsupported fields: " + ", ".join(unknown))
        normalized = _safe_value(dict(evaluation))
        if not isinstance(normalized, Mapping):
            raise BrainMemoryError("evaluation must be a mapping")
        for field in ("evaluator_id", "evaluator_version"):
            if field in normalized:
                _bounded_string(normalized[field], name=f"evaluation.{field}", maximum=MAX_MEMORY_LABEL_BYTES)
        reward = normalized.get("reward")
        if reward is not None and (
            not isinstance(reward, (int, float)) or isinstance(reward, bool) or not math.isfinite(float(reward))
        ):
            raise BrainMemoryError("evaluation.reward must be finite")
        for field in ("passed", "failed", "replan_requested"):
            if field in normalized and not isinstance(normalized[field], bool):
                raise BrainMemoryError(f"evaluation.{field} must be boolean")
        for field in (
            "feedback_digest",
            "evidence_digest",
            "decision_digest",
            "replan_instruction_digest",
        ):
            if field in normalized and normalized[field] is not None and not _valid_digest(normalized[field]):
                raise BrainMemoryError(f"evaluation.{field} must be a lowercase SHA-256 digest")
        if "replan_instruction" in normalized and normalized["replan_instruction"] is not None:
            normalized["replan_instruction"] = _bounded_string(
                normalized["replan_instruction"],
                name="evaluation.replan_instruction",
                maximum=MAX_MEMORY_LESSON_BYTES,
            )
        encoded = _canonical(normalized).encode("utf-8")
        if len(encoded) > MAX_MEMORY_PROVENANCE_BYTES:
            raise BrainMemoryError("evaluation exceeds the bounded size")
        return dict(normalized)

    def _project_row(self, row: sqlite3.Row) -> dict[str, Any]:
        try:
            packet = json.loads(row["packet_json"])
            evaluation = None if row["evaluation_json"] is None else json.loads(row["evaluation_json"])
        except (TypeError, ValueError, json.JSONDecodeError) as error:
            raise BrainMemoryError("episodic memory index contains invalid JSON") from error
        return {
            "schema": MEMORY_SCHEMA,
            "episode_id": row["episode_id"],
            "run_id": row["run_id"],
            "result_kind": row["result_kind"],
            "status": row["status"],
            "task_digest": row["task_digest"],
            "context": packet.get("context", {}),
            "selected_model": packet.get("selected_model", {}),
            "digests": packet.get("digests", {}),
            "route": packet.get("route", {}),
            "tags": packet.get("tags", []),
            "lesson": packet.get("lesson"),
            "evaluation": evaluation,
            "provenance": {
                "record_sequence": row["record_sequence"],
                "record_digest": row["record_digest"],
                "created_ns": row["created_ns"],
                "updated_ns": row["updated_ns"],
                "retention": "metadata_and_digests_only",
            },
        }

    def _begin_locked(self) -> None:
        try:
            self._connection.execute("BEGIN IMMEDIATE")
        except sqlite3.Error as error:
            raise BrainMemoryError("could not begin episodic memory transaction") from error

    def _append_event_locked(self, *, event_type: str, episode_id: str, payload_json: str) -> MemoryReceipt:
        previous = self._head_locked()
        sequence = int(
            self._connection.execute("SELECT COALESCE(MAX(sequence), 0) + 1 FROM memory_events").fetchone()[0]
        )
        created_ns = self._now_ns()
        envelope = {
            "schema": MEMORY_EVENT_SCHEMA,
            "event_type": event_type,
            "episode_id": episode_id,
            "payload": json.loads(payload_json),
            "previous_digest": previous,
            "sequence": sequence,
            "created_ns": created_ns,
        }
        event_digest = _digest(envelope)
        try:
            self._connection.execute(
                "INSERT INTO memory_events (sequence, event_type, episode_id, payload_json, previous_digest, event_digest, created_ns) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (sequence, event_type, episode_id, payload_json, previous, event_digest, created_ns),
            )
        except sqlite3.Error as error:
            raise BrainMemoryError("could not append episodic memory event") from error
        return MemoryReceipt(
            event_type=event_type,
            episode_id=episode_id,
            sequence=sequence,
            event_digest=event_digest,
            head_digest=event_digest,
        )

    def _head_locked(self) -> str:
        row = self._connection.execute(
            "SELECT event_digest FROM memory_events ORDER BY sequence DESC LIMIT 1"
        ).fetchone()
        return "" if row is None else str(row["event_digest"])

    def _now_ns(self) -> int:
        try:
            value = float(self._clock())
        except Exception as error:
            raise BrainMemoryError("memory clock failed") from error
        if not math.isfinite(value):
            raise BrainMemoryError("memory clock returned a non-finite value")
        return int(value * 1_000_000_000)

    def _ensure_capacity_locked(self) -> None:
        page_count = int(self._connection.execute("PRAGMA page_count").fetchone()[0])
        page_size = int(self._connection.execute("PRAGMA page_size").fetchone()[0])
        if page_count * page_size > self.max_bytes:
            raise BrainMemoryError("episodic memory byte capacity is exhausted")
