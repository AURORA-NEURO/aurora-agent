"""Transactional persistence for value-only brain learning records.

The JSONL :class:`~prism_sdk.brain.BrainLearningLedger` remains a useful dependency-free
single-process store.  This module adds the production-oriented SQLite equivalent while keeping
the exact ledger interface used by :class:`~prism_sdk.autonomy.AutonomousAgent` and
:class:`~prism_sdk.brain.AutonomousBrain`.

Only the canonical learning record projection is stored.  SQLite rows contain evaluator evidence
metadata, bandit state, episode/replay digests, and a content digest.  The transaction boundary is
``BEGIN IMMEDIATE`` with ``synchronous=FULL`` so two worker processes cannot interleave a reward
update or an episode identity check.  No provider transcript, prompt, response, credential,
header, tool argument, or raw evidence value is accepted by this layer beyond the parent ledger's
value-only validation contract.
"""

from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import sqlite3
import threading
import time
from typing import Any, Iterator, Mapping

from .brain import (
    BRAIN_LEARNING_SNAPSHOT_SCHEMA,
    MAX_BRAIN_LEARNING_EPISODE_BYTES,
    MAX_BRAIN_REPLAY_BYTES,
    BrainLearningEpisode,
    BrainLearningLedger,
    BrainRunError,
    _normalize_learning_snapshot,
    _validate_learning_ledger_row,
)


SQLITE_BRAIN_LEARNING_SCHEMA = "bioprism-brain-learning-sqlite/0.1"


