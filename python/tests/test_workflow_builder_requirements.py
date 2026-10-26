"""Fixed caller requirements remain binding through planning, dispatch and replay."""
import copy
from dataclasses import replace
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderError
from prism_sdk.workflow_builder import (
    LocalWorkflowBuilder, WorkflowValidationError, compile_workflow_plan,
    execute_workflow_plan, validate_workflow_plan,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import WorkflowRequirementError, compile_workflow_requirements


@pytest.fixture
def case():
    catalog, calls = WorkflowOperationCatalog(), []
    schema = {"type": "object", "properties": {"query": {"type": "string"},
              "limit": {"type": "integer", "default": 5}}, "required": ["query"], "additionalProperties": False}
    for operation in ("local.search", "local.review", "local.extra"):
        def execute(arguments, *, _operation=operation, **_kwargs):
            calls.append((_operation, arguments))
            return WorkflowOperationResult(dict(arguments), {"fixture": True})
        catalog.register(WorkflowOperation(operation, "Local fixture", schema, schema, "local", implementation="local"), execute)
    proposal = {"name": "Search then review", "steps": [
        {"id": "search", "operation": "local.search", "arguments": {"query": "glioma", "limit": 2}, "depends_on": []},
        {"id": "review", "operation": "local.review", "arguments": {"$ref": "search#"}, "depends_on": ["search"]},
    ], "outputs": ["review"]}
    task = "Search glioma with limit two and review the returned data"
    contract = compile_workflow_requirements(task, output_operations=["local.review"], forbidden_operations=["local.extra"],
        argument_rules=[{"operation": "local.search", "path": "/query", "comparison": "equals", "value": "glioma"},
                        {"operation": "local.search", "path": "/limit", "comparison": "equals", "value": 2},
                        {"operation": "local.review", "path": "/limit", "comparison": "at_most", "value": 2}],
        data_links=[{"source_operation": "local.search", "target_operation": "local.review"}],
        max_steps=2, max_calls_by_operation={"local.search": 1, "local.review": 1})
    return catalog, calls, proposal, task, contract


def _builder(catalog, responses, requests):
    runtime = LLMRuntime()
    def handler(request):
        requests.append(request)
        result = responses.pop(0)
        if isinstance(result, Exception):
            raise result
        return json.dumps(result)
    runtime.register_in_memory_provider("fixture", handler)
    return LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture")


def _restamp(value, key):
    value.pop(key, None)
    value[key] = content_digest(value)
    return value


def test_contract_plan_roundtrips_and_replays_without_repeating_completed_source(case):
    catalog, calls, proposal, task, contract = case
    plan = compile_workflow_plan(proposal, catalog, task=task, requirements=contract)
    assert plan.to_dict()["schema"] == "aurora-executable-workflow-plan/0.2"
    assert validate_workflow_plan(plan.to_dict(), catalog) == plan
    paused = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1)
    assert paused.status == "paused"
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=paused.checkpoint).status == "completed"
    assert [operation for operation, _ in calls] == ["local.search", "local.review"]


@pytest.mark.parametrize("mutation", ["query", "limit", "default", "output", "flow", "forbidden", "task"])
def test_structurally_valid_proposals_cannot_silently_relax_caller_intent(case, mutation):
    catalog, calls, proposal, task, contract = case
    if mutation == "query":
        proposal["steps"][0]["arguments"]["query"] = "melanoma"
    elif mutation == "limit":
        proposal["steps"][0]["arguments"]["limit"] = 3
    elif mutation == "default":
        del proposal["steps"][0]["arguments"]["limit"]
    elif mutation == "output":
        proposal["outputs"] = ["search"]
    elif mutation == "flow":
        proposal["steps"][1]["arguments"] = {"query": "invented", "limit": 2}
    elif mutation == "forbidden":
        proposal["steps"][0]["operation"] = "local.extra"
    else:
        task = "A different task"
    with pytest.raises(ValueError):
        compile_workflow_plan(proposal, catalog, task=task, requirements=contract)
    assert calls == []


def test_deferred_argument_predicates_block_before_target_dispatch(case):
    catalog, calls, proposal, task, _ = case
    contract = compile_workflow_requirements(task, argument_rules=[
        {"operation": "local.review", "path": "/query", "comparison": "equals", "value": "different"}])
    plan = compile_workflow_plan(proposal, catalog, task=task, requirements=contract)
    run = execute_workflow_plan(plan, catalog, approved=True)
    assert run.status == "blocked"
    assert run.checkpoint["failure"] == {"step_id": "review", "error_type": "WorkflowRequirementError"}
    assert [operation for operation, _ in calls] == ["local.search"]


