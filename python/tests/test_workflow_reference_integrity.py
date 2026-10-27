"""Reference admission distinguishes impossible paths from uninspected source values."""
from copy import deepcopy
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderError
from prism_sdk.workflow_builder import (
    LocalWorkflowBuilder, WorkflowValidationError, audit_workflow_reference_paths,
    compile_workflow_plan, execute_workflow_plan, validate_workflow_plan,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import (
    WorkflowRequirementError, audit_workflow_requirements, compile_workflow_requirements,
    merge_workflow_requirements, validate_workflow_requirements,
)


TASK = "Review actual fixture output values"


def obj(properties=None, *, required=(), additional=False):
    return {"type": "object", "properties": properties or {}, "required": list(required), "additionalProperties": additional}


def source_schema():
    return obj({
        "records": {"type": "array", "maxItems": 2, "items": obj({"title": {"type": "string"}, "count": {"type": "integer"}})},
        "optional": obj({"tag": {"type": "string"}}),
        "a/b": obj({"~key": {"type": "integer"}}),
    }, required=("records",))


def catalog(schema=None, value=None, consumer_schema=None):
    calls = []
    registry = WorkflowOperationCatalog()
    def produce(arguments, **kwargs):
        calls.append(("producer", arguments))
        return WorkflowOperationResult({"records": [{"title": "actual", "count": 3}], "a/b": {"~key": 0}} if value is None else value, {"fixture": "actual-source"})
    def consume(arguments, **kwargs):
        calls.append(("consumer", deepcopy(arguments)))
        return WorkflowOperationResult({"accepted": True}, {"fixture": "actual-consumer"})
    registry.register(WorkflowOperation("fixture.produce", "Produce actual fixture values", obj(), source_schema() if schema is None else schema,
                                        "fixture", implementation="local"), produce)
    registry.register(WorkflowOperation("fixture.consume", "Consume actual fixture values", obj({"value": {}, "decoy": {}}) if consumer_schema is None else consumer_schema,
                                        obj({"accepted": {"type": "boolean"}}), "fixture", implementation="local"), consume)
    return registry, calls


def proposal(pointer="", *, arguments=None):
    return {"name": "Actual reference values", "steps": [
        {"id": "producer", "operation": "fixture.produce", "arguments": {}, "depends_on": []},
        {"id": "consumer", "operation": "fixture.consume", "arguments": {"value": {"$ref": "producer#" + pointer}} if arguments is None else arguments, "depends_on": ["producer"]},
    ], "outputs": ["consumer"]}


def restamp(value, field):
    value[field] = content_digest({key: child for key, child in value.items() if key != field})
    return value


@pytest.mark.parametrize("pointer", ["/absent", "/records/2", "/records/0/absent", "/records/0/title/nested", "/records/-", "/records/01"])
def test_closed_schema_impossible_fields_indices_and_scalar_traversal_refuse_before_any_calls(pointer):
    registry, calls = catalog()
    report = audit_workflow_reference_paths(proposal(pointer), registry)
    assert report["blocking_issue_count"] == 1 and report["runtime_resolution_count"] == 0
    assert report["references"][0]["status"] == "statically_impossible"
    with pytest.raises(WorkflowValidationError, match="impossible under the source output schema"):
        compile_workflow_plan(proposal(pointer), registry, task=TASK)
    assert calls == []


@pytest.mark.parametrize("pointer,expected", [("/records/0/title", "actual"), ("/a~1b/~0key", 0), ("/records/0/count", 3)])
def test_valid_nested_and_escaped_pointers_deliver_actual_source_values(pointer, expected):
    registry, calls = catalog()
    plan = compile_workflow_plan(proposal(pointer), registry, task=TASK)
    assert calls == []
    result = execute_workflow_plan(plan, registry, approved=True)
    assert result.status == "completed" and calls[1][1]["value"] == expected
    assert [row[0] for row in calls] == ["producer", "consumer"]


@pytest.mark.parametrize("pointer", ["/optional/tag", "/records/1/title"])
def test_optional_members_and_short_arrays_defer_until_actual_value_then_hold_consumer(pointer):
    registry, calls = catalog()
    report = audit_workflow_reference_paths(proposal(pointer), registry)
    assert report["blocking_issue_count"] == 0 and report["runtime_resolution_count"] == 1
    assert "not_existence_proof" in report["references"][0]["reason"]
    plan = compile_workflow_plan(proposal(pointer), registry, task=TASK)
    result = execute_workflow_plan(plan, registry, approved=True)
    assert result.status == "blocked" and [row[0] for row in calls] == ["producer"]
    assert result.checkpoint["attempts"] == {"producer": 1}
    resumed = execute_workflow_plan(plan, registry, approved=True, checkpoint=result.checkpoint)
    assert resumed.status == "retry_required" and [row[0] for row in calls] == ["producer"]


@pytest.mark.parametrize("schema", [obj(additional=True), {"type": "object"}, obj({"dynamic": {}}, additional=True)])
def test_open_output_paths_are_unknown_until_resolution_and_are_never_blacklisted(schema):
    registry, calls = catalog(schema, {})
    candidate = proposal("/json/pointer")
    report = audit_workflow_reference_paths(candidate, registry)
    assert report["references"][0]["status"] == "runtime_resolution_required"
    assert report["source_dispatch"] is False and report["provider_calls"] == 0 and calls == []
    plan = compile_workflow_plan(candidate, registry, task=TASK)
    result = execute_workflow_plan(plan, registry, approved=True)
    assert result.status == "blocked" and [row[0] for row in calls] == ["producer"]


@pytest.mark.parametrize("keyword", ["anyOf", "oneOf"])
def test_union_refuses_only_when_every_alternative_excludes_the_pointer(keyword):
    schema = obj({"choice": {keyword: [{"type": "string"}, obj({"present": {"type": "integer"}})]}})
    registry, calls = catalog(schema, {"choice": {"present": 3}})
    plan = compile_workflow_plan(proposal("/choice/present"), registry, task=TASK)
    assert execute_workflow_plan(plan, registry, approved=True).status == "completed"
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(proposal("/choice/absent"), registry, task=TASK)
    assert len(calls) == 2


def test_allof_closed_branch_refutes_even_when_another_branch_is_unknown():
    registry, calls = catalog(obj({"choice": {"allOf": [{}, obj({"present": {"type": "integer"}})]}}), {})
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(proposal("/choice/absent"), registry, task=TASK)
    assert calls == []


def test_false_allof_output_branch_refuses_even_a_root_reference_before_dispatch():
    registry, calls = catalog({"type": "object", "allOf": [False]}, {})
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(proposal(), registry, task=TASK)
    assert calls == []


def test_reference_schema_analysis_refuses_exhaustion_instead_of_guessing_path_admission():
    registry, calls = catalog({"type": "object", "allOf": [True] * 12_000}, {}, consumer_schema=obj(additional=True))
    candidate = proposal(arguments={f"value{index}": {"$ref": f"producer#/field{index}"} for index in range(9)})
    with pytest.raises(WorkflowValidationError, match="traversal bound"):
        compile_workflow_plan(candidate, registry, task=TASK)
    assert calls == []


@pytest.mark.parametrize("constraint", [{"const": {"items": [3]}}, {"enum": [{"items": [3]}, {"items": []}]}])
def test_const_and_enum_values_refute_only_paths_missing_from_every_admitted_literal(constraint):
    registry, calls = catalog(obj({"fixed": constraint}), {"fixed": {"items": [3]}})
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(proposal("/fixed/items/1"), registry, task=TASK)
    plan = compile_workflow_plan(proposal("/fixed/items/0"), registry, task=TASK)
    assert execute_workflow_plan(plan, registry, approved=True).status == "completed"
    assert calls[1][1] == {"value": 3}


def link(**constraints):
    return {"source_operation": "fixture.produce", "target_operation": "fixture.consume", **constraints}


def test_exact_whole_bundle_link_binds_both_output_pointer_and_consumer_argument_path():
    registry, calls = catalog(obj(additional=True), {"json": {"pointer": {}}})
    contract = compile_workflow_requirements(TASK, data_links=[link(source_pointer="", target_path="/value")])
    assert contract.to_dict()["schema"] == "aurora-workflow-requirements/0.2"
    invalid = proposal("/json/pointer", arguments={"value": {"$ref": "producer#/json/pointer"}, "decoy": {"$ref": "producer#"}})
    assert audit_workflow_requirements(invalid, contract, registry)["blocking_issues"][0]["code"] == "required_value_flow_missing"
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(invalid, registry, task=TASK, requirements=contract)
    assert calls == []
    plan = compile_workflow_plan(proposal(), registry, task=TASK, requirements=contract)
    result = execute_workflow_plan(plan, registry, approved=True)
    assert result.status == "completed" and calls[1][1]["value"] == {"json": {"pointer": {}}}


def test_exact_nested_link_uses_rfc6901_escaping_and_has_no_whole_bundle_only_global_rule():
    registry, calls = catalog(consumer_schema=obj({"a/b": {"type": "array", "items": obj({"~value": {}})}}))
    contract = compile_workflow_requirements(TASK, data_links=[link(source_pointer="/records/0/title", target_path="/a~1b/0/~0value")])
    candidate = proposal(arguments={"a/b": [{"~value": {"$ref": "producer#/records/0/title"}}]})
    plan = compile_workflow_plan(candidate, registry, task=TASK, requirements=contract)
    assert execute_workflow_plan(plan, registry, approved=True).status == "completed"
    assert calls[1][1] == {"a/b": [{"~value": "actual"}]}


@pytest.mark.parametrize("constraints", [{"source_pointer": ""}, {"target_path": "/value"}])
def test_each_optional_link_constraint_can_be_used_independently(constraints):
    registry, calls = catalog()
    contract = compile_workflow_requirements(TASK, data_links=[link(**constraints)])
    compile_workflow_plan(proposal(), registry, task=TASK, requirements=contract)
    invalid = proposal(arguments={"decoy": {"$ref": "producer#/records"}})
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(invalid, registry, task=TASK, requirements=contract)
    assert calls == []


def test_constrained_flow_cannot_be_satisfied_by_an_indirect_transformed_ancestor():
    registry, calls = catalog(consumer_schema=obj(additional=True))
    candidate = proposal()
    candidate["steps"].insert(1, {"id": "bridge", "operation": "fixture.consume", "arguments": {"value": {"$ref": "producer#"}}, "depends_on": ["producer"]})
    candidate["steps"][2].update(arguments={"value": {"$ref": "bridge#"}}, depends_on=["bridge"])
    contract = compile_workflow_requirements(TASK, data_links=[link(source_pointer="", target_path="/value")])
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(candidate, registry, task=TASK, requirements=contract)
    legacy = compile_workflow_requirements(TASK, data_links=[link()])
    compile_workflow_plan(candidate, registry, task=TASK, requirements=legacy)
    assert calls == []


def test_rehashed_invalid_reference_is_rejected_before_initial_or_resumed_dispatch():
    registry, calls = catalog()
    plan = compile_workflow_plan(proposal(), registry, task=TASK)
    paused = execute_workflow_plan(plan, registry, approved=True, max_step_calls=1)
    assert paused.status == "paused" and len(calls) == 1
    forged = plan.to_dict()
    forged["steps"][1]["arguments"]["value"]["$ref"] = "producer#/invented"
    restamp(forged, "plan_digest")
    for checkpoint in (None, paused.checkpoint):
        with pytest.raises(WorkflowValidationError, match="impossible"):
            execute_workflow_plan(forged, registry, approved=True, checkpoint=checkpoint)
    assert len(calls) == 1


def test_rehashed_wrong_bundle_flow_is_refused_on_resume_even_with_existing_source_results():
    registry, calls = catalog(obj(additional=True), {"json": {}})
    contract = compile_workflow_requirements(TASK, data_links=[link(source_pointer="", target_path="/value")])
    plan = compile_workflow_plan(proposal(), registry, task=TASK, requirements=contract)
    paused = execute_workflow_plan(plan, registry, approved=True, max_step_calls=1)
    forged = plan.to_dict()
    forged["steps"][1]["arguments"] = {"value": {"$ref": "producer#/json"}, "decoy": {"$ref": "producer#"}}
    restamp(forged, "plan_digest")
    with pytest.raises(WorkflowRequirementError):
        execute_workflow_plan(forged, registry, approved=True, checkpoint=paused.checkpoint)
    assert [row[0] for row in calls] == ["producer"]


def test_legacy_contract_bytes_and_plan_digest_remain_unchanged_after_new_link_support():
    registry, calls = catalog()
    legacy = compile_workflow_requirements(TASK, data_links=[link()])
    assert legacy.to_dict()["schema"] == "aurora-workflow-requirements/0.1"
    assert "source_pointer" not in legacy.to_dict()["data_links"][0]
    plan = compile_workflow_plan(proposal(), registry, task=TASK, requirements=legacy)
    # Independently reproduced with the committed pre-change compiler and requirements.
    assert legacy.requirements_digest == "b8b5d5100456d44a052209abca9a241ebb2b30715ac325d2b4c0c5e5f83fe76c"
    assert plan.plan_digest == "33181010445f658664ad619ba9a5c1a99feb45b6de46b6e1fe73f704418eaab6"
    assert validate_workflow_plan(plan, registry) == plan
    assert validate_workflow_requirements(legacy, registry) == legacy and calls == []


def test_merge_keeps_stricter_direct_value_flow_and_legacy_link_without_downversion():
    registry, calls = catalog()
    legacy = compile_workflow_requirements(TASK, data_links=[link()])
    exact = compile_workflow_requirements(TASK, data_links=[link(source_pointer="", target_path="/value")])
    merged = merge_workflow_requirements(TASK, legacy, exact, catalog=registry)
    assert merged.to_dict()["schema"] == "aurora-workflow-requirements/0.2"
    assert merged.to_dict()["data_links"] == [link(), link(source_pointer="", target_path="/value")]
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_plan(proposal("/records"), registry, task=TASK, requirements=merged)
    assert calls == []


@pytest.mark.parametrize("constraints", [{"source_pointer": "/~2"}, {"target_path": "value"}, {"target_path": False}, {"source_pointer": "/" + "x" * 512}])
def test_new_reference_constraints_are_bounded_exact_json_pointers(constraints):
    with pytest.raises(WorkflowRequirementError):
        compile_workflow_requirements(TASK, data_links=[link(**constraints)])


def test_legacy_schema_cannot_smuggle_new_link_semantics_by_rehashing():
    contract = compile_workflow_requirements(TASK, data_links=[link(source_pointer="")]).to_dict()
    contract["schema"] = "aurora-workflow-requirements/0.1"
    restamp(contract, "requirements_digest")
    with pytest.raises(WorkflowRequirementError):
        validate_workflow_requirements(contract)


def test_model_repair_receives_impossible_reference_diagnostic_and_uses_actual_output_schema():
    registry, calls = catalog()
    requests = []
    proposals = [proposal("/invented"), proposal("/records/0/title")]
    def model(request):
        requests.append(request)
        return json.dumps(proposals[len(requests) - 1])
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", model)
    builder = LocalWorkflowBuilder(registry, runtime, model="fixture-model", provider="fixture")
    plan = builder.build(TASK, approve_provider_call=True, max_planning_attempts=2)
    assert plan.to_dict()["steps"][1]["arguments"] == {"value": {"$ref": "producer#/records/0/title"}}
    context = json.loads(requests[0].messages[0]["content"])
    producer = next(operation for operation in context["operations"] if operation["operation_id"] == "fixture.produce")
    assert producer["output_schema"] == source_schema()
    issue = json.loads(requests[1].messages[-1]["content"])["structural_issue"]
    assert "producer#/invented" in issue and "consumer" in issue and "impossible" in issue
    assert calls == [] and len(requests) == 2


def test_model_repair_exhaustion_cannot_admit_an_impossible_plan_or_execute_sources():
    registry, calls = catalog()
    requests = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: requests.append(request) or json.dumps(proposal("/invented")))
    builder = LocalWorkflowBuilder(registry, runtime, model="fixture-model", provider="fixture")
    with pytest.raises(WorkflowValidationError, match="failed the workflow proposal"):
        builder.build(TASK, approve_provider_call=True, max_planning_attempts=2)
    assert len(requests) == 2 and calls == []


def test_oversized_output_schema_context_refuses_before_provider_instead_of_omitting_namespace():
    registry, calls = catalog()
    for index in range(3):
        registry.register(WorkflowOperation(f"fixture.large{index}", "Large fixture schema", obj(),
            {"type": "object", "description": "x" * 700_000}, "fixture", implementation="local"),
            lambda *_args, **_kwargs: pytest.fail("no source may execute while planning"))
    requests = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: requests.append(request) or json.dumps(proposal()))
    builder = LocalWorkflowBuilder(registry, runtime, model="fixture-model", provider="fixture")
    with pytest.raises(ProviderError, match="bounded text contract"):
        builder.build(TASK, approve_provider_call=True)
    assert requests == [] and calls == []
