"""Executable workflow claims tested with real catalogue/provider seams and no network."""
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderError
from prism_sdk.workflow_builder import (
    LocalWorkflowBuilder, WorkflowValidationError, compile_workflow_plan,
    execute_workflow_plan, load_workflow_checkpoint, save_workflow_checkpoint,
    validate_workflow_plan,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult


@pytest.fixture
def harness():
    catalog, calls = WorkflowOperationCatalog(), []
    schema = {"type": "object", "properties": {"text": {"type": "string", "minLength": 1}},
              "required": ["text"], "additionalProperties": False}
    def execute(arguments, **_kwargs):
        calls.append(arguments["text"])
        return WorkflowOperationResult({"text": arguments["text"].upper()}, {"kind": "test_local", "synthetic": True})
    catalog.register(WorkflowOperation("local.upper", "Uppercase explicit text", schema, schema,
                                       "local", implementation="local"), execute)
    proposal = {"name": "Two dependent transformations", "steps": [
        {"id": "first", "operation": "local.upper", "arguments": {"text": "glioma"}, "depends_on": []},
        {"id": "second", "operation": "local.upper", "arguments": {"text": {"$ref": "first#/text"}}, "depends_on": ["first"]},
    ], "outputs": ["second"]}
    return catalog, calls, proposal


def test_dag_executes_reference_values_and_metadata_projection_omits_public_values(harness):
    catalog, calls, proposal = harness
    plan = compile_workflow_plan(proposal, catalog, task="Transform text twice")
    run = execute_workflow_plan(plan, catalog, approved=True)
    assert run.status == "completed"
    assert run.outputs == {"second": {"text": "GLIOMA"}}
    assert calls == ["glioma", "GLIOMA"]
    assert "GLIOMA" not in json.dumps(run.to_dict())
    assert "GLIOMA" in json.dumps(run.to_dict(include_values=True))
    assert run.checkpoint["results"]["second"]["arguments_digest"] == content_digest({"text": "GLIOMA"})


def test_no_execution_occurs_without_literal_dispatch_approval(harness):
    catalog, calls, proposal = harness
    plan = compile_workflow_plan(proposal, catalog, task="Transform")
    assert execute_workflow_plan(plan, catalog).status == "approval_required"
    assert calls == []
    with pytest.raises(WorkflowValidationError):
        execute_workflow_plan(plan, catalog, approved=1)
    assert calls == []


def test_budget_pause_roundtrips_disk_without_repeating_completed_steps(harness, tmp_path):
    catalog, calls, proposal = harness
    plan = compile_workflow_plan(proposal, catalog, task="Transform")
    path = tmp_path / "checkpoint.json"
    run = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1,
                                on_checkpoint=lambda value: save_workflow_checkpoint(path, value))
    assert run.status == "paused"
    restored = execute_workflow_plan(plan.to_dict(), catalog, approved=True, checkpoint=load_workflow_checkpoint(path))
    assert restored.status == "completed"
    assert calls == ["glioma", "GLIOMA"]
    replayed = execute_workflow_plan(plan, catalog, checkpoint=restored.checkpoint)
    assert replayed.status == "completed"
    assert len(calls) == 2


@pytest.mark.parametrize("mutation", ["cycle", "missing_dependency", "unbound_reference", "wrong_literal", "extra_argument", "unknown_tool", "duplicate_id"])
def test_invalid_proposals_fail_before_any_executor_is_called(harness, mutation):
    catalog, calls, proposal = harness
    if mutation == "cycle":
        proposal["steps"][0]["depends_on"] = ["second"]
    elif mutation == "missing_dependency":
        proposal["steps"][1]["depends_on"] = ["missing"]
    elif mutation == "unbound_reference":
        proposal["steps"][1]["depends_on"] = []
    elif mutation == "wrong_literal":
        proposal["steps"][0]["arguments"]["text"] = 7
    elif mutation == "extra_argument":
        proposal["steps"][0]["arguments"]["made_up"] = "invalid"
    elif mutation == "unknown_tool":
        proposal["steps"][0]["operation"] = "made.up"
    else:
        proposal["steps"][1]["id"] = "first"
    with pytest.raises(ValueError):
        compile_workflow_plan(proposal, catalog, task="Transform")
    assert calls == []


def test_closed_schema_pointer_failure_refuses_before_any_source_dispatch(harness):
    catalog, calls, proposal = harness
    proposal["steps"][1]["arguments"]["text"] = {"$ref": "first#/missing"}
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(proposal, catalog, task="Transform")
    assert calls == []


def test_persistence_failure_before_dispatch_prevents_the_call(harness):
    catalog, calls, proposal = harness
    def unavailable(_checkpoint):
        raise OSError("fixture storage unavailable")
    with pytest.raises(OSError):
        execute_workflow_plan(compile_workflow_plan(proposal, catalog, task="Transform"), catalog,
                              approved=True, on_checkpoint=unavailable)
    assert calls == []


def test_reviewed_plan_is_immutable_and_catalogue_drift_is_rejected(harness):
    catalog, calls, proposal = harness
    plan = compile_workflow_plan(proposal, catalog, task="Transform")
    copy = plan.to_dict()
    copy["steps"][0]["arguments"]["text"] = "changed"
    with pytest.raises(ValueError):
        validate_workflow_plan(copy, catalog)
    assert plan.to_dict()["steps"][0]["arguments"]["text"] == "glioma"
    operation = catalog.resolve("local.upper")
    catalog.register(WorkflowOperation("local.extra", "Extra", operation.input_schema, operation.output_schema,
                                       "local", implementation="local"), lambda *_args, **_kwargs: None)
    with pytest.raises(ValueError):
        execute_workflow_plan(plan, catalog, approved=True)
    assert calls == []


def test_local_model_proposal_becomes_validated_plan_without_source_dispatch(harness):
    catalog, calls, proposal = harness
    runtime, provider_calls = LLMRuntime(), []
    def model(request):
        provider_calls.append(request)
        return json.dumps(proposal)
    runtime.register_in_memory_provider("fixture_local", model)
    builder = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture")
    with pytest.raises(ValueError):
        builder.build("Transform glioma text")
    assert provider_calls == []
    plan = builder.build("Transform glioma text", approve_provider_call=True)
    assert validate_workflow_plan(plan, catalog) == plan
    assert len(provider_calls) == 1
    assert calls == []
    assert execute_workflow_plan(plan, catalog, approved=True).status == "completed"


def test_local_model_cannot_exceed_the_requested_step_budget(harness):
    catalog, calls, proposal = harness
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture_local", lambda _request: json.dumps(proposal))
    builder = LocalWorkflowBuilder(catalog, runtime, provider="fixture_local", model="fixture")
    with pytest.raises((ValueError, ProviderError)):
        builder.build("Transform", approve_provider_call=True, max_steps=1)
    assert calls == []


def test_unused_steps_are_rejected_before_unnecessary_source_dispatch(harness):
    catalog, calls, proposal = harness
    proposal["steps"][1]["arguments"] = {"text": "unused"}
    proposal["steps"][1]["depends_on"] = []
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(proposal, catalog, task="Transform")
    assert calls == []
