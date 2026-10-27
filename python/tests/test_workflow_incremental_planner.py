"""Incremental syntax removes identifier freedom without supplying a workflow graph."""
import json
from dataclasses import replace

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderError
from prism_sdk.workflow_builder import WorkflowValidationError, compile_workflow_plan, validate_workflow_plan
from prism_sdk.workflow_incremental_planner import (
    IncrementalLocalWorkflowPlanner, IncrementalWorkflowPlanningError,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog
from prism_sdk.workflow_requirements import audit_workflow_requirements, compile_workflow_requirements


def _catalog(input_schema=None):
    catalog = WorkflowOperationCatalog()
    schema = {"type": "object", "properties": {"text": {"type": "string", "minLength": 1}},
              "required": ["text"], "additionalProperties": False}
    output = {"type": "object", "properties": {"text": {"type": "string"}},
              "required": ["text"], "additionalProperties": False}
    def never_dispatch(*_args, **_kwargs):
        pytest.fail("incremental planning must never call an operation executor")
    for operation in ("fixture.alpha", "fixture.beta"):
        catalog.register(WorkflowOperation(operation, "Synthetic text metadata transformation",
            input_schema or schema, output, "fixture", implementation="local"), never_dispatch)
    return catalog


def _append(operation="fixture.alpha"):
    return {"action": "append", "operation": operation}


def _arguments(text="literal", dependencies=()):
    return {"action": "arguments", "arguments": {"text": text},
            "additional_dependencies": list(dependencies)}


def _finish(outputs=("s1",)):
    return {"action": "finish", "name": "Model chosen workflow", "outputs": list(outputs)}


def _planner(decisions, *, catalog=None, callback=None, max_attempts=1):
    catalog = catalog or _catalog()
    runtime, requests = LLMRuntime(), []
    iterator = iter(decisions)
    def provider(request):
        requests.append(request)
        if callback:
            callback(request)
        return json.dumps(next(iterator))
    runtime.register_in_memory_provider("fixture", provider, max_attempts=max_attempts)
    planner = IncrementalLocalWorkflowPlanner(catalog, runtime, model="fixture-model", provider="fixture")
    return planner, catalog, requests


def test_model_chosen_fan_in_derives_only_chosen_reference_and_control_edges():
    decisions = [_append(), _arguments("left"), _append("fixture.beta"), _arguments("right"),
        _append(), _arguments({"$symbol": "s2", "pointer": "/text"}, ["s1"]), _finish(["s3"])]
    planner, catalog, requests = _planner(decisions)
    result = planner.build("Transform two independent synthetic records and combine their order.", approve_provider_call=True)
    assert result.status == "planned"
    plan = validate_workflow_plan(result.plan, catalog).to_dict()
    assert [step["id"] for step in plan["steps"]] == ["s1", "s2", "s3"]
    assert [step["depends_on"] for step in plan["steps"]] == [[], [], ["s1", "s2"]]
    assert plan["steps"][2]["arguments"] == {"text": {"$ref": "s2#/text"}}
    assert plan["outputs"] == ["s3"] and result.to_dict()["model_calls"] == 7
    assert len(requests) == 7 and result.to_dict()["source_dispatches"] == 0
    receipt = result.to_dict()["decisions"][0]
    assert receipt["request_digest"] == content_digest({"model": "fixture-model", "messages": list(requests[0].messages),
        "response_schema": requests[0].response_schema, "require_json": True, "max_output_tokens": 2048, "temperature": 0})
    assert receipt["context_bytes_including_response_schema"] > 0
    assert result.to_dict()["visible_operations"] == ["fixture.alpha", "fixture.beta"]
    # Only protocol syntax is generated. The accepted graph is byte-identical to the
    # existing compiler's result for the same explicit model choices.
    direct = compile_workflow_plan({key: plan[key] for key in ("name", "steps", "outputs")}, catalog, task="Transform two independent synthetic records and combine their order.")
    assert result.plan.plan_digest == direct.plan_digest


def test_argument_request_uses_only_selected_operation_and_enumerated_prior_symbols():
    planner, _, requests = _planner([_append(), _arguments(), _append("fixture.beta"),
        _arguments({"$symbol": "s1", "pointer": "/text"}), _finish(["s2"])])
    result = planner.build("Transform metadata twice.", approve_provider_call=True)
    assert result.status == "planned"
    request = requests[3]
    context = json.loads(request.messages[0]["content"])
    assert context["selected_operation"] == "fixture.beta"
    assert context["accepted_plan_symbols"][0]["id"] == "s1"
    assert "input_schema" not in context["operations"][0] and "output_schema" not in context["operations"][0]
    assert context["selected_operation_descriptor"]["input_schema"] == planner.catalog.resolve("fixture.beta").to_dict()["input_schema"]
    assert "output_schema" not in context["selected_operation_descriptor"]
    producer = context["referenceable_producer_output_schemas"][0]
    assert producer == {"operation_id": "fixture.alpha", "symbols": ["s1"],
        "output_schema": planner.catalog.resolve("fixture.alpha").to_dict()["output_schema"]}
    chooser_context = json.loads(requests[2].messages[0]["content"])
    assert "selected_operation_descriptor" not in chooser_context
    assert "referenceable_producer_output_schemas" not in chooser_context
    assert chooser_context["accepted_plan_symbols"] == context["accepted_plan_symbols"]
    arguments = request.response_schema["anyOf"][0]["properties"]["arguments"]
    assert arguments["anyOf"][1]["properties"]["$symbol"]["enum"] == ["s1"]
    assert "id" not in request.response_schema["anyOf"][0]["properties"]
    assert "operation" not in request.response_schema["anyOf"][0]["properties"]


def test_empty_and_partial_drafts_expose_future_authoring_with_exact_unmodified_prefix_diagnostics():
    task = "Transform the first future metadata output with the second operation."
    contract = compile_workflow_requirements(task,
        required_operations=["fixture.alpha", "fixture.beta"], output_operations=["fixture.beta"],
        data_links=[{"source_operation": "fixture.alpha", "target_operation": "fixture.beta"}],
        max_steps=2)
    planner, _, requests = _planner([_append(), _arguments("future source"),
        _append("fixture.beta"), _arguments({"$symbol": "s1", "pointer": "/text"}), _finish(["s2"])])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned" and result.to_dict()["source_dispatches"] == 0
    contexts = [json.loads(request.messages[0]["content"]) for request in requests]
    empty, partial = contexts[0], contexts[2]
    assert empty["protocol"] == "aurora-incremental-workflow-decisions/0.3"
    assert empty["accepted_plan_symbols"] == [] and empty["prefix_audit"] is None
    assert empty["authoring_state"]["draft"] == "empty"
    assert empty["authoring_state"]["pending_contract_obligations"] == [
        {"code": "required_operation_missing", "operation": "fixture.alpha"},
        {"code": "required_operation_missing", "operation": "fixture.beta"},
        {"code": "required_output_missing", "operation": "fixture.beta"}]
    assert partial["authoring_state"]["draft"] == "partial"
    assert partial["authoring_state"]["accepted_step_count"] == 1
    audit = partial["prefix_audit"]["requirements_audit"]
    assert partial["authoring_state"]["pending_contract_obligations"] == audit["blocking_issues"]
    assert audit["contract_satisfied"] is False and audit["requirements_digest"] == contract.requirements_digest
    claimed_digest = audit.pop("audit_digest")
    assert claimed_digest == content_digest(audit)
    for context, request in zip(contexts, requests):
        assert context["authoring_state"]["execution_status"] == "not_started_by_design"
        assert context["authoring_state"]["observed_source_values"].startswith("unavailable_during_plan_authoring")
        assert context["fixed_caller_requirements"] == contract.to_dict()
        assert {"clarify", "refuse"}.issubset({branch["properties"]["action"]["enum"][0]
            for branch in request.response_schema["anyOf"]})
    assert result.plan.to_dict()["steps"][1]["arguments"] == {"text": {"$ref": "s1#/text"}}


@pytest.mark.parametrize("code", ["forbidden_operation_used", "argument_requirement_unsatisfied",
    "required_value_flow_missing", "future_unknown_issue"])
def test_authoring_context_never_relabels_irreversible_or_unknown_violations_as_pending(code):
    from prism_sdk.workflow_incremental_planner import _authoring_state
    with pytest.raises(IncrementalWorkflowPlanningError, match="non-pending contract violation"):
        _authoring_state([], {"requirements_audit": {"blocking_issues": [{"code": code}]}}, None)


@pytest.mark.parametrize("phase", ["choose", "arguments"])
@pytest.mark.parametrize("action,status,task,message", [
    ("clarify", "clarification_required", "Transform the user-selected record, whose name is unspecified. Do not guess a record.",
     "Which record name should the transformation use?"),
    ("refuse", "refused", "Transform metadata and send it through the unregistered write service. A local read-only result alone does not satisfy this request.",
     "The required write service is unavailable in this catalogue, so I cannot author the complete requested workflow."),
])
def test_future_authoring_semantics_preserve_missing_selector_and_unavailable_write_terminal_decisions(phase, action, status, task, message):
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha"],
        output_operations=["fixture.alpha"], max_steps=1)
    decision = {"action": action, "message": message}
    planner, _, requests = _planner([decision] if phase == "choose" else [_append(), decision])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == status and result.plan is None and "plan" not in result.to_dict()
    assert result.to_dict()["message"] == message and result.to_dict()["accepted_partial_steps"] == []
    assert result.to_dict()["source_dispatches"] == 0 and len(requests) == (1 if phase == "choose" else 2)
    for request in requests:
        branches = [branch["properties"]["action"]["enum"][0] for branch in request.response_schema["anyOf"]]
        assert "clarify" in branches and "refuse" in branches


