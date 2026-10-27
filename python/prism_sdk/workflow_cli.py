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

WORKFLOW_CLI_COMMANDS = frozenset({"workflow-catalogue", "workflow-build", "workflow-template", "workflow-execute", "workflow-run", "workflow-interpret"})


def register_workflow_commands(subparsers: Any) -> None:
    interpret = subparsers.add_parser("workflow-interpret", help="inspect bounded glioma source/limit interpretation without a provider or source call")
    interpret.add_argument("--task", required=True)
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
        parser.add_argument("--molecular-glioma", action="store_true", help="include local molecular catalogue coverage and review operations")
        parser.add_argument("--glioma-mutations", action="store_true", help="include the fixed public GDC TP53 aggregate count reader and review operations")
        parser.add_argument("--gene-references", action="store_true", help="include the fixed Ensembl TP53 reference reader and source-reference review operations")
        if name not in {"workflow-build", "workflow-run"}:
            parser.add_argument("--model", help="local model identity, required with --grounded-glioma")
            parser.add_argument("--base-url", default="http://127.0.0.1:11434/v1")
            parser.add_argument("--timeout", type=float, default=120.0)
            parser.add_argument("--approve-provider-call", action="store_true", help="independent local-model authority captured by the local executor")
        commands[name] = parser
    for name in ("workflow-build", "workflow-run"):
        build = commands[name]
        build.add_argument("--task", required=True)
        build.add_argument("--planner", choices=("local-model", "glioma-intent"), default="local-model", help="local model proposal or bounded typed glioma request compiler")
        build.add_argument("--model", help="installed local Ollama model ID; required for local-model planning or grounded synthesis")
        build.add_argument("--base-url", default="http://127.0.0.1:11434/v1")
        build.add_argument("--timeout", type=float, default=120.0)
        build.add_argument("--approve-provider-call", action="store_true")
        build.add_argument("--operation", action="append", default=None, help="exact operation allowlist; repeat as needed")
        build.add_argument("--max-steps", type=int, default=16)
        build.add_argument("--requirements", help="operator-authored workflow requirements JSON input")
        build.add_argument("--planning-attempts", type=int, choices=range(1, 5), default=1, help="maximum local model planning calls, from one to four")
        build.add_argument("--out", required=True, help="reviewable plan destination, saved before source dispatch")
    template = commands["workflow-template"]
    template.add_argument("--template", choices=("evidence", "molecular", "mutations", "gene-references"), default="evidence")
    template.add_argument("--task", required=True)
    template.add_argument("--no-tcga", action="store_true")
    template.add_argument("--literature-limit", type=int, help="evidence template literature limit, default 10")
    template.add_argument("--trial-page-size", type=int, help="evidence template trial page size, from one to 100; default 25")
    template.add_argument("--trial-max-pages", type=int, help="evidence template trial page count, from one to five; default 2")
    template.add_argument("--no-gwas", action="store_true")
    template.add_argument("--association-page-size", type=int, help="molecular template Open Targets page size, default 10")
    template.add_argument("--gwas-page-size", type=int, help="molecular template GWAS page size, default 10")
    template.add_argument("--gwas-max-pages", type=int, help="molecular template GWAS page cap, default 1")
    template.add_argument("--out", required=True)
    execute = commands["workflow-execute"]
    execute.add_argument("--plan", required=True)
    execute.add_argument("--resume", action="store_true")
    execute.add_argument("--retry-failed", action="store_true", help="explicit failed/interrupted step retry, at most three attempts")
    execute.add_argument("--resume-cancelled", action="store_true", help="explicitly release a retained cancellation hold on resume")
    commands["workflow-run"].set_defaults(resume=False, retry_failed=False, resume_cancelled=False)
    for name in ("workflow-execute", "workflow-run"):
        execute = commands[name]
        execute.add_argument("--approve-source-dispatch", action="store_true")
        execute.add_argument("--checkpoint-store", help="public-value checkpoint destination, atomically saved per step")
        execute.add_argument("--max-step-calls", type=int, default=32)
        execute.add_argument("--retrieved-at", help="explicit whole-second UTC source clock; defaults to current UTC")
        execute.add_argument("--outputs-file", help="public output value destination; stdout remains metadata-only")
        execute.add_argument("--cancel-file", help="request execution cancellation at a step boundary by creating this local signal file")


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
        if args.resume_cancelled and not args.resume:
            raise WorkflowValidationError("releasing workflow cancellation requires an explicit resume")
        paths.append(("plan_input", Path(args.plan), False))
    if args.command in {"workflow-execute", "workflow-run"}:
        if args.cancel_file:
            signal_path = Path(args.cancel_file)
            if not signal_path.parent.is_dir():
                raise WorkflowValidationError("workflow cancellation signal parent must already exist")
            paths.append(("cancel_signal", signal_path, False))
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
        if not writable and role != "cancel_signal" and not path.is_file():
            raise WorkflowValidationError("workflow input file must exist")
        normalized.append((role, path, writable, os.path.normcase(str(path.resolve()))))
    for index, (role, path, writable, identity) in enumerate(normalized):
        for other_role, other, other_writable, other_identity in normalized[:index]:
            if not (writable or other_writable or role == "cancel_signal" or other_role == "cancel_signal"):
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
    if args.molecular_glioma or getattr(args, "template", None) == "molecular":
        from .glioma_molecular_workflow import register_glioma_molecular_workflow_operations
        register_glioma_molecular_workflow_operations(catalog)
    if getattr(args, "glioma_mutations", False) or getattr(args, "template", None) == "mutations":
        from .reviewed_gdc_gene_mutation_retrieval import register_gdc_gene_mutation_operations
        from .glioma_mutation_workflow import register_glioma_mutation_workflow_operations
        register_gdc_gene_mutation_operations(catalog)
        register_glioma_mutation_workflow_operations(catalog)
    if getattr(args, "gene_references", False) or getattr(args, "template", None) == "gene-references":
        from .reviewed_ensembl_gene_retrieval import register_ensembl_gene_operations
        from .glioma_gene_reference_workflow import register_glioma_gene_reference_workflow_operations
        register_ensembl_gene_operations(catalog)
        register_glioma_gene_reference_workflow_operations(catalog)
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
    if args.command == "workflow-interpret":
        from .glioma_workflow_intent import interpret_glioma_workflow_request
        return {"command": args.command, **interpret_glioma_workflow_request(args.task).to_dict()}
    mcp_configs = _preflight_paths(args)
    if args.command in {"workflow-execute", "workflow-run"} and not 1 <= args.max_step_calls <= 32:
        raise WorkflowValidationError("workflow dispatch budget must be between one and thirty-two")
    if args.command == "workflow-template" and args.template == "molecular" and args.grounded_glioma:
        raise WorkflowValidationError("molecular catalogue templates do not implement model synthesis")
    if args.command == "workflow-template":
        if args.template in {"mutations", "gene-references"} and (args.grounded_glioma or args.no_tcga or args.no_gwas
                or any(getattr(args, key) is not None for key in ("literature_limit", "trial_page_size", "trial_max_pages", "association_page_size", "gwas_page_size", "gwas_max_pages"))):
            raise WorkflowValidationError("fixed TP53 review templates do not accept evidence, molecular or synthesis options")
        if args.template == "molecular" and (args.no_tcga or any(getattr(args, key) is not None for key in ("literature_limit", "trial_page_size", "trial_max_pages"))):
            raise WorkflowValidationError("evidence template source options cannot be applied to a molecular template")
        if args.template == "evidence" and (args.no_gwas or any(getattr(args, key) is not None for key in ("association_page_size", "gwas_page_size", "gwas_max_pages"))):
            raise WorkflowValidationError("molecular template source options cannot be applied to an evidence template")
    if args.command in {"workflow-build", "workflow-run"}:
        if not 1 <= args.max_steps <= 32:
            raise WorkflowValidationError("workflow step bound must be between one and thirty-two")
        if args.operation is not None and (not 1 <= len(args.operation) <= 128 or len(set(args.operation)) != len(args.operation)):
            raise WorkflowValidationError("workflow operation allowlist is duplicated or outside its bound")
    retrieved_at = _source_clock(args) if args.command in {"workflow-execute", "workflow-run"} else None
    requirements = _read_json(args.requirements, 1_000_000) if getattr(args, "requirements", None) is not None else None
    if requirements is not None and not isinstance(requirements, dict):
        raise WorkflowValidationError("workflow requirements must be a JSON object")
    if requirements is not None:
        from .workflow_requirements import validate_workflow_requirements
        requirements = validate_workflow_requirements(requirements, task_digest=content_digest({"task": args.task})).to_dict()
    intent = None
    if args.command in {"workflow-build", "workflow-run"} and args.planner == "glioma-intent":
        from .glioma_workflow_intent import interpret_glioma_workflow_request
        if args.planning_attempts != 1:
            raise WorkflowValidationError("typed glioma intent planning does not invoke or repair model proposals")
        intent = interpret_glioma_workflow_request(args.task)
        if intent.status != "ready":
            return {"command": args.command, "status": "needs_clarification", "interpretation": intent.to_dict(), "source_dispatch": False}
    needs_runtime = args.grounded_glioma or (args.command in {"workflow-build", "workflow-run"} and args.planner == "local-model")
    runtime = _local_runtime(args) if needs_runtime else None
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
        return _execute_prepared_workflow_command(args, catalog, retrieved_at, requirements, runtime, intent=intent)


