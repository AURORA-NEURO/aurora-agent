"""Caller declarations are enforced; these fixtures do not measure NLP extraction."""
from copy import deepcopy
import json

import pytest

from prism_sdk.authoring import canonical_json, content_digest
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_builder import (
    LocalWorkflowBuilder, WorkflowValidationError, compile_workflow_plan,
    execute_workflow_plan, validate_workflow_plan,
)
from prism_sdk.workflow_incremental_planner import IncrementalLocalWorkflowPlanner
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_intent_obligations import (
    WorkflowIntentAdmissionError, WorkflowIntentObligationError, WorkflowIntentObligations,
    admit_workflow_intent_obligations, build_workflow_with_obligations,
    compile_workflow_intent_obligations, validate_workflow_intent_admission,
    validate_workflow_intent_obligations,
)
from prism_sdk.workflow_requirements import WorkflowRequirementError, compile_workflow_requirements


TASK = "Read alpha metadata; preserve the payload; review the result."


def _span(text="Read alpha metadata", task=TASK):
    start = task.index(text)
    return {"start": start, "end": start + len(text), "text": text}


def _required(operation="fixture.read", *, identity="read", output=True):
    return {"id": identity, "kind": "required_operation", "span": _span(), "operation": operation, "output": output}


def _equals(value, *, path="/payload", identity="payload", operation="fixture.read"):
    return {"id": identity, "kind": "argument_equals", "span": _span("preserve the payload"),
            "operation": operation, "path": path, "value": value}


def _unresolved():
    return {"id": "unmapped", "kind": "unresolved_action", "span": _span("review the result"), "reason": "No exact action mapping supplied"}


def _catalog(calls=None, *, nested=False, constant="metadata"):
    calls = [] if calls is None else calls
    schema = {"type": "object", "properties": {
        "text": {"type": "string"}, "mode": {"type": "string", "const": constant},
        "limit": {"type": "integer", "default": 5}, "payload": {}},
        "required": ["text", "mode"], "additionalProperties": False}
    if nested:
        schema["properties"]["options"] = {"type": "object", "properties": {"mode": {"const": "summary"}},
                                              "required": ["mode"], "additionalProperties": False}
    catalog = WorkflowOperationCatalog()
    for identity in ("fixture.read", "fixture.review", "fixture.extra", "fixture.write"):
        def execute(arguments, *, _identity=identity, **_kwargs):
            calls.append((_identity, deepcopy(arguments)))
            return WorkflowOperationResult(dict(arguments), {"fixture": True})
        catalog.register(WorkflowOperation(identity, "Local fixture", schema, schema, "local",
            implementation="local", read_only=identity != "fixture.write"), execute)
    return catalog


def _proposal(*, payload=None, nested=False):
    arguments = {"text": "alpha", "mode": "metadata", "payload": payload}
    if nested:
        arguments["options"] = {"mode": "summary"}
    return {"name": "Read", "steps": [{"id": "read", "operation": "fixture.read",
        "arguments": arguments, "depends_on": []}], "outputs": ["read"]}


def _builder(catalog, mode, responses):
    requests = []
    runtime = LLMRuntime()
    def handler(request):
        requests.append(request)
        return json.dumps(responses.pop(0))
    runtime.register_in_memory_provider("fixture", handler)
    cls = LocalWorkflowBuilder if mode == "whole" else IncrementalLocalWorkflowPlanner
    return cls(catalog, runtime, model="fixture", provider="fixture"), requests


def _responses(mode, proposal):
    if mode == "whole":
        return [proposal]
    return [{"action": "append", "operation": "fixture.read"},
            {"action": "arguments", "arguments": proposal["steps"][0]["arguments"], "additional_dependencies": []},
            {"action": "finish", "name": "Read", "outputs": ["s1"]}]


def _restamp(value, key):
    value.pop(key, None)
    value[key] = content_digest(value)
    return value