def test_rehashed_checkpoint_cannot_bypass_resolved_contract_on_resume(case):
    catalog, calls, proposal, task, contract = case
    plan = compile_workflow_plan(proposal, catalog, task=task, requirements=contract)
    completed = execute_workflow_plan(plan, catalog, approved=True).checkpoint
    source, target = completed["results"]["search"], completed["results"]["review"]
    source["value"]["limit"] = target["value"]["limit"] = 9
    source["value_digest"] = content_digest(source["value"])
    target["value_digest"] = content_digest(target["value"])
    target["arguments_digest"] = content_digest(source["value"])
    _restamp(completed, "checkpoint_digest")
    with pytest.raises(WorkflowRequirementError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=completed)
    assert len(calls) == 2


def test_rehashed_plan_still_validates_contract_and_task_binding(case):
    catalog, calls, proposal, task, contract = case
    plan = compile_workflow_plan(proposal, catalog, task=task, requirements=contract).to_dict()
    plan["steps"][0]["arguments"]["limit"] = 12
    _restamp(plan, "plan_digest")
    with pytest.raises(WorkflowRequirementError):
        validate_workflow_plan(plan, catalog)
    plan["task_digest"] = content_digest({"task": "other"})
    _restamp(plan, "plan_digest")
    with pytest.raises(WorkflowRequirementError):
        validate_workflow_plan(plan, catalog)
    assert calls == []


def test_bounded_repair_checks_same_contract_and_dispatches_no_sources(case):
    catalog, calls, proposal, task, contract = case
    invalid = copy.deepcopy(proposal)
    invalid["steps"][0]["arguments"]["limit"] = 8
    requests = []
    builder = _builder(catalog, [invalid, proposal], requests)
    plan = builder.build(task, approve_provider_call=True, requirements=contract, max_planning_attempts=2)
    assert plan.to_dict()["requirements"] == contract.to_dict()
    assert calls == [] and len(requests) == 2
    first_system = json.loads(requests[0].messages[0]["content"])
    second_system = json.loads(requests[1].messages[0]["content"])
    assert first_system["fixed_caller_requirements"] == second_system["fixed_caller_requirements"] == contract.to_dict()
    feedback = json.loads(requests[1].messages[-1]["content"])
    assert feedback["requirement_issues"][0]["code"] == "argument_requirement_unsatisfied"


def test_repair_budget_exhaustion_never_returns_best_effort_plan(case):
    catalog, calls, proposal, task, contract = case
    proposal["steps"][0]["arguments"]["limit"] = 8
    requests = []
    builder = _builder(catalog, [proposal] * 4, requests)
    with pytest.raises(WorkflowValidationError):
        builder.build(task, approve_provider_call=True, requirements=contract, max_planning_attempts=4)
    assert len(requests) == 4 and calls == []


@pytest.mark.parametrize("change", ["different_task", "hidden_required", "steps", "attempts", "unknown"])
def test_invalid_requirements_and_planning_bounds_fail_before_provider(case, change):
    catalog, calls, proposal, task, contract = case
    requests, kwargs = [], {"requirements": contract}
    if change == "different_task":
        task += " changed"
    elif change == "hidden_required":
        kwargs["operation_ids"] = ["local.review"]
    elif change == "steps":
        kwargs["max_steps"] = 1
    elif change == "attempts":
        kwargs["max_planning_attempts"] = True
    else:
        kwargs["requirements"] = compile_workflow_requirements(task, required_operations=["unknown.operation"])
    with pytest.raises(ValueError):
        _builder(catalog, [proposal], requests).build(task, approve_provider_call=True, **kwargs)
    assert requests == [] and calls == []


def test_transport_failure_is_not_retried_as_a_proposal_repair(case):
    catalog, calls, proposal, task, contract = case
    requests = []
    builder = _builder(catalog, [ProviderError("fixture outage")], requests)
    with pytest.raises(ProviderError):
        builder.build(task, approve_provider_call=True, requirements=contract, max_planning_attempts=4)
    assert len(requests) == 1 and calls == []


