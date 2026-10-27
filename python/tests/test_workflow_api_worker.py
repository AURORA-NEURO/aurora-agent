"""The fixed application worker retains contracts and never confuses metadata with evidence."""
from copy import deepcopy
import io
import json
import math
import os
from pathlib import Path
import subprocess
import sys
from threading import Event, Thread
import time
from types import SimpleNamespace
from unittest.mock import Mock

import pytest

from prism_sdk import llm_runtime
from prism_sdk import workflow_api_worker as worker_module
from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_api_worker import (
    MAX_WORKFLOW_API_DIAGNOSTIC_ROWS, MAX_WORKFLOW_API_REQUEST_BYTES, WorkflowAPIError,
    WorkflowAPIWorker, _diagnostic_rows, _error,
)
from prism_sdk.workflow_builder import load_workflow_checkpoint, save_workflow_checkpoint
from prism_sdk.workflow_builder import execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import compile_workflow_requirements
from test_workflow_cli import (ERROR_DETAIL, PUBLIC_VALUE, _FIXED_TP53_OPERATIONS,
                               _assert_fixed_tp53_plan, _fixture_catalog, _proposal)


TASK = "Review glioma evidence using PubMed only; exclude trials and TCGA."


def _invoke(root, request, *, raw=False, flags=()):
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment["PYTHONPATH"] = str(Path(__file__).resolve().parents[1])
    process = subprocess.run([sys.executable, "-m", "prism_sdk.workflow_api_worker", "--root", str(root), *flags],
        input=request if raw else json.dumps(request), text=True, encoding="utf-8", capture_output=True,
        env=environment, timeout=30, cwd=root.parent)
    return process, json.loads(process.stdout)


def _typed(worker):
    return worker.handle({"action": "plan", "task": TASK, "planner": "glioma-intent"})


def _create(worker, plan):
    return worker.handle({"action": "create_run", "plan_id": plan["plan_id"],
                          "expected_plan_digest": plan["plan_digest"]})


def _execute(worker, run, **kwargs):
    return worker.handle({"action": "execute", "run_id": run["run_id"],
        "expected_plan_digest": run["plan_digest"], "approve_source_dispatch": True, **kwargs})


def _fixture(monkeypatch, tmp_path, *, second=True, contract=None):
    factory, calls, state = _fixture_catalog(monkeypatch)
    worker = WorkflowAPIWorker(tmp_path / "application", model="fixture-model")
    worker.catalog = factory()
    requests = []
    runtime = LLMRuntime()
    def provider(request):
        requests.append(request)
        return json.dumps(_proposal(second=second))
    runtime.register_in_memory_provider("ollama", provider)
    monkeypatch.setattr(runtime, "register_provider", lambda config: None)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", lambda: runtime)
    request = {"action": "plan", "task": "Review public metadata", "planner": "local-model",
               "approve_provider_call": True, "max_steps": 2}
    if contract is not None:
        request["requirements"] = contract.to_dict()
    plan = worker.handle(request)
    return worker, calls, state, requests, plan


def test_actual_subprocess_roundtrip_retains_typed_task_and_fixed_contract_across_processes(tmp_path):
    root = tmp_path / "owned"
    process, response = _invoke(root, {"action": "plan", "task": TASK, "planner": "glioma-intent"})
    assert process.returncode == 0 and process.stderr == "" and response["ok"] is True
    plan = response["data"]
    assert plan["task"] == TASK and plan["source_dispatch"] is False
    assert plan["plan"]["requirements"]["forbidden_operations"] == ["clinical_trials.search", "gdc.projects"]
    process, loaded = _invoke(root, {"action": "get_plan", "plan_id": plan["plan_id"]})
    assert process.returncode == 0 and loaded["data"] == plan
    process, run = _invoke(root, {"action": "create_run", "plan_id": plan["plan_id"],
                                "expected_plan_digest": plan["plan_digest"]})
    assert process.returncode == 0 and run["data"]["status"] == "queued"
    process, held = _invoke(root, {"action": "execute", "run_id": run["data"]["run_id"],
        "expected_plan_digest": plan["plan_digest"], "approve_source_dispatch": False})
    assert process.returncode == 0 and held["data"]["status"] == "approval_required"
    assert held["data"]["checkpoint"]["attempts"] == {}


@pytest.mark.parametrize("raw", [
    '{"action":"catalogue","action":"execute"}',
    '{"action":"catalogue"} {"action":"catalogue"}',
    '{"action":"catalogue","secret":NaN}',
    '[' + '[' * 1200 + '0' + ']' * 1200 + ']',
    '{"action":"catalogue","padding":"' + 'x' * MAX_WORKFLOW_API_REQUEST_BYTES + '"}',
], ids=["duplicate_fields", "multiple_values", "nonfinite", "deep", "oversized"])
def test_actual_subprocess_refuses_ambiguous_nonfinite_deep_or_oversized_json_without_tracebacks(tmp_path, raw):
    process, result = _invoke(tmp_path / "owned", raw, raw=True)
    assert process.returncode == 2 and process.stderr == ""
    assert result["ok"] is False and set(result["error"]) == {"code", "message"}
    assert "Traceback" not in process.stdout


