"""Decoder hints are bounded metadata; compiler/source authority never moves to a provider."""
import json

import pytest

from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.tooling import ToolCatalogue, ToolDefinition
from prism_sdk.workflow_builder import LocalWorkflowBuilder, WorkflowValidationError
from prism_sdk.workflow_decoder import WorkflowDecoderSchemaError, prepare_workflow_decoder_schema
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog
from prism_sdk.workflow_requirements import compile_workflow_requirements


def _catalog(consumer=None):
    catalog = WorkflowOperationCatalog()
    def no_dispatch(*_args, **_kwargs):
        pytest.fail("decoder preparation/planning cannot execute an operation")
    for operation in ("fixture.left", "fixture.right"):
        catalog.register(WorkflowOperation(operation, "Synthetic metadata producer",
            {"type": "object", "properties": {"label": {"type": "string", "minLength": 1}}, "required": ["label"], "additionalProperties": False},
            {"type": "object", "properties": {"metadata": {"type": "object"}}, "required": ["metadata"], "additionalProperties": False},
            "fixture", implementation="local"), no_dispatch)
    catalog.register(WorkflowOperation("fixture.consume", "Consume explicit synthetic objects",
        consumer or {"type": "object", "properties": {"payload": {"type": "object"}, "mode": {"const": "review"}},
                     "required": ["mode"], "additionalProperties": False},
        {"type": "object"}, "fixture", implementation="local"), no_dispatch)
    return catalog


def _contract(links, task="Review a synthetic record."):
    return compile_workflow_requirements(task, output_operations=["fixture.consume"], data_links=links,
        max_steps=3, max_calls_by_operation={"fixture.left": 1, "fixture.right": 1, "fixture.consume": 1})


def _link(path="/payload", pointer="", source="fixture.left"):
    return {"source_operation": source, "target_operation": "fixture.consume", "target_path": path, "source_pointer": pointer}


def _arguments(prepared, operation="fixture.consume"):
    return next(branch["properties"]["arguments"] for branch in prepared.response_schema["properties"]["steps"]["items"]["anyOf"]
                if branch["properties"]["operation"]["const"] == operation)


def _checks(schema, value):
    report = ToolCatalogue.from_definitions((ToolDefinition("fixture.schema", schema),)).validate("fixture.schema", value)
    assert not report.warnings
    return report.ok


@pytest.mark.parametrize("pointer,good,bad", [("", "arbitrary_id#", "arbitrary_id#/metadata"),
    ("/metadata", "different_id#/metadata", "different_id#")])
def test_exact_reference_leaf_selects_root_or_child_without_fixing_a_step_id(pointer, good, bad):
    catalog = _catalog()
    prepared = prepare_workflow_decoder_schema(catalog, ["fixture.left", "fixture.consume"], requirements=_contract([_link(pointer=pointer)]))
    arguments = _arguments(prepared)
    assert _checks(arguments, {"payload": {"$ref": good}, "mode": "review"})
    assert not _checks(arguments, {"payload": {"$ref": bad}, "mode": "review"})
    assert not _checks(arguments, {"payload": {"invented": "source object"}, "mode": "review"})
    assert not _checks(arguments, {"payload": {"$ref": good, "extra": 1}, "mode": "review"})
    assert not _checks(arguments, {"payload": {"$ref": good}, "mode": "wrong"})
    assert not _checks(arguments, {"$ref": good})
    assert prepared.audit["coverage"]["link_hints"][0]["source_identity"] == "compiler_only"


def test_root_target_removes_literal_required_fields_and_remains_reference_only():
    prepared = prepare_workflow_decoder_schema(_catalog(), ["fixture.left", "fixture.consume"], requirements=_contract([_link(path="")]))
    arguments = _arguments(prepared)
    assert _checks(arguments, {"$ref": "any_source#"})
    assert not _checks(arguments, {"$ref": "any_source#", "mode": "review"})
    assert not _checks(arguments, {"mode": "review"})


def test_positional_overlays_preserve_preceding_items_require_each_index_and_do_not_mix_homogeneous_items():
    consumer = {"type": "object", "properties": {"rows": {"type": "array", "maxItems": 4,
        "items": {"type": "object", "properties": {"bundle": {"type": "object"}, "marker": {"const": "native"}},
                  "required": ["marker"], "additionalProperties": False}}}, "additionalProperties": False}
    links = [_link("/rows/0/bundle", ""), _link("/rows/2/bundle", "/metadata", "fixture.right")]
    prepared = prepare_workflow_decoder_schema(_catalog(consumer), ["fixture.left", "fixture.right", "fixture.consume"],
        requirements=_contract(links), prefix_items_supported=True)
    rows = _arguments(prepared)["properties"]["rows"]
    variants = rows["anyOf"]
    assert [branch["minItems"] for branch in variants] == [3, 4]
    assert all(branch["minItems"] == branch["maxItems"] == len(branch["prefixItems"]) and "items" not in branch for branch in variants)
    first, middle, last = variants[0]["prefixItems"]
    assert _checks(first, {"marker": "native", "bundle": {"$ref": "a#"}})
    assert not _checks(first, {"marker": "native", "bundle": {"$ref": "a#/metadata"}})
    assert _checks(middle, {"marker": "native"})
    assert not _checks(middle, {"marker": "changed"})
    assert _checks(last, {"marker": "native", "bundle": {"$ref": "b#/metadata"}})
    assert not _checks(last, {"marker": "native", "bundle": {"$ref": "b#"}})
    assert prepared.audit["coverage"]["positional_array_strategy"][0]["native_max_items"] == 4
    assert prepared.audit["coverage"]["positional_array_strategy"][0]["admitted_lengths"] == [3, 4]
    assert _checks(variants[1]["prefixItems"][3], {"marker": "native"})


