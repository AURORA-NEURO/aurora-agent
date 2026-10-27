"""Durable caller declaration review does not attest a human or spend source authority."""
from copy import deepcopy
import json
import os
from pathlib import Path
import subprocess
from types import SimpleNamespace
import sys
from unittest.mock import Mock

import pytest

from prism_sdk import llm_runtime
from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderError
from prism_sdk.workflow_api_worker import WorkflowAPIError, WorkflowAPIWorker
from prism_sdk.workflow_builder import compile_workflow_plan, save_workflow_checkpoint
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_intent_obligations import compile_workflow_intent_obligations
from prism_sdk.workflow_intent_extractor import LocalWorkflowIntentExtractor, validate_workflow_intent_extraction


TASK = "Read metadata; preserve café 🔬."


def _rows(value=1.0):
    text = "preserve café 🔬"
    return [{"id": "payload", "kind": "argument_equals", "operation": "local.read", "path": "/payload", "value": value,
             "span": {"start": TASK.index(text), "end": TASK.index(text) + len(text), "text": text}},
            {"id": "read", "kind": "required_operation", "operation": "local.read", "output": True,
             "span": {"start": 0, "end": 13, "text": "Read metadata"}}]


def _catalog(calls):
    schema = {"type": "object", "properties": {"payload": {}}, "additionalProperties": False}
    catalog = WorkflowOperationCatalog()
    for operation in ("local.read", "local.extra"):
        def execute(arguments, *, _operation=operation, **_kwargs):
            calls.append((_operation, arguments))
            return WorkflowOperationResult(dict(arguments), {"fixture": True})
        catalog.register(WorkflowOperation(operation, "Local fixture", schema, schema, "local", implementation="local"), execute)
    return catalog


def _worker(tmp_path):
    calls = []
    worker = WorkflowAPIWorker(tmp_path / "application", model="fixture")
    worker.catalog = _catalog(calls)
    return worker, calls


def _request(**changes):
    return {"task": TASK, "planner": "local-model", "max_steps": 2, **changes}


def _prepare(worker, rows=None, **changes):
    return worker.handle({"action": "prepare_review", **_request(**changes), "obligations": _rows() if rows is None else rows})


def _accept(worker, review, **changes):
    return worker.handle({"action": "accept_review", "review_id": review["review_id"],
        "expected_review_digest": review["review_digest"], "accept_caller_declarations": True, **changes})


def _plan(worker, review, **changes):
    return worker.handle({"action": "plan", **_request(), "approve_provider_call": True,
        "intent_review_id": review["review_id"],
        "expected_acceptance_digest": review["acceptance"]["acceptance_digest"] if review["acceptance"] else "0" * 64,
        **changes})


def _runtime(monkeypatch, *, incremental=False):
    requests = []
    decisions = ([{"action": "append", "operation": "local.read"},
                  {"action": "arguments", "arguments": {"payload": 1.0}, "additional_dependencies": []},
                  {"action": "finish", "name": "Read", "outputs": ["s1"]}] if incremental else
                 [{"name": "Read", "steps": [{"id": "read", "operation": "local.read", "arguments": {"payload": 1.0}, "depends_on": []}], "outputs": ["read"]}])
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("ollama", lambda request: requests.append(request) or json.dumps(decisions.pop(0)))
    monkeypatch.setattr(runtime, "register_provider", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", lambda: runtime)
    return requests


def _restamp(value):
    value.pop("record_digest", None)
    value["record_digest"] = content_digest(value)
    return value


def _extraction_data(candidate):
    rows = [{**row, "span": {"text": row["span"]["text"], "occurrence": 0}} for row in candidate.to_dict()["obligations"]]
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("ollama", lambda _request: json.dumps({"action": "candidate", "obligations": rows}))
    return LocalWorkflowIntentExtractor(_catalog([]), runtime, model="fixture").extract(TASK, approve_provider_call=True).to_dict()


def test_prepare_accept_and_get_are_model_free_keep_original_candidate_and_return_idempotent_acceptance(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("no model construction")))
    review = _prepare(worker)
    directory = worker.root / "reviews" / review["review_id"]
    original = (directory / "review.json").read_bytes()
    assert review["status"] == "review_required" and review["acceptance"] is None
    assert review["candidate"]["interpretation_basis"] == "model_candidate"
    assert review["candidate"]["obligations"] == _rows()
    assert review["candidate_admission"]["status"] == "needs_clarification"
    assert review["origin"] == {"kind": "client_submitted_rows", "authorship": "unverified"}
    accepted = _accept(worker, review)
    acceptance_bytes = (directory / "acceptance.json").read_bytes()
    assert accepted["status"] == "accepted"
    assert accepted["acceptance"]["admission"]["status"] == "ready_for_constrained_authoring"
    assert "human_presence_authorship_and_extraction_completeness_unassessed" in accepted["acceptance"]["scope"]
    assert accepted["acceptance"]["accepted_declarations"]["interpretation_basis"] == "caller_supplied"
    assert _accept(worker, review) == accepted
    assert (directory / "review.json").read_bytes() == original
    assert (directory / "acceptance.json").read_bytes() == acceptance_bytes
    assert worker.handle({"action": "get_review", "review_id": review["review_id"]}) == accepted
    assert calls == [] and not list((worker.root / "plans").iterdir())


