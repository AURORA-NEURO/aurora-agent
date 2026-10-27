from __future__ import annotations

from copy import deepcopy
from dataclasses import replace
import http.client
import json

import pytest

from prism_sdk.authoring import content_digest
from prism_sdk.llm_runtime import LLMRuntime, ProviderConfig, ollama_provider
from prism_sdk.workflow_builder import (
    LocalWorkflowBuilder,
    WorkflowValidationError,
    compile_workflow_plan,
    execute_workflow_plan,
    load_workflow_checkpoint,
)
from prism_sdk.workflow_integrations import WorkflowOperation, WorkflowOperationCatalog, WorkflowOperationResult


def _object(properties, required=()):
    return {"type": "object", "properties": properties, "required": list(required), "additionalProperties": False}


def _catalog(*, consumer_schema=None, source_value=None, fail_once=False):
    calls = []
    catalog = WorkflowOperationCatalog()
    value = {"number": 3, "numbers": [1, 2]} if source_value is None else source_value
    source_schema = _object({"number": {"type": "integer"}, "numbers": {"type": "array", "items": {"type": "integer"}}}, ("number", "numbers"))
    target_schema = consumer_schema or _object({"numbers": {"type": "array", "items": {"type": "integer"}}}, ("numbers",))

    def source(arguments, **kwargs):
        calls.append("source")
        if fail_once and calls.count("source") == 1:
            raise RuntimeError("private-provider-secret-body")
        return WorkflowOperationResult(value, {"source": "actual-executed-fixture"})

    def consumer(arguments, **kwargs):
        calls.append("consumer")
        total = sum(item["number"] if isinstance(item, dict) else item for item in arguments["numbers"])
        return WorkflowOperationResult({"count": total}, {"source": "actual-consumer"})

    catalog.register(WorkflowOperation("fixture.source", "Read fixture values", _object({}), source_schema, "fixture", implementation="local"), source)
    catalog.register(WorkflowOperation("fixture.consume", "Consume fixture values", target_schema, _object({"count": {"type": "integer"}}, ("count",)), "fixture", implementation="local"), consumer)
    return catalog, calls


def _proposal(numbers=None):
    if numbers is None:
        numbers = [{"$ref": "source#/number"}, 2]
    return {"name": "Checked reference workflow", "steps": [
        {"id": "source", "operation": "fixture.source", "arguments": {}, "depends_on": []},
        {"id": "consumer", "operation": "fixture.consume", "arguments": {"numbers": numbers}, "depends_on": ["source"]},
    ], "outputs": ["consumer"]}


def _rehash(checkpoint):
    checkpoint["checkpoint_digest"] = content_digest({key: value for key, value in checkpoint.items() if key != "checkpoint_digest"})
    return checkpoint


def test_reference_array_scalar_literals_are_validated_and_actual_values_reach_dependency_execution():
    catalog, calls = _catalog()
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    assert calls == []
    held = execute_workflow_plan(plan, catalog)
    assert held.status == "approval_required"
    assert calls == []
    run = execute_workflow_plan(plan, catalog, approved=True)
    assert run.status == "completed"
    assert run.outputs == {"consumer": {"count": 5}}
    assert calls == ["source", "consumer"]
    assert run.checkpoint["results"]["source"]["receipt"] == {"source": "actual-executed-fixture"}


@pytest.mark.parametrize("numbers", [
    [{"$ref": "source#/number"}, "invalid-literal"],
    [{"$ref": "source#/number"}, True],
    [{"$ref": "source#/number"}, {"unexpected": 2}],
])
def test_reference_array_leaves_do_not_exempt_invalid_literal_siblings(numbers):
    catalog, calls = _catalog()
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(_proposal(numbers), catalog, task="Inspect integers")
    assert calls == []


def test_nested_reference_objects_retain_validation_of_literal_fields():
    schema = _object({"numbers": {"type": "array", "items": _object({"number": {"type": "integer"}}, ("number",))}}, ("numbers",))
    catalog, calls = _catalog(consumer_schema=schema)
    proposal = _proposal([{"number": {"$ref": "source#/number"}}, {"number": 2}])
    plan = compile_workflow_plan(proposal, catalog, task="Read nested integers")
    assert execute_workflow_plan(plan, catalog, approved=True).outputs == {"consumer": {"count": 5}}
    proposal["steps"][1]["arguments"]["numbers"][1]["number"] = "invalid"
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(proposal, catalog, task="Read nested integers")
    assert calls == ["source", "consumer"]


def test_referenced_array_root_resolves_and_closed_schema_pointer_failure_prevents_further_dispatch():
    catalog, calls = _catalog()
    plan = compile_workflow_plan(_proposal({"$ref": "source#/numbers"}), catalog, task="Read the source array")
    assert execute_workflow_plan(plan, catalog, approved=True).outputs == {"consumer": {"count": 3}}
    with pytest.raises(WorkflowValidationError, match="impossible"):
        compile_workflow_plan(_proposal({"$ref": "source#/missing"}), catalog, task="Read a missing source array")
    assert calls == ["source", "consumer"]


