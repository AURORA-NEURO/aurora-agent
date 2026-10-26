"""CLI workflow contracts use injected providers and sources, never external services."""

from __future__ import annotations

import io
import json
import os
from datetime import datetime, timezone
from unittest.mock import Mock

import pytest

from prism_sdk.cli import _parser, main
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


def _fixture_catalog(monkeypatch):
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
                  "required": ["public_value"], "additionalProperties": False}
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


def test_unresolvable_dependency_blocks_before_the_dependent_source_is_called(monkeypatch, tmp_path):
    factory, calls, _ = _fixture_catalog(monkeypatch)
    plan, _ = _plan_file(tmp_path, factory, second=True, invalid_reference=True)
    code, payload, errors = _invoke("workflow-execute", "--plan", str(plan), "--approve-source-dispatch")
    assert code == 1 and errors == "" and payload["status"] == "blocked" and len(calls) == 1
    assert PUBLIC_VALUE not in json.dumps(payload)


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