class SQLiteBrainLearningLedger(BrainLearningLedger):
    """Restart-safe, concurrent value-only implementation of ``BrainLearningLedger``.

    The class subclasses the JSONL ledger deliberately: all consumers that require a
    ``BrainLearningLedger`` continue to accept this implementation, while query helpers such as
    ``pending_episodes()``, ``latest_state()``, ``contextual_state()``, and ``replays()`` reuse the
    canonical parent projections over this class's verified ``records()`` result.
    """

    def __init__(
        self,
        path: str | Path,
        *,
        max_records: int = 4096,
        max_bytes: int = 32_000_000,
        clock: Any = time.time,
    ) -> None:
        super().__init__(path, max_records=max_records, max_bytes=max_bytes)
        if not callable(clock):
            raise BrainRunError("SQLite learning ledger clock must be callable")
        self._clock = clock
        self._connection: sqlite3.Connection | None = None
        self._sqlite_lock = threading.RLock()
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            connection = sqlite3.connect(
                str(self.path),
                isolation_level=None,
                check_same_thread=False,
            )
            connection.row_factory = sqlite3.Row
            self._connection = connection
            self._configure()
        except (OSError, sqlite3.Error) as error:
            if self._connection is not None:
                self._connection.close()
                self._connection = None
            raise BrainRunError("SQLite learning ledger could not be opened") from error

    def _configure(self) -> None:
        connection = self._require_connection()
        try:
            connection.execute("PRAGMA journal_mode=WAL")
            connection.execute("PRAGMA synchronous=FULL")
            connection.execute("PRAGMA foreign_keys=ON")
            connection.execute(
                """
                CREATE TABLE IF NOT EXISTS brain_learning_meta (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                )
                """
            )
            connection.execute(
                """
                CREATE TABLE IF NOT EXISTS brain_learning_records (
                    record_index INTEGER PRIMARY KEY AUTOINCREMENT,
                    record_digest TEXT NOT NULL,
                    record_json TEXT NOT NULL,
                    context_digest TEXT,
                    record_type TEXT NOT NULL,
                    episode_id TEXT,
                    created_at REAL NOT NULL
                )
                """
            )
            connection.execute(
                "CREATE INDEX IF NOT EXISTS brain_learning_episode_idx "
                "ON brain_learning_records(record_type, episode_id)"
            )
            existing = connection.execute(
                "SELECT value FROM brain_learning_meta WHERE key = 'schema'"
            ).fetchone()
            if existing is None:
                connection.execute(
                    "INSERT INTO brain_learning_meta(key, value) VALUES('schema', ?)",
                    (SQLITE_BRAIN_LEARNING_SCHEMA,),
                )
            elif existing["value"] != SQLITE_BRAIN_LEARNING_SCHEMA:
                raise BrainRunError("SQLite learning ledger has an incompatible schema")
        except sqlite3.Error as error:
            raise BrainRunError("SQLite learning ledger schema could not be initialized") from error

    def _require_connection(self) -> sqlite3.Connection:
        connection = self._connection
        if connection is None:
            raise BrainRunError("SQLite learning ledger is closed")
        return connection

    @contextmanager
    def _transaction(self) -> Iterator[sqlite3.Connection]:
        with self._sqlite_lock:
            connection = self._require_connection()
            try:
                connection.execute("BEGIN IMMEDIATE")
                yield connection
                connection.execute("COMMIT")
            except Exception:
                try:
                    connection.execute("ROLLBACK")
                except sqlite3.Error:
                    pass
                raise

    @staticmethod
    def _encode_record(record: Mapping[str, Any]) -> tuple[str, bytes, str]:
        try:
            encoded = json.dumps(
                dict(record),
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
                allow_nan=False,
            ).encode("utf-8")
        except (TypeError, ValueError) as error:
            raise BrainRunError("SQLite learning record must be JSON-safe") from error
        digest = hashlib.sha256(encoded).hexdigest()
        return encoded.decode("utf-8"), encoded, digest

    def _capacity(self, connection: sqlite3.Connection, encoded_bytes: int) -> int:
        row = connection.execute(
            "SELECT COUNT(*) AS count, COALESCE(SUM(LENGTH(CAST(record_json AS BLOB))), 0) AS bytes "
            "FROM brain_learning_records"
        ).fetchone()
        count = int(row["count"])
        current_bytes = int(row["bytes"])
        if count >= self.max_records:
            raise BrainRunError("learning ledger record capacity is exhausted")
        if current_bytes + encoded_bytes > self.max_bytes:
            raise BrainRunError("learning ledger capacity is exhausted")
        return count

    def _insert(
        self,
        connection: sqlite3.Connection,
        record: Mapping[str, Any],
        encoded_text: str,
        encoded_bytes: bytes,
        digest: str,
        *,
        context_digest: str | None = None,
        record_type: str = "learning_outcome",
        episode_id: str | None = None,
    ) -> dict[str, Any]:
        record_index = self._capacity(connection, len(encoded_bytes))
        try:
            connection.execute(
                "INSERT INTO brain_learning_records "
                "(record_digest, record_json, context_digest, record_type, episode_id, created_at) "
                "VALUES (?, ?, ?, ?, ?, ?)",
                (
                    digest,
                    encoded_text,
                    context_digest,
                    record_type,
                    episode_id,
                    float(self._clock()),
                ),
            )
        except sqlite3.Error as error:
            raise BrainRunError("SQLite learning record could not be appended") from error
        return {"record_index": record_index, "record_digest": digest}

    def append(
        self,
        report: Mapping[str, Any],
        *,
        context_digest: str | None = None,
        replay: Mapping[str, Any] | None = None,
    ) -> dict[str, Any]:
        if not isinstance(report, Mapping):
            raise BrainRunError("learning ledger report must be an object")
        evidence = report.get("learning_evidence")
        next_state = report.get("next_state")
        if not isinstance(evidence, Mapping) or not isinstance(next_state, Mapping):
            raise BrainRunError("learning ledger report must contain evidence and next_state")
        if context_digest is not None and not isinstance(context_digest, str):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        if context_digest is not None and (
            len(context_digest) != 64 or any(character not in "0123456789abcdef" for character in context_digest)
        ):
            raise BrainRunError("context_digest must be a lowercase SHA-256 digest")
        self._assert_safe(report)
        if replay is not None:
            if not isinstance(replay, Mapping):
                raise BrainRunError("learning ledger replay must be an object")
            self._assert_safe(replay)
            _replay_text, replay_bytes, _replay_digest = self._encode_record(replay)
            if len(replay_bytes) > MAX_BRAIN_REPLAY_BYTES:
                raise BrainRunError("learning ledger replay exceeds the bounded size")
        record: dict[str, Any] = {"learning_evidence": dict(evidence), "next_state": dict(next_state)}
        if context_digest is not None:
            record["context_digest"] = context_digest
        if replay is not None:
            record["replay"] = dict(replay)
        payload = {"schema": self._SCHEMA, "record": record}
        encoded_text, encoded_bytes, digest = self._encode_record(payload)
        if len(encoded_bytes) > self.max_bytes:
            raise BrainRunError("learning ledger record exceeds max_bytes")
        with self._transaction() as connection:
            receipt = self._insert(
                connection,
                payload,
                encoded_text,
                encoded_bytes,
                digest,
                context_digest=context_digest,
            )
        self._invalidate_snapshot_cache()
        return {
            "schema": self._SCHEMA,
            **receipt,
            "evidence_digest": evidence.get("evidence_digest"),
            "replay_digest": None if replay is None else hashlib.sha256(
                json.dumps(dict(replay), ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")
            ).hexdigest(),
        }

    def begin_episode(self, episode: BrainLearningEpisode | Mapping[str, Any]) -> dict[str, Any]:
        normalized = episode if isinstance(episode, BrainLearningEpisode) else BrainLearningEpisode.from_mapping(episode)
        payload = normalized.to_dict()
        self._assert_safe(payload)
        record = {"record_type": "pending_episode", "episode": payload}
        encoded_record_text, encoded_record_bytes, _record_digest = self._encode_record(record)
        if len(encoded_record_bytes) > MAX_BRAIN_LEARNING_EPISODE_BYTES:
            raise BrainRunError("learning episode record exceeds the bounded size")
        envelope = {"schema": self._SCHEMA, "record": record}
        encoded_text, encoded_bytes, digest = self._encode_record(envelope)
        with self._transaction() as connection:
            prior = connection.execute(
                "SELECT record_index, record_digest, record_json FROM brain_learning_records "
                "WHERE record_type = 'pending_episode' AND episode_id = ? "
                "ORDER BY record_index DESC LIMIT 1",
                (normalized.episode_id,),
            ).fetchone()
            if prior is not None:
                prior_payload = self._decode_verified_row(prior)
                if prior_payload.get("record") != record:
                    raise BrainRunError("learning episode identity is already bound to different content")
                return {
                    "schema": self._SCHEMA,
                    "record_index": int(prior["record_index"]) - 1,
                    "record_digest": prior["record_digest"],
                    "episode_id": normalized.episode_id,
                    "idempotent": True,
                }
            receipt = self._insert(
                connection,
                envelope,
                encoded_text,
                encoded_bytes,
                digest,
                record_type="pending_episode",
                episode_id=normalized.episode_id,
            )
        self._invalidate_snapshot_cache()
        return {
            "schema": self._SCHEMA,
            **receipt,
            "episode_id": normalized.episode_id,
            "idempotent": False,
        }

    def _decode_verified_row(self, row: sqlite3.Row) -> dict[str, Any]:
        raw_text = row["record_json"]
        if not isinstance(raw_text, str):
            raise BrainRunError("SQLite learning ledger contains a malformed record")
        raw_bytes = raw_text.encode("utf-8")
        if hashlib.sha256(raw_bytes).hexdigest() != row["record_digest"]:
            raise BrainRunError("SQLite learning ledger record digest mismatch")
        try:
            value = json.loads(raw_text)
        except json.JSONDecodeError as error:
            raise BrainRunError("SQLite learning ledger contains invalid JSON") from error
        if not isinstance(value, Mapping) or value.get("schema") != self._SCHEMA:
            raise BrainRunError("SQLite learning ledger contains an invalid schema")
        record = value.get("record")
        if not isinstance(record, Mapping):
            raise BrainRunError("SQLite learning ledger record is missing its projection")
        return _validate_learning_ledger_row(value)

    def records(self) -> list[dict[str, Any]]:
        with self._sqlite_lock:
            connection = self._require_connection()
            try:
                rows = connection.execute(
                    "SELECT record_index, record_digest, record_json FROM brain_learning_records "
                    "ORDER BY record_index ASC"
                ).fetchall()
            except sqlite3.Error as error:
                raise BrainRunError("SQLite learning ledger records could not be read") from error
            if len(rows) > self.max_records:
                raise BrainRunError("SQLite learning ledger exceeds max_records")
            return [self._decode_verified_row(row) for row in rows]

    def snapshot(self) -> dict[str, Any]:
        """Export the same portable projection as the JSONL ledger."""

        with self._sqlite_lock:
            return self._snapshot_for_rows(self.records())

    def restore(self, snapshot: Mapping[str, Any]) -> None:
        """Atomically replace SQLite rows while preserving portable record indices."""

        normalized = _normalize_learning_snapshot(
            snapshot,
            max_records=self.max_records,
            max_bytes=self.max_bytes,
        )
        with self._transaction() as connection:
            try:
                connection.execute("DELETE FROM brain_learning_records")
                # The public ledger contract exposes zero-based contiguous record indices. Reset
                # SQLite's AUTOINCREMENT sequence so a restore followed by a new append preserves
                # that contract even when replacing a previously longer database.
                connection.execute(
                    "DELETE FROM sqlite_sequence WHERE name = 'brain_learning_records'"
                )
                for index, (row, expected_digest) in enumerate(
                    zip(normalized["records"], normalized["record_digests"]),
                    start=1,
                ):
                    encoded_text, _encoded_bytes, digest = self._encode_record(row)
                    if digest != expected_digest:
                        raise BrainRunError("SQLite learning snapshot record digest changed during restore")
                    record = row["record"]
                    record_type = "pending_episode" if record.get("record_type") == "pending_episode" else "learning_outcome"
                    episode = record.get("episode")
                    episode_id = episode.get("episode_id") if isinstance(episode, Mapping) else None
                    context_digest = record.get("context_digest")
                    connection.execute(
                        "INSERT INTO brain_learning_records "
                        "(record_index, record_digest, record_json, context_digest, record_type, episode_id, created_at) "
                        "VALUES (?, ?, ?, ?, ?, ?, ?)",
                        (
                            index,
                            digest,
                            encoded_text,
                            context_digest,
                            record_type,
                            episode_id,
                            float(self._clock()),
                        ),
                    )
                self._snapshot_generation = int(normalized.get("snapshot_generation", 0))
                self._previous_snapshot_digest = normalized["snapshot_digest"] if self._snapshot_generation else None
                if normalized.get("schema") == BRAIN_LEARNING_SNAPSHOT_SCHEMA:
                    self._snapshot_cache = normalized
                    self._snapshot_cache_record_digests = tuple(normalized["record_digests"])
                else:
                    self._invalidate_snapshot_cache()
            except sqlite3.Error as error:
                raise BrainRunError("SQLite learning snapshot could not be restored") from error

    def close(self) -> None:
        with self._sqlite_lock:
            connection, self._connection = self._connection, None
            if connection is not None:
                try:
                    connection.close()
                except sqlite3.Error as error:
                    raise BrainRunError("SQLite learning ledger could not be closed") from error

    def __enter__(self) -> "SQLiteBrainLearningLedger":
        self._require_connection()
        return self

    def __exit__(self, *_args: object) -> None:
        self.close()


__all__ = ["SQLITE_BRAIN_LEARNING_SCHEMA", "SQLiteBrainLearningLedger"]