def test_reference_in_unknown_property_cannot_bypass_closed_argument_schema():
    catalog, calls = _catalog()
    proposal = _proposal()
    proposal["steps"][1]["arguments"]["unknown"] = {"$ref": "source#/number"}
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(proposal, catalog, task="Inspect source integers")
    assert calls == []


def test_partial_reference_validation_keeps_known_uniqueness_and_parallel_alternatives():
    unique_schema = _object({"numbers": {"type": "array", "items": {"type": "integer"}, "uniqueItems": True}}, ("numbers",))
    catalog, calls = _catalog(consumer_schema=unique_schema)
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(_proposal([{"$ref": "source#/number"}, 2, 2]), catalog, task="Read distinct integers")
    alternatives = _object({"numbers": {"type": "array", "items": {"type": "integer"}}, "marker": {"type": "string"}}, ("numbers", "marker"))
    alternatives["anyOf"] = [{"properties": {"marker": {"const": "allowed"}}}]
    alternatives["oneOf"] = [{"properties": {"numbers": {"type": "array", "items": {"type": kind}}}} for kind in ("integer", "string")]
    catalog, calls = _catalog(consumer_schema=alternatives)
    proposal = _proposal()
    proposal["steps"][1]["arguments"]["marker"] = "invalid"
    with pytest.raises(WorkflowValidationError):
        compile_workflow_plan(proposal, catalog, task="Read constrained integers")
    assert calls == []


def test_normalized_defaults_enter_plan_dispatch_and_checkpoint_argument_identity():
    catalog = WorkflowOperationCatalog()
    calls = []
    source_input = _object({"seed": {"type": "integer", "default": 3}}, ("seed",))
    source_output = _object({"number": {"type": "integer"}}, ("number",))
    target_input = _object({"numbers": {"type": "array", "items": {"type": "integer"}}, "limit": {"type": "integer", "default": 2}}, ("numbers", "limit"))

    def source(arguments, **kwargs):
        calls.append(arguments)
        return WorkflowOperationResult({"number": arguments["seed"]}, {})

    def target(arguments, **kwargs):
        calls.append(arguments)
        return WorkflowOperationResult({"count": sum(arguments["numbers"]) + arguments["limit"]}, {})

    catalog.register(WorkflowOperation("fixture.source", "Default source", source_input, source_output, "fixture", implementation="local"), source)
    catalog.register(WorkflowOperation("fixture.consume", "Default target", target_input, _object({"count": {"type": "integer"}}, ("count",)), "fixture", implementation="local"), target)
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    assert plan.to_dict()["steps"][0]["arguments"] == {"seed": 3}
    assert plan.to_dict()["steps"][1]["arguments"]["limit"] == 2
    checkpoint = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    assert checkpoint["results"]["source"]["arguments_digest"] == content_digest({"seed": 3})
    run = execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint)
    assert calls == [{"seed": 3}, {"numbers": [3, 2], "limit": 2}]
    assert run.outputs == {"consumer": {"count": 7}}
    assert run.checkpoint["results"]["consumer"]["arguments_digest"] == content_digest(calls[1])


def test_whole_argument_reference_defers_defaults_until_the_source_object_resolves():
    target_schema = _object({"numbers": {"type": "array", "items": {"type": "integer"}}, "number": {"type": "integer"}, "limit": {"type": "integer", "default": 2}}, ("numbers", "limit"))
    catalog, calls = _catalog(consumer_schema=target_schema)
    proposal = _proposal()
    proposal["steps"][1]["arguments"] = {"$ref": "source#"}
    plan = compile_workflow_plan(proposal, catalog, task="Consume source objects")
    run = execute_workflow_plan(plan, catalog, approved=True)
    assert run.status == "completed"
    assert run.checkpoint["results"]["consumer"]["arguments_digest"] == content_digest({"numbers": [1, 2], "number": 3, "limit": 2})
    assert calls == ["source", "consumer"]


def test_resume_revalidates_retained_output_schema_even_when_attacker_rehashes_all_values():
    catalog, calls = _catalog()
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    checkpoint = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    row = checkpoint["results"]["source"]
    row["value"]["number"] = "not-an-integer"
    row["value_digest"] = content_digest(row["value"])
    _rehash(checkpoint)
    with pytest.raises(WorkflowValidationError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint)
    assert calls == ["source"]