@pytest.mark.parametrize("mode", ["whole", "incremental"])
@pytest.mark.parametrize("blocked,code", [
    (_required("absent.archive", identity="archive"), "operation_unregistered"),
    (_required("fixture.write", identity="write"), "operation_not_read_only"),
    (_unresolved(), "unresolved_action"),
])
def test_mandatory_unavailable_or_unresolved_actions_block_before_prepared_reader_model_response(mode, blocked, code):
    calls = []
    catalog = _catalog(calls)
    builder, requests = _builder(catalog, mode, _responses(mode, _proposal()))
    obligations = compile_workflow_intent_obligations(TASK, [_required(), blocked])
    with pytest.raises(WorkflowIntentAdmissionError) as captured:
        build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    receipt = captured.value.admission.to_dict()
    assert code in {row["code"] for row in receipt["blocking_issues"]}
    assert receipt["requirements"] is None
    assert receipt["obligations"] == obligations.to_dict()
    assert requests == [] and calls == []
    assert receipt["provider_calls"] == receipt["source_dispatches"] == 0


@pytest.mark.parametrize("mode", ["whole", "incremental"])
def test_model_candidate_remains_unreviewed_even_when_every_listed_operation_is_supported(mode):
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_required()], interpretation_basis="model_candidate")
    builder, requests = _builder(catalog, mode, _responses(mode, _proposal()))
    with pytest.raises(WorkflowIntentAdmissionError) as captured:
        build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    receipt = captured.value.admission.to_dict()
    assert receipt["status"] == "needs_clarification"
    assert receipt["blocking_issues"] == [{"code": "unreviewed_model_candidate"}]
    assert receipt["task_interpretation_completeness"] == "unassessed" and requests == []


def test_interpretation_basis_is_a_host_assertion_not_authenticated_model_or_human_provenance():
    catalog = _catalog()
    candidate = compile_workflow_intent_obligations(TASK, [_required()], interpretation_basis="model_candidate")
    assert admit_workflow_intent_obligations(TASK, candidate, catalog).status == "needs_clarification"
    relabeled = candidate.to_dict()
    relabeled["interpretation_basis"] = "caller_supplied"
    relabeled = _restamp(relabeled, "obligations_digest")
    admission = admit_workflow_intent_obligations(TASK, relabeled, catalog).to_dict()
    assert admission["status"] == "ready_for_constrained_authoring"
    assert admission["declaration_provenance"] == "caller_asserted_not_authenticated;host_must_classify_raw_model_output"
    assert admission["task_interpretation_completeness"] == "unassessed"


@pytest.mark.parametrize("mode", ["whole", "incremental"])
def test_conflicting_direct_const_cannot_be_silently_substituted_by_a_model(mode):
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_equals("fulltext", path="/mode")])
    builder, requests = _builder(catalog, mode, _responses(mode, _proposal()))
    with pytest.raises(WorkflowIntentAdmissionError) as captured:
        build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    receipt = captured.value.admission.to_dict()
    assert receipt["blocking_issues"] == [{"code": "literal_schema_conflict", "obligation_id": "payload",
                                           "operation": "fixture.read", "path": "/mode"}]
    assert receipt["literal_schema_preflight"]["checks"] == [{"obligation_id": "payload", "state": "definite_conflict"}]
    assert receipt["general_satisfiability"] == "not_assessed" and requests == []


def test_nested_schema_compatibility_stays_unassessed_but_substitution_still_fails_native_contract():
    catalog = _catalog(nested=True)
    obligations = compile_workflow_intent_obligations(TASK, [_equals("verbatim", path="/options/mode")])
    admission = admit_workflow_intent_obligations(TASK, obligations, catalog)
    assert admission.status == "ready_for_constrained_authoring"
    assert admission.to_dict()["literal_schema_preflight"]["checks"] == [{"obligation_id": "payload", "state": "not_assessed"}]
    builder, requests = _builder(catalog, "whole", [_proposal(nested=True)])
    with pytest.raises(WorkflowValidationError):
        build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    assert len(requests) == 1
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(_proposal(nested=True), catalog, task=TASK, requirements=admission.requirements)


@pytest.mark.parametrize("expected,substitute", [
    (True, 1), (1, 1.0), (False, 0), ({"n": True}, {"n": 1}), ([1], [1.0]), (None, "null"),
])
def test_canonical_literal_types_remain_distinct_in_derived_contracts(expected, substitute):
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_equals(expected)])
    admission = admit_workflow_intent_obligations(TASK, obligations, catalog)
    rule = admission.requirements.to_dict()["argument_rules"][0]
    assert canonical_json(rule["value"]) == canonical_json(expected)
    correct = compile_workflow_plan(_proposal(payload=expected), catalog, task=TASK, requirements=admission.requirements)
    assert validate_workflow_plan(correct, catalog) == correct
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(_proposal(payload=substitute), catalog, task=TASK, requirements=admission.requirements)


