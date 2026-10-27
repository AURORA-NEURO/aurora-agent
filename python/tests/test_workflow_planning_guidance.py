"""Generic reference guidance preserves caller contracts and never substitutes a plan."""
import json

import pytest

from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.workflow_builder import LocalWorkflowBuilder, audit_workflow_reference_paths
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog
from prism_sdk.workflow_requirements import audit_workflow_requirements, compile_workflow_requirements


def _catalog(*, root_consumer=False):
    catalog = WorkflowOperationCatalog()
    output = {"type": "object", "properties": {
        "metadata": {"type": "object", "properties": {"label": {"type": "string"}},
                     "required": ["label"], "additionalProperties": False}},
        "required": ["metadata"], "additionalProperties": False}
    def refuse_dispatch(*_args, **_kwargs):
        pytest.fail("a planning fixture must never execute an operation")
    catalog.register(WorkflowOperation("fixture.record", "Produce a synthetic record object",
        {"type": "object", "properties": {"label": {"type": "string"}}, "required": ["label"], "additionalProperties": False},
        output, "fixture", implementation="local"), refuse_dispatch)
    consumer_input = output if root_consumer else {"type": "object", "properties": {"payload": {"type": "object"}},
        "required": ["payload"], "additionalProperties": False}
    catalog.register(WorkflowOperation("fixture.review", "Review an explicit synthetic object",
        consumer_input, {"type": "object"}, "fixture", implementation="local"), refuse_dispatch)
    return catalog


def _proposal(reference, *, root_consumer=False, name="Controlled fixture proposal"):
    return {"name": name, "steps": [
        {"id": "record", "operation": "fixture.record", "arguments": {"label": "synthetic developer fixture"}, "depends_on": []},
        {"id": "review", "operation": "fixture.review", "arguments": reference if root_consumer else {"payload": reference}, "depends_on": ["record"]},
    ], "outputs": ["review"]}


def _contract(task, *, pointer="", target="/payload", legacy=False):
    link = {"source_operation": "fixture.record", "target_operation": "fixture.review"}
    if not legacy:
        link.update(source_pointer=pointer, target_path=target)
    return compile_workflow_requirements(task, output_operations=["fixture.review"], data_links=[link],
        max_steps=2, max_calls_by_operation={"fixture.record": 1, "fixture.review": 1})


@pytest.mark.parametrize("pointer", ["", "/metadata"])
def test_exact_source_selection_guidance_generates_a_checked_root_or_child_reference_without_dispatch(pointer):
    task = "Review the selected object from a synthetic record."
    catalog, requests = _catalog(), []
    requirements = _contract(task, pointer=pointer)
    original = requirements.to_dict()
    def model(request):
        requests.append(request)
        instructions = json.loads(request.messages[0]["content"])
        link = instructions["required_value_flow"][0]
        assert link["source_pointer"] == pointer and link["target_path"] == "/payload"
        assert link["reference_suffix"] == "#" + pointer
        assert instructions["references"]["whole_output"] == {"$ref": "producer#"}
        assert "no implicit wrapper" in instructions["references"]["child_output"]
        for descriptor in instructions["operations"]:
            assert descriptor["output_schema"] == catalog.resolve(descriptor["operation_id"]).to_dict()["output_schema"]
        return json.dumps(_proposal({"$ref": "record" + link["reference_suffix"]}))
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture_local", model)
    plan = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture").build(task,
        requirements=requirements, max_steps=2, approve_provider_call=True).to_dict()
    assert plan["requirements"] == original == requirements.to_dict()
    assert plan["steps"][1]["arguments"]["payload"] == {"$ref": "record#" + pointer}
    audit = audit_workflow_requirements({key: plan[key] for key in ("name", "steps", "outputs")}, requirements, catalog)
    assert audit["contract_satisfied"] is True
    assert len(requests) == 1


def test_empty_target_path_guidance_preserves_a_direct_whole_arguments_reference():
    task = "Review a complete synthetic record as the consumer arguments."
    catalog, requests = _catalog(root_consumer=True), []
    requirements = _contract(task, target="")
    def model(request):
        requests.append(request)
        guidance = json.loads(request.messages[0]["content"])["required_value_flow"][0]
        assert guidance["target_path"] == ""
        assert "entire arguments object" in guidance["target_placement"]
        return json.dumps(_proposal({"$ref": "record" + guidance["reference_suffix"]}, root_consumer=True))
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture_local", model)
    plan = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture").build(task,
        requirements=requirements, max_steps=2, approve_provider_call=True).to_dict()
    assert plan["steps"][1]["arguments"] == {"$ref": "record#"}
    assert plan["requirements"]["data_links"][0]["target_path"] == ""
    assert len(requests) == 1


def test_legacy_transitive_flow_guidance_does_not_invent_a_direct_source_or_target_constraint():
    task = "Pass a synthetic record to review under a legacy flow contract."
    catalog = _catalog()
    requirements = _contract(task, legacy=True)
    def model(request):
        guidance = json.loads(request.messages[0]["content"])["required_value_flow"][0]
        assert "source_pointer" not in guidance and "target_path" not in guidance
        assert "reference_suffix" not in guidance and "target_placement" not in guidance
        return json.dumps(_proposal({"$ref": "record#"}))
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture_local", model)
    plan = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture").build(task,
        requirements=requirements, max_steps=2, approve_provider_call=True).to_dict()
    assert plan["requirements"]["schema"].endswith("/0.1")
    assert plan["requirements"] == requirements.to_dict()


def test_impossible_pointer_repair_returns_only_the_second_model_proposal_without_a_synthetic_fallback():
    task = "Pass the native synthetic record to review."
    catalog, requests = _catalog(), []
    requirements = _contract(task)
    def model(request):
        requests.append(request)
        if len(requests) == 1:
            return json.dumps(_proposal({"$ref": "record#/invented_wrapper"}, name="Rejected model proposal"))
        feedback = json.loads(request.messages[-1]["content"])
        assert "impossible" in feedback["structural_issue"]
        assert json.loads(request.messages[0]["content"])["fixed_caller_requirements"] == requirements.to_dict()
        return json.dumps(_proposal({"$ref": "record#"}, name="Actual corrected model proposal"))
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture_local", model)
    plan = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture").build(task,
        requirements=requirements, max_steps=2, max_planning_attempts=2, approve_provider_call=True).to_dict()
    assert plan["name"] == "Actual corrected model proposal"
    assert len(requests) == 2
    audit = audit_workflow_reference_paths({key: plan[key] for key in ("name", "steps", "outputs")}, catalog)
    assert all(row["status"] == "runtime_resolution_required" for row in audit["references"])
