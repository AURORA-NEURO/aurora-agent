"""Fixed stdio application boundary for public executable research workflows.

This repository-defined application interface has no blueprint module identifier. It reuses
the SDK compiler, contract validator and durable executor rather than implementing another DAG
language. It opens no listener and accepts no request-selected paths, programs or credentials.
MCP sessions, protected accounts, effectful operations, grounded synthesis, scheduling and
scientific approval are not implemented by this boundary. See docs/WORKFLOW_API_WORKER.md.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import stat
import sys
import time
from typing import Any, Iterator, Mapping
from urllib.parse import urlsplit
from uuid import uuid4

from .authoring import content_digest
from .glioma_workflow import register_glioma_workflow_operations
from .glioma_workflow_intent import compile_glioma_workflow_request, interpret_glioma_workflow_request
from .workflow_builder import (
    LocalWorkflowBuilder, WorkflowCancellationToken, WorkflowRun, WorkflowValidationError,
    audit_workflow_reference_paths, compile_workflow_plan, execute_workflow_plan, load_workflow_checkpoint,
    save_workflow_checkpoint, validate_workflow_plan,
)
from .workflow_integrations import builtin_workflow_operation_catalog
from .workflow_requirements import (audit_workflow_requirements, compile_workflow_requirements,
                                    merge_workflow_requirements, validate_workflow_requirements)

WORKFLOW_API_RESPONSE_SCHEMA = "aurora-workflow-api-response/0.1"
MAX_WORKFLOW_API_REQUEST_BYTES = 1_000_000
MAX_WORKFLOW_API_RESPONSE_BYTES = 16_000_000
MAX_WORKFLOW_API_DIAGNOSTIC_ROWS = 64
_PLAN_SCHEMA = "aurora-workflow-api-plan/0.1"
_RUN_SCHEMA = "aurora-workflow-api-run/0.1"
_ID = re.compile(r"[0-9a-f]{32}\Z")
_DIGEST = re.compile(r"[0-9a-f]{64}\Z")


class WorkflowAPIError(ValueError):
    """A stable sanitized application refusal, separate from an execution observation."""

    def __init__(self, code: str, message: str, *, diagnostics: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.code = code
        self.diagnostics = diagnostics


def _refuse(code: str, message: str) -> None:
    raise WorkflowAPIError(code, message)


def _json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":"))


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            _refuse("invalid_request", "Duplicate JSON fields are refused.")
        result[key] = value
    return result


def _fields(value: Any, required: set[str], optional: set[str] = frozenset()) -> dict[str, Any]:
    if type(value) is not dict or not required <= set(value) or set(value) - required - optional:
        _refuse("invalid_request", "Request fields do not match the selected action.")
    return value


def _boolean(value: Any) -> bool:
    if type(value) is not bool:
        _refuse("invalid_request", "Approval and control flags must be literal booleans.")
    return value


def _integer(value: Any, minimum: int, maximum: int) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        _refuse("invalid_request", "A numeric control is outside its integer bound.")
    return value


def _identity(value: Any, *, digest: bool = False) -> str:
    if not isinstance(value, str) or not (_DIGEST if digest else _ID).fullmatch(value):
        _refuse("invalid_request", "An opaque workflow identity is invalid.")
    return value


def _task(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or len(value.encode("utf-8")) > 32_000:
        _refuse("invalid_request", "Task text is empty or outside its UTF-8 byte bound.")
    return value


def _stamp(value: dict[str, Any]) -> dict[str, Any]:
    return value | {"record_digest": content_digest(value)}


def _verify(value: Any, schema: str) -> dict[str, Any]:
    if type(value) is not dict or value.get("schema") != schema:
        _refuse("storage_invalid", "A retained application record is invalid.")
    descriptor = {key: child for key, child in value.items() if key != "record_digest"}
    if value.get("record_digest") != content_digest(descriptor):
        _refuse("storage_invalid", "A retained application record failed its consistency check.")
    return descriptor


class WorkflowAPIWorker:
    """One operator-configured application root with explicit request-by-request authority.

    The root must be private to its operator. Consistency digests do not authenticate storage
    against another process with write access to that root; OS permissions establish that trust.
    """

    def __init__(self, root: str | Path, *, base_url: str = "http://127.0.0.1:11434/v1",
                 model: str | None = None, research_sources: bool = False,
                 molecular_glioma: bool = False, timeout_seconds: int = 120) -> None:
        if type(research_sources) is not bool or type(molecular_glioma) is not bool:
            _refuse("invalid_configuration", "Catalogue switches must be literal booleans.")
        if not isinstance(base_url, str):
            _refuse("invalid_configuration", "The local model endpoint is invalid.")
        endpoint = urlsplit(base_url)
        if (endpoint.hostname not in {"localhost", "127.0.0.1", "::1"}
                or endpoint.scheme not in {"http", "https"} or endpoint.username is not None
                or endpoint.password is not None or endpoint.query or endpoint.fragment):
            _refuse("invalid_configuration", "The model endpoint must be an operator-selected loopback URL.")
        if model is not None and (not isinstance(model, str) or not model.strip() or len(model) > 200):
            _refuse("invalid_configuration", "The operator model identity is invalid.")
        self.base_url, self.model = base_url, model
        self.timeout_seconds = _integer(timeout_seconds, 1, 300)
        self.root = Path(os.path.abspath(os.fspath(root)))
        self._check_ancestors(self.root.parent)
        self.root.mkdir(exist_ok=True)
        self._check_directory(self.root)
        self._root_identity = self._directory_identity(self.root)
        for name in ("plans", "runs"):
            path = self.root / name
            path.mkdir(exist_ok=True)
            self._check_directory(path)
        self.catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
        if research_sources:
            from .reviewed_research_sources import register_research_source_operations
            register_research_source_operations(self.catalog)
        if molecular_glioma:
            from .glioma_molecular_workflow import register_glioma_molecular_workflow_operations
            register_glioma_molecular_workflow_operations(self.catalog)

    @staticmethod
    def _directory_identity(path: Path) -> tuple[int, int]:
        info = path.stat()
        return info.st_dev, info.st_ino

    @staticmethod
    def _check_directory(path: Path) -> None:
        info = path.lstat()
        if (not stat.S_ISDIR(info.st_mode) or path.is_symlink()
                or getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
            _refuse("storage_invalid", "Application directories must be ordinary directories without links.")

    @classmethod
    def _check_ancestors(cls, path: Path) -> None:
        for parent in (*reversed(path.parents), path):
            cls._check_directory(parent)

    def _anchor(self) -> None:
        self._check_ancestors(self.root)
        if self._directory_identity(self.root) != self._root_identity:
            _refuse("storage_invalid", "The application root changed identity.")

    def _path(self, group: str, identity: str, leaf: str, *, create: bool = False) -> Path:
        self._anchor()
        _identity(identity)
        parent = self.root / group
        self._check_directory(parent)
        directory = parent / identity
        if create:
            directory.mkdir()
        if not directory.exists():
            _refuse("not_found", "The selected workflow record does not exist.")
        self._check_directory(directory)
        path = directory / leaf
        self._regular(path, missing=True)
        return path

    @staticmethod
    def _regular(path: Path, *, missing: bool = False) -> None:
        try:
            info = path.lstat()
        except FileNotFoundError:
            if missing:
                return
            _refuse("not_found", "The selected workflow record does not exist.")
        if (not stat.S_ISREG(info.st_mode) or path.is_symlink() or info.st_nlink != 1
                or getattr(info, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0)):
            _refuse("storage_invalid", "Application files must be regular files without aliases.")

    def _read(self, path: Path, *, maximum: int = 8_000_000) -> Any:
        self._anchor()
        self._regular(path)
        with path.open("rb") as stream:
            raw = stream.read(maximum + 1)
        if len(raw) > maximum:
            _refuse("storage_invalid", "A retained record exceeds its byte bound.")
        return json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                          parse_constant=lambda _: _refuse("storage_invalid", "Non-finite JSON is refused."))

    def _write(self, path: Path, value: Any) -> None:
        self._anchor()
        self._check_directory(path.parent)
        self._regular(path, missing=True)
        if len(_json(value).encode("utf-8")) > 8_000_000:
            _refuse("storage_invalid", "A retained record exceeds its byte bound.")
        save_workflow_checkpoint(path, value)
        self._anchor()

    @contextmanager
    def _lock(self, path: Path, *, wait_seconds: float = 0.0) -> Iterator[None]:
        if type(wait_seconds) not in {int, float} or not 0 <= wait_seconds <= 1:
            _refuse("invalid_configuration", "The internal lock wait is outside its bound.")
        self._anchor()
        self._regular(path, missing=True)
        with path.open("a+b") as stream:
            self._regular(path)
            if stream.tell() == 0:
                stream.write(b"0")
                stream.flush()
            stream.seek(0)
            deadline = time.monotonic() + wait_seconds
            while True:
                try:
                    if os.name == "nt":
                        import msvcrt
                        msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
                    else:
                        import fcntl
                        fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except OSError:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        _refuse("resource_busy", "The selected workflow is owned by an active operation.")
                    time.sleep(min(0.02, remaining))
                    if time.monotonic() >= deadline:
                        _refuse("resource_busy", "The selected workflow is owned by an active operation.")
            try:
                yield
            finally:
                stream.seek(0)
                if os.name == "nt":
                    import msvcrt
                    msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(stream.fileno(), fcntl.LOCK_UN)

    def _plan(self, plan_id: str) -> dict[str, Any]:
        record = _verify(self._read(self._path("plans", plan_id, "plan.json")), _PLAN_SCHEMA)
        if record.get("plan_id") != plan_id:
            _refuse("storage_invalid", "The retained plan identity is inconsistent.")
        checked = validate_workflow_plan(record["plan"], self.catalog)
        if checked.to_dict()["task_digest"] != content_digest({"task": record["task"]}):
            _refuse("storage_invalid", "The retained plan task is inconsistent.")
        if _json(record["requirements"]) != _json(checked.to_dict().get("requirements")):
            _refuse("storage_invalid", "The retained task contract is inconsistent.")
        return record

    @staticmethod
    def _plan_data(record: Mapping[str, Any]) -> dict[str, Any]:
        return {"status": "planned", "plan_id": record["plan_id"], "task": record["task"],
                "planner": record["planner"], "plan_digest": record["plan"]["plan_digest"],
                "plan": record["plan"], "source_dispatch": False,
                **({"interpretation": record["interpretation"]} if record["interpretation"] is not None else {})}

    def _run(self, run_id: str) -> dict[str, Any]:
        record = _verify(self._read(self._path("runs", run_id, "run.json")), _RUN_SCHEMA)
        if record.get("run_id") != run_id:
            _refuse("storage_invalid", "The retained run identity is inconsistent.")
        validate_workflow_plan(record["plan"], self.catalog)
        _identity(record["plan_id"])
        _integer(record["cancel_release_generation"], 0, 1_000_000)
        return record

    def _save_run(self, record: dict[str, Any]) -> None:
        self._write(self._path("runs", record["run_id"], "run.json"), _stamp(record))

    def _checkpoint(self, run_id: str, plan: dict[str, Any]) -> dict[str, Any] | None:
        path = self._path("runs", run_id, "checkpoint.json")
        if not path.exists():
            return None
        # A zero-dispatch executor invocation performs the authoritative checkpoint replay.
        # With no approval it retains a failure/in-flight boundary and never calls a source.
        checkpoint = load_workflow_checkpoint(path)
        execute_workflow_plan(plan, self.catalog, checkpoint=checkpoint, approved=False)
        return checkpoint

    def _cancel_generation(self, run_id: str) -> int:
        path = self._path("runs", run_id, "cancel.json")
        if not path.exists():
            return 0
        record = _fields(self._read(path, maximum=1_000), {"generation"})
        return _integer(record["generation"], 1, 1_000_000)

    def _inspect(self, run_id: str) -> dict[str, Any]:
        try:
            with self._lock(self._path("runs", run_id, "execute.lock")):
                # Hold the idle observation lock across both atomic record reads. Otherwise a
                # process finishing between those reads and a later probe can look interrupted.
                return self._inspect_state(run_id, active=False)
        except WorkflowAPIError as error:
            if error.code != "resource_busy":
                raise
            return self._inspect_state(run_id, active=True)

    def _inspect_state(self, run_id: str, *, active: bool) -> dict[str, Any]:
        record = self._run(run_id)
        checkpoint = self._checkpoint(run_id, record["plan"])
        status = checkpoint["status"] if checkpoint is not None else record["status"]
        if active:
            status = "running"
        elif record["status"] == "running":
            status = "interrupted"
        elif record["status"] == "error":
            status = "error"
        result = {"run_id": run_id, "plan_id": record["plan_id"],
                  "plan_digest": record["plan"]["plan_digest"], "status": status,
                  "active": active, "metadata_only": True,
                  "cancel_requested": self._cancel_generation(run_id) > record["cancel_release_generation"],
                  "output_review_statuses": {}}
        if checkpoint is not None:
            run = WorkflowRun(_json(checkpoint))
            from .workflow_cli import _output_review_statuses
            result.update(checkpoint=run.to_dict(), output_review_statuses=_output_review_statuses(run.outputs))
        if record["error_type"] is not None:
            result["error_type"] = record["error_type"]
        return result

    def handle(self, request: Mapping[str, Any]) -> dict[str, Any]:
        """Dispatch one bounded JSON action; no field can select an integration process or path."""
        if type(request) is not dict or len(_json(request).encode("utf-8")) > MAX_WORKFLOW_API_REQUEST_BYTES:
            _refuse("invalid_request", "The request is not a bounded JSON object.")
        action = request.get("action")
        if not isinstance(action, str):
            _refuse("invalid_request", "A supported action is required.")
        handler = getattr(self, "_action_" + action, None)
        if action not in {"catalogue", "interpret", "plan", "get_plan", "validate", "create_run", "execute", "inspect", "cancel", "export"} or handler is None:
            _refuse("invalid_request", "The requested action is not supported.")
        return handler(request)

    def _action_catalogue(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action"})
        return self.catalog.to_dict()

    def _action_interpret(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task"})
        return interpret_glioma_workflow_request(_task(request["task"])).to_dict()

    def _action_plan(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task", "planner"},
                {"requirements", "approve_provider_call", "max_steps", "planning_attempts"})
        task = _task(request["task"])
        approve = _boolean(request.get("approve_provider_call", False))
        max_steps = _integer(request.get("max_steps", 16), 1, 32)
        attempts = _integer(request.get("planning_attempts", 1), 1, 4)
        planner = request["planner"]
        if planner not in {"glioma-intent", "local-model"}:
            _refuse("invalid_request", "The selected planner is not supported.")
        contract = request.get("requirements")
        if contract is not None:
            contract = validate_workflow_requirements(contract, self.catalog,
                task_digest=content_digest({"task": task})).to_dict()
        budget_contract = compile_workflow_requirements(task, max_steps=max_steps)
        contract = (merge_workflow_requirements(task, budget_contract, contract, catalog=self.catalog).to_dict()
                    if contract is not None else budget_contract.to_dict())
        interpretation = None
        if planner == "glioma-intent":
            if attempts != 1:
                _refuse("invalid_request", "Typed intent planning has no model repair attempts.")
            interpretation = interpret_glioma_workflow_request(task).to_dict()
            if interpretation["status"] != "ready":
                return {"status": "needs_clarification", "interpretation": interpretation, "source_dispatch": False}
            plan = compile_glioma_workflow_request(task, self.catalog)
            data = plan.to_dict()
            if contract is not None:
                merged = merge_workflow_requirements(task, data["requirements"], contract, catalog=self.catalog)
                plan = compile_workflow_plan({key: data[key] for key in ("name", "steps", "outputs")},
                    self.catalog, task=task, requirements=merged)
            if len(plan.to_dict()["steps"]) > max_steps:
                _refuse("invalid_request", "The typed plan exceeds the requested step bound.")
        else:
            if not approve:
                _refuse("approval_required", "Local model planning requires explicit provider approval.")
            if self.model is None:
                _refuse("invalid_configuration", "No operator model is configured.")
            from .llm_runtime import LLMRuntime, ollama_provider
            runtime = LLMRuntime()
            runtime.register_provider(ollama_provider(base_url=self.base_url, timeout_seconds=self.timeout_seconds,
                max_attempts=1, structured_output_mode="json_schema"))
            plan = LocalWorkflowBuilder(self.catalog, runtime, model=self.model).build(task,
                approve_provider_call=True, max_steps=max_steps, requirements=contract, max_planning_attempts=attempts)
        plan_id = uuid4().hex
        record = {"schema": _PLAN_SCHEMA, "plan_id": plan_id, "task": task, "planner": planner,
                  "plan": plan.to_dict(), "requirements": plan.to_dict().get("requirements"),
                  "interpretation": interpretation}
        self._write(self._path("plans", plan_id, "plan.json", create=True), _stamp(record))
        return self._plan_data(record)

    def _action_get_plan(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "plan_id"})
        return self._plan_data(self._plan(_identity(request["plan_id"])))

    def _action_validate(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "plan_id", "expected_plan_digest", "proposal"})
        plan_id = _identity(request["plan_id"])
        expected = _identity(request["expected_plan_digest"], digest=True)
        _fields(request["proposal"], {"name", "steps", "outputs"})
        with self._lock(self._path("plans", plan_id, "plan.lock")):
            record = self._plan(plan_id)
            if record["plan"]["plan_digest"] != expected:
                _refuse("revision_conflict", "The retained plan changed; review its current revision.")
            try:
                plan = compile_workflow_plan(request["proposal"], self.catalog, task=record["task"],
                                             requirements=record["requirements"])
            except (ValueError, TypeError, KeyError, RecursionError, UnicodeError) as error:
                raise WorkflowAPIError("validation_failed", "The edited proposal failed the retained task contract or workflow checks.",
                    diagnostics=self._validation_diagnostics(request["proposal"], record)) from error
            record["plan"] = plan.to_dict()
            self._write(self._path("plans", plan_id, "plan.json"), _stamp(record))
        return self._plan_data(record) | {"validation": self._validation_diagnostics(request["proposal"], record)}

    def _validation_diagnostics(self, proposal: dict[str, Any], record: dict[str, Any]) -> dict[str, Any]:
        """Project SDK audits into bounded metadata; a failed audit remains unassessed."""
        candidate = proposal
        try:
            normalized = compile_workflow_plan(proposal, self.catalog, task=record["task"]).to_dict()
            candidate = {key: normalized[key] for key in ("name", "steps", "outputs")}
        except (ValueError, TypeError, KeyError, RecursionError, UnicodeError):
            # The reference audit can still explain a path excluded by a closed output schema.
            pass
        result: dict[str, Any] = {"schema": "aurora-workflow-api-validation/0.1", "source_dispatch": False,
                                  "provider_calls": 0, "scope": "proposal_structure_and_retained_contract_only;scientific_validity_unverified"}
        try:
            audit = audit_workflow_reference_paths(candidate, self.catalog)
            result["reference_paths"] = {
                "status": "assessed", "audit_digest": audit["audit_digest"],
                "blocking_issue_count": audit["blocking_issue_count"],
                "runtime_resolution_count": audit["runtime_resolution_count"],
                "references": _diagnostic_rows(audit["references"],
                    {"step_id", "source_step_id", "source_operation", "source_pointer", "status", "reason"}),
                "omitted_reference_count": max(0, len(audit["references"]) - MAX_WORKFLOW_API_DIAGNOSTIC_ROWS),
            }
        except Exception as error:
            result["reference_paths"] = {"status": "not_assessed", "reason": "audit_did_not_complete", "error_type": _safe_error_type(error)}
        if record["requirements"] is None:
            result["requirements"] = {"status": "not_applicable", "reason": "no_retained_additional_contract"}
        else:
            try:
                audit = audit_workflow_requirements(candidate, record["requirements"], self.catalog)
                issues, deferred = audit["blocking_issues"], audit["deferred_arguments"]
                result["requirements"] = {
                    "status": "assessed", "audit_digest": audit["audit_digest"],
                    "contract_satisfied": audit["contract_satisfied"], "blocking_issue_count": len(issues),
                    "deferred_argument_count": len(deferred),
                    "blocking_issues": _diagnostic_rows(issues, {"code", "operation", "rule_index", "link_index", "step_id", "path"}),
                    "deferred_arguments": _diagnostic_rows(deferred, {"operation", "rule_index", "step_id", "path"}),
                    "omitted_blocking_issue_count": max(0, len(issues) - MAX_WORKFLOW_API_DIAGNOSTIC_ROWS),
                    "omitted_deferred_argument_count": max(0, len(deferred) - MAX_WORKFLOW_API_DIAGNOSTIC_ROWS),
                }
            except Exception as error:
                result["requirements"] = {"status": "not_assessed", "reason": "audit_did_not_complete", "error_type": _safe_error_type(error)}
        return result

    def _action_create_run(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "plan_id", "expected_plan_digest"})
        plan_id = _identity(request["plan_id"])
        expected = _identity(request["expected_plan_digest"], digest=True)
        with self._lock(self._path("plans", plan_id, "plan.lock")):
            plan = self._plan(plan_id)
            if plan["plan"]["plan_digest"] != expected:
                _refuse("revision_conflict", "The retained plan changed; review its current revision.")
            run_id = uuid4().hex
            record = {"schema": _RUN_SCHEMA, "run_id": run_id, "plan_id": plan_id,
                      "plan": plan["plan"], "status": "queued", "cancel_release_generation": 0,
                      "execution_count": 0, "error_type": None}
            self._path("runs", run_id, "run.json", create=True)
            self._save_run(record)
        return {"status": "queued", "run_id": run_id, "plan_id": plan_id,
                "plan_digest": expected, "metadata_only": True}

    def _action_inspect(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "run_id"})
        return self._inspect(_identity(request["run_id"]))

    def _action_cancel(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "run_id"})
        run_id = _identity(request["run_id"])
        self._run(run_id)
        with self._lock(self._path("runs", run_id, "cancel.lock")):
            generation = self._cancel_generation(run_id)
            _integer(generation + 1, 1, 1_000_000)
            self._write(self._path("runs", run_id, "cancel.json"), {"generation": generation + 1})
        return {"run_id": run_id, "status": "cancellation_requested", "cancel_requested": True,
                "metadata_only": True}

    def _action_execute(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "run_id", "expected_plan_digest", "approve_source_dispatch"},
                {"resume", "retry_failed", "resume_cancelled", "max_step_calls"})
        run_id = _identity(request["run_id"])
        expected = _identity(request["expected_plan_digest"], digest=True)
        approved = _boolean(request["approve_source_dispatch"])
        resume = _boolean(request.get("resume", False))
        retry = _boolean(request.get("retry_failed", False))
        release = _boolean(request.get("resume_cancelled", False))
        budget = _integer(request.get("max_step_calls", 32), 1, 32)
        if (retry or release) and not resume:
            _refuse("invalid_request", "Retry and cancellation release require explicit resume.")
        record = self._run(run_id)
        with self._lock(self._path("plans", record["plan_id"], "plan.lock")):
            with self._lock(self._path("runs", run_id, "execute.lock"), wait_seconds=1.0):
                record = self._run(run_id)
                current = self._plan(record["plan_id"])
                if expected != record["plan"]["plan_digest"] or expected != current["plan"]["plan_digest"]:
                    _refuse("revision_conflict", "Execution approval does not match the current retained plan.")
                checkpoint = self._checkpoint(run_id, record["plan"])
                if record["execution_count"] and not resume:
                    _refuse("resume_required", "The run was already admitted; explicitly review and resume it.")
                if resume and not record["execution_count"]:
                    _refuse("invalid_request", "A new run cannot be resumed before its first admission.")
                if resume and checkpoint is None:
                    _refuse("recovery_required", "The interrupted run has no durable checkpoint; automatic replay is refused.")
                if release:
                    with self._lock(self._path("runs", run_id, "cancel.lock")):
                        record["cancel_release_generation"] = self._cancel_generation(run_id)
                cancellation = WorkflowCancellationToken()

                def observe() -> None:
                    if self._cancel_generation(run_id) > record["cancel_release_generation"]:
                        cancellation.cancel()

                def persist(value: dict[str, Any]) -> None:
                    observe()
                    self._write(self._path("runs", run_id, "checkpoint.json"), value)

                observe()
                record.update(status="running", error_type=None, execution_count=record["execution_count"] + 1)
                self._save_run(record)
                try:
                    run = execute_workflow_plan(record["plan"], self.catalog, approved=approved,
                        checkpoint=checkpoint, max_step_calls=budget, retry_failed=retry,
                        retrieved_at=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                        on_checkpoint=persist, cancellation=cancellation, resume_cancelled=release)
                    record["status"] = run.status
                    self._save_run(record)
                except Exception as error:
                    record.update(status="error", error_type=_safe_error_type(error))
                    try:
                        self._save_run(record)
                    except Exception:
                        # The running record and durable checkpoint remain an interrupted boundary.
                        pass
                    raise
        return self._inspect(run_id)

    def _action_export(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "run_id", "kind"}, {"format"})
        run_id = _identity(request["run_id"])
        kind = request["kind"]
        if not isinstance(kind, str) or kind not in {"outputs", "checkpoint"}:
            _refuse("invalid_request", "The requested explicit export is not supported.")
        export_format = request.get("format", "native")
        if not isinstance(export_format, str) or export_format not in {"native", "json_text"}:
            _refuse("invalid_request", "The requested explicit export format is not supported.")
        record = self._run(run_id)
        checkpoint = self._checkpoint(run_id, record["plan"])
        if checkpoint is None:
            _refuse("not_found", "The run has no retained checkpoint to export.")
        result = {"run_id": run_id, "plan_digest": record["plan"]["plan_digest"],
                  "status": checkpoint["status"]}
        artifact = checkpoint if kind == "checkpoint" else WorkflowRun(_json(checkpoint)).outputs
        if export_format == "json_text":
            return result | {"kind": kind, "json_text": _json(artifact)}
        result[kind] = artifact
        return result


def _safe_error_type(error: BaseException) -> str:
    name = type(error).__name__
    return name if re.fullmatch(r"[A-Za-z][A-Za-z0-9_]{0,79}", name) else "ExecutionError"


def _diagnostic_rows(rows: list[dict[str, Any]], fields: set[str]) -> list[dict[str, Any]]:
    projected = []
    for row in rows[:MAX_WORKFLOW_API_DIAGNOSTIC_ROWS]:
        clean = {}
        for key in fields:
            value = row.get(key)
            if type(value) is int and 0 <= value <= 1_000_000:
                clean[key] = value
            elif isinstance(value, str):
                clean[key] = (value if len(value.encode("utf-8")) <= 256
                    and not any(ord(char) < 32 or ord(char) == 127 for char in value)
                    else "[metadata_field_omitted]")
        projected.append(clean)
    return projected


def _error(error: BaseException) -> dict[str, Any]:
    if isinstance(error, WorkflowAPIError):
        code, message = error.code, str(error)
    elif isinstance(error, (ValueError, TypeError, KeyError, UnicodeError, RecursionError)):
        code, message = "validation_failed", "The request or retained workflow failed validation."
    elif isinstance(error, OSError):
        code, message = "storage_error", "The application could not access its owned workflow storage."
    else:
        code, message = "execution_error", "The requested operation failed; inspect retained run metadata before retrying."
    detail = {"code": code, "message": message}
    if isinstance(error, WorkflowAPIError) and error.diagnostics is not None:
        detail["diagnostics"] = error.diagnostics
    return {"schema": WORKFLOW_API_RESPONSE_SCHEMA, "ok": False, "error": detail}


def main(argv: list[str] | None = None) -> int:
    """Read exactly one JSON value to EOF, then emit exactly one bounded JSON envelope."""
    parser = argparse.ArgumentParser(description="Fixed public-workflow stdio worker; no HTTP listener")
    parser.add_argument("--root", required=True)
    parser.add_argument("--base-url", default="http://127.0.0.1:11434/v1")
    parser.add_argument("--model")
    parser.add_argument("--research-sources", action="store_true")
    parser.add_argument("--molecular-glioma", action="store_true")
    parser.add_argument("--timeout-seconds", type=int, default=120)
    args = parser.parse_args(argv)
    try:
        raw = sys.stdin.buffer.read(MAX_WORKFLOW_API_REQUEST_BYTES + 1)
        if len(raw) > MAX_WORKFLOW_API_REQUEST_BYTES:
            _refuse("invalid_request", "The request exceeds its UTF-8 byte bound.")
        request = json.loads(raw.decode("utf-8"), object_pairs_hook=_pairs,
                             parse_constant=lambda _: _refuse("invalid_request", "Non-finite JSON is refused."))
        worker = WorkflowAPIWorker(args.root, base_url=args.base_url, model=args.model,
            research_sources=args.research_sources, molecular_glioma=args.molecular_glioma,
            timeout_seconds=args.timeout_seconds)
        response = {"schema": WORKFLOW_API_RESPONSE_SCHEMA, "ok": True, "data": worker.handle(request)}
        encoded = _json(response)
        if len(encoded.encode("utf-8")) + 1 > MAX_WORKFLOW_API_RESPONSE_BYTES:
            _refuse("response_too_large", "The response exceeds its explicit export byte bound.")
        exit_code = 0
    except Exception as error:
        encoded, exit_code = _json(_error(error)), 2
    sys.stdout.buffer.write(encoded.encode("utf-8") + b"\n")
    sys.stdout.buffer.flush()
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