def test_direct_enum_checks_compare_canonical_values_without_claiming_general_satisfiability():
    catalog = WorkflowOperationCatalog()
    schema = {"type": "object", "properties": {"payload": {"enum": [True, 1.0]}}, "additionalProperties": False}
    catalog.register(WorkflowOperation("fixture.read", "Fixture", schema, schema, "local", implementation="local"), lambda *_args, **_kwargs: None)
    blocked = admit_workflow_intent_obligations(TASK, compile_workflow_intent_obligations(TASK, [_equals(1)]), catalog)
    admitted = admit_workflow_intent_obligations(TASK, compile_workflow_intent_obligations(TASK, [_equals(True)]), catalog)
    assert blocked.status == "blocked" and admitted.status == "ready_for_constrained_authoring"
    assert admitted.to_dict()["general_satisfiability"] == "not_assessed"
    assert admitted.to_dict()["literal_schema_preflight"]["checks"][0]["state"] == "no_const_enum_conflict"


@pytest.mark.parametrize("mode", ["whole", "incremental"])
def test_existing_builders_receive_immutable_lowered_obligations_and_dispatch_no_sources(mode):
    calls = []
    catalog = _catalog(calls)
    obligations = compile_workflow_intent_obligations(TASK, [_required(), _equals(["retained"])])
    builder, requests = _builder(catalog, mode, _responses(mode, _proposal(payload=["retained"])))
    result = build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    plan = result if mode == "whole" else result.plan
    assert plan.to_dict()["requirements"]["argument_rules"] == [
        {"operation": "fixture.read", "path": "/payload", "comparison": "equals", "value": ["retained"]}]
    assert plan.to_dict()["requirements"]["output_operations"] == ["fixture.read"]
    assert calls == [] and len(requests) == (1 if mode == "whole" else 3)


def test_admitted_obligations_conjoin_caller_constraints_and_survive_edit_and_checkpoint_replay():
    calls = []
    catalog = _catalog(calls)
    rows = [_required(output=False), _equals("alpha", path="/text"),
            _required("fixture.review", identity="review", output=True)]
    obligations = compile_workflow_intent_obligations(TASK, rows)
    caller = compile_workflow_requirements(TASK, forbidden_operations=["fixture.extra"], max_steps=2,
        max_calls_by_operation={"fixture.read": 1, "fixture.review": 1},
        data_links=[{"source_operation": "fixture.read", "target_operation": "fixture.review", "source_pointer": "", "target_path": ""}])
    admission = admit_workflow_intent_obligations(TASK, obligations, catalog, requirements=caller, max_steps=8)
    contract = admission.requirements.to_dict()
    assert contract["max_steps"] == 2 and contract["data_links"] == caller.to_dict()["data_links"]
    assert contract["forbidden_operations"] == ["fixture.extra"]
    proposal = _proposal()
    proposal["steps"].append({"id": "review", "operation": "fixture.review", "arguments": {"$ref": "read#"}, "depends_on": ["read"]})
    proposal["outputs"] = ["review"]
    plan = compile_workflow_plan(proposal, catalog, task=TASK, requirements=admission.requirements)
    changed = deepcopy(proposal)
    changed["steps"][0]["arguments"]["text"] = "beta"
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(changed, catalog, task=TASK, requirements=plan.to_dict()["requirements"])
    changed = deepcopy(proposal)
    changed["steps"] = changed["steps"][:1]
    changed["outputs"] = ["read"]
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(changed, catalog, task=TASK, requirements=plan.to_dict()["requirements"])
    paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1)
    assert paused.status == "paused" and len(calls) == 1
    complete = execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint)
    assert complete.status == "completed" and [operation for operation, _ in calls] == ["fixture.read", "fixture.review"]
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=complete.checkpoint).status == "completed"
    assert len(calls) == 2