def _execute_prepared_workflow_command(args, catalog, retrieved_at, requirements, runtime, *, intent=None):
    intent_receipt = None
    if args.command == "workflow-catalogue":
        return {"command": args.command, **catalog.to_dict()}
    if args.command in {"workflow-template", "workflow-build", "workflow-run"}:
        if args.command == "workflow-template":
            proposal_builder = glioma_evidence_workflow_proposal
            if args.template == "mutations":
                from .glioma_mutation_workflow import compile_glioma_mutation_workflow
                plan = compile_glioma_mutation_workflow(args.task, catalog, requirements=requirements)
            elif args.template == "gene-references":
                from .glioma_gene_reference_workflow import compile_glioma_gene_reference_workflow
                plan = compile_glioma_gene_reference_workflow(args.task, catalog, requirements=requirements)
            elif args.template == "molecular":
                if args.grounded_glioma:
                    raise WorkflowValidationError("molecular catalogue templates do not implement model synthesis")
                from .glioma_molecular_workflow import glioma_molecular_workflow_proposal
                proposal = glioma_molecular_workflow_proposal(args.task, include_gwas=not args.no_gwas,
                    association_page_size=10 if args.association_page_size is None else args.association_page_size,
                    gwas_page_size=10 if args.gwas_page_size is None else args.gwas_page_size,
                    gwas_max_pages=1 if args.gwas_max_pages is None else args.gwas_max_pages)
            elif args.grounded_glioma:
                from .glioma_workflow_synthesis import glioma_grounded_workflow_proposal
                proposal_builder = glioma_grounded_workflow_proposal
            if args.template == "evidence":
                proposal = proposal_builder(args.task, include_tcga=not args.no_tcga, literature_limit=10 if args.literature_limit is None else args.literature_limit,
                                             trial_page_size=25 if args.trial_page_size is None else args.trial_page_size,
                                             trial_max_pages=2 if args.trial_max_pages is None else args.trial_max_pages)
            if args.template in {"evidence", "molecular"}:
                plan = compile_workflow_plan(proposal, catalog, task=args.task)
        elif args.planner == "glioma-intent":
            from .glioma_workflow_intent import compile_glioma_workflow_request, interpret_glioma_workflow_request
            intent = intent if intent is not None else interpret_glioma_workflow_request(args.task)
            intent_receipt = intent.to_dict()
            if intent.status != "ready":
                return {"command": args.command, "status": "needs_clarification", "interpretation": intent_receipt, "source_dispatch": False}
            if args.planning_attempts != 1:
                raise WorkflowValidationError("typed glioma intent planning does not invoke or repair model proposals")
            plan = compile_glioma_workflow_request(args.task, catalog, intent=intent, grounded=args.grounded_glioma)
            data = plan.to_dict()
            if len(data["steps"]) > args.max_steps or (args.operation is not None and any(step["operation"] not in args.operation for step in data["steps"])):
                raise WorkflowValidationError("typed glioma plan exceeds its explicit step or operation allowlist")
            if requirements is not None:
                from .workflow_requirements import merge_workflow_requirements
                combined = merge_workflow_requirements(args.task, data["requirements"], requirements, catalog=catalog)
                plan = compile_workflow_plan({key: data[key] for key in ("name", "steps", "outputs")}, catalog, task=args.task, requirements=combined)
        else:
            plan = LocalWorkflowBuilder(catalog, runtime, model=args.model).build(
                args.task, approve_provider_call=args.approve_provider_call, operation_ids=args.operation, max_steps=args.max_steps,
                requirements=requirements, max_planning_attempts=args.planning_attempts)
        save_workflow_checkpoint(args.out, plan.to_dict())
        if args.command != "workflow-run":
            return {"command": args.command, "status": "planned", "plan_digest": plan.plan_digest,
                    "plan_file": str(Path(args.out).resolve()), "step_count": len(plan.to_dict()["steps"]), "source_dispatch": False,
                    **({"interpretation": intent_receipt} if intent_receipt is not None else {})}
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
    from .workflow_builder import WorkflowCancellationToken
    cancellation = WorkflowCancellationToken()
    def persist(value):
        if args.cancel_file and (Path(args.cancel_file).exists() or Path(args.cancel_file).is_symlink()):
            cancellation.cancel()
        if args.checkpoint_store:
            save_workflow_checkpoint(args.checkpoint_store, value)
    if args.cancel_file and Path(args.cancel_file).exists():
        cancellation.cancel()
    run = execute_workflow_plan(plan, catalog, approved=args.approve_source_dispatch, checkpoint=checkpoint,
                                max_step_calls=args.max_step_calls, retry_failed=args.retry_failed,
                                retrieved_at=retrieved_at, on_checkpoint=persist, cancellation=cancellation,
                                resume_cancelled=args.resume_cancelled)
    if args.outputs_file:
        save_workflow_checkpoint(args.outputs_file, {"plan_digest": plan.plan_digest, "status": run.status, "outputs": run.outputs})
    result = {"command": args.command, **run.to_dict(), "checkpoint_file": args.checkpoint_store, "outputs_file": args.outputs_file,
              "output_review_statuses": _output_review_statuses(run.outputs)}
    if args.command == "workflow-run":
        result["plan_file"] = str(Path(args.out).resolve())
        if intent_receipt is not None:
            result["interpretation"] = intent_receipt
    return result