def test_structural_repair_receives_previous_proposal_and_bounded_failure_feedback(case):
    catalog, calls, proposal, task, contract = case
    invalid = copy.deepcopy(proposal)
    invalid["steps"][1]["depends_on"] = []
    requests = []
    builder = _builder(catalog, [invalid, proposal], requests)
    builder.build(task, approve_provider_call=True, requirements=contract, max_planning_attempts=2)
    assert json.loads(requests[1].messages[-2]["content"]) == invalid
    assert requests[1].messages[-2]["role"] == "assistant"
    assert json.loads(requests[1].messages[-1]["content"])["structural_issue"] == "workflow reference is outside dependency closure"
    assert calls == []


def test_retry_failed_dispatch_still_consumes_fixed_operation_call_budget(case):
    catalog, calls, proposal, task, contract = case
    def unavailable(arguments, **_kwargs):
        calls.append(("local.search", arguments))
        raise OSError("fixture outage")
    operation = catalog.resolve("local.search")
    replacement = WorkflowOperationCatalog([operation], executors={"local.search": unavailable})
    proposal = {"name": "One call", "steps": [proposal["steps"][0]], "outputs": ["search"]}
    contract = compile_workflow_requirements(task, required_operations=["local.search"], max_calls_by_operation={"local.search": 1})
    plan = compile_workflow_plan(proposal, replacement, task=task, requirements=contract)
    failed = execute_workflow_plan(plan, replacement, approved=True)
    assert failed.status == "failed" and len(calls) == 1
    held = execute_workflow_plan(plan, replacement, approved=True, checkpoint=failed.checkpoint, retry_failed=True)
    assert held.status == "blocked" and len(calls) == 1
    tampered = held.checkpoint
    tampered["attempts"]["search"] = 2
    _restamp(tampered, "checkpoint_digest")
    with pytest.raises(WorkflowRequirementError):
        execute_workflow_plan(plan, replacement, approved=True, checkpoint=tampered)


def test_contractless_legacy_schema_remains_valid(case):
    catalog, _, proposal, task, _ = case
    plan = compile_workflow_plan(proposal, catalog, task=task)
    assert plan.to_dict()["schema"] == "aurora-executable-workflow-plan/0.1"
    assert "requirements" not in plan.to_dict()
    assert validate_workflow_plan(plan, catalog) == plan


def test_checkpoint_callback_cannot_change_catalogue_before_dispatch(case):
    catalog, calls, proposal, task, contract = case
    plan = compile_workflow_plan(proposal, catalog, task=task, requirements=contract)
    def drift(_state):
        operation = catalog.resolve("local.search")
        catalog.register(WorkflowOperation("local.added", "Changed catalogue", operation.input_schema,
                                           operation.output_schema, "local", implementation="local"), lambda *_args, **_kwargs: None)
    with pytest.raises(WorkflowValidationError, match="checkpoint persistence"):
        execute_workflow_plan(plan, catalog, approved=True, on_checkpoint=drift)
    assert calls == []


def test_planner_refuses_hidden_transport_retries_before_invocation(case):
    catalog, calls, proposal, task, contract = case
    requests, runtime = [], LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: requests.append(request) or json.dumps(proposal), max_attempts=2)
    with pytest.raises(WorkflowValidationError, match="one transport attempt"):
        LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture")
    assert requests == calls == []


def test_wrong_response_identity_is_never_repaired_or_compiled(case):
    catalog, calls, proposal, task, contract = case
    requests = []
    class ReboundRuntime(LLMRuntime):
        def invoke(self, *args, **kwargs):
            return replace(super().invoke(*args, **kwargs), model="different-model")
    runtime = ReboundRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: requests.append(request) or json.dumps(proposal))
    builder = LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture")
    with pytest.raises(WorkflowValidationError, match="response changed"):
        builder.build(task, approve_provider_call=True, requirements=contract, max_planning_attempts=4)
    assert len(requests) == 1 and calls == []


@pytest.mark.parametrize("invalid", [[{}], {"local.search": True}, [], [1], "local.search", ["local.search", "local.search"]])
def test_invalid_allowlists_raise_checked_errors_before_provider(case, invalid):
    catalog, calls, proposal, task, contract = case
    requests = []
    builder = _builder(catalog, [proposal], requests)
    with pytest.raises(WorkflowValidationError, match="allowlist"):
        builder.build(task, approve_provider_call=True, operation_ids=invalid)
    assert requests == calls == []