@pytest.mark.parametrize("extra", [{"interpretation_basis": "caller_supplied"}, {"human_reviewed": True},
                                  {"accept_caller_declarations": True}, {"approve_provider_call": True}])
def test_prepare_request_cannot_self_assert_authority_or_approval(tmp_path, extra):
    worker, calls = _worker(tmp_path)
    with pytest.raises(WorkflowAPIError):
        worker.handle({"action": "prepare_review", **_request(), "obligations": _rows(), **extra})
    assert not list((worker.root / "reviews").iterdir()) and calls == []


@pytest.mark.parametrize("change", [{"accept_caller_declarations": False}, {"accept_caller_declarations": 1},
                                   {"expected_review_digest": "0" * 64}, {"obligations": []}])
def test_acceptance_requires_literal_action_and_exact_retained_digest_without_replacement_rows(tmp_path, change):
    worker, _ = _worker(tmp_path)
    review = _prepare(worker)
    with pytest.raises(WorkflowAPIError):
        _accept(worker, review, **change)
    assert not (worker.root / "reviews" / review["review_id"] / "acceptance.json").exists()


def test_unaccepted_candidate_blocks_plan_before_provider_construction(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    review = _prepare(worker)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("must not construct")))
    with pytest.raises(WorkflowAPIError) as error:
        _plan(worker, review)
    assert error.value.code == "review_required" and calls == []
    assert not list((worker.root / "plans").iterdir())


@pytest.mark.parametrize("blocked", ["unavailable", "unresolved"])
def test_caller_acceptance_does_not_resolve_unavailable_or_unmapped_actions(monkeypatch, tmp_path, blocked):
    worker, calls = _worker(tmp_path)
    rows = _rows()
    if blocked == "unavailable":
        rows[1]["operation"] = "absent.archive"
    else:
        rows[1] = {"id": "unmapped", "kind": "unresolved_action", "span": rows[1]["span"], "reason": "No exact mapping"}
    review = _accept(worker, _prepare(worker, rows))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("must not construct")))
    result = _plan(worker, review)
    assert result["status"] in {"blocked", "needs_clarification"}
    assert result["intent_review"]["status"] == "accepted" and "plan" not in result
    assert calls == [] and not list((worker.root / "plans").iterdir())


@pytest.mark.parametrize("change", [{"task": TASK + " "}, {"max_steps": 3}, {"planning_attempts": 2},
                                   {"planner": "local-incremental"}, {"expected_acceptance_digest": "0" * 64}])
def test_changed_review_context_or_acceptance_refuses_before_provider(monkeypatch, tmp_path, change):
    worker, calls = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("must not construct")))
    with pytest.raises(WorkflowAPIError) as error:
        _plan(worker, review, **change)
    assert error.value.code == "revision_conflict" and calls == []


