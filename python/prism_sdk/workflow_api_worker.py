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
from .workflow_intent_obligations import (
    admit_workflow_intent_obligations, compile_workflow_intent_obligations,
    validate_workflow_intent_obligations,
)

WORKFLOW_API_RESPONSE_SCHEMA = "aurora-workflow-api-response/0.1"
MAX_WORKFLOW_API_REQUEST_BYTES = 1_000_000
MAX_WORKFLOW_API_RESPONSE_BYTES = 16_000_000
MAX_WORKFLOW_API_DIAGNOSTIC_ROWS = 64
_PLAN_SCHEMA = "aurora-workflow-api-plan/0.1"
_INTENT_PLAN_SCHEMA = "aurora-workflow-api-plan/0.2"
_REVIEW_SCHEMA = "aurora-workflow-api-intent-review/0.1"
_ACCEPTANCE_SCHEMA = "aurora-workflow-api-intent-acceptance/0.1"
_EVIDENCE_SCHEMA = "aurora-workflow-intent-evidence/0.1"
_ACCEPTANCE_SCOPE = "explicit_api_caller_declaration;human_presence_authorship_and_extraction_completeness_unassessed"
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
                 molecular_glioma: bool = False, glioma_mutations: bool = False,
                 gene_references: bool = False, protein_pathways: bool = False,
                 denominator_coverage: bool = False,
                 capabilities: tuple[str, ...] = (),
                 timeout_seconds: int = 120) -> None:
        from .workflow_capabilities import validate_workflow_capabilities, with_workflow_capabilities
        self.capabilities = validate_workflow_capabilities(capabilities)
        if any(type(value) is not bool for value in (research_sources, molecular_glioma, glioma_mutations, gene_references, protein_pathways, denominator_coverage)):
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
        for name in ("plans", "runs", "reviews"):
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
        if glioma_mutations or denominator_coverage:
            from .reviewed_gdc_gene_mutation_retrieval import register_gdc_gene_mutation_operations
            from .glioma_mutation_workflow import register_glioma_mutation_workflow_operations
            register_gdc_gene_mutation_operations(self.catalog)
            register_glioma_mutation_workflow_operations(self.catalog)
        if gene_references:
            from .reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations
            from .glioma_gene_reference_workflow import register_glioma_gene_reference_workflow_operations
            register_ensembl_gene_operations(self.catalog)
            register_glioma_gene_reference_workflow_operations(self.catalog)
        if protein_pathways:
            from .reviewed_uniprot_retrieval import register_uniprot_operations
            from .reviewed_reactome_retrieval import register_reactome_operations
            from .glioma_protein_pathway_workflow import register_glioma_protein_pathway_workflow_operations
            register_uniprot_operations(self.catalog)
            register_reactome_operations(self.catalog)
            register_glioma_protein_pathway_workflow_operations(self.catalog)
        if denominator_coverage:
            from .reviewed_gdc_ssm_coverage_retrieval import register_gdc_ssm_coverage_operations
            from .glioma_denominator_workflow import register_glioma_denominator_workflow_operations
            register_gdc_ssm_coverage_operations(self.catalog)
            register_glioma_denominator_workflow_operations(self.catalog)
        self.catalog = with_workflow_capabilities(self.catalog, self.capabilities)

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
        raw = self._read(self._path("plans", plan_id, "plan.json"))
        schema = _INTENT_PLAN_SCHEMA if type(raw) is dict and raw.get("schema") == _INTENT_PLAN_SCHEMA else _PLAN_SCHEMA
        record = _verify(raw, schema)
        if record.get("plan_id") != plan_id:
            _refuse("storage_invalid", "The retained plan identity is inconsistent.")
        checked = validate_workflow_plan(record["plan"], self.catalog)
        if checked.to_dict()["task_digest"] != content_digest({"task": record["task"]}):
            _refuse("storage_invalid", "The retained plan task is inconsistent.")
        if _json(record["requirements"]) != _json(checked.to_dict().get("requirements")):
            _refuse("storage_invalid", "The retained task contract is inconsistent.")
        if schema == _PLAN_SCHEMA and "intent_review" in record:
            _refuse("storage_invalid", "A legacy application record cannot carry unvalidated intent review metadata.")
        if schema == _INTENT_PLAN_SCHEMA:
            _fields(record, {"schema", "plan_id", "task", "planner", "plan", "requirements", "interpretation", "intent_review"})
            snapshot = self._validate_review_snapshot(record["intent_review"])
            context = snapshot["review"]["context"]
            acceptance = snapshot["acceptance"]
            if (acceptance is None or context["task"] != record["task"] or context["planner"] != record["planner"]
                    or acceptance["admission"]["status"] != "ready_for_constrained_authoring"
                    or _json(acceptance["admission"]["requirements"]) != _json(record["requirements"])):
                _refuse("storage_invalid", "The retained intent review does not bind the saved task contract.")
        return record

    @staticmethod
    def _plan_data(record: Mapping[str, Any]) -> dict[str, Any]:
        return {"status": "planned", "plan_id": record["plan_id"], "task": record["task"],
                "planner": record["planner"], "plan_digest": record["plan"]["plan_digest"],
                "plan": record["plan"], "source_dispatch": False,
                **({"intent_review": WorkflowAPIWorker._review_data(record["intent_review"])} if "intent_review" in record else {}),
                **({"interpretation": record["interpretation"]} if record["interpretation"] is not None else {})}

    def _review_context(self, request: Mapping[str, Any]) -> dict[str, Any]:
        """Bind declaration review to caller controls, without spending model/source authority."""
        task = _task(request["task"])
        planner = request["planner"]
        if type(planner) is not str or planner not in {"local-model", "local-incremental"}:
            _refuse("invalid_request", "Intent review currently supports the two local model planners.")
        max_steps = _integer(request.get("max_steps", 16), 1, 32)
        attempts = _integer(request.get("planning_attempts", 1), 1, 4)
        operation_ids = None
        model_calls = decision_attempts = None
        if planner == "local-incremental":
            if attempts != 1:
                _refuse("invalid_request", "Incremental planning requires planning_attempts to remain one.")
            model_calls = _integer(request.get("max_model_calls", 64), 1, 128)
            decision_attempts = _integer(request.get("decision_attempts", 2), 1, 4)
            operation_ids = request.get("operation_ids")
            if "operation_ids" in request:
                if (type(operation_ids) is not list or not 1 <= len(operation_ids) <= 128
                        or any(type(item) is not str or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}", item) for item in operation_ids)
                        or len(set(operation_ids)) != len(operation_ids)):
                    _refuse("invalid_request", "Intent review operations require a nonempty exact selection.")
                for operation in operation_ids:
                    self.catalog.resolve(operation)
        elif {"max_model_calls", "decision_attempts", "operation_ids"} & set(request):
            _refuse("invalid_request", "Incremental controls require the local-incremental planner.")
        caller = request.get("requirements")
        budget = compile_workflow_requirements(task, max_steps=max_steps)
        contract = (merge_workflow_requirements(task, budget, caller, catalog=self.catalog)
                    if caller is not None else budget)
        return {"task": task, "planner": planner, "catalog_digest": self.catalog.catalog_digest,
                "max_steps": max_steps, "planning_attempts": attempts, "max_model_calls": model_calls,
                "decision_attempts": decision_attempts, "operation_ids": operation_ids,
                "pre_obligation_requirements": contract.to_dict()}

    @staticmethod
    def _context_request(context: dict[str, Any]) -> dict[str, Any]:
        _fields(context, {"task", "planner", "catalog_digest", "max_steps", "planning_attempts",
                          "max_model_calls", "decision_attempts", "operation_ids", "pre_obligation_requirements"})
        result = {key: context[key] for key in ("task", "planner", "max_steps", "planning_attempts")}
        result["requirements"] = context["pre_obligation_requirements"]
        result.update({key: context[key] for key in ("max_model_calls", "decision_attempts", "operation_ids") if context[key] is not None})
        return result

    def _admit_review(self, context: dict[str, Any], declarations: Mapping[str, Any]) -> dict[str, Any]:
        return admit_workflow_intent_obligations(context["task"], declarations, self.catalog,
            operation_ids=context["operation_ids"], requirements=context["pre_obligation_requirements"],
            max_steps=context["max_steps"]).to_dict()

    def _validate_review_snapshot(self, snapshot: Any) -> dict[str, Any]:
        """Recompute lexical/contract bindings; storage digests do not attest a human."""
        _fields(snapshot, {"review", "acceptance"})
        draft = _verify(snapshot["review"], _REVIEW_SCHEMA)
        _fields(draft, {"schema", "review_id", "context", "candidate", "candidate_admission", "origin"})
        _identity(draft["review_id"])
        context = draft["context"]
        expected_context = self._review_context(self._context_request(context))
        if _json(context) != _json(expected_context):
            _refuse("storage_invalid", "The intent review planning context has changed.")
        candidate = validate_workflow_intent_obligations(draft["candidate"], task=context["task"]).to_dict()
        if candidate["interpretation_basis"] != "model_candidate":
            _refuse("storage_invalid", "Unreviewed input cannot classify itself as a caller declaration.")
        if _json(draft["candidate_admission"]) != _json(self._admit_review(context, candidate)):
            _refuse("storage_invalid", "The unreviewed intent admission is inconsistent.")
        origin = draft["origin"]
        if (type(origin) is not dict or type(origin.get("kind")) is not str
                or origin["kind"] not in {"client_submitted_rows", "local_model_candidate"}
                or len(_json(origin).encode("utf-8")) > 1_000_000):
            _refuse("storage_invalid", "The unreviewed input origin is invalid.")
        if origin["kind"] == "client_submitted_rows":
            _fields(origin, {"kind", "authorship"})
            if origin["authorship"] != "unverified":
                _refuse("storage_invalid", "Client declaration authorship is unverified.")
        else:
            _fields(origin, {"kind", "extraction"})
            from .workflow_intent_extractor import validate_workflow_intent_extraction
            extraction = validate_workflow_intent_extraction(origin["extraction"], task=context["task"],
                catalog=self.catalog, operation_ids=context["operation_ids"])
            if (extraction.status != "candidate" or extraction.obligations is None
                    or _json(extraction.obligations.to_dict()) != _json(candidate)):
                _refuse("storage_invalid", "The original extraction does not bind the retained unreviewed candidate.")
        if snapshot["acceptance"] is not None:
            accepted = _verify(snapshot["acceptance"], _ACCEPTANCE_SCHEMA)
            _fields(accepted, {"schema", "review_id", "review_digest", "accepted_at", "scope", "accepted_declarations", "admission"})
            if (accepted["review_id"] != draft["review_id"] or accepted["review_digest"] != snapshot["review"]["record_digest"]
                    or accepted["scope"] != _ACCEPTANCE_SCOPE or not isinstance(accepted["accepted_at"], str)
                    or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", accepted["accepted_at"])):
                _refuse("storage_invalid", "The caller acceptance does not bind the exact retained review.")
            declarations = compile_workflow_intent_obligations(context["task"], candidate["obligations"]).to_dict()
            if (_json(accepted["accepted_declarations"]) != _json(declarations)
                    or _json(accepted["admission"]) != _json(self._admit_review(context, declarations))):
                _refuse("storage_invalid", "Accepted declarations or their admission have changed.")
        return snapshot

    def _review(self, review_id: str) -> dict[str, Any]:
        raw = self._read(self._path("reviews", review_id, "review.json"))
        acceptance_path = self._path("reviews", review_id, "acceptance.json")
        accepted = self._read(acceptance_path) if acceptance_path.exists() else None
        snapshot = self._validate_review_snapshot({"review": raw, "acceptance": accepted})
        if raw["review_id"] != review_id:
            _refuse("storage_invalid", "The intent review identity is inconsistent.")
        return snapshot

    @staticmethod
    def _review_data(snapshot: Mapping[str, Any]) -> dict[str, Any]:
        draft, acceptance = snapshot["review"], snapshot["acceptance"]
        return {"status": "accepted" if acceptance is not None else "review_required", "review_id": draft["review_id"],
                "review_digest": draft["record_digest"], "task": draft["context"]["task"], "planner": draft["context"]["planner"],
                "context": draft["context"], "candidate": draft["candidate"], "candidate_admission": draft["candidate_admission"],
                "origin": draft["origin"], "acceptance": ({**acceptance, "acceptance_digest": acceptance["record_digest"]}
                                                          if acceptance is not None else None), "source_dispatch": False}

    def _save_review(self, context: dict[str, Any], candidate: Mapping[str, Any], origin: dict[str, Any]) -> dict[str, Any]:
        candidate = validate_workflow_intent_obligations(candidate, task=context["task"]).to_dict()
        if candidate["interpretation_basis"] != "model_candidate":
            _refuse("invalid_request", "Suggested input must remain an unreviewed model candidate.")
        review_id = uuid4().hex
        draft = _stamp({"schema": _REVIEW_SCHEMA, "review_id": review_id, "context": context,
                        "candidate": candidate, "candidate_admission": self._admit_review(context, candidate), "origin": origin})
        snapshot = self._validate_review_snapshot({"review": draft, "acceptance": None})
        self._write(self._path("reviews", review_id, "review.json", create=True), draft)
        return self._review_data(snapshot)

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
        if action not in {"catalogue", "providers", "interpret", "prepare_review", "suggest_review", "accept_review", "get_review",
                          "plan", "get_plan", "validate", "create_run", "execute", "inspect", "cancel", "export", "export_evidence"} or handler is None:
            _refuse("invalid_request", "The requested action is not supported.")
        return handler(request)

    def _action_catalogue(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action"})
        from .workflow_capabilities import workflow_capability_inventory
        return {**self.catalog.to_dict(), "capabilities": workflow_capability_inventory(self.catalog, self.capabilities)}

    def _action_providers(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action"})
        from .workflow_provider_registry import reviewed_workflow_provider_registry
        return reviewed_workflow_provider_registry(self.catalog).to_dict()

    def _action_interpret(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task"})
        return interpret_glioma_workflow_request(_task(request["task"])).to_dict()

    def _action_prepare_review(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task", "planner", "obligations"},
                {"requirements", "max_steps", "planning_attempts", "max_model_calls", "decision_attempts", "operation_ids"})
        context = self._review_context(request)
        candidate = compile_workflow_intent_obligations(context["task"], request["obligations"],
            interpretation_basis="model_candidate").to_dict()
        return self._save_review(context, candidate, {"kind": "client_submitted_rows", "authorship": "unverified"})

    def _action_suggest_review(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task", "planner", "approve_provider_call"},
                {"requirements", "max_steps", "planning_attempts", "max_model_calls", "decision_attempts", "operation_ids"})
        context = self._review_context(request)
        if not _boolean(request["approve_provider_call"]):
            _refuse("approval_required", "Suggesting declarations requires a separate local model approval.")
        if self.model is None:
            _refuse("invalid_configuration", "No operator model is configured.")
        from .llm_runtime import LLMRuntime, ollama_provider
        from .workflow_intent_extractor import LocalWorkflowIntentExtractor
        runtime = LLMRuntime()
        runtime.register_provider(ollama_provider(base_url=self.base_url, timeout_seconds=self.timeout_seconds,
            max_attempts=1, structured_output_mode="json_schema"))
        extraction = LocalWorkflowIntentExtractor(self.catalog, runtime, model=self.model).extract(context["task"],
            approve_provider_call=True, operation_ids=context["operation_ids"])
        data = extraction.to_dict()
        if extraction.status != "candidate":
            if extraction.obligations is not None:
                _refuse("validation_failed", "A noncandidate extraction cannot supply accepted declarations.")
            return {"status": extraction.status, "extraction": data, "source_dispatch": False}
        if extraction.obligations is None:
            _refuse("validation_failed", "The candidate extraction lacks its unreviewed declaration document.")
        return self._save_review(context, extraction.obligations.to_dict(),
            {"kind": "local_model_candidate", "extraction": data})

    def _action_get_review(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "review_id"})
        return self._review_data(self._review(_identity(request["review_id"])))

    def _action_accept_review(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "review_id", "expected_review_digest", "accept_caller_declarations"})
        review_id = _identity(request["review_id"])
        expected = _identity(request["expected_review_digest"], digest=True)
        if not _boolean(request["accept_caller_declarations"]):
            _refuse("approval_required", "Accept the exact declarations explicitly; acceptance proves no human provenance.")
        with self._lock(self._path("reviews", review_id, "accept.lock")):
            snapshot = self._review(review_id)
            draft = snapshot["review"]
            if draft["record_digest"] != expected:
                _refuse("revision_conflict", "The retained declaration review changed; inspect its exact current digest.")
            # A repeated exact acceptance returns the same durable event. No implicit source
            # or model approval follows, and unresolved declarations remain blocking.
            if snapshot["acceptance"] is None:
                context = draft["context"]
                declarations = compile_workflow_intent_obligations(context["task"], draft["candidate"]["obligations"]).to_dict()
                accepted = _stamp({"schema": _ACCEPTANCE_SCHEMA, "review_id": review_id, "review_digest": expected,
                    "accepted_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "scope": _ACCEPTANCE_SCOPE,
                    "accepted_declarations": declarations, "admission": self._admit_review(context, declarations)})
                self._write(self._path("reviews", review_id, "acceptance.json"), accepted)
                snapshot["acceptance"] = accepted
            return self._review_data(snapshot)

    def _action_plan(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action", "task", "planner"},
                {"requirements", "approve_provider_call", "max_steps", "planning_attempts",
                 "max_model_calls", "decision_attempts", "operation_ids", "intent_review_id", "expected_acceptance_digest"})
        task = _task(request["task"])
        approve = _boolean(request.get("approve_provider_call", False))
        max_steps = _integer(request.get("max_steps", 16), 1, 32)
        attempts = _integer(request.get("planning_attempts", 1), 1, 4)
        planner = request["planner"]
        if (type(planner) is not str or planner not in {"glioma-intent", "local-model", "local-incremental", "glioma-mutations", "gene-references", "protein-pathways", "denominator-coverage"}
                | {"capability:" + name for name in self.capabilities}):
            _refuse("invalid_request", "The selected planner is not supported.")
        incremental_fields = {"max_model_calls", "decision_attempts", "operation_ids"}
        if planner != "local-incremental" and incremental_fields & set(request):
            _refuse("invalid_request", "Incremental controls require the local-incremental planner.")
        if planner == "local-incremental":
            if attempts != 1:
                _refuse("invalid_request", "Incremental planning requires planning_attempts to remain one.")
            model_calls = _integer(request.get("max_model_calls", 64), 1, 128)
            decision_attempts = _integer(request.get("decision_attempts", 2), 1, 4)
            operation_ids = request.get("operation_ids")
            if "operation_ids" in request:
                if (type(operation_ids) is not list or not 1 <= len(operation_ids) <= 128
                        or any(type(item) is not str or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,255}", item) for item in operation_ids)
                        or len(set(operation_ids)) != len(operation_ids)):
                    _refuse("invalid_request", "Incremental operations require a nonempty exact allowlist.")
                for operation_id in operation_ids:
                    self.catalog.resolve(operation_id)
        contract = request.get("requirements")
        if contract is not None:
            contract = validate_workflow_requirements(contract, self.catalog,
                task_digest=content_digest({"task": task})).to_dict()
        budget_contract = compile_workflow_requirements(task, max_steps=max_steps)
        contract = (merge_workflow_requirements(task, budget_contract, contract, catalog=self.catalog).to_dict()
                    if contract is not None else budget_contract.to_dict())
        intent_snapshot = None
        review_fields = {"intent_review_id", "expected_acceptance_digest"} & set(request)
        if review_fields:
            if review_fields != {"intent_review_id", "expected_acceptance_digest"}:
                _refuse("invalid_request", "An intent review reference requires its exact acceptance digest.")
            context = self._review_context(request)
            intent_snapshot = self._review(_identity(request["intent_review_id"]))
            accepted = intent_snapshot["acceptance"]
            if accepted is None:
                _refuse("review_required", "The retained candidate has not been accepted as caller declarations.")
            if accepted["record_digest"] != _identity(request["expected_acceptance_digest"], digest=True):
                _refuse("revision_conflict", "The supplied acceptance digest differs from the retained event.")
            if _json(context) != _json(intent_snapshot["review"]["context"]):
                _refuse("revision_conflict", "Task or planning controls changed; prepare and accept a new declaration review.")
            admission = accepted["admission"]
            if admission["status"] != "ready_for_constrained_authoring":
                return {"status": admission["status"], "intent_review": self._review_data(intent_snapshot),
                        "interpretation": admission, "source_dispatch": False}
            # Preserve the historical event; current constraints gate only new authoring.
            from .workflow_literal_policy import assess_workflow_literal_policy
            current = assess_workflow_literal_policy(task, accepted["accepted_declarations"], self.catalog)
            if current.has_definite_conflicts:
                return {"status": "current_policy_conflict", "intent_review": self._review_data(intent_snapshot),
                        "interpretation": current.to_dict(), "source_dispatch": False}
            contract = admission["requirements"]
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
        elif planner.startswith("capability:"):
            if attempts != 1:
                _refuse("invalid_request", "Fixed source templates have no model repair attempts.")
            from .workflow_capabilities import compile_workflow_capability
            profile_id = planner.removeprefix("capability:")
            plan = compile_workflow_capability(task, self.catalog, profile_id,
                enabled_profiles=self.capabilities, requirements=contract)
            interpretation = {"schema": "aurora-workflow-capability-selection/0.1", "profile_id": profile_id,
                "provider_calls": 0, "question_used_as_query": False, "scope": "operator_enabled_fixed_public_sources",
                "scientific_validity": "unassessed"}
        elif planner in {"glioma-mutations", "gene-references", "protein-pathways", "denominator-coverage"}:
            if attempts != 1:
                _refuse("invalid_request", "Fixed source templates have no model repair attempts.")
            if planner == "glioma-mutations":
                from .glioma_mutation_workflow import compile_glioma_mutation_workflow
                plan = compile_glioma_mutation_workflow(task, self.catalog, requirements=contract)
            elif planner == "gene-references":
                from .glioma_gene_reference_workflow import compile_glioma_gene_reference_workflow
                plan = compile_glioma_gene_reference_workflow(task, self.catalog, requirements=contract)
            elif planner == "protein-pathways":
                from .glioma_protein_pathway_workflow import compile_glioma_protein_pathway_workflow
                plan = compile_glioma_protein_pathway_workflow(task, self.catalog, requirements=contract)
            else:
                from .glioma_denominator_workflow import compile_glioma_denominator_workflow
                plan = compile_glioma_denominator_workflow(task, self.catalog, requirements=contract)
            if len(plan.to_dict()["steps"]) > max_steps:
                _refuse("invalid_request", "The fixed source template exceeds the requested step bound.")
            interpretation = {"schema": "aurora-fixed-research-template-selection/0.1", "template": planner,
                "provider_calls": 0, "question_used_as_query": False, "fixed_gene_symbol": "TP53",
                "scope": "operator_enabled_fixed_public_sources", "scientific_validity": "unassessed"}
        else:
            if not approve:
                _refuse("approval_required", "Local model planning requires explicit provider approval.")
            if self.model is None:
                _refuse("invalid_configuration", "No operator model is configured.")
            from .llm_runtime import LLMRuntime, ollama_provider
            runtime = LLMRuntime()
            runtime.register_provider(ollama_provider(base_url=self.base_url, timeout_seconds=self.timeout_seconds,
                max_attempts=1, structured_output_mode="json_schema"))
            if planner == "local-incremental":
                from .workflow_incremental_planner import IncrementalLocalWorkflowPlanner
                result = IncrementalLocalWorkflowPlanner(self.catalog, runtime, model=self.model).build(task,
                    approve_provider_call=True, max_steps=max_steps, requirements=contract,
                    max_model_calls=model_calls, max_decision_attempts=decision_attempts,
                    operation_ids=operation_ids)
                interpretation = result.to_dict()
                if result.status != "planned":
                    if "plan" in interpretation or result.plan is not None:
                        _refuse("validation_failed", "A nonplanned incremental result cannot contain an executable plan.")
                    return {"status": result.status, "planner": planner, "interpretation": interpretation,
                            "source_dispatch": False}
                if result.plan is None:
                    _refuse("validation_failed", "A planned incremental result lacks its compiler-accepted plan.")
                plan = validate_workflow_plan(result.plan, self.catalog)
            else:
                plan = LocalWorkflowBuilder(self.catalog, runtime, model=self.model).build(task,
                    approve_provider_call=True, max_steps=max_steps, requirements=contract, max_planning_attempts=attempts)
        plan_id = uuid4().hex
        record = {"schema": _INTENT_PLAN_SCHEMA if intent_snapshot is not None else _PLAN_SCHEMA, "plan_id": plan_id, "task": task, "planner": planner,
                  "plan": plan.to_dict(), "requirements": plan.to_dict().get("requirements"),
                  "interpretation": interpretation, **({"intent_review": intent_snapshot} if intent_snapshot is not None else {})}
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
        requested_release_generation = self._cancel_generation(run_id) if release else None
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
                if requested_release_generation is not None:
                    with self._lock(self._path("runs", run_id, "cancel.lock")):
                        # The grant covers the cancellation observed before admission waiting.
                        # A later cancellation cannot be consumed by this earlier release.
                        record["cancel_release_generation"] = max(record["cancel_release_generation"],
                                                                    requested_release_generation)
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

    def _action_export_evidence(self, request: dict[str, Any]) -> dict[str, Any]:
        _fields(request, {"action"}, {"plan_id", "run_id", "format"})
        if len({"plan_id", "run_id"} & set(request)) != 1:
            _refuse("invalid_request", "Select exactly one plan or run for explicit intent evidence export.")
        export_format = request.get("format", "native")
        if type(export_format) is not str or export_format not in {"native", "json_text"}:
            _refuse("invalid_request", "The requested explicit evidence format is not supported.")
        run_id = None
        if "run_id" in request:
            run_id = _identity(request["run_id"])
            run = self._run(run_id)
            plan_id, snapshot = run["plan_id"], run["plan"]
            record = self._plan(plan_id)
            if _json(snapshot.get("requirements")) != _json(record["requirements"]):
                _refuse("storage_invalid", "Historical run and immutable intent contracts differ.")
        else:
            plan_id = _identity(request["plan_id"])
            record = self._plan(plan_id)
            snapshot = record["plan"]
        if "intent_review" not in record:
            _refuse("not_found", "This legacy plan has no retained intent review evidence.")
        # Edits preserve intent metadata for a plan ID. A run export selects its original
        # native plan snapshot even after a later valid proposal edit changes the current one.
        if snapshot["task_digest"] != content_digest({"task": record["task"]}):
            _refuse("storage_invalid", "Historical plan and declaration task bindings differ.")
        bundle = {"schema": _EVIDENCE_SCHEMA, "plan_id": plan_id, "run_id": run_id,
                  "task": record["task"], "planner": record["planner"], "plan_digest": snapshot["plan_digest"],
                  "plan_snapshot": snapshot, "intent_review": record["intent_review"],
                  "scope": _ACCEPTANCE_SCOPE, "source_dispatch": False}
        bundle["evidence_digest"] = content_digest(bundle)
        identity = {"kind": "intent_evidence", "plan_id": plan_id, "run_id": run_id, "plan_digest": snapshot["plan_digest"]}
        return identity | ({"json_text": _json(bundle)} if export_format == "json_text" else {"evidence": bundle})

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
    parser.add_argument("--glioma-mutations", action="store_true")
    parser.add_argument("--gene-references", action="store_true")
    parser.add_argument("--protein-pathways", action="store_true")
    parser.add_argument("--denominator-coverage", action="store_true")
    parser.add_argument("--capability", action="append", default=[])
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
            glioma_mutations=args.glioma_mutations, gene_references=args.gene_references,
            protein_pathways=args.protein_pathways,
            denominator_coverage=args.denominator_coverage,
            capabilities=tuple(args.capability),
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