def test_pointer_escaping_is_decoded_once_and_numeric_object_names_are_not_array_indices():
    consumer = {"type": "object", "properties": {"a/b": {"type": "object", "properties": {
        "01": {"type": "object", "properties": {"~value": {"type": "object"}}, "additionalProperties": False}},
        "additionalProperties": False}}, "additionalProperties": False}
    prepared = prepare_workflow_decoder_schema(_catalog(consumer), ["fixture.left", "fixture.consume"],
        requirements=_contract([_link("/a~1b/01/~0value")]))
    assert _checks(_arguments(prepared), {"a/b": {"01": {"~value": {"$ref": "id#"}}}})
    assert not _checks(_arguments(prepared), {"a/b": {"1": {"~value": {"$ref": "id#"}}}})


@pytest.mark.parametrize("links,code", [
    ([_link("/payload"), _link("/payload/child")], "parent_child_conflict"),
    ([_link("/payload", ""), _link("/payload", "/metadata")], "pointer_conflict"),
    ([_link("/payload", ""), _link("/payload", "", "fixture.right")], "source_identity_conflict"),
])
def test_conflicting_direct_leaves_refuse_instead_of_last_write_wins(links, code):
    with pytest.raises(WorkflowDecoderSchemaError, match=code):
        prepare_workflow_decoder_schema(_catalog(), ["fixture.left", "fixture.right", "fixture.consume"], requirements=_contract(links))


@pytest.mark.parametrize("consumer,path,code", [
    ({"type": "object", "properties": {}, "additionalProperties": False}, "/absent", "closed_schema"),
    ({"type": "object", "properties": {"payload": {"type": "string"}}}, "/payload/child", "ambiguous_or_scalar"),
    ({"type": "object", "properties": {"payload": False}}, "/payload", "false_input_schema"),
    ({"type": "object", "properties": {"payload": {}}, "maxProperties": 0}, "/payload", "required_properties_exceed"),
    ({"type": "object", "properties": {"rows": {"type": "array", "maxItems": 1, "items": {"type": "object"}}}}, "/rows/1", "exceeds_bound"),
    ({"type": "object", "properties": {"rows": {"type": "array", "items": {"type": "object"}}}}, "/rows/01", "index_invalid"),
    ({"type": "object", "properties": {"rows": {"type": "array", "items": {"type": "object"}}}}, "/rows/-", "index_invalid"),
])
def test_impossible_or_ambiguous_overlay_refuses_before_a_provider_can_be_called(consumer, path, code):
    with pytest.raises(WorkflowDecoderSchemaError, match=code):
        prepare_workflow_decoder_schema(_catalog(consumer), ["fixture.left", "fixture.consume"], requirements=_contract([_link(path)]), prefix_items_supported=True)


def test_array_targets_require_explicit_boolean_capability_before_provider_dispatch():
    consumer = {"type": "object", "properties": {"rows": {"type": "array", "items": {"type": "object"}}}}
    catalog, calls = _catalog(consumer), []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: calls.append(request))
    builder = LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture", constrain_decoder=True)
    with pytest.raises(WorkflowDecoderSchemaError, match="prefix_items_capability_required"):
        builder.build("Review a synthetic record.", requirements=_contract([_link("/rows/0")]), approve_provider_call=True)
    assert calls == []
    with pytest.raises(WorkflowValidationError, match="literal booleans"):
        LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture", constrain_decoder=1)


def test_source_only_link_is_explicitly_compiler_only_and_artifacts_are_immutable_copies():
    link = {"source_operation": "fixture.left", "target_operation": "fixture.consume", "source_pointer": ""}
    prepared = prepare_workflow_decoder_schema(_catalog(), ["fixture.left", "fixture.consume"], requirements=_contract([link]))
    assert prepared.audit["coverage"]["link_hints"] == [{"link_index": 0, "decoder_hint": "not_encoded", "reason": "no_exact_target_path;compiler_only"}]
    original = prepared.to_dict()
    changed = prepared.response_schema
    changed["properties"].clear()
    assert prepared.to_dict() == original