@pytest.mark.parametrize("mutate", [
    lambda value: value.update(status="silently_unknown"),
    lambda value: value.update(status="completed"),
    lambda value: value.update(status="ready"),
    lambda value: value.update(status="retry_required"),
    lambda value: value.update(in_flight="source"),
    lambda value: value["attempts"].update(consumer=1),
    lambda value: value.update(failure={"step_id": "source", "error_type": "RuntimeError"}, status="failed"),
])
def test_rehashed_checkpoint_cannot_drop_or_fabricate_execution_state(mutate):
    catalog, calls = _catalog()
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    checkpoint = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    mutate(checkpoint)
    _rehash(checkpoint)
    with pytest.raises(WorkflowValidationError):
        execute_workflow_plan(plan, catalog, approved=True, checkpoint=checkpoint)
    assert calls == ["source"]


def test_in_flight_checkpoint_requires_explicit_retry_and_does_not_repeat_completed_sources():
    catalog, calls = _catalog()
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    captured = []

    def persistence_failure(checkpoint):
        captured.append(deepcopy(checkpoint))
        raise RuntimeError("private-persistence-secret-body")

    with pytest.raises(RuntimeError):
        execute_workflow_plan(plan, catalog, approved=True, on_checkpoint=persistence_failure)
    assert calls == []
    interrupted = captured[0]
    assert interrupted["in_flight"] == "source"
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=interrupted).status == "retry_required"
    assert calls == []
    run = execute_workflow_plan(plan, catalog, approved=True, checkpoint=interrupted, retry_failed=True)
    assert run.status == "completed"
    assert run.outputs == {"consumer": {"count": 5}}
    assert calls == ["source", "consumer"]
    replay = execute_workflow_plan(plan, catalog, approved=True, checkpoint=run.checkpoint)
    assert replay.outputs == run.outputs
    assert calls == ["source", "consumer"]


def test_executor_failures_remain_failures_without_exception_body_or_synthetic_values():
    catalog, calls = _catalog(fail_once=True)
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    failed = execute_workflow_plan(plan, catalog, approved=True)
    assert failed.status == "failed"
    assert failed.outputs == {}
    assert "private-provider-secret-body" not in json.dumps(failed.checkpoint)
    assert execute_workflow_plan(plan, catalog, approved=True, checkpoint=failed.checkpoint).status == "retry_required"
    assert calls == ["source"]
    run = execute_workflow_plan(plan, catalog, approved=True, checkpoint=failed.checkpoint, retry_failed=True)
    assert run.outputs == {"consumer": {"count": 5}}
    assert calls == ["source", "source", "consumer"]


def test_checkpoint_loader_rejects_duplicate_json_keys_even_when_last_value_has_valid_digest(tmp_path):
    catalog, _calls = _catalog()
    plan = compile_workflow_plan(_proposal(), catalog, task="Inspect source integers")
    checkpoint = execute_workflow_plan(plan, catalog, approved=True, max_step_calls=1).checkpoint
    encoded = json.dumps(checkpoint, separators=(",", ":")).replace('"status":"paused"', '"status":"ready","status":"paused"')
    target = tmp_path / "duplicate-checkpoint.json"
    target.write_text(encoded, encoding="utf-8")
    with pytest.raises(WorkflowValidationError, match="duplicate"):
        load_workflow_checkpoint(target)


def test_in_memory_planner_uses_runtime_metadata_and_requires_approval_before_invocation():
    catalog, calls = _catalog()
    provider_calls = []
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("local", lambda request: provider_calls.append(request) or json.dumps(_proposal()))
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    with pytest.raises(WorkflowValidationError, match="approval"):
        builder.build("Inspect source integers")
    assert provider_calls == calls == []
    plan = builder.build("Inspect source integers", approve_provider_call=True)
    assert len(provider_calls) == 1
    assert calls == []
    assert execute_workflow_plan(plan, catalog, approved=True).outputs == {"consumer": {"count": 5}}


def test_planner_refuses_provider_replacement_with_remote_transport_before_request():
    catalog, calls = _catalog()
    runtime = LLMRuntime()
    runtime.register_in_memory_provider("local", lambda _request: pytest.fail("replaced provider handler was called"))
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    runtime.register_provider(ProviderConfig(provider="local", base_url="https://remote.invalid", requires_credential=False))
    with pytest.raises(WorkflowValidationError, match="loopback"):
        builder.build("Inspect source integers", approve_provider_call=True)
    assert calls == []


def test_loopback_ollama_provider_metadata_is_accepted_without_discovery_or_invocation():
    catalog, calls = _catalog()
    runtime = LLMRuntime()
    runtime.register_provider(ollama_provider(max_attempts=1))
    builder = LocalWorkflowBuilder(catalog, runtime, model="locally-installed-model")
    assert builder.provider == "ollama"
    assert calls == []


