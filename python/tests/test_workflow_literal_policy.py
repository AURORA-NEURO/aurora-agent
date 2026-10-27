"""Current scalar conflicts gate new authoring without rewriting historical evidence."""
import base64
from copy import deepcopy
import json
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock

import pytest

from prism_sdk import llm_runtime
from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.tooling import ToolValidationIssue
from prism_sdk.workflow_api_worker import WorkflowAPIWorker
from prism_sdk.workflow_builder import LocalWorkflowBuilder, WorkflowValidationError
from prism_sdk.workflow_incremental_planner import IncrementalLocalWorkflowPlanner
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog
from prism_sdk.workflow_intent_obligations import (
    admit_workflow_intent_obligations, build_workflow_with_obligations,
    compile_workflow_intent_obligations, validate_workflow_intent_admission,
)
from prism_sdk.workflow_literal_policy import (
    WorkflowLiteralPolicyError, assess_workflow_literal_policy, validate_workflow_literal_policy_report,
)

TASK = "Read metadata; keep the exact literal."


def _catalog(leaf=None, *, root=None, name="page_size"):
    schema = root if root is not None else {"type": "object", "properties": {
        name: {"type": "integer", "minimum": 1, "maximum": 100, "default": 25} if leaf is None else leaf},
        "additionalProperties": False}
    catalog = WorkflowOperationCatalog()
    catalog.register(WorkflowOperation("fixture.read", "Synthetic metadata", schema,
        {"type": "object", "properties": {"records": {"type": "array"}}}, "fixture", implementation="local"),
        lambda *_args, **_kwargs: pytest.fail("literal policy cannot execute a source"))
    return catalog


def _obligations(value=150, *, path="/page_size", operation="fixture.read"):
    text = "keep the exact literal"
    return compile_workflow_intent_obligations(TASK, [{"id": "literal", "kind": "argument_equals",
        "operation": operation, "path": path, "value": value,
        "span": {"text": text, "start": TASK.index(text), "end": TASK.index(text) + len(text)}}])


def _check(value=150, *, leaf=None, root=None, path="/page_size", name="page_size"):
    catalog, obligations = _catalog(leaf, root=root, name=name), _obligations(value, path=path)
    return assess_workflow_literal_policy(TASK, obligations, catalog).to_dict(), catalog, obligations


@pytest.mark.parametrize("leaf,value,conflict", [
    ({"type": "integer", "maximum": 100}, 150, "maximum"),
    ({"type": "integer", "maximum": 100}, 100, None),
    ({"type": "number", "minimum": 1.5}, 1, "minimum"),
    ({"type": "number", "minimum": 1.5}, 1.5, None),
    ({"type": "integer", "exclusiveMaximum": 100}, 100, "exclusiveMaximum"),
    ({"type": "integer", "exclusiveMinimum": 1}, 1, "exclusiveMinimum"),
    ({"type": "integer"}, True, "type"),
    ({"type": "integer"}, 1.0, "type"),
    ({"type": "number"}, False, "type"),
    ({"type": "number"}, 1, None),
    ({"type": "string"}, " unchanged ", None),
    ({"type": "string"}, [], "type"),
    ({"type": "boolean"}, True, None),
    ({"type": "boolean"}, 1, "type"),
    ({"type": "null"}, None, None),
    ({"type": "null"}, "null", "type"),
    ({"type": "integer", "maximum": 2 ** 64 - 1}, 2 ** 64 - 2, None),
])
def test_only_evaluated_scalar_constraints_determine_conflict_without_coercion(leaf, value, conflict):
    report, _, obligations = _check(value, leaf=leaf)
    check = report["checks"][0]
    assert check["state"] == ("definite_scalar_conflict" if conflict else "no_evaluated_violation")
    assert report["status"] == ("current_policy_conflict" if conflict else "no_evaluated_violation")
    assert [row["code"] for row in check["diagnostics"]] == ([conflict] if conflict else [])
    assert obligations.to_dict()["obligations"][0]["value"] == value
    assert check["literal_digest"] == content_digest(value)
    assert report["whole_call_validity"] == report["general_satisfiability"] == "not_assessed"
    assert report["provider_calls"] == report["source_dispatches"] == 0 and report["plan_created"] is False