def test_unsupported_schema_or_schema_byte_overflow_refuses_without_generic_fallback():
    catalog = _catalog({"type": "object", "properties": {"value": {"anyOf": [{"type": "string"}, {"type": "integer"}]}}})
    with pytest.raises(WorkflowDecoderSchemaError, match="feature_unsupported"):
        prepare_workflow_decoder_schema(catalog, ["fixture.consume"])
    with pytest.raises(WorkflowDecoderSchemaError, match="byte_bound_exceeded"):
        prepare_workflow_decoder_schema(_catalog(), ["fixture.left", "fixture.consume"], max_schema_bytes=100)


@pytest.mark.parametrize("maximum,code", [(None, "finite_maximum_required"), (20, "positional_array_bound")])
def test_unbounded_or_large_positional_ranges_refuse_instead_of_narrowing_native_lengths(maximum, code):
    rows = {"type": "array", "items": {"type": "object"}}
    if maximum is not None:
        rows["maxItems"] = maximum
    catalog = _catalog({"type": "object", "properties": {"rows": rows}})
    with pytest.raises(WorkflowDecoderSchemaError, match=code):
        prepare_workflow_decoder_schema(catalog, ["fixture.left", "fixture.consume"],
            requirements=_contract([_link("/rows/0")]), prefix_items_supported=True)


def test_a_decoder_ignoring_provider_cannot_bypass_source_identity_or_the_attempt_budget():
    catalog, calls = _catalog(), []
    task = "Review a synthetic record."
    contract = _contract([_link()], task)
    proposal = {"name": "Untrusted provider proposal", "steps": [
        {"id": "left", "operation": "fixture.left", "arguments": {"label": "left"}, "depends_on": []},
        {"id": "right", "operation": "fixture.right", "arguments": {"label": "right"}, "depends_on": []},
        {"id": "consumer", "operation": "fixture.consume", "arguments": {"mode": "review", "payload": {"$ref": "right#"}}, "depends_on": ["left", "right"]}], "outputs": ["consumer"]}
    def model(request):
        calls.append(request)
        return json.dumps(proposal)
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", model)
    builder = LocalWorkflowBuilder(catalog, runtime, provider="fixture", model="fixture", constrain_decoder=True)
    with pytest.raises(WorkflowValidationError, match="fixed task contract"):
        builder.build(task, requirements=contract, approve_provider_call=True, max_planning_attempts=2)
    assert len(calls) == 2
    assert json.loads(calls[0].messages[0]["content"])["fixed_caller_requirements"] == contract.to_dict()


def test_legacy_default_keeps_generic_response_schema_and_output_schemas_in_context():
    catalog, observed = _catalog(), []
    proposal = {"name": "One fixture", "steps": [{"id": "a", "operation": "fixture.left", "arguments": {"label": "synthetic"}, "depends_on": []}], "outputs": ["a"]}
    def model(request):
        observed.append(request)
        return json.dumps(proposal)
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", model)
    LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture").build("Read a synthetic record.", approve_provider_call=True)
    assert "anyOf" not in observed[0].response_schema["properties"]["steps"]["items"]
    instructions = json.loads(observed[0].messages[0]["content"])
    assert "decoder_constraint_audit" not in instructions
    assert all(row["output_schema"] == catalog.resolve(row["operation_id"]).to_dict()["output_schema"] for row in instructions["operations"])


def test_opt_in_changes_decoder_schema_without_changing_the_task_prompt_or_producing_a_plan_itself():
    catalog, observed = _catalog(), []
    proposal = {"name": "Actual fixture proposal", "steps": [{"id": "caller_chosen_id", "operation": "fixture.left", "arguments": {"label": "synthetic"}, "depends_on": []}], "outputs": ["caller_chosen_id"]}
    def model(request):
        observed.append(request)
        return json.dumps(proposal)
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", model)
    for enabled in (False, True):
        plan = LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture", constrain_decoder=enabled).build("Read a synthetic record.", approve_provider_call=True)
        assert plan.to_dict()["name"] == proposal["name"]
    assert observed[0].messages == observed[1].messages
    assert observed[0].response_schema != observed[1].response_schema


def test_oversized_context_refuses_before_provider_instead_of_omitting_output_schemas(monkeypatch):
    import prism_sdk.workflow_builder as module
    catalog, calls = _catalog(), []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("fixture", lambda request: calls.append(request))
    monkeypatch.setattr(module, "MAX_WORKFLOW_PLANNING_CONTEXT_BYTES", 100)
    with pytest.raises(WorkflowValidationError, match="context exceeds"):
        LocalWorkflowBuilder(catalog, runtime, model="fixture", provider="fixture", constrain_decoder=True).build("Read synthetic metadata.", approve_provider_call=True)
    assert calls == []