def test_pure_interpretation_and_catalogue_do_not_construct_a_provider(monkeypatch, tmp_path):
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("pure boundary")))
    worker = WorkflowAPIWorker(tmp_path / "owned")
    catalog = worker.handle({"action": "catalogue"})
    assert catalog["operation_count"] == len(catalog["operations"])
    assert {op["implementation"] for op in catalog["operations"]} <= {"local", "live_adapter"}
    result = worker.handle({"action": "interpret", "task": TASK})
    assert result["status"] == "ready" and result["provider_calls"] == 0


def test_unsupported_typed_clause_returns_exact_clarification_and_no_plan(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    result = worker.handle({"action": "plan", "task": "Review glioma evidence after 2024.", "planner": "glioma-intent"})
    assert result["status"] == "needs_clarification" and result["source_dispatch"] is False
    assert result["interpretation"]["unresolved_spans"] and not list((worker.root / "plans").iterdir())


def test_valid_edit_keeps_original_task_contract_and_invalidates_old_run_approval(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    original = _typed(worker)
    run = _create(worker, original)
    proposal = {key: deepcopy(original["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["name"] = "Edited name with identical evidence contract"
    edited = worker.handle({"action": "validate", "plan_id": original["plan_id"],
        "expected_plan_digest": original["plan_digest"], "proposal": proposal})
    assert edited["plan_digest"] != original["plan_digest"] and edited["task"] == TASK
    assert edited["plan"]["requirements"] == original["plan"]["requirements"]
    with pytest.raises(WorkflowAPIError, match="approval"):
        _execute(worker, run)
    assert not (worker.root / "runs" / run["run_id"] / "checkpoint.json").exists()
    with pytest.raises(WorkflowAPIError) as error:
        worker.handle({"action": "validate", "plan_id": original["plan_id"],
            "expected_plan_digest": original["plan_digest"], "proposal": proposal})
    assert error.value.code == "revision_conflict"


def test_retained_contract_consistency_does_not_coerce_literal_true_to_integer_one(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    plan = _typed(worker)
    path = worker.root / "plans" / plan["plan_id"] / "plan.json"
    retained = json.loads(path.read_text(encoding="utf-8"))
    retained["requirements"]["max_calls_by_operation"]["pubmed.search"] = True
    retained["record_digest"] = content_digest({key: value for key, value in retained.items() if key != "record_digest"})
    save_workflow_checkpoint(path, retained)
    with pytest.raises(WorkflowAPIError, match="contract"):
        worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]})


def test_idle_inspection_holds_execution_lock_while_reading_run_and_checkpoint(monkeypatch, tmp_path):
    worker, _, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    _execute(worker, run)
    original = worker._run
    observations = []
    def checked_read(run_id):
        with pytest.raises(WorkflowAPIError) as error:
            with worker._lock(worker._path("runs", run_id, "execute.lock")):
                raise AssertionError("idle inspection must own the observation lock")
        observations.append(error.value.code)
        return original(run_id)
    monkeypatch.setattr(worker, "_run", checked_read)
    result = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    assert result["status"] == "completed" and result["active"] is False
    assert observations == ["resource_busy"]


def test_execution_waits_for_actual_brief_idle_observer_lock_contention_without_duplicate_admission(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    entered, release, collision = Event(), Event(), Event()
    def observer():
        with worker._lock(worker._path("runs", run["run_id"], "execute.lock")):
            entered.set()
            release.wait(5)
    thread = Thread(target=observer)
    thread.start()
    try:
        assert entered.wait(5)
        if os.name == "nt":
            import msvcrt
            owner, function = msvcrt, "locking"
        else:
            import fcntl
            owner, function = fcntl, "flock"
        actual_lock = getattr(owner, function)
        def observe_collision(*args):
            try:
                return actual_lock(*args)
            except OSError:
                # Release only after the admission path really collided with the OS lock.
                # The old immediate-refusal implementation fails this test deterministically.
                collision.set()
                release.set()
                raise
        monkeypatch.setattr(owner, function, observe_collision)
        result = _execute(worker, run, max_step_calls=1)
    finally:
        release.set()
        thread.join(5)
    assert not thread.is_alive() and collision.is_set()
    assert result["status"] == "paused" and len(calls) == 1
    assert result["checkpoint"]["attempts"] == {"first": 1}
    assert worker._run(run["run_id"])["execution_count"] == 1


def test_idle_observer_held_past_admission_bound_refuses_without_run_state_or_source_changes(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    entered, release = Event(), Event()
    path = worker.root / "runs" / run["run_id"] / "run.json"
    original = path.read_bytes()
    def observer():
        with worker._lock(worker._path("runs", run["run_id"], "execute.lock")):
            entered.set()
            release.wait(5)
    thread = Thread(target=observer)
    thread.start()
    try:
        assert entered.wait(5)
        started = time.monotonic()
        with pytest.raises(WorkflowAPIError) as error:
            _execute(worker, run, max_step_calls=1)
        elapsed = time.monotonic() - started
        assert error.value.code == "resource_busy" and 0.9 <= elapsed < 3
        assert not release.is_set()
        assert path.read_bytes() == original and worker._run(run["run_id"])["execution_count"] == 0
        assert not (path.parent / "checkpoint.json").exists() and calls == []
    finally:
        release.set()
        thread.join(5)
    assert not thread.is_alive()


def test_release_queued_behind_observation_cannot_consume_a_later_cancellation(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    worker.handle({"action": "cancel", "run_id": run["run_id"]})
    assert _execute(worker, run)["status"] == "cancelled" and calls == []
    assert worker._cancel_generation(run["run_id"]) == 1
    entered, release, collision = Event(), Event(), Event()
    def observer():
        with worker._lock(worker._path("runs", run["run_id"], "execute.lock")):
            entered.set()
            release.wait(5)
    thread = Thread(target=observer)
    thread.start()
    try:
        assert entered.wait(5)
        if os.name == "nt":
            import msvcrt
            owner, function = msvcrt, "locking"
        else:
            import fcntl
            owner, function = fcntl, "flock"
        actual_lock = getattr(owner, function)
        def later_cancellation_at_collision(*args):
            try:
                return actual_lock(*args)
            except OSError:
                if not collision.is_set():
                    collision.set()
                    # The release request has already observed generation 1 but cannot own
                    # the run yet. This is a distinct later stop request, not its old hold.
                    worker.handle({"action": "cancel", "run_id": run["run_id"]})
                    assert worker._cancel_generation(run["run_id"]) == 2
                    release.set()
                raise
        monkeypatch.setattr(owner, function, later_cancellation_at_collision)
        held = _execute(worker, run, resume=True, resume_cancelled=True)
    finally:
        release.set()
        thread.join(5)
    assert not thread.is_alive() and collision.is_set()
    assert held["status"] == "cancelled" and held["cancel_requested"] is True and calls == []
    assert worker._run(run["run_id"])["cancel_release_generation"] == 1
    assert held["checkpoint"]["attempts"] == {}
    completed = _execute(worker, run, resume=True, resume_cancelled=True)
    assert completed["status"] == "completed" and completed["cancel_requested"] is False
    assert len(calls) == 2 and completed["checkpoint"]["attempts"] == {"first": 1, "second": 1}
    assert worker._run(run["run_id"])["cancel_release_generation"] == 2


@pytest.mark.parametrize("change", ["source_limit", "source_query", "source_operation", "drop_handoff", "replace_contract"])
def test_edit_cannot_weaken_retained_source_scope_arguments_flow_or_task_contract(tmp_path, change):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    original = _typed(worker)
    proposal = {key: deepcopy(original["plan"][key]) for key in ("name", "steps", "outputs")}
    source = proposal["steps"][0]
    if change == "source_limit":
        source["arguments"]["per_specialty_limit"] = 1
    elif change == "source_query":
        source["arguments"]["specialty_lanes"] = ["not_glioma"]
    elif change == "source_operation":
        source["operation"] = "clinical_trials.search"
    elif change == "drop_handoff":
        proposal["steps"] = [source]
        proposal["outputs"] = [source["id"]]
    else:
        proposal["requirements"] = compile_workflow_requirements(TASK).to_dict()
    with pytest.raises((ValueError, TypeError, KeyError)):
        worker.handle({"action": "validate", "plan_id": original["plan_id"],
            "expected_plan_digest": original["plan_digest"], "proposal": proposal})
    assert worker.handle({"action": "get_plan", "plan_id": original["plan_id"]}) == original


def test_model_planning_requires_separate_literal_approval_before_runtime_creation(monkeypatch, tmp_path):
    runtime = Mock(side_effect=AssertionError("approval predates provider"))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", runtime)
    worker = WorkflowAPIWorker(tmp_path / "owned", model="test")
    for approval in (False, 1, "true"):
        with pytest.raises(WorkflowAPIError):
            worker.handle({"action": "plan", "task": "Review metadata", "planner": "local-model",
                           "approve_provider_call": approval})
    assert runtime.call_count == 0


def test_model_plan_retains_caller_contract_and_step_budget_through_edits(monkeypatch, tmp_path):
    contract = compile_workflow_requirements("Review public metadata", argument_rules=[
        {"operation": "fixture.retrieve", "path": "/payload", "comparison": "equals", "value": PUBLIC_VALUE}])
    worker, calls, _, requests, plan = _fixture(monkeypatch, tmp_path, contract=contract)
    assert len(requests) == 1 and calls == []
    assert plan["plan"]["requirements"]["max_steps"] == 2
    assert contract.to_dict()["argument_rules"][0] in plan["plan"]["requirements"]["argument_rules"]
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["steps"][0]["arguments"]["payload"] = "broadened"
    with pytest.raises(ValueError):
        worker.handle({"action": "validate", "plan_id": plan["plan_id"],
            "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    assert calls == []


def test_invalid_edit_returns_actionable_contract_metadata_without_actual_argument_values(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    plan = _typed(worker)
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["steps"][0]["arguments"]["per_specialty_limit"] = 1
    with pytest.raises(WorkflowAPIError) as error:
        worker.handle({"action": "validate", "plan_id": plan["plan_id"],
            "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    detail = _error(error.value)["error"]["diagnostics"]
    requirement = detail["requirements"]
    assert requirement["status"] == "assessed" and requirement["contract_satisfied"] is False
    assert any(row["code"] == "argument_requirement_unsatisfied" and row["path"] == "/per_specialty_limit"
               for row in requirement["blocking_issues"])
    assert all("value" not in row and "arguments" not in row for row in requirement["blocking_issues"])
    assert detail["source_dispatch"] is False and detail["provider_calls"] == 0
    assert worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]}) == plan


def test_impossible_output_pointer_diagnostic_is_authoritative_and_never_calls_fixture_source(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["steps"][1]["arguments"]["payload"] = {"$ref": "first#/missing"}
    with pytest.raises(WorkflowAPIError) as error:
        worker.handle({"action": "validate", "plan_id": plan["plan_id"],
            "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    detail = _error(error.value)["error"]["diagnostics"]["reference_paths"]
    assert detail["status"] == "assessed" and detail["blocking_issue_count"] == 1
    assert detail["references"][0]["source_pointer"] == "/missing"
    assert detail["references"][0]["status"] == "statically_impossible"
    assert calls == [] and PUBLIC_VALUE not in json.dumps(detail)


def test_successful_edit_reports_runtime_reference_work_without_existence_or_science_claims(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["name"] = "Metadata-only editor diagnostic"
    changed = worker.handle({"action": "validate", "plan_id": plan["plan_id"],
        "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    detail = changed["validation"]
    assert detail["reference_paths"]["blocking_issue_count"] == 0
    assert detail["reference_paths"]["runtime_resolution_count"] == 1
    assert detail["reference_paths"]["references"][0]["status"] == "runtime_resolution_required"
    assert detail["requirements"]["contract_satisfied"] is True
    assert calls == [] and PUBLIC_VALUE not in json.dumps(detail)


def test_diagnostic_helper_failure_is_unassessed_and_does_not_fabricate_zero_issues(monkeypatch, tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    plan = _typed(worker)
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    monkeypatch.setattr(worker_module, "audit_workflow_reference_paths", Mock(side_effect=RuntimeError(ERROR_DETAIL)))
    monkeypatch.setattr(worker_module, "audit_workflow_requirements", Mock(side_effect=RuntimeError(ERROR_DETAIL)))
    changed = worker.handle({"action": "validate", "plan_id": plan["plan_id"],
        "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    for item in (changed["validation"]["reference_paths"], changed["validation"]["requirements"]):
        assert item["status"] == "not_assessed" and "blocking_issue_count" not in item
    assert ERROR_DETAIL not in json.dumps(changed["validation"])


def test_diagnostic_projection_bounds_rows_and_text_and_never_copies_nonwhitelisted_values():
    rows = [{"code": "required_value_flow_missing", "step_id": "a", "path": "/" + "x" * 300,
             "actual_value": PUBLIC_VALUE, "receipt": {"secret": ERROR_DETAIL}, "value": ERROR_DETAIL}
            for _ in range(MAX_WORKFLOW_API_DIAGNOSTIC_ROWS + 20)]
    projected = _diagnostic_rows(rows, {"code", "step_id", "path"})
    assert len(projected) == MAX_WORKFLOW_API_DIAGNOSTIC_ROWS
    assert projected[0]["path"] == "[metadata_field_omitted]"
    assert PUBLIC_VALUE not in json.dumps(projected) and ERROR_DETAIL not in json.dumps(projected)


def test_stdio_response_byte_bound_includes_the_protocol_newline(monkeypatch, tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    envelope = {"schema": worker_module.WORKFLOW_API_RESPONSE_SCHEMA, "ok": True,
                "data": worker.catalog.to_dict()}
    bound = len(worker_module._json(envelope).encode("utf-8"))
    output = io.BytesIO()
    monkeypatch.setattr(worker_module, "MAX_WORKFLOW_API_RESPONSE_BYTES", bound)
    monkeypatch.setattr(sys, "stdin", SimpleNamespace(buffer=io.BytesIO(b'{"action":"catalogue"}')))
    monkeypatch.setattr(sys, "stdout", SimpleNamespace(buffer=output))
    code = worker_module.main(["--root", str(worker.root)])
    assert code == 2 and len(output.getvalue()) <= bound
    assert json.loads(output.getvalue())["error"]["code"] == "response_too_large"


def test_exact_approval_and_new_run_admission_cannot_implicitly_replay_a_previous_run(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    with pytest.raises(WorkflowAPIError):
        _execute(worker, dict(run, plan_digest="0" * 64))
    assert calls == []
    first = _execute(worker, run, max_step_calls=1)
    assert first["status"] == "paused" and len(calls) == 1
    with pytest.raises(WorkflowAPIError) as error:
        _execute(worker, run)
    assert error.value.code == "resume_required" and len(calls) == 1
    complete = _execute(worker, run, resume=True)
    assert complete["status"] == "completed" and len(calls) == 2
    assert complete["checkpoint"]["attempts"] == {"first": 1, "second": 1}


def test_polling_and_execution_metadata_omit_public_values_and_source_receipts_but_explicit_exports_retain_them(monkeypatch, tmp_path):
    worker, _, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    result = _execute(worker, run)
    polled = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    assert result == polled and PUBLIC_VALUE not in json.dumps(polled)
    assert "local_transport_fixture" not in json.dumps(polled)
    assert all("value" not in row and "receipt" not in row for row in polled["checkpoint"]["results"].values())
    exported = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "outputs"})
    assert exported["outputs"] == {"second": {"public_value": PUBLIC_VALUE}}
    checkpoint = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "checkpoint"})["checkpoint"]
    assert checkpoint["results"]["first"]["receipt"] == {"source": "local_transport_fixture"}


def test_json_text_export_preserves_big_integers_float_tokens_and_replayable_checkpoint_root_without_source_replay(monkeypatch, tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned", model="fixture-model")
    worker.catalog = WorkflowOperationCatalog()
    schema = {"type": "object", "additionalProperties": False, "properties": {}, "required": []}
    output = {"type": "object", "additionalProperties": False,
              "properties": {"integer": {"type": "integer"}, "positive_zero": {"type": "number"},
                             "negative_zero": {"type": "number"}, "float_one": {"type": "number"},
                             "integer_zero": {"type": "integer"}, "false": {"type": "boolean"}},
              "required": ["integer", "positive_zero", "negative_zero", "float_one", "integer_zero", "false"]}
    value = {"integer": 2 ** 63 + 1, "positive_zero": 0.0, "negative_zero": -0.0,
             "float_one": 1.0, "integer_zero": 0, "false": False}
    calls = []
    def source(arguments, **kwargs):
        calls.append(dict(arguments))
        return WorkflowOperationResult(value, {"fixture": "numeric_transport", "observed_integer": 2 ** 63 + 1})
    worker.catalog.register(WorkflowOperation("fixture.numeric", "Read numeric fixture metadata", schema, output,
        "fixture", implementation="live_adapter"), source)
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("ollama", lambda request: json.dumps({"name": "Numeric export fixture",
        "steps": [{"id": "read", "operation": "fixture.numeric", "arguments": {}, "depends_on": []}], "outputs": ["read"]}))
    monkeypatch.setattr(runtime, "register_provider", lambda config: None)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", lambda: runtime)
    plan = worker.handle({"action": "plan", "task": "Read numeric fixture metadata", "planner": "local-model",
                          "approve_provider_call": True})
    run = _create(worker, plan)
    assert _execute(worker, run)["status"] == "completed" and calls == [{}]
    exported = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "checkpoint", "format": "json_text"})
    assert set(exported) == {"run_id", "plan_digest", "status", "kind", "json_text"}
    assert exported["kind"] == "checkpoint" and "checkpoint" not in exported
    native = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "checkpoint"})["checkpoint"]
    assert exported["json_text"] == worker_module._json(native)
    # The enclosing transport can parse/stringify its string without interpreting payload numbers.
    transported = json.loads(json.dumps(exported))["json_text"]
    assert str(2 ** 63 + 1) in transported and '"positive_zero":0.0' in transported and '"negative_zero":-0.0' in transported
    downloaded = tmp_path / "downloaded-checkpoint.json"
    downloaded.write_text(transported, encoding="utf-8")
    checkpoint = load_workflow_checkpoint(downloaded)
    retained = checkpoint["results"]["read"]["value"]
    assert retained["integer"] == 2 ** 63 + 1 and type(retained["positive_zero"]) is float
    assert type(retained["integer_zero"]) is int and retained["false"] is False and type(retained["float_one"]) is float
    assert math.copysign(1, retained["negative_zero"]) == -1
    assert checkpoint["checkpoint_digest"] == native["checkpoint_digest"]
    assert execute_workflow_plan(plan["plan"], worker.catalog, approved=False, checkpoint=checkpoint).status == "completed"
    outputs = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "outputs", "format": "json_text"})
    assert json.loads(outputs["json_text"]) == {"read": value} and "outputs" not in outputs
    assert calls == [{}]


@pytest.mark.parametrize("export_format", ["unknown", "JSON", "", True, 1, None])
def test_unknown_export_format_is_refused_before_storage_or_dispatch(tmp_path, export_format):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    with pytest.raises(WorkflowAPIError) as error:
        worker.handle({"action": "export", "run_id": "0" * 32, "kind": "checkpoint", "format": export_format})
    assert error.value.code == "invalid_request"
    assert not list((worker.root / "runs").iterdir())


def test_cancel_before_first_dispatch_is_durable_until_separate_explicit_release(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    worker.handle({"action": "cancel", "run_id": run["run_id"]})
    assert _execute(worker, run)["status"] == "cancelled" and calls == []
    assert _execute(worker, run, resume=True)["status"] == "cancelled" and calls == []
    complete = _execute(worker, run, resume=True, resume_cancelled=True)
    assert complete["status"] == "completed" and len(calls) == 2
    assert complete["cancel_requested"] is False


def test_inflight_cancel_drains_real_observation_and_resume_preserves_first_attempt(monkeypatch, tmp_path):
    worker, calls, state, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    entered, release = Event(), Event()
    state["before_execute"] = lambda: (entered.set(), release.wait(10))
    result = []
    thread = Thread(target=lambda: result.append(_execute(worker, run)))
    thread.start()
    try:
        assert entered.wait(10)
        inspected = worker.handle({"action": "inspect", "run_id": run["run_id"]})
        assert inspected["active"] is True and inspected["checkpoint"]["in_flight"] == "first"
        worker.handle({"action": "cancel", "run_id": run["run_id"]})
    finally:
        release.set()
        thread.join(10)
    assert not thread.is_alive() and result[0]["status"] == "cancelled" and len(calls) == 1
    first_digest = result[0]["checkpoint"]["results"]["first"]["value_digest"]
    state["before_execute"] = None
    complete = _execute(worker, run, resume=True, resume_cancelled=True)
    assert complete["status"] == "completed" and len(calls) == 2
    assert complete["checkpoint"]["results"]["first"]["value_digest"] == first_digest


def test_source_failure_retains_sanitized_failure_and_needs_distinct_retry_grant(monkeypatch, tmp_path):
    worker, calls, state, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    state["fail"] = True
    failed = _execute(worker, run)
    assert failed["status"] == "failed" and ERROR_DETAIL not in json.dumps(failed)
    state["fail"] = False
    assert _execute(worker, run, resume=True)["status"] == "retry_required" and len(calls) == 1
    assert _execute(worker, run, resume=True, retry_failed=True)["status"] == "completed"
    assert len(calls) == 3


def test_checkpoint_write_failure_prevents_dispatch_and_survives_as_honest_error(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    original = worker._write
    def fail_checkpoint(path, value):
        if path.name == "checkpoint.json":
            raise OSError(ERROR_DETAIL)
        original(path, value)
    monkeypatch.setattr(worker, "_write", fail_checkpoint)
    with pytest.raises(OSError):
        _execute(worker, run)
    assert calls == []
    status = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    assert status["status"] == "error" and status["error_type"] == "OSError"
    assert ERROR_DETAIL not in json.dumps(status)
    with pytest.raises(WorkflowAPIError) as error:
        _execute(worker, run, resume=True)
    assert error.value.code == "recovery_required" and calls == []


def test_running_record_without_os_lock_reports_interrupted_and_cannot_automatically_replay(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    record = worker._run(run["run_id"])
    record.update(status="running", execution_count=1)
    worker._save_run(record)
    status = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    assert status["status"] == "interrupted" and status["active"] is False
    with pytest.raises(WorkflowAPIError) as error:
        _execute(worker, run, resume=True, retry_failed=True)
    assert error.value.code == "recovery_required" and calls == []


def test_unknown_completion_checkpoint_after_restart_requires_explicit_retry(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    _execute(worker, run, max_step_calls=1)
    path = worker.root / "runs" / run["run_id"] / "checkpoint.json"
    checkpoint = load_workflow_checkpoint(path)
    checkpoint.update(status="running", in_flight="second", attempts={"first": 1, "second": 1})
    checkpoint["checkpoint_digest"] = content_digest({key: value for key, value in checkpoint.items() if key != "checkpoint_digest"})
    save_workflow_checkpoint(path, checkpoint)
    record = worker._run(run["run_id"])
    record["status"] = "running"
    worker._save_run(record)
    assert worker.handle({"action": "inspect", "run_id": run["run_id"]})["status"] == "interrupted"
    assert _execute(worker, run, resume=True)["status"] == "retry_required" and len(calls) == 1
    assert _execute(worker, run, resume=True, retry_failed=True)["status"] == "completed"
    assert len(calls) == 2


def test_actual_process_death_releases_activity_lock_and_retains_unknown_dispatch_boundary(monkeypatch, tmp_path):
    worker, calls, _, _, plan = _fixture(monkeypatch, tmp_path)
    run = _create(worker, plan)
    marker = tmp_path / "entered-source.txt"
    script = """
import sys, time
from pathlib import Path
from pytest import MonkeyPatch
import prism_sdk.workflow_api_worker as module
from test_workflow_cli import _fixture_catalog
patch = MonkeyPatch()
factory, calls, state = _fixture_catalog(patch)
module.builtin_workflow_operation_catalog = factory
module.register_glioma_workflow_operations = lambda catalog: catalog
def admitted():
    Path(sys.argv[2]).write_text('source boundary entered', encoding='utf-8')
    time.sleep(60)
state['before_execute'] = admitted
raise SystemExit(module.main(['--root', sys.argv[1]]))
"""
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    environment["PYTHONPATH"] = os.pathsep.join([str(Path(__file__).resolve().parents[1]), str(Path(__file__).parent)])
    child = subprocess.Popen([sys.executable, "-c", script, str(worker.root), str(marker)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment)
    try:
        child.stdin.write(json.dumps({"action": "execute", "run_id": run["run_id"],
            "expected_plan_digest": run["plan_digest"], "approve_source_dispatch": True}).encode("utf-8"))
        child.stdin.close()
        deadline = time.monotonic() + 20
        while not marker.exists() and time.monotonic() < deadline and child.poll() is None:
            time.sleep(0.05)
        assert marker.exists() and child.poll() is None
        running = worker.handle({"action": "inspect", "run_id": run["run_id"]})
        assert running["active"] is True and running["checkpoint"]["in_flight"] == "first"
        with pytest.raises(WorkflowAPIError) as error:
            _execute(worker, run, resume=True)
        assert error.value.code == "resource_busy" and calls == []
    finally:
        if child.poll() is None:
            child.kill()
        child.wait(timeout=10)
        child.stdout.close()
        child.stderr.close()
        if not child.stdin.closed:
            child.stdin.close()
    interrupted = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    assert interrupted["active"] is False and interrupted["status"] == "interrupted"
    assert interrupted["checkpoint"]["in_flight"] == "first"
    assert _execute(worker, run, resume=True)["status"] == "retry_required" and calls == []
    assert _execute(worker, run, resume=True, retry_failed=True)["status"] == "completed"
    assert len(calls) == 2


@pytest.mark.parametrize("payload", [
    {"action": "catalogue", "argv": ["program"]},
    {"action": "interpret", "task": TASK, "env": {}},
    {"action": "plan", "task": TASK, "planner": "glioma-intent", "mcp": []},
    {"action": "plan", "task": TASK, "planner": "glioma-intent", "root": "other"},
    {"action": "plan", "task": TASK, "planner": "glioma-intent", "max_steps": True},
    {"action": "plan", "task": TASK, "planner": "glioma-intent", "planning_attempts": 2},
    {"action": "execute", "run_id": "../escape", "expected_plan_digest": "0" * 64, "approve_source_dispatch": True},
    {"action": "execute", "run_id": "0" * 32, "expected_plan_digest": "0" * 64, "approve_source_dispatch": 1},
    {"action": "execute", "run_id": "0" * 32, "expected_plan_digest": "0" * 64, "approve_source_dispatch": True, "retry_failed": True},
    {"action": "execute", "run_id": "0" * 32, "expected_plan_digest": "0" * 64, "approve_source_dispatch": True, "max_step_calls": 0},
    {"action": "install_connection", "token": "secret"},
])
def test_request_fields_cannot_expand_process_path_authority_or_control_bounds(tmp_path, payload):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    with pytest.raises(WorkflowAPIError):
        worker.handle(payload)
    assert not list((worker.root / "plans").iterdir()) and not list((worker.root / "runs").iterdir())


def test_symlinked_or_hardlinked_storage_is_refused_without_reading_or_replacing_other_files(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    plan = _typed(worker)
    path = worker.root / "plans" / plan["plan_id"] / "plan.json"
    alias = tmp_path / "alias.json"
    try:
        os.link(path, alias)
    except OSError:
        pytest.skip("Hard links are unavailable on this filesystem")
    with pytest.raises(WorkflowAPIError, match="aliases"):
        worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]})
    assert alias.read_bytes() == path.read_bytes()


def test_replacing_application_root_identity_is_refused_before_reading_a_plan(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    plan = _typed(worker)
    worker.root.rename(tmp_path / "old-owned")
    worker.root.mkdir()
    with pytest.raises(WorkflowAPIError, match="identity"):
        worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]})


def test_error_envelopes_do_not_expose_exception_text_paths_or_source_payloads():
    for error in (RuntimeError(ERROR_DETAIL), OSError(ERROR_DETAIL), ValueError(ERROR_DETAIL)):
        response = _error(error)
        assert response["ok"] is False and ERROR_DETAIL not in json.dumps(response)


@pytest.mark.parametrize("endpoint", ["https://example.com/v1", "http://user:secret@localhost/v1", "http://localhost/v1?key=secret", "file:///model"])
def test_model_configuration_is_operator_loopback_only(tmp_path, endpoint):
    with pytest.raises(WorkflowAPIError):
        WorkflowAPIWorker(tmp_path / "owned", base_url=endpoint)


def test_fixed_tp53_worker_opt_ins_extend_only_selected_sources_and_local_stages_without_default_drift(monkeypatch, tmp_path):
    runtime = Mock(side_effect=AssertionError("fixed worker catalogue must not construct a model"))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", runtime)
    base = WorkflowAPIWorker(tmp_path / "base").handle({"action": "catalogue"})
    assert base["operation_count"] == 10
    base_ids = {row["operation_id"] for row in base["operations"]}
    for kind, config in (("mutations", {"glioma_mutations": True}), ("gene-references", {"gene_references": True})):
        selected = WorkflowAPIWorker(tmp_path / kind, **config).handle({"action": "catalogue"})
        assert {row["operation_id"] for row in selected["operations"]} == base_ids | _FIXED_TP53_OPERATIONS[kind]
    repeated = WorkflowAPIWorker(tmp_path / "repeated").handle({"action": "catalogue"})
    assert repeated["catalog_digest"] == base["catalog_digest"] and repeated["operation_count"] == 10
    runtime.assert_not_called()


@pytest.mark.parametrize("kind,planner,flag", [("mutations", "glioma-mutations", "--glioma-mutations"),
                                              ("gene-references", "gene-references", "--gene-references")])
def test_fixed_tp53_actual_worker_process_persists_no_model_template_contract_and_refuses_implicit_catalogue_authority(tmp_path, kind, planner, flag):
    root = tmp_path / "fixed"
    request = {"action": "plan", "task": "Review public TP53 research source assertions", "planner": planner}
    denied_process, denied = _invoke(root, request)
    assert denied_process.returncode == 2 and denied_process.stderr == "" and denied["ok"] is False
    assert not list((root / "plans").iterdir()) and not list((root / "runs").iterdir())
    process, response = _invoke(root, request, flags=[flag])
    assert process.returncode == 0 and process.stderr == "" and response["ok"] is True
    data = response["data"]
    assert data["planner"] == planner and data["source_dispatch"] is False
    assert data["interpretation"]["provider_calls"] == 0
    _assert_fixed_tp53_plan(data["plan"], kind)
    process, retained = _invoke(root, {"action": "get_plan", "plan_id": data["plan_id"]}, flags=[flag])
    assert process.returncode == 0 and retained["data"] == data
    process, missing_flag = _invoke(root, {"action": "get_plan", "plan_id": data["plan_id"]})
    assert process.returncode == 2 and missing_flag["ok"] is False


@pytest.mark.parametrize("kind,planner,configuration,minimum", [
    ("mutations", "glioma-mutations", {"glioma_mutations": True}, 3),
    ("gene-references", "gene-references", {"gene_references": True}, 4)])
def test_fixed_tp53_worker_step_budget_and_irrelevant_model_attempts_refuse_before_plan_or_run_persistence(monkeypatch, tmp_path, kind, planner, configuration, minimum):
    runtime = Mock(side_effect=AssertionError("fixed source template has no model"))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", runtime)
    worker = WorkflowAPIWorker(tmp_path / kind, **configuration)
    for options in ({"max_steps": minimum - 1}, {"planning_attempts": 2}, {"operation": "arbitrary.read"}):
        with pytest.raises(ValueError):
            worker.handle({"action": "plan", "task": "Review public TP53 source metadata", "planner": planner, **options})
        assert not list((worker.root / "plans").iterdir()) and not list((worker.root / "runs").iterdir())
    data = worker.handle({"action": "plan", "task": "Review public TP53 source metadata", "planner": planner, "max_steps": minimum})
    _assert_fixed_tp53_plan(data["plan"], kind)
    runtime.assert_not_called()


@pytest.mark.parametrize("kind,planner,configuration", [("mutations", "glioma-mutations", {"glioma_mutations": True}),
                                                      ("gene-references", "gene-references", {"gene_references": True})])
def test_fixed_tp53_worker_scientific_review_status_survives_actual_local_execution_and_inspection(monkeypatch, tmp_path, kind, planner, configuration):
    worker = WorkflowAPIWorker(tmp_path / kind, **configuration)
    if kind == "gene-references":
        from test_glioma_gene_reference_workflow import _catalog
        worker.catalog, ensembl, ncbi_calls = _catalog()
        calls = lambda: (len(ensembl.calls), len(ncbi_calls))
        expected_calls = (2, 1)
    else:
        from prism_sdk.glioma_mutation_workflow import register_glioma_mutation_workflow_operations
        from prism_sdk.reviewed_gdc_gene_mutation_retrieval import register_gdc_gene_mutation_operations
        from test_reviewed_gdc_gene_mutation_retrieval import _execute as native_execute
        original = register_glioma_mutation_workflow_operations(register_gdc_gene_mutation_operations(WorkflowOperationCatalog()))
        native, observed = native_execute(), []
        def source(arguments, **kwargs):
            assert kwargs["approve_source_dispatch"] is True
            observed.append(dict(arguments))
            return WorkflowOperationResult(native.bundle, native.receipt)
        def local(operation):
            return lambda arguments, **kwargs: original.execute(operation, arguments, **kwargs)
        worker.catalog = WorkflowOperationCatalog(original.operations(), {operation.operation_id:
            source if operation.operation_id == "gdc.gene_mutation_counts" else local(operation.operation_id)
            for operation in original.operations()})
        calls, expected_calls = lambda: (len(observed),), (1,)
    runtime = Mock(side_effect=AssertionError("fixed source execution must not create a model"))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", runtime)
    plan = worker.handle({"action": "plan", "task": "Review public TP53 metadata", "planner": planner})
    run = _create(worker, plan)
    denied = worker.handle({"action": "execute", "run_id": run["run_id"],
        "expected_plan_digest": run["plan_digest"], "approve_source_dispatch": False})
    assert denied["status"] == "approval_required" and all(count == 0 for count in calls())
    completed = _execute(worker, run, resume=True)
    assert completed["status"] == "completed" and calls() == expected_calls
    status = worker.handle({"action": "inspect", "run_id": run["run_id"]})
    review = status["output_review_statuses"]["review"]
    assert review["status"] == "awaiting_scientific_human_review" and review["human_review_required"] is True
    assert review["scientific_support_established"] is False and review["clinical_actions_authorized"] is False
    _execute(worker, run, resume=True)
    assert calls() == expected_calls
    runtime.assert_not_called()