@pytest.mark.parametrize("bad", [
    {"$symbol": "s2", "pointer": "/text"},
    {"$symbol": "unknown", "pointer": ""},
    {"$symbol": "s1", "pointer": "text"},
    {"$symbol": "s1", "pointer": "/~2"},
    {"$symbol": "s1", "pointer": "/missing"},
    {"$symbol": "s1", "pointer": "/text", "extra": 1},
    {"$ref": "s1#/text"},
])
def test_invalid_or_impossible_binding_never_appends_a_step_or_fabricates_a_plan(bad):
    planner, _, requests = _planner([_append(), _arguments(), _append(), _arguments(bad)])
    result = planner.build("Transform twice.", approve_provider_call=True, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.plan is None
    assert "plan" not in result.to_dict()
    assert len(result.to_dict()["accepted_partial_steps"]) == 1 and len(requests) == 4
    assert result.to_dict()["decisions"][-1]["status"] == "rejected"


@pytest.mark.parametrize("dependencies", [["s2"], ["unknown"], ["s1", "s1"], [1]])
def test_invalid_control_edges_do_not_get_silently_dropped_or_repaired(dependencies):
    planner, _, _ = _planner([_append(), _arguments(), _append(), _arguments("next", dependencies)])
    result = planner.build("Order metadata.", approve_provider_call=True, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and len(result.to_dict()["accepted_partial_steps"]) == 1


@pytest.mark.parametrize("terminal,status", [("clarify", "clarification_required"), ("refuse", "refused")])
@pytest.mark.parametrize("phase", ["choose", "arguments"])
def test_model_clarification_and_refusal_remain_distinct_non_plan_states(terminal, status, phase):
    decision = {"action": terminal, "message": "Please specify the intended metadata scope."}
    planner, _, requests = _planner([decision] if phase == "choose" else [_append(), decision])
    result = planner.build("Unspecified metadata scope.", approve_provider_call=True)
    assert result.status == status and result.plan is None and "plan" not in result.to_dict()
    assert result.to_dict()["message"] == decision["message"]
    assert len(requests) == (1 if phase == "choose" else 2)


def test_rejected_finish_is_not_promoted_by_temporary_prefix_outputs():
    task = "Transform metadata with both operations."
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha", "fixture.beta"],
        output_operations=["fixture.beta"], max_steps=2,
        max_calls_by_operation={"fixture.alpha": 1, "fixture.beta": 1})
    planner, _, _ = _planner([_append(), _arguments(), _finish(), _finish()])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "budget_exhausted" and result.plan is None
    prefix = result.to_dict()["prefix_audit"]
    assert prefix["temporary_audit_outputs"] == ["s1"] and prefix["final_outputs_selected"] is False
    assert prefix["requirements_audit"]["contract_satisfied"] is False
    assert result.to_dict()["requirements_digest"] == contract.requirements_digest


def _selection_catalog():
    return _catalog({"type": "object", "properties": {
        "choices": {"type": "array", "items": {"type": "string", "enum": ["cedar", "elm", "pine"]},
            "minItems": 1, "maxItems": 3, "uniqueItems": True, "default": ["cedar", "elm", "pine"]},
        "mode": {"type": "string", "const": "metadata"}},
        "required": ["mode"], "additionalProperties": False})


def _selection_arguments(arguments):
    return {"action": "arguments", "arguments": arguments, "additional_dependencies": []}


@pytest.mark.parametrize("explicit", [True, False])
def test_enum_members_are_admissible_choices_and_omitted_defaults_keep_native_semantics(explicit):
    task = "Select elm only." if explicit else "Use the declared default selection."
    chosen = ["elm"] if explicit else ["cedar", "elm", "pine"]
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha"],
        output_operations=["fixture.alpha"],
        argument_rules=[{"operation": "fixture.alpha", "path": "/choices", "comparison": "equals", "value": chosen}])
    arguments = {"mode": "metadata", **({"choices": chosen} if explicit else {})}
    planner, catalog, requests = _planner([_append(), _selection_arguments(arguments), _finish()], catalog=_selection_catalog())
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned"
    native = result.plan.to_dict()
    assert native["steps"][0]["arguments"] == {"choices": chosen, "mode": "metadata"}
    context = json.loads(requests[1].messages[0]["content"])
    assert context["selected_operation_descriptor"]["input_schema"] == catalog.resolve("fixture.alpha").to_dict()["input_schema"]
    assert context["selected_operation_descriptor"]["input_schema"]["required"] == ["mode"]
    assert len(context["schema_semantics"]) == 4 and len(json.dumps(context["schema_semantics"]).encode()) < 1600
    assert requests[1].response_schema["anyOf"][0]["properties"]["arguments"]["properties"]["mode"]["const"] == "metadata"
    assert result.plan.plan_digest == compile_workflow_plan(
        {key: native[key] for key in ("name", "steps", "outputs")}, catalog, task=task, requirements=contract).plan_digest