def test_optional_declared_literal_conflict_is_conditional_and_does_not_rewrite_legacy_admission_or_defaults():
    report, catalog, obligations = _check()
    legacy = admit_workflow_intent_obligations(TASK, obligations, catalog)
    assert legacy.status == "ready_for_constrained_authoring"
    assert legacy.to_dict()["literal_schema_preflight"] == {"scope": "direct_top_level_const_and_enum_only",
        "checks": [{"obligation_id": "literal", "state": "not_assessed"}]}
    assert validate_workflow_intent_admission(legacy, task=TASK, obligations=obligations, catalog=catalog) == legacy
    assert report["status"] == "current_policy_conflict"
    assert report["checks"][0]["conditional_on_obligation_required_presence"] is True
    assert "default" not in report["checks"][0]["evaluated_schema"]
    assert legacy.requirements.to_dict()["argument_rules"][0]["value"] == 150


@pytest.mark.parametrize("leaf", [
    {}, True, False, {"type": ["integer", "null"]}, {"type": "object"}, {"type": "array"},
    {"type": "integer", "maximum": "100"}, {"type": "integer", "minimum": True},
    {"type": "integer", "enum": "invalid"}, {"type": "integer", "enum": []},
    {"type": "string", "maximum": 10}, {"type": "string", "pattern": "^[a-z]+$"},
    {"type": "integer", "anyOf": [{"maximum": 100}, {"minimum": 200}]},
])
def test_unsupported_ambiguous_or_malformed_leaf_is_unassessed_not_false_conflict(leaf):
    report, _, _ = _check(150, leaf=leaf)
    assert report["checks"][0]["state"] == "not_assessed"
    assert report["counts"]["definite_scalar_conflict"] == 0
    assert report["status"] == "not_assessed"


@pytest.mark.parametrize("root,path", [
    ({"type": "object", "properties": {}, "additionalProperties": True}, "/page_size"),
    ({"type": "object", "properties": {"options": {"type": "object", "properties": {
        "page_size": {"type": "integer", "maximum": 100}}}}}, "/options/page_size"),
    ({"type": "object", "properties": {"page_size": {"type": "integer", "maximum": 100}},
        "anyOf": [{}, {}]}, "/page_size"),
    ({"type": "object", "properties": {"page_size": {"type": "integer"}}, "required": "page_size"}, "/page_size"),
])
def test_open_nested_alternative_or_malformed_root_paths_are_not_resolved_into_leaf_proof(root, path):
    report, _, _ = _check(root=root, path=path)
    assert report["checks"][0]["state"] == "not_assessed"


def test_escaped_direct_property_names_bind_exact_schema_pointer_without_traversing_nested_paths():
    report, _, _ = _check(name="a/b~c", path="/a~1b~0c")
    assert report["checks"][0]["leaf_schema_pointer"] == "/properties/a~1b~0c"
    assert report["checks"][0]["state"] == "definite_scalar_conflict"


def test_const_enum_remain_exact_legacy_checks_separate_from_the_type_projection():
    report, catalog, obligations = _check(1, leaf={"type": "integer", "const": 2, "enum": [2]})
    assert report["checks"][0]["state"] == "no_evaluated_violation"
    assert report["checks"][0]["evaluated_schema"] == {"type": "integer"}
    assert admit_workflow_intent_obligations(TASK, obligations, catalog).status == "blocked"


def test_native_warnings_and_unexpected_diagnostics_remain_observations_not_caller_conflicts(monkeypatch):
    from prism_sdk import workflow_literal_policy as policy
    observed = SimpleNamespace(issues=(ToolValidationIssue("$.value", "future_constraint", "unhandled"),),
        warnings=(ToolValidationIssue("$.value", "unsupported_schema_keyword", "unchecked"),))
    monkeypatch.setattr(policy.ToolCatalogue, "from_definitions", lambda *_args: SimpleNamespace(validate=lambda *_a: observed))
    report, _, _ = _check()
    check = report["checks"][0]
    assert check["state"] == "not_assessed" and check["diagnostics"][0]["code"] == "future_constraint"
    assert check["warnings"][0]["code"] == "unsupported_schema_keyword"


