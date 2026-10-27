"""Application-owned JSON publication, separate from read-only workflow execution.

This repository-defined contract has no blueprint implementation claim. It binds one exact
artifact manifest to a caller grant, persists grant consumption with CAS, and uses the existing
SQLite effect boundary before publishing. It provides no general effectful workflow operation,
remote sink, secret storage, distributed exactly-once execution, or hostile-filesystem isolation.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass, field
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import stat
import threading
from typing import Any, Callable, Iterator, Mapping

from .authoring import canonical_json, content_digest
from .autonomous_authorization import (
    AutonomousAuthorizationContext, AutonomousAuthorizationPersistenceCoordinator,
    TransactionalJsonAutonomousAuthorizationSnapshotPersistence,
)
from .autonomous_effects import (
    AutonomousEffectBoundary, AutonomousEffectError, AutonomousEffectRequest,
    AutonomousEffectResolution, SQLiteAutonomousEffectJournal,
)
from .autonomy_persistence import AutonomousExecutionController, AutonomousExecutionPolicy

WORKFLOW_ARTIFACT_MANIFEST_SCHEMA = "aurora-workflow-artifact-manifest/0.1"
WORKFLOW_ARTIFACT_RECEIPT_SCHEMA = "aurora-workflow-artifact-receipt/0.1"
MAX_WORKFLOW_ARTIFACT_BYTES = 1_000_000
_OPERATION = "local.artifact.write_json"
_CAPABILITY = "workflow_artifact_write"
_RISK = "local_artifact_write"


class WorkflowArtifactError(AutonomousEffectError):
    """The scoped publication contract could not be established."""


class WorkflowArtifactCancelled(WorkflowArtifactError):
    """Cancellation was observed before publication."""


class SQLiteWorkflowArtifactAuthorizationStore:
    """Durable metadata-only CAS text store for the existing authorization coordinator."""
    def __init__(self, path: str | Path) -> None:
        if (not isinstance(path, (str, Path)) or not str(path) or str(path) == ":memory:"
                or not Path(path).is_absolute()):
            raise WorkflowArtifactError("artifact authorization store requires an absolute persistent path")
        self._lock = threading.RLock()
        connection = None
        try:
            connection = sqlite3.connect(str(path), isolation_level=None, check_same_thread=False)
            connection.execute("PRAGMA journal_mode=WAL")
            connection.execute("PRAGMA synchronous=FULL")
            connection.execute("PRAGMA busy_timeout=5000")
            connection.execute("CREATE TABLE IF NOT EXISTS workflow_artifact_authorization "
                               "(singleton INTEGER PRIMARY KEY CHECK(singleton=1), value TEXT NOT NULL)")
            self._connection = connection
        except sqlite3.Error as error:
            if connection is not None:
                connection.close()
            raise WorkflowArtifactError("artifact authorization store could not be initialized") from error

    def read(self) -> str | None:
        with self._lock:
            row = self._connection.execute("SELECT value FROM workflow_artifact_authorization WHERE singleton=1").fetchone()
            return None if row is None else row[0]

    def write(self, value: str) -> None:
        raise WorkflowArtifactError("artifact authorization requires compare-and-set")

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        with self._lock:
            try:
                self._connection.execute("BEGIN IMMEDIATE")
                current = self.read()
                observed = None if current is None else json.loads(current)["snapshot_digest"]
                if observed != expected_snapshot_digest:
                    self._connection.execute("ROLLBACK")
                    return False
                self._connection.execute("INSERT INTO workflow_artifact_authorization(singleton,value) VALUES(1,?) "
                                         "ON CONFLICT(singleton) DO UPDATE SET value=excluded.value", (value,))
                self._connection.execute("COMMIT")
                return True
            except BaseException:
                if self._connection.in_transaction:
                    self._connection.execute("ROLLBACK")
                raise

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def __enter__(self) -> "SQLiteWorkflowArtifactAuthorizationStore":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise WorkflowArtifactError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _json_bytes(value: Any, maximum: int) -> bytes:
    # Check structure before canonicalization so cycles/depth/large inputs fail predictably.
    nodes = 0
    def walk(item: Any, depth: int) -> None:
        nonlocal nodes
        nodes += 1
        if nodes > 100_000 or depth > 32:
            raise WorkflowArtifactError("artifact JSON structure exceeds its bound")
        if isinstance(item, dict):
            for key, child in item.items():
                if not isinstance(key, str):
                    raise WorkflowArtifactError("artifact JSON keys must be strings")
                walk(key, depth + 1)
                walk(child, depth + 1)
        elif isinstance(item, list):
            for child in item:
                walk(child, depth + 1)
        elif item is not None and type(item) not in (str, bool, int, float):
            raise WorkflowArtifactError("artifact value must be strict JSON")
        if isinstance(item, str) and len(item) > maximum:
            raise WorkflowArtifactError("artifact JSON string exceeds its bound")
    try:
        walk(value, 0)
        encoded = canonical_json(value).encode("utf-8")
    except (TypeError, ValueError, UnicodeError, RecursionError) as error:
        raise WorkflowArtifactError("artifact value must be bounded canonical JSON") from error
    if len(encoded) > maximum:
        raise WorkflowArtifactError("artifact JSON exceeds its byte bound")
    return encoded


def _regular(info: os.stat_result) -> bool:
    return stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and not (
        getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 1024)
    )


def _identity(info: os.stat_result) -> tuple[int, int]:
    return info.st_dev, info.st_ino


@dataclass(frozen=True, slots=True)
class PreparedWorkflowArtifact:
    """Transient canonical payload plus immutable metadata; retain the payload separately."""
    _manifest_json: str = field(repr=False)
    _content: bytes = field(repr=False)

    def __post_init__(self) -> None:
        if (type(self._manifest_json) is not str or len(self._manifest_json) > 16_384
                or type(self._content) is not bytes or len(self._content) > MAX_WORKFLOW_ARTIFACT_BYTES):
            raise WorkflowArtifactError("prepared artifact requires bounded immutable manifest text and content bytes")
        try:
            manifest = json.loads(self._manifest_json)
            if (type(manifest) is not dict or len(self._manifest_json.encode("utf-8")) > 16_384
                    or _json_bytes(manifest, 16_384).decode("utf-8") != self._manifest_json):
                raise WorkflowArtifactError("prepared artifact manifest must be a canonical JSON object")
        except (TypeError, ValueError, UnicodeError, RecursionError) as error:
            raise WorkflowArtifactError("prepared artifact manifest must be a bounded canonical JSON object") from error

    @property
    def manifest_digest(self) -> str:
        return hashlib.sha256(self._manifest_json.encode("utf-8")).hexdigest()

    def manifest(self) -> dict[str, Any]:
        return json.loads(self._manifest_json)


class WorkflowArtifactSink:
    """One application-chosen existing private root and its durable effect boundary.

    The application must exclude untrusted writers from the root and its ancestors. An OS lock
    serializes cooperating sink processes; it is not a distributed lease or filesystem sandbox.
    """
    def __init__(self, root: str | Path, *, journal: SQLiteAutonomousEffectJournal,
                 authorization_persistence: AutonomousAuthorizationPersistenceCoordinator,
                 max_content_bytes: int = MAX_WORKFLOW_ARTIFACT_BYTES) -> None:
        if (not isinstance(journal, SQLiteAutonomousEffectJournal) or journal.path == ":memory:"
                or not Path(journal.path).is_absolute()):
            raise WorkflowArtifactError("artifact sink requires a persistent SQLite effect journal")
        if (not isinstance(authorization_persistence, AutonomousAuthorizationPersistenceCoordinator)
                or not isinstance(authorization_persistence.persistence,
                                  TransactionalJsonAutonomousAuthorizationSnapshotPersistence)):
            raise WorkflowArtifactError("artifact sink requires transactional CAS authorization persistence")
        if type(max_content_bytes) is not int or not 1 <= max_content_bytes <= MAX_WORKFLOW_ARTIFACT_BYTES:
            raise WorkflowArtifactError("artifact byte bound is outside its supported range")
        raw = Path(root)
        if not raw.is_absolute() or ".." in raw.parts:
            raise WorkflowArtifactError("artifact root must be an absolute existing directory")
        self._root = Path(os.path.abspath(raw))
        self._anchors = self._anchor_identities()
        self._root_digest = content_digest({"path": os.path.normcase(str(self._root)),
                                            "identities": [list(item) for item in self._anchors]})
        self._maximum = max_content_bytes
        self._persistence = authorization_persistence
        self._boundary = AutonomousEffectBoundary(journal=journal)

    def _anchor_identities(self) -> tuple[tuple[int, int], ...]:
        identities = []
        for path in (*reversed(self._root.parents), self._root):
            try:
                info = path.lstat()
            except OSError as error:
                raise WorkflowArtifactError("artifact root could not be inspected") from error
            if (not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode)
                    or getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 1024)):
                raise WorkflowArtifactError("artifact root and ancestors must be ordinary directories")
            identities.append(_identity(info))
        return tuple(identities)

    def _check_root(self) -> None:
        if self._anchor_identities() != self._anchors:
            raise WorkflowArtifactError("artifact root identity changed")

    @contextmanager
    def _exclusive(self) -> Iterator[None]:
        self._check_root()
        lock_path = self._root / ".aurora-write.lock"
        descriptor = None
        locked = False
        try:
            descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT | getattr(os, "O_NOFOLLOW", 0), 0o600)
            info = os.fstat(descriptor)
            if not _regular(info) or _identity(lock_path.lstat()) != _identity(info):
                raise WorkflowArtifactError("artifact writer lock is unsafe")
            if os.name == "nt":
                import msvcrt
                if info.st_size == 0:
                    os.write(descriptor, b"0")
                os.lseek(descriptor, 0, os.SEEK_SET)
                msvcrt.locking(descriptor, msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            locked = True
            self._check_root()
            yield
        except OSError as error:
            raise WorkflowArtifactError("artifact writer lock or filesystem operation unavailable") from error
        finally:
            if descriptor is not None:
                if locked:
                    if os.name == "nt":
                        import msvcrt
                        os.lseek(descriptor, 0, os.SEEK_SET)
                        msvcrt.locking(descriptor, msvcrt.LK_UNLCK, 1)
                    else:
                        import fcntl
                        fcntl.flock(descriptor, fcntl.LOCK_UN)
                os.close(descriptor)

    def prepare(self, *, plan_digest: str, task_digest: str, catalog_digest: str,
                step_id: str, resolved_arguments: Mapping[str, Any], filename: str,
                value: Any, policy: AutonomousExecutionPolicy) -> PreparedWorkflowArtifact:
        self._check_root()
        if (not isinstance(filename, str) or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,120}\.json", filename) is None
                or filename.split(".", 1)[0].upper() in {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(10)), *(f"LPT{i}" for i in range(10))}):
            raise WorkflowArtifactError("artifact filename must be one ordinary JSON basename")
        if not isinstance(step_id, str) or re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", step_id) is None:
            raise WorkflowArtifactError("artifact step_id must be a bounded identifier")
        if not isinstance(resolved_arguments, Mapping):
            raise WorkflowArtifactError("artifact resolved arguments must be an object")
        if not isinstance(policy, AutonomousExecutionPolicy):
            raise WorkflowArtifactError("artifact policy must be an AutonomousExecutionPolicy")
        content = _json_bytes(value, self._maximum)
        arguments = _json_bytes(dict(resolved_arguments), self._maximum)
        manifest = {"schema": WORKFLOW_ARTIFACT_MANIFEST_SCHEMA,
                    "operation": _OPERATION, "plan_digest": _digest("plan_digest", plan_digest),
                    "task_digest": _digest("task_digest", task_digest),
                    "catalog_digest": _digest("catalog_digest", catalog_digest), "step_id": step_id,
                    "resolved_arguments_digest": hashlib.sha256(arguments).hexdigest(),
                    "root_digest": self._root_digest, "filename": filename,
                    "content_digest": hashlib.sha256(content).hexdigest(), "content_bytes": len(content),
                    "policy_digest": policy.digest, "overwrite": "refuse_existing"}
        return PreparedWorkflowArtifact(canonical_json(manifest), content)

    def _validated(self, prepared: PreparedWorkflowArtifact) -> dict[str, Any]:
        if type(prepared) is not PreparedWorkflowArtifact:
            raise WorkflowArtifactError("artifact must be a prepared manifest")
        prepared.__post_init__()
        manifest = prepared.manifest()
        # Reconstruct through the only manifest compiler, rejecting edited/forged fields or bytes.
        expected = {"schema", "operation", "plan_digest", "task_digest", "catalog_digest", "step_id",
                    "resolved_arguments_digest", "root_digest", "filename", "content_digest", "content_bytes",
                    "policy_digest", "overwrite"}
        if (set(manifest) != expected or canonical_json(manifest) != prepared._manifest_json
                or manifest["schema"] != WORKFLOW_ARTIFACT_MANIFEST_SCHEMA or manifest["operation"] != _OPERATION
                or manifest["root_digest"] != self._root_digest or manifest["overwrite"] != "refuse_existing"
                or type(manifest["content_bytes"]) is not int or manifest["content_bytes"] != len(prepared._content)
                or len(prepared._content) > self._maximum
                or hashlib.sha256(prepared._content).hexdigest() != manifest["content_digest"]):
            raise WorkflowArtifactError("prepared artifact does not match its exact manifest")
        try:
            if _json_bytes(json.loads(prepared._content), self._maximum) != prepared._content:
                raise WorkflowArtifactError("prepared artifact content must be canonical JSON")
        except (TypeError, ValueError, UnicodeError, RecursionError) as error:
            raise WorkflowArtifactError("prepared artifact content must be bounded canonical JSON") from error
        for name in ("plan_digest", "task_digest", "catalog_digest", "resolved_arguments_digest", "policy_digest"):
            _digest(name, manifest[name])
        filename = manifest["filename"]
        if (not isinstance(filename, str) or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,120}\.json", filename) is None
                or filename.split(".", 1)[0].upper() in {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(10)), *(f"LPT{i}" for i in range(10))}):
            raise WorkflowArtifactError("prepared artifact filename is invalid")
        if not isinstance(manifest["step_id"], str) or re.fullmatch(r"[A-Za-z0-9_.:-]{1,128}", manifest["step_id"]) is None:
            raise WorkflowArtifactError("prepared artifact step is invalid")
        self._check_root()
        return manifest

    def _request(self, prepared: PreparedWorkflowArtifact, manifest: Mapping[str, Any]) -> AutonomousEffectRequest:
        return AutonomousEffectRequest(tool=_OPERATION, call_id="step." + content_digest(manifest["step_id"]),
            risk_class=_RISK, arguments={"manifest_digest": prepared.manifest_digest},
            execution_id="workflow." + manifest["plan_digest"])

    def _receipt(self, prepared: PreparedWorkflowArtifact, manifest: Mapping[str, Any]) -> dict[str, Any]:
        return {"schema": WORKFLOW_ARTIFACT_RECEIPT_SCHEMA, "status": "completed",
                "manifest_digest": prepared.manifest_digest, "content_digest": manifest["content_digest"],
                "content_bytes": manifest["content_bytes"], "root_digest": self._root_digest,
                "effect_id": self._boundary.effect_id(self._request(prepared, manifest)),
                "observation": "exact_canonical_bytes_in_application_root", "overwrite": "refuse_existing"}

    def _stage(self, prepared: PreparedWorkflowArtifact) -> Path:
        return self._root / (".aurora-stage-" + prepared.manifest_digest + ".tmp")

    def _inspect_locked(self, prepared: PreparedWorkflowArtifact, manifest: Mapping[str, Any]) -> dict[str, Any]:
        self._check_root()
        path = self._root / manifest["filename"]
        try:
            info = path.lstat()
        except FileNotFoundError:
            # Staging residue proves an interrupted publisher; absence is not yet retry-safe.
            return {"status": "unknown" if os.path.lexists(self._stage(prepared)) else "not_found",
                    "retry_safe": False, "manifest_digest": prepared.manifest_digest}
        except OSError:
            return {"status": "unknown", "retry_safe": False, "manifest_digest": prepared.manifest_digest}
        if not _regular(info) or info.st_size != len(prepared._content):
            return {"status": "unknown", "retry_safe": False, "manifest_digest": prepared.manifest_digest}
        try:
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0))
            with os.fdopen(descriptor, "rb") as stream:
                opened = os.fstat(stream.fileno())
                data = stream.read(self._maximum + 1)
                final = os.fstat(stream.fileno())
            self._check_root()
            if (not _regular(opened) or _identity(opened) != _identity(info)
                    or (opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns) != (final.st_size, final.st_mtime_ns, final.st_ctime_ns)
                    or _identity(path.lstat()) != _identity(info) or data != prepared._content):
                raise WorkflowArtifactError("artifact observation changed")
        except (OSError, WorkflowArtifactError):
            return {"status": "unknown", "retry_safe": False, "manifest_digest": prepared.manifest_digest}
        return self._receipt(prepared, manifest)

    def inspect(self, prepared: PreparedWorkflowArtifact) -> dict[str, Any]:
        """Read exact bytes without authorization use; this is observation, not grant proof."""
        manifest = self._validated(prepared)
        try:
            with self._exclusive():
                return self._inspect_locked(prepared, manifest)
        except WorkflowArtifactError:
            return {"status": "unknown", "retry_safe": False, "manifest_digest": prepared.manifest_digest}

    def reconcile(self, prepared: PreparedWorkflowArtifact) -> dict[str, Any]:
        """Settle observed completion without a grant use; absent/unknown never authorizes retry."""
        manifest = self._validated(prepared)
        with self._exclusive():
            observed = self._inspect_locked(prepared, manifest)
            effect_id = self._boundary.effect_id(self._request(prepared, manifest))
            record = self._boundary.journal.get(effect_id)
            if record is None:
                return {**observed, "journal_status": "not_found", "authorization_proven": False}
            if record.status in {"dispatching", "dispatched", "uncertain"} and observed["status"] == "completed":
                class Resolver:
                    def resolve(self, record: Any) -> AutonomousEffectResolution:
                        return AutonomousEffectResolution("completed", result=observed)
                record = self._boundary.reconcile(effect_id, Resolver())
            return {**observed, "journal_status": record.status, "authorization_proven": False}

    def publish(self, prepared: PreparedWorkflowArtifact, *, authorization_context: AutonomousAuthorizationContext,
                execution: AutonomousExecutionController, cancelled: Callable[[], bool] | None = None) -> dict[str, Any]:
        """Consume an exact manifest grant before the first admitted no-clobber publication."""
        manifest = self._validated(prepared)
        if (not isinstance(authorization_context, AutonomousAuthorizationContext)
                or authorization_context.gate.ledger is not self._persistence.ledger
                or authorization_context.authorization_digest != prepared.manifest_digest
                or authorization_context.domains != ("operations",)
                or authorization_context.capability != _CAPABILITY or authorization_context.risk_class != _RISK):
            raise WorkflowArtifactError("artifact authorization context does not bind this exact manifest and scope")
        if (not isinstance(execution, AutonomousExecutionController)
                or execution.policy.digest != manifest["policy_digest"]
                or execution.state.execution_id != "workflow." + manifest["plan_digest"]
                or execution.state.domain != "operations" or execution.state.capability != _CAPABILITY
                or execution.state.risk_class != _RISK):
            raise WorkflowArtifactError("artifact execution controller does not bind the approved policy and plan")
        if cancelled is not None and not callable(cancelled):
            raise WorkflowArtifactError("artifact cancellation check must be callable")
        persistence = self._persistence
        ledger = persistence.ledger
        gate = authorization_context.gate
        clock = authorization_context.clock
        execution_journal = execution.journal
        identity = (authorization_context.grant_id, authorization_context.tenant_id,
                    authorization_context.actor_id, authorization_context.session_id,
                    authorization_context.authorization_digest, authorization_context.domains,
                    authorization_context.capability, authorization_context.risk_class)
        def check_bindings() -> None:
            if (self._persistence is not persistence or persistence.ledger is not ledger
                    or authorization_context.gate is not gate or gate.ledger is not ledger
                    or authorization_context.clock is not clock
                    or execution.journal is not execution_journal
                    or (authorization_context.grant_id, authorization_context.tenant_id,
                        authorization_context.actor_id, authorization_context.session_id,
                        authorization_context.authorization_digest, authorization_context.domains,
                        authorization_context.capability, authorization_context.risk_class) != identity
                    or execution.policy.digest != manifest["policy_digest"]
                    or execution.state.execution_id != "workflow." + manifest["plan_digest"]
                    or execution.state.domain != "operations" or execution.state.capability != _CAPABILITY
                    or execution.state.risk_class != _RISK):
                raise WorkflowArtifactError("artifact authority or execution bindings changed")
        def check_cancel() -> None:
            if cancelled is not None:
                state_before_callback = execution.state
                stopped = cancelled()
                if type(stopped) is not bool:
                    raise WorkflowArtifactError("artifact cancellation check must return boolean")
                if execution.state is not state_before_callback:
                    raise WorkflowArtifactError("artifact execution state changed during a caller callback")
                check_bindings()
                if stopped:
                    raise WorkflowArtifactCancelled("artifact publication cancelled")
            check_bindings()
        with self._exclusive():
            check_cancel()
            request = self._request(prepared, manifest)
            effect_id = self._boundary.effect_id(request)
            # Replays use inspect/reconcile; never consume another one-use grant here.
            if self._boundary.journal.get(effect_id) is not None:
                raise WorkflowArtifactError("artifact effect already exists; inspect or reconcile before any retry decision")
            if self._inspect_locked(prepared, manifest)["status"] != "not_found":
                raise WorkflowArtifactError("artifact destination or staging state already exists")
            # Restore current durable authority before checking revocation/use. Newly issued grants must
            # have been flushed by the application; unpersisted grants confer no publication authority.
            if self._persistence.restore() is None:
                raise WorkflowArtifactError("artifact grant must be durably persisted before dispatch")
            grant = self._persistence.ledger.get(authorization_context.grant_id)
            if (grant is None or grant.allowed_domains != ("operations",)
                    or grant.allowed_operations != ("effect_dispatch",)
                    or grant.allowed_capabilities != (_CAPABILITY,)
                    or grant.allowed_risk_classes != (_RISK,) or grant.max_uses != 1):
                raise WorkflowArtifactError("artifact grant must authorize exactly one scoped effect dispatch")
            check_bindings()
            authorization_context.authorize_operation(operation="effect_dispatch", domains=("operations",),
                capability=_CAPABILITY, risk_class=_RISK, resource_digest=effect_id)
            self._persistence.flush()
            check_cancel()
            execution.admit_tool_call(tool=_OPERATION, call_id=request.call_id, read_only=False, approval_required=False)
            def write(_context: Any) -> dict[str, Any]:
                check_cancel()
                stage = self._stage(prepared)
                descriptor = os.open(stage, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_BINARY", 0)
                                     | getattr(os, "O_NOFOLLOW", 0), 0o600)
                published = False
                try:
                    with os.fdopen(descriptor, "wb") as stream:
                        stream.write(prepared._content)
                        stream.flush()
                        os.fsync(stream.fileno())
                    self._check_root()
                    check_cancel()
                    # Grant consumption was fenced before handler entry. Refresh its durable state
                    # immediately before publication so a persisted revocation cannot be ignored.
                    state_before_clock = execution.state
                    now = clock()
                    if execution.state is not state_before_clock:
                        raise WorkflowArtifactError("artifact execution state changed during a caller callback")
                    check_bindings()
                    persistence.restore()
                    check_bindings()
                    current = ledger.get(authorization_context.grant_id)
                    if (current is None or current.status == "revoked" or current.used_count != 1
                            or type(now) is not int or now < 0 or now >= current.expires_at
                            or current.authorization_digest != prepared.manifest_digest
                            or (current.tenant_id, current.actor_id, current.session_id) != identity[1:4]
                            or current.allowed_domains != ("operations",)
                            or current.allowed_operations != ("effect_dispatch",)
                            or current.allowed_capabilities != (_CAPABILITY,)
                            or current.allowed_risk_classes != (_RISK,) or current.max_uses != 1
                            or current.issued_at != grant.issued_at or current.expires_at != grant.expires_at):
                        raise WorkflowArtifactError("artifact grant is no longer valid for publication")
                    os.link(stage, self._root / manifest["filename"], follow_symlinks=False)
                    published = True
                    stage.unlink()
                    self._check_root()
                    self._after_publish()
                    observed = self._inspect_locked(prepared, manifest)
                    if observed["status"] != "completed":
                        raise WorkflowArtifactError("published artifact could not be verified")
                    return observed
                finally:
                    # A real crash may leave a stage; inspection reports uncertainty instead of erasing it.
                    if not published and stage.exists():
                        stage.unlink()
            return self._boundary.execute(request, write, execution=execution, cache_result=False)

    def _after_publish(self) -> None:
        """Private crash-test seam after durable bytes and before completion bookkeeping."""


__all__ = ["WORKFLOW_ARTIFACT_MANIFEST_SCHEMA", "WORKFLOW_ARTIFACT_RECEIPT_SCHEMA",
           "MAX_WORKFLOW_ARTIFACT_BYTES", "WorkflowArtifactError", "WorkflowArtifactCancelled",
           "PreparedWorkflowArtifact", "WorkflowArtifactSink", "SQLiteWorkflowArtifactAuthorizationStore"]