def test_true_full_set_caller_obligation_rejects_a_schema_valid_subset_without_weakening_defaults():
    task = "Select the complete cedar, elm and pine set explicitly."
    complete = ["cedar", "elm", "pine"]
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha"],
        output_operations=["fixture.alpha"], argument_rules=[
            {"operation": "fixture.alpha", "path": "/choices", "comparison": "equals", "value": complete}])
    planner, catalog, requests = _planner([_append(),
        _selection_arguments({"mode": "metadata", "choices": ["elm"]}),
        _selection_arguments({"mode": "metadata", "choices": complete}), _finish()], catalog=_selection_catalog())
    assert catalog.validate_arguments("fixture.alpha", {"mode": "metadata", "choices": ["elm"]})["choices"] == ["elm"]
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned" and len(requests) == 4
    assert result.to_dict()["decisions"][1]["error_code"] == "invalid_arguments_binding_or_prefix_contract"
    assert json.loads(requests[2].messages[0]["content"])["accepted_plan_symbols"] == []
    assert result.plan.to_dict()["steps"][0]["arguments"]["choices"] == complete
    assert result.plan.to_dict()["requirements"] == contract.to_dict()
    assert catalog.resolve("fixture.alpha").to_dict()["input_schema"]["properties"]["choices"]["default"] == complete