@pytest.mark.parametrize("caller", [
    compile_workflow_requirements(TASK, forbidden_operations=["fixture.read"]),
    compile_workflow_requirements(TASK, argument_rules=[{"operation": "fixture.read", "path": "/payload", "comparison": "equals", "value": 1}]),
])
def test_conflicting_caller_requirements_are_not_replaced(caller):
    obligations = compile_workflow_intent_obligations(TASK, [_required(), _equals(True)])
    admission = admit_workflow_intent_obligations(TASK, obligations, _catalog(), requirements=caller)
    assert admission.status == "blocked" and admission.requirements is None
    assert admission.to_dict()["blocking_issues"] == [{"code": "requirements_conflict"}]


def test_conflicting_equalities_within_one_obligation_document_block():
    obligations = compile_workflow_intent_obligations(TASK, [_equals(True), _equals(1, identity="second")])
    admission = admit_workflow_intent_obligations(TASK, obligations, _catalog())
    assert admission.status == "blocked" and admission.requirements is None


def test_repeated_required_operations_mean_presence_and_false_does_not_forbid_output():
    rows = [_required(output=False), _required(identity="again", output=False)]
    obligations = compile_workflow_intent_obligations(TASK, rows)
    catalog = _catalog()
    admission = admit_workflow_intent_obligations(TASK, obligations, catalog)
    assert admission.requirements.to_dict()["required_operations"] == ["fixture.read"]
    assert admission.requirements.to_dict()["output_operations"] == []
    assert len(admission.to_dict()["lowering"]) == 2
    plan = compile_workflow_plan(_proposal(), catalog, task=TASK, requirements=admission.requirements)
    assert len(plan.to_dict()["steps"]) == 1 and plan.to_dict()["outputs"] == ["read"]


def test_public_sdk_exports_the_opt_in_contract_without_replacing_existing_builders():
    import prism_sdk
    assert prism_sdk.compile_workflow_intent_obligations is compile_workflow_intent_obligations
    assert prism_sdk.build_workflow_with_obligations is build_workflow_with_obligations
    assert prism_sdk.LocalWorkflowBuilder is LocalWorkflowBuilder


def test_registered_selection_is_not_a_claim_about_actual_model_visibility():
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_required()])
    receipt = admit_workflow_intent_obligations(TASK, obligations, catalog).to_dict()
    assert "fixture.write" in receipt["operation_selection"]["operation_ids"]
    assert receipt["operation_selection"]["scope"] == "registered_or_caller_selection_before_builder_filtering"
    assert receipt["actual_model_visibility"] == "unassessed"
    requests = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: requests.append(request) or json.dumps(_proposal()))
    builder = LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture", max_visible_operations=1)
    result = build_workflow_with_obligations(builder, TASK, obligations=obligations, approve_provider_call=True)
    assert result.to_dict()["requirements"]["required_operations"] == ["fixture.read"]
    offered = json.loads(requests[0].messages[0]["content"])["operations"]
    assert [operation["operation_id"] for operation in offered] == ["fixture.read"]


@pytest.mark.parametrize("change", ["task", "span", "catalogue", "visibility", "caller", "max_steps"])
def test_admission_receipts_bind_exact_task_spans_catalogue_visibility_and_caller_contract(change):
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_required()])
    admission = admit_workflow_intent_obligations(TASK, obligations, catalog)
    kwargs = {"task": TASK, "obligations": obligations, "catalog": catalog}
    assert validate_workflow_intent_admission(admission, **kwargs) == admission
    if change == "task":
        kwargs["task"] = TASK + " "
    elif change == "span":
        altered = obligations.to_dict()
        altered["obligations"][0]["span"] = _span("review the result")
        kwargs["obligations"] = _restamp(altered, "obligations_digest")
    elif change == "catalogue":
        kwargs["catalog"] = _catalog(constant="catalogue")
    elif change == "visibility":
        kwargs["operation_ids"] = ["fixture.read"]
    elif change == "caller":
        kwargs["requirements"] = compile_workflow_requirements(TASK, max_steps=2)
    else:
        kwargs["max_steps"] = 2
    with pytest.raises(WorkflowIntentObligationError):
        validate_workflow_intent_admission(admission, **kwargs)


def test_rehashed_receipt_cannot_approve_an_unresolved_action_or_change_lowered_requirements():
    catalog = _catalog()
    obligations = compile_workflow_intent_obligations(TASK, [_required(), _unresolved()])
    forged = admit_workflow_intent_obligations(TASK, obligations, catalog).to_dict()
    forged["status"] = "ready_for_constrained_authoring"
    forged["blocking_issues"] = []
    forged["requirements"] = compile_workflow_requirements(TASK, required_operations=["fixture.read"]).to_dict()
    _restamp(forged, "admission_digest")
    with pytest.raises(WorkflowIntentObligationError):
        validate_workflow_intent_admission(forged, task=TASK, obligations=obligations, catalog=catalog)


