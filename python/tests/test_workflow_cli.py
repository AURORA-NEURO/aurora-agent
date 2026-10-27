"""CLI workflow contracts use injected providers and sources, never external services."""

from __future__ import annotations

import io
import json
import os
from pathlib import Path
import sys
from datetime import datetime, timezone
from unittest.mock import Mock

import pytest

from prism_sdk.cli import _parser, main
from prism_sdk.client import Client
from prism_sdk.glioma_workflow import GLIOMA_RECONCILE_OPERATION, GLIOMA_REVIEW_OPERATION
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_builder import compile_workflow_plan, load_workflow_checkpoint, save_workflow_checkpoint, validate_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk import workflow_cli


PUBLIC_VALUE = "PUBLIC_CONTENT_RETAIN_ONLY_IN_EXPLICIT_DESTINATIONS"
ERROR_DETAIL = "REMOTE_EXCEPTION_TEXT_MUST_NOT_REACH_CLI"


def _invoke(*arguments):
    output, errors = io.StringIO(), io.StringIO()
    code = main(arguments, environ={}, writer=output, error_writer=errors)
    return code, json.loads(output.getvalue()) if output.getvalue() else None, errors.getvalue()


def _fixture_catalog(monkeypatch, *, open_output=False):
    calls = []
    state = {"fail": False, "before_execute": None}
    def factory():
        catalog = WorkflowOperationCatalog()
        def execute(arguments, *, approve_source_dispatch=False, retrieved_at=None):
            if state["before_execute"] is not None:
                state["before_execute"]()
            calls.append({"arguments": dict(arguments), "approved": approve_source_dispatch, "retrieved_at": retrieved_at})
            if state["fail"]:
                raise RuntimeError(ERROR_DETAIL)
            return WorkflowOperationResult({"public_value": arguments["payload"]}, {"source": "local_transport_fixture"})
        schema = {"type": "object", "properties": {"payload": {"type": "string"}},
                  "required": ["payload"], "additionalProperties": False}
        output = {"type": "object", "properties": {"public_value": {"type": "string"}},
                  "required": ["public_value"], "additionalProperties": open_output}
        catalog.register(WorkflowOperation("fixture.retrieve", "Retrieve public test metadata", schema, output,
                                           "fixture", implementation="live_adapter"), execute)
        return catalog
    monkeypatch.setattr(workflow_cli, "builtin_workflow_operation_catalog", factory)
    monkeypatch.setattr(workflow_cli, "register_glioma_workflow_operations", lambda catalog: catalog)
    return factory, calls, state


def _proposal(*, second=False, invalid_reference=False):
    steps = [{"id": "first", "operation": "fixture.retrieve", "arguments": {"payload": PUBLIC_VALUE}, "depends_on": []}]
    if second:
        pointer = "missing" if invalid_reference else "public_value"
        steps.append({"id": "second", "operation": "fixture.retrieve", "arguments": {"payload": {"$ref": f"first#/{pointer}"}}, "depends_on": ["first"]})
    return {"name": "Public fixture retrieval", "steps": steps, "outputs": [steps[-1]["id"]]}


def _plan_file(tmp_path, factory, *, second=False, invalid_reference=False):
    target = tmp_path / "plan.json"
    plan = compile_workflow_plan(_proposal(second=second, invalid_reference=invalid_reference), factory(), task="Review public metadata")
    save_workflow_checkpoint(target, plan.to_dict())
    return target, plan


def _provider(monkeypatch, proposal):
    requests = []
    runtime = LLMRuntime()
    def handler(request):
        requests.append(request)
        return json.dumps(proposal)
    runtime.register_in_memory_provider("ollama", handler)
    monkeypatch.setattr(runtime, "register_provider", lambda config: None)
    monkeypatch.setattr(workflow_cli, "LLMRuntime", lambda: runtime)
    return requests


def _openapi_files(tmp_path):
    spec = tmp_path / "reviewed-spec.json"
    document = {"openapi": "3.0.3", "info": {"title": "Operator reviewed fixture specification", "version": "1"},
                "servers": [{"url": "https://research.example.org/api"}], "paths": {"/records": {"get": {
                    "operationId": "list_records", "summary": "List research metadata", "responses": {"200": {
                        "description": "Public metadata", "content": {"application/json": {"schema": {"type": "object"}}}}}}}}}
    spec.write_text(json.dumps(document), encoding="utf-8")
    config = tmp_path / "reviewed-config.json"
    config.write_text(json.dumps({"document": spec.name, "integration_id": "reviewed_fixture",
                                 "server_url": "https://research.example.org/api", "allowed_hosts": ["research.example.org"],
                                 "reviewed_operation_ids": ["list_records"]}), encoding="utf-8")
    return config, spec, document


def test_catalogue_lists_only_registered_implementations_without_contacting_sources(monkeypatch):
    _, calls, _ = _fixture_catalog(monkeypatch)
    code, payload, errors = _invoke("workflow-catalogue")
    assert code == 0 and errors == ""
    assert payload["operation_count"] == payload["integration_count"] == 1
    assert payload["operations"][0]["operation_id"] == "fixture.retrieve"
    assert payload["operations"][0]["implementation"] == "live_adapter"
    assert calls == []


def test_build_without_provider_approval_never_invokes_model_or_writes_a_plan(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    target = tmp_path / "denied-plan.json"
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target))
    assert code == 2 and payload is None and "command failed" in errors
    assert requests == calls == [] and not target.exists()


def test_approved_build_compiles_model_json_with_exact_operation_allowlist_without_source_dispatch(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal(second=True))
    target = tmp_path / "model-plan.json"
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--approve-provider-call", "--operation", "fixture.retrieve", "--max-steps", "2", "--out", str(target))
    assert code == 0 and errors == "" and payload["status"] == "planned" and payload["source_dispatch"] is False
    assert len(requests) == 1 and calls == []
    assert payload["step_count"] == 2 and PUBLIC_VALUE not in json.dumps(payload)
    checked = validate_workflow_plan(json.loads(target.read_text(encoding="utf-8")), factory())
    assert checked.plan_digest == payload["plan_digest"]
    instructions = json.loads(requests[0].messages[0]["content"])
    assert [operation["operation_id"] for operation in instructions["operations"]] == ["fixture.retrieve"]


def _incremental_provider(monkeypatch, decisions):
    requests, iterator = [], iter(decisions)
    runtime = LLMRuntime()
    def handler(request):
        requests.append(request)
        return json.dumps(next(iterator))
    runtime.register_in_memory_provider("ollama", handler)
    monkeypatch.setattr(workflow_cli, "_local_runtime", lambda _args: runtime)
    return requests