@pytest.mark.parametrize("arguments", [{"choices": ["elm"], "mode": "summary"}, {"choices": ["elm"]}])
def test_const_and_required_properties_remain_native_obligations_after_schema_clarification(arguments):
    planner, _, requests = _planner([_append(), _selection_arguments(arguments)], catalog=_selection_catalog())
    result = planner.build("Choose metadata mode and elm.", approve_provider_call=True, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.plan is None
    assert result.to_dict()["accepted_partial_steps"] == [] and len(requests) == 2
    assert result.to_dict()["decisions"][-1]["error_code"] == "invalid_arguments_binding_or_prefix_contract"


def test_workflow_name_cannot_instantiate_a_missing_operation_and_native_finish_feedback_preserves_that_fact():
    task = "Transform a future record and pass it through the second operation."
    contract = compile_workflow_requirements(task,
        required_operations=["fixture.alpha", "fixture.beta"], output_operations=["fixture.beta"],
        max_calls_by_operation={"fixture.alpha": 1, "fixture.beta": 1},
        data_links=[{"source_operation": "fixture.alpha", "target_operation": "fixture.beta"}])
    misleading_name = {"action": "finish", "name": "fixture.beta", "outputs": ["s1"]}
    planner, catalog, requests = _planner([_append(), _arguments(), misleading_name,
        _append("fixture.beta"), _arguments({"$symbol": "s1", "pointer": "/text"}), _finish(["s2"])])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned" and len(requests) == 6
    context = json.loads(requests[3].messages[0]["content"])
    diagnostic = context["validation_feedback"]["finish_diagnostics"]
    assert diagnostic["structure_valid"] is True and diagnostic["finish_accepted"] is False and diagnostic["repairs_applied"] is False
    assert diagnostic["operation_instances"] == [{"symbol": "s1", "operation": "fixture.alpha"}]
    assert diagnostic["selected_output_symbols"] == ["s1"]
    audit = diagnostic["requirements_audit"]
    proposal = {"name": "fixture.beta", "steps": context["accepted_plan_symbols"], "outputs": ["s1"]}
    assert audit == audit_workflow_requirements(proposal, contract, catalog)
    assert audit["blocking_issues"] == [{"code": "required_operation_missing", "operation": "fixture.beta"},
        {"code": "required_output_missing", "operation": "fixture.beta"}]
    assert result.to_dict()["decisions"][2]["finish_diagnostics"]["diagnostics_digest"] == content_digest(diagnostic)
    assert [branch["properties"]["action"]["enum"][0] for branch in requests[3].response_schema["anyOf"]] == ["clarify", "refuse", "append", "finish"]
    assert [step["operation"] for step in result.plan.to_dict()["steps"]] == ["fixture.alpha", "fixture.beta"]


def test_finish_feedback_audits_actual_model_outputs_instead_of_temporary_prefix_outputs():
    task = "Transform a record twice and return both operation outputs."
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha", "fixture.beta"],
        output_operations=["fixture.alpha", "fixture.beta"])
    planner, _, requests = _planner([_append(), _arguments(), _append("fixture.beta"),
        _arguments({"$symbol": "s1", "pointer": "/text"}), _finish(["s2"]), _finish(["s1", "s2"])])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned"
    context = json.loads(requests[5].messages[0]["content"])
    diagnostic = context["validation_feedback"]["finish_diagnostics"]
    assert context["prefix_audit"]["requirements_audit"]["contract_satisfied"] is True
    assert diagnostic["requirements_audit"]["contract_satisfied"] is False
    assert diagnostic["requirements_audit"]["blocking_issues"] == [{"code": "required_output_missing", "operation": "fixture.alpha"}]
    assert result.plan.to_dict()["outputs"] == ["s1", "s2"]