def _output_review_statuses(outputs: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Expose audit disposition without promoting a completed graph to approved science."""
    from .glioma_workflow_synthesis import GLIOMA_DRAFT_AUDIT_SCHEMA
    from .glioma_molecular_workflow import GLIOMA_MOLECULAR_REVIEW_SCHEMA
    from .glioma_workflow import GLIOMA_REVIEW_HANDOFF_SCHEMA
    from .glioma_mutation_workflow import GLIOMA_MUTATION_REVIEW_SCHEMA
    from .glioma_gene_reference_workflow import GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA
    reviewed_schemas = {
        GLIOMA_DRAFT_AUDIT_SCHEMA: {"needs_evidence", "blocked", "held_for_scientific_human_review"},
        GLIOMA_MOLECULAR_REVIEW_SCHEMA: {"needs_evidence", "awaiting_scientific_human_review"},
        GLIOMA_REVIEW_HANDOFF_SCHEMA: {"needs_evidence", "awaiting_human_review"},
        GLIOMA_MUTATION_REVIEW_SCHEMA: {"needs_evidence", "awaiting_scientific_human_review"},
        GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA: {"needs_evidence", "awaiting_scientific_human_review"},
    }
    statuses = {}
    for step_id, value in outputs.items():
        if not isinstance(value, dict) or not isinstance(value.get("schema"), str) or value["schema"] not in reviewed_schemas:
            continue
        row = {"schema": value["schema"]}
        status = value.get("status")
        if isinstance(status, str) and status in reviewed_schemas[value["schema"]]:
            row["status"] = status
        for key in ("human_review_required", "scientific_support_established", "clinical_actions_authorized"):
            if type(value.get(key)) is bool:
                row[key] = value[key]
        statuses[step_id] = row
    return statuses