@pytest.mark.parametrize("incremental", [False, True])
def test_obligation_plan_envelope_is_opt_in_and_restart_recomputes_identical_contract(monkeypatch, tmp_path, incremental):
    worker, calls = _worker(tmp_path)
    options = ({"planner": "local-incremental", "operation_ids": ["local.read"], "max_model_calls": 3,
                "decision_attempts": 1} if incremental else {})
    review = _accept(worker, _prepare(worker, **options))
    requests = _runtime(monkeypatch, incremental=incremental)
    plan = _plan(worker, review, **options)
    assert plan["intent_review"] == review and len(requests) == (3 if incremental else 1)
    path = worker.root / "plans" / plan["plan_id"] / "plan.json"
    retained = json.loads(path.read_text(encoding="utf-8"))
    assert retained["schema"] == "aurora-workflow-api-plan/0.2"
    assert plan["plan"]["schema"] == "aurora-executable-workflow-plan/0.2"
    assert plan["plan"]["requirements"] == review["acceptance"]["admission"]["requirements"]
    restarted = WorkflowAPIWorker(worker.root, model="fixture")
    restarted.catalog = _catalog(calls)
    assert restarted.handle({"action": "get_plan", "plan_id": plan["plan_id"]}) == plan
    assert restarted.handle({"action": "get_review", "review_id": review["review_id"]}) == review
    assert calls == []


def test_accepted_declarations_do_not_grant_model_or_source_approval(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("must not construct")))
    with pytest.raises(WorkflowAPIError) as error:
        _plan(worker, review, approve_provider_call=False)
    assert error.value.code == "approval_required" and calls == []


def test_edit_retains_immutable_review_and_historical_run_evidence_uses_original_native_snapshot(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    _runtime(monkeypatch)
    plan = _plan(worker, review)
    run = worker.handle({"action": "create_run", "plan_id": plan["plan_id"], "expected_plan_digest": plan["plan_digest"]})
    original = worker.handle({"action": "export_evidence", "run_id": run["run_id"], "format": "json_text"})
    proposal = {key: deepcopy(plan["plan"][key]) for key in ("name", "steps", "outputs")}
    proposal["name"] = "Edited presentation"
    edited = worker.handle({"action": "validate", "plan_id": plan["plan_id"], "expected_plan_digest": plan["plan_digest"], "proposal": proposal})
    assert edited["plan_digest"] != plan["plan_digest"] and edited["intent_review"] == review
    assert worker.handle({"action": "export_evidence", "run_id": run["run_id"], "format": "json_text"}) == original
    bundle = json.loads(original["json_text"])
    assert bundle["plan_snapshot"] == plan["plan"] and type(bundle["plan_snapshot"]["steps"][0]["arguments"]["payload"]) is float
    current = worker.handle({"action": "export_evidence", "plan_id": plan["plan_id"]})
    assert current["evidence"]["plan_snapshot"] == edited["plan"] and current["run_id"] is None
    proposal["steps"][0]["arguments"]["payload"] = 1
    with pytest.raises(WorkflowAPIError):
        worker.handle({"action": "validate", "plan_id": plan["plan_id"], "expected_plan_digest": edited["plan_digest"], "proposal": proposal})
    assert calls == [] and not (worker.root / "runs" / run["run_id"] / "checkpoint.json").exists()


def test_review_acceptance_preserves_separate_source_grant_and_native_checkpoint_exports(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    _runtime(monkeypatch)
    plan = _plan(worker, review)
    run = worker.handle({"action": "create_run", "plan_id": plan["plan_id"], "expected_plan_digest": plan["plan_digest"]})
    held = worker.handle({"action": "execute", "run_id": run["run_id"], "expected_plan_digest": plan["plan_digest"],
                          "approve_source_dispatch": False})
    assert held["status"] == "approval_required" and calls == []
    completed = worker.handle({"action": "execute", "run_id": run["run_id"], "expected_plan_digest": plan["plan_digest"],
                               "approve_source_dispatch": True, "resume": True})
    assert completed["status"] == "completed" and len(calls) == 1
    assert "intent_review" not in completed and TASK not in json.dumps(completed)
    checkpoint = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "checkpoint"})["checkpoint"]
    native = worker.handle({"action": "export", "run_id": run["run_id"], "kind": "checkpoint", "format": "json_text"})
    assert json.loads(native["json_text"]) == checkpoint and "intent_review" not in checkpoint
    assert checkpoint["plan_digest"] == plan["plan_digest"] and len(calls) == 1