def test_structural_finish_rejection_does_not_fabricate_an_empty_or_successful_requirements_audit():
    task = "Return two independent transformations."
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha", "fixture.beta"])
    planner, _, requests = _planner([_append(), _arguments(), _append("fixture.beta"), _arguments(),
        _finish(["s2"]), _finish(["s1", "s2"])])
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned"
    diagnostic = json.loads(requests[5].messages[0]["content"])["validation_feedback"]["finish_diagnostics"]
    assert diagnostic["structure_valid"] is False and diagnostic["requirements_audit"] is None
    assert diagnostic["requirements_audit_state"] == "not_run_due_to_native_structure_rejection"
    summary = result.to_dict()["decisions"][4]["finish_diagnostics"]
    assert summary["blocking_issue_count"] is None and summary["deferred_argument_count"] is None


def test_finish_diagnostics_cannot_bypass_the_total_prompt_byte_bound_before_a_retry(monkeypatch):
    from prism_sdk import workflow_incremental_planner as incremental
    task = "Use both transformations."
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha", "fixture.beta"])
    def bound_after_rejected_finish(request):
        if len(requests) == 3:
            current = len(json.dumps({"messages": request.messages, "response_schema": request.response_schema}, ensure_ascii=False).encode())
            monkeypatch.setattr(incremental, "MAX_WORKFLOW_PLANNING_CONTEXT_BYTES", current + 1)
    planner, _, requests = _planner([_append(), _arguments(), _finish()], callback=bound_after_rejected_finish)
    with pytest.raises(IncrementalWorkflowPlanningError, match="context exceeds its byte bound"):
        planner.build(task, approve_provider_call=True, requirements=contract)
    assert len(requests) == 3


def test_immutable_exact_contract_is_final_authority_and_preserves_existing_plan_digest():
    task = "Transform the complete first output using the second operation."
    consumer = {"type": "object", "properties": {"text": {"type": "object"}},
                "required": ["text"], "additionalProperties": False}
    catalog = _catalog()
    catalog.register(WorkflowOperation("fixture.consume", "Consume a complete native object", consumer,
        {"type": "object"}, "fixture", implementation="local"), lambda *_a, **_k: pytest.fail("source dispatch"))
    contract = compile_workflow_requirements(task, required_operations=["fixture.alpha", "fixture.consume"],
        output_operations=["fixture.consume"], max_steps=2,
        data_links=[{"source_operation": "fixture.alpha", "target_operation": "fixture.consume", "source_pointer": "", "target_path": "/text"}],
        argument_rules=[{"operation": "fixture.alpha", "path": "/text", "comparison": "equals", "value": "source"}])
    planner, _, _ = _planner([_append(), _arguments("source"), _append("fixture.consume"),
        _arguments({"$symbol": "s1", "pointer": ""}), _finish(["s2"])], catalog=catalog)
    result = planner.build(task, approve_provider_call=True, requirements=contract)
    assert result.status == "planned"
    plan = result.plan.to_dict()
    assert plan["requirements"] == contract.to_dict() and plan["schema"].endswith("/0.2")
    direct = compile_workflow_plan({key: plan[key] for key in ("name", "steps", "outputs")}, catalog, task=task, requirements=contract)
    assert direct.plan_digest == result.plan.plan_digest
    before = result.to_dict()
    before["plan"]["steps"][0]["arguments"]["text"] = "mutated"
    assert result.plan.to_dict()["steps"][0]["arguments"]["text"] == "source"


