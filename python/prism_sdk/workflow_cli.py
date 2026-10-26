"""Local-model workflow commands; no key, hosted provider or MCP server required.

Plans are reviewable JSON. Explicit checkpoint destinations retain public values, while stdout
contains only metadata. Source dispatch, local provider calls and spec imports have independent
approval flags. These commands do not accept credentials or patient records.
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
from typing import Any
from urllib.parse import urlsplit

from .authoring import content_digest
from .client import Client
from .glioma_workflow import glioma_evidence_workflow_proposal, register_glioma_workflow_operations
from .llm_runtime import LLMRuntime, ollama_provider
from .workflow_builder import (LocalWorkflowBuilder, WorkflowValidationError, compile_workflow_plan,
                               execute_workflow_plan, load_workflow_checkpoint,
                               save_workflow_checkpoint, validate_workflow_plan)
from .workflow_integrations import builtin_workflow_operation_catalog
from .workflow_mcp import register_mcp_workflow_operations
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
        parser.add_argument("--research-sources", action="store_true", help="include the reviewed OpenAlex, Crossref and Zenodo readers")
        parser.add_argument("--mcp-config", action="append", default=[], help="operator-reviewed stdio MCP configuration; run/catalogue only")
        parser.add_argument("--approve-mcp-discovery", action="store_true", help="authorize configured MCP process startup and tool discovery")
        parser.add_argument("--grounded-glioma", action="store_true", help="include local glioma synthesis and exact citation audit operations")
        if name not in {"workflow-build", "workflow-run"}:
            parser.add_argument("--model", help="local model identity, required with --grounded-glioma")
            parser.add_argument("--base-url", default="http://127.0.0.1:11434/v1")
            parser.add_argument("--timeout", type=float, default=120.0)
            parser.add_argument("--approve-provider-call", action="store_true", help="independent local-model authority captured by the local executor")
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
        build.add_argument("--requirements", help="operator-authored workflow requirements JSON input")
        build.add_argument("--planning-attempts", type=int, choices=range(1, 5), default=1, help="maximum local model planning calls, from one to four")
        build.add_argument("--out", required=True, help="reviewable plan destination, saved before source dispatch")
    template = commands["workflow-template"]
    template.add_argument("--task", required=True)
    template.add_argument("--no-tcga", action="store_true")
    template.add_argument("--literature-limit", type=int, default=10)
    template.add_argument("--trial-page-size", type=int, default=25, help="bounded trial registry page size, from one to 100")
    template.add_argument("--trial-max-pages", type=int, default=2, help="bounded trial registry page count, from one to five")
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


def _mcp_configs(args: argparse.Namespace) -> tuple[dict[str, Any], ...]:
    if not args.mcp_config:
        return ()
    if args.command not in {"workflow-run", "workflow-catalogue"}:
        raise WorkflowValidationError("session-bound MCP configurations require workflow-run or workflow-catalogue")
    if args.approve_mcp_discovery is not True:
        raise WorkflowValidationError("MCP process startup and discovery require explicit approval")
    if len(args.mcp_config) > 8:
        raise WorkflowValidationError("workflow MCP config count exceeds eight")
    configs = []
    integrations = set()
    identifier = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}")
    for file_name in args.mcp_config:
        config = _read_json(file_name, 1_000_000)
        if (not isinstance(config, dict)
                or not {"integration_id", "command", "reviewed_read_only_tools"} <= set(config)
                or set(config) - {"integration_id", "command", "reviewed_read_only_tools", "cwd"}):
            raise WorkflowValidationError("workflow MCP config fields are invalid")
        integration_id, command, reviewed = config["integration_id"], config["command"], config["reviewed_read_only_tools"]
        if not isinstance(integration_id, str) or not identifier.fullmatch(integration_id) or integration_id in integrations:
            raise WorkflowValidationError("workflow MCP integration identities must be valid and unique")
        integrations.add(integration_id)
        if (not isinstance(command, list) or not 1 <= len(command) <= 64
                or any(not isinstance(part, str) or not part or "\x00" in part or len(part.encode("utf-8")) > 8192 for part in command)
                or sum(len(part.encode("utf-8")) for part in command) > 32768):
            raise WorkflowValidationError("workflow MCP command must be a bounded argv list")
        if (not isinstance(reviewed, list) or not 1 <= len(reviewed) <= 512
                or any(not isinstance(name, str) or not identifier.fullmatch(name) for name in reviewed)
                or len(set(reviewed)) != len(reviewed)):
            raise WorkflowValidationError("workflow MCP requires a unique reviewed read-only tool list")
        cwd = config.get("cwd")
        if "cwd" in config:
            if not isinstance(cwd, str) or not cwd or "\x00" in cwd or len(cwd.encode("utf-8")) > 4096:
                raise WorkflowValidationError("workflow MCP cwd must be a bounded directory path")
            directory = Path(cwd)
            if not directory.is_absolute():
                directory = Path(file_name).resolve().parent / directory
            if directory.is_symlink() or not directory.is_dir():
                raise WorkflowValidationError("workflow MCP cwd must be an existing directory")
            cwd = str(directory.resolve())
        configs.append({**config, "command": tuple(command), "cwd": cwd})
    return tuple(configs)


def _preflight_paths(args: argparse.Namespace) -> tuple[dict[str, Any], ...]:
    """Keep outputs from replacing review inputs or another artifact needed for resume.

    Resolve aliases and existing hardlinks before an approved provider or source can run; a
    destination discovered to be unusable after dispatch would lose its durable result.
    """
    paths: list[tuple[str, Path, bool]] = []
    mcp_configs = _mcp_configs(args)
    for index, file_name in enumerate(args.mcp_config):
        paths.append((f"mcp_config_{index}", Path(file_name), False))
    if getattr(args, "requirements", None) is not None:
        paths.append(("requirements_input", Path(args.requirements), False))
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
    return mcp_configs


def _source_clock(args: argparse.Namespace) -> str:
    retrieved_at = args.retrieved_at if args.retrieved_at is not None else datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    if not isinstance(retrieved_at, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", retrieved_at):
        raise WorkflowValidationError("workflow source clock must be a whole-second UTC timestamp")
    datetime.strptime(retrieved_at, "%Y-%m-%dT%H:%M:%SZ")
    return retrieved_at


def _local_runtime(args: argparse.Namespace) -> LLMRuntime:
    if not isinstance(args.model, str) or not args.model.strip():
        raise WorkflowValidationError("local-model workflow operations require an explicit model")
    parsed = urlsplit(args.base_url)
    if (parsed.hostname not in {"localhost", "127.0.0.1", "::1"} or parsed.scheme not in {"http", "https"}
            or parsed.username is not None or parsed.password is not None or parsed.fragment or parsed.query):
        raise WorkflowValidationError("workflow model endpoint must be loopback without authority or query fields")
    runtime = LLMRuntime()
    runtime.register_provider(ollama_provider(base_url=args.base_url, timeout_seconds=args.timeout, max_attempts=1, structured_output_mode="json_schema"))
    return runtime


def workflow_cli_catalog(args: argparse.Namespace, *, runtime: LLMRuntime | None = None):
    catalog = register_glioma_workflow_operations(builtin_workflow_operation_catalog())
    if args.research_sources:
        from .reviewed_research_sources import register_research_source_operations
        register_research_source_operations(catalog)
    if args.grounded_glioma:
        from .glioma_workflow_synthesis import register_glioma_synthesis_operations
        register_glioma_synthesis_operations(catalog, runtime=runtime if runtime is not None else _local_runtime(args),
                                             model=args.model, approve_provider_call=args.approve_provider_call)
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
    mcp_configs = _preflight_paths(args)
    retrieved_at = _source_clock(args) if args.command in {"workflow-execute", "workflow-run"} else None
    requirements = _read_json(args.requirements, 1_000_000) if getattr(args, "requirements", None) is not None else None
    if requirements is not None and not isinstance(requirements, dict):
        raise WorkflowValidationError("workflow requirements must be a JSON object")
    if requirements is not None:
        from .workflow_requirements import validate_workflow_requirements
        requirements = validate_workflow_requirements(requirements, task_digest=content_digest({"task": args.task})).to_dict()
    runtime = _local_runtime(args) if args.command in {"workflow-build", "workflow-run"} or args.grounded_glioma else None
    catalog = workflow_cli_catalog(args, runtime=runtime) if args.grounded_glioma else workflow_cli_catalog(args)
    existing_ids = {operation.operation_id for operation in catalog.operations()}
    configured_ids = [f"{config['integration_id']}.{name}" for config in mcp_configs for name in config["reviewed_read_only_tools"]]
    if len(configured_ids) != len(set(configured_ids)) or set(configured_ids) & existing_ids:
        raise WorkflowValidationError("workflow MCP operations conflict with existing registrations")
    with ExitStack() as clients:
        for config in mcp_configs:
            client = clients.enter_context(Client(config["command"], cwd=config["cwd"]))
            register_mcp_workflow_operations(catalog, client, integration_id=config["integration_id"],
                                              reviewed_read_only_tools=config["reviewed_read_only_tools"], approve_tool_discovery=True)
        return _execute_prepared_workflow_command(args, catalog, retrieved_at, requirements, runtime)


def _execute_prepared_workflow_command(args, catalog, retrieved_at, requirements, runtime):
    if args.command == "workflow-catalogue":
        return {"command": args.command, **catalog.to_dict()}
    if args.command in {"workflow-template", "workflow-build", "workflow-run"}:
        if args.command == "workflow-template":
            proposal_builder = glioma_evidence_workflow_proposal
            if args.grounded_glioma:
                from .glioma_workflow_synthesis import glioma_grounded_workflow_proposal
                proposal_builder = glioma_grounded_workflow_proposal
            proposal = proposal_builder(args.task, include_tcga=not args.no_tcga, literature_limit=args.literature_limit,
                                         trial_page_size=args.trial_page_size, trial_max_pages=args.trial_max_pages)
            plan = compile_workflow_plan(proposal, catalog, task=args.task)
        else:
            plan = LocalWorkflowBuilder(catalog, runtime, model=args.model).build(
                args.task, approve_provider_call=args.approve_provider_call, operation_ids=args.operation, max_steps=args.max_steps,
                requirements=requirements, max_planning_attempts=args.planning_attempts)
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
    result = {"command": args.command, **run.to_dict(), "checkpoint_file": args.checkpoint_store, "outputs_file": args.outputs_file,
              "output_review_statuses": _output_review_statuses(run.outputs)}
    if args.command == "workflow-run":
        result["plan_file"] = str(Path(args.out).resolve())
    return result


def _output_review_statuses(outputs: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Expose audit disposition without promoting a completed graph to approved science."""
    from .glioma_workflow_synthesis import GLIOMA_DRAFT_AUDIT_SCHEMA
    statuses = {}
    for step_id, value in outputs.items():
        if not isinstance(value, dict) or value.get("schema") != GLIOMA_DRAFT_AUDIT_SCHEMA:
            continue
        row = {"schema": GLIOMA_DRAFT_AUDIT_SCHEMA}
        status = value.get("status")
        if isinstance(status, str) and status in {"needs_evidence", "blocked", "held_for_scientific_human_review"}:
            row["status"] = status
        for key in ("human_review_required", "scientific_support_established", "clinical_actions_authorized"):
            if type(value.get(key)) is bool:
                row[key] = value[key]
        statuses[step_id] = row
    return statuses