def test_incremental_build_retains_separate_receipt_and_compiler_accepted_plan_without_source_calls(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    requests = _incremental_provider(monkeypatch, [
        {"action": "append", "operation": "fixture.retrieve"},
        {"action": "arguments", "arguments": {"payload": PUBLIC_VALUE}, "additional_dependencies": []},
        {"action": "finish", "name": "Review public metadata", "outputs": ["s1"]}])
    plan_path, receipt_path = tmp_path / "plan.json", tmp_path / "planning.json"
    code, payload, errors = _invoke("workflow-build", "--planner", "local-incremental", "--task", "Review public metadata",
        "--model", "fixture-model", "--approve-provider-call", "--operation", "fixture.retrieve",
        "--model-call-budget", "3", "--out", str(plan_path), "--planning-receipt", str(receipt_path))
    assert code == 0 and not errors and payload["status"] == "planned" and len(requests) == 3 and not calls
    receipt = json.loads(receipt_path.read_text())
    assert receipt["status"] == "planned" and receipt["source_dispatches"] == 0
    assert validate_workflow_plan(json.loads(plan_path.read_text()), factory()).plan_digest == payload["plan_digest"]
    assert PUBLIC_VALUE not in json.dumps(payload)


@pytest.mark.parametrize("action,status", [("clarify", "clarification_required"), ("refuse", "refused")])
def test_incremental_nonplanned_state_writes_only_result_receipt_and_never_runs_sources(monkeypatch, tmp_path, action, status):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _incremental_provider(monkeypatch, [{"action": action, "message": "Need a source scope"}])
    plan, receipt = tmp_path / "plan.json", tmp_path / "receipt.json"
    code, payload, errors = _invoke("workflow-run", "--planner", "local-incremental", "--task", "Review public metadata",
        "--model", "fixture-model", "--approve-provider-call", "--approve-source-dispatch", "--out", str(plan),
        "--planning-receipt", str(receipt))
    assert code == 0 and not errors and payload["status"] == status and payload["source_dispatch"] is False
    assert payload["message"] == "Need a source scope"
    assert len(requests) == 1 and not calls and not plan.exists()
    assert "plan" not in json.loads(receipt.read_text())


def test_incremental_receipt_path_alias_is_refused_before_provider_dispatch(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _incremental_provider(monkeypatch, [])
    path = tmp_path / "plan.json"
    code, payload, errors = _invoke("workflow-build", "--planner", "local-incremental", "--task", "Review public metadata",
        "--model", "fixture-model", "--approve-provider-call", "--out", str(path), "--planning-receipt", str(path))
    assert code == 2 and payload is None and errors and requests == calls == [] and not path.exists()


def test_denominator_cli_template_and_provider_inspection_remain_model_and_source_free(monkeypatch, tmp_path):
    runtime = Mock(side_effect=AssertionError("fixed planning and inspection need no model"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", runtime)
    target = tmp_path / "denominator.json"
    code, result, errors = _invoke("workflow-template", "--template", "denominator-coverage",
        "--task", "Inspect fixed TP53 denominator evidence", "--out", str(target))
    assert code == 0 and errors == "" and target.exists()
    plan = json.loads(target.read_text(encoding="utf-8"))
    assert [step["operation"] for step in plan["steps"]] == ["gdc.gene_mutation_counts", "gdc.ssm_project_coverage",
        "glioma.denominator.assess", "glioma.denominator.review_handoff"]
    assert plan["steps"][2]["arguments"]["coverage_bundle"] == {"$ref": "coverage#"}
    code, registry, errors = _invoke("workflow-catalogue", "--provider-registry", "--denominator-coverage")
    assert code == 0 and errors == "" and registry["counts"]["implemented_reader_paths"] == 20
    assert registry["counts"]["declared_reviewed_service_groups"] == 17
    assert registry["counts"]["independently_live_verified_providers"]["value"] is None
    assert registry["network_requests_on_inspection"] == 0
    runtime.assert_not_called()


def test_fixed_protein_pathway_template_auto_enables_only_its_opt_in_readers_without_model_or_source(monkeypatch, tmp_path):
    monkeypatch.setattr(workflow_cli, "_local_runtime", Mock(side_effect=AssertionError("no model")))
    path = tmp_path / "protein.json"
    code, result, errors = _invoke("workflow-template", "--template", "protein-pathways", "--task", "Review TP53 annotation context", "--out", str(path))
    assert code == 0 and not errors and result["source_dispatch"] is False and result["step_count"] == 4
    plan = json.loads(path.read_text())
    assert {step["operation"] for step in plan["steps"]} == {
        "uniprot.protein_annotation", "reactome.pathway_annotations", "glioma.protein_pathways.analyze", "glioma.protein_pathways.review_handoff"}
    assert plan["requirements"]["schema"].endswith("/0.2")


@pytest.mark.parametrize("base_url", ["https://example.org/v1", "http://10.0.0.1/v1", "file:///tmp/model"])
def test_build_rejects_nonloopback_model_endpoints_before_model_calls(monkeypatch, tmp_path, base_url):
    _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--base-url", base_url, "--approve-provider-call", "--out", str(tmp_path / "plan.json"))
    assert code == 2 and payload is None and requests == []
    assert base_url not in errors


@pytest.mark.parametrize("no_tcga,expected_steps", [(False, 5), (True, 4)])
def test_template_compiles_complete_glioma_retrieval_reconciliation_and_review_dag_without_a_model(monkeypatch, tmp_path, no_tcga, expected_steps):
    provider = Mock(side_effect=AssertionError("template must not create a model runtime"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", provider)
    target = tmp_path / "glioma-plan.json"
    options = ["workflow-template", "--task", "Review glioma research evidence", "--literature-limit", "3", "--out", str(target)]
    if no_tcga:
        options.append("--no-tcga")
    code, payload, errors = _invoke(*options)
    assert code == 0 and errors == "" and payload["step_count"] == expected_steps
    provider.assert_not_called()
    plan = json.loads(target.read_text(encoding="utf-8"))
    operations = [step["operation"] for step in plan["steps"]]
    assert operations[-2:] == [GLIOMA_RECONCILE_OPERATION, GLIOMA_REVIEW_OPERATION]
    assert ("gdc.projects" in operations) is (not no_tcga)
    assert plan["outputs"] == ["review"] and plan["steps"][0]["arguments"]["per_specialty_limit"] == 3


def test_execution_without_dispatch_approval_persists_hold_without_contacting_sources(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    checkpoint = tmp_path / "checkpoint.json"
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint))
    assert code == 1 and errors == "" and payload["status"] == "approval_required"
    assert calls == [] and load_workflow_checkpoint(checkpoint)["status"] == "approval_required"


def test_paused_execution_resumes_completed_steps_without_repeating_sources_and_stdout_excludes_values(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory, second=True)
    checkpoint, output = tmp_path / "checkpoint.json", tmp_path / "public-output.json"
    options = ["workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch",
               "--retrieved-at", "2026-10-26T12:00:00Z"]
    code, payload, errors = _invoke(*options, "--max-step-calls", "1")
    assert code == 0 and errors == "" and payload["status"] == "paused" and len(calls) == 1
    assert PUBLIC_VALUE not in json.dumps(payload)
    assert load_workflow_checkpoint(checkpoint)["results"]["first"]["value"] == {"public_value": PUBLIC_VALUE}
    code, payload, errors = _invoke(*options, "--resume", "--outputs-file", str(output))
    assert code == 0 and errors == "" and payload["status"] == "completed" and len(calls) == 2
    assert PUBLIC_VALUE not in json.dumps(payload)
    assert json.loads(output.read_text(encoding="utf-8"))["outputs"] == {"second": {"public_value": PUBLIC_VALUE}}
    assert [call["retrieved_at"] for call in calls] == ["2026-10-26T12:00:00Z"] * 2
    code, payload, errors = _invoke(*options, "--resume")
    assert code == 0 and payload["status"] == "completed" and len(calls) == 2


def test_source_failure_requires_explicit_retry_and_error_detail_never_reaches_console(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    state["fail"] = True
    plan, _ = _plan_file(tmp_path, factory)
    checkpoint = tmp_path / "checkpoint.json"
    options = ["workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch"]
    code, payload, errors = _invoke(*options)
    assert code == 1 and errors == "" and payload["status"] == "failed" and len(calls) == 1
    assert ERROR_DETAIL not in json.dumps(payload) + errors
    assert ERROR_DETAIL not in checkpoint.read_text(encoding="utf-8")
    state["fail"] = False
    code, payload, errors = _invoke(*options, "--resume")
    assert code == 1 and payload["status"] == "retry_required" and len(calls) == 1
    code, payload, errors = _invoke(*options, "--resume", "--retry-failed")
    assert code == 0 and payload["status"] == "completed" and len(calls) == 2
    assert load_workflow_checkpoint(checkpoint)["attempts"] == {"first": 2}


def test_unresolved_open_output_dependency_blocks_before_the_dependent_source_is_called(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch, open_output=True)
    plan, _ = _plan_file(tmp_path, factory, second=True, invalid_reference=True)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch")
    assert code == 1 and errors == "" and payload["status"] == "blocked" and len(calls) == 1
    assert PUBLIC_VALUE not in json.dumps(payload)


def test_impossible_closed_output_reference_is_refused_before_any_source_call(monkeypatch):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    with pytest.raises(ValueError, match="impossible under the source output schema"):
        compile_workflow_plan(_proposal(second=True, invalid_reference=True), factory(), task="Review public metadata")
    assert calls == []


def test_attempt_limit_is_nonzero_and_never_silently_dispatches_a_fourth_retry(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    state["fail"] = True
    plan, _ = _plan_file(tmp_path, factory)
    checkpoint = tmp_path / "checkpoint.json"
    options = ["workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch"]
    assert _invoke(*options)[0] == 1
    for _ in range(2):
        code, payload, errors = _invoke(*options, "--resume", "--retry-failed")
        assert code == 1 and payload["status"] == "failed"
    code, payload, errors = _invoke(*options, "--resume", "--retry-failed")
    assert code == 1 and errors == "" and payload["status"] == "attempt_limit" and len(calls) == 3


@pytest.mark.parametrize("extra", [["--resume"], ["--retry-failed"]])
def test_resume_controls_require_their_explicit_checkpoint_context(monkeypatch, tmp_path, extra):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", *extra)
    assert code == 2 and payload is None and calls == []


def test_existing_checkpoint_requires_resume_and_remains_unchanged_on_refusal(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    checkpoint = tmp_path / "checkpoint.json"
    checkpoint.write_text("existing operator checkpoint", encoding="utf-8")
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch")
    assert code == 2 and payload is None and calls == []
    assert checkpoint.read_text(encoding="utf-8") == "existing operator checkpoint"


def test_reviewed_openapi_config_requires_approval_and_resolves_document_from_its_own_directory(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    config, spec, _ = _openapi_files(tmp_path)
    code, payload, errors = _invoke("workflow-catalogue", "--openapi-config", str(config))
    assert code == 2 and payload is None and calls == []
    code, payload, errors = _invoke("workflow-catalogue", "--openapi-config", str(config), "--approve-spec-import")
    assert code == 0 and errors == "" and payload["operation_count"] == 2
    imported = next(operation for operation in payload["operations"] if operation["operation_id"] == "reviewed_fixture.list_records")
    assert imported["implementation"] == "live_adapter" and "https://research.example.org/api" in imported["description"]
    assert calls == [] and spec.is_file()


def test_changed_openapi_document_invalidates_a_previously_reviewed_plan_before_dispatch(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    config, spec, document = _openapi_files(tmp_path)
    args = _parser().parse_args(["workflow-catalogue", "--openapi-config", str(config), "--approve-spec-import"])
    catalog = workflow_cli.workflow_cli_catalog(args)
    proposal = {"name": "Reviewed imported operation", "steps": [{"id": "retrieve", "operation": "reviewed_fixture.list_records", "arguments": {}, "depends_on": []}], "outputs": ["retrieve"]}
    plan = compile_workflow_plan(proposal, catalog, task="Review public research metadata")
    plan_file = tmp_path / "plan.json"
    save_workflow_checkpoint(plan_file, plan.to_dict())
    document["info"]["title"] = "A separately reviewed specification"
    spec.write_text(json.dumps(document), encoding="utf-8")
    transport = Mock(side_effect=AssertionError("catalogue drift must refuse before network"))
    monkeypatch.setattr("prism_sdk.workflow_openapi._http_get", transport)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan_file), "--openapi-config", str(config), "--approve-spec-import", "--approve-source-dispatch")
    assert code == 2 and payload is None
    transport.assert_not_called()


@pytest.mark.parametrize("collision", ["output_plan", "output_checkpoint", "checkpoint_plan", "aliased_output_plan", "hardlinked_output_plan"])
def test_artifact_role_collisions_refuse_before_source_dispatch_and_preserve_inputs(monkeypatch, tmp_path, collision):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    before = plan.read_bytes()
    checkpoint = tmp_path / "checkpoint.json"
    options = ["workflow-execute", "--plan", str(plan), "--approve-source-dispatch"]
    if collision == "output_plan":
        options.extend(["--outputs-file", str(plan)])
    elif collision == "output_checkpoint":
        options.extend(["--checkpoint-store", str(checkpoint), "--outputs-file", str(checkpoint)])
    elif collision == "checkpoint_plan":
        options.extend(["--checkpoint-store", str(plan)])
    elif collision == "aliased_output_plan":
        alias = plan.parent / ".." / plan.parent.name / plan.name
        options.extend(["--outputs-file", str(alias)])
    else:
        alias = tmp_path / "linked-plan.json"
        try:
            os.link(plan, alias)
        except OSError:
            pytest.skip("filesystem does not support hardlinks")
        options.extend(["--outputs-file", str(alias)])
    code, payload, errors = _invoke(*options)
    assert code == 2 and payload is None and calls == []
    assert plan.read_bytes() == before and not checkpoint.exists()


@pytest.mark.parametrize("target_role", ["config", "spec"])
def test_plan_output_cannot_replace_openapi_review_inputs_before_provider_or_source_calls(monkeypatch, tmp_path, target_role):
    _fixture_catalog(monkeypatch)
    config, spec, _ = _openapi_files(tmp_path)
    target = config if target_role == "config" else spec
    before = target.read_bytes()
    provider = Mock(side_effect=AssertionError("path preflight must happen before model setup"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", provider)
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model", "--approve-provider-call",
                                   "--openapi-config", str(config), "--approve-spec-import", "--out", str(target))
    assert code == 2 and payload is None and target.read_bytes() == before
    provider.assert_not_called()


@pytest.mark.parametrize("role", ["checkpoint", "outputs"])
@pytest.mark.parametrize("invalid", ["missing_parent", "directory"])
def test_unusable_output_destinations_fail_before_an_approved_source_call(monkeypatch, tmp_path, role, invalid):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    destination = tmp_path / "missing-parent" / "artifact.json" if invalid == "missing_parent" else tmp_path
    option = "--checkpoint-store" if role == "checkpoint" else "--outputs-file"
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", option, str(destination))
    assert code == 2 and payload is None and calls == []


def test_build_output_parent_is_checked_before_a_provider_call(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    target = tmp_path / "missing-parent" / "plan.json"
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--approve-provider-call", "--out", str(target))
    assert code == 2 and payload is None and requests == [] and not target.exists()


def test_symlink_output_target_is_rejected_before_source_dispatch(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    existing = tmp_path / "operator-file.json"
    existing.write_text("operator content", encoding="utf-8")
    alias = tmp_path / "output-link.json"
    try:
        alias.symlink_to(existing)
    except OSError:
        pytest.skip("filesystem does not permit creating symlinks")
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch", "--outputs-file", str(alias))
    assert code == 2 and payload is None and calls == []
    assert existing.read_text(encoding="utf-8") == "operator content"


def test_tampered_resume_checkpoint_is_refused_without_repeating_a_source(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory, second=True)
    checkpoint = tmp_path / "checkpoint.json"
    options = ["workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch"]
    assert _invoke(*options, "--max-step-calls", "1")[0] == 0
    retained = json.loads(checkpoint.read_text(encoding="utf-8"))
    retained["results"]["first"]["value"]["public_value"] = "a changed source result"
    checkpoint.write_text(json.dumps(retained), encoding="utf-8")
    code, payload, errors = _invoke(*options, "--resume")
    assert code == 2 and payload is None and len(calls) == 1


def test_duplicate_json_and_arbitrary_cli_exceptions_do_not_expose_input_or_error_details(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    plan = tmp_path / "invalid-plan.json"
    plan.write_text('{"name":"' + ERROR_DETAIL + '","name":"duplicate"}', encoding="utf-8")
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch")
    assert code == 2 and payload is None and ERROR_DETAIL not in errors
    def fail(args):
        raise RuntimeError(ERROR_DETAIL)
    monkeypatch.setattr(workflow_cli, "workflow_cli_catalog", fail)
    code, payload, errors = _invoke("workflow-catalogue")
    assert code == 2 and payload is None and ERROR_DETAIL not in errors and "RuntimeError" in errors


def test_default_source_clock_is_whole_second_utc_even_when_system_clock_has_microseconds(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    requested_timezones = []
    class FixedDateTime(datetime):
        @classmethod
        def now(cls, tz=None):
            requested_timezones.append(tz)
            return cls(2026, 10, 26, 12, 34, 56, 987654, tzinfo=tz)
    monkeypatch.setattr(workflow_cli, "datetime", FixedDateTime)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert requested_timezones == [timezone.utc]
    assert [call["retrieved_at"] for call in calls] == ["2026-10-26T12:34:56Z"]


@pytest.mark.parametrize("invalid_clock", [
    "", "not-a-clock", "2026-10-26T12:00:00.123456Z", "2026-10-26T12:00:00+00:00",
    "2026-10-26T12:00:00", "2026-02-30T12:00:00Z", "2026-1-2T03:04:05Z", "2026-10-26T25:00:00Z",
])
@pytest.mark.parametrize("resume", [False, True])
def test_invalid_or_fractional_source_clock_refuses_before_dispatch_and_never_mutates_checkpoint(monkeypatch, tmp_path, invalid_clock, resume):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory, second=True)
    checkpoint = tmp_path / "checkpoint.json"
    options = ["workflow-execute", "--plan", str(plan), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch"]
    if resume:
        code, payload, errors = _invoke(*options, "--retrieved-at", "2026-10-26T12:00:00Z", "--max-step-calls", "1")
        assert code == 0 and payload["status"] == "paused" and len(calls) == 1
        before = checkpoint.read_bytes()
        options.append("--resume")
    else:
        before = None
    previous_calls = len(calls)
    code, payload, errors = _invoke(*options, "--retrieved-at", invalid_clock)
    assert code == 2 and payload is None and len(calls) == previous_calls
    if before is None:
        assert not checkpoint.exists()
    else:
        assert checkpoint.read_bytes() == before


def test_one_shot_run_saves_validated_plan_before_first_source_dispatch_and_keeps_stdout_metadata_only(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal(second=True))
    target, checkpoint, outputs = tmp_path / "one-shot-plan.json", tmp_path / "one-shot-checkpoint.json", tmp_path / "one-shot-outputs.json"
    persisted_plans = []
    def inspect_plan_before_source():
        checked = validate_workflow_plan(json.loads(target.read_text(encoding="utf-8")), factory())
        retained = load_workflow_checkpoint(checkpoint)
        assert retained["plan_digest"] == checked.plan_digest and retained["status"] == "running"
        persisted_plans.append(checked.plan_digest)
    state["before_execute"] = inspect_plan_before_source
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--operation", "fixture.retrieve", "--out", str(target), "--checkpoint-store", str(checkpoint),
                                   "--outputs-file", str(outputs), "--approve-provider-call", "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert payload["command"] == "workflow-run" and payload["plan_file"] == str(target.resolve())
    assert len(requests) == 1 and len(calls) == 2 and persisted_plans == [payload["plan_digest"]] * 2
    assert PUBLIC_VALUE not in json.dumps(payload)
    assert json.loads(outputs.read_text(encoding="utf-8"))["outputs"] == {"second": {"public_value": PUBLIC_VALUE}}


@pytest.mark.parametrize("source_approval", [False, True])
def test_one_shot_provider_approval_cannot_be_replaced_by_source_approval(monkeypatch, tmp_path, source_approval):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    target = tmp_path / "one-shot-plan.json"
    options = ["workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target)]
    if source_approval:
        options.append("--approve-source-dispatch")
    code, payload, errors = _invoke(*options)
    assert code == 2 and payload is None and requests == calls == [] and not target.exists()


def test_one_shot_provider_approval_without_source_approval_builds_plan_and_persists_dispatch_hold(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    target, checkpoint = tmp_path / "one-shot-plan.json", tmp_path / "one-shot-checkpoint.json"
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(target), "--checkpoint-store", str(checkpoint), "--approve-provider-call")
    assert code == 1 and errors == "" and payload["status"] == "approval_required"
    assert len(requests) == 1 and calls == []
    assert validate_workflow_plan(json.loads(target.read_text(encoding="utf-8")), factory()).plan_digest == payload["plan_digest"]
    assert load_workflow_checkpoint(checkpoint)["status"] == "approval_required"


def test_one_shot_checkpoint_resumes_through_execute_without_replanning_or_repeating_sources(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal(second=True))
    target, checkpoint, outputs = tmp_path / "one-shot-plan.json", tmp_path / "one-shot-checkpoint.json", tmp_path / "outputs.json"
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(target), "--checkpoint-store", str(checkpoint), "--max-step-calls", "1",
                                   "--approve-provider-call", "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "paused" and len(calls) == len(requests) == 1
    reviewed_plan = target.read_bytes()
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--checkpoint-store", str(checkpoint),
                                   "--resume", "--outputs-file", str(outputs), "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert len(calls) == 2 and len(requests) == 1 and target.read_bytes() == reviewed_plan
    assert json.loads(outputs.read_text(encoding="utf-8"))["outputs"] == {"second": {"public_value": PUBLIC_VALUE}}


def test_one_shot_source_failure_keeps_reviewed_plan_and_failed_checkpoint_for_explicit_retry(monkeypatch, tmp_path):
    factory, calls, state = _fixture_catalog(monkeypatch)
    state["fail"] = True
    requests = _provider(monkeypatch, _proposal())
    target, checkpoint = tmp_path / "one-shot-plan.json", tmp_path / "one-shot-checkpoint.json"
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(target), "--checkpoint-store", str(checkpoint),
                                   "--approve-provider-call", "--approve-source-dispatch")
    assert code == 1 and errors == "" and payload["status"] == "failed" and len(calls) == len(requests) == 1
    assert validate_workflow_plan(json.loads(target.read_text(encoding="utf-8")), factory()).plan_digest == payload["plan_digest"]
    assert load_workflow_checkpoint(checkpoint)["status"] == "failed"
    assert ERROR_DETAIL not in json.dumps(payload) + checkpoint.read_text(encoding="utf-8")


def test_one_shot_plan_persistence_failure_prevents_source_dispatch(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    def fail_save(path, value):
        raise OSError(ERROR_DETAIL)
    monkeypatch.setattr(workflow_cli, "save_workflow_checkpoint", fail_save)
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(tmp_path / "plan.json"), "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and len(requests) == 1 and calls == [] and ERROR_DETAIL not in errors


@pytest.mark.parametrize("collision", ["plan_checkpoint", "plan_outputs", "checkpoint_outputs", "plan_config", "plan_spec", "outputs_config"])
def test_one_shot_artifact_collisions_fail_before_provider_or_source_calls(monkeypatch, tmp_path, collision):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    config, spec, _ = _openapi_files(tmp_path)
    before_config, before_spec = config.read_bytes(), spec.read_bytes()
    target, checkpoint, outputs = tmp_path / "one-shot-plan.json", tmp_path / "checkpoint.json", tmp_path / "outputs.json"
    if collision == "plan_checkpoint":
        checkpoint = target
    elif collision == "plan_outputs":
        outputs = target
    elif collision == "checkpoint_outputs":
        outputs = checkpoint
    elif collision == "plan_config":
        target = config
    elif collision == "plan_spec":
        target = spec
    else:
        outputs = config
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(target), "--checkpoint-store", str(checkpoint), "--outputs-file", str(outputs),
                                   "--openapi-config", str(config), "--approve-spec-import", "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and requests == calls == []
    assert config.read_bytes() == before_config and spec.read_bytes() == before_spec


def test_one_shot_existing_checkpoint_is_not_overwritten_or_used_to_trigger_replanning(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    checkpoint = tmp_path / "operator-checkpoint.json"
    checkpoint.write_text("operator checkpoint", encoding="utf-8")
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(tmp_path / "plan.json"), "--checkpoint-store", str(checkpoint),
                                   "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and requests == calls == []
    assert checkpoint.read_text(encoding="utf-8") == "operator checkpoint"


@pytest.mark.parametrize("flag", ["--resume", "--retry-failed"])
def test_one_shot_command_rejects_resume_controls_that_belong_to_execute(monkeypatch, tmp_path, flag):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(tmp_path / "plan.json"), "--approve-provider-call", "--approve-source-dispatch", flag)
    assert code == 2 and payload is None and requests == calls == []


def test_one_shot_invalid_clock_is_rejected_before_authorized_model_work(monkeypatch, tmp_path):
    _, calls, _ = _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model",
                                   "--out", str(tmp_path / "plan.json"), "--retrieved-at", "2026-10-26T12:00:00.123Z",
                                   "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and requests == calls == []


def _mcp_file(tmp_path, *, integration_id="external_fixture", file_name="mcp-config.json"):
    target = tmp_path / file_name
    target.write_text(json.dumps({"integration_id": integration_id,
                                  "command": [sys.executable, "-u", str(Path(__file__).parent / "fake_mcp_server.py")],
                                  "cwd": ".", "reviewed_read_only_tools": ["echo"]}), encoding="utf-8")
    return target


def _tracking_clients(monkeypatch, *, fail_call=False):
    instances, discoveries, calls = [], [], []
    class TrackingClient(Client):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            instances.append(self)
        def request(self, method, params=None):
            discoveries.append(method)
            return super().request(method, params)
        def call_tool(self, name, arguments=None):
            calls.append((name, dict(arguments or {})))
            if fail_call:
                raise RuntimeError(ERROR_DETAIL)
            return super().call_tool(name, arguments)
    monkeypatch.setattr(workflow_cli, "Client", TrackingClient)
    return instances, discoveries, calls


def _mcp_proposal():
    return {"name": "Reviewed external metadata", "steps": [{"id": "external", "operation": "external_fixture.echo",
             "arguments": {"value": 3}, "depends_on": []}], "outputs": ["external"]}


def test_research_sources_are_explicit_opt_in_without_changing_the_default_catalogue(monkeypatch):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    code, default, errors = _invoke("workflow-catalogue")
    assert code == 0 and default["catalog_digest"] == factory().catalog_digest
    code, expanded, errors = _invoke("workflow-catalogue", "--research-sources")
    assert code == 0 and errors == "" and expanded["operation_count"] == default["operation_count"] + 3
    identifiers = {operation["operation_id"] for operation in expanded["operations"]}
    assert {"openalex.search_works", "crossref.search_works", "zenodo.search_records"} <= identifiers
    assert calls == [] and expanded["catalog_digest"] != default["catalog_digest"]
    assert _invoke("workflow-catalogue")[1]["catalog_digest"] == default["catalog_digest"]


def test_cli_mcp_catalogue_starts_two_reviewed_stdio_peers_and_closes_both_without_tool_calls(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    first = _mcp_file(tmp_path)
    second = _mcp_file(tmp_path, integration_id="second_fixture", file_name="second-config.json")
    instances, discoveries, calls = _tracking_clients(monkeypatch)
    code, payload, errors = _invoke("workflow-catalogue", "--mcp-config", str(first), "--mcp-config", str(second), "--approve-mcp-discovery")
    assert code == 0 and errors == "" and payload["operation_count"] == 3
    assert {"external_fixture.echo", "second_fixture.echo"} <= {operation["operation_id"] for operation in payload["operations"]}
    assert discoveries == ["tools/list", "tools/list"] and calls == []
    assert len(instances) == 2 and all(not client.running and client.session is None for client in instances)
    assert all(client.config.cwd == str(tmp_path.resolve()) for client in instances)
    assert str(Path(__file__).parent / "fake_mcp_server.py") not in json.dumps(payload)


def test_cli_mcp_startup_and_discovery_require_a_separate_literal_flag_before_client_creation(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    constructor = Mock(side_effect=AssertionError("unapproved client startup"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    code, payload, errors = _invoke("workflow-catalogue", "--mcp-config", str(config))
    assert code == 2 and payload is None
    constructor.assert_not_called()


@pytest.mark.parametrize("command", ["workflow-build", "workflow-execute", "workflow-template"])
def test_session_bound_mcp_configs_are_refused_for_cross_process_plan_commands(monkeypatch, tmp_path, command):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    constructor = Mock(side_effect=AssertionError("unsupported command must not start MCP"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    options = [command, "--mcp-config", str(config), "--approve-mcp-discovery"]
    if command == "workflow-execute":
        options.extend(["--plan", str(tmp_path / "plan.json")])
    else:
        options.extend(["--task", "Review public metadata", "--out", str(tmp_path / "plan.json")])
        if command == "workflow-build":
            options.extend(["--model", "fixture-model"])
    code, payload, errors = _invoke(*options)
    assert code == 2 and payload is None
    constructor.assert_not_called()


@pytest.mark.parametrize("edit", [
    lambda value: value.update({"env": {"TOKEN": "must-not-be-accepted"}}),
    lambda value: value.update({"command": "a shell command string"}),
    lambda value: value.update({"command": []}),
    lambda value: value.update({"command": ["x"] * 65}),
    lambda value: value.update({"command": ["x" * 8193]}),
    lambda value: value.update({"command": ["x" * 8192] * 5}),
    lambda value: value.update({"command": ["invalid\x00argument"]}),
    lambda value: value.update({"cwd": "nonexistent-cwd"}),
    lambda value: value.update({"reviewed_read_only_tools": []}),
    lambda value: value.update({"reviewed_read_only_tools": ["echo", "echo"]}),
])
def test_every_mcp_config_is_validated_before_any_subprocess_is_created(monkeypatch, tmp_path, edit):
    _fixture_catalog(monkeypatch)
    first = _mcp_file(tmp_path)
    second = _mcp_file(tmp_path, integration_id="second_fixture", file_name="invalid-config.json")
    value = json.loads(second.read_text(encoding="utf-8"))
    edit(value)
    second.write_text(json.dumps(value), encoding="utf-8")
    constructor = Mock(side_effect=AssertionError("no client may start before all configs validate"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    code, payload, errors = _invoke("workflow-catalogue", "--mcp-config", str(first), "--mcp-config", str(second), "--approve-mcp-discovery")
    assert code == 2 and payload is None
    constructor.assert_not_called()


def test_cli_mcp_run_builds_and_executes_in_one_session_then_closes_it(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    requests = _provider(monkeypatch, _mcp_proposal())
    instances, discoveries, calls = _tracking_clients(monkeypatch)
    target, checkpoint, outputs = tmp_path / "plan.json", tmp_path / "checkpoint.json", tmp_path / "outputs.json"
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target),
                                   "--checkpoint-store", str(checkpoint), "--outputs-file", str(outputs), "--mcp-config", str(config),
                                   "--approve-mcp-discovery", "--approve-provider-call", "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert len(requests) == len(instances) == 1 and discoveries == ["tools/list", "tools/list"]
    assert calls == [("echo", {"value": 3})] and not instances[0].running
    assert json.loads(outputs.read_text(encoding="utf-8"))["outputs"] == {"external": {"ok": True, "echo": {"value": 3}}}


def test_cli_mcp_discovery_and_provider_grants_do_not_authorize_a_tool_call(monkeypatch, tmp_path):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    requests = _provider(monkeypatch, _mcp_proposal())
    instances, discoveries, calls = _tracking_clients(monkeypatch)
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(tmp_path / "plan.json"),
                                   "--mcp-config", str(config), "--approve-mcp-discovery", "--approve-provider-call")
    assert code == 1 and errors == "" and payload["status"] == "approval_required"
    assert len(requests) == 1 and discoveries == ["tools/list"] and calls == [] and not instances[0].running


@pytest.mark.parametrize("failure", ["registration", "provider", "source", "second_startup"])
def test_cli_mcp_clients_close_after_registration_provider_source_or_subsequent_startup_failure(monkeypatch, tmp_path, failure):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    instances, discoveries, calls = _tracking_clients(monkeypatch, fail_call=failure == "source")
    requests = _provider(monkeypatch, _mcp_proposal())
    options = ["workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(tmp_path / "plan.json"),
               "--mcp-config", str(config), "--approve-mcp-discovery", "--approve-provider-call", "--approve-source-dispatch"]
    if failure == "registration":
        value = json.loads(config.read_text(encoding="utf-8"))
        value["reviewed_read_only_tools"] = ["unavailable"]
        config.write_text(json.dumps(value), encoding="utf-8")
    elif failure == "provider":
        def fail(*args, **kwargs):
            raise RuntimeError(ERROR_DETAIL)
        monkeypatch.setattr(workflow_cli, "LocalWorkflowBuilder", fail)
    elif failure == "second_startup":
        second = _mcp_file(tmp_path, integration_id="second_fixture", file_name="second-config.json")
        value = json.loads(second.read_text(encoding="utf-8"))
        value["command"] = [sys.executable, "-c", "raise SystemExit(3)"]
        second.write_text(json.dumps(value), encoding="utf-8")
        options.extend(["--mcp-config", str(second)])
    code, payload, errors = _invoke(*options)
    assert code in {1, 2} and ERROR_DETAIL not in errors + json.dumps(payload)
    assert instances and all(not client.running and client.session is None for client in instances)
    if failure in {"registration", "provider", "second_startup"}:
        assert calls == [] and requests == []
    else:
        assert code == 1 and payload["status"] == "failed" and len(calls) == len(requests) == 1


@pytest.mark.parametrize("role", ["plan", "checkpoint", "outputs"])
def test_mcp_config_cannot_be_overwritten_by_any_run_artifact_before_startup(monkeypatch, tmp_path, role):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    before = config.read_bytes()
    target, checkpoint, outputs = tmp_path / "plan.json", tmp_path / "checkpoint.json", tmp_path / "outputs.json"
    if role == "plan":
        target = config
    elif role == "checkpoint":
        checkpoint = config
    else:
        outputs = config
    constructor = Mock(side_effect=AssertionError("config collision must predate startup"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target),
                                   "--checkpoint-store", str(checkpoint), "--outputs-file", str(outputs), "--mcp-config", str(config),
                                   "--approve-mcp-discovery", "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and config.read_bytes() == before
    constructor.assert_not_called()


def test_requirements_file_and_planning_call_budget_are_forwarded_to_the_builder(monkeypatch, tmp_path):
    from prism_sdk.workflow_requirements import compile_workflow_requirements
    factory, calls, _ = _fixture_catalog(monkeypatch)
    requirements = compile_workflow_requirements("Review public metadata", required_operations=["fixture.retrieve"], output_operations=["fixture.retrieve"])
    requirements_file = tmp_path / "requirements.json"
    requirements_file.write_text(json.dumps(requirements.to_dict()), encoding="utf-8")
    received = []
    class Builder:
        def __init__(self, catalog, runtime, *, model):
            self.catalog = catalog
        def build(self, task, **options):
            received.append((task, options))
            return compile_workflow_plan(_proposal(), self.catalog, task=task)
    monkeypatch.setattr(workflow_cli, "LocalWorkflowBuilder", Builder)
    _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(tmp_path / "plan.json"),
                                   "--requirements", str(requirements_file), "--planning-attempts", "3", "--approve-provider-call")
    assert code == 0 and errors == "" and calls == [] and len(received) == 1
    assert received[0][1]["requirements"] == requirements.to_dict() and received[0][1]["max_planning_attempts"] == 3


@pytest.mark.parametrize("attempts", ["0", "5", "-1"])
def test_planning_attempt_budget_outside_one_to_four_is_rejected_before_provider_calls(monkeypatch, tmp_path, attempts):
    _fixture_catalog(monkeypatch)
    requests = _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-build", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(tmp_path / "plan.json"),
                                   "--planning-attempts", attempts, "--approve-provider-call")
    assert code == 2 and payload is None and requests == []


def test_requirement_task_mismatch_refuses_before_mcp_startup_and_provider_calls(monkeypatch, tmp_path):
    from prism_sdk.workflow_requirements import compile_workflow_requirements
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    requirements = compile_workflow_requirements("A different task", required_operations=["external_fixture.echo"])
    requirements_file = tmp_path / "requirements.json"
    requirements_file.write_text(json.dumps(requirements.to_dict()), encoding="utf-8")
    constructor = Mock(side_effect=AssertionError("task mismatch must predate MCP startup"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    requests = _provider(monkeypatch, _mcp_proposal())
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(tmp_path / "plan.json"),
                                   "--requirements", str(requirements_file), "--mcp-config", str(config), "--approve-mcp-discovery", "--approve-provider-call")
    assert code == 2 and payload is None and requests == []
    constructor.assert_not_called()


@pytest.mark.parametrize("role", ["plan", "checkpoint", "outputs"])
def test_requirement_input_is_protected_from_all_one_shot_output_roles(monkeypatch, tmp_path, role):
    from prism_sdk.workflow_requirements import compile_workflow_requirements
    _fixture_catalog(monkeypatch)
    requirements_file = tmp_path / "requirements.json"
    requirements_file.write_text(json.dumps(compile_workflow_requirements("Review public metadata").to_dict()), encoding="utf-8")
    before = requirements_file.read_bytes()
    target, checkpoint, outputs = tmp_path / "plan.json", tmp_path / "checkpoint.json", tmp_path / "outputs.json"
    if role == "plan":
        target = requirements_file
    elif role == "checkpoint":
        checkpoint = requirements_file
    else:
        outputs = requirements_file
    requests = _provider(monkeypatch, _proposal())
    code, payload, errors = _invoke("workflow-run", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target),
                                   "--checkpoint-store", str(checkpoint), "--outputs-file", str(outputs), "--requirements", str(requirements_file),
                                   "--approve-provider-call", "--approve-source-dispatch")
    assert code == 2 and payload is None and requests == [] and requirements_file.read_bytes() == before


def test_saved_default_plan_keeps_its_old_catalogue_and_research_opt_in_drift_refuses_before_dispatch(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--research-sources", "--approve-source-dispatch")
    assert code == 2 and payload is None and calls == []
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch")
    assert code == 0 and errors == "" and payload["status"] == "completed" and len(calls) == 1


@pytest.mark.parametrize("case", ["count", "duplicate_integration", "existing_operation"])
def test_mcp_config_count_and_namespace_conflicts_refuse_before_any_process(monkeypatch, tmp_path, case):
    _fixture_catalog(monkeypatch)
    config = _mcp_file(tmp_path)
    options = ["workflow-catalogue", "--approve-mcp-discovery"]
    if case == "count":
        for _ in range(9):
            options.extend(["--mcp-config", str(config)])
    elif case == "duplicate_integration":
        second = _mcp_file(tmp_path, file_name="duplicate.json")
        options.extend(["--mcp-config", str(config), "--mcp-config", str(second)])
    else:
        value = json.loads(config.read_text(encoding="utf-8"))
        value.update({"integration_id": "fixture", "reviewed_read_only_tools": ["retrieve"]})
        config.write_text(json.dumps(value), encoding="utf-8")
        options.extend(["--mcp-config", str(config)])
    constructor = Mock(side_effect=AssertionError("MCP count/namespace validation must precede startup"))
    monkeypatch.setattr(workflow_cli, "Client", constructor)
    code, payload, errors = _invoke(*options)
    assert code == 2 and payload is None
    constructor.assert_not_called()


def test_cli_requirements_are_bound_into_an_actual_model_plan_and_unsatisfied_arguments_are_refused(monkeypatch, tmp_path):
    from prism_sdk.workflow_requirements import compile_workflow_requirements
    _fixture_catalog(monkeypatch)
    requirements = compile_workflow_requirements("Review public metadata", output_operations=["fixture.retrieve"],
        argument_rules=[{"operation": "fixture.retrieve", "path": "/payload", "comparison": "equals", "value": PUBLIC_VALUE}])
    requirements_file = tmp_path / "requirements.json"
    requirements_file.write_text(json.dumps(requirements.to_dict()), encoding="utf-8")
    target = tmp_path / "plan.json"
    requests = _provider(monkeypatch, _proposal())
    options = ["workflow-build", "--task", "Review public metadata", "--model", "fixture-model", "--out", str(target),
               "--requirements", str(requirements_file), "--approve-provider-call"]
    code, payload, errors = _invoke(*options)
    assert code == 0 and errors == "" and len(requests) == 1
    assert json.loads(target.read_text(encoding="utf-8"))["requirements"] == requirements.to_dict()
    target.unlink()
    wrong = _proposal()
    wrong["steps"][0]["arguments"]["payload"] = "a different payload"
    requests = _provider(monkeypatch, wrong)
    code, payload, errors = _invoke(*options)
    assert code == 2 and payload is None and len(requests) == 1 and not target.exists()


def _grounded_fixture(monkeypatch, *, transform_draft=None):
    from test_glioma_workflow_synthesis import _draft, _reconciliation
    from prism_sdk.workflow_integrations import builtin_workflow_operation_catalog
    _, source_inputs = _reconciliation(return_sources=True)
    inputs = {source["operation_id"]: source["bundle"] for source in source_inputs}
    descriptors = {operation.operation_id: operation for operation in builtin_workflow_operation_catalog().operations()}
    source_calls, provider_calls = [], []
    def factory():
        catalog = WorkflowOperationCatalog()
        for operation_id, bundle in inputs.items():
            def execute(arguments, *, approve_source_dispatch, retrieved_at, operation_id=operation_id, bundle=bundle):
                source_calls.append(operation_id)
                return WorkflowOperationResult(bundle, {"operation_id": operation_id, "transport": "local_fixture"})
            catalog.register(descriptors[operation_id], execute)
        return catalog
    runtime = LLMRuntime()
    def handler(request):
        provider_calls.append(request)
        context = json.loads(request.messages[-1]["content"])
        draft = _draft(context)
        if transform_draft is not None:
            transform_draft(draft)
        return json.dumps(draft)
    runtime.register_in_memory_provider("ollama", handler)
    monkeypatch.setattr(runtime, "register_provider", lambda config: None)
    monkeypatch.setattr(workflow_cli, "LLMRuntime", lambda: runtime)
    monkeypatch.setattr(workflow_cli, "builtin_workflow_operation_catalog", factory)
    return source_calls, provider_calls


@pytest.mark.parametrize("without_tcga,expected_steps", [(False, 6), (True, 5)])
def test_grounded_template_registers_model_bound_operations_without_invoking_any_model(monkeypatch, tmp_path, without_tcga, expected_steps):
    from prism_sdk.glioma_workflow_synthesis import GLIOMA_AUDIT_OPERATION, GLIOMA_SYNTHESIZE_OPERATION
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target = tmp_path / "grounded-plan.json"
    options = ["workflow-template", "--task", "Review public glioma research evidence gaps", "--grounded-glioma", "--model", "fixture-model", "--out", str(target)]
    if without_tcga:
        options.append("--no-tcga")
    code, payload, errors = _invoke(*options)
    assert code == 0 and errors == "" and payload["step_count"] == expected_steps
    assert source_calls == provider_calls == []
    plan = json.loads(target.read_text(encoding="utf-8"))
    assert [step["operation"] for step in plan["steps"]][-2:] == [GLIOMA_SYNTHESIZE_OPERATION, GLIOMA_AUDIT_OPERATION]
    assert plan["outputs"] == ["audit"]


def test_grounded_template_can_be_reviewed_without_provider_grant_then_executed_with_independent_provider_grant(monkeypatch, tmp_path):
    from prism_sdk.glioma_workflow_synthesis import GLIOMA_DRAFT_AUDIT_SCHEMA
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target, outputs = tmp_path / "grounded-plan.json", tmp_path / "grounded-output.json"
    code, payload, errors = _invoke("workflow-template", "--task", "Review public glioma research evidence gaps", "--grounded-glioma",
                                   "--model", "fixture-model", "--out", str(target))
    assert code == 0 and source_calls == provider_calls == []
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--grounded-glioma", "--model", "fixture-model",
                                   "--approve-source-dispatch", "--approve-provider-call", "--outputs-file", str(outputs))
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert len(source_calls) == 3 and len(provider_calls) == 1
    audit = json.loads(outputs.read_text(encoding="utf-8"))["outputs"]["audit"]
    assert audit["status"] == "held_for_scientific_human_review" and audit["scientific_support_established"] is False
    assert audit["human_review_required"] is True and audit["clinical_actions_authorized"] is False
    assert "Unverified text" not in json.dumps(payload)
    assert audit["unverified_model_answer"] not in json.dumps(payload)
    assert payload["output_review_statuses"] == {"audit": {
        "schema": GLIOMA_DRAFT_AUDIT_SCHEMA, "status": "held_for_scientific_human_review",
        "human_review_required": True, "scientific_support_established": False, "clinical_actions_authorized": False,
    }}


def test_grounded_source_approval_never_enables_a_local_synthesis_provider_call(monkeypatch, tmp_path):
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target = tmp_path / "grounded-plan.json"
    assert _invoke("workflow-template", "--task", "Review public glioma research evidence gaps", "--grounded-glioma",
                   "--model", "fixture-model", "--out", str(target))[0] == 0
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--grounded-glioma", "--model", "fixture-model", "--approve-source-dispatch")
    assert code == 1 and errors == "" and payload["status"] == "failed"
    assert payload["failure"]["step_id"] == "synthesis" and len(source_calls) == 3 and provider_calls == []
    assert payload["output_review_statuses"] == {}


def test_grounded_model_binding_change_refuses_before_any_source_or_provider_call(monkeypatch, tmp_path):
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target = tmp_path / "grounded-plan.json"
    assert _invoke("workflow-template", "--task", "Review public glioma research evidence gaps", "--grounded-glioma",
                   "--model", "fixture-model", "--out", str(target))[0] == 0
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--grounded-glioma", "--model", "changed-model",
                                   "--approve-source-dispatch", "--approve-provider-call")
    assert code == 2 and payload is None and source_calls == provider_calls == []


def test_grounded_flag_requires_model_identity_before_runtime_construction(monkeypatch):
    constructor = Mock(side_effect=AssertionError("missing grounded model must predate runtime setup"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", constructor)
    code, payload, errors = _invoke("workflow-catalogue", "--grounded-glioma")
    assert code == 2 and payload is None
    constructor.assert_not_called()


def test_completed_grounded_graph_reports_blocked_audit_metadata_without_printing_model_text(monkeypatch, tmp_path):
    from prism_sdk.glioma_workflow_synthesis import GLIOMA_DRAFT_AUDIT_SCHEMA
    def unsupported_assertion(draft):
        draft["answer"] = PUBLIC_VALUE
        draft["claims"][0]["text"] = PUBLIC_VALUE
        draft["claims"][0]["assertions"][0]["value"] = "A value absent from the cited metadata"
    source_calls, provider_calls = _grounded_fixture(monkeypatch, transform_draft=unsupported_assertion)
    target = tmp_path / "grounded-plan.json"
    assert _invoke("workflow-template", "--task", "Review public glioma research evidence gaps", "--grounded-glioma",
                   "--model", "fixture-model", "--out", str(target))[0] == 0
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--grounded-glioma", "--model", "fixture-model",
                                   "--approve-source-dispatch", "--approve-provider-call")
    assert code == 0 and errors == "" and payload["status"] == "completed"
    assert len(source_calls) == 3 and len(provider_calls) == 1 and PUBLIC_VALUE not in json.dumps(payload)
    assert payload["output_review_statuses"] == {"audit": {
        "schema": GLIOMA_DRAFT_AUDIT_SCHEMA, "status": "blocked", "human_review_required": True,
        "scientific_support_established": False, "clinical_actions_authorized": False,
    }}


def test_source_only_workflow_has_no_invented_review_status_or_stdout_values(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    target, _ = _plan_file(tmp_path, factory)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--approve-source-dispatch")
    assert code == 0 and errors == "" and len(calls) == 1
    assert payload["output_review_statuses"] == {} and PUBLIC_VALUE not in json.dumps(payload)


def test_template_forwards_operator_trial_page_bounds_without_source_or_provider_calls(monkeypatch, tmp_path):
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target = tmp_path / "bounded-template.json"
    code, payload, errors = _invoke("workflow-template", "--task", "Review public glioma metadata", "--out", str(target),
                                   "--trial-page-size", "7", "--trial-max-pages", "1")
    assert code == 0 and errors == "" and source_calls == provider_calls == []
    plan = json.loads(target.read_text(encoding="utf-8"))
    trials = next(step for step in plan["steps"] if step["operation"] == "clinical_trials.search")
    assert trials["arguments"]["page_size"] == 7 and trials["arguments"]["max_pages"] == 1


@pytest.mark.parametrize("option,value", [("--trial-page-size", "0"), ("--trial-page-size", "101"), ("--trial-max-pages", "0"), ("--trial-max-pages", "6")])
def test_invalid_template_trial_bounds_fail_without_source_or_provider_calls(monkeypatch, tmp_path, option, value):
    source_calls, provider_calls = _grounded_fixture(monkeypatch)
    target = tmp_path / "invalid-template.json"
    code, payload, errors = _invoke("workflow-template", "--task", "Review public glioma metadata", "--out", str(target), option, value)
    assert code == 2 and payload is None and source_calls == provider_calls == [] and not target.exists()


_FIXED_TP53_OPERATIONS = {
    "mutations": {"gdc.gene_mutation_counts", "glioma.mutation.analyze", "glioma.mutation.review_handoff"},
    "gene-references": {"ensembl.gene_references", "ncbi_gene.summary", "glioma.gene_references.analyze", "glioma.gene_references.review_handoff"},
}
_FIXED_TP53_FLAGS = {"mutations": "--glioma-mutations", "gene-references": "--gene-references"}


def _assert_fixed_tp53_plan(plan, kind):
    """Assert the requested fixed graph's source boundaries and retained recovery contract."""
    count = 3 if kind == "mutations" else 4
    assert plan["schema"] == "aurora-executable-workflow-plan/0.2"
    assert len(plan["steps"]) == count and plan["outputs"] == ["review"]
    steps = {row["id"]: row for row in plan["steps"]}
    operations = {row["operation"] for row in plan["steps"]}
    assert operations == _FIXED_TP53_OPERATIONS[kind]
    contract = plan["requirements"]
    assert contract["schema"] == "aurora-workflow-requirements/0.2"
    assert contract["max_steps"] == count
    assert set(contract["required_operations"]) == operations
    assert contract["output_operations"] == [steps["review"]["operation"]]
    assert contract["max_calls_by_operation"] == {operation: 1 for operation in operations}
    links = {(row["source_operation"], row["target_operation"], row["source_pointer"], row["target_path"])
             for row in contract["data_links"]}
    expected = {(steps["analysis"]["operation"], steps["review"]["operation"], "", "/analysis")}
    if kind == "mutations":
        assert steps["counts"]["arguments"] == {"timeout_ms": 30_000}
        assert steps["analysis"]["arguments"]["bundle"] == {"$ref": "counts#"}
        expected.add(("gdc.gene_mutation_counts", steps["analysis"]["operation"], "", "/bundle"))
        assert any(rule["operation"] == "gdc.gene_mutation_counts" and rule["path"] == "/timeout_ms"
                   and rule["comparison"] == "equals" and rule["value"] == 30000 for rule in contract["argument_rules"])
    else:
        assert steps["ensembl"]["arguments"] == {"timeout_ms": 30_000}
        assert steps["ncbi"]["arguments"] == {"gene_symbols": ["TP53"], "timeout_ms": 30_000}
        for producer, target in (("ensembl", "/ensembl_bundle"), ("ncbi", "/ncbi_bundle")):
            assert steps["analysis"]["arguments"][target[1:]] == {"$ref": producer + "#"}
            expected.add((steps[producer]["operation"], steps["analysis"]["operation"], "", target))
            assert any(rule["operation"] == steps[producer]["operation"] and rule["path"] == ""
                       and rule["comparison"] == "equals" and rule["value"] == steps[producer]["arguments"]
                       for rule in contract["argument_rules"])
    assert links == expected
    assert steps["review"]["arguments"] == {"analysis": {"$ref": "analysis#"}}


def test_fixed_tp53_cli_opt_ins_extend_only_the_selected_operations_and_leave_default_catalogue_stable(monkeypatch):
    runtime = Mock(side_effect=AssertionError("catalogue must not construct a model"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", runtime)
    base_code, base, errors = _invoke("workflow-catalogue")
    assert base_code == 0 and errors == "" and base["operation_count"] == 10
    default_ids = {row["operation_id"] for row in base["operations"]}
    assert default_ids == {"pubmed.search", "europe_pmc.search", "clinical_trials.search", "gdc.projects", "cbioportal.studies",
                           "ncbi_gene.summary", "open_targets.associations", "gwas_catalog.associations", GLIOMA_RECONCILE_OPERATION, GLIOMA_REVIEW_OPERATION}
    for kind, flag in _FIXED_TP53_FLAGS.items():
        code, selected, errors = _invoke("workflow-catalogue", flag)
        assert code == 0 and errors == ""
        assert {row["operation_id"] for row in selected["operations"]} == default_ids | _FIXED_TP53_OPERATIONS[kind]
        repeated = _invoke("workflow-catalogue")[1]
        assert repeated["catalog_digest"] == base["catalog_digest"] and repeated["operation_count"] == 10
    runtime.assert_not_called()


@pytest.mark.parametrize("kind", ["mutations", "gene-references"])
def test_fixed_tp53_cli_template_is_a_no_model_exact_contract_and_execution_requires_matching_opt_in(monkeypatch, tmp_path, kind):
    from prism_sdk import reviewed_ensembl_gene_retrieval as ensembl
    from prism_sdk import reviewed_gdc_gene_mutation_retrieval as mutation
    from prism_sdk import reviewed_ncbi_gene_retrieval as ncbi
    runtime = Mock(side_effect=AssertionError("fixed template must not construct a model"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", runtime)
    sources = [Mock(side_effect=AssertionError("source dispatch is not admitted by this test")) for _ in range(3)]
    monkeypatch.setattr(ensembl, "_default_fetch", sources[0])
    monkeypatch.setattr(mutation.ReviewedGdcGeneMutationRetrievalAdapter, "execute", sources[1])
    monkeypatch.setattr(ncbi.ReviewedNcbiGeneRetrievalAdapter, "execute", sources[2])
    target, checkpoint = tmp_path / "plan.json", tmp_path / "run.json"
    code, payload, errors = _invoke("workflow-template", "--template", kind, "--task", "Review fixed public TP53 annotations", "--out", str(target))
    assert code == 0 and errors == "" and payload["source_dispatch"] is False
    _assert_fixed_tp53_plan(json.loads(target.read_text(encoding="utf-8")), kind)
    # Missing catalogue authority is refused before writing a checkpoint or dispatching a source.
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), "--checkpoint-store", str(checkpoint), "--approve-source-dispatch")
    assert code == 2 and payload is None and not checkpoint.exists()
    code, payload, errors = _invoke("workflow-execute", "--plan", str(target), _FIXED_TP53_FLAGS[kind], "--checkpoint-store", str(checkpoint))
    assert code == 1 and errors == "" and payload["status"] == "approval_required"
    assert load_workflow_checkpoint(checkpoint)["attempts"] == {}
    runtime.assert_not_called()
    for source in sources:
        source.assert_not_called()


@pytest.mark.parametrize("kind", ["mutations", "gene-references"])
@pytest.mark.parametrize("option", [["--no-tcga"], ["--no-gwas"], ["--grounded-glioma"],
    ["--literature-limit", "2"], ["--trial-page-size", "1"], ["--trial-max-pages", "1"],
    ["--association-page-size", "1"], ["--gwas-page-size", "1"], ["--gwas-max-pages", "1"], ["--max-steps", "1"]])
def test_fixed_tp53_cli_template_refuses_irrelevant_evidence_molecular_and_model_options_before_persistence(monkeypatch, tmp_path, kind, option):
    runtime = Mock(side_effect=AssertionError("irrelevant options must predate model setup"))
    monkeypatch.setattr(workflow_cli, "LLMRuntime", runtime)
    target = tmp_path / "must-not-exist.json"
    code, payload, errors = _invoke("workflow-template", "--template", kind, "--task", "Review public TP53 metadata",
                                   "--out", str(target), *option)
    assert code == 2 and payload is None and not target.exists()
    runtime.assert_not_called()


@pytest.mark.parametrize("kind", ["mutations", "gene-references"])
@pytest.mark.parametrize("status", ["needs_evidence", "awaiting_scientific_human_review"])
def test_fixed_tp53_review_schemas_project_scientific_holds_without_source_values(kind, status):
    from prism_sdk.glioma_mutation_workflow import GLIOMA_MUTATION_REVIEW_SCHEMA
    from prism_sdk.glioma_gene_reference_workflow import GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA
    schema = GLIOMA_MUTATION_REVIEW_SCHEMA if kind == "mutations" else GLIOMA_GENE_REFERENCE_REVIEW_SCHEMA
    value = {"schema": schema, "status": status, "human_review_required": True,
             "scientific_support_established": False, "clinical_actions_authorized": False,
             "analysis": {"private_fixture_marker": PUBLIC_VALUE}}
    statuses = workflow_cli._output_review_statuses({"review": value})
    assert statuses == {"review": {"schema": schema, "status": status, "human_review_required": True,
                                  "scientific_support_established": False, "clinical_actions_authorized": False}}
    assert PUBLIC_VALUE not in json.dumps(statuses)