def test_root_argument_reference_preserves_complete_native_output_without_an_implicit_wrapper():
    decisions = [_append(), _arguments("native object"), _append("fixture.beta"),
        {"action": "arguments", "arguments": {"$symbol": "s1", "pointer": ""}, "additional_dependencies": []},
        _finish(["s2"])]
    planner, _, _ = _planner(decisions)
    result = planner.build("Pass the complete native output to the second transformation.", approve_provider_call=True)
    assert result.status == "planned"
    assert result.plan.to_dict()["steps"][1]["arguments"] == {"$ref": "s1#"}


def test_nested_array_bindings_retain_literal_siblings_and_native_array_bounds():
    catalog = _catalog()
    schema = {"type": "object", "properties": {"rows": {"type": "array", "minItems": 1, "maxItems": 2,
        "uniqueItems": True, "items": {"type": "object", "properties": {
            "payload": {"type": "object"}, "label": {"type": "string", "enum": ["left", "right"]}},
            "required": ["payload", "label"], "additionalProperties": False}}},
        "required": ["rows"], "additionalProperties": False}
    catalog.register(WorkflowOperation("fixture.collect", "Collect explicit synthetic objects", schema,
        {"type": "object"}, "fixture", implementation="local"), lambda *_a, **_k: pytest.fail("source dispatch"))
    argument = {"action": "arguments", "arguments": {"rows": [
        {"label": "left", "payload": {"$symbol": "s1", "pointer": ""}}]}, "additional_dependencies": []}
    planner, _, requests = _planner([_append(), _arguments(), _append("fixture.collect"), argument, _finish(["s2"])], catalog=catalog)
    result = planner.build("Collect synthetic metadata.", approve_provider_call=True)
    assert result.status == "planned"
    assert result.plan.to_dict()["steps"][1]["arguments"] == {"rows": [{"label": "left", "payload": {"$ref": "s1#"}}]}
    rows = requests[3].response_schema["anyOf"][0]["properties"]["arguments"]["anyOf"][0]["properties"]["rows"]["anyOf"][0]
    assert rows["minItems"] == 1 and rows["maxItems"] == 2 and rows["uniqueItems"] is True
    assert "prefixItems" not in json.dumps(requests[3].response_schema)


@pytest.mark.parametrize("types", [["object", "null"], ["null", "object"]])
@pytest.mark.parametrize("payload", [None, {"child": "literal"}, {"child": {"$symbol": "s1", "pointer": "/text"}}])
def test_exact_nullable_object_union_retains_null_literals_object_constraints_and_type_order(types, payload):
    catalog = _catalog()
    schema = {"type": "object", "properties": {"payload": {"type": types,
        "properties": {"child": {"type": "string"}}, "required": ["child"], "additionalProperties": False}},
        "required": ["payload"], "additionalProperties": False}
    catalog.register(WorkflowOperation("fixture.nullable", "Consume optional synthetic metadata", schema,
        {"type": "object"}, "fixture", implementation="local"), lambda *_a, **_k: pytest.fail("source dispatch"))
    decision = {"action": "arguments", "arguments": {"payload": payload}, "additional_dependencies": ["s1"]}
    planner, _, requests = _planner([_append(), _arguments(), _append("fixture.nullable"), decision, _finish(["s2"])], catalog=catalog)
    result = planner.build("Review optional metadata after its source.", approve_provider_call=True)
    assert result.status == "planned"
    native = requests[3].response_schema["anyOf"][0]["properties"]["arguments"]["anyOf"][0]["properties"]["payload"]["anyOf"][0]
    assert native["type"] == types and native["required"] == ["child"] and native["additionalProperties"] is False
    if payload is None:
        assert result.plan.to_dict()["steps"][1]["arguments"]["payload"] is None
    elif isinstance(payload["child"], dict):
        assert result.plan.to_dict()["steps"][1]["arguments"]["payload"] == {"child": {"$ref": "s1#/text"}}
    else:
        assert result.plan.to_dict()["steps"][1]["arguments"]["payload"] == payload