def test_planner_rejects_duplicate_model_json_fields_before_compilation():
    catalog, calls = _catalog()
    runtime = LLMRuntime()
    proposal_text = json.dumps(_proposal()).replace('"name": "Checked reference workflow"', '"name": "hidden", "name": "Checked reference workflow"')
    runtime.register_in_memory_provider("local", lambda _request: proposal_text)
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    with pytest.raises(WorkflowValidationError):
        builder.build("Inspect source integers", approve_provider_call=True)
    assert calls == []


@pytest.mark.parametrize("changes", [
    {"timeout_seconds": 1800}, {"max_response_bytes": 99_999_999},
    {"allow_insecure_http": True}, {"api_key_header": "X-Changed-Key"},
])
@pytest.mark.parametrize("stage", ["before_request", "during_response"])
def test_full_provider_controls_cannot_change_under_the_same_presentation_metadata(changes, stage):
    catalog, source_calls = _catalog()
    runtime, provider_calls = LLMRuntime(), []
    def handler(request):
        provider_calls.append(request)
        if stage == "during_response":
            runtime.register_provider(replace(config, **changes))
        return json.dumps(_proposal())
    config = runtime.register_in_memory_provider("local", handler)
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    changed = replace(config, **changes)
    assert changed.to_metadata() == config.to_metadata()
    if stage == "before_request":
        runtime.register_provider(changed)
    with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
        builder.build("Inspect source integers", approve_provider_call=True, max_planning_attempts=4)
    assert len(provider_calls) == (0 if stage == "before_request" else 1)
    assert source_calls == []


def test_an_equal_registered_config_replacement_and_equal_numeric_type_mutation_are_detected():
    catalog, source_calls = _catalog()
    for kind in ("equal_registration", "same_object_numeric_type"):
        runtime, provider_calls = LLMRuntime(), []
        config = runtime.register_in_memory_provider("local", lambda request: provider_calls.append(request) or json.dumps(_proposal()))
        builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
        if kind == "equal_registration":
            runtime.register_provider(replace(config))
        else:
            object.__setattr__(config, "max_response_bytes", float(config.max_response_bytes))
        with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
            builder.build("Inspect source integers", approve_provider_call=True)
        assert provider_calls == source_calls == []


@pytest.mark.parametrize("field", ["_handler", "_stream_handler", "_model_discovery_handler", "invoke"])
def test_replacing_a_registered_in_memory_callable_cannot_dispatch_another_implementation(field):
    catalog, source_calls = _catalog()
    runtime, provider_calls = LLMRuntime(), []
    config = runtime.register_in_memory_provider("local", lambda request: provider_calls.append(request) or json.dumps(_proposal()))
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    setattr(config.transport, field, lambda *_a: pytest.fail("replacement callable dispatched"))
    with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
        builder.build("Inspect source integers", approve_provider_call=True)
    assert provider_calls == source_calls == []


def test_replacing_http_constructor_is_detected_without_discovery_or_a_connection(monkeypatch):
    catalog, source_calls = _catalog()
    runtime = LLMRuntime()
    runtime.register_provider(ollama_provider(max_attempts=1))
    builder = LocalWorkflowBuilder(catalog, runtime, model="fixture-local")
    monkeypatch.setattr(http.client, "HTTPConnection", lambda *_a, **_k: pytest.fail("changed HTTP factory called"))
    with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
        builder.build("Inspect source integers", approve_provider_call=True)
    assert source_calls == []


@pytest.mark.parametrize("stage", ["reserve", "mark_dispatched", "clock", "settle"])
def test_provider_binding_is_checked_across_runtime_callbacks_before_concrete_dispatch(stage):
    catalog, source_calls = _catalog()
    provider_calls, quota_events = [], []
    def change():
        object.__setattr__(config, "timeout_seconds", 1800)
    class Reservation:
        def mark_dispatched(self):
            quota_events.append("mark_dispatched")
            if stage == "mark_dispatched": change()
        def release(self): quota_events.append("release")
        def settle(self, *_args):
            quota_events.append("settle")
            if stage == "settle": change()
    class Quota:
        def reserve(self, _metadata):
            if stage == "reserve": change()
            return Reservation()
    def clock():
        if stage == "clock": change()
        return 100.0
    runtime = LLMRuntime(provider_quota=Quota(), clock=clock)
    config = runtime.register_in_memory_provider("local", lambda request: provider_calls.append(request) or json.dumps(_proposal()))
    builder = LocalWorkflowBuilder(catalog, runtime, provider="local", model="fixture-local")
    with pytest.raises(WorkflowValidationError, match="provider configuration changed"):
        builder.build("Inspect source integers", approve_provider_call=True, max_planning_attempts=4)
    assert len(provider_calls) == (1 if stage == "settle" else 0)
    assert source_calls == []
    assert ("release" in quota_events) if stage == "reserve" else ("settle" in quota_events or "release" in quota_events)