@pytest.mark.parametrize("target", ["candidate", "acceptance", "plan"])
def test_restamping_changed_lexical_or_contract_bindings_cannot_pass_recomputation(monkeypatch, tmp_path, target):
    worker, _ = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    if target == "plan":
        _runtime(monkeypatch)
        plan = _plan(worker, review)
        path = worker.root / "plans" / plan["plan_id"] / "plan.json"
        changed = json.loads(path.read_text(encoding="utf-8"))
        changed["intent_review"]["acceptance"]["accepted_declarations"]["obligations"][0]["value"] = 1
    else:
        path = worker.root / "reviews" / review["review_id"] / ("review.json" if target == "candidate" else "acceptance.json")
        changed = json.loads(path.read_text(encoding="utf-8"))
        if target == "candidate":
            changed["candidate"]["obligations"][0]["span"]["start"] += 1
        else:
            changed["accepted_declarations"]["obligations"][0]["value"] = 1
    save_workflow_checkpoint(path, _restamp(changed))
    with pytest.raises(ValueError):
        worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]} if target == "plan" else
                      {"action": "get_review", "review_id": review["review_id"]})


def test_missing_v02_review_is_not_defaulted_to_an_accepted_legacy_record(monkeypatch, tmp_path):
    worker, _ = _worker(tmp_path)
    review = _accept(worker, _prepare(worker))
    _runtime(monkeypatch)
    plan = _plan(worker, review)
    path = worker.root / "plans" / plan["plan_id"] / "plan.json"
    changed = json.loads(path.read_text(encoding="utf-8"))
    del changed["intent_review"]
    save_workflow_checkpoint(path, _restamp(changed))
    with pytest.raises(WorkflowAPIError):
        worker.handle({"action": "get_plan", "plan_id": plan["plan_id"]})


@pytest.mark.parametrize("downgrade", [True, False])
def test_legacy_envelopes_cannot_publish_an_unvalidated_review_field(monkeypatch, tmp_path, downgrade):
    worker, _ = _worker(tmp_path)
    _runtime(monkeypatch)
    if downgrade:
        review = _accept(worker, _prepare(worker))
        plan = _plan(worker, review)
    else:
        plan = worker.handle({"action": "plan", **_request(), "approve_provider_call": True})
    path = worker.root / "plans" / plan["plan_id"] / "plan.json"
    changed = json.loads(path.read_text(encoding="utf-8"))
    changed["schema"] = "aurora-workflow-api-plan/0.1"
    if not downgrade:
        changed["intent_review"] = {"review": {"forged": True}, "acceptance": {"forged": True}}
    save_workflow_checkpoint(path, _restamp(changed))
    for request in ({"action": "get_plan", "plan_id": plan["plan_id"]},
                    {"action": "export_evidence", "plan_id": plan["plan_id"]}):
        with pytest.raises(WorkflowAPIError) as error:
            worker.handle(request)
        assert error.value.code == "storage_invalid"