def test_nullable_object_literal_still_requires_its_native_object_fields():
    schema = {"type": "object", "properties": {"text": {"type": ["object", "null"],
        "properties": {"required_value": {"type": "string"}}, "required": ["required_value"], "additionalProperties": False}},
        "required": ["text"], "additionalProperties": False}
    planner, _, _ = _planner([_append(), _arguments({})], catalog=_catalog(schema))
    result = planner.build("Inspect optional metadata.", approve_provider_call=True, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.to_dict()["accepted_partial_steps"] == []


@pytest.mark.parametrize("kind", ["literal_predicate", "data_link"])
def test_irreversible_contract_violation_is_rejected_at_append(kind):
    task = "Use a specifically bound transformation."
    rules = [{"operation": "fixture.alpha", "path": "/text", "comparison": "equals", "value": "fixed"}] if kind == "literal_predicate" else []
    links = [{"source_operation": "fixture.beta", "target_operation": "fixture.alpha", "source_pointer": "/text", "target_path": "/text"}] if kind == "data_link" else []
    contract = compile_workflow_requirements(task, argument_rules=rules, data_links=links)
    planner, _, _ = _planner([_append(), _arguments("wrong")])
    result = planner.build(task, approve_provider_call=True, requirements=contract, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.to_dict()["accepted_partial_steps"] == []


def test_operation_caps_and_step_limit_remove_append_choices_but_do_not_auto_finish():
    task = "One bounded transformation."
    contract = compile_workflow_requirements(task, max_steps=1, max_calls_by_operation={"fixture.alpha": 1, "fixture.beta": 0})
    planner, _, requests = _planner([_append(), _arguments(), _append()])
    result = planner.build(task, approve_provider_call=True, requirements=contract, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.plan is None
    actions = [branch["properties"]["action"]["enum"][0] for branch in requests[2].response_schema["anyOf"]]
    assert "append" not in actions and "finish" in actions
    initial = next(branch for branch in requests[0].response_schema["anyOf"] if branch["properties"]["action"]["enum"] == ["append"])
    assert initial["properties"]["operation"]["enum"] == ["fixture.alpha"]


@pytest.mark.parametrize("outputs", [["s1"], ["s3"], ["s2", "s2"]])
def test_model_outputs_must_cover_all_steps_without_unknown_or_duplicate_symbols(outputs):
    planner, _, _ = _planner([_append(), _arguments(), _append(), _arguments("independent"), _finish(outputs)])
    result = planner.build("Two metadata outputs.", approve_provider_call=True, max_decision_attempts=1)
    assert result.status == "budget_exhausted" and result.plan is None


def test_total_call_budget_can_stop_between_operation_selection_and_arguments():
    planner, _, requests = _planner([_append()])
    result = planner.build("Transform metadata.", approve_provider_call=True, max_model_calls=1)
    assert result.status == "budget_exhausted" and result.to_dict()["reason"] == "model_call_limit"
    assert result.to_dict()["accepted_partial_steps"] == [] and len(requests) == 1


def test_rejected_arguments_retry_only_explicit_model_choices_and_do_not_consume_a_symbol():
    planner, _, requests = _planner([_append(), _arguments(42), _arguments("model correction"), _finish()])
    result = planner.build("Transform metadata.", approve_provider_call=True)
    assert result.status == "planned" and len(requests) == 4
    assert result.plan.to_dict()["steps"][0]["id"] == "s1"
    assert result.plan.to_dict()["steps"][0]["arguments"] == {"text": "model correction"}
    assert result.to_dict()["decisions"][1]["status"] == "rejected"


@pytest.mark.parametrize("schema", [
    {"type": "object", "properties": {"$symbol": {"type": "string"}}},
    {"type": "object", "properties": {"nested": {"type": "object", "properties": {"$ref": {"type": "string"}}}}},
    {"type": "object", "properties": {"value": {"anyOf": [{"type": "string"}, {"type": "null"}]}}},
    {"type": "object", "properties": {"value": {"type": ["string", "null"]}}},
    {"type": "object", "properties": {"value": {"type": ["array", "null"]}}},
    {"type": "object", "properties": {"value": {"type": ["object", "array"]}}},
    {"type": "object", "properties": {"value": {"type": ["object", "null", "array"]}}},
    {"type": "object", "properties": {"value": {"properties": {"field": {"type": "string"}}}}},
    {"type": "object", "const": {"field": "constant"}},
    {"type": "object", "properties": {"value": {"type": "object", "default": {"$symbol": "s1", "pointer": ""}}}},
    {"type": "object", "required": ["$ref"]},
    {"type": "object", "required": "text"},
    {"type": "object", "properties": {"text": {"type": "string", "enum": None}}},
])
def test_unsupported_or_wire_reserved_schema_is_refused_before_any_provider_call(schema):
    planner, _, requests = _planner([], catalog=_catalog(schema))
    with pytest.raises(IncrementalWorkflowPlanningError):
        planner.build("Transform metadata.", approve_provider_call=True)
    assert requests == []


def test_oversized_argument_decoder_is_refused_before_any_provider_call():
    schema = {"type": "object", "description": "x" * 256_001}
    planner, _, requests = _planner([], catalog=_catalog(schema))
    with pytest.raises(IncrementalWorkflowPlanningError, match="schema_byte_bound"):
        planner.build("Transform metadata.", approve_provider_call=True)
    assert requests == []


@pytest.mark.parametrize("kwargs", [{"approve_provider_call": 1}, {"max_steps": True}, {"max_model_calls": 129},
    {"max_decision_attempts": 0}, {"max_decision_attempts": 5}, {"operation_ids": "fixture.alpha"}])
def test_invalid_grants_and_bounds_fail_before_any_provider_call(kwargs):
    planner, _, requests = _planner([])
    with pytest.raises(IncrementalWorkflowPlanningError):
        planner.build("Transform metadata.", **{"approve_provider_call": True, **kwargs})
    assert requests == []


def test_provider_with_transport_retries_is_rejected_before_planning():
    with pytest.raises(IncrementalWorkflowPlanningError, match="one transport attempt"):
        _planner([], max_attempts=2)


def test_nonlocal_provider_is_refused_without_network_access():
    from prism_sdk.llm_runtime import ollama_provider
    runtime = LLMRuntime()
    runtime.register_provider(ollama_provider(base_url="https://example.invalid/v1", max_attempts=1))
    with pytest.raises(IncrementalWorkflowPlanningError, match="loopback"):
        IncrementalLocalWorkflowPlanner(_catalog(), runtime, model="fixture")


def test_relevant_prior_output_schemas_exceeding_context_bound_are_refused_without_silent_truncation():
    catalog = WorkflowOperationCatalog()
    for index in range(3):
        catalog.register(WorkflowOperation("fixture.large" + str(index), "Synthetic large output schema",
            {"type": "object"}, {"type": "object", "description": "x" * 710_000}, "fixture",
            implementation="local"), lambda *_a, **_k: pytest.fail("source dispatch"))
    decisions = []
    for index in range(3):
        decisions.extend([_append("fixture.large" + str(index)),
            {"action": "arguments", "arguments": {}, "additional_dependencies": []}])
    decisions.append(_append("fixture.large0"))
    planner, _, requests = _planner(decisions, catalog=catalog)
    with pytest.raises(IncrementalWorkflowPlanningError, match="context exceeds"):
        planner.build("Inspect metadata.", approve_provider_call=True)
    assert len(requests) == 7


def test_catalogue_drift_during_a_provider_call_is_an_error_not_a_refusal():
    catalog = _catalog()
    def change(_request):
        catalog.register(WorkflowOperation("fixture.new", "New synthetic operation", {"type": "object"},
            {"type": "object"}, "fixture", implementation="local"), lambda *_a, **_k: pytest.fail("source dispatch"))
    planner, _, requests = _planner([_append()], catalog=catalog, callback=change)
    with pytest.raises(IncrementalWorkflowPlanningError, match="identity changed"):
        planner.build("Transform metadata.", approve_provider_call=True)
    assert len(requests) == 1


@pytest.mark.parametrize("mutation", ["same_metadata_registration", "deadline_and_response_limit", "handler"])
def test_full_provider_binding_drift_is_an_error_even_when_presentation_metadata_is_unchanged(mutation):
    def change(_request):
        original = planner._provider_binding.config
        before = planner.runtime.provider_metadata()
        if mutation == "same_metadata_registration":
            planner.runtime.register_provider(replace(original))
        elif mutation == "deadline_and_response_limit":
            planner.runtime.register_provider(replace(original, timeout_seconds=1800, max_response_bytes=99_999_999))
        else:
            original.transport._handler = lambda _request: json.dumps(_finish())
        assert planner.runtime.provider_metadata() == before
    planner, _, requests = _planner([_append()], callback=change)
    with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
        planner.build("Transform metadata.", approve_provider_call=True)
    assert len(requests) == 1


def test_transport_error_propagates_without_hidden_retry_or_fabricated_result():
    def fail(_request):
        raise ProviderError("Synthetic unavailable local provider")
    planner, _, requests = _planner([], callback=fail)
    with pytest.raises(ProviderError):
        planner.build("Transform metadata.", approve_provider_call=True)
    assert len(requests) == 1


def test_result_digest_covers_failed_decisions_and_inspectable_non_executable_prefix():
    planner, _, _ = _planner([_append(), _arguments()])
    result = planner.build("Transform metadata.", approve_provider_call=True, max_model_calls=2)
    value = result.to_dict()
    digest = value.pop("result_digest")
    assert content_digest(value) == digest and result.plan is None
    assert value["prefix_audit"]["executable_plan"] is False
    assert value["source_dispatches"] == 0 and value["model_calls"] == 2