@pytest.mark.parametrize("field", ["status", "task_digest", "obligations_digest", "catalog_digest", "policy_digest", "checks", "counts"])
def test_restamping_does_not_make_report_binding_or_findings_valid(field):
    report, catalog, obligations = _check()
    assert validate_workflow_literal_policy_report(report, task=TASK, obligations=obligations, catalog=catalog).to_dict() == report
    report[field] = [] if field == "checks" else {} if field == "counts" else "changed"
    report["report_digest"] = content_digest({key: child for key, child in report.items() if key != "report_digest"})
    with pytest.raises(WorkflowValidationError, match="exact bindings"):
        validate_workflow_literal_policy_report(report, task=TASK, obligations=obligations, catalog=catalog)


def test_catalogue_mutation_during_assessment_refuses_before_returning_a_report(monkeypatch):
    catalog, obligations = _catalog(), _obligations()
    resolve = catalog.resolve
    def changed(name):
        result = resolve(name)
        catalog.register(WorkflowOperation("fixture.extra", "Extra", {"type": "object"}, {"type": "object"}, "fixture"), lambda *_a, **_k: None)
        return result
    monkeypatch.setattr(catalog, "resolve", changed)
    with pytest.raises(WorkflowValidationError, match="catalogue changed"):
        assess_workflow_literal_policy(TASK, obligations, catalog)


def test_no_literal_rows_and_unregistered_operations_never_form_a_vacuous_validity_claim():
    catalog = _catalog()
    required = compile_workflow_intent_obligations(TASK, [{"id": "read", "kind": "required_operation",
        "operation": "fixture.read", "output": True, "span": {"start": 0, "end": 13, "text": "Read metadata"}}])
    empty = assess_workflow_literal_policy(TASK, required, catalog).to_dict()
    assert empty["status"] == "not_assessed" and empty["no_literal_obligations"] is True
    assert all(count == 0 for count in empty["counts"].values())
    absent = assess_workflow_literal_policy(TASK, _obligations(operation="unregistered.read"), catalog).to_dict()
    assert absent["checks"][0]["state"] == "not_assessed" and absent["checks"][0]["reason"] == "operation_unregistered"
    assert absent["status"] == "not_assessed"


@pytest.mark.parametrize("literal,expected", [(50, "partially_assessed"), (150, "current_policy_conflict")])
def test_mixed_assessed_and_unknown_findings_keep_real_assessment_and_conflicts_distinct(literal, expected):
    root = {"type": "object", "properties": {"known": {"type": "integer", "maximum": 100}, "unknown": {}}}
    catalog = _catalog(root=root)
    known = _obligations(literal, path="/known").to_dict()["obligations"][0]
    unknown = {**deepcopy(known), "id": "unknown", "path": "/unknown"}
    declarations = compile_workflow_intent_obligations(TASK, [known, unknown])
    report = assess_workflow_literal_policy(TASK, declarations, catalog).to_dict()
    assert report["status"] == expected and report["counts"]["not_assessed"] == 1
    assert report["counts"]["no_evaluated_violation"] == (1 if literal == 50 else 0)
    assert report["counts"]["definite_scalar_conflict"] == (1 if literal == 150 else 0)
    assert report["whole_call_validity"] == "not_assessed"


def test_changed_policy_definition_is_a_validation_failure_not_a_caller_literal_conflict(monkeypatch):
    from prism_sdk import workflow_literal_policy as policy
    monkeypatch.setitem(policy._POLICY, "defaults", "changed")
    with pytest.raises(WorkflowValidationError, match="policy definition changed"):
        assess_workflow_literal_policy(TASK, _obligations(), _catalog())


@pytest.mark.parametrize("builder_type", [LocalWorkflowBuilder, IncrementalLocalWorkflowPlanner])
def test_both_authoring_wrappers_block_new_numeric_conflicts_before_any_provider_call(builder_type):
    catalog, obligations, calls = _catalog(), _obligations(), []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: calls.append(request) or pytest.fail("must not invoke"))
    builder = builder_type(catalog, runtime, model="fixture", provider="fixture")
    with pytest.raises(WorkflowLiteralPolicyError) as caught:
        build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    assert caught.value.report.to_dict()["status"] == "current_policy_conflict" and calls == []


