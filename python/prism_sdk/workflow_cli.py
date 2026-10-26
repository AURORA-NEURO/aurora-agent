"""Local-model workflow commands; no key, hosted provider or MCP server required.

Plans are reviewable JSON. Explicit checkpoint destinations retain public values, while stdout
contains only metadata. Source dispatch, local provider calls and spec imports have independent
approval flags. These commands do not accept credentials or patient records.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
from typing import Any
from urllib.parse import urlsplit

from .glioma_workflow import glioma_evidence_workflow_proposal, register_glioma_workflow_operations
from .llm_runtime import LLMRuntime, ollama_provider
from .workflow_builder import (LocalWorkflowBuilder, WorkflowValidationError, compile_workflow_plan,
                               execute_workflow_plan, load_workflow_checkpoint,
                               save_workflow_checkpoint, validate_workflow_plan)
from .workflow_integrations import builtin_workflow_operation_catalog
from .workflow_openapi import import_openapi_operations

WORKFLOW_CLI_COMMANDS = frozenset({"workflow-catalogue", "workflow-build", "workflow-template", "workflow-execute", "workflow-run"})


def register_workflow_commands(subparsers: Any) -> None:
    commands = {}
    for name, description in (
        ("workflow-catalogue", "list executable integrations without contacting sources"),
        ("workflow-build", "compile natural language into a checked DAG using local Ollama"),
        ("workflow-template", "compile the glioma public-evidence workflow without a model call"),
        ("workflow-execute", "execute or resume a checked read-only integration DAG"),
        ("workflow-run", "build, save and execute a natural-language workflow with local Ollama"),
    ):
        parser = subparsers.add_parser(name, help=description)
        parser.add_argument("--openapi-config", action="append", default=[], help="JSON reviewed OpenAPI import configuration")
        parser.add_argument("--approve-spec-import", action="store_true")
        commands[name] = parser
    for name in ("workflow-build", "workflow-run"):
        build = commands[name]
        build.add_argument("--task", required=True)
        build.add_argument("--model", required=True, help="installed local Ollama model ID")
        build.add_argument("--base-url", default="http://127.0.0.1:11434/v1")
        build.add_argument("--timeout", type=float, default=120.0)
        build.add_argument("--approve-provider-call", action="store_true")
        build.add_argument("--operation", action="append", default=None, help="exact operation allowlist; repeat as needed")
        build.add_argument("--max-steps", type=int, default=16)
        build.add_argument("--out", required=True, help="reviewable plan destination, saved before source dispatch")
    template = commands["workflow-template"]
    template.add_argument("--task", required=True)
    template.add_argument("--no-tcga", action="store_true")
    template.add_argument("--literature-limit", type=int, default=10)
    template.add_argument("--out", required=True)
    execute = commands["workflow-execute"]
    execute.add_argument("--plan", required=True)
    execute.add_argument("--resume", action="store_true")
    execute.add_argument("--retry-failed", action="store_true", help="explicit failed/interrupted step retry, at most three attempts")
    commands["workflow-run"].set_defaults(resume=False, retry_failed=False)
    for name in ("workflow-execute", "workflow-run"):
        execute = commands[name]
        execute.add_argument("--approve-source-dispatch", action="store_true")
        execute.add_argument("--checkpoint-store", help="public-value checkpoint destination, atomically saved per step")
        execute.add_argument("--max-step-calls", type=int, default=32)
        execute.add_argument("--retrieved-at", help="explicit whole-second UTC source clock; defaults to current UTC")
        execute.add_argument("--outputs-file", help="public output value destination; stdout remains metadata-only")


def _read_json(path: str | Path, maximum: int = 8_000_000) -> Any:
    target = Path(path)
    if target.is_symlink() or not target.is_file() or target.stat().st_size > maximum:
        raise WorkflowValidationError("workflow input must be a bounded regular file")
    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        result = {}
        for key, value in items:
            if key in result:
                raise WorkflowValidationError("workflow input contains duplicate JSON keys")
            result[key] = value
        return result
    return json.loads(target.read_text(encoding="utf-8"), object_pairs_hook=pairs)


def _preflight_paths(args: argparse.Namespace) -> None:
    """Keep outputs from replacing review inputs or another artifact needed for resume.

    Resolve aliases and existing hardlinks before an approved provider or source can run; a
    destination discovered to be unusable after dispatch would lose its durable result.
    """
    paths: list[tuple[str, Path, bool]] = []
    if len(args.openapi_config) > 64:
        raise WorkflowValidationError("workflow OpenAPI config count exceeds its bound")
    for index, file_name in enumerate(args.openapi_config):
        config = _read_json(file_name, 1_000_000)
        if (not isinstance(config, dict)
                or set(config) != {"document", "integration_id", "server_url", "allowed_hosts", "reviewed_operation_ids"}
                or not isinstance(config["document"], str) or not config["document"]):
            raise WorkflowValidationError("workflow OpenAPI config fields are invalid")
        spec = Path(config["document"])
        if not spec.is_absolute():
            spec = Path(file_name).resolve().parent / spec
        paths.extend([(f"config_{index}", Path(file_name), False), (f"specification_{index}", spec, False)])
    if args.command in {"workflow-build", "workflow-template", "workflow-run"}:
        paths.append(("plan_output", Path(args.out), True))
    if args.command == "workflow-execute":
        if args.resume and not args.checkpoint_store:
            raise WorkflowValidationError("workflow resume requires a checkpoint store")
        if args.retry_failed and not args.resume:
            raise WorkflowValidationError("workflow retry requires an explicit resume")
        paths.append(("plan_input", Path(args.plan), False))
    if args.command in {"workflow-execute", "workflow-run"}:
        if args.checkpoint_store and not args.resume and Path(args.checkpoint_store).exists():
            raise WorkflowValidationError("workflow checkpoint already exists; explicitly resume or choose a new path")
        for role, value in (("checkpoint", args.checkpoint_store), ("public_outputs", args.outputs_file)):
            if value:
                paths.append((role, Path(value), True))
    normalized: list[tuple[str, Path, bool, str]] = []
    for role, path, writable in paths:
        if path.is_symlink() or (path.exists() and not path.is_file()):
            raise WorkflowValidationError("workflow artifacts require regular files without symlink targets")
        if writable and not path.parent.is_dir():
            raise WorkflowValidationError("workflow output parent directory must already exist")
        if not writable and not path.is_file():
            raise WorkflowValidationError("workflow input file must exist")
        normalized.append((role, path, writable, os.path.normcase(str(path.resolve()))))
    for index, (_, path, writable, identity) in enumerate(normalized):
        for _, other, other_writable, other_identity in normalized[:index]:
            if not (writable or other_writable):
                continue  # Reusing one reviewed document as multiple inputs is harmless.
            same_file = identity == other_identity
            if not same_file and path.exists() and other.exists():
                same_file = path.samefile(other)
            if same_file:
                raise WorkflowValidationError("workflow input and output artifact paths must be distinct")


def _source_clock(args: argparse.Namespace) -> str:
    retrieved_at = args.retrieved_at if args.retrieved_at is not None else datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    if not isinstance(retrieved_at, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", retrieved_at):
        raise WorkflowValidationError("workflow source clock must be a whole-second UTC timestamp")
    datetime.strptime(retrieved_at, "%Y-%m-%dT%H:%M:%SZ")
    return retrieved_at


def workflow_cli_catalog(args: argparse.Namespace):
    catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
    if len(args.openapi_config) > 64:
        raise WorkflowValidationError("workflow OpenAPI config count exceeds its bound")
    for file_name in args.openapi_config:
        if args.approve_spec_import is not True:
            raise WorkflowValidationError("workflow OpenAPI import requires approval")
        config = _read_json(file_name, 1_000_000)
        if not isinstance(config, dict) or set(config) != {"document", "integration_id", "server_url", "allowed_hosts", "reviewed_operation_ids"}:
            raise WorkflowValidationError("workflow OpenAPI config fields are invalid")
        spec = Path(config["document"])
        if not spec.is_absolute():
            spec = Path(file_name).resolve().parent / spec
        import_openapi_operations(catalog, _read_json(spec, 4_000_000), integration_id=config["integration_id"],
                                  server_url=config["server_url"], allowed_hosts=config["allowed_hosts"],
                                  reviewed_operation_ids=config["reviewed_operation_ids"], approve_spec_import=True)
    return catalog


def execute_workflow_command(args: argparse.Namespace) -> dict[str, Any]:
    _preflight_paths(args)
    retrieved_at = _source_clock(args) if args.command in {"workflow-execute", "workflow-run"} else None
    catalog = workflow_cli_catalog(args)
    if args.command == "workflow-catalogue":
        return {"command": args.command, **catalog.to_dict()}
    if args.command in {"workflow-template", "workflow-build", "workflow-run"}:
        if args.command == "workflow-template":
            proposal = glioma_evidence_workflow_proposal(args.task, include_tcga=not args.no_tcga, literature_limit=args.literature_limit)
            plan = compile_workflow_plan(proposal, catalog, task=args.task)
        else:
            parsed = urlsplit(args.base_url)
            if parsed.hostname not in {"localhost", "127.0.0.1", "::1"} or parsed.scheme not in {"http", "https"}:
                raise WorkflowValidationError("workflow model endpoint must be loopback")
            runtime = LLMRuntime()
            runtime.register_provider(ollama_provider(base_url=args.base_url, timeout_seconds=args.timeout, max_attempts=1, structured_output_mode="json_schema"))
            plan = LocalWorkflowBuilder(catalog, runtime, model=args.model).build(
                args.task, approve_provider_call=args.approve_provider_call, operation_ids=args.operation, max_steps=args.max_steps)
        save_workflow_checkpoint(args.out, plan.to_dict())
        if args.command != "workflow-run":
            return {"command": args.command, "status": "planned", "plan_digest": plan.plan_digest,
                    "plan_file": str(Path(args.out).resolve()), "step_count": len(plan.to_dict()["steps"]), "source_dispatch": False}
    if args.command not in {"workflow-execute", "workflow-run"}:
        raise WorkflowValidationError("unknown workflow command")
    if args.resume and not args.checkpoint_store:
        raise WorkflowValidationError("workflow resume requires a checkpoint store")
    if args.retry_failed and not args.resume:
        raise WorkflowValidationError("workflow retry requires an explicit resume")
    if args.command == "workflow-execute":
        plan = validate_workflow_plan(_read_json(args.plan), catalog)
    checkpoint = load_workflow_checkpoint(args.checkpoint_store) if args.resume else None
    if args.checkpoint_store and not args.resume and Path(args.checkpoint_store).exists():
        raise WorkflowValidationError("workflow checkpoint already exists; explicitly resume or choose a new path")
    persist = (lambda value: save_workflow_checkpoint(args.checkpoint_store, value)) if args.checkpoint_store else None
    run = execute_workflow_plan(plan, catalog, approved=args.approve_source_dispatch, checkpoint=checkpoint,
                                max_step_calls=args.max_step_calls, retry_failed=args.retry_failed,
                                retrieved_at=retrieved_at, on_checkpoint=persist)
    if args.outputs_file:
        save_workflow_checkpoint(args.outputs_file, {"plan_digest": plan.plan_digest, "status": run.status, "outputs": run.outputs})
    result = {"command": args.command, **run.to_dict(), "checkpoint_file": args.checkpoint_store, "outputs_file": args.outputs_file}
    if args.command == "workflow-run":
        result["plan_file"] = str(Path(args.out).resolve())
    return result