def test_suggestion_delegates_once_and_retains_only_unreviewed_candidate_without_plan(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    candidate = compile_workflow_intent_obligations(TASK, _rows(), interpretation_basis="model_candidate")
    extraction_data = _extraction_data(candidate)
    observed = []
    class Extractor:
        def __init__(self, catalog, runtime, *, model):
            assert catalog is worker.catalog and model == "fixture"
        def extract(self, task, **options):
            observed.append((task, options))
            return SimpleNamespace(status="candidate", obligations=candidate,
                to_dict=lambda: extraction_data)
    _runtime(monkeypatch)
    monkeypatch.setitem(sys.modules, "prism_sdk.workflow_intent_extractor", SimpleNamespace(
        LocalWorkflowIntentExtractor=Extractor, validate_workflow_intent_extraction=validate_workflow_intent_extraction))
    result = worker.handle({"action": "suggest_review", **_request(), "approve_provider_call": True})
    assert result["status"] == "review_required" and result["candidate"]["interpretation_basis"] == "model_candidate"
    assert result["origin"]["extraction"] == extraction_data
    assert result["acceptance"] is None and observed == [(TASK, {"approve_provider_call": True, "operation_ids": None})]
    assert calls == [] and not list((worker.root / "plans").iterdir())


@pytest.mark.parametrize("status", ["needs_clarification", "refused", "rejected"])
def test_noncandidate_suggestion_retains_outcome_but_creates_no_review_or_plan(monkeypatch, tmp_path, status):
    worker, calls = _worker(tmp_path)
    extractor = Mock(return_value=SimpleNamespace(extract=lambda *_args, **_kwargs: SimpleNamespace(
        status=status, obligations=None, to_dict=lambda: {"status": status, "raw_response": {"text": "untrusted"}})))
    _runtime(monkeypatch)
    monkeypatch.setitem(sys.modules, "prism_sdk.workflow_intent_extractor", SimpleNamespace(LocalWorkflowIntentExtractor=extractor))
    result = worker.handle({"action": "suggest_review", **_request(), "approve_provider_call": True})
    assert result["status"] == status and "review_id" not in result
    assert not list((worker.root / "reviews").iterdir()) and not list((worker.root / "plans").iterdir()) and calls == []


def test_suggestion_transport_failure_propagates_without_fallback_or_review(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    failure = Mock(side_effect=ProviderError("fixture transport failure"))
    extractor = Mock(return_value=SimpleNamespace(extract=failure))
    _runtime(monkeypatch)
    monkeypatch.setitem(sys.modules, "prism_sdk.workflow_intent_extractor", SimpleNamespace(LocalWorkflowIntentExtractor=extractor))
    with pytest.raises(ProviderError):
        worker.handle({"action": "suggest_review", **_request(), "approve_provider_call": True})
    assert failure.call_count == 1 and not list((worker.root / "reviews").iterdir()) and calls == []


def test_actual_worker_processes_retain_unreviewed_and_accepted_lexical_bindings_without_model(tmp_path):
    root = tmp_path / "owned"
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", PYTHONPATH=str(Path(__file__).resolve().parents[1]))
    def invoke(request):
        result = subprocess.run([sys.executable, "-m", "prism_sdk.workflow_api_worker", "--root", str(root)],
            input=json.dumps(request), text=True, capture_output=True, encoding="utf-8", env=environment,
            cwd=tmp_path, timeout=30)
        assert result.returncode == 0 and result.stderr == ""
        return json.loads(result.stdout)["data"]
    rows = [{"id": "read", "kind": "required_operation", "operation": "pubmed.search", "output": True,
             "span": {"start": 0, "end": 13, "text": "Read metadata"}}]
    review = invoke({"action": "prepare_review", **_request(), "obligations": rows})
    assert invoke({"action": "get_review", "review_id": review["review_id"]}) == review
    accepted = invoke({"action": "accept_review", "review_id": review["review_id"],
        "expected_review_digest": review["review_digest"], "accept_caller_declarations": True})
    assert invoke({"action": "get_review", "review_id": review["review_id"]}) == accepted
    assert accepted["candidate"]["interpretation_basis"] == "model_candidate"
    assert accepted["acceptance"]["admission"]["status"] == "ready_for_constrained_authoring"
    assert not list((root / "plans").iterdir()) and not list((root / "runs").iterdir())


def test_fresh_production_worker_revalidates_an_offline_seeded_v02_native_plan_and_review(tmp_path):
    worker = WorkflowAPIWorker(tmp_path / "owned")
    rows = [{"id": "read", "kind": "required_operation", "operation": "pubmed.search", "output": True,
             "span": {"start": 0, "end": 13, "text": "Read metadata"}}]
    review = _accept(worker, _prepare(worker, rows))
    native = compile_workflow_plan({"name": "Offline seeded fixture", "steps": [
        {"id": "pubmed", "operation": "pubmed.search", "arguments": {"per_specialty_limit": 1}, "depends_on": []}],
        "outputs": ["pubmed"]}, worker.catalog, task=TASK, requirements=review["acceptance"]["admission"]["requirements"])
    plan_id = "b" * 32
    record = {"schema": "aurora-workflow-api-plan/0.2", "plan_id": plan_id, "task": TASK,
              "planner": "local-model", "plan": native.to_dict(), "requirements": native.to_dict()["requirements"],
              "interpretation": None, "intent_review": worker._review(review["review_id"])}
    worker._write(worker._path("plans", plan_id, "plan.json", create=True), _restamp(record))
    environment = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", PYTHONPATH=str(Path(__file__).resolve().parents[1]))
    result = subprocess.run([sys.executable, "-m", "prism_sdk.workflow_api_worker", "--root", str(worker.root)],
        input=json.dumps({"action": "get_plan", "plan_id": plan_id}), text=True, capture_output=True,
        encoding="utf-8", env=environment, cwd=tmp_path, timeout=30)
    assert result.returncode == 0 and result.stderr == ""
    loaded = json.loads(result.stdout)["data"]
    assert loaded["plan"] == native.to_dict() and loaded["intent_review"] == review
    assert not list((worker.root / "runs").iterdir())


def test_real_extractor_with_inmemory_response_is_retained_unreviewed_by_suggestion(monkeypatch, tmp_path):
    worker, calls = _worker(tmp_path)
    rows = [{**row, "span": {"text": row["span"]["text"], "occurrence": 0}} for row in _rows()]
    requests = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("ollama", lambda request: requests.append(request) or
        json.dumps({"action": "candidate", "obligations": rows}))
    monkeypatch.setattr(runtime, "register_provider", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", lambda: runtime)
    result = worker.handle({"action": "suggest_review", **_request(), "approve_provider_call": True})
    assert len(requests) == 1 and result["status"] == "review_required"
    assert result["candidate"]["obligations"] == _rows() and result["acceptance"] is None
    extraction = result["origin"]["extraction"]
    assert extraction["provider_invocations"] == 1 and extraction["accepted"] is False
    assert extraction["raw_response"]["retention"] == "complete" and extraction["span_resolution"]
    assert worker.handle({"action": "get_review", "review_id": result["review_id"]}) == result
    assert calls == [] and not list((worker.root / "plans").iterdir())


def test_restamped_raw_selector_cannot_disagree_with_the_retained_native_candidate(tmp_path):
    worker, calls = _worker(tmp_path)
    candidate = compile_workflow_intent_obligations(TASK, _rows(), interpretation_basis="model_candidate")
    review = worker._save_review(worker._review_context(_request()), candidate.to_dict(),
        {"kind": "local_model_candidate", "extraction": _extraction_data(candidate)})
    path = worker.root / "reviews" / review["review_id"] / "review.json"
    record = json.loads(path.read_text(encoding="utf-8"))
    extraction = record["origin"]["extraction"]
    decision = json.loads(extraction["raw_response"]["text"])
    decision["obligations"][0]["span"]["text"] = "Read metadata"
    text = json.dumps(decision)
    extraction["raw_response"].update(text=text, text_digest=content_digest({"text": text}), utf8_bytes=len(text.encode("utf-8")))
    extraction.pop("extraction_digest")
    extraction["extraction_digest"] = content_digest(extraction)
    save_workflow_checkpoint(path, _restamp(record))
    with pytest.raises(ValueError):
        worker.handle({"action": "get_review", "review_id": review["review_id"]})
    assert calls == [] and not list((worker.root / "plans").iterdir())