def _legacy():
    return json.loads((Path(__file__).parent / "fixtures/workflow_literal_policy_legacy_review_0_1.json").read_text(encoding="utf-8"))


def _restore(tmp_path, case):
    fixture = _legacy()
    descriptor = fixture["operation"]
    catalog = WorkflowOperationCatalog()
    catalog.register(WorkflowOperation(**{key: value for key, value in descriptor.items() if key != "schema"}),
        lambda *_a, **_k: pytest.fail("historical review cannot execute source"))
    assert catalog.catalog_digest == fixture["catalog_digest"]
    worker = WorkflowAPIWorker(tmp_path / "application", model="fixture")
    worker.catalog = catalog
    directory = worker.root / "reviews" / case["accepted"]["review_id"]
    directory.mkdir()
    raw = {name: base64.b64decode(value, validate=True) for name, value in case["native_files_base64"].items()}
    for name, value in raw.items():
        (directory / name).write_bytes(value)
    return worker, directory, raw


@pytest.mark.parametrize("case_index", [0, 1], ids=["old-numeric-conflict", "old-safe-literal"])
def test_actual_old_durable_review_and_idempotent_acceptance_replay_exact_bytes(case_index, tmp_path):
    fixture = _legacy()
    assert fixture["captured_source_head"] == "25c6816b4012c7c4152c2e62393e955bc0883a82"
    case = fixture["cases"][case_index]
    worker, directory, raw = _restore(tmp_path, case)
    loaded = worker.handle({"action": "get_review", "review_id": case["accepted"]["review_id"]})
    assert loaded == case["accepted"]
    assert loaded["acceptance"]["admission"]["status"] == "ready_for_constrained_authoring"
    assert loaded["acceptance"]["admission"]["schema"] == "aurora-workflow-intent-admission/0.1"
    assert worker.handle({"action": "accept_review", "review_id": loaded["review_id"],
        "expected_review_digest": loaded["review_digest"], "accept_caller_declarations": True}) == loaded
    assert {name: (directory / name).read_bytes() for name in raw} == raw


def test_old_numeric_acceptance_stays_readable_but_current_gate_returns_nonplan_before_provider_construction(tmp_path, monkeypatch):
    case = _legacy()["cases"][0]
    worker, directory, raw = _restore(tmp_path, case)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", Mock(side_effect=AssertionError("no provider construction")))
    result = worker.handle({"action": "plan", "task": case["task"], "planner": "local-model", "max_steps": 1,
        "approve_provider_call": True, "intent_review_id": case["accepted"]["review_id"],
        "expected_acceptance_digest": case["accepted"]["acceptance"]["acceptance_digest"]})
    assert result["status"] == "current_policy_conflict" and "plan" not in result and result["source_dispatch"] is False
    assert result["interpretation"]["checks"][0]["diagnostics"][0]["code"] == "maximum"
    assert result["intent_review"] == case["accepted"]
    assert {name: (directory / name).read_bytes() for name in raw} == raw
    assert not list((worker.root / "plans").iterdir()) and not list((worker.root / "runs").iterdir())


def test_old_safe_acceptance_can_author_new_plan_without_rewriting_old_events(tmp_path, monkeypatch):
    case = _legacy()["cases"][1]
    worker, directory, raw = _restore(tmp_path, case)
    runtime, calls = LLMRuntime(), []
    proposal = {"name": "Synthetic read", "steps": [{"id": "read", "operation": "fixture.read",
        "arguments": {"page_size": 50}, "depends_on": []}], "outputs": ["read"]}
    runtime.register_in_memory_provider("ollama", lambda request: calls.append(request) or json.dumps(proposal))
    monkeypatch.setattr(runtime, "register_provider", lambda *_a, **_k: None)
    monkeypatch.setattr(llm_runtime, "LLMRuntime", lambda: runtime)
    result = worker.handle({"action": "plan", "task": case["task"], "planner": "local-model", "max_steps": 1,
        "approve_provider_call": True, "intent_review_id": case["accepted"]["review_id"],
        "expected_acceptance_digest": case["accepted"]["acceptance"]["acceptance_digest"]})
    assert result["status"] == "planned" and len(calls) == 1
    assert {name: (directory / name).read_bytes() for name in raw} == raw
    assert not list((worker.root / "runs").iterdir())