def test_hidden_required_action_blocks_and_all_unresolved_findings_are_retained():
    rows = [_required(), _required("absent.archive", identity="archive"), _unresolved()]
    obligations = compile_workflow_intent_obligations(TASK, rows)
    admission = admit_workflow_intent_obligations(TASK, obligations, _catalog(), operation_ids=["fixture.review"])
    assert {row["code"] for row in admission.to_dict()["blocking_issues"]} == {
        "operation_outside_selection", "operation_unregistered", "unresolved_action"}
    assert admission.to_dict()["obligations"]["obligations"] == rows


def test_character_spans_preserve_non_ascii_text_without_reinterpreting_utf8_offsets():
    task = "Évidence: read alpha."
    row = _required()
    row["span"] = _span("read alpha", task)
    obligations = compile_workflow_intent_obligations(task, [row])
    assert validate_workflow_intent_obligations(obligations, task=task) == obligations
    assert row["span"]["start"] != len(task[:row["span"]["start"]].encode("utf-8"))
    altered = obligations.to_dict()
    altered["obligations"][0]["span"]["start"] += 1
    with pytest.raises(WorkflowIntentObligationError):
        validate_workflow_intent_obligations(_restamp(altered, "obligations_digest"), task=task)


@pytest.mark.parametrize("mutation", ["parser_basis", "human_flag", "span_bool", "duplicate_id", "extra_field", "integer_key", "ref_literal", "nonfinite", "oversize"])
def test_malformed_or_self_asserted_authority_documents_are_rejected(mutation):
    rows = [_required()]
    basis = "caller_supplied"
    if mutation == "parser_basis":
        basis = "verified_parser"
    elif mutation == "human_flag":
        rows[0]["human_reviewed"] = True
    elif mutation == "span_bool":
        rows[0]["span"]["start"] = False
    elif mutation == "duplicate_id":
        rows.append(deepcopy(rows[0]))
    elif mutation == "extra_field":
        rows[0]["essential"] = False
    else:
        values = {"integer_key": {1: "value"}, "ref_literal": {"nested": {"$ref": "s1#"}},
                  "nonfinite": float("nan"), "oversize": "x" * 128_000}
        rows = [_equals(values[mutation])]
    with pytest.raises(WorkflowIntentObligationError):
        compile_workflow_intent_obligations(TASK, rows, interpretation_basis=basis)


def test_duplicate_json_keys_are_not_silently_collapsed():
    descriptor = compile_workflow_intent_obligations(TASK, [_required()])
    encoded = descriptor._encoded.replace('"schema":', '"schema":"discarded","schema":', 1)
    with pytest.raises(WorkflowIntentObligationError):
        validate_workflow_intent_obligations(WorkflowIntentObligations(encoded), task=TASK)


@pytest.mark.parametrize("mode", ["whole", "incremental"])
@pytest.mark.parametrize("with_contract", [False, True])
def test_absent_feature_preserves_existing_builder_result_bytes_and_request_shape(mode, with_contract):
    catalog = _catalog()
    options = {"approve_provider_call": True}
    if with_contract:
        options["requirements"] = compile_workflow_requirements(TASK, required_operations=["fixture.read"])
    direct, direct_requests = _builder(catalog, mode, _responses(mode, _proposal()))
    wrapped, wrapped_requests = _builder(catalog, mode, _responses(mode, _proposal()))
    result1 = direct.build(TASK, **options)
    result2 = build_workflow_with_obligations(wrapped, TASK, **options)
    plan1 = result1 if mode == "whole" else result1.plan
    plan2 = result2 if mode == "whole" else result2.plan
    assert plan1._encoded == plan2._encoded
    assert [request.messages for request in direct_requests] == [request.messages for request in wrapped_requests]
    assert [request.response_schema for request in direct_requests] == [request.response_schema for request in wrapped_requests]
    assert plan1.to_dict()["schema"] == ("aurora-executable-workflow-plan/0.2" if with_contract else "aurora-executable-workflow-plan/0.1")
