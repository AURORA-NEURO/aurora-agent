"""Generated intent constraints cannot replace additional caller-owned requirements."""
import pytest

from prism_sdk.workflow_builder import compile_workflow_plan, execute_workflow_plan
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult
from prism_sdk.workflow_requirements import (
    WorkflowRequirementError, compile_workflow_requirements, merge_workflow_requirements,
)


def _rule(comparison, value):
    return {"operation": "local.read", "path": "/limit", "comparison": comparison, "value": value}


def _contract(*rules, task="Read public glioma metadata", **kwargs):
    return compile_workflow_requirements(task, argument_rules=list(rules), **kwargs)


def _catalog():
    catalog, calls = WorkflowOperationCatalog(), []
    schema = {"type": "object", "required": ["limit"], "properties": {"limit": {"type": "integer"}}, "additionalProperties": False}
    def execute(arguments, **_kwargs):
        calls.append(arguments)
        return WorkflowOperationResult(arguments, {"fixture": True})
    catalog.register(WorkflowOperation("local.read", "Read local fixture", schema, schema, "local", implementation="local"), execute)
    return catalog, calls


def test_merged_plan_preserves_both_contracts_and_tighter_call_budget():
    catalog, calls = _catalog()
    one = _contract(_rule("equals", 2), max_steps=3)
    two = _contract(_rule("at_most", 5), output_operations=["local.read"], max_steps=1, max_calls_by_operation={"local.read": 1})
    merged = merge_workflow_requirements("Read public glioma metadata", one, two, catalog=catalog)
    data = merged.to_dict()
    assert data["max_steps"] == 1 and data["max_calls_by_operation"] == {"local.read": 1}
    assert data["argument_rules"] == [_rule("equals", 2), _rule("at_most", 5)]
    proposal = {"name": "Two constraints", "steps": [{"id": "source", "operation": "local.read", "arguments": {"limit": 2}, "depends_on": []}], "outputs": ["source"]}
    plan = compile_workflow_plan(proposal, catalog, task="Read public glioma metadata", requirements=merged)
    assert execute_workflow_plan(plan, catalog, approved=True).status == "completed"
    assert calls == [{"limit": 2}]
    proposal["steps"][0]["arguments"]["limit"] = 4
    with pytest.raises(ValueError):
        compile_workflow_plan(proposal, catalog, task="Read public glioma metadata", requirements=merged)


@pytest.mark.parametrize("left,right", [
    (_rule("equals", 2), _rule("equals", 3)),
    (_rule("equals", False), _rule("equals", 0)),
    (_rule("equals", None), _rule("equals", 0)),
    (_rule("equals", 2), _rule("at_least", 3)),
    (_rule("at_least", 7), _rule("at_most", 5)),
])
def test_directly_conflicting_predicates_fail_without_choosing_a_winner(left, right):
    with pytest.raises(WorkflowRequirementError, match="conflicting"):
        merge_workflow_requirements("Read public glioma metadata", _contract(left), _contract(right))


def test_repeated_contracts_are_deduplicated_but_distinct_contains_predicates_remain():
    first = {"operation": "local.read", "path": "/tags", "comparison": "contains", "value": "glioma"}
    second = {**first, "value": "public"}
    one, two = _contract(first), _contract(second)
    merged = merge_workflow_requirements("Read public glioma metadata", one, two, one).to_dict()
    assert merged["argument_rules"] == [first, second]
    assert one.to_dict()["argument_rules"] == [first]


def test_forbidden_source_required_by_another_contract_is_a_conflict():
    with pytest.raises(WorkflowRequirementError, match="conflict"):
        merge_workflow_requirements("Read public glioma metadata",
            _contract(_rule("equals", 2)), _contract(forbidden_operations=["local.read"]))


def test_merged_task_identity_and_catalogue_remain_authoritative():
    catalog, calls = _catalog()
    with pytest.raises(WorkflowRequirementError, match="different task"):
        merge_workflow_requirements("Other request", _contract(), catalog=catalog)
    with pytest.raises(WorkflowRequirementError, match="unregistered"):
        merge_workflow_requirements("Read public glioma metadata", _contract(required_operations=["unknown.read"]), catalog=catalog)
    assert calls == []


def test_merge_requires_at_least_one_bounded_contract_and_compatible_call_caps():
    with pytest.raises(WorkflowRequirementError):
        merge_workflow_requirements("Read public glioma metadata")
    with pytest.raises(WorkflowRequirementError):
        merge_workflow_requirements("Read public glioma metadata", *([_contract()] * 17))
    with pytest.raises(WorkflowRequirementError, match="conflict"):
        merge_workflow_requirements("Read public glioma metadata", _contract(required_operations=["local.read"]),
                                    _contract(max_calls_by_operation={"local.read": 0}))
