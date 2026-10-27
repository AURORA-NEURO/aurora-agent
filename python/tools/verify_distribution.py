"""Build and exercise the distributable Python SDK without using the source tree at runtime."""

from __future__ import annotations

import configparser
import json
from email.parser import BytesParser
from email.policy import default
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import venv
import zipfile


PYTHON_ROOT = Path(__file__).resolve().parents[1]
PACKAGE_ROOT = PYTHON_ROOT / "prism_sdk"
PROJECT = tomllib.loads((PYTHON_ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]


def run(arguments: list[str], *, cwd: Path, env: dict[str, str] | None = None,
        input_text: str | None = None) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        arguments,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
        input=input_text,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {arguments!r}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def verify_wheel(wheel_path: Path) -> int:
    expected_modules = {
        path.relative_to(PYTHON_ROOT).as_posix()
        for path in PACKAGE_ROOT.rglob("*.py")
    }
    with zipfile.ZipFile(wheel_path) as wheel:
        names = set(wheel.namelist())
        missing = sorted(expected_modules - names)
        if missing:
            raise RuntimeError(f"wheel omits Python package modules: {missing[:12]!r}")
        unexpected = sorted(name for name in names if name.endswith(".py") and not name.startswith("prism_sdk/"))
        if unexpected:
            raise RuntimeError(f"wheel includes Python files outside the SDK package: {unexpected[:12]!r}")

        metadata_paths = [name for name in names if name.endswith(".dist-info/METADATA")]
        if len(metadata_paths) != 1:
            raise RuntimeError("wheel must contain exactly one distribution METADATA file")
        metadata = BytesParser(policy=default).parsebytes(wheel.read(metadata_paths[0]))
        expected_name = PROJECT["name"].lower().replace("_", "-")
        actual_name = metadata.get("Name", "").lower().replace("_", "-")
        if actual_name != expected_name:
            raise RuntimeError("wheel distribution name does not match python/pyproject.toml")
        if metadata.get("Version") != PROJECT["version"]:
            raise RuntimeError("wheel version does not match python/pyproject.toml")
        if metadata.get("Requires-Python") != PROJECT["requires-python"]:
            raise RuntimeError("wheel Python requirement does not match python/pyproject.toml")
        if not PROJECT.get("dependencies", []) and metadata.get_all("Requires-Dist"):
            raise RuntimeError("dependency-free SDK wheel unexpectedly declares runtime dependencies")

        entrypoint_paths = [name for name in names if name.endswith(".dist-info/entry_points.txt")]
        if len(entrypoint_paths) != 1:
            raise RuntimeError("wheel must contain exactly one console-entry-point manifest")
        entrypoints = configparser.ConfigParser()
        entrypoints.read_string(wheel.read(entrypoint_paths[0]).decode("utf-8"))
        expected_entrypoint = PROJECT["scripts"]["aurora-agent"]
        actual_entrypoint = entrypoints.get("console_scripts", "aurora-agent", fallback=None)
        if actual_entrypoint != expected_entrypoint:
            raise RuntimeError("wheel CLI entry point does not match python/pyproject.toml")
    return len(expected_modules)


def main() -> int:
    clean_env = os.environ.copy()
    clean_env.pop("PYTHONPATH", None)
    # Bytecode caches are unnecessary for installed-source import and behavior checks.
    clean_env["PYTHONDONTWRITEBYTECODE"] = "1"
    with tempfile.TemporaryDirectory(prefix="aurora-python-wheel-") as temporary:
        temporary_root = Path(temporary)
        # Build from a pristine package copy. Repeated local wheel builds must not discover
        # stale build/lib trees as namespace packages or recursively package earlier builds.
        source_root = temporary_root / "source"
        source_root.mkdir()
        for name in ("pyproject.toml", "README.md"):
            shutil.copy2(PYTHON_ROOT / name, source_root / name)
        shutil.copytree(PACKAGE_ROOT, source_root / "prism_sdk",
                        ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo"))
        wheel_dir = temporary_root / "wheel"
        wheel_dir.mkdir()
        run(
            [
                sys.executable,
                "-m",
                "pip",
                "wheel",
                "--disable-pip-version-check",
                "--no-deps",
                "--wheel-dir",
                str(wheel_dir),
                str(source_root),
            ],
            cwd=temporary_root,
            env=clean_env,
        )
        wheels = list(wheel_dir.glob("*.whl"))
        if len(wheels) != 1:
            raise RuntimeError(f"expected one Python SDK wheel, found {len(wheels)}")
        module_count = verify_wheel(wheels[0])

        environment = temporary_root / "venv"
        venv.EnvBuilder(with_pip=True).create(environment)
        if os.name == "nt":
            interpreter = environment / "Scripts" / "python.exe"
            executable = environment / "Scripts" / "aurora-agent.exe"
        else:
            interpreter = environment / "bin" / "python"
            executable = environment / "bin" / "aurora-agent"
        run(
            [
                str(interpreter),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-deps",
                "--no-index",
                "--no-compile",
                str(wheels[0]),
            ],
            cwd=temporary_root,
            env=clean_env,
        )
        run(
            [
                str(interpreter),
                "-c",
                "import prism_sdk, prism_sdk.cli; from pathlib import Path; "
                "from prism_sdk.autonomy import AutonomousAgent; "
                "from prism_sdk.autonomous_goal_agent import AutonomousGoalAgentRuntime; "
                "from prism_sdk.autonomous_goal_control_loop import AutonomousGoalControlLoop; "
                "from prism_sdk.autonomous_goal_worker import AutonomousGoalWorker; "
                "assert callable(prism_sdk.cli.main); "
                "assert len(prism_sdk.__all__) == len(set(prism_sdk.__all__)), "
                "'public exports must be unique'; "
                "assert all(name in prism_sdk.__dict__ for name in prism_sdk.__all__), "
                "'every declared public export must resolve in the installed package'; "
                "autonomy_exports = ('AutonomousAgent', 'AutonomousGoalLedger', "
                "'AutonomousGoalControlLoop', 'AutonomousGoalWorker', 'LocalWorkflowBuilder', "
                "'WorkflowOperationCatalog', 'compile_workflow_plan', 'execute_workflow_plan', "
                "'import_openapi_operations'); "
                "assert all(type(getattr(prism_sdk, name)).__name__ != '_UnavailableFeature' "
                "for name in autonomy_exports), 'core autonomy exports must be implemented'; "
                "assert all(callable(getattr(prism_sdk, name)) for name in autonomy_exports), "
                "'core autonomy exports must be callable'; "
                "workflow_exports = ('ResearchSourceQueryPlan', 'ResearchSourceHttpResponse', "
                "'prepare_research_source_query', 'execute_research_source_query', "
                "'register_research_source_operations', 'McpWorkflowError', "
                "'mcp_workflow_server_identity', 'register_mcp_workflow_operations', "
                "'LocalGliomaWorkflowSynthesizer', 'prepare_glioma_synthesis_context', "
                "'audit_glioma_synthesis_draft', 'register_glioma_synthesis_operations', "
                "'glioma_grounded_workflow_proposal', 'WorkflowRequirements', "
                "'compile_workflow_requirements', 'validate_workflow_requirements', "
                "'audit_workflow_requirements', 'enforce_workflow_step_requirements', 'merge_workflow_requirements', "
                "'WorkflowCancellationToken', 'GliomaWorkflowIntent', 'interpret_glioma_workflow_request', "
                "'compile_glioma_workflow_request', 'analyze_glioma_molecular_coverage', "
                "'glioma_molecular_review_handoff', 'register_glioma_molecular_workflow_operations', "
                "'glioma_molecular_workflow_proposal', 'WorkflowConnectionContext', "
                "'WorkflowConnectionResource', 'WorkflowConnectionSession', 'WorkflowConnectionError', "
                "'WorkflowArtifactSink', 'PreparedWorkflowArtifact', 'SQLiteWorkflowArtifactAuthorizationStore', "
                "'WorkflowArtifactError', 'WorkflowArtifactCancelled', 'audit_workflow_reference_paths', "
                "'ReviewedGdcGeneMutationRetrievalConfig', 'ReviewedGdcGeneMutationRetrievalPlan', "
                "'ReviewedGdcGeneMutationRetrievalAdapter', 'validate_reviewed_gdc_gene_mutation_bundle', "
                "'EnsemblGeneRetrievalConfig', 'EnsemblGeneHttpResponse', 'retrieve_ensembl_gene_references', "
                "'validate_ensembl_gene_bundle', 'register_ensembl_gene_operations', 'register_gdc_gene_mutation_operations', "
                "'analyze_glioma_mutation_counts', 'validate_glioma_mutation_analysis', 'glioma_mutation_review_handoff', "
                "'validate_glioma_mutation_handoff', 'register_glioma_mutation_workflow_operations', "
                "'compile_glioma_mutation_workflow', 'analyze_glioma_gene_references', "
                "'validate_glioma_gene_reference_analysis', 'glioma_gene_reference_review_handoff', "
                "'validate_glioma_gene_reference_handoff', 'register_glioma_gene_reference_workflow_operations', "
                "'compile_glioma_gene_reference_workflow'); "
                "assert all(name in prism_sdk.__all__ and callable(getattr(prism_sdk, name)) "
                "and type(getattr(prism_sdk, name)).__name__ != '_UnavailableFeature' "
                "for name in workflow_exports), 'new workflow exports must be real installed APIs'; "
                "from prism_sdk import reviewed_research_sources, workflow_mcp, "
                "glioma_workflow_synthesis, workflow_requirements, workflow_connections, workflow_artifact; "
                "assert prism_sdk.register_research_source_operations is "
                "reviewed_research_sources.register_research_source_operations; "
                "assert prism_sdk.register_mcp_workflow_operations is "
                "workflow_mcp.register_mcp_workflow_operations; "
                "assert prism_sdk.LocalGliomaWorkflowSynthesizer is "
                "glioma_workflow_synthesis.LocalGliomaWorkflowSynthesizer; "
                "assert prism_sdk.WorkflowRequirements is workflow_requirements.WorkflowRequirements; "
                "assert prism_sdk.WorkflowConnectionSession is workflow_connections.WorkflowConnectionSession; "
                "assert prism_sdk.WorkflowArtifactSink is workflow_artifact.WorkflowArtifactSink; "
                "subject = prism_sdk.WorkflowConnectionContext('tenant', 'actor', 'session'); "
                "assert subject.to_dict() == {'tenant_id': 'tenant', 'actor_id': 'actor', 'session_id': 'session'}; "
                "scope = prism_sdk.WorkflowConnectionResource('metadata.read', {'owner': ['reviewed']}); "
                "assert scope.to_dict()['argument_values'] == {'owner': ['reviewed']}; "
                "source_catalog = prism_sdk.register_research_source_operations(prism_sdk.WorkflowOperationCatalog()); "
                "assert {op.operation_id for op in source_catalog.operations()} == "
                "{'openalex.search_works', 'crossref.search_works', 'zenodo.search_records'}; "
                "task = 'Retrieve two public glioma metadata records'; "
                "requirements = prism_sdk.compile_workflow_requirements(task, "
                "required_operations=['crossref.search_works'], output_operations=['crossref.search_works'], "
                "argument_rules=[{'operation': 'crossref.search_works', 'path': '/limit', "
                "'comparison': 'equals', 'value': 2}], max_calls_by_operation={'crossref.search_works': 1}); "
                "proposal = {'name': 'Installed contract smoke', 'steps': [{'id': 'metadata', "
                "'operation': 'crossref.search_works', 'arguments': {'query': 'glioma', 'limit': 2}, "
                "'depends_on': []}], 'outputs': ['metadata']}; "
                "plan = prism_sdk.compile_workflow_plan(proposal, source_catalog, task=task, requirements=requirements); "
                "assert plan.to_dict()['requirements']['requirements_digest'] == requirements.requirements_digest; "
                "assert prism_sdk.audit_workflow_requirements(proposal, requirements, source_catalog)['contract_satisfied'] is True; "
                "merged = prism_sdk.merge_workflow_requirements(task, requirements, requirements, catalog=source_catalog); "
                "assert merged.requirements_digest == requirements.requirements_digest; "
                "control = prism_sdk.WorkflowCancellationToken(); control.cancel(); "
                "cancelled = prism_sdk.execute_workflow_plan(plan, source_catalog, cancellation=control); "
                "assert cancelled.status == 'cancelled' and not cancelled.checkpoint['results']; "
                "assert cancelled.checkpoint['cancel_requested'] is True; "
                "intent = prism_sdk.interpret_glioma_workflow_request('Review public glioma evidence.'); "
                "assert intent.status == 'ready' and intent.to_dict()['provider_calls'] == 0; "
                "async_exports = ((AutonomousGoalWorker, 'run_async'), "
                "(AutonomousGoalControlLoop, 'run_async'), "
                "(AutonomousGoalAgentRuntime, 'run_async'), "
                "(AutonomousGoalAgentRuntime, 'run_with_trace_async'), "
                "(AutonomousAgent, 'run_goal_control_loop_async')); "
                "assert all(callable(getattr(owner, method, None)) for owner, method in async_exports), "
                "'async autonomous goal entry points must be present in the installed wheel'; "
                "assert not hasattr(prism_sdk, 'aurora_sdk_symbol_that_does_not_exist'), "
                "'unknown exports must not be fabricated'; "
                "assert Path(prism_sdk.__file__).resolve().is_relative_to(Path.cwd() / 'venv')",
            ],
            cwd=temporary_root,
            env=clean_env,
        )
        help_output = run([str(executable), "--help"], cwd=temporary_root, env=clean_env)
        if "grounded-portfolio" not in help_output.stdout:
            raise RuntimeError("installed aurora-agent CLI help omits the grounded portfolio command")
        for command in ("workflow-catalogue", "workflow-build", "workflow-run", "workflow-template", "workflow-execute", "workflow-interpret"):
            if command not in help_output.stdout:
                raise RuntimeError("installed aurora-agent CLI help omits a workflow command")
        catalogue_output = run([str(executable), "workflow-catalogue", "--research-sources"], cwd=temporary_root, env=clean_env)
        catalogue = json.loads(catalogue_output.stdout)
        identities = {operation["operation_id"] for operation in catalogue["operations"]}
        if not {"pubmed.search", "openalex.search_works", "crossref.search_works", "zenodo.search_records",
                "glioma.evidence.reconcile", "glioma.evidence.review_handoff"}.issubset(identities):
            raise RuntimeError("installed executable workflow catalogue omits its source or composition adapters")
        template_path = temporary_root / "glioma-plan.json"
        run([str(executable), "workflow-template", "--task", "Review public glioma research metadata",
             "--out", str(template_path)], cwd=temporary_root, env=clean_env)
        template = json.loads(template_path.read_text(encoding="utf-8"))
        if len(template["steps"]) != 5 or template["outputs"] != ["review"]:
            raise RuntimeError("installed workflow template does not compose its five evidence stages")
        typed_path = temporary_root / "typed-plan.json"
        run([str(executable), "workflow-build", "--planner", "glioma-intent", "--task",
             "Review glioma evidence using PubMed only; exclude trials and TCGA.", "--out", str(typed_path)], cwd=temporary_root, env=clean_env)
        typed = json.loads(typed_path.read_text(encoding="utf-8"))
        if len(typed["steps"]) != 3 or typed["requirements"]["output_operations"] != ["glioma.evidence.review_handoff"]:
            raise RuntimeError("installed typed request compiler did not bind its exact source/review contract")
        molecular_path = temporary_root / "molecular-plan.json"
        run([str(executable), "workflow-template", "--template", "molecular", "--task", "Review public glioma molecular catalogue metadata",
             "--out", str(molecular_path)], cwd=temporary_root, env=clean_env)
        molecular = json.loads(molecular_path.read_text(encoding="utf-8"))
        if len(molecular["steps"]) != 6 or {step["operation"] for step in molecular["steps"]} != {
                "cbioportal.studies", "ncbi_gene.summary", "open_targets.associations", "gwas_catalog.associations",
                "glioma.molecular.coverage", "glioma.molecular.review_handoff"}:
            raise RuntimeError("installed molecular workflow does not compose its real reviewed source operations")

        fixed_templates = {
            "mutations": {"gdc.gene_mutation_counts", "glioma.mutation.analyze", "glioma.mutation.review_handoff"},
            "gene-references": {"ensembl.gene_references", "ncbi_gene.summary", "glioma.gene_references.analyze", "glioma.gene_references.review_handoff"},
        }
        for name, expected_operations in fixed_templates.items():
            path = temporary_root / (name + "-plan.json")
            run([str(executable), "workflow-template", "--template", name, "--task", "Review fixed public TP53 sources",
                 "--out", str(path)], cwd=temporary_root, env=clean_env)
            plan = json.loads(path.read_text(encoding="utf-8"))
            if ({step["operation"] for step in plan["steps"]} != expected_operations
                    or plan["requirements"]["schema"] != "aurora-workflow-requirements/0.2"
                    or plan["requirements"]["max_steps"] != len(expected_operations)
                    or plan["outputs"] != ["review"]):
                raise RuntimeError("installed fixed source template lost its exact retained contract")

        application_root = temporary_root / "application"
        application_root.mkdir()
        def worker_call(request: dict, flags: tuple[str, ...] = ()) -> dict:
            result = run([str(interpreter), "-m", "prism_sdk.workflow_api_worker", "--root", str(application_root), *flags],
                         cwd=temporary_root, env=clean_env, input_text=json.dumps(request))
            envelope = json.loads(result.stdout)
            if envelope.get("schema") != "aurora-workflow-api-response/0.1" or envelope.get("ok") is not True:
                raise RuntimeError("installed application worker did not emit its successful protocol envelope")
            return envelope["data"]
        for planner, flag, expected_steps in (("glioma-mutations", "--glioma-mutations", 3), ("gene-references", "--gene-references", 4)):
            fixed_plan = worker_call({"action": "plan", "planner": planner, "task": "Review fixed public TP53 sources"}, (flag,))
            if (fixed_plan["source_dispatch"] is not False or len(fixed_plan["plan"]["steps"]) != expected_steps
                    or fixed_plan["interpretation"]["provider_calls"] != 0
                    or fixed_plan["interpretation"]["question_used_as_query"] is not False):
                raise RuntimeError("installed fixed source worker planner changed its no-model scope")
        application_plan = worker_call({"action": "plan", "planner": "glioma-intent",
            "task": "Review glioma evidence using PubMed only; exclude trials and TCGA."})
        if application_plan["source_dispatch"] is not False or application_plan["plan"]["requirements"]["schema"] != "aurora-workflow-requirements/0.2":
            raise RuntimeError("installed worker failed to bind exact native value flow without source dispatch")
        application_run = worker_call({"action": "create_run", "plan_id": application_plan["plan_id"],
            "expected_plan_digest": application_plan["plan_digest"]})
        held_run = worker_call({"action": "execute", "run_id": application_run["run_id"],
            "expected_plan_digest": application_plan["plan_digest"], "approve_source_dispatch": False})
        if held_run["status"] != "approval_required" or held_run["metadata_only"] is not True or "outputs" in held_run:
            raise RuntimeError("installed worker crossed source approval or default output retention boundary")
        exported = worker_call({"action": "export", "run_id": application_run["run_id"],
                                "kind": "checkpoint", "format": "json_text"})
        if "checkpoint" in exported or "outputs" in exported or not isinstance(exported.get("json_text"), str):
            raise RuntimeError("installed worker must export one authoritative artifact string without a native duplicate")
        exported_checkpoint = json.loads(exported["json_text"])
        if exported_checkpoint.get("schema") != "aurora-executable-workflow-checkpoint/0.2" or exported_checkpoint.get("status") != "approval_required":
            raise RuntimeError("installed worker exported a wrapper or changed the retained checkpoint status")

    print(f"verified prism-sdk wheel: {module_count} modules installed; aurora-agent --help succeeded")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
